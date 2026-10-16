#!/usr/bin/env python3
"""Check reporting failures before benchmark processes start."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class Arguments(unittest.TestCase):
    def test_duplicate_map_names_fail_before_output_creation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            maps = []
            for name in ("first", "second"):
                parent = root / name
                parent.mkdir()
                path = parent / "map.osm.pbf"
                path.write_bytes(b"")
                maps.append(str(path))
            output = root / "results"
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name("compare.py")),
                 "--candidate", "unused", str(root / "missing-binary"), "entities",
                 "--maps", *maps, "--output", str(output)],
                text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("Map filenames must be unique", result.stderr)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
