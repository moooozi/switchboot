import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const script = fileURLToPath(new URL("../tools/prerender-icons.js", import.meta.url));
const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><rect width="128" height="128" fill="blue"/></svg>';

function fixture(t, { corrupt = false, generic = true, overlay = true, source = true } = {}) {
  const root = mkdtempSync(join(tmpdir(), "switchboot-icons-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "static/os-icons"), { recursive: true });
  mkdirSync(join(root, "src-tauri/icons-raw"), { recursive: true });
  if (source) writeFileSync(join(root, "static/os-icons/fixture.svg"), corrupt ? "invalid svg" : svg);
  if (generic) writeFileSync(join(root, "src-tauri/icons-raw/shortcut-generic.svg"), svg);
  if (overlay) writeFileSync(join(root, "src-tauri/icons-raw/shortcut-overlay.svg"), svg);
  return { root, result: spawnSync(process.execPath, [script], { cwd: root, encoding: "utf8", timeout: 30000 }) };
}

test("valid sources produce all required platform icons", (t) => {
  const { root, result } = fixture(t);
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  for (const name of ["fixture", "generic"]) {
    const outputs = process.platform === "linux"
      ? [32, 128, 512].map((size) => `src-tauri/icons-linux/hicolor/${size}x${size}/apps/swboot-${name}.png`)
      : [`src-tauri/resources/icons/ico/${name}.ico`];
    for (const output of outputs) assert.ok(statSync(join(root, output)).size > 0);
  }
});

for (const [name, options] of [["corrupt SVG", { corrupt: true }], ["missing generic", { generic: false }], ["missing overlay", { overlay: false }], ["no OS icons", { source: false }]]) {
  test(`icon generation fails for ${name}`, (t) => {
    const { result } = fixture(t, options);
    assert.ifError(result.error);
    assert.equal(result.status, 1, result.stdout);
  });
}
