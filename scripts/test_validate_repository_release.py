"""Test the publication gate without accessing GitHub or deploying Pages."""

import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts/validate_repository_release.py"


class RepositoryReleaseTests(unittest.TestCase):
    def setUp(self):
        self.release = {
            "tagName": "v1.0.0",
            "isDraft": False,
            "isPrerelease": False,
            "publishedAt": "2026-10-10T12:00:00Z",
        }

    def validate(self, release=None, tag="v1.0.0", raw=None):
        return subprocess.run(
            [sys.executable, str(SCRIPT), tag],
            check=False,
            input=raw
            if raw is not None
            else json.dumps(self.release if release is None else release),
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_published_stable_release_is_accepted(self):
        result = self.validate()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_draft_and_prerelease_are_rejected(self):
        for field in ("isDraft", "isPrerelease"):
            with self.subTest(field=field):
                self.assertNotEqual(
                    self.validate({**self.release, field: True}).returncode, 0
                )

    def test_unpublished_release_is_rejected(self):
        for value in (None, "", "   ", False):
            with self.subTest(value=value):
                self.assertNotEqual(
                    self.validate({**self.release, "publishedAt": value}).returncode, 0
                )

    def test_missing_or_mistyped_metadata_fails_closed(self):
        for field in self.release:
            with self.subTest(missing=field):
                release = dict(self.release)
                del release[field]
                self.assertNotEqual(self.validate(release).returncode, 0)
        for field in ("isDraft", "isPrerelease"):
            for value in (0, None, "false"):
                with self.subTest(field=field, value=value):
                    self.assertNotEqual(
                        self.validate({**self.release, field: value}).returncode, 0
                    )

    def test_wrong_or_unstable_tag_is_rejected(self):
        self.assertNotEqual(
            self.validate({**self.release, "tagName": "v0.9.0"}).returncode, 0
        )
        for tag in ("1.0.0", "v1.0.0-beta.1", "v1.0.0\n"):
            with self.subTest(tag=tag):
                self.assertNotEqual(self.validate(tag=tag).returncode, 0)

    def test_invalid_json_or_metadata_shape_is_rejected(self):
        for raw in ("invalid", "null", "[]", "false"):
            with self.subTest(raw=raw):
                self.assertNotEqual(self.validate(raw=raw).returncode, 0)

    def test_release_creation_and_asset_upload_use_one_draft_only_step(self):
        workflow = (ROOT / ".github/workflows/release-pipeline.yml").read_text(
            encoding="utf-8"
        )
        release_steps = [
            step
            for step in workflow.split("      - name:")
            if "uses: softprops/action-gh-release@" in step
        ]
        self.assertEqual(len(release_steps), 1)
        step = release_steps[0]
        self.assertIn("          draft: true\n", step)
        self.assertIn("          prerelease: false\n", step)
        self.assertIn("          files: |\n", step)
        for asset in ("*.deb", "*.rpm", "*.exe", "*.asc", "update-sig/*.sig"):
            self.assertIn(f"            artifacts/{asset}\n", step)

    def test_workflow_gate_precedes_download_and_deployment_for_all_triggers(self):
        workflow = (ROOT / ".github/workflows/update-repository.yml").read_text(
            encoding="utf-8"
        )
        gate = workflow.index("      - name: Require a published stable release\n")
        download = workflow.index("      - name: Download release assets")
        deployment = workflow.index("      - name: Deploy to GitHub Pages")
        self.assertLess(gate, download)
        self.assertLess(download, deployment)
        step = workflow[
            gate : workflow.index("      - name: Check for GPG secrets", gate)
        ]
        self.assertNotIn("        if:", step)
        self.assertIn("--json tagName,isDraft,isPrerelease,publishedAt", step)
        self.assertIn(
            'python3 scripts/validate_repository_release.py "$TAG" < release-metadata.json',
            step,
        )
        self.assertIn("set -euo pipefail", step)


if __name__ == "__main__":
    unittest.main()
