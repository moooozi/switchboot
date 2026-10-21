"""Safe tests for the local updater harness; no installer or service calls."""

import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "local_update_test", Path(__file__).with_name("local_update_test.py")
)
local = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(local)


class LocalUpdateTests(unittest.TestCase):
    def test_versions_require_forward_stable_release(self):
        self.assertEqual(local.versions("0.3.0", None), "0.3.1")
        self.assertEqual(local.versions("0.3.0", "1.0.0"), "1.0.0")
        for target in ["0.3.0", "0.2.9", "0.3.1-beta.1", "01.0.0"]:
            with self.assertRaises(ValueError):
                local.versions("0.3.0", target)

    def test_patch_only_changes_snapshot_and_keeps_tls_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            module = source / "crates/switchboot_updater/src"
            module.mkdir(parents=True)
            (module / "lib.rs").write_text(
                'const FEED: &str = "production";\nconst PUBLIC_KEY: &str = "production-key";\n'
            )
            (module / "download.rs").write_text("builder().https_only(true).build()")
            certificate = root / "ca.pem"
            certificate.write_text("fixture certificate")
            local.patch_snapshot(
                source, "https://localhost:18443", "fixture-key", certificate
            )
            text = (module / "download.rs").read_text()
            self.assertIn(".https_only(true)", text)
            self.assertIn(".add_root_certificate", text)
            self.assertNotIn("danger_accept", text)
            self.assertEqual(
                (module / "local-test-ca.pem").read_text(), "fixture certificate"
            )
            self.assertIn(
                '"https://localhost:18443/latest.json"', (module / "lib.rs").read_text()
            )

    def test_patch_fails_when_anchor_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.rs"
            path.write_text("same same")
            with self.assertRaises(ValueError):
                local.replace_once(path, "same", "new")
            self.assertEqual(path.read_text(), "same same")

    def test_machine_install_requires_explicit_confirmation(self):
        import argparse

        with patch.object(local, "run") as run:
            with self.assertRaises(ValueError):
                local.install(argparse.Namespace(confirm_machine_change=False))
            run.assert_not_called()

    def test_baseline_only_file_is_owned_and_target_omits_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hooks = root / "packaging/nsis/switchboot-hooks.nsh"
            hooks.parent.mkdir(parents=True)
            original = "!macro InstallFiles\n!macroend\n"
            hooks.write_text(original)
            icons = root / "crates/switchboot_core/resources/icons/ico"
            icons.mkdir(parents=True)
            (icons / "fixture.ico").write_bytes(b"fixture")
            observed = []

            def inspect_build(args, **_kwargs):
                if str(args[0]) == local.sys.executable:
                    observed.append(
                        (
                            (hooks.parent / "owned-icons.nsh").read_text(),
                            hooks.read_text(),
                            args,
                        )
                    )

            with patch.object(local, "run", side_effect=inspect_build):
                local.build_installer(root, "0.3.2", "baseline", root / "target")
                self.assertEqual(hooks.read_text(), original)
                local.build_installer(root, "0.3.3", "target", root / "target")
            baseline, target = observed
            self.assertIn("update-test-obsolete.txt", baseline[0])
            self.assertIn(
                f'File "{hooks.parent / "update-test-obsolete.txt"}"', baseline[1]
            )
            self.assertNotIn("update-test-obsolete.txt", target[0])
            self.assertEqual(target[1], original)
            self.assertIn("--output", target[2])
            self.assertTrue(
                Path(target[2][1])
                .as_posix()
                .endswith("packaging/nsis/toolkit/build.py")
            )

    def test_version_bump_is_scoped_to_root_package(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "Cargo.toml").write_text('[package]\nversion = "0.3.0"\n')
            (source / "Cargo.lock").write_text(
                'name = "other"\nversion = "0.3.0"\nname = "switchboot"\nversion = "0.3.0"\n'
            )
            local.set_version(source, "0.3.0", "0.3.1")
            self.assertIn(
                'name = "other"\nversion = "0.3.0"', (source / "Cargo.lock").read_text()
            )
            self.assertIn(
                'name = "switchboot"\nversion = "0.3.1"',
                (source / "Cargo.lock").read_text(),
            )


if __name__ == "__main__":
    unittest.main()
