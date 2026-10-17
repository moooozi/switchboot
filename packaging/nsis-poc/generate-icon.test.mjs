import assert from "node:assert/strict";
import test from "node:test";
import sharp from "sharp";
import { createPreviewIcon, iconSizes } from "./generate-icon.mjs";

test("ICO has exact-size logo/title frames through 225%, rendered from SVG", async () => {
  const ico = await createPreviewIcon();
  assert.equal(ico.readUInt16LE(2), 1);
  assert.equal(ico.readUInt16LE(4), iconSizes.length);
  for (const size of [60, 75, 90, 105, 120, 135, 256]) assert.ok(iconSizes.includes(size));
  for (const [index, size] of iconSizes.entries()) {
    const entry = 6 + index * 16;
    assert.equal(ico[entry] || 256, size);
    const length = ico.readUInt32LE(entry + 8);
    const offset = ico.readUInt32LE(entry + 12);
    assert.ok(offset + length <= ico.length);
    if (size > 48) {
      const metadata = await sharp(ico.subarray(offset, offset + length)).metadata();
      assert.equal(metadata.width, size);
      assert.equal(metadata.height, size);
      assert.equal(metadata.hasAlpha, true);
    } else {
      assert.equal(ico.readInt32LE(offset + 4), size);
      assert.equal(ico.readInt32LE(offset + 8), size * 2);
    }
  }
});
