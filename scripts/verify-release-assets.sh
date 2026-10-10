#!/usr/bin/env bash
# Trust only the configured release-signing key, not keys downloaded with assets.
set -euo pipefail
: "${GPG_PRIVATE_KEY:?Release-signing key is required to verify assets}"
artifacts=${1:?Artifact directory required}
shopt -s nullglob
debs=("$artifacts"/*.deb)
rpms=("$artifacts"/*.rpm)
if [ "${#debs[@]}" -ne 1 ] || [ "${#rpms[@]}" -ne 1 ]; then
  echo 'Expected exactly one DEB and one RPM' >&2
  exit 1
fi
GNUPGHOME=$(mktemp -d)
export GNUPGHOME
trap 'gpgconf --kill gpg-agent >/dev/null 2>&1 || true; rm -rf "$GNUPGHOME"' EXIT
printf '%s\n' "$GPG_PRIVATE_KEY" | gpg --batch --import
for package in "${debs[@]}" "${rpms[@]}" "$artifacts/Switchboot_x64-setup.exe" "$artifacts/Switchboot_x64-portable.exe"; do
  test -s "$package"
  test -s "$package.asc"
  gpg --batch --no-auto-key-retrieve --verify "$package.asc" "$package"
done
