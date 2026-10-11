"""Check PR artifact collection using harmless build fixtures."""
import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "collect_ci_artifacts", Path(__file__).with_name("collect-ci-artifacts.py"))
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


class CiArtifactTests(unittest.TestCase):
    def fixture(self, *, native, windows):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        if native:
            (root / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
        release = root / ("target/release" if native else "src-tauri/target/release")
        release.mkdir(parents=True)
        binary = release / ("switchboot.exe" if windows else "switchboot")
        binary.write_bytes(b"harmless executable fixture")
        binary.chmod(0o755)
        if windows:
            installer = (root / "artifacts/Switchboot_x64-setup.exe" if native else
                         release / "bundle/nsis/fixture-setup.exe")
            installer.parent.mkdir(parents=True)
            installer.write_bytes(b"harmless installer fixture")
        else:
            for extension in ["deb", "rpm"]:
                folder = release / f"bundle/{extension}"
                folder.mkdir(parents=True)
                (folder / f"switchboot-1.0.0.{extension}").write_bytes(b"harmless package fixture")
        return root, release

    def test_windows_builds_include_portable_installer_and_identity(self):
        for native in [False, True]:
            with self.subTest(native=native):
                root, _ = self.fixture(native=native, windows=True)
                collector.collect(root, windows=True)
                output = root / "artifacts/ci"
                self.assertEqual((output / "Switchboot_x64-portable.exe").read_bytes(),
                                 b"harmless executable fixture")
                self.assertEqual((output / "Switchboot_x64-setup.exe").read_bytes(),
                                 b"harmless installer fixture")
                metadata = json.loads((output / "build-info.json").read_text())
                self.assertEqual(metadata["layout"], "iced" if native else "tauri")
                self.assertFalse(metadata["signed"])

    def test_linux_builds_include_packages_and_executable_tar(self):
        for native in [False, True]:
            with self.subTest(native=native):
                root, release = self.fixture(native=native, windows=False)
                collector.collect(root, windows=False)
                output = root / "artifacts/ci"
                for extension in ["deb", "rpm"]:
                    self.assertEqual((output / f"switchboot-1.0.0.{extension}").read_bytes(),
                                     b"harmless package fixture")
                with tarfile.open(output / "Switchboot_linux_x64.tar.gz") as archive:
                    self.assertEqual(archive.getnames(), ["switchboot"])
                    self.assertEqual(archive.extractfile("switchboot").read(),
                                     (release / "switchboot").read_bytes())
                    self.assertEqual(archive.getmember("switchboot").mode,
                                     (release / "switchboot").stat().st_mode & 0o777)

    def test_missing_empty_and_ambiguous_outputs_fail(self):
        root, release = self.fixture(native=True, windows=False)
        (release / "bundle/deb/duplicate.deb").write_bytes(b"duplicate")
        with self.assertRaisesRegex(RuntimeError, "Expected one"):
            collector.collect(root, windows=False)
        root, release = self.fixture(native=True, windows=True)
        (release / "switchboot.exe").write_bytes(b"")
        with self.assertRaisesRegex(RuntimeError, "Missing or empty"):
            collector.collect(root, windows=True)
        root, release = self.fixture(native=False, windows=True)
        (release / "switchboot.exe").unlink()
        with self.assertRaisesRegex(RuntimeError, "Missing or empty"):
            collector.collect(root, windows=True)
