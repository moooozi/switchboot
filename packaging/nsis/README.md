# Native NSIS installer candidate

This is the first **real** installer using the custom Rust/Win32 selection UI and
stock NSIS 3. It installs the current Iced executable, shortcut icons, an all-users
Start menu shortcut, an uninstaller and the Windows service. **Running it requests
UAC and changes the machine.** Do not confuse it with the harmless previews.

The released baseline is Tauri's NSIS installer, not Inno. The service hooks were
read from `main:src-tauri/platform/windows/nsis-hooks.nsi` and adapted here. They
still invoke `switchboot.exe --cli /install_service` and `/uninstall_service`;
unlike the old log-only hooks, nonzero results stop the operation. Inno's update
work was migration-branch work, not production acceptance. Its packaging and the
release workflows remain unchanged while this candidate is validated.

## Build without installing

Requirements: Windows 10/11 x64, stock NSIS 3, Rust/MSVC with both the normal x64
target and `i686-pc-windows-msvc`, Python 3.11+ (prefer `uv`), Node/pnpm and installed
repository tooling dependencies. No Tauri runtime or third-party NSIS plug-in is
required. NSIS uses its stock x86 Unicode runtime; the application payload is x64.

From the repository root:

```sh
uv run --no-project python packaging/nsis/build.py
```

The build generates icons, builds the real application and the UI DLL with its
`installer` feature, validates DLL exports in a matching x86 process, and packages
`artifacts/Switchboot_x64-nsis-setup.exe`. The version comes from root `Cargo.toml`.
The script **never runs Setup**. This local candidate is unsigned; it does not
replace `Switchboot_x64-setup.exe` or modify the updater feed/signing pipeline.

## Appearance and interaction

- Light/dark mode follows `AppsUseLightTheme` when a selection window opens.
  `SWITCHBOOT_NSIS_THEME=light` or `dark` is a process-only testing override; it
  never changes the Windows theme. Live theme switching is not implemented yet.
- The existing supersampled rounded controls, native SVG with ICO fallback,
  native editable path, Browse picker, keyboard focus and per-monitor DPI behavior
  are retained. High contrast keeps native controls/system colors.
- Fresh interactive installation selects a dedicated absolute application folder.
  Existing installations keep their registered folder and show **Uninstall /
  Reinstall / Launch**. Launch is the primary action. Removal/reinstallation
  requires confirmation, defaults to No, and Escape always cancels.
- NSIS's real progress/details page owns extraction and reports errors. There is
  no simulated progress or unsafe mid-replacement cancellation. The custom UI is
  used for selection/maintenance, not for the engine progress/uninstall pages.
- Successful fresh installation closes Setup; it does not automatically start the
  real application. Reinstall and explicit restart switches request relaunch.

## Installation/update contracts

The installer reuses the 64-bit HKLM uninstall key
`Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot` and its
`InstallLocation`, matching the released per-machine Tauri installation.
`SwitchbootInstaller=native-nsis` distinguishes native ownership from Tauri.
The experimental Inno `_is1` key is also recognized; duplicate registrations are
rejected rather than guessed. Downgrades and silent manual overwrite of an
existing installation are rejected.

- Legacy Tauri `/UPDATE /P /R /ARGS ...` is accepted directly. `/UPDATE` selects
  unattended installation, `/R` requests relaunch, and parsing of installer
  switches stops at `/ARGS`. Application arguments are never replayed.
- Current Iced `/UPGRADE /UPGRADEPID=<pid>` is accepted with its existing
  `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /NOCLOSEAPPLICATIONS /RESTARTAPP`
  arguments. No updater Rust change is required while the two packaging paths
  coexist. `/UPDATE` and `/UPGRADE` cannot be combined.
- A native handoff verifies the **exact GUI executable path**, opens its existing
  `Local\Switchboot.Upgrade.<pid>` event, requests shutdown, and waits on the same
  process handle (35 seconds). Manual native maintenance discovers only matching
  GUI-owned events. CLI workers/services are not killed. An exclusive, non-writing
  executable-open check waits up to 15 seconds before any service removal/copy.
