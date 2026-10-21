"""Check trusted build selection, payload identity and provenance enforcement."""

import hashlib
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from validated_release import (
    find_build,
    rpm_signing_predicate,
    select_run,
    verify_payload,
    verify_provenance,
)

REPOSITORY = "moooozi/switchboot"
COMMIT = "a" * 40


def successful_run(**changes):
    return {
        "id": 12,
        "head_sha": COMMIT,
        "event": "push",
        "head_branch": "main",
        "path": ".github/workflows/pr-validation.yml",
        "status": "completed",
        "conclusion": "success",
        "repository": {"full_name": REPOSITORY},
        "head_repository": {"full_name": REPOSITORY},
        **changes,
    }


class ValidatedReleaseTests(unittest.TestCase):
    def test_only_successful_main_push_from_this_repository_is_eligible(self):
        good = successful_run()
        for changes in [
            {"head_sha": "b" * 40},
            {"event": "pull_request"},
            {"event": "workflow_dispatch"},
            {"head_branch": "feature"},
            {"path": ".github/workflows/unrelated.yml"},
            {"head_repository": {"full_name": "someone/fork"}},
            {"repository": {"full_name": "someone/fork"}},
            {"conclusion": "failure"},
            {"conclusion": "cancelled"},
        ]:
            with self.subTest(changes=changes):
                bad = successful_run(id=13, **changes)
                self.assertEqual(select_run([bad, good], REPOSITORY, COMMIT), good)
                with self.assertRaises((LookupError, ValueError)):
                    select_run([bad], REPOSITORY, COMMIT)

    def test_pending_validation_can_be_waited_on_but_failed_validation_stops(self):
        with self.assertRaises(LookupError):
            select_run([], REPOSITORY, COMMIT)
        with self.assertRaises(LookupError):
            select_run(
                [successful_run(status="in_progress", conclusion=None)],
                REPOSITORY,
                COMMIT,
            )
        with self.assertRaisesRegex(ValueError, "did not succeed"):
            select_run([successful_run(conclusion="failure")], REPOSITORY, COMMIT)

    def test_both_unexpired_platform_artifacts_are_required(self):
        artifacts = [
            {"name": f"switchboot-pr-{platform}-{COMMIT}", "expired": False}
            for platform in ("Linux", "Windows")
        ]
        with patch(
            "validated_release.api_get",
            side_effect=[
                {"workflow_runs": [successful_run()]},
                {"artifacts": artifacts},
            ],
        ):
            self.assertEqual(find_build(REPOSITORY, COMMIT), 12)
        for invalid in [
            artifacts[:1],
            [*artifacts, artifacts[0]],
            [
                artifacts[0],
                {**artifacts[1], "expired": True},
            ],
        ]:
            with (
                self.subTest(artifacts=invalid),
                patch(
                    "validated_release.api_get",
                    side_effect=[
                        {"workflow_runs": [successful_run()]},
                        {"artifacts": invalid},
                    ],
                ),
                self.assertRaisesRegex(ValueError, "missing or expired"),
            ):
                find_build(REPOSITORY, COMMIT)

    def fixture(self, platform):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        directory = Path(temp.name)
        metadata = {
            "commit": COMMIT,
            "layout": "iced",
            "platform": platform,
            "signed": False,
        }
        (directory / "build-info.json").write_text(
            json.dumps(metadata), encoding="utf-8"
        )
        names = (
            ["Switchboot_x64-setup.exe", "Switchboot_x64-portable.exe"]
            if platform == "windows"
            else [
                "switchboot_0.5.0_amd64.deb",
                "switchboot-0.5.0-1.x86_64.rpm",
                "Switchboot_linux_x64.tar.gz",
            ]
        )
        for name in names:
            (directory / name).write_bytes(b"harmless artifact")
        return directory

    def test_payloads_require_matching_commit_platform_version_and_file_set(self):
        for platform in ("linux", "windows"):
            with self.subTest(platform=platform):
                directory = self.fixture(platform)
                verify_payload(directory, COMMIT, "0.5.0", platform)
                with self.assertRaises(ValueError):
                    verify_payload(directory, "b" * 40, "0.5.0", platform)
                with self.assertRaises(ValueError):
                    verify_payload(
                        directory,
                        COMMIT,
                        "0.5.0",
                        "windows" if platform == "linux" else "linux",
                    )
                (directory / "unexpected.exe").write_bytes(b"untrusted")
                with self.assertRaises(ValueError):
                    verify_payload(directory, COMMIT, "0.5.0", platform)
        directory = self.fixture("linux")
        with self.assertRaises(ValueError):
            verify_payload(directory, COMMIT, "0.5.1", "linux")
        directory = self.fixture("windows")
        (directory / "Switchboot_x64-setup.exe").write_bytes(b"")
        with self.assertRaises(ValueError):
            verify_payload(directory, COMMIT, "0.5.0", "windows")

    def test_provenance_verification_pins_builder_commit_and_main_and_fails_closed(
        self,
    ):
        directory = self.fixture("windows")
        with patch("validated_release.subprocess.run") as run:
            verify_provenance(directory, REPOSITORY, COMMIT, "windows")
        self.assertEqual(run.call_count, 2)
        for call in run.call_args_list:
            command = call.args[0]
            self.assertEqual(command[command.index("--source-digest") + 1], COMMIT)
            self.assertEqual(
                command[command.index("--source-ref") + 1], "refs/heads/main"
            )
            self.assertEqual(
                command[command.index("--signer-workflow") + 1],
                f"{REPOSITORY}/.github/workflows/pr-validation.yml",
            )
            self.assertTrue(call.kwargs["check"])
        with (
            patch(
                "validated_release.subprocess.run",
                side_effect=subprocess.CalledProcessError(1, "gh"),
            ),
            self.assertRaises(subprocess.CalledProcessError),
        ):
            verify_provenance(directory, REPOSITORY, COMMIT, "windows")

    def test_signed_rpm_provenance_records_exact_unsigned_input_and_validation_run(
        self,
    ):
        directory = self.fixture("linux")
        rpm = directory / "switchboot-0.5.0-1.x86_64.rpm"
        with patch.dict(
            os.environ,
            {
                "GITHUB_WORKFLOW_REF": f"{REPOSITORY}/.github/workflows/linux-sign.yml@refs/tags/v0.5.0",
                "GITHUB_RUN_ID": "34",
                "GITHUB_RUN_ATTEMPT": "1",
            },
        ):
            predicate = rpm_signing_predicate(rpm, REPOSITORY, COMMIT, "12")
        dependency = predicate["buildDefinition"]["resolvedDependencies"][0]
        self.assertEqual(
            dependency["digest"]["sha256"], hashlib.sha256(rpm.read_bytes()).hexdigest()
        )
        self.assertIn("/actions/runs/12#", dependency["uri"])
        self.assertEqual(
            predicate["buildDefinition"]["externalParameters"]["commit"], COMMIT
        )
        self.assertIn(
            "/actions/runs/34/attempts/1",
            predicate["runDetails"]["metadata"]["invocationId"],
        )


if __name__ == "__main__":
    unittest.main()
