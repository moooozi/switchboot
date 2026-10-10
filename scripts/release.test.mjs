import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "switchboot-release-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "scripts"));
  mkdirSync(join(root, "src"), { recursive: true });
  writeFileSync(join(root, "src/lib.rs"), "// fixture\n");
  copyFileSync(new URL("./release.mjs", import.meta.url), join(root, "scripts/release.mjs"));
  writeFileSync(join(root, "package.json"), '{"version":"1.0.0"}\n');
  writeFileSync(join(root, "Cargo.toml"), '[package]\nname = "fixture"\nversion = "1.0.0"\n');
  writeFileSync(join(root, "Cargo.lock"), 'version = 4\n\n[[package]]\nname = "fixture"\nversion = "1.0.0"\n');
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
    const files = ["package.json", "Cargo.toml", "Cargo.lock"];
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

test("force-retag is rejected without modifying the original tag", (t) => {
  const { git, release } = fixture(t);
  git("tag", "v1.0.0");
  const original = git("rev-parse", "v1.0.0");
  const result = release("--force-retag", "--no-sign", "--no-push");
  assert.equal(result.status, 2);
  assert.match(result.stderr, /immutable/);
  assert.equal(git("rev-parse", "v1.0.0"), original);
  assert.equal(git("status", "--porcelain"), "");
});

test("existing release tags cannot be reused", (t) => {
  const { git, release } = fixture(t);
  git("tag", "v1.0.1");
  const result = release("patch", "--no-sign", "--no-push");
  assert.equal(result.status, 2);
  assert.equal(git("status", "--porcelain"), "");
});

for (const detached of [false, true]) {
  test(`release rejects ${detached ? "detached HEAD" : "a feature branch"}`, (t) => {
    const { git, release } = fixture(t);
    if (detached) git("checkout", "--detach");
    else git("checkout", "-b", "feature");
    const result = release("patch", "--dry-run", "--no-sign");
    assert.equal(result.status, 2);
    assert.match(result.stderr, /must be made from main/);
    assert.equal(git("status", "--porcelain"), "");
  });
}

test("local release synchronizes manifests, commits, and creates an annotated tag", (t) => {
  const { root, git, release } = fixture(t);
  const message = 'Release "quoted" & literal $HOME';
  const result = release("patch", "--no-push", "--no-sign", "-m", message);
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version, "1.0.1");
  for (const file of ["Cargo.toml", "Cargo.lock"]) {
    assert.match(readFileSync(join(root, file), "utf8"), /version = "1\.0\.1"/);
  }
  assert.equal(git("log", "-1", "--format=%s"), message);
  assert.equal(git("cat-file", "-t", "v1.0.1"), "tag");
  assert.equal(git("rev-parse", "v1.0.1^{commit}"), git("rev-parse", "HEAD"));
  assert.equal(git("status", "--porcelain"), "");
});

function remoteFixture(t) {
  const fixtureData = fixture(t);
  const remote = mkdtempSync(join(tmpdir(), "switchboot-remote-"));
  t.after(() => rmSync(remote, { recursive: true, force: true }));
  fixtureData.git("init", "--bare", "--initial-branch=main", remote);
  fixtureData.git("remote", "add", "origin", remote);
  fixtureData.git("push", "origin", "main");
  return { ...fixtureData, remote };
}

test("release atomically publishes its commit and annotated tag to a local remote", (t) => {
  const { git, release, remote } = remoteFixture(t);
  const result = release("patch", "--yes", "--no-sign");
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  const head = git("rev-parse", "HEAD");
  assert.equal(git("-C", remote, "rev-parse", "refs/heads/main"), head);
  assert.equal(git("-C", remote, "rev-parse", "v1.0.1^{commit}"), head);
  assert.equal(git("-C", remote, "cat-file", "-t", "v1.0.1"), "tag");
  assert.equal(git("status", "--porcelain"), "");
});

test("remote-only tag collision fails before changing files or refs", (t) => {
  const { git, release, remote } = remoteFixture(t);
  const head = git("rev-parse", "HEAD");
  git("-C", remote, "update-ref", "refs/tags/v1.0.1", head);
  const result = release("patch", "--yes", "--no-sign");
  assert.equal(result.status, 2);
  assert.match(result.stderr, /Remote tag.*already exists/);
  assert.equal(git("rev-parse", "HEAD"), head);
  assert.equal(git("status", "--porcelain"), "");
  assert.equal(git("tag", "--list"), "");
});

