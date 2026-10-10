import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";

const require = createRequire(import.meta.url);
const cli = join(dirname(require.resolve("@tauri-apps/cli")), "tauri.js");

test("Tauri generates updater signatures for unsigned artifacts", (t) => {
  const directory = mkdtempSync(join(tmpdir(), "switchboot-updater-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const key = join(directory, "fixture.key");
  const installer = join(directory, "fixture.exe");
  const run = (args, env = {}) => {
    const result = spawnSync(process.execPath, [cli, ...args], {
      encoding: "utf8", timeout: 30000, env: { ...process.env, ...env },
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
  };
  run(["signer", "generate", "--ci", "--password", "fixture", "--write-keys", key]);
  writeFileSync(installer, "fixture installer bytes");
  const env = {
    TAURI_SIGNING_PRIVATE_KEY: readFileSync(key, "utf8"),
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "fixture",
  };
  run(["signer", "sign", installer], env);
  const signature = readFileSync(installer + ".sig", "utf8").trim();
  assert.match(Buffer.from(signature, "base64").toString("utf8"), /untrusted comment:/);
  writeFileSync(installer, "changed fixture installer bytes");
  run(["signer", "sign", installer], env);
  assert.notEqual(readFileSync(installer + ".sig", "utf8").trim(), signature);
});
