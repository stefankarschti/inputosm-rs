#!/usr/bin/env python3
"""Record instrumented worker durations separately from native timings."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

FIELDS = ["lock_wait", "framing", "payload_read", "blob_decode", "decode_total",
          "strings", "group_consumer", "entity_handler"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--case", action="append", nargs=2, metavar=("MAP", "THREADS"), required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    cases = [(Path(path).resolve(), int(threads)) for path, threads in args.case]
    if any(not 1 <= threads <= 256 for _, threads in cases):
        parser.error("Invalid thread count")
    if len({(path.stem, threads) for path, threads in cases}) != len(cases):
        parser.error("Output case names must be unique")
    binary = args.binary.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "manifest.json").write_text(json.dumps({
        "binary": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "raw_fields_nanoseconds": FIELDS,
        "cases": [{"path": str(p), "threads": t, "bytes": p.stat().st_size} for p, t in cases],
    }, indent=2) + "\n")
    results = []
    for path, threads in cases:
        expected = None
        for mode in ("entities", "views"):
            command = [str(binary), str(path), mode, str(threads)]
            process = subprocess.run(command, capture_output=True, text=True)
            prefix = f"{path.stem}-{threads}-{mode}"
            (args.output / (prefix + ".csv")).write_text(process.stdout)
            (args.output / (prefix + ".stages.csv")).write_text(process.stderr)
            if process.returncode:
                raise RuntimeError(f"Failed: {command}\n{process.stderr}")
            rows = list(csv.DictReader(io.StringIO(process.stdout)))
            if len(rows) != 1:
                raise RuntimeError("Expected one benchmark row")
            row = rows[0]
            parity = tuple(int(row[k]) for k in ("nodes", "ways", "relations", "checksum"))
            if expected is not None and parity != expected:
                raise RuntimeError("Entity/view checksum mismatch")
            expected = parity
            workers = {}
            for line in process.stderr.splitlines():
                fields = line.split(",")
                if len(fields) != 10 or fields[0] != "profile_thread":
                    raise RuntimeError(f"Unexpected profile line: {line}")
                thread = int(fields[1])
                if thread in workers:
                    raise RuntimeError("Duplicate profile thread")
                workers[thread] = dict(zip(FIELDS, map(int, fields[2:])))
            if set(workers) != set(range(threads)):
                raise RuntimeError("Missing profile thread")
            sums = {key: sum(w[key] for w in workers.values()) / 1e9 for key in FIELDS}
            if sums["decode_total"] < sums["strings"] + sums["group_consumer"]:
                raise RuntimeError("Invalid nested decode timers")
            if sums["group_consumer"] < sums["entity_handler"]:
                raise RuntimeError("Invalid nested consumer timers")
            sums["columns_and_decode_overhead"] = sums["decode_total"] - sums["strings"] - sums["group_consumer"]
            sums["adapter"] = sums["group_consumer"] - sums["entity_handler"] if mode == "entities" else 0
            results.append({"map": path.name, "threads": threads, "mode": mode,
                            "command": command, "csv": row, "worker_seconds": sums})
            (args.output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print(prefix, row["seconds"], flush=True)


if __name__ == "__main__":
    main()
