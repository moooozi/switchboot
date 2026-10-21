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

The toolkit repository's `.github/workflows/build-dll.yml` publishes the checked
x86 installer DLL as an immutable `dll-<toolkit-commit>` release. Windows validation
and release builds download this DLL from `moooozi/nsis-kit`, using the exact
submodule commit rather than a moving latest release. There is no DLL compilation
in Switchboot's packaging jobs. See the
[NSIS build guide](../packaging/nsis/README.md#build) for artifact verification.

Linux demo startup uses Xvfb with an in-memory backend and a 15-second observation
window. Package assembly does not install packages or run firmware operations.

`.github/workflows/nsis-validation.yml` invokes the independent toolkit's Python
runner on Windows. The toolkit repository also has its own `.github/workflows/ci.yml`.
The [toolkit validation reference](../packaging/nsis/toolkit/README.md#validation)
defines its checks and fixture boundaries.

## Packaging configuration constraints

The [Windows build guide](../packaging/nsis/README.md#build) defines the supported
builder and output. CI configuration has these constraints:

| Configuration | Constraint |
| --- | --- |
| `pr-validation.yml`, `windows-build.yml` | Windows packaging uses `packaging/nsis/build.py --output artifacts/Switchboot_x64-setup.exe`, stock NSIS and the x86 Rust target for the UI plugin |
| `scripts/test_native_packaging.py` | Linux package fixtures are independent of Windows packaging; NSIS lifecycle fixtures live in `packaging/nsis/test_installer.py` |
| `nsis-validation.yml` | Checkout needs recursive submodule initialization to provide `packaging/nsis/toolkit/ci.py` |
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
uploaded on failure. These artifacts are unsigned test builds.

PR jobs use read-only repository permissions, checkout without persisted Git
credentials and no release secrets. They use `pull_request`, not
`pull_request_target`. Release signing/publication run in dedicated workflows.

Configure the `main` ruleset to require `Validate (ubuntu-latest)` and
`Validate (windows-latest)` when enforcing the gate. Workflow definitions provide
status checks; repository rules control merge requirements.

Artifact-collector checks:

```console
python -B -m unittest discover -s scripts -p test_ci_artifacts.py
```
