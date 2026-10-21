# Reusable local Windows update test

This harness builds an isolated copy of the current working tree, including
uncommitted implementation files, then creates a baseline installer and a newer
signed installer. It hosts `latest.json` and the payload on loopback HTTPS.
No VM, production signing key, public feed change, system certificate import,
or disabled TLS verification is needed.

**Installing the baseline replaces the machine's real Switchboot installation
and changes the real Windows service. This is not a side-by-side sandbox.**
It deliberately keeps the real NSIS registration key and service identity so the update
exercises the production lifecycle. Preparation and probing do not install
anything. No test requires firmware mutations or a Windows reboot.

## Prerequisites

Windows, Rust/Cargo, Git, OpenSSL, Python 3.11+,
stock NSIS 3, and generated app/shortcut icons. OpenSSL from Git for Windows is
also detected when it is absent from PATH. Cargo may download dependencies.

Keep an official installer available for restoration. Save/discard edits and
close the real GUI before installing the baseline. Do not run boot shortcuts,
firmware operations or other Switchboot clients during this test.

## Snapshot dependency

The build requires `packaging/nsis/toolkit/` inside the isolated source snapshot.
The snapshot helper enumerates superproject files without copying submodule
contents, so `prepare` cannot complete with the toolkit gitlink alone. Submodule
snapshot support is required before running preparation. The following procedure
describes the harness interface and the resulting test environment.

## Prepare (safe: no installation)

```sh
python scripts/local_update_test.py prepare
```

The baseline uses the root package's current version; the target defaults to its
next patch version. Override with `prepare --target-version X.Y.Z --port 18443`.
The target must be newer than the baseline. Compare these versions with your
installed version before installing: this harness does not prevent downgrading
an existing local acceptance build.

Everything is created under ignored `artifacts/local-update-test/`:

- `source/`: snapshot, with local-only feed/key/CA and a LOCAL UPDATE TEST GUI title.
- `baseline-setup.exe`, `target-setup.exe`: actual NSIS application installers.
- The baseline owns `update-test-obsolete.txt`; the target omits it to exercise
  registry-based obsolete-file cleanup.
- `site/`: **only** the manifest and signed target installer served over HTTPS.
- `private/`: disposable Minisign key and local TLS CA/server keys/certificates.
- `helper/`, `target/`: signing and download-verification helper and build cache.
- `config.json`: versions and local endpoint.

The private test key has an empty password and is not a production credential.
Do not publish this environment or its installers. The test CA is trusted only
by the copied updater code, not by Windows or your normal application builds.
Certificates expire after 30 days; rebuild the environment afterward.

Preparation ends with a bounded local HTTPS probe. The helper uses the copied
production manifest, HTTP, bounded-download and signature modules to download
and verify the signed installer. It never launches it. This verifies transport
and cryptography, not GUI integration, installed-mode policy or UAC.

To repeat that safe probe with the server stopped:

```sh
python scripts/local_update_test.py probe
```

Preparation refuses to overwrite an existing environment. To rebuild, first
restore an official build if a test build is installed, stop the server, then
explicitly remove `artifacts/local-update-test/`. Production source files and
versions are never modified. Failure leaves the environment available for
inspection; remove it before preparing again.

## Install baseline (changes this machine)

```sh
python scripts/local_update_test.py install-baseline --confirm-machine-change
```

This opens the normal interactive NSIS baseline installer. Choose the displayed Upgrade, Reinstall or Downgrade action for
an existing installation and approve UAC. The machine-wide installation directory
is fixed. No automatic restart or GUI launch is requested. Confirm installation
succeeded before proceeding. The command deliberately requires explicit consent.

## Exercise the actual in-app update

In a separate terminal, start the server:

```sh
python scripts/local_update_test.py serve --seconds 1800
```

It binds **127.0.0.1 only**, serves only `/latest.json` and `/setup.exe`, and stops
after the selected duration (maximum 24 hours) or Ctrl+C. The feed uses `localhost`;
there is no LAN exposure and no external hosting requirement. Do not run `probe`
while `serve` owns the same port.

1. Launch the installed baseline through its Start menu entry, not the snapshot
   executable (which would be portable).
2. Confirm the window says **LOCAL UPDATE TEST** and offers the target version.
3. Click Upgrade and approve UAC. Confirm download progress, GUI exit, replacement
   and exactly one reopened unelevated GUI.
4. Confirm the installed executable/registration version is the target version,
   `update-test-obsolete.txt` was removed, and the service is usable. User-created
   files must survive. Do not change firmware to check this.
5. Stop the HTTPS server. Check that ordinary boot-entry loading works.

The target also points to this local feed. Once updated, it should report current
while the server is running. When the server stops, update checks fail; it does
not fall back to the public feed.

For failures before handoff, check the UI error and server request log. Setup
reports errors after GUI exit. Native installer launches have no explicit `/LOG`.
Direct installer runs do not count as in-app acceptance.
Record baseline/target versions, observed behavior, UAC outcome, installed version
and any available logs. A successful safe probe alone is not end-to-end acceptance.

## Restore

Close the test GUI and use an official installer to replace the test build.
If its version is lower than the test version, treat that as an intentional local
rollback and verify the resulting registration and service. Do not delete or
stop the service manually unless diagnosing an explicitly authorized failure.
Confirm the official GUI has its standard title and uses the public feed before
deleting the environment. There is no system certificate to remove.

## Automated harness checks

```sh
python -B -m unittest discover -s scripts -p test_local_update_test.py -v
```

These tests do not run installers or touch services. Existing updater unit tests
cover signature tampering/wrong keys and bounded responses; the Windows handoff
fixtures cover GUI/worker timeouts without running Switchboot.
