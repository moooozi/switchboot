import { spawnSync } from "child_process";
import crypto from "crypto";
import fs from "fs";
import fsPromises from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";


function escapeForNSIS(s) {
  if (!s) return "";
  return String(s).replace(/"/g, '\\"');
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

async function main() {
  const scriptDir = path.resolve(__dirname);
  const tauriPath = path.resolve(scriptDir, "..", "..", "tauri.conf.json");
  if (!fs.existsSync(tauriPath)) {
    console.error("tauri.conf.json not found at", tauriPath);
    process.exit(1);
  }

  const tauri = JSON.parse(fs.readFileSync(tauriPath, "utf8"));
  const product = tauri.productName || "";
  const fileTauriVersion = tauri.version || "";
  // Allow CI to override version via env var (workflow sets VERSION)
  const envVersion =
    process.env.VERSION ||
    process.env.GITHUB_REF_NAME ||
    process.env.GITHUB_SHA;
  const version = envVersion || fileTauriVersion;
  if (envVersion) console.log("Using version from environment:", envVersion);
  const publisher = (tauri.bundle && tauri.bundle.publisher) || "";
  const copyright = (tauri.bundle && tauri.bundle.copyright) || "";
  const identifier = tauri.identifier || "";

  // Keep original product version and set a fixed file version
  const productVersion = version;
  // normalize to X.X.X.X for VIProductVersion
  function normalizeToFour(versionStr) {
    if (!versionStr) return "0.0.0.0";
    const m = /^([0-9]+(?:\.[0-9]+){0,3})/.exec(versionStr);
    const base = m ? m[1] : "0.0.0.0";
    const parts = base.split(".").slice(0, 4);
    while (parts.length < 4) parts.push("0");
    return parts.join(".");
  }
  const viProductVersion = normalizeToFour(productVersion);
  const fileVersion = "1.0";

  const metaLines = [];
  metaLines.push(`!define PRODUCTNAME "${escapeForNSIS(product)} Portable"`);
  // PRODUCT_VERSION is original, VI_PRODUCT_VERSION is 4-part numeric, FILE_VERSION is fixed to '1.0'
  metaLines.push(`!define PRODUCT_VERSION "${escapeForNSIS(productVersion)}"`);
  metaLines.push(
    `!define VI_PRODUCT_VERSION "${escapeForNSIS(viProductVersion)}"`
  );
  metaLines.push(`!define FILE_VERSION "${escapeForNSIS(fileVersion)}"`);
  metaLines.push(`!define PUBLISHER "${escapeForNSIS(publisher)}"`);
  metaLines.push(`!define COPYRIGHT "${escapeForNSIS(copyright)}"`);
  metaLines.push(`!define IDENTIFIER "${escapeForNSIS(identifier)}"`);

  const metaPath = path.join(scriptDir, "metadata.nsh");
  await fsPromises.writeFile(metaPath, metaLines.join("\n"), {
    encoding: "utf8",
  });
  console.log("Wrote", metaPath);
  console.log(await fsPromises.readFile(metaPath, "utf8"));

  // Try to find makensis in PATH first. On Windows CI runners `which` may
  // return MSYS-style paths like `/c/Program Files (x86)/NSIS/makensis` which
  // cannot be directly spawned by Node. Prefer `where` on Windows and
  // normalize MSYS paths to native Windows paths.
  let makensisPath = "makensis";
  let foundInPath = false;
  try {
    if (process.platform === "win32") {
      const whereRes = spawnSync("where", ["makensis"]);
      if (whereRes.status === 0) {
        makensisPath = whereRes.stdout.toString().split(/\r?\n/)[0].trim();
        foundInPath = true;
      } else {
        // fallback to which if where isn't available (rare)
        const which = spawnSync("which", ["makensis"]);
        if (which.status === 0) {
          makensisPath = which.stdout.toString().trim();
          foundInPath = true;
        }
      }
    } else {
      const which = spawnSync("which", ["makensis"]);
      if (which.status === 0) {
        makensisPath = which.stdout.toString().trim();
        foundInPath = true;
      }
    }
  } catch {}

  // If we found an MSYS-style path on Windows (/c/...), convert it to a
  // proper Windows path and ensure it has an .exe extension so spawnSync can
  // locate it.
  if (foundInPath && process.platform === "win32") {
    let p = makensisPath;
    const msysMatch = p.match(/^\/([a-zA-Z])\/(.*)/);
    if (msysMatch) {
      p = msysMatch[1] + ":\\" + msysMatch[2].split("/").join("\\");
    }
    if (!/\.exe$/i.test(p)) p = p + ".exe";
    makensisPath = p;
  }

  // If not found in PATH, use the hardcoded Windows path
  if (!foundInPath) {
    makensisPath = "C:\\Program Files (x86)\\NSIS\\makensis.exe";
    if (!fs.existsSync(makensisPath)) {
      console.error("makensis.exe not found in PATH or at", makensisPath);
      process.exit(1);
    }
  }

  const args = ["nsis-portable.nsi"];
  console.log("Running makensis", makensisPath, args.join(" "));
  // If we're not on Windows, NSIS source references should point to the
  // cross-compiled release directory `target/x86_64-pc-windows-msvc/release`.
  const isWindows = process.platform === "win32";
  const nsisTemplatePath = path.join(scriptDir, "nsis-portable.nsi");
  let nsisPathToUse = nsisTemplatePath;
  let generatedNsisPath = null;
  if (!isWindows) {
    const template = await fsPromises.readFile(nsisTemplatePath, "utf8");
    // Replace occurrences of ..\..\target\release\ with
    // ..\..\target\x86_64-pc-windows-msvc\release\ (NSIS uses Windows-style backslashes)
    const replaced = template
      .split("..\\..\\target\\release\\")
      .join("..\\..\\target\\x86_64-pc-windows-msvc\\release\\");
    generatedNsisPath = path.join(
      scriptDir,
      `nsis-portable.generated.nsi`
    );
    await fsPromises.writeFile(generatedNsisPath, replaced, {
      encoding: "utf8",
    });
    nsisPathToUse = generatedNsisPath;
  }

  const argsToUse = [nsisPathToUse];
  console.log("Running makensis", makensisPath, argsToUse.join(" "));

  const res = spawnSync(makensisPath, argsToUse, {
    cwd: scriptDir,
    stdio: "inherit",
  });
  let exitCode = 1;
  if (res.error) {
    console.error("Failed to run makensis:", res.error);
    exitCode = 1;
  } else {
    exitCode = res.status ?? 0;
  }

  // Attempt to delete metadata.nsh even if makensis failed
  try {
    await fsPromises.unlink(metaPath);
    console.log("Deleted", metaPath);
  } catch (err) {
    console.warn("Could not delete metadata.nsh:", err.message || err);
  }

  // Remove generated NSIS script if we created one
  if (generatedNsisPath) {
    try {
      await fsPromises.unlink(generatedNsisPath);
      console.log("Deleted generated NSIS script", generatedNsisPath);
    } catch (err) {
      console.warn(
        "Could not delete generated NSIS script:",
        err.message || err
      );
    }
  }

  process.exit(exitCode);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
