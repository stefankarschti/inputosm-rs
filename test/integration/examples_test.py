#!/usr/bin/env python3
"""Check runnable examples and optional C++ output parity."""
import argparse
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/data"))
from pbf_test_data import statistics_fixture

EXAMPLES = ROOT / "target/release/examples"
CPP = None
NAMES = ("count_blocks", "count_entity", "custom_log", "read_block", "lat_stat", "statistics", "extract_ferries", "export_csv", "export_db")


def execute(name, *arguments, directory=None, cpp=False, success=True):
    binary = (CPP if cpp else EXAMPLES) / name
    result = subprocess.run([str(binary), *map(str, arguments)], cwd=directory, capture_output=True, timeout=60)
    if success and result.returncode:
        raise AssertionError(f"{name}: {result.stderr.decode(errors='replace')}")
    if not success and result.returncode == 0:
        raise AssertionError(f"{name} accepted invalid arguments or input")
    return result


def records(path, kind):
    data = path.read_bytes()
    offset = 0
    result = []
    def take(fmt):
        nonlocal offset
        value = struct.unpack_from("<" + fmt, data, offset)
        offset += struct.calcsize("<" + fmt)
        return value
    def text():
        nonlocal offset
        end = data.index(0, offset)
        value = data[offset:end]
        offset = end + 1
        return value
    while offset < len(data):
        start = offset
        take("q")
        if kind == "node":
            take("ii")
        else:
            count, = take("h")
            if count < 0:
                raise AssertionError("Negative reference count")
            for _ in range(count):
                take("q")
                if kind == "relation":
                    text()
                    take("b")
        count, = take("h")
        if count < 0:
            raise AssertionError("Negative tag count")
        for _ in range(count):
            text()
            text()
        result.append(data[start:offset])
    return sorted(result)


def stable_output(output):
    return b"\n".join(line for line in output.splitlines() if not line.startswith(b"running on "))


