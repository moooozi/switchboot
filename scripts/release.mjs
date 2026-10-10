#!/usr/bin/env node
// scripts/release.mjs
// Single-source version bumper for Switchboot.
//
// Bumps the version in BOTH package.json and Cargo.toml, commits the
// change, and creates an annotated git tag `vX.Y.Z`. It does NOT push
// automatically â€” after the commit + tag are made locally you are asked to
// confirm before anything leaves your machine. Pushing the tag is what triggers
// the release pipeline (which builds and publishes a *draft* GitHub release).
//
// Usage:
//   pnpm release patch          # 0.2.7 -> 0.2.8
//   pnpm release minor          # 0.2.7 -> 0.3.0
//   pnpm release major          # 0.2.7 -> 1.0.0
//   pnpm release 1.2.3          # explicit version
//
// Flags:
//   --dry-run       preview the release without changing files, refs, or remotes
//   --no-push       commit & tag locally, never ask to push
//   -y, --yes       skip the push confirmation and push immediately
//   --sign          force a GPG-signed annotated tag
//   --no-sign       force an unsigned annotated tag
//   -m, --message MSG     custom commit + tag message (default: "Release vX.Y.Z")
//   --no-verify     skip the clean-working-tree check (not recommended)
// Release tags are immutable. Rerun CI for a failed build; use a new version
// when changing the source of an already tagged release.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PKG_PATH = resolve(ROOT, "package.json");
const CARGO_PATH = resolve(ROOT, "Cargo.toml");
const CARGO_LOCK_PATH = resolve(ROOT, "Cargo.lock");
const CARGO_DIR = ROOT;

const SEMVER = /^\d+\.\d+\.\d+$/;

