// Compare real Direct2D readback with build-time SVG rasterization, not ICO fallback.
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const directory = new URL("../../artifacts/nsis-svg-evaluation/", import.meta.url);
const source = fileURLToPath(new URL("../../app-icon.svg", import.meta.url));
const metrics = JSON.parse(await readFile(new URL("svg-render-metrics.json", directory), "utf8"));
const results = [];
const layers = [];
let y = 38;
const label = (text, width, height) => Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"><text x="8" y="18" fill="white" font-family="Segoe UI" font-size="13">${text}</text></svg>`);
layers.push({ input: label("Direct2D SVG", 200, 30), left: 0, top: 0 },
  { input: label("Build-time SVG reference", 220, 30), left: 220, top: 0 },
  { input: label("Difference (4x amplified)", 220, 30), left: 440, top: 0 });

for (const sample of metrics.samples) {
  const size = sample.pixels;
  const native = await readFile(new URL(`svg-${size}.bgra`, directory));
  const reference = await sharp(source).resize(size, size).ensureAlpha().raw().toBuffer();
  if (native.length !== size * size * 4) throw new Error("Invalid native bitmap dimensions");
  const actual = Buffer.alloc(size * size * 3);
  const expected = Buffer.alloc(actual.length);
  const difference = Buffer.alloc(actual.length);
  let total = 0;
  let maximum = 0;
  for (let pixel = 0; pixel < size * size; pixel++) {
    const a = native[pixel * 4 + 3];
    const b = reference[pixel * 4 + 3];
    for (let channel = 0; channel < 3; channel++) {
      const index = pixel * 3 + channel;
      actual[index] = Math.min(255, Math.round(native[pixel * 4 + 2 - channel] + 23 * (255 - a) / 255));
      expected[index] = Math.round(reference[pixel * 4 + channel] * b / 255 + 23 * (255 - b) / 255);
      const error = Math.abs(actual[index] - expected[index]);
      difference[index] = Math.min(255, error * 4);
      total += error;
      maximum = Math.max(maximum, error);
    }
  }
  const png = await sharp(actual, { raw: { width: size, height: size, channels: 3 } }).png().toBuffer();
  await writeFile(new URL(`svg-${size}.png`, directory), png);
  const mean = total / actual.length;
  results.push({ ...sample, mean_absolute_rgb_error: mean, maximum_rgb_error: maximum });
  if (size <= 135 && [60, 75, 90, 105, 120, 135].includes(size)) {
    layers.push({ input: label(`${size}px (${size / 60 * 100}%)`, 220, 26), left: 0, top: y });
    y += 26;
    layers.push({ input: png, left: 8, top: y },
      { input: await sharp(expected, { raw: { width: size, height: size, channels: 3 } }).png().toBuffer(), left: 228, top: y },
      { input: await sharp(difference, { raw: { width: size, height: size, channels: 3 } }).png().toBuffer(), left: 448, top: y });
    y += size + 12;
  }
}
await sharp({ create: { width: 660, height: y, channels: 3, background: "#171717" } })
  .composite(layers).png().toFile(fileURLToPath(new URL("comparison.png", directory)));
const report = { backend: metrics.backend, init_ms: metrics.init_ms, samples: results };
await writeFile(new URL("comparison.json", directory), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
// Broad tolerance permits different edge antialiasing but catches clipping/missing content.
if (results.some((result) => result.mean_absolute_rgb_error > 12)) {
  throw new Error("Native SVG differs substantially from the reference; inspect comparison.png");
}
