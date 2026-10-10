import { mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const output = new URL("../crates/switchboot_core/icons/", import.meta.url);
const source = new URL("../app-icon.svg", import.meta.url);
await mkdir(output, { recursive: true });
await sharp(fileURLToPath(source))
  .resize(128, 128).png().toFile(fileURLToPath(new URL("128x128.png", output)));

// BMP frames work with NSIS and Windows executable resource tools.
const sizes = [16, 32, 48, 128];
const frames = [];
for (const size of sizes) {
  const pixels = await sharp(fileURLToPath(source))
    .resize(size, size).ensureAlpha().raw().toBuffer();
  const maskStride = Math.ceil(size / 32) * 4;
  const frame = Buffer.alloc(40 + size * size * 4 + maskStride * size);
  frame.writeUInt32LE(40, 0);
  frame.writeInt32LE(size, 4);
  frame.writeInt32LE(size * 2, 8);
  frame.writeUInt16LE(1, 12);
  frame.writeUInt16LE(32, 14);
  frame.writeUInt32LE(size * size * 4, 20);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const src = (y * size + x) * 4;
      const dst = 40 + ((size - y - 1) * size + x) * 4;
      frame[dst] = pixels[src + 2];
      frame[dst + 1] = pixels[src + 1];
      frame[dst + 2] = pixels[src];
      frame[dst + 3] = pixels[src + 3];
      if (pixels[src + 3] === 0) {
        frame[40 + size * size * 4 + (size - y - 1) * maskStride + (x >> 3)] |= 0x80 >> (x % 8);
      }
    }
  }
  frames.push(frame);
}
const header = Buffer.alloc(6 + sizes.length * 16);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(sizes.length, 4);
let offset = header.length;
for (let index = 0; index < sizes.length; index++) {
  const entry = 6 + index * 16;
  header[entry] = header[entry + 1] = sizes[index];
  header.writeUInt16LE(1, entry + 4);
  header.writeUInt16LE(32, entry + 6);
  header.writeUInt32LE(frames[index].length, entry + 8);
  header.writeUInt32LE(offset, entry + 12);
  offset += frames[index].length;
}
await writeFile(new URL("icon.ico", output), Buffer.concat([header, ...frames]));