function sh(command, args, cwd = ROOT) {
  return execFileSync(command, args, { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8", cwd }).trim();
}
function run(command, args, cwd = ROOT) {
  execFileSync(command, args, { stdio: "inherit", cwd, env: { ...process.env, GIT_EDITOR: "true" } });
}
function escapeReg(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function parseArgs(argv) {
  const opts = { dryRun: false, push: true, yes: false, verify: true, sign: null, message: null, bump: null, explicit: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-run") opts.dryRun = true;
    else if (a === "--no-push") opts.push = false;
    else if (a === "-y" || a === "--yes") opts.yes = true;
    else if (a === "--no-verify") opts.verify = false;
    else if (a === "--sign") opts.sign = true;
    else if (a === "--no-sign") opts.sign = false;
    else if (a === "--force-retag") die("Release tags are immutable. Rerun CI for the existing tag or release a new version.");
    else if (a === "-m" || a === "--message") opts.message = argv[++i];
    else if (!a.startsWith("-")) {
      if (a === "patch" || a === "minor" || a === "major") opts.bump = a;
      else if (SEMVER.test(a)) opts.explicit = a;
      else die(`Unknown argument: ${a}`);
    } else {
      die(`Unknown flag: ${a}`);
    }
  }
  return opts;
}

function die(msg, code = 2) {
  console.error(msg);
  process.exit(code);
}

function readPackageVersion() {
  return JSON.parse(readFileSync(PKG_PATH, "utf8")).version;
}
function readCargoVersion() {
  const txt = readFileSync(CARGO_PATH, "utf8");
  const m = txt.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error("Could not find [package] version in Cargo.toml");
  return m[1];
}
function readCargoName() {
  const txt = readFileSync(CARGO_PATH, "utf8");
  const m = txt.match(/^\[package\][\s\S]*?^name\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error("Could not find [package] name in Cargo.toml");
  return m[1];
}
// Patch only the unique local package entry; dependency versions remain untouched.
function syncCargoLock(version) {
  if (!existsSync(CARGO_LOCK_PATH)) throw new Error("Cargo.lock is required for a locked release.");
  const name = readCargoName();
  const lock = readFileSync(CARGO_LOCK_PATH, "utf8");
  const entries = [...lock.matchAll(/^\[\[package\]\]\r?\n[\s\S]*?(?=^\[\[package\]\]|$(?![\s\S]))/gm)];
  const local = entries.filter(([entry]) =>
    new RegExp(`^name = "${escapeReg(name)}"\\r?$`, "m").test(entry) && !/^source\s*=/m.test(entry));
  if (local.length !== 1) throw new Error(`Expected exactly one local ${name} entry in Cargo.lock.`);
  const [entry] = local[0];
  const versions = [...entry.matchAll(/^version = "([^"]+)"\r?$/gm)];
  if (versions.length !== 1 || versions[0][1] !== current) {
    throw new Error(`Local ${name} lock version must match ${current}.`);
  }
  const patched = entry.replace(/^(version = ")[^"]+("\r?)$/m, `$1${version}$2`);
  writeFileSync(CARGO_LOCK_PATH, lock.slice(0, local[0].index) + patched + lock.slice(local[0].index + entry.length));
  try {
    sh("cargo", ["--version"]);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    console.warn("Cargo unavailable: validated local lock entry directly; run cargo check --locked before publishing.");
    return;
  }
  // Real Cargo errors are fatal, not a reason to silently fall back.
  sh("cargo", ["metadata", "--offline", "--locked", "--no-deps", "--format-version", "1"], CARGO_DIR);
}
function bumpVersion(v, kind) {
  const [maj, min, pat] = v.split(".").map(Number);
  if (kind === "major") return `${maj + 1}.0.0`;
  if (kind === "minor") return `${maj}.${min + 1}.0`;
  return `${maj}.${min}.${pat + 1}`;
}
function compare(a, b) {
  const A = a.split(".").map(Number);
  const B = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if (A[i] < B[i]) return -1;
    if (A[i] > B[i]) return 1;
  }
  return 0;
}
function latestTagVersion() {
  try {
    const out = sh("git", ["tag", "--list", "--sort=-v:refname", "v[0-9]*.[0-9]*.[0-9]*"]);
    const first = out.split("\n").map((s) => s.trim()).find(Boolean);
    return first ? first.replace(/^v/, "") : null;
  } catch {
    return null;
  }
}
function isCleanTree() {
  return sh("git", ["--no-optional-locks", "status", "--porcelain"]) === "";
}
function gitSigningEnabled() {
  try {
    const gpg = sh("git", ["config", "--get", "commit.gpgsign"]) === "true";
    const key = sh("git", ["config", "--get", "user.signingkey"]);
    return gpg && key.length > 0;
  } catch {
    return false;
  }
}
function originUrl() {
  try {
    let url = sh("git", ["config", "--get", "remote.origin.url"]).replace(/\.git$/, "");
    url = url.replace(/^git@github\.com:/, "https://github.com/");
    return url;
  } catch {
    return null;
  }
}

const opts = parseArgs(process.argv.slice(2));

const pkgV = readPackageVersion();
const cargoV = readCargoVersion();
if (pkgV !== cargoV) {
  die(`Version mismatch: package.json=${pkgV}  Cargo.toml=${cargoV}\nFix the mismatch before releasing.`);
}

const current = pkgV;
let next;
if (opts.explicit) {
  next = opts.explicit;
} else if (opts.bump) {
  next = bumpVersion(current, opts.bump);

} else {
  die("Usage: pnpm release <patch|minor|major|X.Y.Z> [--dry-run] [--no-push] [-y] [--sign|--no-sign] [-m MSG]");
}

if (!SEMVER.test(next)) die(`Invalid version: ${next}`);

if (compare(next, current) <= 0) die(`New version ${next} must be greater than current ${current}.`);
const latest = latestTagVersion();
if (latest && compare(next, latest) <= 0) {
  die(`New version ${next} must be greater than the latest existing tag v${latest}.`);
}
const branch = sh("git", ["rev-parse", "--abbrev-ref", "HEAD"]);
if (branch !== "main") die(`Releases must be made from main, not '${branch}'.`);
if (sh("git", ["tag", "--list", `v${next}`])) die(`Tag v${next} already exists; release tags are immutable.`);

// Check the remote before changing files, not after committing and tagging.
if (opts.push && !opts.dryRun) {
  let remoteHead;
  try {
    const remoteTag = sh("git", ["ls-remote", "--tags", "origin", `refs/tags/v${next}`]);
    if (remoteTag) die(`Remote tag v${next} already exists; release tags are immutable.`);
    remoteHead = sh("git", ["ls-remote", "--heads", "origin", "refs/heads/main"]).split(/\s+/)[0];
    if (!remoteHead) die("origin/main does not exist; initialize it before releasing.");
    sh("git", ["merge-base", "--is-ancestor", remoteHead, "HEAD"]);
  } catch {
    die("Cannot verify origin/main. Fetch origin and resolve any divergence before releasing.");
  }
}

if (opts.verify && !isCleanTree()) {
  console.error("Working tree is not clean. Commit or stash your changes first (or pass --no-verify).");
  run("git", ["--no-optional-locks", "status", "--short"]);
  process.exit(1);
}

const sign = opts.sign ?? gitSigningEnabled();
const tag = `v${next}`;
// Conventional `chore(release)` commit so git-cliff excludes it from the notes.
const commitMessage = opts.message ?? `chore(release): ${tag}`;
const tagMessage = opts.message ?? `Release ${tag}`;

console.log(`Releasing ${current} -> ${next}`);
console.log(`  package.json : ${PKG_PATH}`);
console.log(`  Cargo.toml   : ${CARGO_PATH}`);
console.log(`  tag          : ${tag}   (GPG-signed: ${sign})`);

console.log(`  commit       : ${next === current ? "(none â€” version unchanged)" : commitMessage}`);

if (opts.dryRun) {
  console.log("\n--dry-run: preview only; no files, commits, tags, or remotes changed.");
  process.exit(0);
}

const addFiles = ["package.json", "Cargo.toml", "Cargo.lock"];
if (sh("git", ["--no-optional-locks", "status", "--porcelain", "--", ...addFiles])) {
  die("Version files must be clean, even with --no-verify.");
}
const originalHead = sh("git", ["rev-parse", "HEAD"]);
if (!existsSync(CARGO_LOCK_PATH)) die("Cargo.lock is required for a locked release.");
const originals = new Map(addFiles.map((file) => [file, readFileSync(resolve(ROOT, file))]));
try {
  // --- update files ---
  const pkgJson = JSON.parse(readFileSync(PKG_PATH, "utf8"));
  pkgJson.version = next;
  writeFileSync(PKG_PATH, JSON.stringify(pkgJson, null, 2) + "\n");

  let cargo = readFileSync(CARGO_PATH, "utf8");
  const cargoNext = cargo.replace(
    /^(\[package\][\s\S]*?^version\s*=\s*")([^"]+)(")/m,
    `$1${next}$3`,
  );
  if (cargo === cargoNext) throw new Error("Failed to update Cargo.toml [package] version.");
  writeFileSync(CARGO_PATH, cargoNext);

  // --- sync Cargo.lock to the new package version ---
  syncCargoLock(next);


  // --- commit the version files (package.json, Cargo.toml, Cargo.lock) ---
  run("git", ["add", "--", ...addFiles]);
  run("git", ["commit", "--only", "-m", commitMessage, "--", ...addFiles]);

  // --- create a new (signed) annotated tag; never replace an existing tag ---
  run("git", ["tag", sign ? "-s" : "-a", "-m", tagMessage, tag]);
} catch (error) {
  if (sh("git", ["rev-parse", "HEAD"]) === originalHead) {
    for (const [file, contents] of originals) writeFileSync(resolve(ROOT, file), contents);
    run("git", ["restore", "--staged", "--", ...addFiles]);
    console.error("Release preparation failed; original version files restored. No release commit was created.");
  } else {
    console.error(`Release commit retained. Inspect git status and HEAD, then create the missing ${tag} tag on that commit; do not bump again or replace an existing tag.`);
  }
  die(error.message, 1);
}

console.log(`\nCommitted '${commitMessage}' on '${branch}' and created tag ${tag}.`);
console.log("(Nothing has been pushed yet.)");

// --- push (only with explicit confirmation) ---
let pushNow = false;
if (!opts.push) {
  console.log("--no-push set: not pushing.");
} else if (opts.yes) {
  pushNow = true;
} else if (!input.isTTY) {
  console.log("Non-interactive shell: not pushing. Re-run with -y/--yes to push, or push manually (see below).");
} else {
  const rl = readline.createInterface({ input, output });
  let ans;
  do {
    ans = (await rl.question(`Push '${branch}' and ${tag} to origin now? (y/n) `)).trim().toLowerCase();
  } while (ans !== "y" && ans !== "n");
  rl.close();
  pushNow = ans === "y";
}

const tagPush = `git push --atomic origin ${branch} ${tag}`;

if (pushNow) {
  console.log(`\nPushing to origin...`);
  try {
    run("git", ["push", "--atomic", "origin", branch, tag]);
  } catch (error) {
    console.error(`Push failed; local commit and tag retained. Check remote refs before retrying: ${tagPush}`);
    die(error.message, 1);
  }

  const url = originUrl();
  console.log("\nDone. The pipeline will now build and open a DRAFT release:");
  if (url) console.log(`  ${url}/actions   (build progress)`);
  if (url) console.log(`  ${url}/releases  (review & publish the draft)`);
} else {
  console.log("\nNot pushed. When you are ready, run:");

  console.log(`  ${tagPush}`);
}
