#!/usr/bin/env bash
set -euo pipefail
version=${1:?Release version required}
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Invalid release version' >&2; exit 1; }
shopt -s nullglob
debs=()
rpms=()
for package in target/release/bundle/deb/*.deb; do
  if [ "$(dpkg-deb --field "$package" Version)" = "$version" ] && \
     [ "$(dpkg-deb --field "$package" Architecture)" = 'amd64' ]; then
    debs+=("$package")
  fi
done
for package in target/release/bundle/rpm/*.rpm; do
  if [ "$(rpm -qp --qf '%{VERSION} %{ARCH}' "$package")" = "$version x86_64" ]; then
    rpms+=("$package")
  fi
done
if [ "${#debs[@]}" -ne 1 ] || [ "${#rpms[@]}" -ne 1 ]; then
  echo "Expected exactly one DEB and one RPM for $version and x86_64" >&2
  exit 1
fi
for package in "${debs[@]}" "${rpms[@]}"; do
  test -s "$package"
done
mkdir -p artifacts
cp "${debs[0]}" "${rpms[0]}" artifacts/
echo "deb_artifact=$(basename "${debs[0]}")" >> "$GITHUB_OUTPUT"
echo "rpm_artifact=$(basename "${rpms[0]}")" >> "$GITHUB_OUTPUT"
