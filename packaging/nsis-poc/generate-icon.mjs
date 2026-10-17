import { mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import sharp from "sharp";
import { encodeIco } from "../../tools/icon-formats.mjs";

// Exact 60-DIP logo + 16/32-DIP window icons at 100/125/150/175/200/225%.
// The 256px source frame also covers intermediate and higher Windows scales.
export const iconSizes = [...new Set([
  ...[16, 32, 60].flatMap((dip) => [1, 1.25, 1.5, 1.75, 2, 2.25].map((scale) => dip * scale)),
  256,
])].sort((a, b) => a - b);

export async function createPreviewIcon() {
  const source = new URL("../../app-icon.svg", import.meta.url);
  const frames = {};
  for (const size of iconSizes) {
    frames[size] = await sharp(fileURLToPath(source))
      .resize(size, size).ensureAlpha().png().toBuffer();
  }
  return encodeIco(frames, iconSizes);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const output = new URL("../../artifacts/nsis-preview.ico", import.meta.url);
  await mkdir(new URL(".", output), { recursive: true });
  await writeFile(output, await createPreviewIcon());
  console.log(`Generated preview ICO: ${iconSizes.join(", ")}px`);
}
