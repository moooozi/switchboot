# Switchboot

Switchboot manages EFI boot entries on Windows and Linux with a native Rust UI built using [iced](https://iced.rs/).

Reorder boot entries, choose the default or next boot target, schedule firmware setup, create boot shortcuts, and undo or redo changes. Boot-order edits are saved explicitly; BootNext and firmware-setup changes are applied immediately. Restart always requires confirmation.

## Development

Install a current stable Rust toolchain, Node and pnpm. Initialize the `issconfig`
submodule and generate the icon assets from the SVG sources before the first
Cargo build:

```sh
git submodule update --init --recursive
pnpm install --frozen-lockfile
pnpm icons
node packaging/generate-app-icons.mjs
cargo run -- --demo
```

Demo mode uses an in-memory backend and never accesses EFI, requests elevation, creates system shortcuts, or restarts the machine. For real firmware access, use `cargo run`. The real GUI runs without elevation and delegates privileged operations to the CLI backend.

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

Pull requests are checked on Windows and Linux, with unsigned installers, packages
and binaries available as workflow artifacts. See [PR CI](docs/pr-ci.md) for checks,
downloads and required branch-rule settings.

```sh
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --manifest-path packaging/updater-sign/Cargo.toml
cargo build --locked --release --bin switchboot
```

Node and pnpm are tooling dependencies for icon generation and release preparation.
Generated PNGs and ICOs are ignored build artifacts. Repeat generation after
editing SVG sources; CI generates them before building and packaging:

```sh
pnpm install --frozen-lockfile
pnpm icons
node packaging/generate-app-icons.mjs
node --test scripts/release.test.mjs scripts/icons.test.mjs scripts/app-icons.test.mjs
python -B -m unittest discover -s scripts -p 'test_*.py'
```

Linux release packages are built by `packaging/linux.sh`; Windows installers by Inno Setup using `packaging/windows.iss`.

The Windows installer uses the self-contained [issconfig visual library](issconfig/README.md). Switchboot owns its branding, payload, service lifecycle and upgrade migration in `packaging/windows.iss`; the library owns themes, compact layout and the folder browser. Its standalone fixture compile checks run with the Windows packaging tests.

The library's upstream is [moooozi/issconfig](https://github.com/moooozi/issconfig). Switchboot includes version `0.1.0` as a Git submodule. Builds consume its local sources; CI initializes the submodule during checkout. After updating the library, run `python scripts/test_native_packaging.py` on Windows.

`app-icon.svg` is the source for the app icon. After editing it, run `node packaging/generate-app-icons.mjs` and commit only the SVG source. The generated PNGs and ICO in `crates/switchboot_core/icons` stay local or in CI build output. The native window embeds the 128px PNG; Windows builds embed the ICO and version metadata into the executable. Linux packages install 32, 48, 64, 128 and 256px PNGs and scalable SVGs in the hicolor theme. The app icon and desktop file use `dev.zidane.switchboot`; shortcut icons use the `dev.zidane.switchboot-` prefix. App and shortcut ICOs use DIB frames at 16, 32 and 48px and PNG frames at 128 and 256px. Cargo builds consume generated app icons; run the generator first on a fresh checkout.

## Releases

From a clean working tree on `main`, run `pnpm release patch` (or `minor`, `major`, or an explicit version). The script updates the root Cargo manifest, lockfile and tooling metadata, commits and creates an annotated tag. It asks before pushing branch and tag atomically; `--dry-run` previews without changes.

Tags are immutable. Failed builds can be rerun on the same tag; source changes require a new release. Tag pushes create a draft GitHub Release. Publishing the draft updates the signed APT/RPM repositories. Artifacts carry GPG signatures and build provenance; verify provenance with `gh attestation verify <asset> --repo moooozi/switchboot`.

## Application behavior

Drag-and-drop supports reordering and moving entries between the boot order and Others, including edge scrolling. Up/Down controls are also available. Each completed drop is one undoable edit and remains local until Save. Keyboard shortcuts are Ctrl+S to save, Ctrl+Z to undo and Ctrl+Shift+Z to redo.

Repeat GUI launches activate the existing per-user window. Demo mode uses separate ownership from the real GUI; CLI, service and shortcut operations run independently.

Updates are distributed through Linux package managers and signed Windows installer downloads.

See [the validation checklist](docs/migration-validation.md) for demo checks and device tests covering UAC/polkit, firmware persistence and the installed Windows service.
