"""Select native Bash for shell fixtures, avoiding the Windows WSL launcher."""
import os
from pathlib import Path
import shutil

WINDOWS = os.name == "nt"


def find_bash():
    if not WINDOWS:
        return shutil.which("bash")
    git = shutil.which("git")
    if git:
        for parent in Path(git).resolve().parents:
            candidate = parent / "usr/bin/bash.exe"
            if candidate.is_file():
                return str(candidate)
    raise RuntimeError("Git for Windows Bash is required; refusing to use a WSL bash launcher")


BASH = find_bash()


def bash_env(env=None):
    result = dict(os.environ if env is None else env)
    if WINDOWS:
        # PowerShell's PATH need not include Git's Unix tools (cp, cat, etc.).
        result["PATH"] = str(Path(BASH).parent) + os.pathsep + result.get("PATH", "")
    return result
