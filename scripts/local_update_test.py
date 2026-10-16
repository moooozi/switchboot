"""Isolated Windows updater builds and a bounded loopback HTTPS test feed.

Run with: uv run --no-project python scripts/local_update_test.py --help
Never reads production signing secrets or changes the system certificate store.
"""
import argparse

from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import shutil
import ssl
import subprocess
import threading
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
ENV = ROOT / "artifacts/local-update-test"


def run(args, cwd=ROOT, timeout=1800):
    subprocess.run([str(arg) for arg in args], cwd=cwd, check=True, timeout=timeout)


def replace_once(path, old, new):
    text = path.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise ValueError(f"Expected exactly one patch anchor in {path}: {old!r}")
    path.write_text(text.replace(old, new), encoding="utf-8")


def versions(current, requested):
    pattern = r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    if not re.fullmatch(pattern, current):
        raise ValueError("The baseline must be a stable X.Y.Z version")
    parts = tuple(map(int, current.split(".")))
    target = requested or f"{parts[0]}.{parts[1]}.{parts[2] + 1}"
    if not re.fullmatch(pattern, target) or tuple(map(int, target.split("."))) <= parts:
        raise ValueError("Target must be a stable version newer than the baseline")
    return target


def snapshot(destination):
    # Include current edits and untracked implementation, but not ignored secrets,
    # artifacts or build output. Copy ignored generated icons explicitly below.
    output = subprocess.check_output(
        ["git", "--no-pager", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
    )
    for name in set(os.fsdecode(output).split("\0")) - {""}:
        source = ROOT / name
        if source.is_file():
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
    for name in ["issconfig", "crates/switchboot_core/icons",
                 "crates/switchboot_core/resources/icons/ico"]:
        source = ROOT / name
        if not source.is_dir():
            raise ValueError(f"Missing {source}; initialize submodules/generate icons first")
        shutil.copytree(source, destination / name, dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns(".git", "__pycache__", "output"))


def patch_snapshot(source, url, key, certificate):
    lib = source / "crates/switchboot_updater/src/lib.rs"
    text = lib.read_text(encoding="utf-8")
    text, count = re.subn(r'^const FEED: &str = .*;$',
                         f'const FEED: &str = "{url}/latest.json";', text, flags=re.M)
    if count != 1:
        raise ValueError("Feed patch anchor changed")
    text, count = re.subn(r'^const PUBLIC_KEY: &str = .*;$',
                         f'const PUBLIC_KEY: &str = "{key}";', text, flags=re.M)
    if count != 1:
        raise ValueError("Public key patch anchor changed")
    lib.write_text(text, encoding="utf-8")
    download = source / "crates/switchboot_updater/src/download.rs"
    replace_once(download, ".https_only(true)",
                 '.https_only(true)\n                .no_proxy()\n'
                 '                .add_root_certificate(reqwest::Certificate::from_pem('
                 'include_bytes!("local-test-ca.pem"))?)')
    shutil.copy2(certificate, download.parent / "local-test-ca.pem")


HELPER = r'''
use base64::{Engine, engine::general_purpose::STANDARD};
use minisign::{KeyPair, PublicKey, SecretKeyBox};
use std::{error::Error as StdError, fs, io::Cursor};
use switchboot_updater::{Error, Release, Result};
#[path = "../../source/crates/switchboot_updater/src/download.rs"] mod download;
#[path = "../../source/crates/switchboot_updater/src/manifest.rs"] mod manifest;
#[path = "../../source/crates/switchboot_updater/src/signature.rs"] mod signature;
fn main() -> std::result::Result<(), Box<dyn StdError>> {
    let args: Vec<String> = std::env::args().collect();
    match args[1].as_str() {
        "keys" => {
            let pair = KeyPair::generate_encrypted_keypair(Some(String::new()))?;
            fs::write(&args[2], pair.sk.to_box(None)?.to_string())?;
            fs::write(&args[3], STANDARD.encode(pair.pk.to_box()?.to_string()))?;
        }
        "sign" => {
            let secret = SecretKeyBox::from_string(&fs::read_to_string(&args[2])?)?
                .into_secret_key(Some(String::new()))?;
            let public = PublicKey::from_secret_key(&secret)?;
            let bytes = fs::read(&args[3])?;
            let sig = minisign::sign(Some(&public), &secret, Cursor::new(&bytes), None, None)?;
            fs::write(&args[4], STANDARD.encode(sig.to_string()))?;
        }
        "probe" => {
            let current = semver::Version::parse(&args[4])?;
            let client = download::Client::new(&current)?;
            let update = manifest::parse(&client.manifest(&args[2])?, &current)?
                .ok_or("No newer local update")?;
            let bytes = client.installer(update.url, &|_, _| {})?;
            signature::verify(&bytes, &update.signature, &fs::read_to_string(&args[3])?)?;
            if !bytes.starts_with(b"MZ") { return Err("Not a Windows EXE".into()); }
            println!("Verified signed local installer {} ({} bytes); NOT launched.",
                     update.release.version, bytes.len());
        }
        _ => return Err("Unknown helper command".into()),
    }
    Ok(())
}
'''


def make_helper():
    helper = ENV / "helper"
    (helper / "src").mkdir(parents=True)
    # Paths in HELPER are relative to helper/src, hence ../../source.
    (helper / "src/main.rs").write_text(HELPER, encoding="utf-8")
    (helper / "Cargo.toml").write_text('''[package]
name = "switchboot-local-update-helper"
version = "0.1.0"
edition = "2024"
[workspace]
[dependencies]
base64 = "0.22"
minisign = "0.9"
minisign-verify = "0.2"
semver = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] }
switchboot_updater = { path = "../source/crates/switchboot_updater" }
''', encoding="utf-8")
    return helper


def compiler():
    found = shutil.which("iscc")
    if not found:
        found = str(Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)"))
                    / "Inno Setup 6/ISCC.exe")
    if not Path(found).is_file():
        raise ValueError("Inno Setup 6 compiler not found")
    return found


