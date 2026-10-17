"""Opt-in desktop smoke tests. Only launch the harmless NSIS preview artifact.

Run directly, not from the production packaging test discovery. Uses only stdlib.
"""
import argparse
import ctypes as C
import os
import sys
from ctypes import wintypes as W
from pathlib import Path
import struct
import subprocess
import time
import unittest
import zlib

ROOT = Path(__file__).resolve().parents[2]
RENDERER = "svg"
THEME = "dark"
EXE = ROOT / "artifacts/Switchboot_NSIS-preview-svg.exe"
UI_CLASS = "SwitchbootNSISPreview"
u = C.WinDLL("user32", use_last_error=True)
g = C.WinDLL("gdi32", use_last_error=True)
u.SetProcessDpiAwarenessContext.argtypes = [C.c_void_p]
u.SetProcessDpiAwarenessContext(C.c_void_p(-4))
CALLBACK = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
u.EnumWindows.argtypes = [CALLBACK, W.LPARAM]
u.EnumChildWindows.argtypes = [W.HWND, CALLBACK, W.LPARAM]
u.GetClassNameW.argtypes = [W.HWND, W.LPWSTR, C.c_int]
u.GetWindowTextW.argtypes = [W.HWND, W.LPWSTR, C.c_int]
u.GetDlgItem.argtypes = [W.HWND, C.c_int]
u.GetDlgItem.restype = W.HWND
u.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
u.SendMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
u.SendMessageW.restype = C.c_ssize_t
u.SetWindowTextW.argtypes = [W.HWND, W.LPCWSTR]
u.GetWindowRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
u.IsWindowVisible.argtypes = [W.HWND]
u.GetDC.argtypes = [W.HWND]
u.GetDC.restype = W.HDC
u.ReleaseDC.argtypes = [W.HWND, W.HDC]
u.PrintWindow.argtypes = [W.HWND, W.HDC, W.UINT]
g.CreateCompatibleDC.argtypes = [W.HDC]
g.CreateCompatibleDC.restype = W.HDC
g.CreateCompatibleBitmap.argtypes = [W.HDC, C.c_int, C.c_int]
g.CreateCompatibleBitmap.restype = W.HBITMAP
g.SelectObject.argtypes = [W.HDC, W.HGDIOBJ]
g.SelectObject.restype = W.HGDIOBJ
g.DeleteObject.argtypes = [W.HGDIOBJ]
g.DeleteDC.argtypes = [W.HDC]
g.GetDIBits.argtypes = [W.HDC, W.HBITMAP, W.UINT, W.UINT, C.c_void_p, C.c_void_p, W.UINT]
u.GetPropW.argtypes = [W.HWND, W.LPCWSTR]
u.GetPropW.restype = W.HANDLE
u.GetDpiForWindow.argtypes = [W.HWND]
u.GetDpiForWindow.restype = W.UINT
u.GetClientRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
u.SetWindowPos.argtypes = [W.HWND, W.HWND, C.c_int, C.c_int, C.c_int, C.c_int, W.UINT]
u.MapWindowPoints.argtypes = [W.HWND, W.HWND, C.POINTER(W.POINT), W.UINT]
u.UpdateWindow.argtypes = [W.HWND]
u.GetUpdateRect.argtypes = [W.HWND, C.POINTER(W.RECT), W.BOOL]
u.GetCursorPos.argtypes = [C.POINTER(W.POINT)]
u.SetCursorPos.argtypes = [C.c_int, C.c_int]
u.GetWindowDpiAwarenessContext.argtypes = [W.HWND]
u.GetWindowDpiAwarenessContext.restype = C.c_void_p
u.AreDpiAwarenessContextsEqual.argtypes = [C.c_void_p, C.c_void_p]
MONITOR_CALLBACK = C.WINFUNCTYPE(W.BOOL, W.HMONITOR, W.HDC, C.POINTER(W.RECT), W.LPARAM)
u.EnumDisplayMonitors.argtypes = [W.HDC, C.POINTER(W.RECT), MONITOR_CALLBACK, W.LPARAM]
u.GetMonitorInfoW.argtypes = [W.HMONITOR, C.c_void_p]


class MonitorInfo(C.Structure):
    _fields_ = [("size", W.DWORD), ("monitor", W.RECT), ("work", W.RECT), ("flags", W.DWORD)]


class LogFont(C.Structure):
    _fields_ = [("height", W.LONG), ("width", W.LONG), ("escapement", W.LONG),
                ("orientation", W.LONG), ("weight", W.LONG),
                ("italic", W.BYTE), ("underline", W.BYTE), ("strike", W.BYTE),
                ("charset", W.BYTE), ("out", W.BYTE), ("clip", W.BYTE),
                ("quality", W.BYTE), ("pitch", W.BYTE), ("face", W.WCHAR * 32)]


