"""Exercise release preparation in disposable repositories and local bare remotes."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


@unittest.skipUnless(
    shutil.which("git") and shutil.which("cargo"), "Git and Cargo required"
)
class ReleaseTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "scripts").mkdir()
        (self.root / "src").mkdir()
        (self.root / "src/lib.rs").write_text("// fixture\n")
        shutil.copyfile(ROOT / "scripts/release.py", self.root / "scripts/release.py")
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "fixture"\nversion = "1.0.0"\n'
        )
        (self.root / "Cargo.lock").write_text(
            'version = 4\n\n[[package]]\nname = "fixture"\nversion = "1.0.0"\n'
        )
        self.env = {
            **os.environ,
            "GIT_EDITOR": "true",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "",
        }
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Release Test")
        self.git("config", "user.email", "release-test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.autocrlf", "false")
        self.git("add", ".")
        self.git("commit", "-m", "fixture")

    def git(self, *args):
        return subprocess.run(
            ["git", "--no-pager", *args],
            cwd=self.root,
            env=self.env,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def release(self, *args):
        return subprocess.run(
            [sys.executable, "scripts/release.py", *args],
            check=False,
            cwd=self.root,
            env=self.env,
            capture_output=True,
            text=True,
            timeout=60,
            stdin=subprocess.DEVNULL,
        )

    def files(self):
        return [
            (self.root / name).read_bytes() for name in ("Cargo.toml", "Cargo.lock")
        ]

    def assert_clean(self):
        self.assertEqual(self.git("status", "--porcelain"), "")

    def remote(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.git("init", "--bare", "--initial-branch=main", temporary.name)
        self.git("remote", "add", "origin", temporary.name)
        self.git("push", "origin", "main")
        return temporary.name

    def test_dry_run_bumps_have_no_side_effects(self):
        before = self.files()
        head = self.git("rev-parse", "HEAD")
        for argument, version in [
            ("patch", "1.0.1"),
            ("minor", "1.1.0"),
            ("major", "2.0.0"),
            ("1.2.3", "1.2.3"),
        ]:
            with self.subTest(argument=argument):
                result = self.release(argument, "--dry-run", "--yes", "--no-sign")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(f"Releasing 1.0.0 -> {version}", result.stdout)
                self.assertEqual(self.files(), before)
                self.assertEqual(self.git("rev-parse", "HEAD"), head)
                self.assertEqual(self.git("tag", "--list"), "")
                self.assert_clean()

    def test_existing_tags_are_immutable(self):
        self.git("tag", "v1.0.1")
        original = self.git("rev-parse", "v1.0.1")
        for args in [("--force-retag",), ("patch",), ("1.0.0", "--dry-run")]:
            result = self.release(*args, "--no-push", "--no-sign")
            self.assertEqual(result.returncode, 2)
            self.assertEqual(self.git("rev-parse", "v1.0.1"), original)
            self.assert_clean()

    def test_feature_branch_and_detached_head_are_rejected(self):
        for detached in (False, True):
            with self.subTest(detached=detached):
                self.git("checkout", "--detach") if detached else self.git(
                    "checkout", "-b", "feature"
                )
                result = self.release("patch", "--dry-run", "--no-sign")
                self.assertEqual(result.returncode, 2)
                self.assertIn("must be made from main", result.stderr)
                self.assert_clean()

    def test_local_release_updates_versions_and_annotates_tag(self):
        message = 'Release "quoted" & literal $HOME `text`'
        result = self.release("patch", "--no-push", "--no-sign", "-m", message)
        self.assertEqual(result.returncode, 0, result.stderr)
        for contents in self.files():
            self.assertIn(b'version = "1.0.1"', contents)
        self.assertEqual(self.git("log", "-1", "--format=%s"), message)
        self.assertEqual(self.git("cat-file", "-t", "v1.0.1"), "tag")
        self.assertEqual(
            self.git("rev-parse", "v1.0.1^{commit}"), self.git("rev-parse", "HEAD")
        )
        self.assert_clean()

    def test_atomic_push_to_local_remote(self):
        remote = self.remote()
        result = self.release("patch", "--yes", "--no-sign")
        self.assertEqual(result.returncode, 0, result.stderr)
        head = self.git("rev-parse", "HEAD")
        self.assertEqual(self.git("-C", remote, "rev-parse", "refs/heads/main"), head)
        self.assertEqual(self.git("-C", remote, "rev-parse", "v1.0.1^{commit}"), head)
        self.assertEqual(self.git("-C", remote, "cat-file", "-t", "v1.0.1"), "tag")
        self.assert_clean()

    def test_remote_tag_collision_fails_before_mutation(self):
        remote = self.remote()
        head = self.git("rev-parse", "HEAD")
        self.git("-C", remote, "update-ref", "refs/tags/v1.0.1", head)
        result = self.release("patch", "--yes", "--no-sign")
        self.assertEqual(result.returncode, 2)
        self.assertIn("Remote tag", result.stderr)
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(self.git("tag", "--list"), "")
        self.assert_clean()

    def test_diverged_remote_main_fails_before_mutation(self):
        remote = self.remote()
        head = self.git("rev-parse", "HEAD")
        divergent = self.git("commit-tree", "HEAD^{tree}", "-m", "divergent root")
        self.git("push", "origin", f"{divergent}:refs/heads/main", "--force")
        result = self.release("patch", "--yes", "--no-sign")
        self.assertEqual(result.returncode, 2)
        self.assertIn("resolve any divergence", result.stderr)
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(
            self.git("-C", remote, "rev-parse", "refs/heads/main"), divergent
        )
        self.assert_clean()

    def test_preparation_failures_restore_original_files(self):
        original_lock = (self.root / "Cargo.lock").read_text()
        original_manifest = (self.root / "Cargo.toml").read_text()
        for problem in ("missing", "duplicate", "stale", "cargo"):
            with self.subTest(problem=problem):
                lock, manifest = original_lock, original_manifest
                if problem == "missing":
                    lock = lock.replace('name = "fixture"', 'name = "other"')
                if problem == "duplicate":
                    lock += original_lock[original_lock.index("[[package]]") :]
                if problem == "stale":
                    lock = lock.replace('version = "1.0.0"', 'version = "0.9.0"')
                if problem == "cargo":
                    manifest += "\n[dependencies\n"
                (self.root / "Cargo.lock").write_text(lock)
                (self.root / "Cargo.toml").write_text(manifest)
                self.git("add", ".")
                self.git("commit", "-m", problem)
                before, head = self.files(), self.git("rev-parse", "HEAD")
                result = self.release("patch", "--no-push", "--no-sign")
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn("original version files restored", result.stderr)
                self.assertEqual(self.files(), before)
                self.assertEqual(self.git("rev-parse", "HEAD"), head)
                self.assertEqual(self.git("tag", "--list"), "")
                self.assert_clean()

    def stage_user_work(self):
        (self.root / "unrelated.txt").write_text("user work\n")
        self.git("add", "unrelated.txt")

    def test_commit_failure_preserves_unrelated_staged_work(self):
        self.stage_user_work()
        hook = self.root / ".git/hooks/pre-commit"
        hook.write_text("#!/bin/sh\nexit 1\n")
        hook.chmod(0o755)
        before, head = self.files(), self.git("rev-parse", "HEAD")
        result = self.release("patch", "--no-push", "--no-sign", "--no-verify")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("original version files restored", result.stderr)
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(self.files(), before)
        self.assertEqual(self.git("status", "--porcelain"), "A  unrelated.txt")

    def test_successful_commit_preserves_unrelated_staged_work(self):
        self.stage_user_work()
        result = self.release("patch", "--no-push", "--no-sign", "--no-verify")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("unrelated", self.git("ls-tree", "--name-only", "HEAD"))
        self.assertEqual(self.git("status", "--porcelain"), "A  unrelated.txt")

    def test_signing_failure_retains_release_commit(self):
        self.git("config", "gpg.program", "nonexistent-switchboot-fixture-gpg")
        head = self.git("rev-parse", "HEAD")
        result = self.release("patch", "--no-push", "--sign")
        self.assertEqual(result.returncode, 1)
        self.assertIn("Release commit retained", result.stderr)
        self.assertNotEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(self.git("tag", "--list"), "")
        self.assert_clean()

    def test_dirty_version_files_rejected_even_without_verification(self):
        with (self.root / "Cargo.toml").open("a") as manifest:
            manifest.write("# user change\n")
        before = self.files()
        result = self.release("patch", "--no-push", "--no-sign", "--no-verify")
        self.assertEqual(result.returncode, 2)
        self.assertIn("Version files must be clean", result.stderr)
        self.assertEqual(self.files(), before)

    def test_noninteractive_release_does_not_push_without_yes(self):
        remote = self.remote()
        old_head = self.git("rev-parse", "HEAD")
        result = self.release("patch", "--no-sign")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git("-C", remote, "rev-parse", "main"), old_head)
        self.assertEqual(self.git("-C", remote, "tag", "--list"), "")

    def test_invalid_versions_are_rejected(self):
        for version in ("1.0.0", "01.2.3", "1.2.3-rc1", "1.2", "nonsense"):
            result = self.release(version, "--dry-run", "--no-sign")
            self.assertEqual(result.returncode, 2)
            self.assert_clean()


if __name__ == "__main__":
    unittest.main()