def set_version(source, old, new):
    replace_once(source / "Cargo.toml", f'version = "{old}"', f'version = "{new}"')
    # The workspace lock must track the temporary root version as well.
    replace_once(source / "Cargo.lock", f'name = "switchboot"\nversion = "{old}"',
                 f'name = "switchboot"\nversion = "{new}"')
    package = source / "package.json"
    text = package.read_text(encoding="utf-8")
    text, count = re.subn(r'("version"\s*:\s*)"[^"]+"', rf'\g<1>"{new}"', text, count=1)
    if count != 1:
        raise ValueError("Missing package version")
    package.write_text(text, encoding="utf-8")


def prepare(args):
    if os.name != "nt":
        raise ValueError("Preparing native test installers requires Windows")
    iscc = compiler()
    for tool in ["cargo", "git", "openssl"]:
        if not shutil.which(tool):
            raise ValueError(f"Required tool not found: {tool}")
    if ENV.exists():
        raise ValueError(f"{ENV} already exists. Remove it explicitly to rebuild with fresh keys.")
    current = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    target = versions(current, args.target_version)
    ENV.mkdir(parents=True)
    source = ENV / "source"
    snapshot(source)
    private = ENV / "private"
    private.mkdir()
    ca, ca_key = private / "ca.pem", private / "ca-key.pem"
    cert, cert_key = private / "server.pem", private / "server-key.pem"
    run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-noenc", "-days", "30",
         "-subj", "/CN=Switchboot local test CA", "-addext", "basicConstraints=critical,CA:TRUE",
         "-keyout", ca_key, "-out", ca])
    run(["openssl", "req", "-new", "-newkey", "rsa:2048", "-noenc", "-subj", "/CN=localhost",
         "-keyout", cert_key, "-out", private / "server.csr"])
    extensions = private / "server.ext"
    extensions.write_text("basicConstraints=critical,CA:FALSE\n"
                          "subjectAltName=DNS:localhost,IP:127.0.0.1\n"
                          "extendedKeyUsage=serverAuth\n", encoding="ascii")
    run(["openssl", "x509", "-req", "-in", private / "server.csr", "-CA", ca,
         "-CAkey", ca_key, "-CAcreateserial", "-days", "30", "-extfile", extensions, "-out", cert])
    url = f"https://localhost:{args.port}"
    # Compile helper before patching in the certificate/key. Then rebuild its
    # probe after patching, so it uses exactly the client's test TLS behavior.
    helper = make_helper()
    target_dir = ENV / "target"
    run(["cargo", "build", "--manifest-path", helper / "Cargo.toml", "--target-dir", target_dir])
    binary = target_dir / "debug/switchboot-local-update-helper.exe"
    run([binary, "keys", private / "update.key", private / "update.pub"])
    patch_snapshot(source, url, (private / "update.pub").read_text(), ca)
    # Make test builds unmistakable in the GUI. Never change installation identity:
    # this intentionally exercises the real AppId and service on the local machine.
    replace_once(source / "crates/switchboot_ui/src/lib.rs", 'text("Switchboot").size(30)',
                 'text("Switchboot — LOCAL UPDATE TEST").size(30)')
    site = ENV / "site"
    site.mkdir()
    for version, label in [(current, "baseline"), (target, "target")]:
        if label == "target":
            set_version(source, current, target)
        run(["cargo", "build", "--release", "--bin", "switchboot", "--locked"], cwd=source)
        run([iscc, f"/DVERSION={version}", "packaging/windows.iss"], cwd=source)
        shutil.copy2(source / "artifacts/Switchboot_x64-setup.exe", ENV / f"{label}-setup.exe")
    shutil.copy2(ENV / "target-setup.exe", site / "setup.exe")
    run(["cargo", "build", "--manifest-path", helper / "Cargo.toml", "--target-dir", target_dir])
    run([binary, "sign", private / "update.key", site / "setup.exe", private / "setup.sig"])
    manifest = {"version": target, "notes": "LOCAL TEST ONLY — disposable signing key",
                "platforms": {"windows-x86_64": {
                    "url": f"{url}/setup.exe",
                    "signature": (private / "setup.sig").read_text()}}}
    (site / "latest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    (ENV / "config.json").write_text(json.dumps({"baseline": current, "target": target,
        "port": args.port, "feed": f"{url}/latest.json"}, indent=2), encoding="utf-8")
    print(f"Prepared {current} -> {target} in {ENV}. No installer has been run.")
    probe(None)


