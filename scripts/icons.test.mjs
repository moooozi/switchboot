import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import sharp from "sharp";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const script = fileURLToPath(new URL("../tools/prerender-icons.js", import.meta.url));
const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128" stroke="red"><rect width="128" height="128" fill="blue"/></svg>';

function fixture(t, { corrupt = false, generic = true, overlay = true, source = true } = {}) {
  const root = mkdtempSync(join(tmpdir(), "switchboot-icons-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "static/os-icons"), { recursive: true });
  mkdirSync(join(root, "crates/switchboot_core/icons-raw"), { recursive: true });
  if (source) writeFileSync(join(root, "static/os-icons/fixture.svg"), corrupt ? "invalid svg" : svg);
  if (generic) writeFileSync(join(root, "crates/switchboot_core/icons-raw/shortcut-generic.svg"), svg);
  if (overlay) writeFileSync(join(root, "crates/switchboot_core/icons-raw/shortcut-overlay.svg"), svg);
  if (process.platform === "linux") {
    const stale = join(root, "crates/switchboot_core/icons-linux/hicolor/512x512/apps");
    mkdirSync(stale, { recursive: true });
    for (const name of ["fixture", "generic"]) writeFileSync(join(stale, `dev.zidane.switchboot-${name}.png`), "stale");
  }
  return { root, result: spawnSync(process.execPath, [script], { cwd: root, encoding: "utf8", timeout: 30000 }) };
}

test("valid sources produce all required platform icons", async (t) => {
  const { root, result } = fixture(t);
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  for (const name of ["fixture", "generic"]) {
    if (process.platform === "linux") {
      const scalable = readFileSync(join(root, `crates/switchboot_core/icons-linux/hicolor/scalable/apps/dev.zidane.switchboot-${name}.svg`));
      assert.equal((await sharp(scalable).metadata()).format, "svg");
      assert.ok(scalable.toString().includes("fill=\"blue\""));
      if (name === "fixture") {
        assert.ok(scalable.toString().includes("<g transform="), "Shortcut overlay is preserved");
        assert.ok(scalable.toString().includes("stroke=\"red\""), "Source root styles are preserved");
      }
      assert.equal(existsSync(join(root, `crates/switchboot_core/icons-linux/hicolor/512x512/apps/dev.zidane.switchboot-${name}.png`)), false);
    }
    const outputs = process.platform === "linux"
      ? [32, 48, 64, 128, 256].map((size) => `crates/switchboot_core/icons-linux/hicolor/${size}x${size}/apps/dev.zidane.switchboot-${name}.png`)
      : [`crates/switchboot_core/resources/icons/ico/${name}.ico`];
    for (const output of outputs) {
      assert.ok(statSync(join(root, output)).size > 0);
      if (!output.endsWith(".ico")) {
        const metadata = await sharp(readFileSync(join(root, output))).metadata();
        const size = Number(output.match(/hicolor\/(\d+)x/)[1]);
        assert.equal(metadata.width, size);
        assert.equal(metadata.height, size);
        continue;
      }
      const ico = readFileSync(join(root, output));
      assert.equal(ico.readUInt16LE(0), 0);
      assert.equal(ico.readUInt16LE(2), 1);
      const sizes = [256, 128, 48, 32, 16];
      assert.equal(ico.readUInt16LE(4), sizes.length);
      let offset = 6 + sizes.length * 16;
      for (const [i, size] of sizes.entries()) {
        const entry = 6 + i * 16;
        assert.equal(ico[entry] || 256, size);
        assert.equal(ico[entry + 1] || 256, size);
        assert.equal(ico[entry + 2], 0);
        assert.equal(ico[entry + 3], 0);
        assert.equal(ico.readUInt16LE(entry + 4), 1);
        assert.equal(ico.readUInt16LE(entry + 6), 32);
        const length = ico.readUInt32LE(entry + 8);
        assert.equal(ico.readUInt32LE(entry + 12), offset);
        const frame = ico.subarray(offset, offset + length);
        assert.equal(frame.length, length);
        if (size > 48) {
          assert.equal(frame.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
          const { info } = await sharp(frame).raw().toBuffer({ resolveWithObject: true });
          assert.equal(info.width, size);
          assert.equal(info.height, size);
          assert.equal(info.channels, 4);
        } else {
          assert.equal(frame.readUInt32LE(0), 40);
          assert.equal(frame.readInt32LE(4), size);
          assert.equal(frame.readInt32LE(8), size * 2);
          assert.equal(frame.readUInt16LE(14), 32);
          assert.equal(length, 40 + size * size * 4 + Math.ceil(size / 32) * 4 * size);
          // The opaque blue fixture must survive RGBA -> bottom-up BGRA conversion.
          if (name === "generic") assert.deepEqual([...frame.subarray(40, 44)], [255, 0, 0, 255]);
        }
        offset += length;
      }
      assert.equal(offset, ico.length);
      assert.ok(ico.length < 353118 / 2, "Large PNG frames keep ICOs smaller than all-DIB icons");
    }
  }
});

for (const [name, options] of [["corrupt SVG", { corrupt: true }], ["missing generic", { generic: false }], ["missing overlay", { overlay: false }], ["no OS icons", { source: false }]]) {
  test(`icon generation fails for ${name}`, (t) => {
    const { result } = fixture(t, options);
    assert.ifError(result.error);
    assert.equal(result.status, 1, result.stdout);
  });
}
