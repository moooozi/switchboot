"""Generate app and shortcut icons from SVG sources using prebuilt Python wheels."""
import argparse
from io import BytesIO
import math
from pathlib import Path
import re
import struct
import sys
from xml.dom import minidom

from PIL import Image, ImageOps
import resvg_py

ROOT = Path(__file__).resolve().parent.parent
LINUX_SIZES = (32, 48, 64, 128, 256)
ICO_SIZES = (16, 32, 48, 128, 256)


def rasterize(svg, size):
    png = resvg_py.svg_to_bytes(svg_string=svg, width=size, height=size,
                                skip_system_fonts=True)
    with Image.open(BytesIO(png)) as image:
        return ImageOps.pad(image.convert("RGBA"), (size, size), color=(0, 0, 0, 0))


def png_bytes(image):
    output = BytesIO()
    image.save(output, format="PNG", compress_level=9)
    return output.getvalue()


def dib_frame(image):
    size = image.width
    mask_stride = (size + 31) // 32 * 4
    mask = bytearray(mask_stride * size)
    alpha = image.getchannel("A").tobytes()
    for y in range(size):
        for x in range(size):
            if alpha[y * size + x] == 0:
                mask[(size - y - 1) * mask_stride + x // 8] |= 0x80 >> (x % 8)
    header = struct.pack("<IIIHHIIIIII", 40, size, size * 2, 1, 32, 0,
                         size * size * 4, 0, 0, 0, 0)
    # ICO DIBs store straight-alpha BGRA pixels, bottom row first.
    return header + image.tobytes("raw", "BGRA", 0, -1) + mask


def encode_ico(svg, sizes=ICO_SIZES):
    header = bytearray(struct.pack("<HHH", 0, 1, len(sizes)))
    frames = []
    offset = 6 + len(sizes) * 16
    for size in sizes:
        image = rasterize(svg, size)
        frame = png_bytes(image) if size > 48 else dib_frame(image)
        header.extend(struct.pack("<BBBBHHII", size % 256, size % 256,
                                  0, 0, 1, 32, len(frame), offset))
        frames.append(frame)
        offset += len(frame)
    return bytes(header) + b"".join(frames)


def svg_root(svg):
    document = minidom.parseString(svg)
    root = document.documentElement
    if root.localName != "svg":
        raise ValueError("Source does not contain an SVG root")
    return root


def read_svg(path):
    svg = path.read_text(encoding="utf-8")
    try:
        svg_root(svg)
        rasterize(svg, 16)
    except Exception as error:
        raise ValueError(f"Invalid SVG {path}: {error}") from error
    return svg


def compose(source, overlay):
    root = svg_root(source)
    view_box = root.getAttribute("viewBox")
    if view_box:
        values = [float(v) for v in re.split(r"[\s,]+", view_box.strip())]
    else:
        values = [0, 0, float(root.getAttribute("width").removesuffix("px") or 128),
                  float(root.getAttribute("height").removesuffix("px") or 128)]
    if len(values) != 4 or not all(math.isfinite(v) for v in values) or min(values[2:]) <= 0:
        raise ValueError("Invalid source viewBox")
    view_box = " ".join(f"{v:g}" for v in values)
    scale = 108 / max(values[2:])
    # Match the existing JavaScript layout's rounding, including half pixels.
    width, height = (math.floor(v * scale + 0.5) for v in values[2:])
    x, y = (math.floor(10 + (108 - v) / 2 + 0.5) for v in (width, height))
    overlay_root = svg_root(overlay)
    inner = "".join(child.toxml() for child in overlay_root.childNodes)

    def first_dimension(attribute):
        for element in overlay_root.getElementsByTagName("*"):
            if element.hasAttribute(attribute):
                return float(element.getAttribute(attribute).removesuffix("px"))
        return 32

    # Retain the established overlay sizing after removing its outer SVG.
    natural = max(32, first_dimension("width"), first_dimension("height"))
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">'
            f'<svg x="{x}" y="{y}" width="{width}" height="{height}" viewBox="{view_box}" '
            f'preserveAspectRatio="xMidYMid meet">{root.toxml()}</svg>'
            f'<g transform="translate(64, 64) scale({64 / natural:g})">{inner}</g></svg>')


def write(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content.encode("utf-8") if isinstance(content, str) else content)


def generate_icons(root=ROOT, platform=None):
    platform = platform or {"win32": "windows", "linux": "linux"}.get(sys.platform)
    if platform not in ("windows", "linux", "all"):
        raise ValueError("Icon generation supports Windows and Linux only")
    root = Path(root)
    core = root / "crates/switchboot_core"
    app = read_svg(root / "app-icon.svg")
    overlay = read_svg(core / "icons-raw/shortcut-overlay.svg")
    generic = read_svg(core / "icons-raw/shortcut-generic.svg")
    sources = sorted(p for p in (root / "static/os-icons").glob("*.svg")
                     if p.name != "shortcut-overlay.svg")
    if not sources:
        raise ValueError("No source OS icons found")
    shortcuts = [("generic", generic)]
    # Validate all inputs before replacing generated assets.
    for path in sources:
        svg = compose(read_svg(path), overlay)
        rasterize(svg, 16)
        shortcuts.append((path.stem, svg))
    for size in LINUX_SIZES:
        write(core / f"icons/{size}x{size}.png", png_bytes(rasterize(app, size)))
    write(core / "icons/icon.ico", encode_ico(app))
    (core / "icons/512x512.png").unlink(missing_ok=True)
    for name, svg in shortcuts:
        if platform in ("windows", "all"):
            write(core / f"resources/icons/ico/{name}.ico", encode_ico(svg, tuple(reversed(ICO_SIZES))))
        if platform in ("linux", "all"):
            hicolor = core / "icons-linux/hicolor"
            write(hicolor / f"scalable/apps/dev.zidane.switchboot-{name}.svg", svg)
            for size in LINUX_SIZES:
                write(hicolor / f"{size}x{size}/apps/dev.zidane.switchboot-{name}.png",
                      png_bytes(rasterize(svg, size)))
            (hicolor / f"512x512/apps/dev.zidane.switchboot-{name}.png").unlink(missing_ok=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--platform", choices=("windows", "linux", "all"))
    args = parser.parse_args()
    try:
        generate_icons(args.root, args.platform)
    except Exception as error:
        parser.exit(1, f"{error}\n")
    print(f"Generated application and shortcut icons under {args.root}")
