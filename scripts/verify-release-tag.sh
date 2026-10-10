#!/usr/bin/env bash
# Check against the fetched main ref, never the tag's claimed targetCommitish.
set -euo pipefail
tag=${1:?Release tag required}
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Invalid release tag' >&2; exit 1; }
commit=$(git rev-parse --verify "refs/tags/${tag}^{commit}")
[[ "$commit" == "$(git rev-parse HEAD)" ]] || { echo 'Checkout does not match release tag' >&2; exit 1; }
git merge-base --is-ancestor "$commit" refs/remotes/origin/main || {
  echo 'Release tag must belong to origin/main' >&2
  exit 1
}
