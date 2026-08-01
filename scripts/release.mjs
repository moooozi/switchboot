#!/usr/bin/env node
// scripts/release.mjs
// Single-source version bumper for Switchboot.
//
// Bumps the version in BOTH package.json and src-tauri/Cargo.toml, commits the
// change, creates an annotated git tag `vX.Y.Z`, and pushes both to origin.
// Pushing the tag is what triggers the release pipeline (which builds and
// publishes a *draft* GitHub release).
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
//   --dry-run     write files but do NOT commit / tag / push
//   --no-push     commit & tag locally, but do not push to origin
//   --sign        force a GPG-signed annotated tag
//   --no-sign     force an unsigned annotated tag
//   -m, --message MSG   custom commit + tag message (default: "Release vX.Y.Z")
//   --no-verify   skip the clean-working-tree check (not recommended)

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PKG_PATH = resolve(ROOT, "package.json");
const CARGO_PATH = resolve(ROOT, "src-tauri/Cargo.toml");

const SEMVER = /^\d+\.\d+\.\d+$/;

function sh(cmd) {
  return execSync(cmd, { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8", cwd: ROOT }).trim();
}
function run(cmd) {
  execSync(cmd, { stdio: "inherit", cwd: ROOT });
}

function parseArgs(argv) {
  const opts = { dryRun: false, push: true, verify: true, sign: null, message: null, bump: null, explicit: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-run") opts.dryRun = true;
    else if (a === "--no-push") opts.push = false;
    else if (a === "--no-verify") opts.verify = false;
    else if (a === "--sign") opts.sign = true;
    else if (a === "--no-sign") opts.sign = false;
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

if (!opts.bump && !opts.explicit) {
  die("Usage: pnpm release <patch|minor|major|X.Y.Z> [--dry-run] [--no-push] [--sign|--no-sign] [-m MSG]");
}

const pkgV = readPackageVersion();
const cargoV = readCargoVersion();
if (pkgV !== cargoV) {
  die(`Version mismatch: package.json=${pkgV}  Cargo.toml=${cargoV}\nFix the mismatch before releasing.`);
}

const current = pkgV;
const next = opts.explicit ?? bumpVersion(current, opts.bump);
if (!SEMVER.test(next)) die(`Invalid version: ${next}`);
if (compare(next, current) <= 0) die(`New version ${next} must be greater than current ${current}.`);

const latest = latestTagVersion();
if (latest && compare(next, latest) <= 0) {
  die(`New version ${next} must be greater than the latest existing tag v${latest}.`);
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

console.log(`Releasing ${current} -> ${next}`);
console.log(`  package.json : ${PKG_PATH}`);
console.log(`  Cargo.toml   : ${CARGO_PATH}`);
console.log(`  tag          : ${tag}   (GPG-signed: ${sign})`);
console.log(`  commit       : ${commitMessage}`);

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

if (opts.dryRun) {
  console.log("\n--dry-run: files written locally, skipping commit / tag / push.");
  console.log("Review the changes with: git diff");
  process.exit(0);
}

// --- commit the two version files only ---
run("git add package.json src-tauri/Cargo.toml");
run(`git commit -m ${JSON.stringify(commitMessage)}`);

// --- create (signed) annotated tag ---
if (sign) {
  run(`git tag -s -m ${JSON.stringify(tagMessage)} ${tag}`);
} else {
  run(`git tag -a -m ${JSON.stringify(tagMessage)} ${tag}`);
}

// --- push ---
if (opts.push) {
  const branch = sh("git rev-parse --abbrev-ref HEAD");
  console.log(`\nPushing branch '${branch}' and tag '${tag}' to origin...`);
  run(`git push origin ${branch}`);
  run(`git push origin ${tag}`);

  const url = originUrl();
  console.log("\nDone. The pipeline will now build and open a DRAFT release:");
  if (url) console.log(`  ${url}/actions   (build progress)`);
  if (url) console.log(`  ${url}/releases  (review & publish the draft)`);
} else {
  console.log(`\nDone locally. Trigger CI later with:  git push origin ${tag}`);
}
