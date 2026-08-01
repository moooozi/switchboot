#!/usr/bin/env node
// scripts/release.mjs
// Single-source version bumper for Switchboot.
//
// Bumps the version in BOTH package.json and src-tauri/Cargo.toml, commits the
// change, and creates an annotated git tag `vX.Y.Z`. It does NOT push
// automatically — after the commit + tag are made locally you are asked to
// confirm before anything leaves your machine. Pushing the tag is what triggers
// the release pipeline (which builds and publishes a *draft* GitHub release).
//
//   tauri.conf.json already reads its version from package.json
//   (`"version": "../package.json"`), so it needs no change.
//
// Usage:
//   pnpm release patch          # 0.2.7 -> 0.2.8
//   pnpm release minor          # 0.2.7 -> 0.3.0
//   pnpm release major          # 0.2.7 -> 1.0.0
//   pnpm release 1.2.3          # explicit version
//
// Flags:
//   --dry-run       write files but do NOT commit / tag / push
//   --no-push       commit & tag locally, never ask to push
//   -y, --yes       skip the push confirmation and push immediately
//   --sign          force a GPG-signed annotated tag
//   --no-sign       force an unsigned annotated tag
//   -m, --message MSG     custom commit + tag message (default: "Release vX.Y.Z")
//   --no-verify     skip the clean-working-tree check (not recommended)
//   --force-retag   re-release an existing version: overwrite its tag, bypass the
//                   "must be greater than current/latest tag" checks, and force-
//                   push the tag. With no version arg it retags the current
//                   version without making a new commit.
//
//   pnpm release --force-retag        # retag the current version
//   pnpm release --force-retag 1.2.3  # overwrite an existing v1.2.3 tag

import { execSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PKG_PATH = resolve(ROOT, "package.json");
const CARGO_PATH = resolve(ROOT, "src-tauri/Cargo.toml");
const CARGO_LOCK_PATH = resolve(ROOT, "src-tauri/Cargo.lock");
const CARGO_DIR = resolve(ROOT, "src-tauri");

const SEMVER = /^\d+\.\d+\.\d+$/;

function sh(cmd, cwd = ROOT) {
  return execSync(cmd, { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8", cwd }).trim();
}
function run(cmd, cwd = ROOT) {
  execSync(cmd, { stdio: "inherit", cwd });
}
function escapeReg(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function parseArgs(argv) {
  const opts = { dryRun: false, push: true, yes: false, verify: true, sign: null, message: null, bump: null, explicit: null, forceRetag: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-run") opts.dryRun = true;
    else if (a === "--no-push") opts.push = false;
    else if (a === "-y" || a === "--yes") opts.yes = true;
    else if (a === "--no-verify") opts.verify = false;
    else if (a === "--sign") opts.sign = true;
    else if (a === "--no-sign") opts.sign = false;
    else if (a === "--force-retag") opts.forceRetag = true;
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
// Keep src-tauri/Cargo.lock in sync with the new package version. Uses
// `cargo update -p <name> --precise <ver>` when cargo is available (touches only
// this crate's lock entry), else falls back to a direct patch of the lock.
function syncCargoLock(version) {
  if (!existsSync(CARGO_LOCK_PATH)) {
    console.log("  Cargo.lock not found — nothing to sync.");
    return;
  }
  const name = readCargoName();
  try {
    sh("cargo --version");
    run(`cargo update -p ${name} --precise ${version}`, CARGO_DIR);
    return;
  } catch {
    // cargo unavailable — patch the lock entry directly.
    let lock = readFileSync(CARGO_LOCK_PATH, "utf8");
    const re = new RegExp(`(name = "${escapeReg(name)}"\\nversion = ")([^"]+)(")`);
    const patched = lock.replace(re, `$1${version}$3`);
    if (patched === lock) {
      console.warn(`  Warning: could not update ${name} in Cargo.lock. Run 'cargo update -p ${name}' manually.`);
      return;
    }
    writeFileSync(CARGO_LOCK_PATH, patched);
  }
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
    const out = sh("git tag --list --sort=-v:refname 'v[0-9]*.[0-9]*.[0-9]*'");
    const first = out.split("\n").map((s) => s.trim()).find(Boolean);
    return first ? first.replace(/^v/, "") : null;
  } catch {
    return null;
  }
}
function isCleanTree() {
  return sh("git status --porcelain") === "";
}
function gitSigningEnabled() {
  try {
    const gpg = sh("git config --get commit.gpgsign") === "true";
    const key = sh("git config --get user.signingkey");
    return gpg && key.length > 0;
  } catch {
    return false;
  }
}
function originUrl() {
  try {
    let url = sh("git config --get remote.origin.url").replace(/\.git$/, "");
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
} else if (opts.forceRetag) {
  next = current;
} else {
  die("Usage: pnpm release <patch|minor|major|X.Y.Z> [--dry-run] [--no-push] [-y] [--sign|--no-sign] [-m MSG] [--force-retag]");
}

if (!SEMVER.test(next)) die(`Invalid version: ${next}`);

if (!opts.forceRetag) {
  if (compare(next, current) <= 0) die(`New version ${next} must be greater than current ${current}.`);

  const latest = latestTagVersion();
  if (latest && compare(next, latest) <= 0) {
    die(`New version ${next} must be greater than the latest existing tag v${latest}.`);
  }
}

if (opts.verify && !isCleanTree()) {
  console.error("Working tree is not clean. Commit or stash your changes first (or pass --no-verify).");
  run("git status --short");
  process.exit(1);
}

const sign = opts.sign ?? gitSigningEnabled();
const tag = `v${next}`;
// Conventional `chore(release)` commit so git-cliff excludes it from the notes.
const commitMessage = opts.message ?? `chore(release): ${tag}`;
const tagMessage = opts.message ?? `Release ${tag}`;

if (opts.forceRetag) {
  console.log(`Force-retagging ${current === next ? `current version ${next}` : `${current} -> ${next}`}`);
} else {
  console.log(`Releasing ${current} -> ${next}`);
}
console.log(`  package.json : ${PKG_PATH}`);
console.log(`  Cargo.toml   : ${CARGO_PATH}`);
console.log(`  tag          : ${tag}   (GPG-signed: ${sign})`);
if (opts.forceRetag) console.log(`  force-retag  : true (existing ${tag} tag will be overwritten)`);
console.log(`  commit       : ${next === current ? "(none — version unchanged)" : commitMessage}`);

// --- update files ---
const pkgJson = JSON.parse(readFileSync(PKG_PATH, "utf8"));
pkgJson.version = next;
writeFileSync(PKG_PATH, JSON.stringify(pkgJson, null, 2) + "\n");

let cargo = readFileSync(CARGO_PATH, "utf8");
const cargoNext = cargo.replace(
  /^(\[package\][\s\S]*?^version\s*=\s*")([^"]+)(")/m,
  `$1${next}$3`,
);
if (cargo === cargoNext) die("Failed to update Cargo.toml [package] version.");
writeFileSync(CARGO_PATH, cargoNext);

// --- sync Cargo.lock to the new package version ---
syncCargoLock(next);

if (opts.dryRun) {
  console.log("\n--dry-run: files written locally, skipping commit / tag / push.");
  console.log("Review the changes with: git diff");
  process.exit(0);
}

// --- commit the version files (package.json, Cargo.toml, Cargo.lock) ---
const addFiles = ["package.json", "src-tauri/Cargo.toml"];
if (existsSync(CARGO_LOCK_PATH)) addFiles.push("src-tauri/Cargo.lock");
run(`git add ${addFiles.join(" ")}`);

const needsCommit = next !== current;
let committed = false;
if (needsCommit) {
  run(`git commit -m ${JSON.stringify(commitMessage)}`);
  committed = true;
} else if (opts.forceRetag) {
  console.log("  Version unchanged — retagging without a new commit.");
}

// --- create (signed) annotated tag (force-move it when --force-retag) ---
const tagArgs = [opts.forceRetag && "-f", sign ? "-s" : "-a", "-m", JSON.stringify(tagMessage), tag]
  .filter(Boolean)
  .join(" ");
run(`git tag ${tagArgs}`);

const branch = sh("git rev-parse --abbrev-ref HEAD");
if (committed) {
  console.log(`\nCommitted '${commitMessage}' on '${branch}' and created tag ${tag}.`);
} else {
  console.log(`\nRecreated tag ${tag} on '${branch}' (no new commit).`);
}
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

const tagPush = opts.forceRetag ? `git push origin ${tag} --force` : `git push origin ${tag}`;

if (pushNow) {
  console.log(`\nPushing to origin...`);
  if (committed) run(`git push origin ${branch}`);
  run(tagPush);

  const url = originUrl();
  console.log("\nDone. The pipeline will now build and open a DRAFT release:");
  if (url) console.log(`  ${url}/actions   (build progress)`);
  if (url) console.log(`  ${url}/releases  (review & publish the draft)`);
} else {
  console.log("\nNot pushed. When you are ready, run:");
  if (committed) console.log(`  git push origin ${branch}`);
  console.log(`  ${tagPush}`);
}
