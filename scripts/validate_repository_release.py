"""Require a published stable release before updating package repositories."""

import json
import re
import sys


def validate_release(release, tag):
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", tag):
        raise ValueError("Expected a stable release tag (vX.Y.Z)")
    if not isinstance(release, dict) or release.get("tagName") != tag:
        raise ValueError("Release metadata does not match the requested tag")
    if release.get("isDraft") is not False:
        raise ValueError("Repository updates require a non-draft release")
    if release.get("isPrerelease") is not False:
        raise ValueError("Repository updates do not accept prereleases")
    if (
        not isinstance(release.get("publishedAt"), str)
        or not release["publishedAt"].strip()
    ):
        raise ValueError("Release has not been published")


def main():
    if len(sys.argv) != 2:
        print(
            "Usage: validate_repository_release.py <tag> (release JSON on stdin)",
            file=sys.stderr,
        )
        return 1
    try:
        validate_release(json.load(sys.stdin), sys.argv[1])
    except (ValueError, TypeError) as error:
        print(f"Release validation failed: {error}", file=sys.stderr)
        return 1
    print(f"Validated published stable release {sys.argv[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
