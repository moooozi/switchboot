"""Safe lifecycle integration tests: compile a private HKCU/temp-directory fixture.

Never runs the production installer, real Switchboot, SCM commands, firmware or
an elevated process. The real NSIS source and service hooks drive a mock EXE.
"""
import os
import re
import ctypes as C
from ctypes import wintypes as W
import hashlib

from pathlib import Path
import shutil
import sys
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
    std::fs::write(directory.join("fixture-service-started"), args.join(" ")).unwrap();
    if let Ok(ms) = env::var("NATIVE_INSTALLER_FIXTURE_DELAY_MS") { std::thread::sleep(std::time::Duration::from_millis(ms.parse().unwrap())); }
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "--cli");
    assert!(["/install_service", "/uninstall_service"].contains(&args[1].as_str()));
    let mut log = OpenOptions::new().create(true).append(true).open(directory.join("fixture.log")).unwrap();
    writeln!(log, "{}", args[1]).unwrap();
    if env::var("NATIVE_INSTALLER_FIXTURE_FAIL").as_deref() == Ok(args[1].as_str()) { std::process::exit(7); }

}
'''


class FixtureProcess:
    """Hold a stable handle only after validating this exact fixture image."""
    kernel = C.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
    kernel.OpenProcess.restype = W.HANDLE
    kernel.QueryFullProcessImageNameW.argtypes = [W.HANDLE, W.DWORD, W.LPWSTR, C.POINTER(W.DWORD)]
    kernel.WaitForSingleObject.argtypes = [W.HANDLE, W.DWORD]
    kernel.TerminateProcess.argtypes = [W.HANDLE, W.UINT]
    kernel.CloseHandle.argtypes = [W.HANDLE]

    @classmethod
    def open(cls, pid, expected_path, digest):
        handle = cls.kernel.OpenProcess(0x101001, False, pid)
        if not handle:
            return None  # Already exited before the trace was observed.
        if cls.kernel.WaitForSingleObject(handle, 0) == 0:
            cls.kernel.CloseHandle(handle)
            return None
        path = C.create_unicode_buffer(32768)
        length = W.DWORD(len(path))
        try:
            if not cls.kernel.QueryFullProcessImageNameW(handle, 0, path, C.byref(length)):
                raise AssertionError("Cannot identify fixture process")
            if path.value.casefold() != expected_path.casefold():
                raise AssertionError("PID no longer identifies the traced fixture")
            with Path(path.value).open("rb") as image:
                if hashlib.file_digest(image, "sha256").digest() != digest:
                    raise AssertionError("Process image is not this private mock uninstaller")
        except FileNotFoundError:
            ended = cls.kernel.WaitForSingleObject(handle, 1000) == 0
            cls.kernel.CloseHandle(handle)
            if ended:
                return None
            raise
        except BaseException:
            cls.kernel.CloseHandle(handle)
            raise
        process = cls()
        process.handle = handle
        return process

    def exited(self):
        return self.kernel.WaitForSingleObject(self.handle, 0) == 0

    def cleanup(self):
        if not self.exited():
            # This handle is pinned to a verified mock image, never a live app.
            self.kernel.TerminateProcess(self.handle, 2)
            self.kernel.WaitForSingleObject(self.handle, 5000)
        self.kernel.CloseHandle(self.handle)


class InstallerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, str(ROOT / "packaging/nsis/toolkit"))
        from desktop_helpers import DesktopLease
        lease = DesktopLease()
        cls.addClassCleanup(lease.close)
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
        def read_script(path):
            source = path.read_text()
            def include(match):
                relative = match.group(1)
                if relative in ["owned-icons.nsh", "service-hooks.nsh"]:
                    return f'!include "{relative}"'
                return read_script(path.parent / Path(relative.replace("\\", "/")))
            source = re.sub(r'!include "\$\{__FILEDIR__\}\\([^"]+)"', include, source)
            return source.replace("${__FILEDIR__}", str(path.parent))
        source = read_script(HERE / "setup.nsi")
        source = source.replace('Function Fail\n', f'''Function Fail
  FileOpen $8 "{cls.base / 'failure.txt'}" w
  FileWriteUTF16LE $8 "$Failure"
  FileClose $8
''')

        def replace(old, new):
            nonlocal source
            if old not in source:
                raise AssertionError(f"Fixture transformation no longer matches: {old}")
            source = source.replace(old, new)

        if "RequestExecutionLevel user" not in source or "UninstPage uninstConfirm" not in source:
            raise AssertionError("Setup must start unelevated and confirm interactive uninstall")
        # Separate feature/target: the fixture's broker uses 'open', NEVER 'runas'.
        target = ROOT / "artifacts/nsis-fixture-ui"
        subprocess.run(["cargo", "build", "--locked", "--release", "--manifest-path", str(ROOT / "packaging/nsis/toolkit/ui/Cargo.toml"), "--target", "i686-pc-windows-msvc", "--features", "fixture-engine", "--target-dir", str(target)], check=True, timeout=180)
        trace = f'''  System::Call 'kernel32::GetCurrentProcessId() i .r7'
  IfSilent trace_silent trace_normal
trace_silent:
  StrCpy $6 "silent"
  Goto trace_write
trace_normal:
  StrCpy $6 "normal"
trace_write:
  FileOpen $8 "{cls.base / 'uninstall-trace.txt'}" a
  FileSeek $8 0 END
  FileWriteUTF16LE $8 "$7|$6|$EXEPATH|$CMDLINE$\\r$\\n"
  FileClose $8
  ClearErrors
