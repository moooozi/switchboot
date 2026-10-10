#!/usr/bin/env python3
"""Generate an updater feed without silently downgrading the published version."""
import argparse
import datetime
import json
from pathlib import Path
import re


def parse_version(value):
    if not isinstance(value, str) or not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError(f"Invalid version: {value!r}")
    return tuple(map(int, value.split(".")))


def generate(version, repository, root=Path("repo")):
    requested = parse_version(version)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Repository must be owner/name")
    latest = root / "updates/latest.json"
    if latest.exists():
        previous = json.loads(latest.read_text(encoding="utf-8"))
        if requested < parse_version(previous["version"]):
            raise ValueError(f"Refusing to replace version {previous['version']} with {version}")

    signature_path = root / "updates" / version / "Switchboot_x64-setup.exe.sig"
    signature = signature_path.read_text(encoding="utf-8").strip()
    if not signature:
        raise ValueError("Updater signature is empty")
    data = {
        "version": version,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {
            "windows-x86_64": {
                "signature": signature,
                "url": f"https://github.com/{repository}/releases/download/v{version}/Switchboot_x64-setup.exe",
            }
        },
    }
    temporary = latest.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    temporary.replace(latest)
    return data


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("repository", help="GitHub owner/name")
    args = parser.parse_args()
    try:
        generate(args.version, args.repository)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"Error: {error}\n")
    print(f"Generated latest.json for version {args.version}")
