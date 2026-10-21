"""Collect unsigned PR builds; never install or execute the application."""

import json
import os
import shutil
import tarfile
from pathlib import Path


def required_file(path):
    if not path.is_file() or path.stat().st_size == 0:
        raise RuntimeError(f"Missing or empty build artifact: {path}")
    return path


def unique_file(folder, pattern):
    files = list(folder.glob(pattern))
    if len(files) != 1:
        raise RuntimeError(f"Expected one {pattern} in {folder}, found {len(files)}")
    return required_file(files[0])


def collect(root, *, windows):
    release = root / "target/release"
    output = root / "artifacts/ci"
    output.mkdir(parents=True, exist_ok=True)
    if windows:
        executable = required_file(release / "switchboot.exe")
        installer = required_file(root / "artifacts/Switchboot_x64-setup.exe")
        shutil.copy2(executable, output / "Switchboot_x64-portable.exe")
        shutil.copy2(installer, output / "Switchboot_x64-setup.exe")
    else:
        executable = required_file(release / "switchboot")
        deb = unique_file(release / "bundle/deb", "*.deb")
        rpm = unique_file(release / "bundle/rpm", "*.rpm")
        # Tar preserves executable permissions when downloaded from Actions.
        with tarfile.open(output / "Switchboot_linux_x64.tar.gz", "w:gz") as archive:
            archive.add(executable, arcname="switchboot")
        shutil.copy2(deb, output / deb.name)
        shutil.copy2(rpm, output / rpm.name)
    metadata = {
        "commit": os.environ.get("GITHUB_SHA", "local"),
        "layout": "iced",
        "platform": "windows" if windows else "linux",
        "signed": False,
    }
    (output / "build-info.json").write_text(
        json.dumps(metadata, indent=2) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    collect(Path(__file__).resolve().parent.parent, windows=os.name == "nt")
