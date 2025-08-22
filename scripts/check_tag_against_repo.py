#!/usr/bin/env python3
"""
Check whether the provided tag (vMAJOR.MINOR.PATCH) is newer than any existing git tag in the repository.
Exit non-zero if an equal or greater version exists. Prints helpful messages.

Usage: check_tag_against_repo.py vX.Y.Z
"""
import re
import subprocess
import sys


semver_re = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)(?:[.-].*)?$")


def parse_ver(tag):
    m = semver_re.match(tag)
    if not m:
        raise SystemExit(f"Invalid tag format: {tag}. Expected vMAJOR.MINOR.PATCH")
    return tuple(int(x) for x in m.groups())


def list_git_tags():
    try:
        raw = subprocess.check_output(["git", "tag", "--list"]).decode().splitlines()
        return raw
    except subprocess.CalledProcessError as e:
        raise SystemExit(f"Failed to list git tags: {e}")


def main():
    if len(sys.argv) < 2:
        print("Usage: check_tag_against_repo.py vX.Y.Z")
        return 2

    tag = sys.argv[1]
    try:
        cur = parse_ver(tag)
    except SystemExit as e:
        print(e)
        return 2

    tags = list_git_tags()
    greater = []
    equal = []
    for t in tags:
        try:
            v = parse_ver(t)
        except Exception:
            continue
        if v > cur:
            greater.append(t)
        if v == cur:
            equal.append(t)

    if greater:
        print(f"Found existing greater tags in repo: {greater}")
        print("Refusing to proceed unless repo-config/FORCE_REPO_UPDATE is present")
        return 3

    if equal:
        print(f"Found existing equal tags in repo: {equal}")
        print("Refusing to proceed unless repo-config/FORCE_REPO_UPDATE is present")
        return 4

    print("No equal or greater tags found — OK to proceed")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
