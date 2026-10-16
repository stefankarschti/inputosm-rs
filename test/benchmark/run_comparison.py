#!/usr/bin/env python3
"""Compare C++ and Rust with the original sequential and random workloads."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import time

PARITY = ("blocks", "max_index", "nodes", "ways", "relations", "checksum", "values_checksum", "strings")
METRICS = ("setup_seconds", "index_build_seconds", "read_seconds", "teardown_seconds", "total_seconds",
           "cpu_seconds", "max_rss_kib", "live_rss_kib", "live_pss_kib", "live_anon_kib", "page_table_kib",
           "minor_faults", "major_faults", "index_bytes", "sampled_rss_kib", "sampled_anon_kib", "sampled_pte_kib")


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def memory(pid):
    try:
        lines = Path(f"/proc/{pid}/status").read_text().splitlines()
    except OSError:
        return {}
    names = {"VmRSS:": "sampled_rss_kib", "RssAnon:": "sampled_anon_kib", "VmPTE:": "sampled_pte_kib"}
    return {names[parts[0]]: int(parts[1]) for line in lines if len(parts := line.split()) >= 2 and parts[0] in names}


def run(command, prefix):
    pid_file = prefix.with_suffix(".pid")
    peaks = {}
    with prefix.with_suffix(".csv").open("w") as output, prefix.with_suffix(".stderr").open("w") as error:
        process = subprocess.Popen(command, stdout=output, stderr=error,
                                   env={**os.environ, "INPUTOSM_BENCH_PID_FILE": str(pid_file)})
        try:
            while process.poll() is None:
                try:
                    pid = int(pid_file.read_text())
                except (OSError, ValueError):
                    pid = None
                if pid is not None:
                    for key, value in memory(pid).items():
                        peaks[key] = max(peaks.get(key, 0), value)
                time.sleep(0.05)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            pid_file.unlink(missing_ok=True)
    if process.returncode:
        raise RuntimeError(f"Command failed ({process.returncode}): {command}; see {prefix}.stderr")
    with prefix.with_suffix(".csv").open() as source:
        rows = list(csv.DictReader(source))
    if len(rows) != 1:
        raise RuntimeError(f"Expected one CSV row: {command}")
    return rows[0], peaks


def jobs(deferred, requests, max_index):
    result = []
    for metadata in (0, 1):
        for mode in ("entities", "blocks"):
            result.append((f"{mode}-metadata-{metadata}", mode, [str(metadata)]))
    for mode in ("random", "random-index"):
        result.append((mode, mode, ["0", str(requests), str(max_index)]))
    if deferred:
        for mask in range(1, 16):
            result.append((f"counts-{mask}", "blocks", [f"--counts={mask}", "--consume=count"]))
        for name, flags in [
            ("nodes-positions", ["--nodes=7"]), ("ways-tags-refs", ["--ways=3"]),
            ("relations-full", ["--relations=31"]), ("strings", ["--strings"]),
            ("full-values", ["--nodes=31", "--ways=15", "--relations=31", "--strings"]),
        ]:
            result.append((name, "blocks", [*flags, "--consume=all"]))
        for mode in ("random", "random-index"):
            result.append((f"{mode}-counts", mode, ["0", str(requests), str(max_index), "--counts=15", "--consume=count"]))
    return result


def signature(row):
    return tuple(int(row[key]) for key in PARITY)


def main(deferred=False):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpp", type=Path, required=True)
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--threads", type=int, nargs="+", default=[1, 32])
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--requests", type=int, default=256)
    parser.add_argument("--only", default="", help="Select case names that contain this text")
    args = parser.parse_args()
    if args.rounds < 1 or args.requests < 1 or any(t < 1 or t > 256 for t in args.threads):
        parser.error("Use positive rounds and requests, with 1..256 threads")
    if len(set(args.threads)) != len(args.threads):
        parser.error("Thread counts must be unique")
    path = args.input.resolve(strict=True)
    binaries = {name: binary.resolve(strict=True) for name, binary in [("cpp", args.cpp), ("rust", args.rust)]}
    selected = [job for job in jobs(deferred, args.requests, 1) if args.only in job[0]]
    if not selected:
        parser.error("The case filter selected no workloads")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    initial = path.stat()
    manifest = {"utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "platform": platform.platform(),
                "arguments": {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()},
                "input": {"path": str(path), "bytes": initial.st_size, "mtime_ns": initial.st_mtime_ns},
                "cache": "No cache clearing. Each case has one warmup per implementation and thread count.",
                "sampling_interval_seconds": 0.05, "binaries": {}}
    for name, binary in binaries.items():
        copied = output / name
        shutil.copy2(binary, copied)
        manifest["binaries"][name] = {"source": str(binary), "copy": str(copied), "sha256": digest(copied)}
        binaries[name] = copied
    save(output / "manifest.json", manifest)
    reference, _ = run([str(binaries["rust"]), str(path), "blocks", "1", "1", "--counts=15"], output / "reference")
    max_index = int(reference["max_index"])
    selected = [job for job in jobs(deferred, args.requests, max_index) if args.only in job[0]]
    if any(mode.startswith("random") for _, mode, _ in selected) and (max_index == 0 or max_index != int(reference["blocks"])):
        raise RuntimeError("Random workloads require consecutive OSMData indexes starting at 1")
    records, expected = [], {}
    for threads in args.threads:
        for case, mode, options in selected:
            for round_index in range(-1, args.rounds):
                order = ("cpp", "rust") if round_index % 2 == 0 else ("rust", "cpp")
                for implementation in order:
                    command = [str(binaries[implementation]), str(path), mode, str(threads), "1", *options]
                    prefix = output / f"{case}-{threads}-{round_index}-{implementation}"
                    row, peaks = run(command, prefix)
                    key = (case, threads)
                    values = signature(row)
                    if key in expected and values != expected[key]:
                        raise RuntimeError(f"Count or checksum mismatch: {prefix}")
                    expected[key] = values
                    records.append({"case": case, "threads": threads, "round": round_index,
                                    "implementation": implementation, "command": command, "csv": row, **peaks})
                    save(output / "results.json", records)
                    print(f"{prefix.name}: {row['read_seconds']} s read", flush=True)
    final = path.stat()
    if (initial.st_size, initial.st_mtime_ns, initial.st_ino) != (final.st_size, final.st_mtime_ns, final.st_ino):
        raise RuntimeError("The input file changed during the benchmark")
    summary = []
    for threads in args.threads:
        for case, _, _ in selected:
            for implementation in binaries:
                rows = [r for r in records if r["round"] >= 0 and (r["case"], r["threads"], r["implementation"]) == (case, threads, implementation)]
                metrics = {}
                for metric in METRICS:
                    values = [float(value) for r in rows if (value := r.get(metric, r["csv"].get(metric))) not in (None, "")]
                    metrics[metric] = {"samples": len(values), "median": statistics.median(values), "min": min(values), "max": max(values)} if values else None
                summary.append({"case": case, "threads": threads, "implementation": implementation, "metrics": metrics})
    save(output / "summary.json", summary)


if __name__ == "__main__":
    main()