def load_config():
    return json.loads((ENV / "config.json").read_text(encoding="utf-8"))


class FeedHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        # Serve no source, keys, directory listings or arbitrary filesystem paths.
        if self.path not in ("/latest.json", "/setup.exe"):
            self.send_error(404)
            return
        super().do_GET()

    def do_HEAD(self):
        if self.path not in ("/latest.json", "/setup.exe"):
            self.send_error(404)
            return
        super().do_HEAD()


class LocalServer(ThreadingHTTPServer):
    daemon_threads = True


def server(config):
    httpd = LocalServer(("127.0.0.1", config["port"]), partial(FeedHandler, directory=ENV / "site"))
    try:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(ENV / "private/server.pem", ENV / "private/server-key.pem")
        httpd.socket = context.wrap_socket(httpd.socket, server_side=True)
        httpd.timeout = 1
        return httpd
    except BaseException:
        httpd.server_close()
        raise


def serve(args):
    config = load_config()
    with server(config) as httpd:
        print(f"Serving {config['feed']} for at most {args.seconds} seconds. Ctrl+C to stop.", flush=True)
        deadline = time.monotonic() + args.seconds
        while time.monotonic() < deadline:
            httpd.handle_request()


def probe(_args):
    config = load_config()
    with server(config) as httpd:
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            run([ENV / "target/debug/switchboot-local-update-helper.exe", "probe", config["feed"],
                 ENV / "private/update.pub", config["baseline"]], timeout=120)
        finally:
            httpd.shutdown()
            thread.join(timeout=5)


def install(args):
    if not args.confirm_machine_change:
        raise ValueError("This replaces the machine's installed Switchboot and changes its service. "
                         "Repeat with --confirm-machine-change only after reading docs/local-update-test.md.")
    load_config()
    print("Launching BASELINE installer. No reboot or firmware operation is requested.", flush=True)
    run([ENV / "baseline-setup.exe", "/NORESTART", f"/LOG={ENV / 'baseline-install.log'}"], timeout=600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare", help="Build isolated baseline/target; never install")
    prepare_parser.add_argument("--target-version")
    prepare_parser.add_argument("--port", type=int, default=18443)
    prepare_parser.set_defaults(action=prepare)
    serve_parser = commands.add_parser("serve", help="Serve loopback HTTPS, bounded runtime")
    serve_parser.add_argument("--seconds", type=int, default=1800)
    serve_parser.set_defaults(action=serve)
    commands.add_parser("probe", help="Download and verify via production modules; never launch").set_defaults(action=probe)
    install_parser = commands.add_parser("install-baseline", help="Explicitly replace the local installed app")
    install_parser.add_argument("--confirm-machine-change", action="store_true")
    install_parser.set_defaults(action=install)
    args = parser.parse_args()
    if hasattr(args, "port") and not 1024 <= args.port <= 65535:
        parser.error("Port must be between 1024 and 65535")
    if hasattr(args, "seconds") and not 1 <= args.seconds <= 86400:
        parser.error("Server lifetime must be between 1 and 86400 seconds")
    try:
        args.action(args)
    except KeyboardInterrupt:
        print("Stopped.")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Local update test failed: {error}\n")


if __name__ == "__main__":
    main()
