#!/usr/bin/env python3
"""Check benchmark arguments, timing fields, and comparison reports."""
import csv
import io
import json
import os
import shutil
import sys
from pathlib import Path
import subprocess
import tempfile
import unittest

from run_comparison import jobs, signature

ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "target/release/pbf_benchmark"
FIXTURE = ROOT / "tests/data/comprehensive.osm.pbf"


def run(*args, success=True):
    result = subprocess.run([str(BINARY), str(FIXTURE), *args], text=True, capture_output=True, timeout=30)
    if success and result.returncode:
        raise AssertionError(result.stderr)
    if not success and result.returncode == 0:
        raise AssertionError(f"Accepted invalid arguments: {args}")
    return list(csv.DictReader(io.StringIO(result.stdout))) if success else []


class Benchmark(unittest.TestCase):
    def test_invalid_arguments(self):
        for args in [("bad", "1", "1"), ("blocks", "0", "1"), ("blocks", "1", "0"),
                     ("blocks", "1", "1", "--nodes=32"), ("blocks", "1", "1", "--consume=bad"),
                     ("entities", "1", "1", "--nodes=1"), ("entities", "1", "1", "--consume=all"),
                     ("random", "1", "1", "0", "1", "0"), ("random", "1", "1", "0", "0", "1")]:
            run(*args, success=False)

    def test_repetitions_modes_masks_and_timing_boundaries(self):
        reference = run("blocks", "1", "1", "--counts=15")[0]
        for mode in ("entities", "blocks", "random", "random-index"):
            rows = run(mode, "4", "3", "1", "32", reference["max_index"])
            self.assertEqual(len(rows), 3)
            for i, row in enumerate(rows):
                self.assertEqual(int(row["iteration"]), i)
                self.assertEqual(signature(row), signature(rows[0]))
                self.assertEqual([row[k] for k in ("node_fields", "way_fields", "relation_fields")], ["31", "7", "31"])
                total = sum(float(row[k]) for k in ("setup_seconds", "read_seconds", "teardown_seconds"))
                self.assertAlmostEqual(total, float(row["total_seconds"]), places=7)
                self.assertGreaterEqual(float(row["teardown_seconds"]), 0)
                self.assertEqual(int(row["requests"]), 32 if mode.startswith("random") else 0)
                if mode == "random-index":
                    self.assertGreater(int(row["index_bytes"]), 0)
                    self.assertLessEqual(float(row["index_build_seconds"]), float(row["setup_seconds"]))
        row = run("blocks", "1", "1", "--counts=15")[0]
        self.assertEqual([row[k] for k in ("node_fields", "way_fields", "relation_fields")], ["-1"] * 3)

    @unittest.skipUnless(sys.platform == "linux" and shutil.which("cc"), "Requires Linux and a C compiler")
    def test_random_worker_creation_failure_exits(self):
        with tempfile.TemporaryDirectory() as directory:
            shim = Path(directory) / "fail_threads.so"
            subprocess.run(["cc", "-shared", "-fPIC", "-o", str(shim),
                            str(ROOT / "test/benchmark/fail_threads.c"), "-ldl"], check=True)
            for mode in ("random", "random-index"):
                result = subprocess.run([str(BINARY), str(FIXTURE), mode, "4", "1", "0", "8", "1"],
                                        env={**os.environ, "LD_PRELOAD": str(shim)}, capture_output=True, timeout=5)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn(b"thread 'main' panicked", result.stderr)

    def test_runner_manifests_and_reports(self):
        self.assertEqual(len(jobs(False, 8, 1)), 6)
        self.assertEqual(len(jobs(True, 8, 1)), 28)
        with tempfile.TemporaryDirectory() as directory:
            for script, extra in [("run_comparison.py", []), ("run_deferred.py", ["--only", "counts-15"])]:
                output = Path(directory) / script
                subprocess.run(["python3", str(ROOT / "test/benchmark" / script), "--cpp", str(BINARY), "--rust", str(BINARY),
                                "--input", str(FIXTURE), "--output", str(output), "--threads", "1", "--rounds", "1", "--requests", "8", *extra],
                               check=True, capture_output=True, timeout=30)
                manifest = json.loads((output / "manifest.json").read_text())
                self.assertEqual(manifest["binaries"]["cpp"]["sha256"], manifest["binaries"]["rust"]["sha256"])
                results = json.loads((output / "results.json").read_text())
                self.assertEqual({r["round"] for r in results}, {-1, 0})
                summary = json.loads((output / "summary.json").read_text())
                self.assertTrue(summary)
                self.assertTrue(all(s["metrics"]["read_seconds"]["samples"] == 1 for s in summary))
                self.assertFalse(list(output.glob("*.pid")))


if __name__ == "__main__":
    unittest.main()
