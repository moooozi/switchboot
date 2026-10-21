# Validation guide

Switchboot separates automated mock checks from device operations. Unattended
checks use demo backends, private registry keys and temporary files. Firmware
changes, live service installation and restarts require device-owner authorization.

## Automated checks

Initialize the toolkit and generate icons before building:

```console
git submodule update --init --recursive
python -m pip install --only-binary=:all: -r requirements-dev.txt
python scripts/generate_icons.py
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python -B -m unittest discover -s scripts -p 'test_*.py'
```

Windows packaging and updater checks:

```console
python packaging/nsis/toolkit/ci.py
python packaging/nsis/test_installer.py
cargo test --locked -p switchboot_updater --all-targets
cargo test --locked --manifest-path packaging/updater-sign/Cargo.toml
```

The [toolkit validation reference](../packaging/nsis/toolkit/README.md#validation)
describes its independent workspace and fixture coverage. Run optional
[desktop previews](../packaging/nsis/toolkit/README.md#harmless-ui-previews) according
to their desktop isolation and timing requirements.

On Linux, package-container checks require Bash, dpkg-deb, rpm and rpmbuild.
They query/extract DEB and RPM contents without installing packages. See [CI configuration](pr-ci.md) for workflow commands and configuration constraints.

## Demo UI checks

```console
cargo run -- --demo
```

Check boot-order reordering, moves between Boot order and Others, edge scrolling,
keyboard controls, canceled drops, Save/Discard and undo/redo. Each completed drop
creates one undoable change; changes stay local until Save. Check BootNext,
firmware-setup selection and restart confirmation against the demo backend.

Launch demo mode twice and confirm the existing window activates, including when
minimized. Inspect light/dark rendering, scaling, keyboard focus and the simulated
update link/progress modal. Demo operations must remain inside the demo backend.

## Windows installer acceptance

Use a test device or disposable VM with recovery available. Record the commit,
OS/build, setup and installed versions, observations and relevant logs separately
from reference documentation.

- Check the [installer presentation contract](../packaging/nsis/toolkit/README.md#presentation-and-maintenance)
  in light, dark and high-contrast modes: text, logo, keyboard access, scaling,
  destination, all-users note and disk-space requirement.
- Exercise Install, Upgrade, Reinstall, Downgrade and Uninstall. Check both displayed
  versions and close-all-instances wording. Cancel UAC and confirm the operation
  can be retried without installation changes.
- Confirm installed mode uses `swboot-cli` and its installed executable. Inspect
  entries without changing firmware. Confirm Launch/restart opens an unelevated
  GUI, and direct uninstall asks for confirmation unless `/S` is supplied.
- Test ownership cleanup with obsolete packaged files and unowned user files.
  Verify malformed supported manifests stop before shutdown or mutation; absent
  and unsupported protocols select uninstall-then-install.
- Exercise failures caused by file locks and integration command errors. Verify
  useful errors and retry behavior against the toolkit failure contract.
- Use the [local signed update harness](local-update-test.md) to exercise download,
  verification, UAC, GUI shutdown, replacement, service setup and a single relaunch.

## Linux and firmware acceptance

On the target Linux distribution, verify the desktop entry and icons, the
`/usr/bin/switchboot-cli -> switchboot` symlink and a graphical polkit authentication
agent. Run the GUI as a normal user and inspect entries before changing firmware.

Only with explicit device-owner authorization, record BootOrder, BootNext and
firmware-setup state. Make one reversible change, Save, refresh, verify persistence
and restore the recorded state. Check undo/redo and canceled authentication.
Confirm errors clear busy state without corrupting the draft or history.

Deleting real entries and restarting are separate device actions requiring explicit
authorization. A demo or mocked success does not establish firmware persistence,
UAC/polkit behavior or installed service correctness.
