"""Safe lifecycle integration tests: compile a private HKCU/temp-directory fixture.

Never runs the production installer, real Switchboot, SCM commands, firmware or
an elevated process. The real NSIS source and service hooks drive a mock EXE.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest
import uuid
import winreg

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent

PAYLOAD = r'''
use std::{env, fs::OpenOptions, io::Write, path::PathBuf};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(a: *const u8, manual: i32, initial: i32, name: *const u16) -> *mut u8;
    fn WaitForSingleObject(handle: *mut u8, timeout: u32) -> u32;
    fn CloseHandle(handle: *mut u8) -> i32;
}
fn main() {
    let directory: PathBuf = env::current_exe().unwrap().parent().unwrap().into();
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--fixture-gui"] {
        let name: Vec<u16> = format!("Local\\Switchboot.Upgrade.{}", std::process::id()).encode_utf16().chain(Some(0)).collect();
        unsafe {
            let event = CreateEventW(std::ptr::null(), 1, 0, name.as_ptr());
            assert!(!event.is_null());
            std::fs::write(directory.join("ready"), b"ready").unwrap();
            let result = WaitForSingleObject(event, 20000);
            CloseHandle(event);
            assert_eq!(result, 0);
        }
        return;
    }
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "--cli");
    assert!(["/install_service", "/uninstall_service"].contains(&args[1].as_str()));
    let mut log = OpenOptions::new().create(true).append(true).open(directory.join("fixture.log")).unwrap();
    writeln!(log, "{}", args[1]).unwrap();
    if env::var("SWITCHBOOT_NSIS_FIXTURE_FAIL").as_deref() == Ok(args[1].as_str()) { std::process::exit(7); }
}
'''


class InstallerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="switchboot-nsis-fixture-")
        cls.addClassCleanup(cls.temp.cleanup)
        cls.base = Path(cls.temp.name)
        cls.key = rf"Software\Switchboot\NsisFixture\{uuid.uuid4().hex}"
        cls.addClassCleanup(lambda: cls.delete_tree(cls.key))
        cls.exe = cls.base / "fixture-setup.exe"
        payload = cls.base / "payload"
        payload.mkdir()
        (payload / "main.rs").write_text(PAYLOAD)
        subprocess.run(["rustc", "--edition=2024", "-O", str(payload / "main.rs"), "-o", str(payload / "switchboot.exe")], check=True, timeout=60)
        (payload / "fixture.ico").write_bytes(b"Harmless icon payload fixture")
        source = (HERE / "setup.nsi").read_text()

        def replace(old, new):
            nonlocal source
            if old not in source:
                raise AssertionError(f"Fixture transformation no longer matches: {old}")
            source = source.replace(old, new)

        replace("RequestExecutionLevel admin", "RequestExecutionLevel user")
        replace("HKLM", "HKCU")
        replace("SetShellVarContext all", "SetShellVarContext current")
        replace(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot", cls.key + r"\NSIS")
        replace(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\dev.zidane.switchboot_is1", cls.key + r"\Inno")
        replace(r"Global\Switchboot.NativeInstaller", "Local\\Switchboot.NsisFixture." + uuid.uuid4().hex)
        replace(r"$SMPROGRAMS\Switchboot.lnk", r"$INSTDIR\fixture.lnk")
        replace(r"..\..\crates\switchboot_core\icons\icon.ico", str(ROOT / "crates/switchboot_core/icons/icon.ico"))
        replace(r"..\nsis-poc\ui\target\i686-pc-windows-msvc\release", str(ROOT / "packaging/nsis-poc/ui/target/i686-pc-windows-msvc/release"))
        replace(r'File "..\..\target\release\switchboot.exe"', f'File "{payload / "switchboot.exe"}"')
        replace(r'File "..\..\crates\switchboot_core\resources\icons\ico\*.ico"', f'File "{payload / "fixture.ico"}"')
        # Exercise maintenance execution without opening UI or touching live registrations.
        replace("  Pop $RestartApp", '  Pop $RestartApp\n  ReadEnvStr $Action "SWITCHBOOT_NSIS_FIXTURE_ACTION"')
        # Reinstall normally relaunches; mock launch must not acquire desktop tokens.
        replace('  switchboot_nsis_ui::Launch "$INSTDIR"\n  Pop $Failure', '  StrCpy $Failure ""')
        if "RequestExecutionLevel admin" in source or "HKLM" in source or r"$SMPROGRAMS" in source:
            raise AssertionError("Unsafe fixture configuration")
        (cls.base / "setup.nsi").write_text(source)
        shutil.copyfile(HERE / "service-hooks.nsh", cls.base / "service-hooks.nsh")
        (cls.base / "owned-icons.nsh").write_text('Delete "$INSTDIR\\resources\\icons\\ico\\fixture.ico"\n')
        makensis = shutil.which("makensis") or r"C:\Program Files (x86)\NSIS\makensis.exe"
        subprocess.run([makensis, "-WX", "-V2", "-DVERSION=0.3.0", f"-DOUTPUT={cls.exe}", str(cls.base / "setup.nsi")], check=True, timeout=60)

    @staticmethod
    def delete_tree(path):
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path, 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as key:
                children = []
                try:
                    while True:
                        children.append(winreg.EnumKey(key, len(children)))
                except OSError:
                    pass
            for child in children:
                InstallerTests.delete_tree(path + "\\" + child)
            winreg.DeleteKeyEx(winreg.HKEY_CURRENT_USER, path, winreg.KEY_WOW64_64KEY)
        except FileNotFoundError:
            pass

    def setUp(self):
        self.delete_tree(self.key + r"\NSIS")
        self.directory = self.base / uuid.uuid4().hex
        self.directory.mkdir()

    def run_setup(self, *args, expected=0, **environment):
        result = subprocess.run([str(self.exe), "/S", *args, f"/D={self.directory}"], env={**os.environ, **environment}, timeout=45)
        self.assertEqual(result.returncode, expected)

    def installed_value(self, name):
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as key:
            return winreg.QueryValueEx(key, name)[0]

    def test_fresh_update_reinstall_uninstall_preserves_unowned_files(self):
        self.run_setup()
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
        self.assertEqual(self.installed_value("UninstallString"), f'"{self.directory / "uninstall.exe"}"')
        self.assertEqual(self.installed_value("SwitchbootInstaller"), "native-nsis")
        self.run_setup("/UPDATE")
        self.run_setup("/UPDATE", SWITCHBOOT_NSIS_FIXTURE_ACTION="reinstall")
        (self.directory / "user.txt").write_text("keep")
        result = subprocess.run([str(self.directory / "uninstall.exe"), "/S", f"_?={self.directory}"], timeout=30)
        self.assertEqual(result.returncode, 0)
        self.assertFalse((self.directory / "switchboot.exe").exists())
        self.assertTrue((self.directory / "user.txt").exists())
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")

    def test_failed_service_removal_retains_files_and_registration(self):
        self.run_setup()
        self.run_setup("/UPDATE", expected=2, SWITCHBOOT_NSIS_FIXTURE_FAIL="/uninstall_service")
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
        result = subprocess.run([str(self.directory / "uninstall.exe"), "/S", f"_?={self.directory}"], env={**os.environ, "SWITCHBOOT_NSIS_FIXTURE_FAIL": "/uninstall_service"}, timeout=30)
        self.assertEqual(result.returncode, 2)
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))

    def test_failed_service_install_reports_nonzero_and_keeps_repair_registration(self):
        self.run_setup(expected=2, SWITCHBOOT_NSIS_FIXTURE_FAIL="/install_service")
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))

    def test_native_handoff_signals_only_the_mock_gui_then_updates(self):
        self.run_setup()
        process = subprocess.Popen([str(self.directory / "switchboot.exe"), "--fixture-gui"])
        self.addCleanup(lambda: process.poll() is None and process.kill())
        deadline = time.monotonic() + 10
        while not (self.directory / "ready").exists():
            self.assertLess(time.monotonic(), deadline, "Mock GUI did not start")
            time.sleep(0.02)
        self.run_setup("/UPGRADE", f"/UPGRADEPID={process.pid}")
        self.assertEqual(process.wait(timeout=5), 0)
        self.assertTrue((self.directory / "switchboot.exe").is_file())

    def test_stale_uninstaller_cannot_remove_a_different_directory(self):
        self.run_setup()
        other = self.base / uuid.uuid4().hex
        other.mkdir()
        before = (self.directory / "fixture.log").read_text()
        result = subprocess.run([str(self.directory / "uninstall.exe"), "/S", f"_?={other}"], timeout=30)
        self.assertEqual(result.returncode, 2)
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertEqual((self.directory / "fixture.log").read_text(), before)
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))

    def test_foreign_pid_aborts_before_service_or_file_changes(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        self.run_setup("/UPGRADE", f"/UPGRADEPID={os.getpid()}", expected=2)
        self.assertEqual((self.directory / "fixture.log").read_text(), before)

    def test_silent_manual_overwrite_and_downgrade_are_rejected(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        self.run_setup(expected=2)
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0, winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.SetValueEx(key, "DisplayVersion", 0, winreg.REG_SZ, "9.0.0")
        self.run_setup("/UPDATE", expected=2)
        self.assertEqual((self.directory / "fixture.log").read_text(), before)

    def test_missing_uninstaller_stops_reinstall_before_file_replacement(self):
        self.run_setup()
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0, winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.SetValueEx(key, "UninstallString", 0, winreg.REG_SZ, f'"{self.directory / "missing.exe"}"')
        before = (self.directory / "fixture.log").read_text()
        self.run_setup("/UPDATE", expected=2, SWITCHBOOT_NSIS_FIXTURE_ACTION="reinstall")
        self.assertEqual((self.directory / "fixture.log").read_text(), before)
        self.assertTrue((self.directory / "switchboot.exe").is_file())


if __name__ == "__main__":
    unittest.main(verbosity=2)