class Examples(unittest.TestCase):
    def test_arguments_and_missing_input(self):
        for name in NAMES:
            execute(name, success=False)
        execute("read_block", ROOT / "tests/data/ordinary.osm.pbf", "1", "--bad", success=False)
        execute("read_block", ROOT / "tests/data/ordinary.osm.pbf", "-1", success=False)
        execute("count_entity", "/inputosm-missing-file", success=False)

    def test_counts_random_access_log_and_latitude(self):
        path = ROOT / "tests/data/comprehensive.osm.pbf"
        counts = execute("count_entity", path).stdout
        self.assertIn(b"nodes: 17,005\nways: 12\nrelations: 4\n", counts)
        execute("count_blocks", path)
        a = execute("read_block", path, "1").stdout
        self.assertEqual(a, execute("read_block", path, "1", "--index").stdout)
        self.assertIn(b" UTC [TRC]:", execute("custom_log", path).stderr)
        latitude = execute("lat_stat", path).stdout
        self.assertIn(b"17,005", latitude.splitlines()[-1])
        self.assertEqual(len(latitude.splitlines()), 96)
        self.assertIn(b"3 ferries", execute("extract_ferries", path).stdout)

    def test_statistics_multiple_groups_and_timestamp_units(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "statistics.pbf"
            path.write_bytes(statistics_fixture())
            for metadata in (False, True):
                args = ("1",) if metadata else ()
                output = execute("statistics", path, *args).stdout
                for expected in (
                    "nodes: 7\nways: 3\nrelations: 3\n", "max nodes per block: 5\nmax node tags per block: 3\n",
                    "max ways per block: 2\nmax way tags per block: 2\nmax way nodes per block: 5\n",
                    "max relations per block: 2\nmax relation tags per block: 3\nmax relation members per block: 4\n",
                    "max file block index: 4\n", "nodes with tags: 3\nways with tags: 2\nrelations with tags: 2\n",
                    "max node id: 100\nmax way id: 90\nmax relation id: 91\n",
                ):
                    self.assertIn(expected.encode(), output)
                for kind, timestamp in zip(("node", "way", "relation"), ("2223-07-06 14:13:20", "2109-06-06 22:13:20", "2118-12-09 03:33:20")):
                    if not metadata:
                        timestamp = "1970-01-01 00:00:00"
                    self.assertIn(f"max {kind} timestamp: {timestamp} GMT\n".encode(), output)
                if CPP:
                    self.assertEqual(stable_output(output), stable_output(execute("statistics", path, *args, cpp=True).stdout))

    def test_export_records_and_existing_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = ROOT / "tests/data/sample.osm"
            binary, csv = root / "binary", root / "csv"
            execute("export_db", source, binary)
            execute("export_csv", source, csv)
            expected_node = struct.pack("<qiih", 1, 525200000, 134050000, 2) + b"name\0Node One\0amenity\0cafe\0"
            self.assertIn(expected_node, records(binary / "node", "node"))
            self.assertEqual(records(binary / "way", "way"), [struct.pack("<qhqqh", 10, 2, 1, 2, 2) + b"highway\0residential\0name\0A Street\0"])
            self.assertEqual(records(binary / "relation", "relation"), [struct.pack("<qhq", 20, 2, 1) + b"stop\0\0" + struct.pack("<q", 10) + b"route\0\1" + struct.pack("<h", 2) + b"type\0route\0route\0bus\0"])
            self.assertIn(b'1;0;0;1577934245;111;\'"name"=>"Node One","amenity"=>"cafe"\';POINT(52.5200000 13.4050000)\n', (csv / "nodes.csv").read_bytes())
            self.assertEqual((csv / "way_node.csv").read_bytes(), b"10;1;0\n10;2;1\n")
            self.assertEqual((csv / "relation_members.csv").read_bytes(), b'20;1;N;"stop";0\n20;10;W;"route";1\n')
            for name, target in [("export_db", binary), ("export_csv", csv)]:
                before = {p.name: p.read_bytes() for p in target.iterdir()}
                execute(name, source, target, success=False)
                self.assertEqual(before, {p.name: p.read_bytes() for p in target.iterdir()})
            bad = root / "bad.osm"
            bad.write_text('<osm><node id="1" lat="0" lon="0"><tag k="name" v="a;b"/></node></osm>')
            execute("export_csv", bad, root / "literal-export")
            self.assertIn(b'"name"=>"a;b"', (root / "literal-export/nodes.csv").read_bytes())
            if CPP:
                cpp = root / "literal-cpp"
                cpp.mkdir()
                execute("export_csv", bad, directory=cpp, cpp=True)
                self.assertEqual((cpp / "nodes.csv").read_bytes(), (root / "literal-export/nodes.csv").read_bytes())

    def test_cpp_parity(self):
        if CPP is None:
            self.skipTest("Supply --cpp-examples for C++ output comparisons")
        for fixture in ("ordinary.osm.pbf", "comprehensive.osm.pbf"):
            source = ROOT / "tests/data" / fixture
            for name in ("count_blocks", "count_entity", "lat_stat", "statistics", "extract_ferries", "read_block"):
                args = ("1",) if name == "read_block" else ()
                self.assertEqual(stable_output(execute(name, source, *args).stdout), stable_output(execute(name, source, *args, cpp=True).stdout), name)
            for name in ("export_db", "export_csv"):
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    cpp, rust = root / "cpp", root / "rust"
                    cpp.mkdir()
                    execute(name, source, directory=cpp, cpp=True)
                    execute(name, source, rust)
                    self.assertEqual({p.name for p in cpp.iterdir()}, {p.name for p in rust.iterdir()})
                    for p in cpp.iterdir():
                        if name == "export_db":
                            self.assertEqual(records(p, p.name), records(rust / p.name, p.name))
                        else:
                            self.assertEqual(sorted(p.read_bytes().splitlines()), sorted((rust / p.name).read_bytes().splitlines()))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--examples", type=Path, default=EXAMPLES)
    parser.add_argument("--cpp-examples", type=Path)
    args = parser.parse_args()
    EXAMPLES = args.examples.resolve()
    CPP = args.cpp_examples.resolve() if args.cpp_examples else None
    unittest.main(argv=[sys.argv[0]])
