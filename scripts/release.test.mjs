import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "switchboot-release-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "scripts"));
  mkdirSync(join(root, "src-tauri"));
  copyFileSync(new URL("./release.mjs", import.meta.url), join(root, "scripts/release.mjs"));
  writeFileSync(join(root, "package.json"), '{"version":"1.0.0"}\n');
  writeFileSync(join(root, "src-tauri/Cargo.toml"), '[package]\nname = "fixture"\nversion = "1.0.0"\n');
  writeFileSync(join(root, "src-tauri/Cargo.lock"), 'version = 4\n\n[[package]]\nname = "fixture"\nversion = "1.0.0"\n');
  const git = (...args) => execFileSync("git", ["--no-pager", ...args], {
    cwd: root, encoding: "utf8", env: { ...process.env, GIT_EDITOR: "true", GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "" },
  }).trim();
  git("init", "--initial-branch=main");
  git("config", "user.name", "Release Test");
  git("config", "user.email", "release-test@example.invalid");
  git("config", "commit.gpgsign", "false");
  git("add", ".");
  git("commit", "-m", "fixture");
  const release = (...args) => spawnSync(process.execPath, ["scripts/release.mjs", ...args], {
    cwd: root, encoding: "utf8", timeout: 30000,
    env: { ...process.env, GIT_EDITOR: "true", GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "" },
  });
  return { root, git, release };
}

for (const [argument, expected] of [["patch", "1.0.1"], ["minor", "1.1.0"], ["major", "2.0.0"], ["1.2.3", "1.2.3"]]) {
  test(`dry-run ${argument} previews ${expected} without side effects`, (t) => {
    const { root, git, release } = fixture(t);
    const files = ["package.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"];
    const before = files.map((file) => readFileSync(join(root, file), "utf8"));
    const head = git("rev-parse", "HEAD");
    const result = release(argument, "--dry-run", "--yes", "--no-sign");
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    assert.ok(result.stdout.includes(`Releasing 1.0.0 -> ${expected}`));
    assert.deepEqual(files.map((file) => readFileSync(join(root, file), "utf8")), before);
    assert.equal(git("rev-parse", "HEAD"), head);
    assert.equal(git("tag", "--list"), "");
    assert.equal(git("status", "--porcelain"), "");
  });
}

test("dry-run still rejects invalid releases", (t) => {
  const { git, release } = fixture(t);
  const result = release("1.0.0", "--dry-run", "--no-sign");
  assert.equal(result.status, 2);
  assert.match(result.stderr, /must be greater/);
  assert.equal(git("status", "--porcelain"), "");
});
