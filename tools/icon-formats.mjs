import sharp from "sharp";

// Raster fallbacks for Linux desktops; scalable SVGs cover larger sizes.
export const linuxIconSizes = [32, 48, 64, 128, 256];

// Small DIB frames for Windows/NSIS, PNG compression for larger frames.
export async function encodeIco(buffers, sizes) {
  const frames = [];
  for (const size of sizes) {
    if (size > 48) {
      frames.push(buffers[size]);
      continue;
    }
    const pixels = await sharp(buffers[size]).ensureAlpha().raw().toBuffer();
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
  for (const [index, size] of sizes.entries()) {
    const entry = 6 + index * 16;
    header[entry] = header[entry + 1] = size === 256 ? 0 : size;
    header.writeUInt16LE(1, entry + 4);
    header.writeUInt16LE(32, entry + 6);
    header.writeUInt32LE(frames[index].length, entry + 8);
    header.writeUInt32LE(offset, entry + 12);
    offset += frames[index].length;
  }
  return Buffer.concat([header, ...frames]);
}
