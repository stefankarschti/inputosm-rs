#!/usr/bin/env python3
"""Measure the complete-field consumer and verify its checksum."""
import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import statistics
import subprocess
import time


def save(path, data):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(data, indent=2) + "\n")
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", action="append", nargs=3, metavar=("NAME", "BINARY", "MODE"), required=True)
    parser.add_argument("--maps", nargs="+", type=Path, required=True)
    parser.add_argument("--threads", nargs="+", type=int, default=[1, 32])
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--single-cpu", type=int)
    parser.add_argument("--evict-file-cache", action="store_true",
                        help="Request file-cache eviction before each process with POSIX_FADV_DONTNEED")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.rounds < 1 or any(not 1 <= t <= 256 for t in args.threads):
        parser.error("Invalid rounds or thread counts")
    if len({p.name for p in args.maps}) != len(args.maps):
        parser.error("Map filenames must be unique")
    if args.single_cpu is not None and args.single_cpu not in os.sched_getaffinity(0):
        parser.error("Selected CPU is unavailable")
    if args.evict_file_cache and not hasattr(os, "posix_fadvise"):
        parser.error("File-cache advice is unavailable on this platform")
    candidates = [(name, Path(binary).resolve(), mode) for name, binary, mode in args.candidate]
    if len({c[0] for c in candidates}) != len(candidates):
        parser.error("Candidate names must be unique")
    if any(mode not in ("entities", "views", "public-views", "sequential-views", "mapped-views") for _, _, mode in candidates):
        parser.error("Invalid consumer mode")
    args.output.mkdir(parents=True, exist_ok=False)
    save(args.output / "manifest.json", {
        "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "cpu_count": os.cpu_count(), "rounds": args.rounds,
        "threads": args.threads,
        "single_cpu": args.single_cpu,
        "cache": ("POSIX_FADV_DONTNEED before every process; underlying storage caches remain uncontrolled."
                  if args.evict_file_cache else "No cache clearing. One warmup per candidate, map, and thread count."),
        "candidates": [{"name": n, "path": str(p), "mode": m, "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for n, p, m in candidates],
        "maps": [{"path": str(p.resolve()), "bytes": p.stat().st_size, "mtime_ns": p.stat().st_mtime_ns} for p in args.maps],
    })
    records = []
    expected = {}
    for path in args.maps:
        initial = path.stat()
        for threads in args.threads:
            for iteration in range(-1, args.rounds):
                shift = (iteration + 1) % len(candidates)
                order = candidates[shift:] + candidates[:shift]
                if iteration % 2:
                    order = order[::-1]
                for name, binary, mode in order:
                    if args.evict_file_cache:
                        with path.open("rb") as source:
                            os.posix_fadvise(source.fileno(), 0, 0, os.POSIX_FADV_DONTNEED)
                    command = [str(binary), str(path.resolve()), mode, str(threads)]
                    if threads == 1 and args.single_cpu is not None:
                        command = ["taskset", "-c", str(args.single_cpu), *command]
                    timer = args.output / "time.txt"
                    p = subprocess.run(["/usr/bin/time", "-f", "%e,%U,%S,%M", "-o", str(timer), *command], text=True, capture_output=True)
                    if p.returncode:
                        raise RuntimeError(f"Failed: {command}\n{p.stdout}\n{p.stderr}")
                    rows = list(csv.DictReader(io.StringIO(p.stdout)))
                    if len(rows) != 1:
                        raise RuntimeError(f"Expected one output row: {command}")
                    row = rows[0]
                    parity = tuple(int(row[k]) for k in ("nodes", "ways", "relations", "checksum"))
                    key = str(path.resolve())
                    if key in expected and expected[key] != parity:
                        raise RuntimeError(f"Checksum mismatch: {command}: {parity} != {expected[key]}")
                    expected[key] = parity
                    wall, user, system, rss = timer.read_text().strip().split(",")
                    records.append({"map": path.name, "threads": threads, "round": iteration, "candidate": name,
                                    "command": command, "csv": row, "process_wall_seconds": float(wall),
                                    "user_seconds": float(user), "system_seconds": float(system), "max_rss_kib": int(rss), "stderr": p.stderr})
                    save(args.output / "results.json", records)
                    print(f"{path.name} {threads} {iteration} {name}: {row['seconds']}s", flush=True)
        final = path.stat()
        if (initial.st_size, initial.st_mtime_ns) != (final.st_size, final.st_mtime_ns):
            raise RuntimeError(f"Input changed: {path}")
    summary = []
    for path in args.maps:
        for threads in args.threads:
            for name, _, _ in candidates:
                group = [r for r in records if r['map'] == path.name and r['threads'] == threads and r['candidate'] == name and r['round'] >= 0]
                times = [float(r['csv']['seconds']) for r in group]
                summary.append({"map": path.name, "threads": threads, "candidate": name,
                                "median_seconds": statistics.median(times), "min_seconds": min(times), "max_seconds": max(times),
                                "median_rss_kib": statistics.median(r['max_rss_kib'] for r in group)})
    save(args.output / "summary.json", summary)


if __name__ == "__main__":
    main()
