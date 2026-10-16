# Switchboot AI Coding Guidelines

## Architecture

Switchboot is a Rust 2024 desktop application using iced 0.14 to manage EFI boot entries on Windows and Linux.

- Root `switchboot` package: `src/main.rs` routes GUI, CLI, shortcut and Windows service modes; `src/backend.rs` implements the UI backend interface.
- `crates/switchboot_ui`: native iced views, typed messages, blocking-worker tasks, pure state/history model, and in-memory demo backend. It does not depend on platform firmware code.
- `crates/switchboot_core`: privileged CLI operations, platform APIs, JSON stdin/stdout transport and secure named-pipe IPC. It does not depend on iced.
- `crates/switchboot_updater`: signed release checks, bounded downloads, verification and Windows installer launch/observation. It depends on neither iced nor firmware code. The root adapter supplies application version and installed-mode policy; the UI owns update presentation and state. Native `/UPGRADE` uses a per-GUI event: Inno requests GUI exit after preflight, then waits for process exit and executable release before service removal or file replacement. CLI workers and the service close through their existing disconnect lifecycle. Keep legacy `/UPDATE` separate; Inno owns replacement and restart.
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
- Test state behavior with mock backends, never live firmware or real restarts.

## Development

```sh
cargo run -- --demo              # Safe UI development; no EFI access/elevation/reboot
cargo run                       # Real firmware backend
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm install --frozen-lockfile  # Optional Node icon/release tooling
pnpm icons
node packaging/generate-app-icons.mjs
cargo build --release --bin switchboot
```

Linux real firmware access needs polkit, a graphical authentication agent and the sibling `switchboot-cli` symlink. Do not run the real GUI as root. Windows portable mode uses UAC; installed mode uses the Windows service. Shortcuts remain disabled in Windows portable mode.

## Packaging and releases

- `packaging/linux.sh`: native DEB/RPM packages with polkit integration and hicolor icons.
- `packaging/windows.iss`: Inno Setup entry point with declarative sections and all Inno event handlers. It delegates registration/uninstaller helpers, shutdown, maintenance, legacy migration and service lifecycle to the adjacent `windows-*.iss` modules; modules own their state and do not declare Inno event handlers. Portable release is the same executable.
- `issconfig`: upstream visual-library submodule; initialize with `git submodule update --init --recursive` before packaging tests. Keep application lifecycle hooks in `packaging/windows.iss`.
- `tools/prerender-icons.js`: shortcut icons from `static/os-icons` and `crates/switchboot_core/icons-raw`.
- `packaging/generate-app-icons.mjs`: app PNG/ICO generation without Tauri.
- Node/pnpm is only used for icon and release tooling; no JavaScript frontend or webview runtime is needed.
- `pnpm release <patch|minor|major|X.Y.Z>` is the version/commit/tag entry point. It synchronizes root `Cargo.toml`, `Cargo.lock` and `package.json`.
- Releases require a clean tree on `main`. Annotated `vX.Y.Z` tags are immutable; never replace an existing tag. The script asks before atomic branch/tag push (`--dry-run`, `--no-push`, `--yes`, `--sign`, `--no-sign` are supported).
- Tag push builds Linux/Windows and opens a draft release; publishing it updates APT/RPM repositories through `update-repository.yml`. Preserve metadata validation, restore-before-update, explicit bootstrap and no-downgrade checks.
- Retain GPG signatures and build provenance. Preserve existing signed updater feed and key compatibility for previous releases. In-app updates are restricted to installed Windows mode; Linux uses native package updates and Windows portable/demo modes never self-update. See `docs/windows-updates.md`.

## Migration follow-ups

The native UI supports drag-and-drop ordering and moving entries between the boot order and Others, with edge scrolling and Up/Down controls. Drops remain local until Save and create one undoable change. Repeat GUI launches activate the existing per-user window; demo and real GUI ownership are separate, and CLI/service/shortcut modes bypass GUI ownership.

Remaining gates are Linux CI, interactive desktop validation, and owner-authorized device acceptance for UAC/polkit, firmware persistence and installed Windows service lifecycle. See `docs/migration-validation.md`. Keep unattended checks mock/demo-only. Do not reintroduce Tauri or Svelte.
