#!/usr/bin/env node
import fs from "fs";
import path from "path";
import { encodeIco, linuxIconSizes } from "./icon-formats.mjs";

// Small helper CLI to generate OS icons.
// Linux emits scalable SVGs and hicolor PNG fallbacks; Windows emits mixed DIB/PNG ICOs.

const cwd = process.cwd();
const STATIC_OS_ICONS = path.resolve(cwd, "static", "os-icons");
const RAW_ICON_DIR = path.resolve(cwd, "crates", "switchboot_core", "icons-raw");

const isWin = process.platform === "win32";
const isLinux = process.platform === "linux";
const produceLinuxPngs = isLinux;
const produceIco = isWin;
if (!isWin && !isLinux) throw new Error("Icon generation supports Windows and Linux only");

// Output directories
const OUT_DIR = isLinux
  ? path.resolve(cwd, "crates", "switchboot_core", "icons-linux")
  : path.resolve(cwd, "crates", "switchboot_core", "resources", "icons");
const OUT_ICO_DIR = path.join(OUT_DIR, "ico");

// Sizes used
const ICO_SIZES = [16, 32, 48, 128, 256];
const LINUX_HICOLOR_SIZES = linuxIconSizes.map((px) => ({ dir: `${px}x${px}`, px }));

// Canvas/layout constants
const CANVAS_SIZE = 128;
const CANVAS_PADDING = 10;
const OVERLAY_TARGET_PX = 64;
const OVERLAY_PADDING_X = 0;
const OVERLAY_PADDING_Y = 0;

// Utilities
function ensureDir(dir) {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
}

function readSvg(file) {
  const svg = fs.readFileSync(file, "utf8");
  if (!/<svg\b[^>]*>/i.test(svg)) {
    throw new Error(`Source does not contain an SVG root: ${file}`);
  }
  return svg;
}

function stripSvgWrapper(svg) {
  return svg
    .replace(/<\?xml[^>]*>/, "")
    .replace(/<svg[^>]*>/, "")
    .replace(/<\/svg>/, "");
}

function detectViewBoxOrSize(svg) {
  // prefer viewBox otherwise width/height
  let minX = 0,
    minY = 0,
    w = null,
    h = null;
  const vb = svg.match(/\bviewBox\s*=\s*"([^\"]+)"/i);
  if (vb) {
    const parts = vb[1].trim().split(/\s+/);
    if (parts.length >= 4) {
      minX = parseFloat(parts[0]);
      minY = parseFloat(parts[1]);
      w = parseFloat(parts[2]);
      h = parseFloat(parts[3]);
    }
  }
  if (w == null || h == null) {
    const wm = svg.match(/\bwidth\s*=\s*"([0-9\.]+)\s*(px)?"/i);
    const hm = svg.match(/\bheight\s*=\s*"([0-9\.]+)\s*(px)?"/i);
    if (wm) w = parseFloat(wm[1]);
    if (hm) h = parseFloat(hm[1]);
  }
  if (!w || !h) {
    w = w || 128;
    h = h || 128;
  }
  return { minX, minY, w, h };
}

async function tryImport(moduleName) {
  try {
    return (await import(moduleName)).default;
  } catch (e) {
    return null;
  }
}

async function rasterizeBuffers(sharp, svgString, sizes = ICO_SIZES) {
  const buffers = {};
  for (const s of sizes) {
    try {
      buffers[s] = await sharp(Buffer.from(svgString))
        .resize({ width: s, height: s, fit: "contain" })
        .png({ compressionLevel: 9 })
        .toBuffer();
    } catch (e) {
      throw new Error(`Failed to rasterize at ${s}px`, { cause: e });
    }
  }
  return buffers;
}

async function writeHicolorPngs(sharp, svgString, name) {
  const scalable = path.join(OUT_DIR, "hicolor", "scalable", "apps");
  ensureDir(scalable);
  fs.writeFileSync(path.join(scalable, `dev.zidane.switchboot-${name}.svg`), svgString);
  fs.rmSync(path.join(OUT_DIR, "hicolor", "512x512", "apps", `dev.zidane.switchboot-${name}.png`), { force: true });
  for (const s of LINUX_HICOLOR_SIZES) {
    const dirPath = path.join(OUT_DIR, "hicolor", s.dir, "apps");
    ensureDir(dirPath);

    try {
      const buf = await sharp(Buffer.from(svgString))
        .resize({ width: s.px, height: s.px, fit: "contain" })
        .png()
        .toBuffer();
      const outName = `dev.zidane.switchboot-${name}.png`;
      const outPath = path.join(dirPath, outName);
      fs.writeFileSync(outPath, buf);
      console.log(`Wrote ${path.relative(cwd, outPath)}`);
    } catch (e) {
      throw new Error(`Failed to write hicolor PNG ${name} ${s.px}px`, { cause: e });
    }
  }
}

async function writeIcoFromBuffers(buffers, name) {
  try {
    // prefer larger sizes first
    const prefer = [256, 128, 48, 32, 16];
    const ordered = prefer.filter((s) => buffers[s]);
    if (ordered.length === 0) {
      throw new Error(`No PNG buffers available to build ICO for ${name}`);
    }
    const icoBuf = await encodeIco(buffers, ordered);
    ensureDir(OUT_ICO_DIR);
    const icoPath = path.join(OUT_ICO_DIR, `${name}.ico`);
    fs.writeFileSync(icoPath, icoBuf);
    console.log(`Wrote ${path.relative(cwd, icoPath)}`);
  } catch (e) {
    throw new Error(`Failed to create ICO for ${name}`, { cause: e });
  }
}

