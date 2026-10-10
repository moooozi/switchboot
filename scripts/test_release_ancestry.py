"""Exercise CI ancestry checks using disposable local Git refs."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parent / "verify-release-tag.sh"


@unittest.skipUnless(shutil.which("bash"), "Bash required")
class ReleaseAncestryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "root")
        self.git("tag", "v1.0.0")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")

    def git(self, *args):
        return subprocess.check_output(["git", "--no-pager", *args], cwd=self.root,
                                       text=True, stderr=subprocess.STDOUT).strip()

    def check(self, tag="v1.0.0"):
        return subprocess.run(["bash", SCRIPT.as_posix(), tag], cwd=self.root,
                              capture_output=True, text=True, timeout=10)

    def test_tag_at_main_is_accepted(self):
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_old_main_tag_is_accepted_with_matching_checkout(self):
        self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "later")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.git("checkout", "--detach", "v1.0.0")
        self.assertEqual(self.check().returncode, 0)

    def test_off_main_tag_is_rejected(self):
        self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "off main")
        self.git("tag", "v1.0.1")
        self.assertNotEqual(self.check("v1.0.1").returncode, 0)

    def test_missing_main_tag_or_mismatched_checkout_is_rejected(self):
        self.assertNotEqual(self.check("v9.0.0").returncode, 0)
        self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "other")
        self.assertNotEqual(self.check().returncode, 0)
        self.git("checkout", "--detach", "v1.0.0")
        self.git("update-ref", "-d", "refs/remotes/origin/main")
        self.assertNotEqual(self.check().returncode, 0)


if __name__ == "__main__":
    unittest.main()
