import stat
from pathlib import Path
import tempfile
import unittest
import zipfile

from scripts.fetch_rife import extract_zip


class ProviderArchive(unittest.TestCase):
    def test_rejects_escaping_names_windows_devices_and_links(self):
        for name in ["../outside", "/outside", "C:/outside", "sub\\..\\escape", "NUL.txt", "x:stream", "trailing. ", "link"]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temp:
                archive = Path(temp, "test.zip")
                with zipfile.ZipFile(archive, "w") as z:
                    member = zipfile.ZipInfo(name)
                    if name == "link":
                        member.external_attr = (stat.S_IFLNK | 0o777) << 16
                    z.writestr(member, b"x")
                with self.assertRaises(ValueError):
                    extract_zip(archive, Path(temp, "output"))

    def test_copies_regular_members_within_budget(self):
        with tempfile.TemporaryDirectory() as temp:
            archive = Path(temp, "test.zip")
            with zipfile.ZipFile(archive, "w") as z:
                z.writestr("lib/example", b"data")
            dest = Path(temp, "out")
            extract_zip(archive, dest)
            self.assertEqual((dest / "lib/example").read_bytes(), b"data")
