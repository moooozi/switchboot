# Switchboot

Switchboot manages EFI boot entries on Windows and Linux with a native Rust UI built using [iced](https://iced.rs/).

Reorder boot entries, choose the default or next boot target, schedule firmware setup, create boot shortcuts, and undo or redo changes. Boot-order edits are saved explicitly; BootNext and firmware-setup changes are applied immediately. Restart always requires confirmation.

## Development

Install a current stable Rust toolchain, then run:

```sh
cargo run -- --demo
```

Demo mode uses an in-memory backend and never accesses EFI, requests elevation, creates system shortcuts, or restarts the machine. For real firmware access, use `cargo run`. The real GUI runs without elevation and delegates privileged operations to the existing CLI transport.

On Linux, install polkit and run a graphical authentication agent. Development builds need a sibling CLI symlink:

```sh
cargo build
ln -sf switchboot target/debug/switchboot-cli
cargo run
```

Packaged installations supply the symlink and polkit policy. Do not run the real GUI as root. Windows portable builds request UAC when needed; installed builds use the service. Shortcuts require installed mode on Windows.

## Workspace

| Package | Responsibility |
| --- | --- |
| `switchboot` | Executable mode routing and backend adapter |
| `switchboot_ui` | iced UI, typed messages/tasks, state/history and demo backend |
| `switchboot_core` | EFI CLI, privileged IPC, services and platform shortcuts |

The UI crate exposes a typed backend interface, allowing state tests without firmware access. Blocking platform calls run on worker threads. Successful firmware writes advance undo/redo history; failures retain the previous state and cursor.

## Build and verify

```sh
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --manifest-path packaging/updater-sign/Cargo.toml
cargo build --locked --release --bin switchboot
```

Node and pnpm are optional development tools for icon generation and release preparation:

```sh
pnpm install --frozen-lockfile
pnpm icons
node packaging/generate-app-icons.mjs
node --test scripts/release.test.mjs scripts/icons.test.mjs
python -B -m unittest discover -s scripts -p 'test_*.py'
```

Linux release packages are built by `packaging/linux.sh`; Windows installers by `packaging/windows.nsi`. Release workflows use Cargo and native package tools. WebKit, WebView2, Svelte, Vite and Tauri are no longer runtime or build dependencies.

## Releases

From a clean working tree on `main`, run `pnpm release patch` (or `minor`, `major`, or an explicit version). The script updates the root Cargo manifest, lockfile and tooling metadata, commits and creates an annotated tag. It asks before pushing branch and tag atomically; `--dry-run` previews without changes.

Tags are immutable. Failed builds can be rerun on the same tag; source changes require a new release. Tag pushes create a draft GitHub Release. Publishing the draft updates the signed APT/RPM repositories. Artifacts carry GPG signatures and build provenance; verify provenance with `gh attestation verify <asset> --repo moooozi/switchboot`.

## Native migration status

The first iced UI covers the firmware management workflow, order editing, save/discard, bounded undo/redo, SVG OS icons, shortcuts, refresh, restart confirmation, and Ctrl+S / Ctrl+Z / Ctrl+Shift+Z shortcuts. Up/Down controls replace drag-and-drop during this phase.

Native drag-and-drop, single-instance window focusing, and the automatic updater UI remain follow-ups. Existing signed updater feed compatibility is retained for previous releases. Native users can install updates through the package manager or download the signed installer.
