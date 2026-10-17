"""Build the real NSIS installer without running it or changing the live service."""
from pathlib import Path
import runpy
import sys

if __name__ == "__main__":
    builder = Path(__file__).resolve().parents[1] / "nsis-poc/build.py"
    sys.argv = [str(builder), "--installer", *sys.argv[1:]]
    runpy.run_path(str(builder), run_name="__main__")