- Legacy Tauri and Inno migrations run their **registered uninstaller**, check its
  exit code, and wait for its registration/application executable to disappear
  before copying new files. NSIS `_?=` runs removal in place so the engine waits
  for the actual uninstaller. The old owner handles its own service/GUI cleanup.
- Native updates perform shutdown, remove the old service, copy files, register
  the uninstaller, then install the service. Service failure is a nonzero Setup
  result, never a successful update. Failed service setup keeps registration so
  a later interactive reinstall can repair it; it is not transactional rollback.
- Launch/restart uses the interactive shell's **unelevated token and user
  environment**, never Setup's elevated token. It fails explicitly if that token
  cannot be acquired or is elevated; there is no elevated GUI fallback.
- Uninstall verifies ownership, completes native shutdown, and removes the service
  before deleting installer-owned files. It does not recursively delete the
  application directory or user content. Old stale uninstallers cannot remove a
  different registered installation.

## Safe validation

Build the real candidate first to generate icons and the installer-feature DLL.
Then run from the repository root:

```sh
cargo fmt --manifest-path packaging/nsis-poc/ui/Cargo.toml -- --check
cargo clippy --locked --manifest-path packaging/nsis-poc/ui/Cargo.toml --target i686-pc-windows-msvc --all-targets --features installer -- -D warnings
cargo test --locked --manifest-path packaging/nsis-poc/ui/Cargo.toml --target i686-pc-windows-msvc --features installer
uv run --no-project python packaging/nsis/test_installer.py
```

`test_installer.py` compiles a **separate, unelevated fixture** from the real NSIS
source and service hooks. It replaces every payload with a tiny mock executable,
uses a unique private HKCU subtree and temporary directories, redirects shortcuts
inside that directory, and stubs original-user launch. It never executes real
Switchboot, calls SCM/firmware, opens real installation keys or runs the real
installer. It covers fresh install, legacy-style native update, reinstall,
uninstall/user-file preservation, service failure, missing uninstaller, downgrade
and silent overwrite rejection, foreign PID rejection, and native handoff to a
mock GUI event. Private registration and files are cleaned up afterwards.

Harmless visual desktop checks (these briefly open/move **only preview windows**):

```sh
uv run --no-project python packaging/nsis-poc/build.py --renderer svg
uv run --no-project python packaging/nsis-poc/test_preview.py --renderer svg --theme dark
uv run --no-project python packaging/nsis-poc/test_preview.py --renderer svg --theme light
uv run --no-project python packaging/nsis-poc/build.py --maintenance
uv run --no-project python packaging/nsis-poc/test_maintenance.py
```

These visual builds omit the `installer` feature and cannot call the OS lifecycle
exports. Rebuild the real candidate before rerunning lifecycle fixture tests.
Maintenance fixtures only return action exit codes; they never remove or launch
an installed application. Captures include `artifacts/nsis-preview.png` and
`artifacts/nsis-maintenance-light.png` / `nsis-maintenance-dark.png`.

## Required before production promotion

The compile/mock/visual checks are **not** device or production acceptance. In a
snapshot-capable VM, explicitly validate fresh UAC install, real SCM/service
failure behavior, original-user launch (including over-the-shoulder elevation),
GUI refusal with unsaved changes, busy workers, repair/uninstall, upgrade from the
actually released Tauri version, and any locally installed Inno candidate. Check
high contrast, screen readers, keyboard-only navigation and additional physical
DPI configurations manually. There is no live firmware/restart test here.

Finally validate the **full old-client feed/download/signature/launch path** with
the unchanged updater key before changing release workflows. An accepted NSIS
command line and a mock update do not prove that signed production update path.
Keep Inno output and the signed updater feed untouched until the owner authorizes
promotion. No release is published or installed by these commands.
