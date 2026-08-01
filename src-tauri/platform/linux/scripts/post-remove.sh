#!/bin/sh
# Removes the switchboot-cli symlink created by post-install.sh.
#
# Works for both deb postrm ($1 = remove|purge) and rpm %postun ($1 = 0 on full
# removal). During upgrades the link is left in place and recreated by postinst.
set -e

case "$1" in
  remove | purge | 0)
    rm -f /usr/bin/switchboot-cli
    ;;
esac
