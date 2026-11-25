import hashlib
import io
from pathlib import Path
import tempfile
import unittest

from scripts.fetch_runtime import verified_copy


class RuntimeIntegrity(unittest.TestCase):
    def test_accepts_exact_content(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "member"
            verified_copy(io.BytesIO(b"abc"), path, 3, hashlib.sha256(b"abc").hexdigest())
            self.assertEqual(path.read_bytes(), b"abc")

    def test_rejects_truncation_extra_data_and_wrong_hash(self):
        digest = hashlib.sha256(b"abc").hexdigest()
        for data in (b"ab", b"abcd", b"xyz"):
            with self.subTest(data=data), tempfile.TemporaryDirectory() as folder:
                with self.assertRaises(ValueError):
                    verified_copy(io.BytesIO(data), Path(folder) / "member", 3, digest)
