"""Run the real build-version validator in a disposable minimal Cargo project."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


@unittest.skipUnless(shutil.which("cargo"), "Cargo required")
class BuildVersionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.crate = self.root / "src-tauri"
        (self.crate / "src").mkdir(parents=True)
        (self.crate / "src/lib.rs").write_text("", encoding="utf-8")
        self.manifest = self.crate / "Cargo.toml"
        self.manifest.write_text('[package]\nname = "build-version-fixture"\nversion = "1.0.0"\nedition = "2024"\n[build-dependencies]\nserde_json = "1"\n', encoding="utf-8")
        source = (ROOT / "src-tauri/build.rs").read_text(encoding="utf-8")
        validator = source[source.index("fn validate_version()"):source.index("/// Generate app_build_info")]
        (self.crate / "build.rs").write_text(
            "use std::{fs, path::Path};\nfn main() { validate_version().unwrap(); }\n" + validator,
            encoding="utf-8")

    def build(self, version):
        (self.root / "package.json").write_text(json.dumps({"version": version}), encoding="utf-8")
        before = self.manifest.read_bytes()
        result = subprocess.run(["cargo", "build", "--offline", "--manifest-path", str(self.manifest)],
                                env={**os.environ, "CARGO_TARGET_DIR": str(self.root / "target")},
                                capture_output=True, text=True, timeout=120)
        self.assertEqual(self.manifest.read_bytes(), before)
        return result

    def test_matching_versions_build_without_mutating_manifest(self):
        result = self.build("1.0.0")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_mismatched_versions_fail_without_mutating_manifest(self):
        result = self.build("2.0.0")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("differs from Cargo package version", result.stderr)


if __name__ == "__main__":
    unittest.main()
