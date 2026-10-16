#!/usr/bin/env python3
"""Compare equivalent reader workloads in separate processes."""
import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time

WORKLOADS = {
    "counts": ["blocks", "--counts=15"],
    "nodes": ["blocks", "--nodes=7", "--consume=all"],
    "full": ["blocks", "--nodes=31", "--ways=7", "--relations=31", "--consume=all"],
    "entities": ["entities"],
}
PARITY = ("blocks", "max_index", "nodes", "ways", "relations", "checksum", "values_checksum", "strings")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpp", required=True, type=Path)
    parser.add_argument("--rust", required=True, type=Path)
    parser.add_argument("--maps", nargs="+", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--threads", nargs="+", type=int, default=[1, 32])
    parser.add_argument("--workloads", nargs="+", choices=WORKLOADS, default=["counts", "nodes", "full"])
    args = parser.parse_args()
    if args.rounds < 1 or any(t < 1 or t > 256 for t in args.threads):
        parser.error("Invalid rounds or thread counts")
    args.output.mkdir(parents=True, exist_ok=False)
    binaries = {"cpp": args.cpp.resolve(), "rust": args.rust.resolve()}
    manifest = {
        "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "platform": platform.platform(), "cpu_count": os.cpu_count(),
        "binaries": {k: {"path": str(p), "sha256": sha(p)} for k, p in binaries.items()},
        "rounds": args.rounds, "threads": args.threads, "workloads": args.workloads,
        "cache": "No cache clearing; one warmup per workload, thread count, and implementation.",
        "maps": [{"path": str(p.resolve()), "bytes": p.stat().st_size, "mtime_ns": p.stat().st_mtime_ns} for p in args.maps],
        "cpuinfo": Path("/proc/cpuinfo").read_text(),
    }
    save(args.output / "manifest.json", manifest)
    records = []
    expected = {}
    for path in args.maps:
        initial = path.stat()
        for threads in args.threads:
            for workload in args.workloads:
                mode, *options = WORKLOADS[workload]
                for round_index in range(-1, args.rounds):
                    order = ["cpp", "rust"] if round_index % 2 == 0 else ["rust", "cpp"]
                    for implementation in order:
                        command = [str(binaries[implementation]), str(path.resolve()), mode, str(threads), "1", *options]
                        timing = args.output / "process-time.txt"
                        start = time.perf_counter()
                        process = subprocess.run(["/usr/bin/time", "-f", "%e,%U,%S,%M", "-o", str(timing), *command], text=True, capture_output=True)
                        elapsed = time.perf_counter() - start
                        prefix = f"{path.stem}-{threads}-{workload}-{round_index}-{implementation}"
                        (args.output / f"{prefix}.csv").write_text(process.stdout)
                        (args.output / f"{prefix}.stderr").write_text(process.stderr)
                        if process.returncode:
                            raise RuntimeError(f"Failed command: {command}\n{process.stderr}")
                        rows = list(csv.DictReader(io.StringIO(process.stdout)))
                        if len(rows) != 1:
                            raise RuntimeError(f"Expected one CSV row: {command}")
                        row = rows[0]
                        values = tuple(int(row[k]) for k in PARITY)
                        key = (str(path), threads, workload)
                        if key in expected and expected[key] != values:
                            raise RuntimeError(f"Parity mismatch for {prefix}: {values} != {expected[key]}")
                        expected[key] = values
                        _, user, system, rss = timing.read_text().strip().split(",")
                        record = {"map": path.name, "threads": threads, "workload": workload,
                                  "round": round_index, "implementation": implementation, "command": command,
                                  "process_wall_seconds": elapsed, "cpu_seconds": float(user) + float(system),
                                  "max_rss_kib": int(rss), "csv": row}
                        records.append(record)
                        save(args.output / "results.json", records)
                        print(f"{prefix}: {float(row['total_seconds']):.3f}s API, {elapsed:.3f}s process, {int(rss)/1024:.1f} MiB", flush=True)
        final = path.stat()
        if (initial.st_size, initial.st_mtime_ns, initial.st_ino) != (final.st_size, final.st_mtime_ns, final.st_ino):
            raise RuntimeError(f"Input changed: {path}")
    summaries = []
    for path in args.maps:
        for threads in args.threads:
            for workload in args.workloads:
                summary = {"map": path.name, "threads": threads, "workload": workload}
                for implementation in binaries:
                    rows = [r for r in records if r["round"] >= 0 and (r["map"], r["threads"], r["workload"], r["implementation"]) == (path.name, threads, workload, implementation)]
                    times = [float(r["csv"]["total_seconds"]) for r in rows]
                    summary[implementation] = {"median_seconds": statistics.median(times), "min_seconds": min(times), "max_seconds": max(times),
                                               "median_cpu_seconds": statistics.median(r["cpu_seconds"] for r in rows),
                                               "max_rss_kib": max(r["max_rss_kib"] for r in rows)}
                summary["cpp_over_rust"] = summary["cpp"]["median_seconds"] / summary["rust"]["median_seconds"]
                summaries.append(summary)
    save(args.output / "summary.json", summaries)
    (args.output / "process-time.txt").unlink()


if __name__ == "__main__":
    main()
