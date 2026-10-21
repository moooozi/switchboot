"""Prepare an immutable Switchboot release; push only after confirmation.

Requires Python 3.11+ and Git. Cargo validates the updated lockfile when available.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_FILES = ["Cargo.toml", "Cargo.lock"]
SEMVER = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", re.ASCII)


def run(*command):
    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        env={**os.environ, "GIT_EDITOR": "true"},
    ).stdout.strip()


def git(*arguments):
    return run("git", "--no-optional-locks", *arguments)


def optional_git(*arguments):
    try:
        return git(*arguments)
    except subprocess.CalledProcessError:
        return ""


def version_tuple(value):
    if not SEMVER.fullmatch(value):
        raise ValueError(f"Invalid version: {value}; expected X.Y.Z")
    return tuple(map(int, value.split(".")))


def patch_version(text, version, section):
    # Patch only the requested table, retaining comments and unrelated versions.
    pattern = rf"(^\[{re.escape(section)}\][^\n]*\n)(.*?)(?=^\[|\Z)"
    matches = list(re.finditer(pattern, text, re.MULTILINE | re.DOTALL))
    if len(matches) != 1:
        raise ValueError(f"Expected exactly one [{section}] table")
    match = matches[0]
    body, count = re.subn(
        r'^(version\s*=\s*)"[^"]+"',
        lambda m: f'{m[1]}"{version}"',
        match[2],
        flags=re.MULTILINE,
    )
    if count != 1:
        raise ValueError(f"Expected exactly one version in [{section}]")
    return text[: match.start(2)] + body + text[match.end(2) :]


def sync_lock(text, name, current, version):
    entries = list(
        re.finditer(
            r"^\[\[package\]\][^\n]*\n.*?(?=^\[\[package\]\]|\Z)",
            text,
            re.MULTILINE | re.DOTALL,
        )
    )
    local = []
    for match in entries:
        package = tomllib.loads(match[0])["package"][0]
        if package.get("name") == name and "source" not in package:
            local.append((match, package))
    if len(local) != 1 or local[0][1].get("version") != current:
        raise ValueError(
            f"Expected exactly one local {name} lock entry matching {current}"
        )
    match = local[0][0]
    patched, count = re.subn(
        r'^(version\s*=\s*)"[^"]+"',
        lambda m: f'{m[1]}"{version}"',
        match[0],
        flags=re.MULTILINE,
    )
    if count != 1:
        raise ValueError("Expected one local lock version")
    return text[: match.start()] + patched + text[match.end() :]


def write_text(path, text):
    path.write_bytes(text.encode("utf-8"))


def release(args):
    if args.force_retag:
        raise ValueError(
            "Release tags are immutable. Rerun CI or release a new version."
        )
    manifest_path = ROOT / "Cargo.toml"
    # Parse only the package table here. Full Cargo validation happens inside the
    # recoverable preparation block, including for a malformed dependency table.
    manifest = manifest_path.read_bytes().decode("utf-8")
    package_match = re.search(
        r"^\[package\][^\n]*\n.*?(?=^\[|\Z)", manifest, re.MULTILINE | re.DOTALL
    )
    if package_match is None:
        raise ValueError("Missing [package] table")
    package = tomllib.loads(package_match[0])["package"]
    current = package["version"]
    old = version_tuple(current)
    if args.version in ("patch", "minor", "major"):
        major, minor, patch = old
        next_version = {
            "patch": f"{major}.{minor}.{patch + 1}",
            "minor": f"{major}.{minor + 1}.0",
            "major": f"{major + 1}.0.0",
        }[args.version]
    else:
        next_version = args.version
    next_tuple = version_tuple(next_version)
    tags = [
        tag[1:]
        for tag in git("tag", "--list", "v*").splitlines()
        if SEMVER.fullmatch(tag[1:])
    ]
    if next_tuple <= old or any(next_tuple <= version_tuple(tag) for tag in tags):
        raise ValueError(
            "New version must be greater than the current version and existing release tags"
        )
    if git("rev-parse", "--abbrev-ref", "HEAD") != "main":
        raise ValueError("Releases must be made from main")
    tag = f"v{next_version}"
    if args.push and not args.dry_run:
        if git("ls-remote", "--tags", "origin", f"refs/tags/{tag}"):
            raise ValueError(
                f"Remote tag {tag} already exists; release tags are immutable"
            )
        try:
            remote_head = git(
                "ls-remote", "--heads", "origin", "refs/heads/main"
            ).split()[0]
            git("merge-base", "--is-ancestor", remote_head, "HEAD")
        except (subprocess.CalledProcessError, IndexError) as error:
            raise ValueError(
                "Cannot verify origin/main. Fetch origin and resolve any divergence before releasing."
            ) from error
    if args.verify and git("status", "--porcelain"):
        raise ValueError("Working tree is not clean. Commit or stash changes first.")
    sign = args.sign
    if sign is None:
        sign = optional_git("config", "--get", "commit.gpgsign") == "true" and bool(
            optional_git("config", "--get", "user.signingkey")
        )
    print(f"Releasing {current} -> {next_version}\n  tag: {tag} (GPG-signed: {sign})")
    if args.dry_run:
        print("--dry-run: preview only; no files, commits, tags, or remotes changed.")
        return 0
    if git("status", "--porcelain", "--", *VERSION_FILES):
        raise ValueError("Version files must be clean, even with --no-verify")
    head = git("rev-parse", "HEAD")
    originals = {name: (ROOT / name).read_bytes() for name in VERSION_FILES}
    try:
        write_text(manifest_path, patch_version(manifest, next_version, "package"))
        write_text(
            ROOT / "Cargo.lock",
            sync_lock(
                originals["Cargo.lock"].decode("utf-8"),
                package["name"],
                current,
                next_version,
            ),
        )
        if shutil.which("cargo"):
            run(
                "cargo",
                "metadata",
                "--offline",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            )
        else:
            print(
                "Cargo unavailable: validated local lock entry; run cargo check --locked before publishing.",
                file=sys.stderr,
            )
        git("add", "--", *VERSION_FILES)
        git(
            "commit",
            "--only",
            "-m",
            args.message or f"chore(release): {tag}",
            "--",
            *VERSION_FILES,
        )
        git("tag", "-s" if sign else "-a", "-m", args.message or f"Release {tag}", tag)
    except Exception as error:
        if git("rev-parse", "HEAD") == head:
            for name, contents in originals.items():
                (ROOT / name).write_bytes(contents)
            git("restore", "--staged", "--", *VERSION_FILES)
            print(
                "Release preparation failed; original version files restored. No release commit was created.",
                file=sys.stderr,
            )
        else:
            print(
                f"Release commit retained. Inspect HEAD, then create the missing {tag} tag on that commit; do not bump again or replace an existing tag.",
                file=sys.stderr,
            )
        print_error(error)
        # Restore state for every failure, then surface unexpected programming errors.
        if not isinstance(error, (OSError, ValueError, subprocess.SubprocessError)):
            raise
        return 1
    print(
        f"Created release commit and annotated tag {tag}. Nothing has been pushed yet."
    )
    push = args.push and args.yes
    if args.push and not args.yes and sys.stdin.isatty():
        answer = ""
        while answer not in ("y", "n"):
            try:
                answer = (
                    input(f"Push main and {tag} to origin now? (y/n) ").strip().lower()
                )
            except (EOFError, KeyboardInterrupt):
                answer = "n"
        push = answer == "y"
    command = f"git push --atomic origin main {tag}"
    if push:
        try:
            git("push", "--atomic", "origin", "main", tag)
        except subprocess.CalledProcessError as error:
            print(
                f"Push failed; local commit and tag retained. Check remote refs before retrying: {command}",
                file=sys.stderr,
            )
            print_error(error)
            return 1
        print(
            "Pushed. The release pipeline will reuse validated builds for a draft release."
        )
    else:
        print(f"Not pushed. When ready, run:\n  {command}")
    return 0


def print_error(error):
    print(
        error.stderr.strip()
        if isinstance(error, subprocess.CalledProcessError) and error.stderr
        else str(error),
        file=sys.stderr,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", help="patch, minor, major, or X.Y.Z")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--no-push", dest="push", action="store_false")
    parser.add_argument("-y", "--yes", action="store_true")
    parser.add_argument("--no-verify", dest="verify", action="store_false")
    signing = parser.add_mutually_exclusive_group()
    signing.add_argument("--sign", dest="sign", action="store_true")
    signing.add_argument("--no-sign", dest="sign", action="store_false")
    parser.set_defaults(sign=None)
    parser.add_argument("-m", "--message")
    parser.add_argument("--force-retag", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not args.version and not args.force_retag:
        parser.error("specify patch, minor, major, or X.Y.Z")
    try:
        return release(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print_error(error)
        return 2


if __name__ == "__main__":
    sys.exit(main())
