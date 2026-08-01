#!/bin/sh
# Creates the switchboot-cli symlink on install/upgrade.
#
# Tauri's deb/rpm bundler dereferences symlinks and cannot ship one directly,
# so the link is installed here. The main binary detects CLI mode via argv[0]
# ending in "switchboot-cli", so the link only needs to point at the binary.
set -e

ln -sf switchboot /usr/bin/switchboot-cli
