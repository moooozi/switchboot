"""Test extracted legacy migration with a disposable HKCU key and fake uninstaller."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import uuid

from test_native_packaging import ISCC, ROOT


@unittest.skipUnless(os.name == "nt" and ISCC and shutil.which("rustc"),
                     "Windows, Inno Setup and Rust required")
class LegacyInstallerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="switchboot-legacy-fixture-")
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name)
        cls.key = "Software\\SwitchbootLegacyFixture\\" + uuid.uuid4().hex
        helper = cls.root / "helper.rs"
        helper.write_text(r'''
#[link(name="kernel32")]
unsafe extern "system" { fn GetCommandLineW() -> *const u16; }
fn main() {
    let exe = std::env::current_exe().unwrap();
    let args = unsafe {
        let pointer = GetCommandLineW();
        let mut length = 0;
        while *pointer.add(length) != 0 { length += 1; }
        String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length))
    };
    std::fs::write(exe.parent().unwrap().join("arguments.txt"), args).unwrap();
    if exe.file_name().unwrap() == "fail.exe" { std::process::exit(9); }
}
''')
        subprocess.run(["rustc", str(helper), "-o", str(cls.root / "uninstall.exe")],
                       check=True, capture_output=True, timeout=60)
        shutil.copyfile(cls.root / "uninstall.exe", cls.root / "fail.exe")
        (cls.root / "marker.txt").write_text("harmless fixture payload")
        fixture = cls.root / "fixture.iss"
        fixture.write_text(f'''
#define InstallationRegistryRoot HKCU
#define InstallationNativeKey "{cls.key}-native"
#define InstallationLegacyKey "{cls.key}"
[Setup]
AppName=Legacy migration fixture
AppVersion=1.0
PrivilegesRequired=lowest
DefaultDirName={{tmp}}\\legacy-unused
Uninstallable=no
CreateUninstallRegKey=no
CloseApplications=no
DisableWelcomePage=yes
DisableDirPage=no
DisableReadyPage=yes
DisableFinishedPage=yes
OutputDir=.
OutputBaseFilename=fixture
[Files]
Source: "marker.txt"; DestDir: "{{app}}"
[Code]
#include "{(ROOT / 'packaging/windows-installation.iss').as_posix()}"
#include "{(ROOT / 'packaging/windows-legacy.iss').as_posix()}"
function InitializeSetup: Boolean;
var Probe: String;
begin
  Probe := ExpandConstant('{{param:PROBE|}}');
  if Probe <> '' then begin
    SaveStringToFile(Probe, IntToStr(Ord(HasSwitch('/UPDATE'))) + ',' + IntToStr(UpgradePID), False);
    Result := False;
    Exit;
  end;
  ValidateUpgradeArguments;
  InitializeLegacyUpdate;
  Result := True;
end;
procedure InitializeWizard;
begin InitializeLegacyWizard; end;
function ShouldSkipPage(PageID: Integer): Boolean;
begin Result := LegacyPassiveUpgrade and (PageID = wpSelectDir); end;
procedure CurPageChanged(PageID: Integer);
begin LegacyPageChanged(PageID); end;
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin Result := MigrateLegacyInstallation; end;
''', encoding="utf-8-sig")
        result = subprocess.run([ISCC, "/Q", str(fixture)], capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)

    def setUp(self):
        import winreg
        self.winreg = winreg
        self.case = Path(tempfile.mkdtemp(dir=self.root))
        self.app = self.case / "old install é"
        self.app.mkdir()
        (self.app / "switchboot.exe").write_text("not an executable; never launched")
        self.uninstaller = self.app / "uninstall.exe"
        shutil.copyfile(self.root / "uninstall.exe", self.uninstaller)
        self.register(self.uninstaller)
        self.addCleanup(lambda: self.winreg.DeleteKey(self.winreg.HKEY_CURRENT_USER, self.key))

    def register(self, uninstaller):
        with self.winreg.CreateKey(self.winreg.HKEY_CURRENT_USER, self.key) as key:
            self.winreg.SetValueEx(key, "InstallLocation", 0, self.winreg.REG_SZ, str(self.app))
            self.winreg.SetValueEx(key, "UninstallString", 0, self.winreg.REG_SZ, f'"{uninstaller}"')

    def install(self, arguments):
        log = self.case / "setup.log"
        result = subprocess.run([str(self.root / "fixture.exe"), "/NORESTART", f"/DIR={self.app}",
                                 f"/LOG={log}", *arguments], capture_output=True, timeout=15)
        return result.returncode, log.read_text(encoding="utf-8-sig")

    def test_passive_legacy_switches_advance_and_preserve_nsis_argument_order(self):
        code, log = self.install(["/P", "/R", "/UPDATE", "/ARGS"])
        self.assertEqual(code, 0, log)
        self.assertEqual(log.count("queued Install button click"), 1)
        self.assertTrue((self.app / "marker.txt").exists())
        arguments = (self.app / "arguments.txt").read_text(encoding="utf-8")
        self.assertTrue(arguments.endswith('/S /UPDATE _?=' + str(self.app)), arguments)

    def test_native_silent_legacy_migration_uses_same_registered_uninstaller(self):
        code, log = self.install(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/UPDATE"])
        self.assertEqual(code, 0, log)
        self.assertIn("Previous NSIS installation removed", log)
        self.assertTrue((self.app / "marker.txt").exists())

    def test_failed_legacy_uninstaller_blocks_payload(self):
        shutil.copyfile(self.root / "fail.exe", self.app / "fail.exe")
        self.register(self.app / "fail.exe")
        code, log = self.install(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/UPDATE"])
        self.assertNotEqual(code, 0)
        self.assertIn("exit code 9", log)
        self.assertFalse((self.app / "marker.txt").exists())

    def test_args_marker_excludes_application_arguments_from_setup_parsing(self):
        probe = self.case / "probe.txt"
        self.install(["/VERYSILENT", "/SUPPRESSMSGBOXES", f"/PROBE={probe}",
                      "/ARGS", "/UPDATE", "/UPGRADEPID=1234"])
        self.assertEqual(probe.read_text(), "0,0")

    def test_incompatible_upgrade_switches_are_rejected(self):
        code, log = self.install(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/UPGRADE",
                                  "/UPDATE", "/UPGRADEPID=1234"])
        self.assertNotEqual(code, 0)
        self.assertIn("cannot be combined", log)
        self.assertFalse((self.app / "marker.txt").exists())


if __name__ == "__main__":
    unittest.main()
