import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("generate_latest_json.py").resolve()


class FeedTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.updates = self.root / "repo/updates"
        self.signature = self.updates / "1.2.3/Switchboot_x64-setup.exe.sig"
        self.signature.parent.mkdir(parents=True)
        self.signature.write_text("fixture-signature\n", encoding="utf-8")
        self.latest = self.updates / "latest.json"

    def run_cli(self, version="1.2.3", repository="example/switchboot"):
        return subprocess.run(
            [sys.executable, str(SCRIPT), version, repository],
            check=False,
            cwd=self.root,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_bootstrap_generates_feed_for_selected_repository(self):
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        data = json.loads(self.latest.read_text())
        self.assertEqual(data["version"], "1.2.3")
        self.assertEqual(
            data["platforms"]["windows-x86_64"]["signature"], "fixture-signature"
        )
        self.assertEqual(
            data["platforms"]["windows-x86_64"]["url"],
            "https://github.com/example/switchboot/releases/download/v1.2.3/Switchboot_x64-setup.exe",
        )
        self.assertFalse(self.latest.with_suffix(".json.tmp").exists())

    def test_forward_update_and_same_version_redeploy(self):
        for previous in ["1.2.2", "1.2.3"]:
            self.latest.write_text(json.dumps({"version": previous}))
            result = self.run_cli()
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_downgrade_preserves_existing_feed(self):
        original = '{"version":"2.0.0"}\n'
        self.latest.write_text(original)
        result = self.run_cli()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Refusing", result.stderr)
        self.assertEqual(self.latest.read_text(), original)

    def test_invalid_inputs_and_missing_signature_do_not_publish(self):
        for version, repository in [
            ("../bad", "example/app"),
            ("1.2.3", "bad"),
            ("1.2.4", "example/app"),
        ]:
            result = self.run_cli(version, repository)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(self.latest.exists())

    def test_empty_signature_preserves_existing_feed(self):
        original = '{"version":"1.2.2"}\n'
        self.latest.write_text(original)
        self.signature.write_text("\n")
        self.assertNotEqual(self.run_cli().returncode, 0)
        self.assertEqual(self.latest.read_text(), original)

    def test_corrupt_existing_metadata_is_not_overwritten(self):
        self.latest.write_text("not json")
        self.assertNotEqual(self.run_cli().returncode, 0)
        self.assertEqual(self.latest.read_text(), "not json")


if __name__ == "__main__":
    unittest.main()
