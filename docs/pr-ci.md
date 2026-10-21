# Continuous integration

## Validation jobs

`.github/workflows/pr-validation.yml` defines independent Windows and Ubuntu
jobs for pull requests targeting `main`, pushes to `main`, merge groups and manual
runs. One platform failure does not cancel the other's diagnostics.

The jobs install tooling, generate icons and run Rust formatting, Clippy with
warnings denied, locked Rust tests and Python tooling/package checks.
Windows compiles the release executable once and packages the same executable in
both the installer and portable download. Packaging uses `--payload` and
`--plugin-artifact` to avoid compiling the app or DLL again.

Windows jobs download the production DLL from the `dll-<toolkit-commit>` release
in `moooozi/nsis-kit`, matching the pinned submodule commit. See the
[NSIS build guide](../packaging/nsis/README.md#build) for artifact verification.

Linux demo startup uses Xvfb with an in-memory backend and a 15-second observation
window. Package assembly does not install packages or run firmware operations.

## Packaging configuration constraints

The [Windows build guide](../packaging/nsis/README.md#build) defines the supported
builder and output. CI configuration has these constraints:

| Configuration | Constraint |
| --- | --- |
| `pr-validation.yml` | Windows packaging uses stock NSIS and `packaging/nsis/build.py` with `--payload`, `--plugin-artifact` and `--output artifacts/Switchboot_x64-setup.exe` |
| `scripts/test_native_packaging.py` | Linux package fixtures are independent of Windows packaging; NSIS lifecycle fixtures live in `packaging/nsis/test_installer.py` |
| `scripts/collect-ci-artifacts.py` and Windows signing step | The installer input is `artifacts/Switchboot_x64-setup.exe`; configure the NSIS output to that name or align those consumers |

These constraints affect executable workflows, not the local NSIS build commands.
Do not use a successful mock test or a checked-in workflow as evidence that a
release artifact was built or delivered. See the [validation guide](migration-validation.md)
for local commands and device acceptance.

## Artifacts and permissions

The artifact collector expects a Windows portable executable and installer, or
Linux DEB/RPM packages and a tarball preserving executable permissions.
`build-info.json` records commit, platform, application layout and signing status.
Artifacts are retained for 14 days; validation logs are retained for 7 days and
uploaded on failure. These artifacts are unsigned builds.

PR jobs use read-only repository permissions, checkout without persisted Git
credentials and no release secrets. They use `pull_request`, not
`pull_request_target`.

After both platform jobs pass on a push to `main`, a separate job attests the
installer, portable executable, DEB and RPM. Only this job receives OIDC and
attestation write permissions; it is skipped for PR, merge-group and manual runs.

Tag releases select a successful `main`-push validation run for the exact tagged
commit. If validation is still running, release preparation waits up to an hour.
Failed validation or missing/expired artifacts stop the release. Rerun the
original main-push validation if its artifacts have expired.

`linux-sign.yml` and `windows-sign.yml` download those builds and verify their
identity and provenance against the validation workflow, commit and `main` ref.
They sign the existing files without compiling the app or rebuilding packages.
The Windows job uses the small updater-signing tool; it runs on Ubuntu.
Detached signatures preserve the original build provenance. Embedded RPM signing
changes the package digest, so the signed RPM receives its own provenance with
the verified unsigned RPM digest and validation run recorded as its input.
The release workflow adds detached GPG signatures and opens a draft.

Configure the `main` ruleset to require `Validate (ubuntu-latest)` and
`Validate (windows-latest)` when enforcing the gate. Workflow definitions provide
status checks; repository rules control merge requirements.

Artifact-collector checks:

```console
python -B -m unittest discover -s scripts -p test_ci_artifacts.py
```
