"""Check shell selection without requiring a WSL installation."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import bash_test_support as support


class BashSelectionTests(unittest.TestCase):
    def test_unix_uses_path_bash(self):
        with patch.object(support, "WINDOWS", False), patch.object(
                support.shutil, "which", return_value="/bin/bash") as which:
            self.assertEqual(support.find_bash(), "/bin/bash")
            which.assert_called_once_with("bash")

    def test_windows_selects_git_bash_not_path_launcher(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bash = root / "usr/bin/bash.exe"
            bash.parent.mkdir(parents=True)
            bash.touch()
            for directory in ("cmd", "mingw64/bin"):
                git = root / directory / "git.exe"
                git.parent.mkdir(parents=True)
                git.touch()
                with self.subTest(directory=directory), patch.object(support, "WINDOWS", True), patch.object(
                        support.shutil, "which", return_value=str(git)) as which:
                    self.assertEqual(Path(support.find_bash()), bash)
                    which.assert_called_once_with("git")

    def test_windows_fails_if_git_bash_is_unavailable(self):
        with patch.object(support, "WINDOWS", True), patch.object(support.shutil, "which", return_value=None):
            with self.assertRaisesRegex(RuntimeError, "refusing to use a WSL"):
                support.find_bash()

    def test_windows_environment_includes_git_unix_tools(self):
        bash = str(Path("fixture") / "usr/bin/bash.exe")
        with patch.object(support, "WINDOWS", True), patch.object(support, "BASH", bash):
            original = {"PATH": "existing", "FIXTURE": "value"}
            result = support.bash_env(original)
            self.assertEqual(result["PATH"], str(Path(bash).parent) + support.os.pathsep + "existing")
            self.assertEqual(result["FIXTURE"], "value")
            self.assertEqual(original["PATH"], "existing")


if __name__ == "__main__":
    unittest.main()
