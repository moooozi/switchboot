# Native migration validation

The `PR validation` workflow runs on pull requests targeting `main`, pushes to
`main`, merge-queue groups and manual dispatch. It checks formatting, Clippy, Rust
and tooling tests on Windows and Ubuntu, compiles the native executable, and
assembles unsigned packages.
It never installs a service or package, changes firmware, reboots, signs artifacts
or publishes a release. Its artifacts are for testing rather than distribution.

The package tests build real Inno Setup, DEB and RPM containers around harmless fixture
files. Linux assertions check executable permissions, desktop launcher, icons,
CLI symlink, polkit annotations and package metadata. CI sets
`SWITCHBOOT_REQUIRE_PACKAGING_TESTS=1` so missing host tools or app icons fail
instead of silently skipping the host's package test. Linux also launches
`switchboot --demo` under Xvfb and requires it to remain running for 15 seconds.
This checks startup, not rendering quality or interactive behavior.

## Local automated checks

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
node --test scripts/release.test.mjs scripts/icons.test.mjs scripts/app-icons.test.mjs
python -B -m unittest discover -s scripts -p 'test_*.py'
cargo run -- --demo
```

Package tests need generated app icons (`node packaging/generate-app-icons.mjs`),
Inno Setup 6.7 or newer on Windows, or Bash, dpkg-deb, rpm and rpmbuild on Linux. Linux package
assembly additionally needs `pnpm icons` and a release binary. Container tests
extract/query packages; they do not run their contents.

## Opt-in device acceptance

Run these checks separately on a disposable VM with snapshots or a test device.
Record OS/build, application commit, package version, observed behavior and logs.
Automated mocks cannot validate UAC, polkit, firmware persistence or installed
service behavior. No real-device acceptance results are claimed yet.

Installed Windows self-updates and the Tauri-to-Inno handoff also require the
disposable-VM checks in [Windows updates](windows-updates.md). Preserve the
existing signing key when creating those test artifacts. Do not point the public
feed at a migration build until that acceptance path has been verified.

1. In demo mode, reorder entries, save/discard, undo/redo, change BootNext and
   firmware setup, and exercise restart confirmation. Confirm these stay within
   the demo backend. Drag entries within the boot order and between it and Others;
   check edge scrolling, canceled drops and undo/redo. Check keyboard controls and
   window scaling visually. Launch `--demo` again and confirm the existing window
   activates without opening another; repeat with the existing window minimized.
2. On Linux, install the validation DEB/RPM on its supported distribution. Verify
   `/usr/bin/switchboot-cli` resolves to `/usr/bin/switchboot`, the desktop entry
   launches a non-root GUI, icons appear, and a graphical polkit authentication
   agent is running. First inspect the entries without changing them.
3. On Windows, first test portable mode as a standard user: inspect entries,
   verify UAC transport behavior, and confirm shortcuts are disabled. Install the
   Inno Setup package on a separate snapshot, then use `sc.exe qc swboot-cli` to verify
   the service points to the installed `switchboot.exe --cli /service_connector`.
   Launch from the Start menu as a standard user and inspect entries. Check
   `sc.exe query swboot-cli` while the GUI uses the service.
4. Only after explicit device-owner authorization, record original BootOrder,
   BootNext and firmware-setup state. Make one reversible order change, Save,
   refresh and verify persistence. Restore the exact recorded order. Test a
   BootNext change and clear/restore it without rebooting; verify undo/redo.
   Do the same for firmware setup, restoring its original state.
5. Test canceled authentication and unavailable transport: errors must surface,
   busy state must clear, and state/history must remain recoverable. Avoid
   deleting real boot entries or altering boot files.
6. Windows installed-mode lifecycle: close the GUI, install a newer validation
   build, inspect the service executable path, relaunch and confirm entries are
   readable. Uninstall and verify the service, Start menu shortcut and installed
   resources are removed. Record uninstall errors rather than ignoring them.
   Also upgrade an existing NSIS installation: confirm the install directory is
   retained and the old uninstall registration is removed only after success.
   Check the installer theme at 100%, 150% and 200% scaling: Next/Install uses
   the app's cyan and secondary buttons use neutral gray pills in dark mode.
   Verify hover, pressed and disabled states, Tab/Shift+Tab focus, Enter/Escape,
   Back navigation and Browse. High-contrast mode must retain native controls.
   Trigger Folder Exists and cancel-install confirmations. Check their pill
   buttons, default action, Tab/Enter/Escape behavior and repeated opening/closing.
   Confirm the welcome, brief introduction, donation link and installation path
   share one setup page, including upgrades. Install should go directly to
   progress without a separate review page. Check logo padding and wrapped text.
   Check both light and dark mode in the reduced-height window. The path field
   should have rounded corners, padding and a cyan focus border. Verify typing,
   selection, copy/paste, path completion and Browse still update the install path.
   Browse should use Inno's folder tree with rounded path and list containers.
   Check typed paths, tree selection, spaces and Unicode, drive roots, keyboard
   navigation and cancellation. Tree selection retains Inno's behavior of
   appending the Switchboot subfolder; typed paths are accepted as entered.
   Check the rounded scrollbar thumb after expanding/collapsing folders, wheel
   and keyboard scrolling, track clicks and thumb dragging in both color modes.
7. Actual reboot and boot-target verification is a separate owner-authorized
   device test. Ensure console/recovery access and a known-good boot path before
   selecting it. Restore firmware state after the test.

## Current execution limits

### Owner-authorized update check on 2026-10-16

#### Follow-up: native `/UPGRADE` handoff

The native shutdown failure below was addressed with a per-GUI named event.
The GUI exits on an accepted request; its CLI worker and service use their
existing disconnect lifecycle. No upgrade-specific transport shutdown API was
added. Setup waits for the exact GUI process and exclusive executable access
before attempting service removal or replacing files.

The handoff-enabled 0.3.0 build was installed, and a genuine 0.3.1 binary and
installer were built using a temporary synchronized version bump. Cargo.toml,
Cargo.lock and package.json were restored byte-for-byte afterward. The baseline
GUI was reopened immediately before testing. With its worker and service running,
Setup was invoked with:

```text
Switchboot-0.3.1-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /RESTARTAPP /UPGRADE /UPGRADEPID=6364 /LOG=<file> /LOGCLOSEAPPLICATIONS
```

Setup signaled the GUI at 11:26:18.589, observed GUI exit at 11:26:18.854,
and confirmed executable release at 11:26:18.955 (local time). Restart Manager
then found no applications using the executable. Setup returned 0, reported no
Windows restart required, and reopened the app through its original-user Run entry.

The installed executable and Inno registration both reported 0.3.1. Its SHA-256
matched the built payload:
`01DFEDFD30D4F6A7ECB23E0D92ABDF49DC72F1523F62EE9FCED31B6211418375`.
None of the baseline process IDs remained. The 11:26:35 snapshot contained one
GUI window (PID 1568), its worker, and the running demand-start service. There
was no legacy NSIS registration. No process-killing command, firmware mutation,
or Windows restart was used for this handoff test.

Five harmless fixture tests also exercised the shared Inno handoff script:
success, missing PID, wrong executable, GUI timeout, and a worker holding the
executable after GUI exit. Failure cases installed no payload, and the timeout
cases did not kill their fixture processes. Production UAC cancellation, later
installer failures, and the public feed/download/signature/in-app launch flow
remain separate acceptance checks.

Evidence is retained under ignored `artifacts/upgrade-handoff-acceptance/`,
including both versioned binaries/installers, `before.json`, `native-before.json`,
`native-update.log`, `native-result.json`, `after.json`, and restored source hashes.
The installed app is now the local acceptance build 0.3.1; tracked source versions
remain 0.3.0.

#### Initial native update attempt, before handoff

The baseline was the installed Tauri 0.2.6 binary, with both legacy NSIS and
pre-existing Inno 0.3.0 registrations. A fresh release build was packaged as
0.3.0 using the current installer script. An initial invocation with native
silent arguments exited with code 2 and left the Tauri binary unchanged.

The requested compatibility invocation then succeeded:

```text
Switchboot-0.3.0-setup.exe /P /R /UPDATE /NORESTART /LOG=<file> /ARGS
```

Setup returned 0, removed the NSIS installation, installed Iced 0.3.0, and
reopened a GUI. The installed binary hash matched the build. Only the Inno
registration remained. The demand-start service was running after GUI startup.

Next, a genuine 0.3.1 executable and installer were built with a temporary,
synchronized version change in Cargo.toml, Cargo.lock and package.json. All
three source files were restored byte-for-byte after building. With the Iced
0.3.0 GUI still open, the bumped installer was launched with:

```text
Switchboot-0.3.1-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /RESTARTAPP /LOG=<file> /LOGCLOSEAPPLICATIONS
```

**Native automatic closure failed.** Setup removed the service, then Restart
Manager detected two processes using switchboot.exe: the GUI and its hidden
`--cli /service_manager` worker. It reported that some applications could not
be shut down. The suppressed Abort/Retry/Ignore prompt defaulted to Abort,
and Setup exited with code 5 before replacing files. The original GUI and
worker remained, and the installed binary hash and registration stayed at 0.3.0.
This test does not establish which process prevented shutdown.

Because Setup had already removed the service, its registration was restored
with the installed 0.3.0 executable's `--cli /install_service` command and the
service was started and observed running immediately afterward. A later snapshot
showed no GUI/worker processes and a stopped demand-start service; the cause of
that later closure was not established. No firmware mutation or Windows restart
was requested. The installed app remains Iced 0.3.0 with its service registered.

The compatibility handoff passed, but this initial native open-app upgrade
attempt failed. Public feed/download/signature verification and the in-app update UI
were not exercised. Evidence, binaries, installers and logs are retained under
ignored `artifacts/native-update-acceptance/`, including `before.json`,
`iced-baseline.json`, `native-before.json`, `legacy-install.log`,
`native-update.log`, `native-result.json`, `service-restored.json` and `after.json`.

### Owner-authorized migration check on 2026-10-15

#### Follow-up: passive automation with the old GUI open

After the owner reinstalled 0.2.6 and opened its GUI, the passive compatibility
path was integrated into `packaging/windows.iss`. The installed executable's
hash again matched the recorded 0.2.6 baseline. The service was running, and the
baseline contained the legacy 0.2.6 registration plus the pre-existing Inno 0.3.0
registration. This remains a mixed baseline rather than a clean-machine test.

The real Inno script passed the fixture packaging compilation test and was
compiled around the release binary. It was launched with:

```text
Switchboot_x64-setup.exe /P /R /UPDATE /NORESTART /LOG=<file> /ARGS
```

Setup logged the resolved legacy directory, one queued Install click and
successful NSIS removal, then installed 0.3.0 and launched the GUI as the original
user. Setup returned 0 and reported no Windows restart required. The three old
process IDs were gone; a new window titled Switchboot was present and the service
was Running. The installed binary hash matched the release build. Only the Inno
registration remained, and the old NSIS uninstaller was absent.

NSIS removal closed the old app before Restart Manager queried open files;
Restart Manager reported no remaining application. This validates the open-app
legacy migration path, but not native Restart Manager closure in isolation.
The public feed/download/signature path was not exercised. No firmware changes
or Windows restart were requested. Evidence is retained in ignored
`artifacts/migration-passive/before.json`, `after.json`, `result.json` and
`update.log`.

#### Initial attempts, before passive automation

The installed executable and NSIS uninstaller were both 0.2.6 in the standard
Program Files directory. There was also a pre-existing Inno 0.3.0 registration
and uninstaller in that same directory. This was a mixed installation baseline,
not a clean-machine test. The demand-start `swboot-cli` service was stopped and
no GUI was running. Baseline files and selected registry/service metadata were
saved under ignored `artifacts/migration-test/` before changing the installation.

The current release binary and an Inno 6.7.3 installer were built locally.
Invoking the installer with `/P /R /UPDATE /NORESTART /LOG=<file> /ARGS` failed in
`InitializeSetup` with `Unable to start silent update: Access is denied.` No
installed file or legacy registration changed. Source inspection confirmed that
Inno prohibits script execution of its source installer before `ssInstall`.
The non-working self-relaunch adapter was replaced with an explicit compatibility
error. At that point, legacy argument compatibility remained unimplemented;
the later passive automation described above replaced that interim behavior.

A second invocation with `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /UPDATE
/RESTARTAPP /LOG=<file>` succeeded (Setup exit code 0). The log recorded successful
NSIS removal, native installation and an original-user `[Run]` launch. Only the
Inno uninstall registration remained; the legacy `uninstall.exe` was absent.
Installed version 0.3.0 matched the built executable's SHA-256:
`90A508B6BDD32D3C7B6723970893A850CB79683C459166F69A7655B7EB0A3697`.
The service was observed running and the GUI/service-manager processes were
observed launching immediately after installation. They were no longer running
at the later snapshot, and the service was stopped. Sustained GUI operation and
the cause of that later exit were not established by this check.

Setup reported that no Windows restart was required. No firmware mutation or
reboot was requested. Restart Manager found no open application during migration,
so this run did not validate automatic closure of a running GUI, slow shutdown,
locked files or the in-app download/signature-verification path. Logs and snapshots
are `artifacts/migration-test/legacy-update.log`, `native-update.log`,
`native-result.json`, `before.json` and `after.json`. The public update feed and
release signing secrets were not used or changed.

### Windows checks recorded on 2026-10-11

After deferring the in-app updater, formatting and workspace Clippy with warnings
denied passed. All 46 workspace tests passed (one child-process fixture is
intentionally ignored when run directly), along with two independent legacy
signer tests and 25 Node tooling tests. The Python suite ran 35 tests with seven
expected host/tool skips and no failures. The locked debug executable build
also passed.

A demo-only Windows process smoke check observed repeat launches exiting with
code 0, one surviving GUI process, restoration of the minimized Switchboot
window, foreground focus and empty stderr. The probe waited for startup and
checked the window title before retaining its native handle; an earlier probe
that captured a handle immediately during startup failed its restoration check.
Only processes launched by the check were stopped. These observations do not
replace interactive drag-and-drop, scaling or device acceptance.

The migration development machine is Windows. WSL is not installed and Docker
is unavailable, so Linux compilation, graphical startup and package assembly
must be verified by the workflow or a separate Linux host. A checked-in workflow
is not evidence of a passing run; record its run URL and results after pushing.
