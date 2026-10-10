"""Test actual artifact-selection script with mocked package metadata tools."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parent / "collect-linux-artifacts.sh"
MOCKS = r'''
function dpkg-deb {
  read -r version arch < "$2"
  if [ "$3" = Version ]; then printf '%s' "$version"; else printf '%s' "$arch"; fi
}
function rpm {
  cat "$4"
}
source "$SCRIPT" "$RELEASE_VERSION"
'''


@unittest.skipUnless(shutil.which("bash"), "Bash required")
class LinuxArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bundle = self.root / "src-tauri/target/release/bundle"
        (self.bundle / "deb").mkdir(parents=True)
        (self.bundle / "rpm").mkdir()

    def package(self, name, version="1.0.0", arch=None):
        kind = name.rsplit(".", 1)[1]
        (self.bundle / kind / name).write_text(
            f"{version} {arch or ('amd64' if kind == 'deb' else 'x86_64')}\n", encoding="utf-8", newline="\n")

    def collect(self):
        return subprocess.run(["bash", "-c", MOCKS], cwd=self.root,
                              env={**os.environ, "SCRIPT": SCRIPT.as_posix(), "RELEASE_VERSION": "1.0.0",
                                   "GITHUB_OUTPUT": (self.root / "outputs").as_posix()},
                              capture_output=True, text=True, timeout=10)

    def test_exact_version_is_selected_instead_of_stale_cache(self):
        for kind in ("deb", "rpm"):
            self.package(f"current.{kind}")
            self.package(f"stale.{kind}", "0.9.0")
            self.package(f"other-arch.{kind}", arch="arm64")
        result = self.collect()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(sorted(p.name for p in (self.root / "artifacts").iterdir()), ["current.deb", "current.rpm"])
        self.assertIn("deb_artifact=current.deb", (self.root / "outputs").read_text())

    def test_missing_either_package_fails(self):
        self.package("current.deb")
        self.assertNotEqual(self.collect().returncode, 0)
        self.assertFalse((self.root / "artifacts").exists())

    def test_ambiguous_matching_packages_fail(self):
        self.package("current.deb")
        self.package("duplicate.deb")
        self.package("current.rpm")
        self.assertNotEqual(self.collect().returncode, 0)
        self.assertFalse((self.root / "artifacts").exists())


if __name__ == "__main__":
    unittest.main()
