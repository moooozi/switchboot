"""Exercise composite signing scripts without production keys or certificates."""
import base64
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

    def test_invalid_key_fails_and_cleans_temporary_keyring(self):
        self.env["GPG_PRIVATE_KEY"] = "invalid fixture key"
        self.assertNotEqual(self.sign().returncode, 0)
        self.assertEqual(list(self.scratch.iterdir()), [])


POWERSHELL = shutil.which("pwsh") or shutil.which("powershell.exe")


@unittest.skipUnless(POWERSHELL, "PowerShell required")
class AuthenticodeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.payload = self.root / "payload.exe"
        self.payload.write_bytes(b"fixture")
        self.env = {**os.environ, "FILES_RAW": str(self.payload), "CERT_PFX": base64.b64encode(b"fixture").decode(),
                    "CERT_PASSWORD": "fixture", "RUNNER_TEMP": str(self.root)}

    def sign(self, failure=""):
        script = self.root / "sign.ps1"
        mock = "function signtool { $global:LASTEXITCODE = 0; if ($args[0] -eq '" + failure + "') { $global:LASTEXITCODE = 1 } }\n"
        script.write_text(mock + action_script("authenticode-sign"), encoding="utf-8")
        return subprocess.run([POWERSHELL, "-NoProfile", "-NonInteractive", "-File", str(script)],
                              env=self.env, capture_output=True, text=True, timeout=15)

    def test_success_removes_certificate(self):
        result = self.sign()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(list(self.root.glob("*.pfx")), [])

    def test_signing_and_verification_failures_remove_certificate(self):
        for failure in ["sign", "verify"]:
            result = self.sign(failure)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(list(self.root.glob("*.pfx")), [])

    def test_missing_input_fails_instead_of_skipping(self):
        self.env["FILES_RAW"] = str(self.root / "missing.exe")
        result = self.sign()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or empty", result.stderr)
        self.assertEqual(list(self.root.glob("*.pfx")), [])


if __name__ == "__main__":
    unittest.main()
