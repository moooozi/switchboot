"""Exercise GPG signing without production keys."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parent.parent


def action_script(name):
    action = (ROOT / ".github/actions" / name / "action.yml").read_text(encoding="utf-8")
    return textwrap.dedent(action.split("      run: |\n", 1)[1])


GPG = os.environ.get("SWITCHBOOT_TEST_GPG") or shutil.which("gpg")


@unittest.skipUnless(GPG and shutil.which("bash") and (os.name != "nt" or os.environ.get("SWITCHBOOT_TEST_GPG")),
                     "Native Linux GPG or explicitly configured Windows GPG required")
class GpgSigningTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "keys"
        self.home.mkdir()
        self.scratch = self.root / "scratch"
        self.scratch.mkdir()
        self.payload = self.root / "payload.bin"
        self.payload.write_bytes(b"release fixture")
        self.script = self.root / "sign.sh"
        self.script.write_text(action_script("gpg-sign"), encoding="utf-8")
        self.env = {**os.environ, "GNUPGHOME": str(self.home),
                    "PATH": str(Path(GPG).parent) + os.pathsep + os.environ["PATH"]}
        self.addCleanup(lambda: subprocess.run(["gpgconf", "--kill", "gpg-agent"], env=self.env,
                                               capture_output=True, timeout=10))
        result = subprocess.run([GPG, "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                                 "--quick-generate-key", "Release Fixture <fixture@example.invalid>",
                                 "ed25519", "sign", "0"], env=self.env, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        key = subprocess.check_output([GPG, "--batch", "--armor", "--export-secret-keys"], env=self.env, timeout=10)
        self.env.update(FILES_RAW=self.payload.as_posix(), GPG_PRIVATE_KEY=key.decode(),
                        GPG_PASSPHRASE="", TMPDIR=self.scratch.as_posix())

    def sign(self):
        return subprocess.run(["bash", self.script.as_posix()], env=self.env,
                              capture_output=True, text=True, timeout=30)

    def test_real_signature_verifies_and_temporary_keyring_is_removed(self):
        result = self.sign()
        self.assertEqual(result.returncode, 0, result.stderr)
        verify = subprocess.run([GPG, "--batch", "--verify", str(self.payload) + ".asc", str(self.payload)],
                                env=self.env, capture_output=True, timeout=10)
        self.assertEqual(verify.returncode, 0, verify.stderr)
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_missing_input_fails_instead_of_skipping(self):
        self.env["FILES_RAW"] = (self.root / "missing.bin").as_posix()
        result = self.sign()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or empty", result.stderr)
        self.assertFalse(Path(str(self.payload) + ".asc").exists())

    def prepare_release_assets(self):
        artifacts = self.root / "artifacts"
        artifacts.mkdir()
        packages = [artifacts / name for name in (
            "fixture.deb", "fixture.rpm", "Switchboot_x64-setup.exe", "Switchboot_x64-portable.exe")]
        for package in packages:
            package.write_bytes(b"release asset fixture")
        self.env["FILES_RAW"] = ";".join(package.as_posix() for package in packages)
        result = self.sign()
        self.assertEqual(result.returncode, 0, result.stderr)
        return artifacts, packages

    def verify_assets(self, artifacts):
        return subprocess.run(["bash", (ROOT / "scripts/verify-release-assets.sh").as_posix(), artifacts.as_posix()],
                              env=self.env, capture_output=True, text=True, timeout=30)

    def test_downloaded_release_assets_verify_with_configured_key(self):
        artifacts, _ = self.prepare_release_assets()
        result = self.verify_assets(artifacts)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_tampered_release_asset_is_rejected_and_keyring_is_cleaned(self):
        artifacts, packages = self.prepare_release_assets()
        packages[0].write_bytes(b"tampered asset fixture")
        result = self.verify_assets(artifacts)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_missing_signature_or_key_fails_closed(self):
        artifacts, packages = self.prepare_release_assets()
        Path(str(packages[0]) + ".asc").unlink()
        self.assertNotEqual(self.verify_assets(artifacts).returncode, 0)
        self.env["GPG_PRIVATE_KEY"] = ""
        self.assertNotEqual(self.verify_assets(artifacts).returncode, 0)
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_invalid_key_fails_and_cleans_temporary_keyring(self):
        self.env["GPG_PRIVATE_KEY"] = "invalid fixture key"
        self.assertNotEqual(self.sign().returncode, 0)
        self.assertEqual(list(self.scratch.iterdir()), [])



if __name__ == "__main__":
    unittest.main()
