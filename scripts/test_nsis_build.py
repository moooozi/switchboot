"""Check installer assembly and DLL identity without compiling or running payloads."""

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

NSIS = Path(__file__).resolve().parent.parent / "packaging/nsis"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


artifact = load_module("dll_artifact", NSIS / "toolkit/dll_artifact.py")
with mock.patch.dict(sys.modules, {"dll_artifact": artifact}):
    builder = load_module("nsis_build", NSIS / "build.py")


class NsisBuildTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.directory = self.root / "dll"
        self.directory.mkdir()
        self.dll = self.directory / artifact.DLL_NAME
        self.dll.write_bytes(b"harmless DLL fixture")
        self.revision = "a" * 40
        self.identity = artifact.metadata(self.dll, self.revision)
        self.write_identity()

    def write_identity(self):
        (self.directory / "plugin.json").write_text(
            json.dumps(self.identity), encoding="utf-8"
        )

    def test_prebuilt_inputs_only_run_installer_compiler(self):
        (self.root / "Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
        icons = self.root / "crates/switchboot_core/resources/icons/ico"
        icons.mkdir(parents=True)
        (icons / "fixture.ico").write_bytes(b"icon")
        here = self.root / "packaging/nsis"
        here.mkdir(parents=True)
        payload = self.root / "switchboot.exe"
        payload.write_bytes(b"harmless application fixture")
        output = self.root / "setup.exe"

        def compile_installer(command, **kwargs):
            output.write_bytes(b"harmless installer fixture")

        with (
            mock.patch.object(builder, "ROOT", self.root),
            mock.patch.object(builder, "HERE", here),
            mock.patch.object(builder, "find_makensis", return_value="makensis"),
            mock.patch.object(builder, "build_plugin") as compile_dll,
            mock.patch.object(builder, "verify_payload_architecture") as verify_payload,
            mock.patch.object(builder, "verify_unelevated_manifest") as verify_manifest,
            mock.patch.object(artifact, "toolkit_revision", return_value=self.revision),
            mock.patch.object(
                builder.subprocess, "run", side_effect=compile_installer
            ) as run,
        ):
            builder.build(output, payload=payload, plugin_artifact=self.directory)
        compile_dll.assert_not_called()
        verify_payload.assert_called_once_with(payload.resolve())
        verify_manifest.assert_called_once_with(output)
        self.assertEqual(run.call_count, 1)
        command = run.call_args.args[0]
        self.assertEqual(command[0], "makensis")
        self.assertIn(f"-DPAYLOAD_EXE={payload.resolve()}", command)
        self.assertIn(f"-DPLUGIN_DIRECTORY={self.dll.resolve().parent}", command)


if __name__ == "__main__":
    unittest.main()
