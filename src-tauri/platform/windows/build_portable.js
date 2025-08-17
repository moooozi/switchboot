import fs from 'fs';
import fsPromises from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';
import crypto from 'crypto';
import { spawnSync } from 'child_process';

function randomSuffix(len = 6) {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  const bytes = crypto.randomBytes(len);
  let out = '';
  for (let i = 0; i < len; i++) {
    out += chars[bytes[i] % chars.length];
  }
  return out;
}

function escapeForNSIS(s) {
  if (!s) return '';
  return String(s).replace(/"/g, '\\"');
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

async function main() {
  const scriptDir = path.resolve(__dirname);
  const tauriPath = path.resolve(scriptDir, '..', '..', 'tauri.conf.json');
  if (!fs.existsSync(tauriPath)) {
    console.error('tauri.conf.json not found at', tauriPath);
    process.exit(1);
  }

  const tauri = JSON.parse(fs.readFileSync(tauriPath, 'utf8'));
  const product = tauri.productName || '';
  const fileTauriVersion = tauri.version || '';
  // Allow CI to override version via env var (workflow sets VERSION)
  const envVersion = process.env.VERSION || process.env.GITHUB_REF_NAME || process.env.GITHUB_SHA;
  const version = envVersion || fileTauriVersion;
  if (envVersion) console.log('Using version from environment:', envVersion);
  const publisher = (tauri.bundle && tauri.bundle.publisher) || '';
  const copyright = (tauri.bundle && tauri.bundle.copyright) || '';
  const identifier = tauri.identifier || '';

  // Keep original product version and set a fixed file version
  const productVersion = version;
  // normalize to X.X.X.X for VIProductVersion
  function normalizeToFour(versionStr) {
    if (!versionStr) return '0.0.0.0';
    const m = /^([0-9]+(?:\.[0-9]+){0,3})/.exec(versionStr);
    const base = m ? m[1] : '0.0.0.0';
    const parts = base.split('.').slice(0, 4);
    while (parts.length < 4) parts.push('0');
    return parts.join('.');
  }
  const viProductVersion = normalizeToFour(productVersion);
  // User requested FileVersion always '1.0'
  const fileVersion = '1.0';

  const metaLines = [];
  metaLines.push(`!define PRODUCTNAME "${escapeForNSIS(product)} Portable"`);
  // PRODUCT_VERSION is original, VI_PRODUCT_VERSION is 4-part numeric, FILE_VERSION is fixed to '1.0'
  metaLines.push(`!define PRODUCT_VERSION "${escapeForNSIS(productVersion)}"`);
  metaLines.push(`!define VI_PRODUCT_VERSION "${escapeForNSIS(viProductVersion)}"`);
  metaLines.push(`!define FILE_VERSION "${escapeForNSIS(fileVersion)}"`);
  metaLines.push(`!define PUBLISHER "${escapeForNSIS(publisher)}"`);
  metaLines.push(`!define COPYRIGHT "${escapeForNSIS(copyright)}"`);
  metaLines.push(`!define IDENTIFIER "${escapeForNSIS(identifier)}"`);

  const metaPath = path.join(scriptDir, 'metadata.nsh');
  await fsPromises.writeFile(metaPath, metaLines.join('\n'), { encoding: 'utf8' });
  console.log('Wrote', metaPath);
  console.log(await fsPromises.readFile(metaPath, 'utf8'));

  const suffix = randomSuffix(6);
  console.log('Using suffix:', suffix);

  const makensisPath = 'C:\\Program Files (x86)\\NSIS\\makensis.exe';
  if (!fs.existsSync(makensisPath)) {
    console.error('makensis.exe not found at', makensisPath);
    process.exit(1);
  }

  const args = [`/DSUFFIX=${suffix}`, 'nsis-portable.nsi'];
  console.log('Running makensis', makensisPath, args.join(' '));

  const res = spawnSync(makensisPath, args, { cwd: scriptDir, stdio: 'inherit' });
  let exitCode = 1;
  if (res.error) {
    console.error('Failed to run makensis:', res.error);
    exitCode = 1;
  } else {
    exitCode = res.status ?? 0;
  }

  // Attempt to delete metadata.nsh even if makensis failed
  try {
    await fsPromises.unlink(metaPath);
    console.log('Deleted', metaPath);
  } catch (err) {
    console.warn('Could not delete metadata.nsh:', err.message || err);
  }

  process.exit(exitCode);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
