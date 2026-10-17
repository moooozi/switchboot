"""Opt-in desktop tests of the harmless maintenance preview, never the real installer."""

import os
import subprocess
import unittest

from test_preview import ROOT, UI_CLASS, caption, click, screenshot, u, wait_for, windows

EXE = ROOT / "artifacts/Switchboot_NSIS-maintenance-preview.exe"


class MaintenanceTests(unittest.TestCase):
    def preview(self, theme):
        self.assertTrue(EXE.is_file(), "Build with --maintenance first")
        before = set(windows(UI_CLASS))
        process = subprocess.Popen([str(EXE)], env={**os.environ, "SWITCHBOOT_NSIS_THEME": theme})
        self.addCleanup(lambda: process.poll() is None and process.kill())
        window = wait_for(lambda: next((w for w in windows(UI_CLASS) if w not in before), None), "maintenance fixture")
        self.assertEqual(caption(window), "Setup - Switchboot (NSIS preview)")
        self.assertFalse(u.GetDlgItem(window, 101))
        self.assertEqual(caption(u.GetDlgItem(window, 104)), "&Uninstall")
        self.assertEqual(caption(u.GetDlgItem(window, 105)), "&Reinstall")
        self.assertEqual(caption(u.GetDlgItem(window, 1)), "&Launch")
        return process, window

    def confirm(self, window, control, yes):
        before = set(windows("#32770"))
        click(window, control)
        dialog = wait_for(lambda: next((w for w in windows("#32770") if w not in before and caption(w) == "Switchboot Setup"), None), "safe action confirmation")
        click(dialog, 6 if yes else 7)
        wait_for(lambda: dialog not in windows("#32770"), "confirmation close")

    def test_launch_and_both_themes(self):
        for theme in ["light", "dark"]:
            with self.subTest(theme=theme):
                process, window = self.preview(theme)
                screenshot(window, ROOT / f"artifacts/nsis-maintenance-{theme}.png")
                click(window, 1)
                self.assertEqual(process.wait(timeout=10), 10)

    def test_uninstall_and_reinstall_only_return_actions(self):
        for control, code in [(104, 12), (105, 11)]:
            with self.subTest(control=control):
                process, window = self.preview("light")
                self.confirm(window, control, False)
                self.assertIsNone(process.poll())
                self.confirm(window, control, True)
                self.assertEqual(process.wait(timeout=10), code)

    def test_escape_is_cancel_not_reinstall(self):
        process, window = self.preview("dark")
        # IsDialogMessage translates Escape to IDCANCEL even without a Cancel button.
        u.PostMessageW(window, 0x100, 0x1B, 0)
        self.assertEqual(process.wait(timeout=10), 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
