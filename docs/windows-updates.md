# Windows updates and the Tauri migration

The updater lives in `crates/switchboot_updater`, with separate modules for feed
validation, bounded HTTPS downloads, signature verification and installer staging.
It has no dependency on iced or the firmware core. The root `src/updater.rs`
adapter implements `Installation` using installed-mode detection and maps typed
errors/releases to the UI boundary. It supplies the root application's version
explicitly; the updater crate's own version is unrelated to release comparisons.
The UI's `updates` module owns a typed state machine and its view. Application
code retains the shared guard against firmware operations and unsaved changes.

This crate boundary isolates the network, cryptography and Windows process code
for testing without widening the privileged firmware core. Both update entry
points reject non-Windows hosts and ask the host for an installed directory
before requesting the feed. Installation revalidates that directory before
launch. Unused staging is removed automatically; a launched installer keeps its
source available in temporary storage so it survives application shutdown.

Installed Windows builds check the existing HTTPS feed at startup and expose a
manual retry. A quiet **Upgrade to vX.Y.Z** link below the application title
starts the download with one click. A centered **Updating Switchboot** modal
shows downloaded bytes and a progress bar; verification and installer launch
follow automatically. Release notes and a separate install panel are omitted; unsaved boot-order edits,
discovery and pending firmware operations block installation. Linux and Windows
portable mode do not check for updates or show update controls. Demo mode always
offers a local sample update so its link and progress modal can be
previewed on any platform. Checking repeats the sample; clicking Upgrade simulates download progress, then
shows a preview-only message without any network access, elevation, installer launch or restart.
The backend independently checks installed mode before checking/downloading and
again before launching Setup. Firmware IPC and the privileged service are not
used to download or execute update payloads.

The updater accepts newer stable semantic versions, downloads over HTTPS with
timeouts and size limits, and verifies the base64 Minisign signature with the
public key from the last Tauri configuration. Only verified Windows EXE bytes
are written to a random temporary directory. Inno's loader starts unelevated and
requests UAC internally, preserving its original-user token. Native launches use
`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /NOCLOSEAPPLICATIONS /RESTARTAPP /UPGRADE /UPGRADEPID=<GUI PID>`.
An installed GUI owns the session-local `Local\Switchboot.Upgrade.<PID>` event,
using the creating user's default security descriptor. Handles are not inherited
by CLI children, and an existing event is treated as an error.

After elevation and preflight, `PrepareToInstall` opens the specified GUI process,
verifies its executable matches the selected installation, and signals its event.
Only a GUI with no unsaved edits or pending operations accepts the request and
exits. Closing its pipes lets the CLI worker and service finish their existing
disconnect lifecycle; no update-specific shutdown code runs in the CLI transport.
Setup waits up to 35 seconds for that exact process handle, then up to 15 seconds
for exclusive write access to the executable. This checks that the worker and
service released the file without writing bytes or killing processes. Failure
aborts before service removal or file replacement. The implementation lives in
`packaging/windows-shutdown.iss` and is shared with harmless fixture tests.

Firmware and draft controls remain locked while Setup runs. Canceling UAC or
failing preflight leaves the GUI and service running; process polling releases
the UI guard if Setup exits before the GUI closes. Once the GUI has exited, Setup
owns reporting later errors. `/NOCLOSEAPPLICATIONS` prevents Restart Manager
from closing any CLI instances during native handoff updates. Normal installs
and legacy migration retain the installer's `CloseApplications=yes` behavior.

