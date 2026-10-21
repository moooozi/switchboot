# Switchboot AI Coding Guidelines

## Architecture

Switchboot is a Rust 2024 desktop application using iced 0.14 to manage EFI boot entries on Windows and Linux.

- Root `switchboot` package: `src/main.rs` routes GUI, CLI, shortcut and Windows service modes; `src/backend.rs` implements the UI backend interface.
- `crates/switchboot_ui`: native iced views, typed messages, blocking-worker tasks, pure state/history model, and in-memory demo backend. It does not depend on platform firmware code.
- `crates/switchboot_core`: privileged CLI operations, platform APIs, JSON stdin/stdout transport and secure named-pipe IPC. It does not depend on iced.
- `crates/switchboot_updater`: signed updates and Windows installer observation, independent of iced and firmware code. The root adapter supplies version/installed-mode policy; the UI owns presentation. Validate ownership and app prerequisites before GUI handoff, then wait for exit before service/file changes. Reject unsafe manifests and preserve user files. See [update contracts](docs/windows-updates.md).
- Keep one executable: CLI invocations and Windows services use the running executable. Linux policy uses `/usr/bin/switchboot` and the sibling `switchboot-cli` symlink.
- `firmware_variables` handles EFI variables; `pipeguard` secures Windows IPC. Preserve privilege separation; never call firmware directly from the UI.

## UI conventions

- Use iced's application builder, `Message` enums and `Task`-returning updates. Run blocking platform calls with `tokio::task::spawn_blocking`.
- `Backend` is a typed, `Send + Sync` boundary injected as `Arc<dyn Backend>`.
- Store the entry catalog separately from the draft boot order and saved baseline. Default badges derive from the saved baseline.
- Boot order changes remain local until Save. Successful saves retain the submitted snapshot as baseline.
- Firmware mutations and undo/redo share an exclusive operation guard. Commit state and history cursor only on success; failed operations retain the previous state.
- BootNext uses `Option<u16>` (Boot0000 is valid). Firmware setup is a separate target enum, never a synthetic entry ID.
- History is bounded to 50 changes. Removing an entry only removes its ID from the draft, preserving the catalog for undo.
- Drag-and-drop supports reordering, moves between Boot order and Others, edge scrolling and Up/Down controls. Each completed drop creates one undoable change.
- Repeat GUI launches activate the per-user window. Demo and real GUI ownership are separate; CLI/service/shortcut modes bypass GUI ownership.
- Test state behavior with mock backends, never live firmware or real restarts.

## Development

```sh
cargo run -- --demo              # Safe UI development; no EFI access/elevation/reboot
cargo run                       # Real firmware backend
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
python -m pip install --only-binary=:all: -r requirements-dev.txt
python scripts/generate_icons.py  # Generate app and shortcut icons
cargo build --release --bin switchboot
```

Linux real firmware access needs polkit, a graphical authentication agent and the sibling `switchboot-cli` symlink. Do not run the real GUI as root. Windows portable mode uses UAC; installed mode uses the Windows service. Shortcuts remain disabled in Windows portable mode.

## Packaging and releases

- Keep product integrations in Switchboot hooks and reusable behavior in the [NSIS toolkit](packaging/nsis/toolkit/README.md). See [Windows builds and file layout](packaging/nsis/README.md), [icons and Linux routing](docs/tooling-evaluation.md), and [CI constraints](docs/pr-ci.md).
- `packaging/linux.sh` builds DEB/RPM packages with polkit policy and hicolor icons. Portable Windows distribution uses the application executable.
- `python scripts/release.py <patch|minor|major|X.Y.Z>` synchronizes root `Cargo.toml` and its local package entry in `Cargo.lock`, then prepares the release commit/tag. Root `Cargo.toml` is the application version source. Python handles development tooling; only icon generation requires the pinned binary wheels in `requirements-dev.txt`.
- Require a clean tree on `main`. Annotated `vX.Y.Z` tags are immutable. Confirm atomic branch/tag push; the command supports `--dry-run`, `--no-push`, `--yes`, `--sign` and `--no-sign`.
- Tag push triggers build/draft-release workflows; publishing updates APT/RPM repositories through `update-repository.yml`. Preserve metadata validation, restore-before-update, explicit bootstrap and no-downgrade checks.
- Preserve GPG signatures, provenance, signed updater feed and verification-key compatibility for supported clients. [Windows updates](docs/windows-updates.md) defines update policy and signing contracts.

## Validation

Use the [validation guide](docs/migration-validation.md) for test commands and device acceptance. Unattended checks must use mocks/demo; UAC/polkit, firmware persistence and live service acceptance require device-owner authorization.