// Build a composed SVG: source centered into CANVAS_SIZE with overlay at bottom-right
function composeSvg(sourceInner, sourceBox, overlayInner) {
  const { minX, minY, w: srcW, h: srcH } = sourceBox;
  const srcMax = Math.max(srcW, srcH);
  const canvasInner = CANVAS_SIZE - CANVAS_PADDING * 2;
  const sourceScale = canvasInner / srcMax;
  const finalSrcW = Math.round(srcW * sourceScale);
  const finalSrcH = Math.round(srcH * sourceScale);
  const srcX = Math.round(CANVAS_PADDING + (canvasInner - finalSrcW) / 2);
  const srcY = Math.round(CANVAS_PADDING + (canvasInner - finalSrcH) / 2);

  // overlay sizing
  const overlayMatch = overlayInner.match(/viewBox\s*=\s*"([^\"]+)"/i);
  let overlayNatural = 32;
  if (overlayMatch) {
    const parts = overlayMatch[1].trim().split(/\s+,?\s+/);
    if (parts.length === 4)
      overlayNatural = Math.max(
        parseFloat(parts[2]) || 32,
        parseFloat(parts[3]) || 32
      );
  } else {
    const wM = overlayInner.match(/width\s*=\s*"([^\"]+)"/i);
    const hM = overlayInner.match(/height\s*=\s*"([^\"]+)"/i);
    if (wM) overlayNatural = Math.max(overlayNatural, parseFloat(wM[1]));
    if (hM) overlayNatural = Math.max(overlayNatural, parseFloat(hM[1]));
  }
  const overlaySizePx = OVERLAY_TARGET_PX;
  const overlayX = CANVAS_SIZE - overlaySizePx - OVERLAY_PADDING_X;
  const overlayY = CANVAS_SIZE - overlaySizePx - OVERLAY_PADDING_Y;

  return `<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" width=\"${CANVAS_SIZE}\" height=\"${CANVAS_SIZE}\" viewBox=\"0 0 ${CANVAS_SIZE} ${CANVAS_SIZE}\">\n  <svg x=\"${srcX}\" y=\"${srcY}\" width=\"${finalSrcW}\" height=\"${finalSrcH}\" viewBox=\"${minX} ${minY} ${srcW} ${srcH}\" preserveAspectRatio=\"xMidYMid meet\">\n    ${sourceInner}\n  </svg>\n  <g transform=\"translate(${overlayX}, ${overlayY}) scale(${overlaySizePx / overlayNatural})\">\n    ${overlayInner}\n  </g>\n</svg>`;
}

async function run() {
  ensureDir(OUT_DIR);
  if (produceLinuxPngs) ensureDir(path.join(OUT_DIR, "hicolor"));
  if (produceIco) ensureDir(OUT_ICO_DIR);

  // locate overlay
  let overlayPath = path.join(RAW_ICON_DIR, "shortcut-overlay.svg");
  if (!fs.existsSync(overlayPath))
    overlayPath = path.join(STATIC_OS_ICONS, "..", "shortcut-overlay.svg");
  if (!fs.existsSync(overlayPath)) {
    console.error("Missing shortcut-overlay.svg in helper or static/os-icons");
    process.exit(1);
  }
  const overlay = readSvg(overlayPath);
  const overlayInner = stripSvgWrapper(overlay);

  // list source icons
  const files = fs
    .readdirSync(STATIC_OS_ICONS)
    .filter((f) => f.endsWith(".svg") && f !== "shortcut-overlay.svg");
  if (files.length === 0) throw new Error("No source OS icons found");

  // dynamic imports
  const sharp = await tryImport("sharp");
  if (!sharp) throw new Error("Required dependency sharp could not be loaded");

  // process generic first
  const genericCandidates = [
    path.join(RAW_ICON_DIR, "shortcut-generic.svg"),
    path.join(cwd, "static", "shortcut-generic.svg"),
  ];
  const genericPath = genericCandidates.find((p) => fs.existsSync(p));
  if (!genericPath) throw new Error("Required generic shortcut icon is missing");
  if (genericPath) {
    const genericSvg = readSvg(genericPath);
    if (produceLinuxPngs) {
      if (!sharp)
        console.error(
          "sharp is required to emit Linux PNG icons. Install it with: pnpm add -D sharp"
        );
      else await writeHicolorPngs(sharp, genericSvg, "generic");
    }

    if (produceIco) {
      const buffers = await rasterizeBuffers(sharp, genericSvg);
      await writeIcoFromBuffers(buffers, "generic");
    }
  }

  // iterate icons
  for (const f of files) {
    const name = path.basename(f, ".svg");
    const srcSvg = readSvg(path.join(STATIC_OS_ICONS, f));
    // Keep root-level fills and strokes, especially for monochrome EFI icons.
    const sourceInner = srcSvg.replace(/<\?xml[^>]*\?>/g, "");
    const sourceBox = detectViewBoxOrSize(srcSvg);
    const combined = composeSvg(sourceInner, sourceBox, overlayInner);

    if (produceLinuxPngs) {
      if (!sharp) {
        console.error(
          "sharp is required to emit Linux PNG icons. Install it with: pnpm add -D sharp"
        );
        process.exit(1);
      }
      await writeHicolorPngs(sharp, combined, name);
    }

    // produce in-memory PNGs for ICO
    let producedBuffers = {};
    if (produceIco) {
      if (!sharp) {
        console.error(
          "sharp is required to emit PNG/ICO. Install it with: pnpm add -D sharp"
        );
        process.exit(1);
      }
      producedBuffers = await rasterizeBuffers(sharp, combined, ICO_SIZES);
    }

    if (!produceIco) {
      console.log(
        "Platform configured to not produce ICOs; skipping .ico generation"
      );
    } else {
      await writeIcoFromBuffers(producedBuffers, name);
    }
  }
}

run().catch((err) => {
  console.error(err);
  process.exit(1);
});
