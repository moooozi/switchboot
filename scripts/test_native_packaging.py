"""Build real native package containers around a harmless fixture executable."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET

from bash_test_support import BASH, bash_env

ROOT = Path(__file__).resolve().parent.parent
NSIS = shutil.which("makensis")


class NativePackagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # CI must execute the package-container test for its host platform.
        # Local machines can still run the remaining suite without these tools.
        if os.environ.get("SWITCHBOOT_REQUIRE_PACKAGING_TESTS") == "1":
            tools = [NSIS] if os.name == "nt" else [
                BASH, shutil.which("dpkg-deb"), shutil.which("rpmbuild"),
                shutil.which("rpm"), shutil.which("true"),
            ]
            if not all(tools):
                raise RuntimeError("Required native packaging tools are unavailable")
            icon = "icon.ico" if os.name == "nt" else "128x128.png"
            if not (ROOT / "crates/switchboot_core/icons" / icon).is_file():
                raise RuntimeError("Generate application icons before native packaging tests")

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
        for size in [32, 48, 64, 128, 256, 512]:
            shutil.copyfile(ROOT / f"crates/switchboot_core/icons/{size}x{size}.png",
                            self.core / f"icons/{size}x{size}.png")
        shutil.copyfile(ROOT / "app-icon.svg", self.root / "app-icon.svg")
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
        executable = extracted / "usr/bin/switchboot"
        self.assertEqual(executable.read_bytes(), (self.root / "target/release/switchboot").read_bytes())
        self.assertEqual(executable.stat().st_mode & 0o777, 0o755)
        for size in [32, 48, 64, 128, 256, 512]:
            installed = extracted / f"usr/share/icons/hicolor/{size}x{size}/apps/switchboot.png"
            self.assertEqual(installed.read_bytes(),
                             (self.core / f"icons/{size}x{size}.png").read_bytes())
        self.assertEqual((extracted / "usr/share/icons/hicolor/scalable/apps/switchboot.svg").read_bytes(),
                         (ROOT / "app-icon.svg").read_bytes())
        desktop = (extracted / "usr/share/applications/dev.zidane.switchboot.desktop").read_text()
        self.assertIn("Exec=switchboot\n", desktop)
        policy = ET.parse(extracted / "usr/share/polkit-1/actions/com.switchboot.cli.policy")
        actions = policy.findall("action")
        self.assertEqual(len(actions), 5)
        for action in actions:
            annotations = {a.attrib["key"]: a.text for a in action.findall("annotate")}
            self.assertEqual(annotations["org.freedesktop.policykit.exec.path"], "/usr/bin/switchboot")
            self.assertTrue(annotations["org.freedesktop.policykit.exec.argv1"])
        metadata = subprocess.check_output(["dpkg-deb", "--field", deb, "Version", "Architecture"], text=True)
        self.assertIn("Version: 1.0.0", metadata)
        self.assertIn("Architecture: amd64", metadata)
        rpm = self.root / "target/release/bundle/rpm/switchboot-1.0.0-1.x86_64.rpm"
        version = subprocess.check_output(["rpm", "-qp", "--qf", "%{VERSION} %{ARCH}", rpm], text=True)
        self.assertEqual(version, "1.0.0 x86_64")
        files = subprocess.check_output(["rpm", "-qpl", rpm], text=True).splitlines()
        for name in ["/usr/bin/switchboot", "/usr/bin/switchboot-cli",
                     "/usr/share/applications/dev.zidane.switchboot.desktop",
                     "/usr/share/polkit-1/actions/com.switchboot.cli.policy",
                     "/usr/share/icons/hicolor/128x128/apps/switchboot.png"]:
            self.assertIn(name, files)
        for size in [32, 48, 64, 128, 256, 512]:
            self.assertIn(f"/usr/share/icons/hicolor/{size}x{size}/apps/switchboot.png", files)
        self.assertIn("/usr/share/icons/hicolor/scalable/apps/switchboot.svg", files)
        dump = subprocess.check_output(["rpm", "-qp", "--dump", rpm], text=True)
        cli_line = next(line for line in dump.splitlines() if line.startswith("/usr/bin/switchboot-cli "))
        self.assertEqual(cli_line.split()[-1], "switchboot")

    @unittest.skipUnless(BASH, "Bash required")
    def test_linux_packager_rejects_invalid_version_before_building(self):
        for version in ["", "1.2", "../1.2.3", "1.2.3;touch unexpected"]:
            with self.subTest(version=version):
                result = subprocess.run([BASH, "packaging/linux.sh", version], cwd=self.root,
                                        env=bash_env(), capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.root / "target/release/bundle").exists())


if __name__ == "__main__":
    unittest.main()