'''
        replace("Function un.onInit\n", "Function un.onInit\n" + trace)

        replace("HKLM", "HKCU")
        replace("SetShellVarContext all", "SetShellVarContext current")
        # Hard stop before ANY fixture mutation outside this temporary tree,
        # even if a malformed /D= is ignored by NSIS's command-line parser.
        prefix = str(cls.base) + "\\"
        guard = f'  StrCpy $0 "$INSTDIR" {len(prefix)}\n  StrCmp $0 "{prefix}" +3\n    SetErrorLevel 2\n    Quit\n'
        replace('Section "${APP_NAME}"\n', 'Section "${APP_NAME}"\n' + guard)
        replace('selection_done:\n', 'selection_done:\n' + guard)
        replace('Section "Uninstall"\n', 'Section "Uninstall"\n' + guard + '  FileOpen $8 "$INSTDIR\\fixture-uninstall-ran" w\n  FileClose $8\n')
        replace(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot", cls.key + r"\NSIS")
        replace('Function ${PREFIX}RemoveNativeInstallation\n', 'Function ${PREFIX}RemoveNativeInstallation\n' + guard)
        # Fixtures accept only their private temporary subtree instead of Program Files.
        replace('  StrCmp "$INSTDIR" "${APP_DIRECTORY}" location_ok',
                f'  StrCpy $0 "$INSTDIR" {len(prefix)}\n  StrCmp $0 "{prefix}" location_ok')
        replace(r"$SMPROGRAMS\${APP_NAME}.lnk", r"$INSTDIR\fixture.lnk")
        replace(f'!define SWITCHBOOT_ICONS "{HERE}\\..\\..\\crates\\switchboot_core\\resources\\icons\\ico"',
                f'!define SWITCHBOOT_ICONS "{payload}"')
        # Exercise maintenance execution without opening UI or touching live registrations.
        replace("  Call ReadOptions", '  Call ReadOptions\n  ReadEnvStr $9 "NATIVE_INSTALLER_FIXTURE_ACTION"\n  ${If} $9 != ""\n    StrCpy $Action $9\n  ${EndIf}')
        # Explicit launch requests are traced without acquiring desktop tokens.
        replace('  native_nsis_ui::Launch /NOUNLOAD "$INSTDIR"\n  Pop $Failure', '  FileOpen $8 "$INSTDIR\\fixture-launch" w\n  FileClose $8\n  StrCpy $Failure ""')
        if "RequestExecutionLevel admin" in source or "HKLM" in source or r"$SMPROGRAMS" in source:
            raise AssertionError("Unsafe fixture configuration")
        (cls.base / "setup.nsi").write_text(source)
        shutil.copyfile(HERE / "service-hooks.nsh", cls.base / "service-hooks.nsh")
        (cls.base / "owned-icons.nsh").write_text('!macro AppOwnedFiles CALLBACK\n  !insertmacro ${CALLBACK} "resources\\icons\\ico\\fixture.ico"\n!macroend\n')
        makensis = shutil.which("makensis") or r"C:\Program Files (x86)\NSIS\makensis.exe"
        subprocess.run([makensis, "-INPUTCHARSET", "UTF8", "-WX", "-V2", "-DVERSION=0.3.0", f"-DOUTPUT={cls.exe}", f"-DPAYLOAD_EXE={payload / 'switchboot.exe'}", f"-DPLUGIN_DIRECTORY={target / 'i686-pc-windows-msvc/release'}", str(cls.base / "setup.nsi")], check=True, timeout=60)

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
        (self.base / "uninstall-trace.txt").unlink(missing_ok=True)
        self.directory = self.base / ("Unicode 预览 " + uuid.uuid4().hex)
        self.directory.mkdir()

    def run_setup(self, *args, expected=0, **environment):
        failure = self.base / "failure.txt"
        failure.unlink(missing_ok=True)
        command = subprocess.list2cmdline([str(self.exe), "/S", *args]) + f" /D={self.directory}"
        result = subprocess.run(command, env={**os.environ, **environment}, timeout=45)
        self.assertEqual(result.returncode, expected,
                         failure.read_text(encoding="utf-16-le") if failure.exists() else "")

    @staticmethod
    def reap_fixture(process):
        if process.poll() is None:
            process.kill()
        process.wait(timeout=10)

    def preview(self, **environment):
        sys.path.insert(0, str(ROOT / "packaging/nsis/toolkit"))
        import desktop_helpers as desktop
        desktop.u.GetWindowThreadProcessId.argtypes = [desktop.W.HWND, desktop.C.POINTER(desktop.W.DWORD)]
        def own_window(hwnd, pid):
            actual = desktop.W.DWORD()
            desktop.u.GetWindowThreadProcessId(hwnd, desktop.C.byref(actual))
            return actual.value == pid
        command = subprocess.list2cmdline([str(self.exe)]) + f" /D={self.directory}"
        process = subprocess.Popen(command, env={**os.environ, "NATIVE_INSTALLER_THEME": "light", **environment})
        self.addCleanup(self.reap_fixture, process)
        window = desktop.wait_for(lambda: next((w for w in desktop.windows(desktop.UI_CLASS) if own_window(w, process.pid)), None), "unelevated selection window")
        def stop_mock_worker():
            pid = desktop.u.GetPropW(window, "NativeInstaller.NsisWorkerPID")
            if pid:
                with self.exe.open("rb") as image:
                    digest = hashlib.file_digest(image, "sha256").digest()
                owned = FixtureProcess.open(pid, str(self.exe), digest)
                if owned:
                    owned.cleanup()
        self.addCleanup(stop_mock_worker)
        return desktop, process, window

    def assert_primary_at_footer(self, desktop, window):
        button = desktop.u.GetDlgItem(window, 1)
        rect = W.RECT()
        desktop.u.GetWindowRect(button, C.byref(rect))
        origin = W.POINT(0, 0)
        desktop.u.MapWindowPoints(window, None, C.byref(origin), 1)
        dpi = desktop.u.GetDpiForWindow(window)
        self.assertEqual(rect.left - origin.x, desktop.scale(548, dpi))
        self.assertEqual(rect.right - origin.x, desktop.scale(548, dpi) + desktop.scale(80, dpi))
        self.assertEqual(rect.top - origin.y, desktop.scale(272, dpi))

    def check_custom_install(self, theme):
        desktop, process, window = self.preview(NATIVE_INSTALLER_FIXTURE_DELAY_MS="1200", NATIVE_INSTALLER_THEME=theme)
        self.assertFalse((self.directory / "switchboot.exe").exists())
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")
        desktop.click(window, 1)
        desktop.wait_for(lambda: desktop.u.GetPropW(window, "NativeInstaller.NsisOperation") == 1, "same-window progress")
        self.assertFalse(desktop.u.IsWindowVisible(desktop.u.GetDlgItem(window, 101)))
        self.assertFalse(desktop.u.IsWindowVisible(desktop.u.GetDlgItem(window, 1)))
        desktop.wait_for(lambda: "service" in desktop.caption(desktop.u.GetDlgItem(window, 106)), "actual worker progress report")
        worker_pid = desktop.u.GetPropW(window, "NativeInstaller.NsisWorkerPID")
        self.assertTrue(worker_pid)
        for dialog in desktop.windows("#32770"):
            pid = desktop.W.DWORD()
            desktop.u.GetWindowThreadProcessId(dialog, desktop.C.byref(pid))
            self.assertNotIn(pid.value, [process.pid, worker_pid], "Classic NSIS installer UI must remain hidden")
        desktop.screenshot(window, ROOT / f"artifacts/nsis-install-progress-{theme}.png")
        # Closing cannot abandon a worker part-way through service/file changes.
        desktop.u.PostMessageW(window, 0x10, 0, 0)
        self.assertIsNone(process.poll())
        desktop.wait_for(lambda: desktop.u.GetPropW(window, "NativeInstaller.NsisOperation") == 2, "same-window completion")
        self.assertIn(window, desktop.windows(desktop.UI_CLASS))
        self.assertEqual(desktop.caption(desktop.u.GetDlgItem(window, 1)), "Close")
        self.assert_primary_at_footer(desktop, window)
        self.assertIn("have been installed", desktop.caption(desktop.u.GetDlgItem(window, 106)))
        desktop.screenshot(window, ROOT / f"artifacts/nsis-install-complete-{theme}.png")
        desktop.click(window, 1)
        self.assertEqual(process.wait(timeout=10), 0)

    def test_custom_window_survives_install_progress_and_completion(self):
        self.check_custom_install("light")

    def test_worker_hands_foreground_back_once(self):
        desktop, process, window = self.preview(NATIVE_INSTALLER_FIXTURE_DELAY_MS="1200")
        desktop.u.SetForegroundWindow.argtypes = [W.HWND]
        desktop.u.GetForegroundWindow.restype = W.HWND
        desktop.u.SetForegroundWindow(window)
        if desktop.u.GetForegroundWindow() != window:
            self.skipTest("Windows declined initial fixture activation; cannot test foreground handoff on this desktop")
        desktop.click(window, 1)
        desktop.wait_for(lambda: desktop.u.GetPropW(window, "NativeInstaller.NsisFocusResult"), "worker foreground handoff")
        self.assertEqual(desktop.u.GetPropW(window, "NativeInstaller.NsisFocusGrant"), 1)
        self.assertEqual(desktop.u.GetPropW(window, "NativeInstaller.NsisFocusResult"), 1)
        self.assertEqual(desktop.u.GetForegroundWindow(), window)
        desktop.wait_for(lambda: desktop.u.GetPropW(window, "NativeInstaller.NsisOperation") == 2, "completion after foreground handoff")
        self.assertEqual(desktop.u.GetPropW(window, "NativeInstaller.NsisFocusHandoffs"), 1)
        self.assertEqual(desktop.u.GetPropW(window, "NativeInstaller.NsisFocusFrontendResult"), 1)
        desktop.click(window, 1)
        self.assertEqual(process.wait(timeout=10), 0)

    def test_custom_window_uses_dark_theme_through_completion(self):
        self.check_custom_install("dark")

    def test_reinstall_starts_without_an_extra_confirmation(self):
        self.run_setup()
        desktop, process, window = self.preview(NATIVE_INSTALLER_FIXTURE_DELAY_MS="500")
        desktop.click(window, 105)
        desktop.wait_for(lambda: desktop.u.GetPropW(window, "NativeInstaller.NsisOperation") == 2, "reinstall without a confirmation dialog")
        self.assertFalse((self.directory / "fixture-uninstall-ran").exists(), "Ownership-aware reinstall must preserve the existing installation")
        self.assertIn("have been installed", desktop.caption(desktop.u.GetDlgItem(window, 106)))
        desktop.click(window, 1)
        self.assertEqual(process.wait(timeout=10), 0)

    def test_canceling_permission_keeps_selection_and_makes_no_changes(self):
        desktop, process, window = self.preview(NATIVE_INSTALLER_FIXTURE_UAC="cancel")
        desktop.click(window, 1)
        desktop.wait_for(lambda: "permission was canceled" in desktop.caption(desktop.u.GetDlgItem(window, 106)), "permission cancellation")
        self.assertEqual(desktop.caption(desktop.u.GetDlgItem(window, 101)), str(self.directory))
        self.assertFalse((self.directory / "switchboot.exe").exists())
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")
        desktop.click(window, 2)
        self.assertEqual(process.wait(timeout=10), 1)

    def test_worker_errors_stay_in_the_custom_window(self):
        desktop, process, window = self.preview(NATIVE_INSTALLER_FIXTURE_DELAY_MS="700", NATIVE_INSTALLER_FIXTURE_FAIL="/install_service")
        desktop.click(window, 1)
        desktop.wait_for(lambda: desktop.u.GetPropW(window, "NativeInstaller.NsisOperation") == 2, "custom error result")
        self.assertIn("service install failed", desktop.caption(desktop.u.GetDlgItem(window, 106)))
        self.assert_primary_at_footer(desktop, window)
        desktop.screenshot(window, ROOT / "artifacts/nsis-install-error-light.png")
        desktop.click(window, 1)
        self.assertEqual(process.wait(timeout=10), 2)

    def installed_value(self, name):
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as key:
            return winreg.QueryValueEx(key, name)[0]

    def remove_ownership_protocol(self):
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0,
                           winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.DeleteValue(key, "FileOwnershipProtocol")

    def test_native_installation_without_protocol_uses_old_uninstaller(self):
        self.run_setup()
        for protocol in [None, 1, 2, 3, 99]:
            with self.subTest(protocol=protocol):
                if protocol is None:
                    self.remove_ownership_protocol()
                else:
                    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0,
                                       winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
                        winreg.SetValueEx(key, "FileOwnershipProtocol", 0, winreg.REG_DWORD, protocol)
                marker = self.directory / "fixture-uninstall-ran"
                marker.unlink(missing_ok=True)
                self.run_setup("/UPDATE")
                self.assertTrue(marker.is_file())
                self.assertEqual(self.installed_value("FileOwnershipProtocol"), 4)

    def test_supported_update_does_not_run_the_full_uninstaller(self):
        self.run_setup()
        self.run_setup("/UPDATE")
        self.assertFalse((self.directory / "fixture-uninstall-ran").exists())
        self.assertEqual((self.directory / "fixture.log").read_text().splitlines(),
                         ["/install_service", "/uninstall_service", "/install_service"])

    def write_ownership_value(self, name, value, kind=winreg.REG_MULTI_SZ):
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0,
                           winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.SetValueEx(key, name, 0, kind, value)

    def test_registry_manifest_contains_relative_files_and_separate_keys(self):
        self.run_setup()
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0,
                           winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as key:
            files, kind = winreg.QueryValueEx(key, "OwnedFiles")
            self.assertEqual(kind, winreg.REG_MULTI_SZ)
            self.assertEqual(files, ["switchboot.exe", "uninstall.exe", r"resources\icons\ico\fixture.ico"])
            keys, kind = winreg.QueryValueEx(key, "OwnedRegistryKeys")
            self.assertEqual(kind, winreg.REG_MULTI_SZ)
            self.assertEqual(keys, ["HKCU:64\\" + self.key + r"\NSIS"])

    def test_invalid_owned_lists_are_rejected_before_any_installation_changes(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        executable = self.directory / "switchboot.exe"
        executable_bytes = executable.read_bytes()
        files = ["switchboot.exe", "uninstall.exe"]
        paths = [r"..\..\outside.txt", r"resource\..\..\..\outside.txt",
                 r"resources\..\outside.txt", r".\outside.txt", r"C:\outside.txt",
                 r"C:outside.txt", r"\rooted.txt", r"\\server\share\outside.txt",
                 r"\\?\C:\outside.txt", r"resources\*.ico", r"resources\file:stream",
                 "resources/../outside.txt", "", "x" * 901, "file. ", "file.",
                 "file\r\n[RegistryKeys]", "bad\tfile", r"resource\\file.txt"]
        outside = self.base / "outside-owned-list"
        outside.mkdir()
        sentinel = outside / "keep.txt"
        sentinel.write_text("preserve")
        junction = self.directory / "linked-directory"
        result = subprocess.run(["cmd", "/c", "mklink", "/J", str(junction), str(outside)],
                                capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, "Unable to create the private junction fixture")
        self.addCleanup(junction.rmdir)
        paths.append(r"linked-directory\keep.txt")
        for path in paths:
            with self.subTest(path=path):
                self.write_ownership_value("OwnedFiles", [*files, path])
                self.run_setup("/UPDATE", expected=2)
                self.assertIn("unsafe path" if path else "Registry string list",
                              (self.base / "failure.txt").read_text(encoding="utf-16-le"))
                self.assertEqual(executable.read_bytes(), executable_bytes)
                self.assertEqual((self.directory / "fixture.log").read_text(), before)
                self.assertFalse((self.directory / "fixture-uninstall-ran").exists())
                self.assertEqual(self.installed_value("FileOwnershipProtocol"), 4)
                self.assertEqual(sentinel.read_text(), "preserve")
        self.write_ownership_value("OwnedFiles", files)
        for registry_paths in [["switchboot.exe"], ["HKCU:64\\Unrelated"], []]:
            with self.subTest(registry_paths=registry_paths):
                self.write_ownership_value("OwnedRegistryKeys", registry_paths)
                self.run_setup("/UPDATE", expected=2)
                self.assertIn("registry ownership" if registry_paths else "Registry string list",
                              (self.base / "failure.txt").read_text(encoding="utf-16-le"))
                self.assertEqual((self.directory / "fixture.log").read_text(), before)
                self.assertEqual(sentinel.read_text(), "preserve")
        self.write_ownership_value("OwnedRegistryKeys", ["HKCU:64\\" + self.key + r"\NSIS"])
        # Valid relative entries may be missing already, or contain Unicode and
        # spaces. Registry data exceeds the NSIS string buffer in this fixture.
        obsolete = self.directory / "old payload \u00e9\u4e2d\u6587.dat"
        obsolete.write_text("old owned payload")
        user_file = self.directory / "user-data.txt"
        user_file.write_text("preserve")
        self.write_ownership_value("OwnedFiles", [*files, *[f"obsolete-{i:04}.dat" for i in range(1000)], obsolete.name])
        self.run_setup("/UPDATE")
        self.assertFalse(obsolete.exists())
        self.assertEqual(user_file.read_text(), "preserve")
        self.assertEqual(sentinel.read_text(), "preserve")
        self.assertFalse((self.directory / "fixture-uninstall-ran").exists())

    def test_advertised_registry_manifest_must_be_complete_and_typed(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        for value, kind in [("switchboot.exe", winreg.REG_SZ), ([], winreg.REG_MULTI_SZ),
                            (["switchboot.exe"], winreg.REG_MULTI_SZ),
                            (["unrelated.txt", "another.txt"], winreg.REG_MULTI_SZ)]:
            with self.subTest(value=value, kind=kind):
                self.write_ownership_value("OwnedFiles", value, kind)
                self.run_setup("/UPDATE", expected=2)
                self.assertEqual((self.directory / "fixture.log").read_text(), before)
                self.assertTrue((self.directory / "switchboot.exe").is_file())
                self.assertFalse((self.directory / "fixture-uninstall-ran").exists())
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0,
                           winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.DeleteValue(key, "OwnedFiles")
        self.run_setup("/UPDATE", expected=2)
        self.assertEqual((self.directory / "fixture.log").read_text(), before)

    def test_supported_upgrade_reads_registry_without_running_old_uninstaller(self):
        self.run_setup()
        # A harmless mock app exits zero but implements no installer operations.
        # A supported upgrade must not need to execute it at all.
        shutil.copyfile(self.directory / "switchboot.exe", self.directory / "uninstall.exe")
        obsolete = self.directory / "old-owned.txt"
        obsolete.write_text("obsolete")
        self.write_ownership_value("OwnedFiles", ["switchboot.exe", "uninstall.exe", obsolete.name])
        self.run_setup("/UPDATE")
        self.assertFalse(obsolete.exists())
        self.assertEqual((self.directory / "fixture.log").read_text().splitlines(),
                         ["/install_service", "/uninstall_service", "/install_service"])

    def test_windows_inherited_registry_permissions_do_not_block_lifecycle(self):
        # Reproduce the machine key's compatibility ACE on our private HKCU parent.
        class Trustee(C.Structure):
            _fields_ = [("multiple", C.c_void_p), ("operation", W.DWORD),
                        ("form", W.DWORD), ("kind", W.DWORD), ("name", C.c_void_p)]
        class Access(C.Structure):
            _fields_ = [("permissions", W.DWORD), ("mode", W.DWORD),
                        ("inheritance", W.DWORD), ("trustee", Trustee)]
        advapi = C.WinDLL("advapi32", use_last_error=True)
        kernel = C.WinDLL("kernel32", use_last_error=True)
        advapi.GetSecurityInfo.argtypes = [W.HANDLE, C.c_int, W.DWORD, C.c_void_p,
            C.c_void_p, C.POINTER(C.c_void_p), C.c_void_p, C.POINTER(C.c_void_p)]
        advapi.SetSecurityInfo.argtypes = [W.HANDLE, C.c_int, W.DWORD,
            C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p]
        advapi.ConvertStringSidToSidW.argtypes = [W.LPCWSTR, C.POINTER(C.c_void_p)]
        advapi.SetEntriesInAclW.argtypes = [W.ULONG, C.POINTER(Access), C.c_void_p, C.POINTER(C.c_void_p)]
        kernel.LocalFree.argtypes = [C.c_void_p]
        with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, self.key, 0,
                winreg.KEY_READ | winreg.KEY_WOW64_64KEY | 0x40000) as key:
            acl, descriptor, sid, updated = (C.c_void_p() for _ in range(4))
            try:
                self.assertEqual(advapi.GetSecurityInfo(int(key), 4, 4, None, None,
                    C.byref(acl), None, C.byref(descriptor)), 0)
                self.assertTrue(advapi.ConvertStringSidToSidW("S-1-5-13", C.byref(sid)))
                access = Access(0x3001f, 1, 2, Trustee(None, 0, 0, 2, sid.value))
                self.assertEqual(advapi.SetEntriesInAclW(1, C.byref(access), acl, C.byref(updated)), 0)
                self.assertEqual(advapi.SetSecurityInfo(int(key), 4, 4, None, None, updated, None), 0)
            finally:
                for pointer in [updated, sid, descriptor]:
                    if pointer.value:
                        kernel.LocalFree(pointer)
        self.run_setup()
        self.run_setup("/UPDATE")
        self.run_setup("/UPDATE", NATIVE_INSTALLER_FIXTURE_ACTION="uninstall")
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")

    def test_no_transaction_state_and_unowned_icons_are_preserved(self):
        self.run_setup()
        self.assertEqual(list(self.base.glob(".switchboot-stage-*")), [])
        self.assertFalse((self.directory / ".switchboot-transaction").exists())
        self.assertFalse((self.directory / ".switchboot-files.json").exists())
        icon = self.directory / "resources/icons/ico/user.ico"
        icon.write_bytes(b"keep user icon")
        self.run_setup("/UPDATE")
        self.assertFalse((self.directory / ".switchboot-transaction").exists())
        self.assertEqual(icon.read_bytes(), b"keep user icon")
        self.run_setup("/UPDATE", NATIVE_INSTALLER_FIXTURE_ACTION="uninstall")
        self.assertEqual(icon.read_bytes(), b"keep user icon")

    def test_old_transaction_directory_is_not_used_or_deleted(self):
        # Existing candidate leftovers are not trusted or interpreted by new Setup.
        leftover = self.directory / ".switchboot-transaction"
        leftover.mkdir()
        sentinel = leftover / "journal.json"
        sentinel.write_bytes(b"not installer authority")
        self.run_setup()
        self.run_setup("/UPDATE")
        self.run_setup("/UPDATE", NATIVE_INSTALLER_FIXTURE_ACTION="uninstall")
        self.assertEqual(sentinel.read_bytes(), b"not installer authority")

    def test_fresh_update_reinstall_uninstall_preserves_unowned_files(self):
        self.run_setup()
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
        self.assertEqual(self.installed_value("UninstallString"), f'"{self.directory / "uninstall.exe"}" /S')
        self.assertEqual(self.installed_value("SwitchbootInstaller"), "native-nsis")
        self.run_setup("/UPDATE")
        self.run_setup("/UPDATE", NATIVE_INSTALLER_FIXTURE_ACTION="reinstall")
        (self.directory / "user.txt").write_text("keep")
        result = subprocess.run(subprocess.list2cmdline([str(self.directory / "uninstall.exe"), "/S"]) + f" _?={self.directory}", timeout=30)
        self.assertEqual(result.returncode, 0)
        self.assertFalse((self.directory / "switchboot.exe").exists())
        self.assertTrue((self.directory / "user.txt").exists())
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")

    def check_brokered_uninstall(self, silent):
        self.run_setup()
        uninstaller = self.directory / "uninstall.exe"
        with uninstaller.open("rb") as image:
            digest = hashlib.file_digest(image, "sha256").digest()
        command = subprocess.list2cmdline([str(uninstaller), *(["/S"] if silent else [])])
        process = subprocess.Popen(command, env={**os.environ, "NATIVE_INSTALLER_FIXTURE_BROKER": "1", "NATIVE_INSTALLER_FIXTURE_DELAY_MS": "1000"})
        self.addCleanup(self.reap_fixture, process)
        sys.path.insert(0, str(ROOT / "packaging/nsis/toolkit"))
        import desktop_helpers as desktop
        desktop.u.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
        tracked = {}
        seen = set()
        visible = False
        confirmed = False
        deadline = time.monotonic() + 15
        journal = self.base / "uninstall-trace.txt"
        while time.monotonic() < deadline:
            rows = journal.read_text(encoding="utf-16-le").splitlines() if journal.exists() else []
            for row in rows:
                fields = row.split("|", 3)
                if len(fields) != 4 or not fields[0].isdigit():
                    continue
                pid = int(fields[0])
                if pid not in seen:
                    seen.add(pid)
                    owned = FixtureProcess.open(pid, fields[2], digest)
                    if owned:
                        tracked[pid] = owned
                        self.addCleanup(owned.cleanup)
            for window in desktop.windows("#32770"):
                pid = W.DWORD()
                desktop.u.GetWindowThreadProcessId(window, C.byref(pid))
                if pid.value in tracked:
                    visible = True
                    self.assertFalse(silent, "Explicit /S must not show a window")
                    if not confirmed and desktop.caption(desktop.u.GetDlgItem(window, 1)).replace("&", "") == "Uninstall":
                        self.assertTrue((self.directory / "switchboot.exe").is_file())
                        self.assertFalse((self.directory / "fixture-uninstall-ran").exists())
                        confirmed = True
                        desktop.click(window, 1)
            if len(seen) >= 2 and not uninstaller.exists() and all(item.exited() for item in tracked.values()):
                break
            time.sleep(0.03)
        self.assertGreaterEqual(len(seen), 2, rows)
        self.assertTrue(all(row.split("|")[1] == ("silent" if silent else "normal") for row in rows), rows)
        self.assertEqual(visible, not silent)
        self.assertEqual(confirmed, not silent)
        self.assertFalse(uninstaller.exists(), rows)
        self.assertTrue(all(item.exited() for item in tracked.values()), "Every broker/worker must exit automatically")
        self.assertFalse((self.directory / "switchboot.exe").exists())
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")
        self.run_setup()  # No orphan uninstaller may block the next fresh install.

    def test_brokered_uninstaller_preserves_interactive_mode_and_deletes_itself(self):
        self.check_brokered_uninstall(False)

    def test_brokered_silent_uninstall_deletes_itself_and_exits(self):
        self.check_brokered_uninstall(True)

    def test_interactive_uninstall_confirmation_can_be_canceled(self):
        self.run_setup()
        sys.path.insert(0, str(ROOT / "packaging/nsis/toolkit"))
        import desktop_helpers as desktop
        desktop.u.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
        command = subprocess.list2cmdline([str(self.directory / "uninstall.exe")]) + f" _?={self.directory}"
        process = subprocess.Popen(command)
        self.addCleanup(self.reap_fixture, process)
        def confirmation():
            for window in desktop.windows("#32770"):
                pid = W.DWORD()
                desktop.u.GetWindowThreadProcessId(window, C.byref(pid))
                if pid.value == process.pid and desktop.caption(desktop.u.GetDlgItem(window, 1)).replace("&", "") == "Uninstall":
                    return window
        window = desktop.wait_for(confirmation, "uninstall confirmation")
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertFalse((self.directory / "fixture-uninstall-ran").exists())
        desktop.click(window, 2)
        self.assertEqual(process.wait(timeout=10), 1)
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertTrue((self.directory / "uninstall.exe").is_file())
        self.assertFalse((self.directory / "fixture-uninstall-ran").exists())
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))

    def test_windows_uninstall_registration_skips_second_confirmation(self):
        self.run_setup()
        command = self.installed_value("UninstallString")
        self.assertEqual(command, self.installed_value("QuietUninstallString"))
        self.assertTrue(command.endswith(' /S'))
        result = subprocess.run(command, timeout=15)
        self.assertEqual(result.returncode, 0)
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline and ((self.directory / "switchboot.exe").exists() or (self.directory / "uninstall.exe").exists()):
            time.sleep(0.03)
        self.assertFalse((self.directory / "switchboot.exe").exists())
        self.assertFalse((self.directory / "uninstall.exe").exists())


    def test_extraction_failure_is_reported_and_can_retry(self):
        obstacle = self.directory / "resources"
        obstacle.write_text("unowned file")
        self.run_setup(expected=2)
        self.assertEqual(obstacle.read_text(), "unowned file")
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")
        obstacle.unlink()
        self.run_setup()
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))

    def test_failed_service_removal_retains_files_and_registration(self):
        self.run_setup()
        self.run_setup("/UPDATE", expected=2, NATIVE_INSTALLER_FIXTURE_FAIL="/uninstall_service")
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
        result = subprocess.run(subprocess.list2cmdline([str(self.directory / "uninstall.exe"), "/S"]) + f" _?={self.directory}", env={**os.environ, "NATIVE_INSTALLER_FIXTURE_FAIL": "/uninstall_service"}, timeout=30)
        self.assertEqual(result.returncode, 2)
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))

    def test_failed_install_after_removal_can_retry_as_a_fresh_install(self):
        self.run_setup()
        self.remove_ownership_protocol()
        # Preserve an unrelated file that blocks the new payload after the old
        # uninstaller has removed its known files and registration.
        resources = self.directory / "resources"
        resources.rename(self.directory / "saved-user-copy")
        resources.write_text("unrelated file")
        self.run_setup("/UPDATE", expected=2)
        self.assertTrue((self.directory / "fixture-uninstall-ran").is_file())
        self.assertEqual(resources.read_text(), "unrelated file")
        with self.assertRaises(FileNotFoundError):
            self.installed_value("InstallLocation")
        resources.unlink()
        self.run_setup()
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
        self.assertTrue((self.directory / "uninstall.exe").is_file())

    def test_locked_application_files_report_failure_and_allow_retry(self):
        self.run_setup()
        application = self.directory / "switchboot.exe"
        application_bytes = application.read_bytes()
        kernel = C.WinDLL("kernel32", use_last_error=True)
        kernel.CreateFileW.argtypes = [W.LPCWSTR, W.DWORD, W.DWORD, C.c_void_p,
                                     W.DWORD, W.DWORD, W.HANDLE]
        kernel.CreateFileW.restype = W.HANDLE
        kernel.CloseHandle.argtypes = [W.HANDLE]
        for name in ["switchboot.exe", "uninstall.exe"]:
            with self.subTest(file=name):
                path = self.directory / name
                before = path.read_bytes()
                handle = kernel.CreateFileW(str(path), 0x80000000, 1, None, 3, 0, None)
                self.assertNotEqual(handle, C.c_void_p(-1).value)
                try:
                    self.run_setup("/UPDATE", expected=2)
                    self.assertEqual(path.read_bytes(), before)
                    self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
                finally:
                    kernel.CloseHandle(handle)
                # Removal is not transactional: a locked uninstaller can fail
                # after the application was deleted. Restore the mock payload
                # before retrying its service-removal command.
                if not application.exists():
                    application.write_bytes(application_bytes)
                self.run_setup("/UPDATE")

    def test_production_destination_is_read_only_without_browse(self):
        desktop, process, window = self.preview()
        edit = desktop.u.GetDlgItem(window, 101)
        desktop.u.GetWindowLongW.argtypes = [W.HWND, C.c_int]
        desktop.u.GetWindowLongW.restype = W.LONG
        desktop.u.IsWindowEnabled.argtypes = [W.HWND]
        desktop.u.IsWindowEnabled.restype = W.BOOL
        style = desktop.u.GetWindowLongW(edit, -16)
        self.assertTrue(style & 0x800)  # ES_READONLY
        self.assertFalse(style & 0x10000)  # No WS_TABSTOP
        self.assertFalse(desktop.u.IsWindowEnabled(edit))
        self.assertFalse(desktop.u.GetDlgItem(window, 100))
        self.assertEqual(desktop.caption(edit), str(self.directory))
        scope = desktop.u.GetDlgItem(window, 107)
        space = desktop.u.GetDlgItem(window, 108)
        self.assertEqual(desktop.caption(scope), "Installs for all users.")
        self.assertEqual(desktop.caption(desktop.u.GetDlgItem(window, 102)), "Installation folder")
        self.assertRegex(desktop.caption(space), r"^At least [1-9][0-9]* (KB|MB) of disk space is required\.$")
        path_rect, scope_rect, footer_rect = W.RECT(), W.RECT(), W.RECT()
        desktop.u.GetWindowRect(edit, C.byref(path_rect))
        desktop.u.GetWindowRect(scope, C.byref(scope_rect))
        desktop.u.GetWindowRect(space, C.byref(footer_rect))
        self.assertGreater(scope_rect.top, path_rect.bottom)
        self.assertGreater(footer_rect.top, scope_rect.bottom)
        desktop.screenshot(window, ROOT / "artifacts/nsis-install-selection.png")
        desktop.click(window, 2)
        self.assertEqual(process.wait(timeout=10), 1)

    def test_failed_service_install_retains_repair_metadata_and_can_retry(self):
        self.run_setup(expected=2, NATIVE_INSTALLER_FIXTURE_FAIL="/install_service")
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertTrue((self.directory / "uninstall.exe").is_file())
        self.assertFalse((self.directory / ".switchboot-transaction").exists())
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
        for name in ["FileOwnershipProtocol", "OwnedFiles", "OwnedRegistryKeys"]:
            with self.subTest(unpublished=name), self.assertRaises(FileNotFoundError):
                self.installed_value(name)
        self.run_setup("/UPDATE")
        self.assertEqual(self.installed_value("FileOwnershipProtocol"), 4)
        self.assertEqual(self.installed_value("DisplayVersion"), "0.3.0")

    def test_native_handoff_signals_only_the_mock_gui_then_updates(self):
        self.run_setup()
        process = subprocess.Popen([str(self.directory / "switchboot.exe"), "--fixture-gui"])
        self.addCleanup(lambda: process.poll() is None and process.kill())
        deadline = time.monotonic() + 10
        while not (self.directory / "ready").exists():
            self.assertLess(time.monotonic(), deadline, "Mock GUI did not start")
            time.sleep(0.02)
        self.run_setup("/UPGRADE", f"/UPGRADEPID=+000{process.pid}")
        self.assertEqual(process.wait(timeout=5), 0)
        self.assertTrue((self.directory / "switchboot.exe").is_file())

    def test_stale_uninstaller_cannot_remove_a_different_directory(self):
        self.run_setup()
        other = self.base / uuid.uuid4().hex
        other.mkdir()
        before = (self.directory / "fixture.log").read_text()
        result = subprocess.run(subprocess.list2cmdline([str(self.directory / "uninstall.exe"), "/S"]) + f" _?={other}", timeout=30)
        self.assertEqual(result.returncode, 2)
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertEqual((self.directory / "fixture.log").read_text(), before)
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))

    def test_foreign_pid_aborts_before_service_or_file_changes(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        self.run_setup("/UPGRADE", f"/UPGRADEPID={os.getpid()}", expected=2)
        self.assertEqual((self.directory / "fixture.log").read_text(), before)

    def test_released_tauri_installation_migrates_through_its_nsis_uninstaller(self):
        # This private legacy fixture has no native ownership marker. Its old
        # uninstaller only removes the mock executable and its private HKCU key.
        legacy_exe = self.directory / "switchboot.exe"
        legacy_exe.write_bytes(b"legacy fixture payload")
        old_uninstaller = self.directory / "old-uninstaller.exe"
        source = self.directory / "old-uninstaller.nsi"
        source.write_text(f'''
