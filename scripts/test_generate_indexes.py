from pathlib import Path
import tempfile
import unittest
from generate_indexes import generate_indexes


class IndexTests(unittest.TestCase):
    def test_nested_indexes_escape_names_sort_directories_first_and_are_repeatable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo & packages"
            directory = root / "z directory"
            directory.mkdir(parents=True)
            (root / "a & b.rpm").write_bytes(b"package")
            (directory / "package.deb").write_bytes(b"package")
            generate_indexes(root)
            index = root / "index.html"
            first = index.read_bytes()
            text = first.decode()
            self.assertIn('href="a%20%26%20b.rpm">a &amp; b.rpm', text)
            self.assertIn("repo &amp; packages", text)
            self.assertLess(text.index('href="z%20directory/"'), text.index('href="a%20%26%20b.rpm"'))
            self.assertIn('href="package.deb"', (directory / "index.html").read_text())
            generate_indexes(root)
            self.assertEqual(index.read_bytes(), first)
            self.assertNotIn('href="index.html"', text)

    def test_missing_directory_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(ValueError):
                generate_indexes(Path(temporary) / "missing")
