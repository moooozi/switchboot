"""Select trusted validation builds and check their downloaded release payloads."""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path


def trusted_run(run, repository, commit):
    return (
        run.get("head_sha") == commit
        and run.get("event") == "push"
        and run.get("head_branch") == "main"
        and run.get("path") == ".github/workflows/pr-validation.yml"
        and run.get("repository", {}).get("full_name") == repository
        and run.get("head_repository", {}).get("full_name") == repository
    )


def select_run(runs, repository, commit):
    candidates = [
        run
        for run in runs
        if trusted_run(run, repository, commit)
        and run.get("status") == "completed"
        and run.get("conclusion") == "success"
    ]
    if not candidates:
        eligible = [run for run in runs if trusted_run(run, repository, commit)]
        if (
            eligible
            and max(eligible, key=lambda run: run["id"])["status"] == "completed"
        ):
            raise ValueError(
                "Main validation did not succeed; fix or rerun validation before releasing"
            )
        raise LookupError(
            "No successful main-push validation exists for the release commit"
        )
    return max(candidates, key=lambda run: run["id"])


def verify_payload(directory, commit, version, platform):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Invalid release version")
    expected = {
        "commit": commit,
        "layout": "iced",
        "platform": platform,
        "signed": False,
    }
    metadata = json.loads((directory / "build-info.json").read_text(encoding="utf-8"))
    if metadata != expected:
        raise ValueError("Downloaded artifact identity does not match the release")
    names = (
        {"Switchboot_x64-setup.exe", "Switchboot_x64-portable.exe"}
        if platform == "windows"
        else {f"switchboot_{version}_amd64.deb", f"switchboot-{version}-1.x86_64.rpm"}
    )
    allowed = names | {"build-info.json"}
    if platform == "linux":
        allowed.add("Switchboot_linux_x64.tar.gz")
    if {path.name for path in directory.iterdir()} != allowed:
        raise ValueError("Downloaded artifact has missing or unexpected files")
    for name in allowed:
        path = directory / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Missing, empty or linked release input: {name}")


def api_get(repository, path):
    request = urllib.request.Request(
        f"https://api.github.com/repos/{repository}/{path}",
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {os.environ['GH_TOKEN']}",
            "User-Agent": "Switchboot-release",
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def find_build(repository, commit):
    query = urllib.parse.urlencode(
        {"head_sha": commit, "event": "push", "per_page": 100}
    )
    runs = api_get(repository, f"actions/workflows/pr-validation.yml/runs?{query}")
    run = select_run(runs["workflow_runs"], repository, commit)
    artifacts = api_get(repository, f"actions/runs/{run['id']}/artifacts?per_page=100")
    for platform in ("Linux", "Windows"):
        name = f"switchboot-pr-{platform}-{commit}"
        matching = [item for item in artifacts["artifacts"] if item["name"] == name]
        if len(matching) != 1 or matching[0]["expired"]:
            raise ValueError(
                f"Required validation artifact is missing or expired: {name}"
            )
    return run["id"]


def verify_provenance(directory, repository, commit, platform):
    patterns = ("*.exe",) if platform == "windows" else ("*.deb", "*.rpm")
    for pattern in patterns:
        for path in sorted(directory.glob(pattern)):
            subprocess.run(
                [
                    "gh",
                    "attestation",
                    "verify",
                    str(path),
                    "--repo",
                    repository,
                    "--signer-workflow",
                    f"{repository}/.github/workflows/pr-validation.yml",
                    "--source-digest",
                    commit,
                    "--source-ref",
                    "refs/heads/main",
                    "--deny-self-hosted-runners",
                ],
                check=True,
            )


def rpm_signing_predicate(path, repository, commit, validation_run):
    with path.open("rb") as payload:
        digest = hashlib.file_digest(payload, "sha256").hexdigest()
    return {
        "buildDefinition": {
            "buildType": f"https://github.com/{repository}/rpm-signing/v1",
            "externalParameters": {"commit": commit, "validationRun": validation_run},
            "resolvedDependencies": [
                {
                    "uri": f"https://github.com/{repository}/actions/runs/{validation_run}#{path.name}",
                    "digest": {"sha256": digest},
                }
            ],
        },
        "runDetails": {
            "builder": {
                "id": f"https://github.com/{os.environ['GITHUB_WORKFLOW_REF']}"
            },
            "metadata": {
                "invocationId": f"https://github.com/{repository}/actions/runs/{os.environ['GITHUB_RUN_ID']}/attempts/{os.environ['GITHUB_RUN_ATTEMPT']}"
            },
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    find = commands.add_parser("find")
    find.add_argument("repository")
    find.add_argument("commit")
    verify = commands.add_parser("verify")
    verify.add_argument("directory", type=Path)
    verify.add_argument("commit")
    verify.add_argument("version")
    verify.add_argument("platform", choices=("linux", "windows"))
    verify.add_argument("--repository", required=True)
    predicate = commands.add_parser("rpm-predicate")
    predicate.add_argument("rpm", type=Path)
    predicate.add_argument("repository")
    predicate.add_argument("commit")
    predicate.add_argument("validation_run")
    find.add_argument("--wait-seconds", type=int, default=0)
    args = parser.parse_args()
    if args.command == "find":
        deadline = time.monotonic() + args.wait_seconds
        while True:
            try:
                run_id = find_build(args.repository, args.commit)
                break
            except LookupError:
                if time.monotonic() >= deadline:
                    raise
                print(
                    "Waiting for successful main validation of the release commit...",
                    file=sys.stderr,
                    flush=True,
                )
                time.sleep(30)
        print(run_id)
    elif args.command == "verify":
        verify_payload(args.directory, args.commit, args.version, args.platform)
        verify_provenance(args.directory, args.repository, args.commit, args.platform)
        print(f"Verified {args.platform} artifacts from {args.commit}")
    else:
        print(
            json.dumps(
                rpm_signing_predicate(
                    args.rpm, args.repository, args.commit, args.validation_run
                ),
                indent=2,
            )
        )


if __name__ == "__main__":
    main()