Unicode true
RequestExecutionLevel user
SilentInstall silent
OutFile "{old_uninstaller}"
Section
  SetRegView 64
  Delete "{legacy_exe}"
  DeleteRegKey HKCU "{self.key}\\NSIS"
SectionEnd
''', encoding="utf-8")
        compiler = shutil.which("makensis") or r"C:\Program Files (x86)\NSIS\makensis.exe"
        subprocess.run([compiler, "-INPUTCHARSET", "UTF8", "-WX", "-V2", str(source)],
                       check=True, timeout=30)
        with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0,
                               winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            for name, value in [("InstallLocation", str(self.directory)),
                                ("UninstallString", f'"{old_uninstaller}"'),
                                ("DisplayVersion", "0.2.6")]:
                winreg.SetValueEx(key, name, 0, winreg.REG_SZ, value)
        self.run_setup("/UPDATE", "/P", "/ARGS", "ignored application argument")
        self.assertEqual(self.installed_value("SwitchbootInstaller"), "native-nsis")
        self.assertEqual(self.installed_value("DisplayVersion"), "0.3.0")
        self.assertNotEqual(legacy_exe.read_bytes(), b"legacy fixture payload")

    def test_adapter_argument_boundaries_and_invalid_updates(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        for args in [("/UPGRADE",), ("/UPGRADEPID=9",), ("/UPGRADEPID=",),
                     ("/UPDATE", "/UPGRADE", "/UPGRADEPID=9"),
                     ("/UPGRADE", "/UPGRADEPID=invalid"),
                     ("/UPGRADE", "/UPGRADEPID=4294967296")]:
            with self.subTest(args=args):
                self.run_setup(*args, expected=2)
                self.assertEqual((self.directory / "fixture.log").read_text(), before)
        # Arguments belonging to the application never become worker options.
        self.run_setup("/UPDATE", "/ARGS", "/WORKER", "/ACTION=uninstall",
                       "/UPGRADE", "/UPGRADEPID=invalid", "/R")
        self.assertTrue((self.directory / "switchboot.exe").is_file())
        self.assertFalse((self.directory / "fixture-launch").exists())

    def test_reinstall_launch_requires_an_explicit_request(self):
        self.run_setup()
        self.run_setup("/UPDATE", NATIVE_INSTALLER_FIXTURE_ACTION="reinstall")
        self.assertFalse((self.directory / "fixture-launch").exists())
        self.run_setup("/UPDATE", "/R", NATIVE_INSTALLER_FIXTURE_ACTION="reinstall")
        self.assertTrue((self.directory / "fixture-launch").is_file())

    def test_maintenance_wording_matches_versions_and_requires_closing_app(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        for version, label in [("0.2.9", "&Upgrade"), ("0.3.0", "&Reinstall"), ("9.0.0", "&Downgrade")]:
            with self.subTest(version=version):
                with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0,
                                   winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
                    winreg.SetValueEx(key, "DisplayVersion", 0, winreg.REG_SZ, version)
                desktop, process, window = self.preview(NATIVE_INSTALLER_THEME="light")
                self.assertEqual(desktop.caption(desktop.u.GetDlgItem(window, 105)), label)
                description = desktop.caption(desktop.u.GetDlgItem(window, 109))
                self.assertIn(f"Installed version: {version}", description)
                self.assertIn("Setup version: 0.3.0", description)
                self.assertIn("Close all running instances of Switchboot", description)
                self.assertEqual((self.directory / "fixture.log").read_text(), before)
                desktop.screenshot(window, ROOT / f"artifacts/nsis-maintenance-{label[1:].lower()}-wording.png")
                desktop.u.PostMessageW(window, 0x10, 0, 0)
                self.assertEqual(process.wait(timeout=10), 1)

    def test_interactive_reinstall_allows_downgrade(self):
        self.run_setup()
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0, winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.SetValueEx(key, "DisplayVersion", 0, winreg.REG_SZ, "9.0.0")
        before = (self.directory / "fixture.log").read_text()
        desktop, process, window = self.preview()
        self.assertEqual(self.installed_value("DisplayVersion"), "9.0.0")
        self.assertEqual((self.directory / "fixture.log").read_text(), before)
        self.assertEqual(desktop.caption(desktop.u.GetDlgItem(window, 105)), "&Downgrade")
        desktop.click(window, 105)
        desktop.wait_for(lambda: desktop.u.GetPropW(window, "NativeInstaller.NsisOperation") == 2, "manual downgrade completion")
        self.assertIn("have been installed", desktop.caption(desktop.u.GetDlgItem(window, 106)))
        self.assertFalse((self.directory / "fixture-uninstall-ran").exists())
        self.assertEqual(self.installed_value("DisplayVersion"), "0.3.0")
        desktop.click(window, 1)
        self.assertEqual(process.wait(timeout=10), 0)

    def test_silent_manual_overwrite_and_automatic_downgrade_are_rejected(self):
        self.run_setup()
        before = (self.directory / "fixture.log").read_text()
        self.run_setup(expected=2)
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0, winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.SetValueEx(key, "DisplayVersion", 0, winreg.REG_SZ, "9.0.0")
        self.run_setup("/UPDATE", expected=2)
        self.run_setup("/UPGRADE", f"/UPGRADEPID={os.getpid()}", expected=2)
        self.assertEqual(self.installed_value("DisplayVersion"), "9.0.0")
        self.assertEqual((self.directory / "fixture.log").read_text(), before)

    def test_native_reinstall_refuses_a_missing_old_uninstaller(self):
        self.run_setup()
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, self.key + r"\NSIS", 0, winreg.KEY_WRITE | winreg.KEY_WOW64_64KEY) as key:
            winreg.SetValueEx(key, "UninstallString", 0, winreg.REG_SZ, f'"{self.directory / "missing.exe"}"')
        (self.directory / "uninstall.exe").unlink()
        before = (self.directory / "fixture.log").read_text()
        self.run_setup("/UPDATE", expected=2, NATIVE_INSTALLER_FIXTURE_ACTION="reinstall")
        self.assertFalse((self.directory / "uninstall.exe").exists())
        self.assertEqual((self.directory / "fixture.log").read_text(), before)
        self.assertEqual(self.installed_value("InstallLocation"), str(self.directory))
        self.assertTrue((self.directory / "switchboot.exe").is_file())


if __name__ == "__main__":
    unittest.main(verbosity=2)