test("diverged remote main fails before changing files or refs", (t) => {
  const { git, release, remote } = remoteFixture(t);
  const head = git("rev-parse", "HEAD");
  const divergent = git("commit-tree", "HEAD^{tree}", "-m", "divergent root");
  git("push", "origin", `${divergent}:refs/heads/main`, "--force");
  const result = release("patch", "--yes", "--no-sign");
  assert.equal(result.status, 2);
  assert.match(result.stderr, /resolve any divergence/);
  assert.equal(git("rev-parse", "HEAD"), head);
  assert.equal(git("status", "--porcelain"), "");
  assert.equal(git("-C", remote, "rev-parse", "refs/heads/main"), divergent);
});

for (const problem of ["missing local entry", "duplicate local entry", "stale version", "Cargo error"]) {
  test(`release restores versions after ${problem}`, (t) => {
    const { root, git, release } = fixture(t);
    const lockPath = join(root, "Cargo.lock");
    const originalLock = readFileSync(lockPath, "utf8");
    if (problem === "missing local entry") writeFileSync(lockPath, originalLock.replace('name = "fixture"', 'name = "other"'));
    if (problem === "duplicate local entry") writeFileSync(lockPath, originalLock + originalLock.slice(originalLock.indexOf("[[package]]")));
    if (problem === "stale version") writeFileSync(lockPath, originalLock.replace('version = "1.0.0"', 'version = "0.9.0"'));
    if (problem === "Cargo error") writeFileSync(join(root, "Cargo.toml"), readFileSync(join(root, "Cargo.toml"), "utf8") + '\n[dependencies\n');
    git("add", ".");
    git("commit", "-m", problem);
    const head = git("rev-parse", "HEAD");
    const files = ["package.json", "Cargo.toml", "Cargo.lock"];
    const before = files.map((file) => readFileSync(join(root, file), "utf8"));
    const result = release("patch", "--no-push", "--no-sign");
    assert.equal(result.status, 1, result.stdout);
    assert.match(result.stderr, /original version files restored/);
    assert.deepEqual(files.map((file) => readFileSync(join(root, file), "utf8")), before);
    assert.equal(git("rev-parse", "HEAD"), head);
    assert.equal(git("status", "--porcelain"), "");
    assert.equal(git("tag", "--list"), "");
  });
}

test("commit failure restores versions while preserving unrelated staged user work", (t) => {
  const { root, git, release } = fixture(t);
  writeFileSync(join(root, "unrelated.txt"), "user work\n");
  git("add", "unrelated.txt");
  const hook = join(root, ".git/hooks/pre-commit");
  writeFileSync(hook, "#!/bin/sh\nexit 1\n");
  chmodSync(hook, 0o755);
  const head = git("rev-parse", "HEAD");
  const result = release("patch", "--no-push", "--no-sign", "--no-verify");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /original version files restored/);
  assert.equal(git("rev-parse", "HEAD"), head);
  assert.equal(JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version, "1.0.0");
  assert.equal(git("status", "--porcelain"), "A  unrelated.txt");
});

test("release commits only version files when unrelated changes are staged", (t) => {
  const { root, git, release } = fixture(t);
  writeFileSync(join(root, "unrelated.txt"), "user work\n");
  git("add", "unrelated.txt");
  const result = release("patch", "--no-push", "--no-sign", "--no-verify");
  assert.equal(result.status, 0, result.stderr);
  assert.doesNotMatch(git("ls-tree", "--name-only", "HEAD"), /unrelated/);
  assert.equal(git("status", "--porcelain"), "A  unrelated.txt");
});

test("tag signing failure retains the release commit and provides recovery guidance", (t) => {
  const { git, release } = fixture(t);
  git("config", "gpg.program", "nonexistent-switchboot-fixture-gpg");
  const head = git("rev-parse", "HEAD");
  const result = release("patch", "--no-push", "--sign");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Release commit retained/);
  assert.notEqual(git("rev-parse", "HEAD"), head);
  assert.equal(git("tag", "--list"), "");
  assert.equal(git("status", "--porcelain"), "");
});

test("dry-run still rejects invalid releases", (t) => {
  const { git, release } = fixture(t);
  const result = release("1.0.0", "--dry-run", "--no-sign");
  assert.equal(result.status, 2);
  assert.match(result.stderr, /must be greater/);
  assert.equal(git("status", "--porcelain"), "");
});
