# Native icon and tooling evaluation

## Linux app icons

The generator emits 32, 48, 64, 128 and 256px transparent PNGs. Packages install
them as `hicolor/<size>x<size>/apps/dev.zidane.switchboot.png`, and install
`app-icon.svg` as `hicolor/scalable/apps/dev.zidane.switchboot.svg`. Shortcut icons
use the `dev.zidane.switchboot-` prefix, with scalable SVGs and the same PNG sizes.

Only SVG sources are versioned. PNGs and ICOs are ignored build artifacts,
generated locally before the first Cargo build and regenerated in CI before
building and packaging. Preserve this rule in any tooling migration.

The 48px size follows the minimum recommended by the
[freedesktop icon-theme specification](https://specifications.freedesktop.org/icon-theme/latest/#installing-application-icons).
Scalable SVGs support larger display sizes. No macOS bundles or ICNS files are generated.

Both app and shortcut ICOs use DIB frames at 16, 32 and 48px, and PNG frames at
128 and 256px. Small DIBs preserve Windows compatibility; larger PNGs limit file size.

## Linux CLI symlink

The current implementation already uses a real filesystem symlink. The packaging
script runs `ln -s switchboot` inside its temporary package staging directory;
DEB and RPM contain `/usr/bin/switchboot-cli -> switchboot`. No installed shell
wrapper or install-time script creates the link. The package manager installs it.

`cli_user/single_run.rs` invokes `pkexec` with the sibling symlink directly, using
argument arrays. CLI mode is detected through argv[0]. The daemon invokes the
executable directly with `--cli --daemon`. Neither path runs `sh` or `bash`.
Polkit policy refers to the real `/usr/bin/switchboot` executable.

Recommendation: retain the packaged symlink. Moving its creation into the running
app adds no benefit for installed builds and would require elevation to write in
`/usr/bin`. Do not have the GUI repair package-owned files or request elevation
solely to create the link.

Optional development improvement: replace the documented manual `ln -sf` step
with `std::os::unix::fs::symlink` for writable sibling directories. Validate an
existing link's target, refuse to overwrite any file or unrelated link, handle
concurrent creation, and return a useful error on read-only directories. Use
temporary-directory tests; do not execute firmware commands. This is evaluated,
not implemented in this change.

## Removing Node and package.json

Recommendation: feasible as a separate tooling migration. Keep Python for CI/CD
and local orchestration, make the root Cargo version authoritative, and remove
`package.json`, `pnpm-lock.yaml`, Node/pnpm setup and JS tooling after replacements
pass their current tests. Do not delete the manifest first: both Rust build scripts
currently read it, and release validation checks it against the tag and Cargo.

Required replacements:

| Existing responsibility | Proposed replacement |
| --- | --- |
| `scripts/release.mjs` and tests | Python release command and unittest fixtures |
| `scripts/generate_indexes.js` | Python directory listing / HTML generation with escaping |
| App and shortcut SVG rasterization / ICO generation | Small Rust icon tool using resvg/tiny-skia and PNG encoding, invoked by Python |
| Build metadata and version checks | Root `CARGO_PKG_VERSION`; explicit propagation to core build metadata, validated against root Cargo.toml |
| CI version reads and release validation | Python 3.11+ `tomllib` reading Cargo.toml |
| `pnpm dev/build/check` aliases | Document the direct Cargo commands |

Python standard library is sufficient for orchestration, version parsing, Git
subprocesses, HTML indexes and tests. Preserve clean-main checks, synchronized
Cargo.toml/Cargo.lock, rollback before a release commit, immutable annotated tags,
signing options and confirmation before atomic branch/tag push. Preserve repository
metadata checks, GPG signatures, provenance and signed updater compatibility.

SVG conversion is the main dependency decision. A Python-only alternative is
CairoSVG plus Pillow, but [CairoSVG requires native Cairo on Windows](https://cairosvg.org/documentation/)
and supports only a subset of SVG filters. A Rust icon tool fits the current Rust
workspace and the renderer already used by iced. Keep it outside the shipped app;
Python can invoke it with explicit subprocess arguments. Test visual parity,
transparency, overlay composition and mixed DIB/PNG ICO encoding on both hosts.

`svgo` currently has a config but no callers in tracked scripts or workflows. It
can be removed with the migration unless an explicit manual optimization workflow
is retained. No dependencies or manifests are removed in this evaluation.

Follow-ups:

- Port the release and repository-index scripts and preserve their behavior tests.
- Build and validate the icon tool on Windows and Linux.
- Change build metadata and all CI version reads to Cargo; ensure core metadata
  rebuilds when the root version changes.
- Update workflows, docs and contributor instructions, then remove Node manifests,
  lockfile, JS scripts and obsolete SVG optimization configuration.
- Validate Linux DEB/RPM icons in a real desktop session and Windows installer icons.
