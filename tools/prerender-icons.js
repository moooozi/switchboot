#!/usr/bin/env node
import fs from 'fs';
import path from 'path';

const base = path.resolve(process.cwd(), 'static', 'os-icons');
// render outputs should live in the tauri resources folder (not in static)
const outDir = path.resolve(process.cwd(), 'src-tauri', 'resources', 'icons');
const outSvg = path.join(outDir, 'svg');
const outIco = path.join(outDir, 'ico');

// Behavior: Linux -> emit composed SVGs; Windows -> emit ICOs (composed from rasterized sizes).
const isWin = process.platform === 'win32';
const isLinux = process.platform === 'linux';
const produceSvg = isLinux;
// Produce ICOs on Windows and other non-Linux platforms (keeps Windows behavior).
const produceIco = isWin || (!isWin && !isLinux);

// create necessary dirs
if (!fs.existsSync(outDir)) fs.mkdirSync(outDir, { recursive: true });
if (produceSvg && !fs.existsSync(outSvg)) fs.mkdirSync(outSvg, { recursive: true });
if (produceIco && !fs.existsSync(outIco)) fs.mkdirSync(outIco, { recursive: true });

// helper files may live under src-tauri/resources/icons/helper now (not in static)
const helperDir = path.resolve(process.cwd(), 'src-tauri', 'resources', 'icons', 'helper');
let overlayPath = path.join(helperDir, 'shortcut-overlay.svg');
if (!fs.existsSync(overlayPath)){
  // fallback to static location used by the frontend
  overlayPath = path.join(base, '..', 'shortcut-overlay.svg');
}
if (!fs.existsSync(overlayPath)){
  console.error('Missing shortcut-overlay.svg in helper or static/os-icons');
  process.exit(1);
}
const overlay = fs.readFileSync(overlayPath, 'utf8');
const files = fs.readdirSync(base).filter(f => f.endsWith('.svg') && f !== 'shortcut-overlay.svg');

// sizes to produce; include multiple sizes so we can compose a multi-size ICO
const sizes = [16, 32, 48, 128, 256];

