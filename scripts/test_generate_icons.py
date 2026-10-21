"""Validate both desktop icon formats using isolated SVG fixtures."""
from io import BytesIO
from pathlib import Path
import struct
import tempfile
import unittest

from PIL import Image
from generate_icons import (LINUX_SIZES, ICO_SIZES, compose, dib_frame,
                            encode_ico, generate_icons)

SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128" stroke="red"><rect width="128" height="128" fill="blue"/></svg>'


class IconTests(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for name in ("app-icon.svg", "static/os-icons/fixture.svg",
                     "crates/switchboot_core/icons-raw/shortcut-generic.svg",
                     "crates/switchboot_core/icons-raw/shortcut-overlay.svg"):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(SVG, encoding="utf-8")
        return root

    def assert_ico(self, data, sizes, blue=False):
        self.assertEqual(struct.unpack_from("<HHH", data), (0, 1, len(sizes)))
        offset = 6 + len(sizes) * 16
        for index, size in enumerate(sizes):
            width, height, colors, reserved, planes, bits, length, frame_offset = struct.unpack_from("<BBBBHHII", data, 6 + index * 16)
            self.assertEqual((width or 256, height or 256), (size, size))
            self.assertEqual((colors, reserved, planes, bits), (0, 0, 1, 32))
            self.assertEqual(frame_offset, offset)
            frame = data[offset:offset + length]
            self.assertEqual(len(frame), length)
            if size <= 48:
                self.assertEqual(struct.unpack_from("<IIIHH", frame), (40, size, size * 2, 1, 32))
                self.assertEqual(length, 40 + size * size * 4 + (size + 31) // 32 * 4 * size)
                if blue:
                    # Inspect a center pixel, away from the red fixture stroke.
                    pixel = 40 + (size // 2 * size + size // 2) * 4
                    self.assertEqual(frame[pixel:pixel + 4], bytes((255, 0, 0, 255)))
            else:
                self.assertEqual(frame[:8], b"\x89PNG\r\n\x1a\n")
                with Image.open(BytesIO(frame)) as image:
                    self.assertEqual(image.size, (size, size))
                    self.assertEqual(image.mode, "RGBA")
            offset += length
        self.assertEqual(offset, len(data))
        self.assertLess(len(data), 353118 // 2)

    def test_ico_frames_preserve_sizes_offsets_and_blue_pixels(self):
        self.assert_ico(encode_ico(SVG), ICO_SIZES, blue=True)

    def test_dib_preserves_straight_alpha_flips_rows_and_sets_mask(self):
        image = Image.new("RGBA", (16, 16))
        image.putpixel((0, 0), (128, 64, 32, 128))
        frame = dib_frame(image)
        top = 40 + 15 * 16 * 4
        self.assertEqual(frame[top:top + 4], bytes((32, 64, 128, 128)))
        self.assertEqual(frame[40:44], bytes(4))
        self.assertEqual(frame[40 + 16 * 16 * 4 + 15 * 4], 0x7f)

    def test_generates_all_platform_outputs_and_removes_obsolete_sizes(self):
        root = self.fixture()
        core = root / "crates/switchboot_core"
        stale = ["icons/512x512.png", "icons-linux/hicolor/512x512/apps/dev.zidane.switchboot-fixture.png",
                 "icons-linux/hicolor/512x512/apps/dev.zidane.switchboot-generic.png"]
        for name in stale:
            path = core / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"stale")
        generate_icons(root, "all")
        for name in stale:
            self.assertFalse((core / name).exists())
        for size in LINUX_SIZES:
            for name in [f"icons/{size}x{size}.png",
                         f"icons-linux/hicolor/{size}x{size}/apps/dev.zidane.switchboot-fixture.png",
                         f"icons-linux/hicolor/{size}x{size}/apps/dev.zidane.switchboot-generic.png"]:
                with Image.open(core / name) as image:
                    self.assertEqual(image.size, (size, size))
                    self.assertEqual(image.mode, "RGBA")
        self.assert_ico((core / "icons/icon.ico").read_bytes(), ICO_SIZES)
        for name in ("fixture", "generic"):
            self.assert_ico((core / f"resources/icons/ico/{name}.ico").read_bytes(), tuple(reversed(ICO_SIZES)))
        scalable = (core / "icons-linux/hicolor/scalable/apps/dev.zidane.switchboot-fixture.svg").read_text()
        self.assertIn('stroke="red"', scalable)
        self.assertIn('<g transform="', scalable)
        before = (core / "icons/icon.ico").read_bytes()
        generate_icons(root, "all")
        self.assertEqual((core / "icons/icon.ico").read_bytes(), before)

    def test_platform_limits_shortcut_outputs(self):
        for platform in ("windows", "linux"):
            root = self.fixture()
            generate_icons(root, platform)
            core = root / "crates/switchboot_core"
            self.assertEqual((core / "resources/icons/ico/fixture.ico").exists(), platform == "windows")
            self.assertEqual((core / "icons-linux/hicolor/scalable/apps/dev.zidane.switchboot-fixture.svg").exists(), platform == "linux")

    def test_bad_or_missing_sources_fail_before_overwriting_outputs(self):
        for name in ("app-icon.svg", "static/os-icons/fixture.svg",
                     "crates/switchboot_core/icons-raw/shortcut-generic.svg",
                     "crates/switchboot_core/icons-raw/shortcut-overlay.svg"):
            for corrupt in (False, True):
                with self.subTest(name=name, corrupt=corrupt):
                    root = self.fixture()
                    output = root / "crates/switchboot_core/icons/icon.ico"
                    output.parent.mkdir(parents=True)
                    output.write_bytes(b"previous output")
                    if corrupt:
                        (root / name).write_text("invalid SVG")
                    else:
                        (root / name).unlink()
                    with self.assertRaises((ValueError, FileNotFoundError)):
                        generate_icons(root, "all")
                    self.assertEqual(output.read_bytes(), b"previous output")

    def test_composition_preserves_styles_and_layout(self):
        svg = compose(SVG, SVG)
        self.assertIn('stroke="red"', svg)
        self.assertIn('translate(64, 64) scale(0.5)', svg)
        self.assertIn('x="10" y="10" width="108" height="108"', svg)
