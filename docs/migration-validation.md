# Native migration validation

The `Native validation` workflow runs on feature-branch pushes, pull requests and
manual dispatch. It checks formatting, Clippy, Rust and tooling tests on Windows
and Ubuntu 22.04, compiles the native executable, and assembles unsigned packages.
It never installs a service or package, changes firmware, reboots, signs artifacts
or publishes a release. Its artifacts are for testing rather than distribution.

The package tests build real NSIS, DEB and RPM containers around harmless fixture
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
node --test scripts/release.test.mjs scripts/icons.test.mjs
python -B -m unittest discover -s scripts -p 'test_*.py'
cargo run -- --demo
```

Package tests need generated app icons (`node packaging/generate-app-icons.mjs`),
NSIS on Windows, or Bash, dpkg-deb, rpm and rpmbuild on Linux. Linux package
assembly additionally needs `pnpm icons` and a release binary. Container tests
extract/query packages; they do not run their contents.

## Opt-in device acceptance

Run these checks separately on a disposable VM with snapshots or a test device.
Record OS/build, application commit, package version, observed behavior and logs.
Automated mocks cannot validate UAC, polkit, firmware persistence or installed
service behavior. No real-device acceptance results are claimed yet.

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
   NSIS package on a separate snapshot, then use `sc.exe qc swboot-cli` to verify
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
7. Actual reboot and boot-target verification is a separate owner-authorized
   device test. Ensure console/recovery access and a known-good boot path before
   selecting it. Restore firmware state after the test.

## Current execution limits

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
