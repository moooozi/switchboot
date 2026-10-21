"""Execute core metadata generation with a distinct application version."""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


@unittest.skipUnless(shutil.which("cargo"), "Cargo required")
class BuildVersionTests(unittest.TestCase):
    def test_core_metadata_tracks_root_manifest_and_rebuilds_on_change(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            core = root / "crates/core"
            (core / "src").mkdir(parents=True)
            manifest = root / "Cargo.toml"
            (core / "Cargo.toml").write_text(
                '[package]\nname = "metadata-fixture"\nversion = "0.1.0"\nedition = "2024"\n[workspace]\n[build-dependencies]\ntoml = "1"\n'
            )
            shutil.copyfile(ROOT / "crates/switchboot_core/build.rs", core / "build.rs")
            (core / "src/main.rs").write_text(
                'include!(concat!(env!("OUT_DIR"), "/app_build_info.rs"));\nfn main() { println!("{APP_VERSION} {APP_IDENTIFIER_VERSION}"); }\n'
            )
            env = {
                **os.environ,
                "CARGO_TARGET_DIR": str(ROOT / "target/metadata-tests"),
            }

            def build():
                before = manifest.read_bytes()
                result = subprocess.run(
                    [
                        "cargo",
                        "run",
                        "--offline",
                        "--quiet",
                        "--manifest-path",
                        str(core / "Cargo.toml"),
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                    env=env,
                    timeout=120,
                )
                self.assertEqual(manifest.read_bytes(), before)
                return result

            for version in ("1.2.3", "1.2.4"):
                manifest.write_text(f'[package]\nname = "app"\nversion = "{version}"\n')
                result = build()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    result.stdout.strip(), f"{version} dev.zidane.switchboot-{version}"
                )
            manifest.write_text('[package]\nname = "app"\n')
            result = build()
            self.assertNotEqual(result.returncode, 0)
