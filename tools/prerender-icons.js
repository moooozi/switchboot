#!/usr/bin/env node
import fs from "fs";
import path from "path";

// Small helper CLI to generate OS icons.
// Behavior unchanged: on Linux we emit hicolor PNGs; on Windows/others we emit ICOs.

const cwd = process.cwd();
const STATIC_OS_ICONS = path.resolve(cwd, "static", "os-icons");
const RAW_ICON_DIR = path.resolve(cwd, "src-tauri", "icons-raw");

const isWin = process.platform === "win32";
const isLinux = process.platform === "linux";
const produceLinuxPngs = isLinux;
const produceIco = isWin || (!isWin && !isLinux);

// Output directories
const OUT_DIR = isLinux
  ? path.resolve(cwd, "src-tauri", "icons-linux")
  : path.resolve(cwd, "src-tauri", "resources", "icons");
const OUT_ICO_DIR = path.join(OUT_DIR, "ico");

// Sizes used
const ICO_SIZES = [16, 32, 48, 128, 256];
const LINUX_HICOLOR_SIZES = [
  { dir: "32x32", px: 32 },
  { dir: "128x128", px: 128 },
  { dir: "512x512", px: 512 },
];

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
  return fs.readFileSync(file, "utf8");
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
        .png()
        .toBuffer();
    } catch (e) {
      console.error(`Failed to rasterize at ${s}px:`, e.message || e);
    }
  }
  return buffers;
}

async function writeHicolorPngs(sharp, svgString, name) {
  for (const s of LINUX_HICOLOR_SIZES) {
    const dirPath = path.join(OUT_DIR, "hicolor", s.dir, "apps");
    ensureDir(dirPath);
    try {
      const buf = await sharp(Buffer.from(svgString))
        .resize({ width: s.px, height: s.px, fit: "contain" })
        .png()
        .toBuffer();
      const outName = `swboot-${name}.png`;
      const outPath = path.join(dirPath, outName);
      fs.writeFileSync(outPath, buf);
      console.log(`Wrote ${path.relative(cwd, outPath)}`);
    } catch (e) {
      console.error(
        `Failed to write hicolor PNG ${name} ${s.px}px:`,
        e.message || e
      );
    }
  }
}

async function writeIcoFromBuffers(pngToIco, buffers, name) {
  try {
    // prefer larger sizes first
    const prefer = [256, 128, 48, 32, 16];
    const ordered = prefer.map((s) => buffers[s]).filter(Boolean);
    if (ordered.length === 0) {
      console.warn("No PNG buffers available to build ICO for", name);
      return;
    }
    const icoBuf = await pngToIco(ordered);
    ensureDir(OUT_ICO_DIR);
    const icoPath = path.join(OUT_ICO_DIR, `${name}.ico`);
    fs.writeFileSync(icoPath, icoBuf);
    console.log(`Wrote ${path.relative(cwd, icoPath)}`);
  } catch (e) {
    console.error("Failed to create ICO for", name, e.message || e);
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

  // dynamic imports
  const sharp = await tryImport("sharp");
  const pngToIco = await tryImport("png-to-ico");

  // process generic first
  const genericCandidates = [
    path.join(RAW_ICON_DIR, "shortcut-generic.svg"),
    path.join(cwd, "static", "shortcut-generic.svg"),
  ];
  const genericPath = genericCandidates.find((p) => fs.existsSync(p));
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
      if (!sharp || !pngToIco)
        console.log(
          "sharp/png-to-ico not available; skipping generic.ico generation"
        );
      else {
        const buffers = [];
        for (const s of ICO_SIZES) {
          try {
            const buf = await sharp(Buffer.from(genericSvg))
              .resize({ width: s, height: s, fit: "contain" })
              .png()
              .toBuffer();
            buffers.push(buf);
          } catch (e) {
            console.error(
              `Failed to rasterize generic at ${s}px:`,
              e.message || e
            );
          }
        }
        if (buffers.length) {
          ensureDir(OUT_ICO_DIR);
          try {
            const icoBuf = await pngToIco(buffers);
            const icoPath = path.join(OUT_ICO_DIR, "generic.ico");
            fs.writeFileSync(icoPath, icoBuf);
            console.log(`Wrote ${path.relative(cwd, icoPath)}`);
          } catch (e) {
            console.error("Failed to create generic ICO:", e.message || e);
          }
        }
      }
    }
  }

  // iterate icons
  for (const f of files) {
    const name = path.basename(f, ".svg");
    const srcSvg = readSvg(path.join(STATIC_OS_ICONS, f));
    const sourceInner = stripSvgWrapper(srcSvg);
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

    if (!pngToIco) {
      console.log(
        "png-to-ico not installed; skipping .ico generation (install png-to-ico)"
      );
    } else if (!produceIco) {
      console.log(
        "Platform configured to not produce ICOs; skipping .ico generation"
      );
    } else {
      await writeIcoFromBuffers(pngToIco, producedBuffers, name);
    }
  }
}

run().catch((err) => {
  console.error(err);
  process.exit(1);
});
