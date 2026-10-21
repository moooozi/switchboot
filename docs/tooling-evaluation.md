# Development tooling

## Icons

`app-icon.svg` is the application icon source. Shortcut icons come from
`static/os-icons` and `crates/switchboot_core/icons-raw`. Generate all application
and host-platform shortcut assets with the Python development script:

```console
python -m pip install --only-binary=:all: -r requirements-dev.txt
python scripts/generate_icons.py
```

The script uses pinned resvg-py and Pillow binary wheels. Installation never
compiles native code when using `--only-binary=:all:`. The application build has
no dependency on these Python packages; generate assets before invoking Cargo.
App PNGs and the app ICO are always generated. Shortcut outputs default to the
host platform; use `--platform windows|linux|all` to select outputs explicitly
and `--root PATH` for an isolated fixture or source tree. The renderer does not
load system fonts. Keep icon sources self-contained SVGs.

SVG sources are versioned; generated PNGs and ICOs are ignored build artifacts.
Generate them before Cargo builds and package assembly.

Linux packages install 32, 48, 64, 128 and 256px application PNGs under
`hicolor/<size>x<size>/apps/dev.zidane.switchboot.png`, plus a scalable SVG.
Shortcut icons use the `dev.zidane.switchboot-` prefix and the same PNG sizes.

Windows app and shortcut ICOs contain DIB frames at 16, 32 and 48px, and PNG
frames at 128 and 256px. The application embeds its native window icon; NSIS uses
consumer-supplied ICO/SVG branding. Toolkit previews use independent example assets.

## Linux executable routing

`packaging/linux.sh` places the executable at `/usr/bin/switchboot` and packages
`/usr/bin/switchboot-cli -> switchboot` as a filesystem symlink. The package manager
owns both paths. The GUI does not create or repair package-owned files.

`cli_user/single_run.rs` invokes `pkexec` with the sibling CLI symlink using an
argument array. CLI mode is selected through argv[0]. The daemon invokes the
executable with `--cli --daemon`. Polkit policy identifies `/usr/bin/switchboot`.

## Languages and responsibilities

| Tool | Responsibility |
| --- | --- |
| Rust/Cargo | Desktop app, privileged core, updater, native NSIS UI, updater signer |
| Python 3.11+ | Icon generation with binary wheels; release preparation, repository indexes, NSIS build/validation runners, package tests, artifact collection and local update harness use the standard library |
| NSIS | Windows payload extraction, registration, shortcuts, ownership cleanup and app lifecycle hooks |
| Bash/package tools | Linux DEB/RPM assembly and repository operations |

Root `Cargo.toml` owns the application version. The core build script reads it
for application metadata and versioned IPC identifiers; internal crate versions
remain independent. Release preparation updates only the root manifest and its
local lockfile entry, preserving dependency versions:

```console
python scripts/release.py patch --dry-run
python scripts/release.py patch --no-push
```

The release tool retains immutable annotated tags, optional signing, clean-tree
and main-branch checks, preflight remote checks, rollback before a release commit,
and an atomic branch/tag push after confirmation. `--yes` explicitly authorizes
pushing; noninteractive runs otherwise keep the release local. See `--help` for
other options. Repository publication uses `scripts/generate_indexes.py`.

[Release rules](../AGENTS.md#packaging-and-releases) define version synchronization,
tagging and push requirements. [Validation commands](migration-validation.md#automated-checks)
cover tooling tests; [CI documentation](pr-ci.md) covers artifact-collector checks.

Icon changes also require visual inspection on Windows and Linux. Keep tooling
out of the shipped application.
