# Windows NSIS installer

Switchboot uses stock NSIS 3 and the [NSIS Kit](toolkit/README.md) native Rust/Win32
interface. This directory supplies app configuration and lifecycle hooks; the
[toolkit reference](toolkit/README.md#presentation-and-maintenance) defines the
shared UI, elevation and maintenance behavior.

## Switchboot configuration

| Setting | Value |
| --- | --- |
| Fresh installation directory | `$PROGRAMFILES64\Switchboot` (fixed) |
| Application | `switchboot.exe` |
| 64-bit HKLM uninstall key | `Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot` |
| Ownership marker | `SwitchbootInstaller = native-nsis` |
| Service commands | Installed executable with `--cli /install_service` or `--cli /uninstall_service` |

| File | Responsibility |
| --- | --- |
| `setup.nsi` | Branding, destination, registration identity and hook includes |
| `switchboot-hooks.nsh` | Update options, preflight/shutdown, icon extraction, client compatibility and replacement hooks |
| `service-hooks.nsh` | Checked service commands and messages |
| `owned-icons.nsh` | Generated enumeration shared by ownership publication and uninstall |
| `build.py` | App/icon compilation and installer assembly |
| `test_installer.py` | Private mock lifecycle and UI integration tests |

Hooks follow the [toolkit lifecycle contract](toolkit/README.md#lifecycle-hooks).
[Windows updates](../../docs/windows-updates.md) defines Switchboot's updater flags,
GUI event protocol and signing. The [ownership protocol](toolkit/README.md#file-ownership-protocol)
defines replacement, fallback, path validation and failure behavior.

Legacy Tauri upgrades automatically run the previous uninstaller in its registered
directory, then install into `$PROGRAMFILES64\Switchboot`. User files left in the
old directory are preserved. Failed removal stops the upgrade before extraction.

## Build

Requires Windows 10/11 x64, stock NSIS 3, Python 3.11+, Rust/MSVC with x86 and
x64 targets and Visual Studio C++ build tools. Icon tooling uses prebuilt Python wheels.
From the repository root:

```console
git submodule update --init --recursive
python -m pip install --only-binary=:all: -r requirements-dev.txt
python packaging/nsis/build.py
```

The builder generates icons, compiles the app and writes
`artifacts/Switchboot_x64-nsis-setup.exe` using the root package version.
`--output path/to/setup.exe` selects a separate artifact. It checks PE architecture,
plugin exports, fixture-code exclusion, NSIS warnings and the unelevated manifest;
it never runs Setup. `--preview` delegates to the generic toolkit preview builder.

CI builds the application once and supplies `--payload target/release/switchboot.exe`
to package that exact executable; icons must already have been generated. The same
executable is uploaded as the portable build.

Validation and release packaging download `native_nsis_ui.dll` and `plugin.json`
from the `dll-<toolkit-commit>` release in `moooozi/nsis-kit`, matching the pinned
submodule commit, and pass `--plugin-artifact artifacts/nsis-dll`.
The builder rejects a DLL with a mismatched toolkit revision, feature set, target
or checksum. A missing release fails packaging rather than silently recompiling
or using a different revision. Local builds without these options continue to
compile the application and DLL from source.

## Tests and previews

[Validation commands](../../docs/migration-validation.md#automated-checks) cover the
independent toolkit and Switchboot adapter. App tests run a mock executable with
private HKCU registration and temporary files, exercising service-command results
and GUI handoff without real firmware or service changes.

[Generic previews](toolkit/README.md#harmless-ui-previews) provide build/test commands,
artifact names and desktop-test requirements. For real UAC, accessibility, service
integration and signed delivery, use [device acceptance](../../docs/migration-validation.md#windows-installer-acceptance).
See [CI configuration](../../docs/pr-ci.md) for workflow constraints.
