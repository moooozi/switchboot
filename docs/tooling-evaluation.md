# Development tooling

## Icons

`app-icon.svg` is the application icon source. Generate app PNG/ICO files with:

```console
node packaging/generate-app-icons.mjs
```

Shortcut icons come from `static/os-icons` and
`crates/switchboot_core/icons-raw`:

```console
pnpm icons
```

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
| Rust/Cargo | Desktop app, privileged core, updater, native NSIS UI and updater signer |
| Python | NSIS build/validation runners, package tests, artifact collection and local update harness |
| Node/pnpm | SVG icon generation, release version/tag tooling and repository index generation |
| NSIS | Windows payload extraction, registration, shortcuts, ownership cleanup and app lifecycle hooks |
| Bash/package tools | Linux DEB/RPM assembly and repository operations |

[Release rules](../AGENTS.md#packaging-and-releases) define version synchronization,
tagging and push requirements. [Validation commands](migration-validation.md#automated-checks)
cover tooling tests; [CI documentation](pr-ci.md) covers artifact-collector checks.

Icon changes also require visual inspection on Windows and Linux. Keep tooling
out of the shipped application.
