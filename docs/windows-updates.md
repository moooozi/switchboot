# Windows application updates

[Windows packaging](../packaging/nsis/README.md) configures the NSIS installer.
This guide defines application update policy, handoff and signing.

## Application boundaries

`crates/switchboot_updater` owns release checks, bounded HTTPS downloads,
signature verification, temporary installer storage and process observation.
`src/updater.rs` supplies the application version and installed-mode policy.
`crates/switchboot_ui` owns update presentation and guards against installation
while boot-order edits or firmware operations are pending.

Self-updates are available in installed Windows mode. Installed-mode detection
compares the running executable's directory with the Windows service executable's
directory. Linux uses its package manager; Windows portable mode has no self-update
controls. Demo mode simulates an update locally without network or installer access.

Installed builds check the feed at startup. A newer stable semantic version adds
an **Upgrade to vX.Y.Z** link below the application title. Background check failures
are quiet. Selecting Upgrade displays downloaded bytes and progress, verifies the
installer and launches Setup. User-initiated failures provide an error and retry.

## Verification and launch

Downloads require HTTPS, timeouts and size limits. The updater verifies the base64
Minisign signature with its configured public key before writing executable bytes
to a random temporary directory. Unused staging is cleaned up; a launched installer
keeps its source available through application shutdown.

The installer starts with:

```text
/S /RESTARTAPP /UPGRADE /UPGRADEPID=<GUI PID>
```

Setup starts as the user; the worker requests UAC for machine changes. The updater
observes the installer process separately from the application shutdown request.
If Setup exits while the GUI is open, the UI can report cancellation or failure.
After GUI exit, Setup owns error reporting. Setup has no explicit `/LOG` option.

## Shutdown and replacement

The installed GUI creates the session-local event `Local\Switchboot.Upgrade.<PID>`.
Its handles are not inherited by CLI children. An existing event is an error.

After [ownership validation](../packaging/nsis/toolkit/README.md#file-ownership-protocol)
and app preflight, the worker opens the supplied PID, verifies its executable
matches the installed app, signals the event and waits up to 35 seconds for exit.
A rejected preflight or shutdown timeout stops installation before service/file
changes. The GUI accepts shutdown only when its operation guards allow it; closing
its pipes lets CLI workers and the service finish their disconnect lifecycle.

The worker then follows the [replacement lifecycle](../packaging/nsis/toolkit/README.md#lifecycle-hooks),
with Switchboot hooks removing/configuring its service. Generic replacement,
fallback, uninstall and failure semantics are specified by the toolkit.

## Updater client compatibility

The Switchboot adapter accepts Tauri/NSIS client arguments `/UPDATE /P /R /ARGS`.
`/UPDATE` selects unattended replacement and `/R` requests a GUI launch. Tokens
after `/ARGS` are not replayed. Registered Tauri/NSIS installations are removed
through their own uninstaller before the NSIS payload is installed.

Application option parsing, event naming, service commands and compatibility
policy live in `packaging/nsis/switchboot-hooks.nsh` and `service-hooks.nsh`.
The toolkit transports validated consumer arguments through both interactive and
silent worker launch routes without interpreting Switchboot flags.

## Signing and acceptance

Updater clients use the `windows-x86_64` feed entry and the configured verification
key. `TAURI_SIGNING_PRIVATE_KEY` and its password are the release signer inputs;
key rotation requires an explicit client compatibility plan. Minisign updater
verification, GPG artifact signatures and build provenance serve separate purposes.

Use the [local update harness](local-update-test.md) for loopback HTTPS testing.
Use the [validation guide](migration-validation.md) for UAC, installed service,
file preservation, cancellation and device checks. Mocks test sequencing and
failure behavior; installed-device checks test Windows integration.
