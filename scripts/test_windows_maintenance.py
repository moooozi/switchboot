"""Exercise installed-app actions with HKCU fixtures, never Switchboot/services."""
import ctypes
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest
import uuid

from test_native_packaging import ISCC, ROOT


@unittest.skipUnless(os.name == "nt" and ISCC and shutil.which("rustc"),
                     "Windows, Inno Setup and Rust required")
class MaintenanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="switchboot-maintenance-")
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name)
        cls.app_id = "sb-maint-" + uuid.uuid4().hex[:16]
        cls.app_name = cls.app_id
        cls.key = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\" + cls.app_id + "_is1"
        source = cls.root / "helper.rs"
        source.write_text('fn main() { let exe = std::env::current_exe().unwrap(); if exe.file_name().unwrap() == "fail.exe" { std::process::exit(9); } std::fs::write(exe.parent().unwrap().join("launched.txt"), "launched").unwrap(); }')
        subprocess.run(["rustc", str(source), "-o", str(cls.root / "switchboot.exe")],
                       check=True, capture_output=True, timeout=60)
        shutil.copyfile(cls.root / "switchboot.exe", cls.root / "fail.exe")
        cls.compile_fixture("old", False)
        cls.compile_fixture("new", True)
        cls.user32 = ctypes.WinDLL("user32", use_last_error=True)
        cls.callback_type = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
        cls.user32.GetWindowTextLengthW.argtypes = [ctypes.c_void_p]
        cls.user32.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        cls.user32.PostMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
        cls.user32.EnumWindows.argtypes = [cls.callback_type, ctypes.c_void_p]
        cls.user32.EnumChildWindows.argtypes = [ctypes.c_void_p, cls.callback_type, ctypes.c_void_p]
        cls.user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
        from ctypes import wintypes
        cls.rect_type = wintypes.RECT
        cls.user32.GetWindowRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.RECT)]
        cls.user32.GetWindowLongW.argtypes = [ctypes.c_void_p, ctypes.c_int]

    @classmethod
    def compile_fixture(cls, label, maintenance):
        includes = ""
        configuration = ""
        if maintenance:
            configuration = f'#include "{(ROOT / "issconfig/issconfig.iss").as_posix()}"'
            includes = f'''
#define ISSCONFIG_CODE
#include "{(ROOT / 'issconfig/issconfig.iss').as_posix()}"
#define InstallationRegistryRoot HKCU
#define InstallationNativeKey "{cls.key}"
#define InstallationLegacyKey "{cls.key}-unused"
#include "{(ROOT / 'packaging/windows-installation.iss').as_posix()}"
#include "{(ROOT / 'packaging/windows-shutdown.iss').as_posix()}"

#include "{(ROOT / 'packaging/windows-maintenance.iss').as_posix()}"
function InitializeSetup: Boolean;
begin DetectMaintenanceInstallation; Result := True; end;
procedure InitializeWizard;
begin IssConfigInitialize; InitializeMaintenancePage; end;
procedure DeinitializeSetup;
begin IssConfigShutdown; end;
function ShouldSkipPage(PageID: Integer): Boolean;
begin Result := (MaintenanceKey <> '') and (PageID = wpSelectDir); end;
procedure CurPageChanged(PageID: Integer);
begin IssConfigPageChanged(PageID); MaintenancePageChanged(PageID); end;
function NextButtonClick(PageID: Integer): Boolean;
begin Result := MaintenanceCanContinue(PageID); end;
procedure CancelButtonClick(PageID: Integer; var Cancel, Confirm: Boolean);
begin if MaintenanceCanCloseWithoutConfirmation(PageID) then Confirm := False; end;
'''
        # This is deliberately not the Switchboot AppId or service. Only the
        # production maintenance/handoff include runs; all payloads are harmless.
        script = f'''
{configuration}
[Setup]
AppId={cls.app_id}
AppName={cls.app_name}
AppVersion=1.0
PrivilegesRequired=lowest
DefaultDirName={{tmp}}\\maintenance-unused
DisableWelcomePage=yes
DisableDirPage=no
DisableReadyPage=yes
DisableFinishedPage=yes
CloseApplications=no

OutputDir=.
OutputBaseFilename={label}
[Files]
Source: "switchboot.exe"; DestDir: "{{app}}"
Source: "{label}.txt"; DestDir: "{{app}}"
[Code]
{includes}
procedure CurStepChanged(Step: TSetupStep);
begin
  if Step = ssInstall then begin
'''
        if maintenance:
            script += '''    if FileExists(ExpandConstant('{app}\\old.txt')) then
      RaiseException('Old payload still exists before replacement');
'''
        script += '''  end;
end;
'''
        (cls.root / f"{label}.txt").write_text(label)
        path = cls.root / f"{label}.iss"
        path.write_text(script, encoding="utf-8-sig")
        result = subprocess.run([ISCC, "/Q", str(path)], capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)

    def setUp(self):
        import winreg
        self.winreg = winreg
        self.case = Path(tempfile.mkdtemp(dir=self.root))
        self.app = self.case / "installed app é"
        subprocess.run([str(self.root / "old.exe"), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART",
                        f"/DIR={self.app}"], check=True, capture_output=True, timeout=30)
        self.addCleanup(self.remove_fixture)
        self.process = None

    def remove_fixture(self):
        if self.process and self.process.poll() is None:
            for window in self.windows():
                if self.app_name in self.text(window):
                    self.user32.PostMessageW(window, 0x0010, 0, 0)
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                subprocess.run(["taskkill", "/PID", str(self.process.pid), "/T", "/F"],
                               capture_output=True, timeout=10)
                self.process.wait(timeout=5)
        uninstaller = self.app / "unins000.exe"
        if uninstaller.exists():
            subprocess.run([str(uninstaller), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
                           capture_output=True, timeout=30)
        try:
            self.winreg.DeleteKey(self.winreg.HKEY_CURRENT_USER, self.key)
        except FileNotFoundError:
            pass

    def text(self, window):
        value = ctypes.create_unicode_buffer(self.user32.GetWindowTextLengthW(window) + 1)
        self.user32.GetWindowTextW(window, value, len(value))
        return value.value

    def windows(self, parent=None):
        found = []
        callback = self.callback_type(lambda window, _: found.append(window) is None)
        if parent is None:
            self.user32.EnumWindows(callback, None)
        else:
            self.user32.EnumChildWindows(parent, callback, None)
        return found

    def start_maintenance(self):
        self.process = subprocess.Popen([str(self.root / "new.exe"), "/NORESTART",
                                         f"/LOG={self.case / 'maintenance.log'}"],
                                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            for window in self.windows():
                if self.app_name in self.text(window) and self.user32.IsWindowVisible(window):
                    controls = self.windows(window)
                    buttons = {self.text(control): control for control in controls
                               if self.user32.IsWindowVisible(control)}
                    if all(name in buttons for name in ["Uninstall", "Reinstall", "Launch"]):
                        self.assertIn("Switchboot is already installed", buttons)
                        self.assertNotIn("&Next >", buttons)
                        self.assert_footer_layout(window, buttons)
                        return buttons
            time.sleep(0.05)
        self.fail("Installed-app page did not appear")

    def assert_footer_layout(self, window, buttons):
        bounds = self.rect_type()
        self.user32.GetWindowRect(window, ctypes.byref(bounds))
        rectangles = []
        for name in ["Uninstall", "Reinstall", "Launch"]:
            rect = self.rect_type()
            self.user32.GetWindowRect(buttons[name], ctypes.byref(rect))
            rectangles.append(rect)
        self.assertEqual(len({rect.top for rect in rectangles}), 1)
        for first, second in zip(rectangles, rectangles[1:]):
            self.assertLess(first.right, second.left)
        self.assertGreater(rectangles[0].left, bounds.left + (bounds.right - bounds.left) // 3)
        self.assertLess(bounds.bottom - rectangles[-1].bottom, (bounds.bottom - bounds.top) // 5)
        self.assertEqual(self.user32.GetWindowLongW(buttons["Launch"], -16) & 0xF, 1)
        self.assertNotEqual(self.user32.GetWindowLongW(buttons["Uninstall"], -16) & 0xF, 1)

    def click(self, buttons, caption):
        self.user32.PostMessageW(buttons[caption], 0x00F5, 0, 0)
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            details = [(self.text(window), [self.text(child) for child in self.windows(window)])
                       for window in self.windows()
                       if self.app_name in self.text(window) or self.text(window) == "Error"]
            log = (self.case / "maintenance.log").read_text(encoding="utf-8-sig", errors="replace")
            self.fail(f"Fixture did not finish after {caption}: {details}\n" + "\n".join(log.splitlines()[-20:]))

    def registered(self):
        try:
            with self.winreg.OpenKey(self.winreg.HKEY_CURRENT_USER, self.key):
                return True
        except FileNotFoundError:
            return False

    def test_launch_preserves_installation_and_does_not_install(self):
        self.click(self.start_maintenance(), "Launch")
        deadline = time.monotonic() + 5
        while not (self.app / "launched.txt").exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertTrue((self.app / "launched.txt").exists())
        self.assertTrue((self.app / "old.txt").exists())
        self.assertFalse((self.app / "new.txt").exists())
        self.assertTrue(self.registered())

    def test_uninstall_removes_old_installation_without_installing(self):
        self.click(self.start_maintenance(), "Uninstall")
        self.assertFalse((self.app / "switchboot.exe").exists())
        self.assertFalse((self.app / "old.txt").exists())
        self.assertFalse((self.app / "new.txt").exists())
        self.assertFalse(self.registered())

    def test_reinstall_removes_old_payload_then_installs_at_registered_directory(self):
        self.click(self.start_maintenance(), "Reinstall")
        self.assertEqual(self.process.returncode, 0)
        self.assertFalse((self.app / "old.txt").exists())
        self.assertTrue((self.app / "new.txt").exists())
        self.assertTrue((self.app / "switchboot.exe").exists())
        self.assertTrue(self.registered())

    def test_missing_uninstaller_blocks_reinstallation(self):
        with self.winreg.OpenKey(self.winreg.HKEY_CURRENT_USER, self.key, 0, self.winreg.KEY_SET_VALUE) as key:
            self.winreg.SetValueEx(key, "UninstallString", 0, self.winreg.REG_SZ, str(self.app / "missing.exe"))
        buttons = self.start_maintenance()
        self.user32.PostMessageW(buttons["Reinstall"], 0x00F5, 0, 0)
        self.assert_action_error("uninstaller was not found")

    def test_failed_uninstaller_blocks_reinstallation(self):
        with self.winreg.OpenKey(self.winreg.HKEY_CURRENT_USER, self.key, 0, self.winreg.KEY_SET_VALUE) as key:
            self.winreg.SetValueEx(key, "UninstallString", 0, self.winreg.REG_SZ, str(self.root / "fail.exe"))
        buttons = self.start_maintenance()
        self.user32.PostMessageW(buttons["Reinstall"], 0x00F5, 0, 0)
        self.assert_action_error("exit code 9")

    def assert_action_error(self, fragment):
        deadline = time.monotonic() + 5
        error_window = None
        while time.monotonic() < deadline:
            for window in self.windows():
                if any(fragment in self.text(child) for child in self.windows(window)):
                    error_window = window
                    break
            if error_window:
                break
            time.sleep(0.05)
        self.assertIsNotNone(error_window, f"Expected error containing {fragment!r}")
        self.assertIsNone(self.process.poll())
        self.assertTrue((self.app / "old.txt").exists())
        self.assertFalse((self.app / "new.txt").exists())
        # Close only our fixture's error and maintenance windows.
        self.user32.PostMessageW(error_window, 0x0010, 0, 0)


if __name__ == "__main__":
    unittest.main()