g.GetObjectW.argtypes = [W.HGDIOBJ, C.c_int, C.c_void_p]


def monitor_work_areas():
    result = []

    @MONITOR_CALLBACK
    def visit(monitor, _dc, _rect, _param):
        info = MonitorInfo(size=C.sizeof(MonitorInfo))
        if u.GetMonitorInfoW(monitor, C.byref(info)):
            result.append(info.work)
        return True

    u.EnumDisplayMonitors(None, None, visit, 0)
    return result


def scale(value, dpi):
    return (value * dpi + 48) // 96


def windows(class_name):
    found = []

    @CALLBACK
    def visit(hwnd, _):
        name = C.create_unicode_buffer(256)
        u.GetClassNameW(hwnd, name, len(name))
        if name.value == class_name and u.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True

    u.EnumWindows(visit, 0)
    return found


def wait_for(predicate, description):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError("Timed out: " + description)


def caption(hwnd):
    text = C.create_unicode_buffer(1024)
    u.SendMessageW(hwnd, 0xD, len(text), C.addressof(text))
    return text.value


def set_text(hwnd, text):
    # Cross-process controls require WM_SETTEXT; SetWindowText only updates the
    # cached window caption and does not exercise the edit control's buffer.
    value = C.create_unicode_buffer(text)
    u.SendMessageW(hwnd, 0xC, 0, C.addressof(value))


def click(hwnd, control):
    # Post WM_COMMAND so modal Browse/error dialogs cannot block the test thread.
    u.PostMessageW(hwnd, 0x111, control, u.GetDlgItem(hwnd, control) or 0)


def screenshot(hwnd, path):
    """PrintWindow capture of just our window, never the user's desktop."""
    rect = W.RECT()
    u.GetWindowRect(hwnd, C.byref(rect))
    width, height = rect.right - rect.left, rect.bottom - rect.top
    dc = u.GetDC(hwnd)
    memory = g.CreateCompatibleDC(dc)
    bitmap = g.CreateCompatibleBitmap(dc, width, height)
    previous = g.SelectObject(memory, bitmap)
    try:
        if not u.PrintWindow(hwnd, memory, 2):
            raise AssertionError("PrintWindow failed")
        g.SelectObject(memory, previous)
        header = struct.pack("<IiiHHIIiiII", 40, width, height, 1, 32, 0, width * height * 4, 0, 0, 0, 0)
        info = C.create_string_buffer(header)
        pixels = C.create_string_buffer(width * height * 4)
        if not g.GetDIBits(memory, bitmap, 0, height, pixels, info, 0):
            raise AssertionError("GetDIBits failed")
        rows = []
        for y in reversed(range(height)):
            row = pixels.raw[y * width * 4:(y + 1) * width * 4]
            rgb = bytearray(width * 3)
            rgb[0::3], rgb[1::3], rgb[2::3] = row[2::4], row[1::4], row[0::4]
            rows.append(b"\0" + rgb)

        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

        path.write_bytes(b"\x89PNG\r\n\x1a\n"
                         + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                         + chunk(b"IDAT", zlib.compress(b"".join(rows))) + chunk(b"IEND", b""))
        return width, height, pixels.raw
    finally:
        g.SelectObject(memory, previous)
        g.DeleteObject(bitmap)
        g.DeleteDC(memory)
        u.ReleaseDC(hwnd, dc)


class PreviewTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(EXE.is_file(), "Run build.py first")
        self.baseline = set(windows(UI_CLASS))
        self.dialog_baseline = set(windows("#32770"))
        env = os.environ.copy()
        env["SWITCHBOOT_NSIS_THEME"] = THEME
        self.expected_renderer = 1 if RENDERER == "svg" else 2
        if self._testMethodName == "test_forced_svg_fallback" and RENDERER == "svg":
            env["SWITCHBOOT_NSIS_FORCE_ICO"] = "1"
            self.expected_renderer = 3
        else:
            env.pop("SWITCHBOOT_NSIS_FORCE_ICO", None)
        self.process = subprocess.Popen([str(EXE)], env=env)
        self.addCleanup(self.stop_preview)

        def startup():
            for dialog in windows("#32770"):
                if dialog in self.dialog_baseline or caption(dialog) != "Setup - Switchboot (NSIS preview)":
                    continue
                labels = []
                @CALLBACK
                def visit(child, _):
                    labels.append(caption(child))
                    return True
                u.EnumChildWindows(dialog, visit, 0)
                if any("Unable to create the preview UI" in value or "Unable to load the preview UI" in value for value in labels):
                    click(dialog, 1)
                    raise AssertionError("Preview startup failed: " + " ".join(labels))
            return next((w for w in windows(UI_CLASS) if w not in self.baseline), None)

        self.window = wait_for(startup, "preview window")
        self.pid = W.DWORD()
        u.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
        u.GetWindowThreadProcessId(self.window, C.byref(self.pid))

    def stop_preview(self):
        for hwnd in windows(UI_CLASS):
            if hwnd not in self.baseline:
                u.PostMessageW(hwnd, 0x10, 0, 0)
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            # Only our preview process tree, never Switchboot or any service.
            subprocess.run(["taskkill", "/PID", str(self.process.pid), "/T", "/F"], capture_output=True, timeout=10)
            self.process.wait(timeout=5)

    def assert_layout(self, window):
        dpi = u.GetDpiForWindow(window)
        self.assertEqual(u.GetPropW(window, "Switchboot.NsisLogoRenderer"), self.expected_renderer,
                         "SVG checks must not silently pass using the ICO fallback")
        client = W.RECT()
        u.GetClientRect(window, C.byref(client))
        self.assertEqual((client.right, client.bottom), (scale(796, dpi), scale(394, dpi)))
        for id, bounds in [(101, (72, 218, 538, 23)), (102, (54, 168, 300, 22)),
                           (100, (644, 208, 98, 42)), (103, (54, 310, 680, 22)),
                           (1, (572, 340, 98, 42)), (2, (685, 340, 98, 42))]:
            control = u.GetDlgItem(window, id)
            if not control:
                continue
            rect = W.RECT()
            u.GetWindowRect(control, C.byref(rect))
            u.MapWindowPoints(None, window, C.cast(C.byref(rect), C.POINTER(W.POINT)), 2)
            x, y, width, height = bounds
            self.assertEqual((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top),
                             tuple(scale(v, dpi) for v in (x, y, width, height)))
            self.assertGreaterEqual(rect.left, 0)
            self.assertLessEqual(rect.right, client.right)
            self.assertLessEqual(rect.bottom, client.bottom)
            font = LogFont()
            handle = u.SendMessageW(control, 0x31, 0, 0)  # WM_GETFONT
            self.assertTrue(g.GetObjectW(handle, C.sizeof(font), C.byref(font)))
            self.assertEqual(font.height, -scale(15, dpi))
        return dpi

    def test_selected_logo_renderer(self):
        self.assertEqual(u.GetPropW(self.window, "Switchboot.NsisLogoRenderer"), self.expected_renderer)

    def test_forced_svg_fallback(self):
        if RENDERER != "svg":
            self.skipTest("SVG-only fallback test")
        self.assert_layout(self.window)
        screenshot(self.window, ROOT / "artifacts/nsis-preview-svg-fallback.png")

    def test_per_monitor_v2_initial_layout(self):
        self.assertTrue(u.AreDpiAwarenessContextsEqual(u.GetWindowDpiAwarenessContext(self.window), C.c_void_p(-4)))
        self.assert_layout(self.window)

    def test_monitor_moves_relayout_selection_and_completion(self):
        path = r"C:\Preview DPI\Switchboot"
        edit = u.GetDlgItem(self.window, 101)
        set_text(edit, path)
        u.SendMessageW(edit, 0xB1, 2, 7)  # EM_SETSEL
        seen = set()
        for _ in range(2):
            for area in monitor_work_areas():
                self.assertTrue(u.SetWindowPos(self.window, None, area.left + 30, area.top + 30, 0, 0, 0x15))
                u.UpdateWindow(self.window)
                seen.add(self.assert_layout(self.window))
                self.assertEqual(caption(edit), path)
                self.assertEqual(u.SendMessageW(edit, 0xB0, 0, 0), 2 | (7 << 16))  # EM_GETSEL
                dpi = u.GetDpiForWindow(self.window)
                screenshot(self.window, ROOT / f"artifacts/nsis-preview-dpi-{dpi}.png")
        click(self.window, 1)
        finished = wait_for(lambda: next((w for w in windows(UI_CLASS)
            if w not in self.baseline and not u.GetDlgItem(w, 101)), None), "completion page")
        for area in monitor_work_areas():
            u.SetWindowPos(finished, None, area.left + 30, area.top + 30, 0, 0, 0x15)
            u.UpdateWindow(finished)
            self.assert_layout(finished)
        click(finished, 1)
        self.assertEqual(self.process.wait(timeout=10), 0)
        if len(seen) < 2:
            self.skipTest("Layout passed, but multiple physical monitor DPIs were not available")

    def test_hover_repaints_on_transition_not_every_mouse_move(self):
        button = u.GetDlgItem(self.window, 1)
        previous, rect = W.POINT(), W.RECT()
        u.GetCursorPos(C.byref(previous))
        self.addCleanup(u.SetCursorPos, previous.x, previous.y)
        u.GetWindowRect(button, C.byref(rect))
        # TrackMouseEvent checks the real pointer: posting synthetic moves with
        # the pointer elsewhere immediately generates a legitimate mouse-leave.
        u.SetCursorPos((rect.left + rect.right) // 2, (rect.top + rect.bottom) // 2)
        time.sleep(0.1)
        u.SendMessageW(button, 0x2A3, 0, 0)  # WM_MOUSELEAVE
        u.UpdateWindow(button)
        u.SendMessageW(button, 0x200, 0, 8 | (8 << 16))
        u.UpdateWindow(button)
        for x in range(9, 30):
            u.SendMessageW(button, 0x200, 0, x | (8 << 16))
            self.assertFalse(u.GetUpdateRect(button, None, False), "Stationary hover must not repaint")
        screenshot(self.window, ROOT / "artifacts/nsis-preview-hover.png")
        u.SendMessageW(button, 0x2A3, 0, 0)
        u.UpdateWindow(button)

    def test_button_edges_have_antialiased_pixels(self):
        u.UpdateWindow(self.window)
        width, height, pixels = screenshot(self.window, ROOT / "artifacts/nsis-preview-aa.png")
        window, button = W.RECT(), W.RECT()
        u.GetWindowRect(self.window, C.byref(window))
        u.GetWindowRect(u.GetDlgItem(self.window, 1), C.byref(button))
        blends = 0
        for y in range(button.top - window.top, button.top - window.top + 3):
            for x in range(button.left - window.left, button.right - window.left):
                i = ((height - 1 - y) * width + x) * 4
                blue, green, red = pixels[i:i + 3]
                if ((THEME == "dark" and 0 < red < 23 and 23 < green < 185 and 23 < blue < 232)
                        or (THEME == "light" and 0 < red < 245 and 185 < green < 245 and 232 < blue < 245)):
                    blends += 1
        self.assertGreater(blends, 0, "Rounded edges should blend cyan into the background")

    def test_cancel_and_capture(self):
        self.assertEqual(caption(u.GetDlgItem(self.window, 101)), r"C:\Program Files\Switchboot")
        screenshot(self.window, ROOT / "artifacts/nsis-preview.png")
        click(self.window, 2)
        self.assertEqual(self.process.wait(timeout=10), 1)

    def test_install_runs_nsis_fixture_then_completes(self):
        # This deliberately nonexistent Unicode destination must remain untouched.
        destination = ROOT / "artifacts/预览-only-do-not-create"
        self.assertFalse(destination.exists())
        set_text(u.GetDlgItem(self.window, 101), str(destination))
        click(self.window, 1)
        finished = wait_for(lambda: next((w for w in windows(UI_CLASS)
            if w not in self.baseline and not u.GetDlgItem(w, 101)), None), "completion page")
        screenshot(finished, ROOT / "artifacts/nsis-preview-complete.png")
        click(finished, 1)
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assertFalse(destination.exists())

    def test_browse_cancel_preserves_typed_path(self):
        path = r"C:\Preview only\Switchboot"
        set_text(u.GetDlgItem(self.window, 101), path)
        previous = set(windows("#32770"))
        click(self.window, 100)
        dialog = wait_for(lambda: next((w for w in windows("#32770") if w not in previous), None), "folder browser")
        click(dialog, 2)
        wait_for(lambda: dialog not in windows("#32770"), "folder browser close")
        self.assertEqual(caption(u.GetDlgItem(self.window, 101)), path)
        click(self.window, 2)
        self.assertEqual(self.process.wait(timeout=10), 1)

    def test_relative_path_is_rejected(self):
        set_text(u.GetDlgItem(self.window, 101), "relative-path")
        click(self.window, 1)
        wait_for(lambda: caption(u.GetDlgItem(self.window, 103)), "inline path validation")
        self.assertEqual(caption(u.GetDlgItem(self.window, 103)), "Enter an absolute installation folder path.")
        self.assertTrue(u.GetDlgItem(self.window, 101))
        click(self.window, 2)
        self.assertEqual(self.process.wait(timeout=10), 1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--renderer", choices=["raster", "svg"], default="svg")
    parser.add_argument("--theme", choices=["light", "dark"], default="dark")
    args, remaining = parser.parse_known_args()
    RENDERER = args.renderer
    THEME = args.theme
    EXE = ROOT / f"artifacts/Switchboot_NSIS-preview-{RENDERER}.exe"
    unittest.main(argv=[sys.argv[0], *remaining], verbosity=2)