async function combine(){
  // attempt to load sharp and png-to-ico dynamically (only required when building ICOs)
  let sharp = null;
  let pngToIco = null;
  try{ sharp = (await import('sharp')).default; }catch(e){}
  try{ pngToIco = (await import('png-to-ico')).default; }catch(e){}

  for(const f of files){
    const name = path.basename(f, '.svg');
    const svg = fs.readFileSync(path.join(base, f), 'utf8');
    const inner = svg.replace(/<\?xml[^>]*>/,'').replace(/<svg[^>]*>/,'').replace(/<\/svg>/,'');
    const overlayInner = overlay.replace(/<\?xml[^>]*>/,'').replace(/<svg[^>]*>/,'').replace(/<\/svg>/,'');

    // determine source SVG natural size and origin: prefer viewBox if present, otherwise width/height
    let srcMinX = 0, srcMinY = 0, srcW = null, srcH = null;
    const vbMatch = svg.match(/\bviewBox\s*=\s*"([^"]+)"/i);
    if (vbMatch){
      const parts = vbMatch[1].trim().split(/\s+/);
      if (parts.length >= 4){
        srcMinX = parseFloat(parts[0]);
        srcMinY = parseFloat(parts[1]);
        srcW = parseFloat(parts[2]);
        srcH = parseFloat(parts[3]);
      }
    }
    if (srcW == null || srcH == null){
      const wM = svg.match(/\bwidth\s*=\s*"([0-9\.]+)\s*(px)?"/i);
      const hM = svg.match(/\bheight\s*=\s*"([0-9\.]+)\s*(px)?"/i);
      if (wM) srcW = parseFloat(wM[1]);
      if (hM) srcH = parseFloat(hM[1]);
      // when only width/height present, origin remains 0,0
    }
    if (!srcW || !srcH){
      // fallback assume 128x128 natural
      srcW = srcW || 128;
      srcH = srcH || 128;
    }

    // scale and center source into canvas, preserving aspect ratio
    const canvasSize = 128;
    const srcMax = Math.max(srcW, srcH);
    const sourceScale = canvasSize / srcMax;

    // overlay detection: auto-detect overlay's natural size using viewBox or width/height attributes
    let overlayW = null, overlayH = null;
    const vb = overlay.match(/viewBox\s*=\s*"([^"]+)"/i);
    if (vb){
      const parts = vb[1].trim().split(/\s+,?\s+/);
      if (parts.length === 4){
        overlayW = parseFloat(parts[2]);
        overlayH = parseFloat(parts[3]);
      }
    }
    if (overlayW == null || overlayH == null){
      const wMatch = overlay.match(/width\s*=\s*"([^"]+)"/i);
      const hMatch = overlay.match(/height\s*=\s*"([^"]+)"/i);
      if (wMatch) overlayW = parseFloat(wMatch[1]);
      if (hMatch) overlayH = parseFloat(hMatch[1]);
    }
    const overlayNatural = Math.max(overlayW || 32, overlayH || 32);

  // Make the overlay larger and add padding so it doesn't sit flush against the corner.
  // Use separate X and Y padding so vertical inset can be smaller than horizontal.
  const overlayTargetSize = 48; // desired final overlay size in px inside canvas (larger than before)
  const overlayPaddingX = 4; // horizontal padding between overlay and canvas edge
  const overlayPaddingY = -10; // vertical padding between overlay and canvas edge (smaller than X)
  const overlayTranslateX = canvasSize - overlayTargetSize - overlayPaddingX;
  const overlayTranslateY = canvasSize - overlayTargetSize - overlayPaddingY;
  const overlayScale = overlayTargetSize / overlayNatural;
  const tx_overlay = overlayTranslateX / overlayScale;
  const ty_overlay = overlayTranslateY / overlayScale;

    // inject xlink namespace if needed
    const needsXlink = /xlink:/i.test(inner) || /xlink:/i.test(overlayInner);
    const xmlnsXlink = needsXlink ? ' xmlns:xlink="http://www.w3.org/1999/xlink"' : '';

  // debug: log detected sizes
  console.log(`Detected source size for ${name}: ${srcW}x${srcH}, sourceScale=${sourceScale}`);
  console.log(`Detected overlay natural size: ${overlayNatural}`);

  // compute final size of the source after scaling
  const finalSrcW = Math.round(srcW * sourceScale);
  const finalSrcH = Math.round(srcH * sourceScale);
  const srcX = Math.round((canvasSize - finalSrcW) / 2);
  const srcY = Math.round((canvasSize - finalSrcH) / 2);

  // overlay size and position
  const overlaySizePx = overlayTargetSize; // e.g. 48
  // position overlay inset by respective padding so it doesn't touch the canvas edge
  const overlayX = canvasSize - overlaySizePx - overlayPaddingX;
  const overlayY = canvasSize - overlaySizePx - overlayPaddingY;

  // embed source and overlay; use a <g> with translate+scale for overlay so padding is applied reliably
  const combined = `<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg"${xmlnsXlink} width=\"${canvasSize}\" height=\"${canvasSize}\" viewBox=\"0 0 ${canvasSize} ${canvasSize}\">\n  <svg x=\"${srcX}\" y=\"${srcY}\" width=\"${finalSrcW}\" height=\"${finalSrcH}\" viewBox=\"${srcMinX} ${srcMinY} ${srcW} ${srcH}\" preserveAspectRatio=\"xMidYMid meet\">\n    ${inner}\n  </svg>\n  <g transform=\"translate(${overlayX}, ${overlayY}) scale(${overlaySizePx/overlayNatural})\">\n    ${overlayInner}\n  </g>\n</svg>`;

    // Before processing individual icons, also emit the generic shortcut assets (no overlay).
    // The generic source lives at static/shortcut-generic.svg
    // generic helper source may live in helper dir or in static
    let genericSrcPath = path.join(helperDir, 'shortcut-generic.svg');
    if (!fs.existsSync(genericSrcPath)){
      genericSrcPath = path.join(process.cwd(), 'static', 'shortcut-generic.svg');
    }
    if (fs.existsSync(genericSrcPath)){
      const genericSvg = fs.readFileSync(genericSrcPath, 'utf8');
      if (produceSvg){
        try{
          const genericOut = path.join(outSvg, 'generic.svg');
          fs.writeFileSync(genericOut, genericSvg);
          console.log(`Wrote ${path.relative(process.cwd(), genericOut)}`);
        }catch(e){
          console.error('Failed to write generic SVG:', e.message || e);
        }
      }

      if (produceIco){
        if (!sharp || !pngToIco){
          console.log('sharp/png-to-ico not available; skipping generic.ico generation');
        } else {
          try{
            const buffers = [];
            for (const s of sizes){
              try{
                const buf = await sharp(Buffer.from(genericSvg)).resize({ width: s, height: s, fit: 'contain' }).png().toBuffer();
                buffers.push(buf);
              }catch(e){
                console.error(`Failed to rasterize generic at ${s}px:`, e.message || e);
              }
            }
            if (buffers.length){
              const icoBuf = await pngToIco(buffers);
              const icoPath = path.join(outIco, 'generic.ico');
              fs.writeFileSync(icoPath, icoBuf);
              console.log(`Wrote ${path.relative(process.cwd(), icoPath)}`);
            }
          }catch(e){
            console.error('Failed to create generic ICO:', e.message || e);
          }
        }
      }
    }

    // If Linux: write the composed SVG and move on.
    if (produceSvg){
      try{
        const svgPath = path.join(outSvg, `${name}.svg`);
        fs.writeFileSync(svgPath, combined);
        console.log(`Wrote ${path.relative(process.cwd(), svgPath)}`);
      }catch(e){
        console.error(`Failed to write SVG for ${name}:`, e.message || e);
      }
    }

    // For ICO generation we need raster buffers; produce them in-memory using sharp (no PNG files written to disk).
    const producedPngBuffers = {}; // size -> buffer
    if (produceIco){
      if (!sharp){
        console.error('sharp is required to emit PNG/ICO. Install it with: pnpm add -D sharp');
        process.exit(1);
      }
      for(const s of sizes){
        try{
          const buf = await sharp(Buffer.from(combined)).resize({ width: s, height: s, fit: 'contain' }).png().toBuffer();
          producedPngBuffers[s] = buf;
          console.log(`Produced in-memory PNG ${name}-${s}`);
        }catch(e){
          console.error(`Failed to produce PNG ${name}-${s}:`, e.message || e);
        }
      }
    }

    // Produce ICO for every icon (useful when Windows shortcuts reference non-Windows icons)
    if (!pngToIco){
      console.log('png-to-ico not installed; skipping .ico generation (install png-to-ico)');
    } else if (!produceIco){
      console.log('Platform configured to not produce ICOs; skipping .ico generation');
    } else {
      try{
        // prefer 256, 128, 48, 32, 16 order
        const orderSizes = [256, 128, 48, 32, 16];
        const buffers = [];
        for (const s of orderSizes){
          if (producedPngBuffers[s]) buffers.push(producedPngBuffers[s]);
          else {
            // if PNGs were written to disk instead, read them back
            // Historically we wrote PNGs to disk, but current flow keeps them in-memory.
            const pngPath = path.join(outDir, 'png', `${name}-${s}.png`);
            if (fs.existsSync(pngPath)) buffers.push(fs.readFileSync(pngPath));
          }
        }
        if (buffers.length === 0){
          console.warn('No PNG buffers available to build ICO for', name);
        }else{
          const icoBuf = await pngToIco(buffers);
          const icoPath = path.join(outIco, `${name}.ico`);
          fs.writeFileSync(icoPath, icoBuf);
          console.log(`Wrote ${path.relative(process.cwd(), icoPath)}`);
        }
      }catch(e){
        console.error('Failed to create ICO for', name, e.message || e);
      }
    }
  }
}

combine().catch(err => { console.error(err); process.exit(1); });
