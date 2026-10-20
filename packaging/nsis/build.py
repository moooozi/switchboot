"""Build the native installer (or an isolated preview). Never execute Setup."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tomllib
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent / "toolkit"))
from build_support import build_lock, build_plugin, find_makensis, verify_payload_architecture, verify_unelevated_manifest

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
TOOLKIT = HERE / "toolkit"


def build(output):
    makensis = find_makensis()
    subprocess.run(["pnpm", "icons"], cwd=ROOT, shell=True, check=True, timeout=120)
    subprocess.run(["node", "packaging/generate-app-icons.mjs"], cwd=ROOT, check=True, timeout=60)
    subprocess.run(["cargo", "build", "--locked", "--release", "--bin", "switchboot", "--target", "x86_64-pc-windows-msvc", "--target-dir", str(ROOT / "target")], cwd=ROOT, check=True, timeout=600)
    dll = build_plugin(features="installer")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    payload = ROOT / "target/x86_64-pc-windows-msvc/release/switchboot.exe"
    verify_payload_architecture(payload)
    defines = [f"-DVERSION={version}", f"-DOUTPUT={output}", f"-DPAYLOAD_EXE={payload}", f"-DPLUGIN_DIRECTORY={dll.parent}"]
    script = HERE / "setup.nsi"
    icons = sorted((ROOT / "crates/switchboot_core/resources/icons/ico").glob("*.ico"))
    if not icons:
        raise SystemExit("Generated shortcut icons are missing")
    (HERE / "owned-icons.nsh").write_text(
        '; Generated owned payload list, shared by registry publication and uninstall.\n!macro AppOwnedFiles CALLBACK\n'
        + ''.join(f'  !insertmacro ${{CALLBACK}} "resources\\icons\\ico\\{icon.name}"\n' for icon in icons)
        + '!macroend\n', encoding="utf-8")
    subprocess.run([makensis, "-INPUTCHARSET", "UTF8", "-WX", "-V3", *defines, str(script)], cwd=ROOT, check=True, timeout=120)
    verify_unelevated_manifest(output)
    output.with_suffix(".build.json").write_text(json.dumps({
        "renderer": "svg", "real_installer": True,
        "installer_bytes": output.stat().st_size, "ui_dll_bytes": dll.stat().st_size,
    }, indent=2), encoding="utf-8")
    print(f"Built {output} ({output.stat().st_size} bytes); UI DLL: {dll.stat().st_size} bytes")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preview", action="store_true", help="Build a harmless fixture, not the installer")
    parser.add_argument("--renderer", choices=["raster", "svg"], default="svg")
    parser.add_argument("--maintenance", action="store_true")
    parser.add_argument("--output", type=Path, help="Production installer output path")
    args = parser.parse_args()
    if not args.preview and (args.maintenance or args.renderer != "svg"):
        parser.error("Maintenance/raster modes require --preview")
    if args.preview and args.output:
        parser.error("--output applies only to the production installer")
    if os.name != "nt":
        raise SystemExit("NSIS packaging requires Windows and the MSVC toolchain.")
    (ROOT / "artifacts").mkdir(exist_ok=True)
    if args.preview:
        command = [sys.executable, str(TOOLKIT / "build_preview.py"), "--renderer", args.renderer]
        if args.maintenance:
            command.append("--maintenance")
        subprocess.run(command, check=True)
    else:
        output = (args.output or ROOT / "artifacts/Switchboot_x64-nsis-setup.exe").resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        with build_lock():
            build(output)


if __name__ == "__main__":
    main()
