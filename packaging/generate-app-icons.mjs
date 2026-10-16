import { mkdir, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import sharp from "sharp";
import { encodeIco, linuxIconSizes } from "../tools/icon-formats.mjs";

const output = new URL("../crates/switchboot_core/icons/", import.meta.url);
const source = new URL("../app-icon.svg", import.meta.url);
await mkdir(output, { recursive: true });
await rm(new URL("512x512.png", output), { force: true });
const icoSizes = [16, 32, 48, 128, 256];
const buffers = {};
for (const size of new Set([...linuxIconSizes, ...icoSizes])) {
  buffers[size] = await sharp(fileURLToPath(source))
    .resize(size, size).ensureAlpha().png().toBuffer();
  if (linuxIconSizes.includes(size)) {
    await writeFile(new URL(`${size}x${size}.png`, output), buffers[size]);
  }
}
await writeFile(new URL("icon.ico", output), await encodeIco(buffers, icoSizes));
