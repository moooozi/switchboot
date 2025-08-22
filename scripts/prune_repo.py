#!/usr/bin/env python3
"""
Prune repository packages according to rules:
- Keep the current version (argument 3)
- Keep the latest previous series release (same major, lower minor) for rollback
- Optionally remove versions > current unless PRUNE_ALLOW_GREATER is set (arg 4)

Usage: prune_repo.py <repo_root> <app_name> <tag> <allow_greater_flag>
"""
import os
import re
import sys
from functools import total_ordering


semver_re = re.compile(r"v?(\d+)\.(\d+)\.(\d+)")


@total_ordering
class Version:
    def __init__(self, major, minor, patch):
        self.major = int(major)
        self.minor = int(minor)
        self.patch = int(patch)

    @classmethod
    def parse(cls, s):
        m = semver_re.search(s)
        if not m:
            return None
        return cls(int(m.group(1)), int(m.group(2)), int(m.group(3)))

    def __eq__(self, other):
        return (self.major, self.minor, self.patch) == (other.major, other.minor, other.patch)

    def __lt__(self, other):
        return (self.major, self.minor, self.patch) < (other.major, other.minor, other.patch)

    def is_same_series(self, other):
        return self.major == other.major and self.minor == other.minor

    def is_previous_series(self, other):
        # previous series: same major, lower minor
        return self.major == other.major and self.minor < other.minor

    def __repr__(self):
        return f"v{self.major}.{self.minor}.{self.patch}"


def list_packages(dirpath, pattern):
    out = []
    if not os.path.isdir(dirpath):
        return out
    for fn in os.listdir(dirpath):
        if not fn.endswith(pattern):
            continue
        out.append(os.path.join(dirpath, fn))
    return out


def extract_ver_from_rpm(fn):
    # Try to parse name-version-release.arch.rpm
    base = os.path.basename(fn)
    if not base.endswith('.rpm'):
        return None
    # strip arch and .rpm
    try:
        core = base.rsplit('.', 2)[0]
    except Exception:
        core = base[:-4]
    # find the version part: look for -<digit>.
    m = re.search(r"-(\d+\.\d+\.\d+([^-]*)?)", core)
    if not m:
        return Version.parse(core)
    return Version.parse(m.group(1))


def extract_ver_from_deb(fn):
    # Debian: name_version_arch.deb
    base = os.path.basename(fn)
    if not base.endswith('.deb'):
        return None
    m = re.search(r"_([0-9]+\.[0-9]+\.[0-9]+)[^_]*_", base)
    if not m:
        m2 = re.search(r"_([0-9]+\.[0-9]+\.[0-9]+)\.deb$", base)
        if not m2:
            return Version.parse(base)
        return Version.parse(m2.group(1))
    return Version.parse(m.group(1))


def prune_packages(pkg_map, current_ver, allow_greater):
    # pkg_map: name -> list of (ver, path)
    to_keep = set()
    to_delete = set()

    for name, items in pkg_map.items():
        # sort by version
        items_sorted = sorted(items, key=lambda iv: iv[0])

        # remove versions greater than current unless allowed
        filtered = []
        for v, p in items_sorted:
            if v is None:
                # unknown version string: keep for safety
                filtered.append((v, p))
                continue
            if v > current_ver and not allow_greater:
                to_delete.add(p)
            else:
                filtered.append((v, p))

        if not filtered:
            continue

        # always keep exact current versions
        for v, p in filtered:
            if v == current_ver:
                to_keep.add(p)

        # find latest previous series (same major, lower minor)
        prev_series = [ (v,p) for (v,p) in filtered if v is not None and v.is_previous_series(current_ver)]
        if prev_series:
            # keep the highest in prev_series
            v,p = max(prev_series, key=lambda iv: iv[0])
            to_keep.add(p)

        # keep current series latest (current minor) as current already covered

        # any remaining that are not in to_keep kept only if not older than desired; delete others
        for v,p in filtered:
            if p in to_keep:
                continue
            # delete older same-series patches and older minors (unless selected)
            to_delete.add(p)

    return to_keep, to_delete


def main():
    if len(sys.argv) < 4:
        print("Usage: prune_repo.py <repo_root> <app_name> <tag> <allow_greater_flag>")
        return 2

    repo = sys.argv[1]
    app_name = sys.argv[2]
    tag = sys.argv[3]
    allow_greater = False
    if len(sys.argv) >= 5 and sys.argv[4] not in ("0", "", None):
        allow_greater = True

    cur = Version.parse(tag)
    if cur is None:
        print(f"Cannot parse current version from tag: {tag}")
        return 3

    # collect RPMs and DEBs
    rpm_dir = os.path.join(repo, 'rpm', 'x86_64')
    deb_dir = os.path.join(repo, 'deb', 'pool', 'main', app_name[0].lower(), app_name)

    rpm_files = list_packages(rpm_dir, '.rpm')
    deb_files = list_packages(deb_dir, '.deb')

    rpm_map = {}
    for p in rpm_files:
        # extract package name: attempt to find the first '-' before a digit (start of version)
        bn = os.path.basename(p)
        # try to match name-version... pattern
        m = re.match(r"^(.+?)-(?=\d)", bn)
        if m:
            name = m.group(1)
        else:
            # fallback: take prefix before first '-'
            name = bn.split('-')[0]
        
        # only process packages that match the specified app name
        if name != app_name:
            continue
            
        v = extract_ver_from_rpm(bn)
        rpm_map.setdefault(name, []).append((v, p))

    deb_map = {}
    for p in deb_files:
        bn = os.path.basename(p)
        name = bn.split('_')[0]
        
        # only process packages that match the specified app name
        if name != app_name:
            continue
            
        v = extract_ver_from_deb(bn)
        deb_map.setdefault(name, []).append((v, p))

    kept = set()
    deleted = set()

    k,d = prune_packages(rpm_map, cur, allow_greater)
    kept |= k
    deleted |= d

    k,d = prune_packages(deb_map, cur, allow_greater)
    kept |= k
    deleted |= d

    # actually delete
    for p in deleted:
        try:
            os.remove(p)
            print(f"Deleted: {p}")
        except Exception as e:
            print(f"Failed to delete {p}: {e}")

    print("Kept files:")
    for p in kept:
        print(p)

    return 0


if __name__ == '__main__':
    raise SystemExit(main())
