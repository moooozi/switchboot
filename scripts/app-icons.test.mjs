import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import sharp from "sharp";

const icons = new URL("../crates/switchboot_core/icons/", import.meta.url);

test("Linux app icons cover the agreed desktop sizes with RGBA PNGs", async () => {
  for (const size of [32, 48, 64, 128, 256, 512]) {
    const metadata = await sharp(await readFile(new URL(`${size}x${size}.png`, icons))).metadata();
    assert.equal(metadata.format, "png");
    assert.equal(metadata.width, size);
    assert.equal(metadata.height, size);
    assert.equal(metadata.channels, 4);
    assert.equal(metadata.hasAlpha, true);
  }
});

test("Windows icon contains small BMP frames and PNG frames above 48px", async () => {
  const ico = await readFile(new URL("icon.ico", icons));
  const sizes = [16, 32, 48, 128, 256];
  assert.equal(ico.readUInt16LE(0), 0);
  assert.equal(ico.readUInt16LE(2), 1);
  assert.equal(ico.readUInt16LE(4), sizes.length);
  let offset = 6 + sizes.length * 16;
  for (const [index, size] of sizes.entries()) {
    const entry = 6 + index * 16;
    assert.equal(ico[entry] || 256, size);
    assert.equal(ico[entry + 1] || 256, size);
    assert.equal(ico.readUInt16LE(entry + 6), 32);
    assert.equal(ico.readUInt32LE(entry + 12), offset);
    const length = ico.readUInt32LE(entry + 8);
    const frame = ico.subarray(offset, offset + length);
    assert.equal(frame.length, length);
    if (size > 48) {
      const metadata = await sharp(frame).metadata();
      assert.equal(metadata.format, "png");
      assert.equal(metadata.width, size);
      assert.equal(metadata.height, size);
      assert.equal(metadata.hasAlpha, true);
    } else {
      assert.equal(frame.readUInt32LE(0), 40);
      assert.equal(frame.readInt32LE(4), size);
      assert.equal(frame.readInt32LE(8), size * 2);
      assert.equal(frame.readUInt16LE(14), 32);
      assert.equal(length, 40 + size * size * 4 + Math.ceil(size / 32) * 4 * size);
    }
    offset += length;
  }
  assert.equal(offset, ico.length);
});
