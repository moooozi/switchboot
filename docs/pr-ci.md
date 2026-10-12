# Pull request validation

`.github/workflows/pr-validation.yml` validates PRs targeting `main`, pushes to
`main`, merge-queue groups and manual runs. Windows and Ubuntu run independently;
one platform failing does not cancel the other's diagnostics.

The workflow validates the native iced Cargo workspace on both platforms.

Checks include frozen Node dependencies, SVG icon generation, Rust formatting,
Clippy with warnings treated as errors, locked Rust tests, all Node test files and
Python tooling tests. The native workspace also requires host package-container
tests. On Linux, native demo startup is checked under Xvfb with an in-memory backend.
No real firmware or restart operations are run.

Successful runs build unsigned packages and upload:

- Windows portable executable and NSIS installer.
- Linux DEB, RPM and a tarball containing the executable with its permissions.
- `build-info.json` recording the tested commit, platform and application layout.

Build artifacts are retained for 14 days. Validation logs are uploaded even on
failure and retained for 7 days. Downloads are under the workflow run's Artifacts
section; they are test builds, not signed releases.

PR jobs use read-only repository permissions, checkout without persisted Git
credentials and no release secrets. They use `pull_request`, not
`pull_request_target`. Packaging assembles files without installing packages,
starting services, signing artifacts, publishing releases or updating repositories.
Release workflows are unchanged.

To enforce this gate, configure the `main` branch ruleset to require
`Validate (ubuntu-22.04)` and `Validate (windows-latest)`. Workflow files create
status checks; repository rules must explicitly require them before merging.

Local artifact-collector checks:

```sh
python -B -m unittest discover -s scripts -p 'test_ci_artifacts.py'
```