Setup silently reuses the existing Inno installation directory through the stable
`AppId` and Inno's default `UsePreviousAppDir=yes`; the updater does not pass `/DIR`.
A single `[Run]` entry
reopens the GUI as the original user only after service setup succeeds.
`RestartApplications=no` avoids a second restart through Restart Manager and
also accommodates Tauri clients that already exited before launching Setup.
Using `RestartApplications=yes` instead would require the application to call
[RegisterApplicationRestart](https://jrsoftware.org/ishelp/topic_setup_restartapplications.htm).
Installer process polling observes completion separately from the shutdown event.
The staged installer stays in Windows temporary storage so the exiting process
does not delete Setup's source while it is starting. Normal temporary-storage
cleanup may reclaim it later. A launched installer can still fail. Setup's own
error handling applies, and nonzero exit codes are reported if the app is still open.

Keep `TAURI_SIGNING_PRIVATE_KEY` and its password unchanged in release secrets.
The Windows workflow signs the final `Switchboot_x64-setup.exe` directly, using
the same base64 Minisign format as Tauri. The feed keeps `windows-x86_64` and the
existing endpoint. GPG signatures and build provenance remain separate from this
updater signature. No Tauri runtime or frontend is reintroduced.

## Installer source layout

`packaging/windows.iss` is the entry point: it keeps the declarative installer
sections and all Inno event handlers together. Events delegate in lifecycle order
to these modules; includes do not register hidden Inno event handlers.

| Module | Owns |
| --- | --- |
| `windows-installation.iss` | Setup arguments (`/ARGS` boundary included), registry keys, directory/uninstaller lookup, registered-uninstaller execution and completion checks |
| `windows-shutdown.iss` | Native GUI discovery, event signaling, process waits and executable-release checks |
| `windows-maintenance.iss` | Manual Uninstall / Reinstall / Launch workflow and its wizard state |
| `windows-legacy.iss` | Legacy Tauri argument compatibility, passive wizard advancement, NSIS migration and leftover cleanup |
| `windows-service.iss` | Service CLI commands, installation/removal policy and service setup results |

Modules declare their own state. The main file coordinates it: a successful
maintenance reinstall skips redundant removal, legacy migration owns its NSIS
removal, and native updates finish shutdown before service removal. Successful
post-install service setup gates restart and legacy cleanup. `issconfig` remains
responsible only for appearance and layout; it contains no application lifecycle.

## Manual installation when Switchboot is already installed

Launching Setup manually detects the registered native or legacy installation
and shows **Switchboot is already installed** with three actions:

- **Uninstall** runs the registered uninstaller and exits Setup after successful removal.
- **Reinstall** runs the registered uninstaller, confirms the old registration and
  executable were removed, then installs into the same registered directory.
- **Launch** opens the installed application as the original user and exits Setup
  without uninstalling or copying files.

There is no destination selection for an existing installation. Missing or failed
uninstallers stop Reinstall before new files are installed. Native manual actions
first discover matching GUI processes and use the same shutdown event as updates;
workers and services are not killed. The uninstaller also performs this shutdown
check before removing service registration. A GUI with unsaved changes or pending
work may refuse shutdown; finish that work and retry. Legacy NSIS uninstallers
continue to own their legacy application shutdown.

This maintenance page is separate from `/UPGRADE` and legacy `/UPDATE`. Ordinary
silent installs over an existing registration are rejected rather than choosing
a destructive action implicitly. Fresh installations retain directory selection.
The page lives in `packaging/windows-maintenance.iss`; fixture tests use a separate
HKCU installation and harmless payloads, never the real Switchboot service.

## Legacy handoff

The v0.2.6 release's lockfile used `tauri-plugin-updater` 2.9.0. Its
[Windows extraction and launch implementation](https://github.com/tauri-apps/plugins-workspace/blob/updater-v2.9.0/plugins/updater/src/updater.rs)
recognizes PE executables and treats them as NSIS installers; it does not require
an NSIS-specific payload header. Signature validation happens before extraction.
Thus an Inno EXE signed by the existing key is expected to pass that path.
However, recognizing and verifying the EXE is insufficient: the legacy launch
arguments do not put Inno into silent mode. The device test on 2026-10-15
confirmed that the proposed self-relaunch adapter fails before installing.

Inno recognizes `/UPDATE` before Tauri's `/ARGS` marker. `/R` requests restarting
the GUI. App arguments following `/ARGS` are not forwarded to the restarted
application, avoiding replay of privileged CLI actions. Ordinary GUI launches
do not require arguments. `/P` and `/S` are not native Inno silent switches.
[Inno cannot enable silent mode from script](https://jrsoftware.org/isfaq.php),
and its [script helper implementation](https://github.com/jrsoftware/issrc/blob/is-6_7_3/Projects/Src/Setup.ScriptFunc.HelperFunc.pas)
protects the source installer from script execution before `ssInstall`.
Consequently, the installer uses a scoped passive-upgrade path instead of
self-relaunch. `/UPDATE` combined with `/P` or `/S`, before `/ARGS`, enables it
only if the legacy registration resolves to an existing application directory.
The directory comes exclusively from the legacy registration's `InstallLocation`
value in the 64-bit HKLM uninstall key; a missing or invalid value rejects this
automatic compatibility path. Setup does not derive it from `UninstallString`.
The directory is selected automatically and the configuration page is skipped.
On Ready, one queued `BM_CLICK` advances the actual Install button after the page
callback returns. Progress and errors remain visible; Preparing errors, app-close
decisions and reboot choices are never clicked automatically. Reboot choices
default to No. The same installer retains native `/VERYSILENT` behavior for Iced
updates and interactive behavior for normal installations.

This path was integrated and exercised against an open 0.2.6 installation on
2026-10-15. The update completed automatically, reopened the 0.3.0 GUI and left
the service running. NSIS removal closed the old GUI before Restart Manager ran.
The test invoked Setup directly with Tauri-style arguments, plus logging and
`/NORESTART`; it did not use the old client's feed, download or signature checks.
See the detailed record in `docs/migration-validation.md`.

### Button automation evaluation (2026-10-15)

An original [scripted Enter-key example](https://stackoverflow.com/questions/8757161/innosetup-how-to-start-the-silent-installation-after-showing-my-own-form-from-t)
demonstrates the established pattern of skipping pages and posting a message to
advance the wizard. Inno itself simulates Next button clicks in its native silent
path, but that path also adds failure handling; see its
[wizard implementation](https://github.com/jrsoftware/issrc/blob/is-6_7_3/Projects/Src/Setup.WizardForm.pas).
The [PostMessage script function](https://jrsoftware.org/ishelp/topic_isxfunc_postmessage.htm)
can queue a click after the current page callback returns, avoiding synchronous
navigation from inside that callback. Sending `BM_CLICK` to the actual Next button
targets the control directly instead of relying on keyboard focus.

A harmless fixture was compiled and executed using pinned Inno 6.7.3. It runs
unelevated, creates no uninstall registration, does not close other applications,
and installs one text file only under ignored `artifacts/button-evaluation/`.
It neither uses the Switchboot application nor touches services or firmware.
For `/UPDATE` before `/ARGS`, the fixture skips the directory page and posts one
`BM_CLICK` to `WizardForm.NextButton.Handle` on `wpReady`. Its regular welcome,
ready and finished pages use the same disabled settings as Switchboot.

Observed results:

| Invocation | Result |
| --- | --- |
| `/P /R /UPDATE /ARGS` (plus fixture log/case arguments) | Dummy payload installed automatically; exit 0; `WizardSilent=False` |
| Normal launch | Waited for directory input; no payload installed; harness canceled it cleanly, exit 2 |
| Legacy update with injected `PrepareToInstall` failure | Stopped on Preparing; no payload installed; harness canceled it cleanly, exit 7 |

The fixture, logs and result JSON are retained under `artifacts/button-evaluation/`.
This proves unattended advancement through the normal installer path, not native
`/VERYSILENT` semantics or end-to-end Switchboot migration. It does not test
issconfig integration, UAC, actual NSIS removal, service failures, application/file
locks, reboot-required states or focus changes during the queued click.

The resulting implementation uses a narrow legacy **passive upgrade** path with this
pattern, gated on the recognized updater arguments and an existing installation.
Keep progress and failure pages visible. Post only the initial Install click;
do not click through Preparing failures, unexpected pages or reboot choices.
Retain `/R` restart-after-success behavior and ignore app arguments after `/ARGS`.
Normal installs remain interactive, and native Iced updates continue
using `/VERYSILENT`. This avoids an additional launcher for the legacy migration
without trying to recreate every native silent-mode behavior.

During legacy migration, the NSIS uninstaller owns service removal. Setup reads the legacy
`UninstallString` as the quoted uninstaller executable path and runs it with
`/S /UPDATE _?=<InstallLocation>`. `InstallLocation` supplies only the installation
directory; `UninstallString` supplies only the uninstaller. Neither substitutes
for the other, and missing or invalid values stop migration. The NSIS
`_?=` argument must be last and unquoted: it runs in place, allowing Setup to wait
for removal rather than racing a copied uninstaller. If the uninstaller is missing,
cannot be launched or returns a nonzero exit code, Setup stops with an explicit
error before replacing application files, including when installing in the same directory.
Registration and leftover `uninstall.exe` are retired after successful Inno
installation and service setup. Every legacy migration must remove the old
installation successfully. Setup launches the registered executable directly
with the migration arguments, without a command shell.
Native Inno upgrades attempt service removal before replacing files. Removal is
best effort: failures are logged and installation continues. In `/UPGRADE` mode,
the GUI handoff and executable-release checks must succeed before this attempt.
After file installation, Setup registers the demand-start service without another
removal. If best-effort removal left the service registered, installation stops
it and refreshes its configuration and permissions; errors remain fatal to service
setup. The GUI starts the service when needed.

## Local test environment

For repeatable loopback HTTPS tests with disposable signing keys and isolated
builds, see [Local Windows update testing](local-update-test.md). Preparation
and signed-download probing are non-privileged; installing the baseline is an
explicit separate action that replaces the local application and service.

## Acceptance still required

The owner-authorized 2026-10-16 test passed the legacy compatibility handoff to
Iced 0.3.0. An initial native update failed because Restart Manager could not
close the GUI and worker. After adding `/UPGRADE`, the repeated 0.3.0-to-0.3.1
test passed: GUI exit and executable release completed before service removal,
Restart Manager found no applications to close, the local test
payload installed, and one original-user GUI reopened with its service
running. See `docs/migration-validation.md` for evidence and remaining limits.

Automated checks compile the real Inno script without running its payload, test
the legacy signer's output with the native verifier (including altered bytes and
wrong keys), and exercise UI operation guards and recovery using mock backends.
Updater unit tests also cover streamed size limits, unused staging cleanup,
installer exit codes and disabled/stale UI transitions. UI tests
check that launching Setup keeps the app open, excludes firmware operations and
releases the guard on cancellation.
They do not install services, change firmware, elevate or restart a real device.

On an owner-authorized disposable Windows VM, record logs and verify:

1. A released Tauri build downloads, verifies and launches a newly signed Inno
   payload through its actual updater. Test `/P /R /UPDATE /ARGS`, `/S /UPDATE`
   and native Iced update switches, including spaces and Unicode in paths.
2. Legacy removal succeeds, and missing/failing uninstallers stop Setup with an
   explicit error before file replacement. Verify only one uninstall registration remains and no delayed NSIS
   process removes the new files. Test cancellation and interrupted installation.
3. An installed Iced build updates to a newer signed version, requests UAC,
   accepts the installer handoff and exits, registers the demand-start service and reopens a single unelevated
   GUI. Verify slow closure and a locked executable are handled by Setup without
   reporting success or reopening the app before replacement succeeds. Cancel UAC;
   simulate offline, corrupt payload, invalid signature and service setup failure.
   Verify recovery when best-effort removal leaves an existing service, including
   retrying Setup after a service setup failure. Confirm the GUI starts the service
   when needed, without changing firmware.
4. Portable, demo and Linux variants make no update requests. A pending firmware
   operation or unsaved boot-order change prevents installation. No firmware
   change or reboot is necessary for any updater acceptance test.
