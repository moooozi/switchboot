"""Build real native package containers around a harmless fixture executable."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

from bash_test_support import BASH, bash_env

ROOT = Path(__file__).resolve().parent.parent
NSIS = shutil.which("makensis")


class NativePackagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        shutil.copytree(ROOT / "packaging", self.root / "packaging",
                        ignore=shutil.ignore_patterns("target", "__pycache__"))
        self.core = self.root / "crates/switchboot_core"
        (self.core / "icons").mkdir(parents=True)
        (self.root / "target/release").mkdir(parents=True)
        (self.root / "artifacts").mkdir()

    @unittest.skipUnless(NSIS, "NSIS required")
    def test_nsis_builds_installer_with_service_and_shortcut_resources(self):
        icon = ROOT / "crates/switchboot_core/icons/icon.ico"
        if not icon.exists():
            self.skipTest("Run node packaging/generate-app-icons.mjs first")
        shutil.copyfile(icon, self.core / "icons/icon.ico")
        resources = self.core / "resources/icons/ico"
        resources.mkdir(parents=True)
        shutil.copyfile(icon, resources / "generic.ico")
        (self.root / "target/release/switchboot.exe").write_bytes(b"harmless fixture executable")
        version_flag = "/DVERSION=1.0.0" if os.name == "nt" else "-DVERSION=1.0.0"
        result = subprocess.run([NSIS, version_flag, "packaging/windows.nsi"],
                                cwd=self.root, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertGreater((self.root / "artifacts/Switchboot_x64-setup.exe").stat().st_size, 0)

    @unittest.skipUnless(BASH and shutil.which("dpkg-deb") and shutil.which("rpmbuild"),
                         "Linux package tools required")
    def test_linux_packages_ship_cli_symlink_and_authorization_policy(self):
        png = ROOT / "crates/switchboot_core/icons/128x128.png"
        if not png.exists():
            self.skipTest("Run node packaging/generate-app-icons.mjs first")
        shutil.copyfile(png, self.core / "icons/128x128.png")
        icon_dir = self.core / "icons-linux/hicolor/128x128/apps"
        icon_dir.mkdir(parents=True)
        shutil.copyfile(png, icon_dir / "swboot-generic.png")
        shutil.copytree(ROOT / "crates/switchboot_core/platform/linux/polkit-1",
                        self.core / "platform/linux/polkit-1")
        shutil.copyfile(shutil.which("true"), self.root / "target/release/switchboot")
        result = subprocess.run([BASH, "packaging/linux.sh", "1.0.0"], cwd=self.root,
                                env=bash_env(), capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        deb = self.root / "target/release/bundle/deb/switchboot_1.0.0_amd64.deb"
        extracted = self.root / "extracted"
        subprocess.run(["dpkg-deb", "--extract", deb, extracted], check=True)
        self.assertTrue((extracted / "usr/bin/switchboot-cli").is_symlink())
        self.assertEqual((extracted / "usr/bin/switchboot-cli").readlink(), Path("switchboot"))
        self.assertTrue((extracted / "usr/share/polkit-1/actions/com.switchboot.cli.policy").exists())
        rpm = self.root / "target/release/bundle/rpm/switchboot-1.0.0-1.x86_64.rpm"
        version = subprocess.check_output(["rpm", "-qp", "--qf", "%{VERSION} %{ARCH}", rpm], text=True)
        self.assertEqual(version, "1.0.0 x86_64")


if __name__ == "__main__":
    unittest.main()
