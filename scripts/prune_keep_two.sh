#!/usr/bin/env bash
set -euo pipefail
# Prune RPM and DEB packages keeping only the latest two versions per package name.
# RPM parsing uses `rpm -qp` when available for accurate NAME and VERSION-RELEASE extraction.

repo_root="$1"
rpm_dir="$repo_root/rpm/x86_64"
deb_dir="$repo_root/deb/pool/main/s/switchboot"

tmpfile=$(mktemp)
trap 'rm -f "$tmpfile"' EXIT

echo "Scanning RPMs in $rpm_dir"
if [ -d "$rpm_dir" ]; then
  find "$rpm_dir" -maxdepth 1 -type f -name '*.rpm' -print0 | while IFS= read -r -d '' f; do
    # Try to get NAME and VERSION-RELEASE from the RPM itself
    if command -v rpm >/dev/null 2>&1; then
      if rpm_info=$(rpm -qp --qf '%{NAME}|%{VERSION}-%{RELEASE}\n' "$f" 2>/dev/null); then
        name=$(echo "$rpm_info" | cut -d'|' -f1)
        ver=$(echo "$rpm_info" | cut -d'|' -f2)
      else
        # fallback to filename parsing
        base=$(basename "$f" .rpm)
        arch=${base##*.}
        core=${base%.*}
        name=$(echo "$core" | sed -E 's/(-[0-9].*)$//')
        ver=$(echo "$core" | sed -E 's/.*-([0-9].*)$/\1/')
      fi
    else
      base=$(basename "$f" .rpm)
      arch=${base##*.}
      core=${base%.*}
      name=$(echo "$core" | sed -E 's/(-[0-9].*)$//')
      ver=$(echo "$core" | sed -E 's/.*-([0-9].*)$/\1/')
    fi
    # write: name|ver|path
    printf '%s|%s|%s\n' "$name" "$ver" "$f" >> "$tmpfile"
  done
fi

if [ -f "$tmpfile" ]; then
  awk -F'|' '{print $1}' "$tmpfile" | sort -u | while IFS= read -r pkg; do
    # get records for this pkg, sort by version (version-sort), keep last two
    grep "^${pkg}|" "$tmpfile" | sort -t'|' -k2 -V > "${tmpfile}.${pkg}"
    count=$(wc -l < "${tmpfile}.${pkg}" | tr -d ' ')
    if [ "$count" -le 2 ]; then
      rm -f "${tmpfile}.${pkg}"
      continue
    fi
    # remove everything except last two
    to_delete=$(head -n $((count-2)) "${tmpfile}.${pkg}" | awk -F'|' '{print $3}')
    for p in $to_delete; do
      rm -f "$p" || true
    done
    rm -f "${tmpfile}.${pkg}"
  done
fi

# For DEBs: keep the latest 3 versions per package. Use dpkg-deb when available
if [ -d "$deb_dir" ]; then
  deb_tmp=$(mktemp)
  trap 'rm -f "$deb_tmp"' RETURN
  # collect package|version|path
  find "$deb_dir" -maxdepth 1 -type f -name '*.deb' -print0 | while IFS= read -r -d '' f; do
    if command -v dpkg-deb >/dev/null 2>&1; then
      if deb_info=$(dpkg-deb -f "$f" Package 2>/dev/null); then
        pkg=$(dpkg-deb -f "$f" Package 2>/dev/null || true)
        ver=$(dpkg-deb -f "$f" Version 2>/dev/null || "")
      else
        name=$(basename "$f")
        pkg=$(echo "$name" | sed -E 's/(_[0-9].*)$//' )
        ver=$(echo "$name" | sed -E 's/.*_([0-9].*)\.deb$/\1/')
      fi
    else
      name=$(basename "$f")
      pkg=$(echo "$name" | sed -E 's/(_[0-9].*)$//' )
      ver=$(echo "$name" | sed -E 's/.*_([0-9].*)\.deb$/\1/')
    fi
    printf '%s|%s|%s\n' "$pkg" "$ver" "$f" >> "$deb_tmp"
  done

  if [ -f "$deb_tmp" ]; then
    awk -F'|' '{print $1}' "$deb_tmp" | sort -u | while IFS= read -r pkg; do
      grep "^${pkg}|" "$deb_tmp" | sort -t'|' -k2 -V > "${deb_tmp}.${pkg}"
      count=$(wc -l < "${deb_tmp}.${pkg}" | tr -d ' ')
      if [ "$count" -le 3 ]; then
        rm -f "${deb_tmp}.${pkg}"
        continue
      fi
      to_delete=$(head -n $((count-3)) "${deb_tmp}.${pkg}" | awk -F'|' '{print $3}')
      for p in $to_delete; do
        rm -f "$p" || true
      done
      rm -f "${deb_tmp}.${pkg}"
    done
  fi
fi

# remove empty directories under repo_root
find "$repo_root" -type d -empty -delete || true

echo "Prune complete"
