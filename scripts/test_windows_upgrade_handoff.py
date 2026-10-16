"""Exercise the real handoff script with disposable processes, never Switchboot."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from test_native_packaging import ISCC, ROOT


HELPER = r'''
use std::{ffi::c_void, io::{self, Write}, time::Duration};
#[link(name="kernel32")]
unsafe extern "system" {
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, timeout: u32) -> u32;
}
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "worker" {
        println!("ready"); io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(60)); return;
    }
    let name: Vec<u16> = format!(r"Local\Switchboot.Upgrade.{}", std::process::id())
        .encode_utf16().chain(Some(0)).collect();
    let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
    assert!(!event.is_null());
    println!("ready"); io::stdout().flush().unwrap();
    if mode == "ignore" { std::thread::sleep(Duration::from_secs(60)); }
    else { assert_eq!(unsafe { WaitForSingleObject(event, 30_000) }, 0); }
}
'''


@unittest.skipUnless(os.name == "nt" and ISCC and shutil.which("rustc"),
                     "Windows, Inno Setup and Rust required")
class UpgradeHandoffTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="switchboot-handoff-test-")
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name)
        source = cls.root / "helper.rs"
        source.write_text(HELPER)
        cls.helper = cls.root / "helper.exe"
        subprocess.run(["rustc", str(source), "-o", str(cls.helper)], check=True,
                       capture_output=True, timeout=60)
        (cls.root / "marker.txt").write_text("harmless fixture payload")
        include = (ROOT / "packaging/windows-shutdown.iss").as_posix()
        installation = (ROOT / "packaging/windows-installation.iss").as_posix()
        fixture = cls.root / "fixture.iss"
        fixture.write_text(f'''
#define UpgradeGUIWaitMs 500
#define UpgradeFileWaitAttempts 5
[Setup]
AppName=Switchboot handoff fixture
AppVersion=1.0.0
DefaultDirName={{tmp}}\\switchboot-handoff-unused
PrivilegesRequired=lowest
Uninstallable=no
CreateUninstallRegKey=no
CloseApplications=no
DisableWelcomePage=yes
DisableDirPage=yes
DisableReadyPage=yes
DisableFinishedPage=yes
OutputDir=.
OutputBaseFilename=fixture
[Files]
Source: "marker.txt"; DestDir: "{{app}}"
[Code]
#include "{installation}"
#include "{include}"
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  if ExpandConstant('{{param:MANUAL|0}}') = '1' then
    Result := PrepareInstalledShutdown(ExpandConstant('{{app}}'))
  else
    Result := PrepareUpgradeShutdown;
  if Result <> '' then Log('Fixture handoff failed: ' + Result);
end;
''')
        compiled = subprocess.run([ISCC, "/Q", str(fixture)],
                                  capture_output=True, text=True, timeout=60)
        if compiled.returncode:
            raise RuntimeError(compiled.stdout + compiled.stderr)
        cls.installer = cls.root / "fixture.exe"

    def setUp(self):
        self.case = Path(tempfile.mkdtemp(dir=self.root))
        self.app = self.case / "app"
        self.app.mkdir()
        self.binary = self.app / "switchboot.exe"
        shutil.copyfile(self.helper, self.binary)

    def start_helper(self, mode=""):
        process = subprocess.Popen([str(self.binary), mode], stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        def cleanup():
            if process.poll() is None:
                process.terminate()
            process.communicate(timeout=10)
        self.addCleanup(cleanup)
        self.assertEqual(process.stdout.readline().strip(), "ready")
        return process

    def install(self, pid=None, manual=False):
        log = self.case / "setup.log"
        args = [str(self.installer), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART",
                "/MANUAL=1" if manual else "/UPGRADE", f"/DIR={self.app}", f"/LOG={log}"]
        if pid is not None:
            args.append(f"/UPGRADEPID={pid}")
        result = subprocess.run(args, timeout=15, capture_output=True)
        return result.returncode, log.read_text(encoding="utf-8-sig")

    def test_gui_exits_and_payload_installs_after_handoff(self):
        gui = self.start_helper()
        code, log = self.install(gui.pid)
        self.assertEqual(code, 0, log)
        self.assertEqual(gui.wait(timeout=5), 0)
        self.assertTrue((self.app / "marker.txt").exists())
        self.assertIn("shutdown complete", log)

    def test_manual_shutdown_discovers_gui_without_pid_argument(self):
        gui = self.start_helper()
        code, log = self.install(manual=True)
        self.assertEqual(code, 0, log)
        self.assertEqual(gui.wait(timeout=5), 0)
        self.assertTrue((self.app / "marker.txt").exists())
        self.assertIn("requested GUI shutdown", log)

    def test_manual_shutdown_without_running_gui_installs(self):
        code, log = self.install(manual=True)
        self.assertEqual(code, 0, log)
        self.assertTrue((self.app / "marker.txt").exists())

    def test_manual_unresponsive_gui_aborts_without_killing_it(self):
        gui = self.start_helper("ignore")
        code, log = self.install(manual=True)
        self.assertNotEqual(code, 0, log)
        self.assertIn("did not finish upgrade shutdown", log)
        self.assertIsNone(gui.poll())
        self.assertFalse((self.app / "marker.txt").exists())

    def test_missing_pid_rejects_before_payload(self):
        code, log = self.install()
        self.assertNotEqual(code, 0)
        self.assertIn("requires a valid /UPGRADEPID", log)
        self.assertFalse((self.app / "marker.txt").exists())

    def test_wrong_executable_is_not_signaled(self):
        gui = self.start_helper()
        code, log = self.install(os.getpid())
        self.assertNotEqual(code, 0)
        self.assertIn("does not belong to the selected installation", log)
        self.assertIsNone(gui.poll())
        self.assertFalse((self.app / "marker.txt").exists())

    def test_unresponsive_gui_times_out_without_installing_or_killing_it(self):
        gui = self.start_helper("ignore")
        code, log = self.install(gui.pid)
        self.assertNotEqual(code, 0)
        self.assertIn("did not finish upgrade shutdown", log)
        self.assertIsNone(gui.poll())
        self.assertFalse((self.app / "marker.txt").exists())

    def test_worker_lock_after_gui_exit_aborts_without_killing_worker(self):
        worker = self.start_helper("worker")
        gui = self.start_helper()
        code, log = self.install(gui.pid)
        self.assertNotEqual(code, 0)
        self.assertEqual(gui.wait(timeout=5), 0)
        self.assertIsNone(worker.poll())
        self.assertIn("still in use after GUI shutdown", log)
        self.assertFalse((self.app / "marker.txt").exists())


if __name__ == "__main__":
    unittest.main()
