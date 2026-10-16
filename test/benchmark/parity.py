#!/usr/bin/env python3
"""Compare the original and Rust benchmark outputs before timing."""
import argparse
import csv
import io
from pathlib import Path
import subprocess

FIELDS = ("blocks", "max_index", "nodes", "ways", "relations", "checksum", "values_checksum", "strings")


def run(binary, path, mode, threads, options):
    result = subprocess.run([str(binary), str(path), mode, str(threads), "1", *options], text=True, capture_output=True, check=True)
    row = list(csv.DictReader(io.StringIO(result.stdout)))[0]
    return tuple(int(row[k]) for k in FIELDS)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpp", required=True, type=Path)
    parser.add_argument("--rust", required=True, type=Path)
    parser.add_argument("--maps", nargs="*", type=Path, default=[Path("tests/data/ordinary.osm.pbf"), Path("tests/data/comprehensive.osm.pbf")])
    args = parser.parse_args()
    cases = [[f"--{kind}={mask}", "--consume=all"] for kind, maximum in [("nodes", 31), ("ways", 15), ("relations", 31), ("counts", 15)] for mask in range(maximum + 1)]
    cases += [["--strings", "--consume=all"], ["--nodes=31", "--ways=15", "--relations=31", "--strings", "--counts=15", "--consume=all"]]
    count = 0
    for path in args.maps:
        for options in cases:
            a, b = [run(binary, path, "blocks", 1, options) for binary in [args.cpp, args.rust]]
            assert a == b, (path, options, a, b)
            count += 1
        maximum = run(args.cpp, path, "blocks", 1, ["--counts=0"])[1]
        for mode in ["random", "random-index"]:
            for threads in [1, 4]:
                options = ["0", "64", str(maximum), "--nodes=31", "--ways=7", "--relations=31", "--consume=all"]
                a, b = [run(binary, path, mode, threads, options) for binary in [args.cpp, args.rust]]
                assert a == b, (path, mode, threads, a, b)
                count += 1
        for metadata in ["0", "1"]:
            a, b = [run(binary, path, "entities", 4, [metadata]) for binary in [args.cpp, args.rust]]
            assert a == b, (path, metadata, a, b)
            count += 1
    print(f"{count} C++/Rust comparisons passed")


if __name__ == "__main__":
    main()
