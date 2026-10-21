"""Generate browsable directory indexes for the package repositories."""

import argparse
import html
from pathlib import Path
from urllib.parse import quote


def generate_indexes(directory):
    directory = Path(directory)
    if not directory.is_dir():
        raise ValueError(f"Repository directory does not exist: {directory}")
    # Exclude our own output so repeated runs produce identical indexes.
    entries = sorted(
        (p for p in directory.iterdir() if p.name != "index.html"),
        key=lambda p: (not p.is_dir(), p.name.casefold(), p.name),
    )
    title = html.escape(f"Index of {directory.as_posix()}")
    lines = [
        f'<!doctype html>\n<html><head><meta charset="utf-8"><title>{title}</title></head><body>',
        f"<h1>{title}</h1>\n<ul>",
    ]
    for entry in entries:
        suffix = "/" if entry.is_dir() else ""
        lines.append(
            f'<li><a href="{quote(entry.name, safe="")}{suffix}">{html.escape(entry.name)}{suffix}</a></li>'
        )
    lines.append("</ul>\n</body></html>\n")
    (directory / "index.html").write_text("\n".join(lines), encoding="utf-8")
    for entry in entries:
        if entry.is_dir() and not entry.is_symlink():
            generate_indexes(entry)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directories", nargs="+", type=Path)
    for target in parser.parse_args().directories:
        generate_indexes(target)
