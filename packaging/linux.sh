#!/usr/bin/env bash
set -euo pipefail
version=${1:?Release version required}
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Invalid release version' >&2; exit 1; }
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
binary="$root/target/release/switchboot"
test -s "$binary"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/root/usr/bin" "$stage/root/usr/share/applications" "$stage/root/usr/share/polkit-1" "$stage/root/usr/share/icons/hicolor"
install -m755 "$binary" "$stage/root/usr/bin/switchboot"
ln -s switchboot "$stage/root/usr/bin/switchboot-cli"
install -m644 packaging/dev.zidane.switchboot.desktop "$stage/root/usr/share/applications/"
cp -R crates/switchboot_core/platform/linux/polkit-1/. "$stage/root/usr/share/polkit-1/"
cp -R crates/switchboot_core/icons-linux/hicolor/. "$stage/root/usr/share/icons/hicolor/"
for size in 32 48 64 128 256 512; do
  icon_dir="$stage/root/usr/share/icons/hicolor/${size}x${size}/apps"
  mkdir -p "$icon_dir"
  install -m644 "crates/switchboot_core/icons/${size}x${size}.png" "$icon_dir/switchboot.png"
done
mkdir -p "$stage/root/usr/share/icons/hicolor/scalable/apps"
install -m644 app-icon.svg "$stage/root/usr/share/icons/hicolor/scalable/apps/switchboot.svg"
mkdir -p "$stage/root/DEBIAN" target/release/bundle/deb target/release/bundle/rpm
cat > "$stage/root/DEBIAN/control" <<EOF
Package: switchboot
Version: $version
Architecture: amd64
Maintainer: M Zidane <switchboot@zidane.dev>
Depends: polkitd, libc6, libgcc-s1, libfontconfig1, libxkbcommon0, libwayland-client0
Section: utils
Priority: optional
Description: Manage EFI boot entries with a native desktop interface
EOF
dpkg-deb --root-owner-group --build "$stage/root" "target/release/bundle/deb/switchboot_${version}_amd64.deb"
rm -rf "$stage/root/DEBIAN"
mkdir -p "$stage/rpm/BUILD" "$stage/rpm/RPMS" "$stage/rpm/SOURCES" "$stage/rpm/SPECS" "$stage/rpm/SRPMS"
cat > "$stage/rpm/SPECS/switchboot.spec" <<EOF
Name: switchboot
Version: $version
Release: 1
Summary: Manage EFI boot entries
License: Unspecified
BuildArch: x86_64
Requires: polkit, fontconfig, libxkbcommon, libwayland-client.so.0()(64bit)
%description
Switchboot provides a native desktop interface for managing EFI boot entries.
%install
mkdir -p %{buildroot}
cp -a "$stage/root/." %{buildroot}/
%files
/usr/bin/switchboot
/usr/bin/switchboot-cli
/usr/share/applications/dev.zidane.switchboot.desktop
/usr/share/polkit-1/actions/com.switchboot.cli.policy
/usr/share/icons/hicolor/*/apps/*.png
/usr/share/icons/hicolor/scalable/apps/switchboot.svg
EOF
rpmbuild --define "_topdir $stage/rpm" --define '_build_id_links none' --define '__os_install_post %{nil}' -bb "$stage/rpm/SPECS/switchboot.spec"
cp "$stage/rpm/RPMS/x86_64/switchboot-${version}-1.x86_64.rpm" target/release/bundle/rpm/
