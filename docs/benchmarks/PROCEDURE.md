# C++ and Rust comparison

The Rust executable accepts the original benchmark arguments.
Its output contains entity counts, ID checksums, selected-value checksums, and elapsed times.
The Python runner measures process CPU time and peak resident memory with `/usr/bin/time`.

The main timing uses `total_seconds` from each executable.
It includes reader setup, reading, and teardown.
Both implementations exclude live-memory sampling from this timing.
Process wall and CPU measurements include that sampling.
Earlier recorded Rust measurements predate the sampling and timer changes described below.

```sh
cargo build --release --bin pbf_benchmark
python3 test/benchmark/compare.py \
  --cpp target/comparison/cpp \
  --rust target/release/pbf_benchmark \
  --maps /mnt/maps/berlin-260514.osm.pbf \
  --output docs/benchmarks/local --rounds 3 --threads 1 32
```

The default workloads count entities, decode node positions, and decode entity columns with metadata.
The `full` workload selects node mask 31, way mask 7, and relation mask 31.
It excludes optional way locations and string bytes.
Its tag checksums use string IDs.
The callbacks consume every selected value.

The runner compares equivalent counts and checksums before accepting measurements.
Each sample uses a fresh process.
One warmup precedes three measured rounds by default.
The runner alternates implementation order.
It records the command, binary hash, input metadata, raw output, CPU time, and peak memory.

The counts workload compares entity totals.
The entity workload compares totals and summed IDs.
Column workloads also compare selected-value sums.
These sums do not verify order or entity associations.
The entity comparison does not verify resolved coordinates, tags, members, or metadata.

Inputs must remain unchanged during the benchmark.
The runner does not clear the operating system page cache.
Warmup does not guarantee that a complete planet file remains cached.
Unless separate evidence establishes cache residency, report these runs as uncontrolled-cache measurements.

Use `--workloads counts nodes full entities` to select workloads.
Use `--threads 32` for parallel measurements only.
The `entities` workload uses the resolved entity API without metadata.
The other workloads use the selective block API.

The executable also supports `random` and `random-index` modes.
They use the original SplitMix64 seed and request assignment.
The indexed mode includes index construction in total time.
The C++ implementation shares indexes between readers.
The Rust implementation constructs one index per reader.
Report this difference with random-access results.

## Build and validate the C++ comparison

The build script uses the original project sources and its existing dependency archives.
It creates a new executable without changing the original project.

```sh
sh test/benchmark/build_cpp.sh "$HOME/inputosm" "$HOME/inputosm/build" target/comparison/cpp
python3 test/benchmark/parity.py --cpp target/comparison/cpp --rust target/release/pbf_benchmark
```

The parity script compares all field masks on the two original PBF fixtures.
It also compares random access, indexed access, and resolved entities.
The complete run contains 208 C++/Rust comparisons.


## Original benchmark workflows

The imported runners compare the current C++ and Rust executables.
They copy both binaries into a new output directory and record their SHA-256 hashes.
They retain each command, raw CSV, standard error, input metadata, and summary statistics.
They check counts and checksums across implementations and repetitions.

```sh
python3 test/benchmark/run_comparison.py \
  --cpp target/comparison/cpp --rust target/release/pbf_benchmark \
  --input tests/data/comprehensive.osm.pbf \
  --output target/comparison/basic --threads 1 4 --rounds 3 --requests 256
python3 test/benchmark/run_deferred.py \
  --cpp target/comparison/cpp --rust target/release/pbf_benchmark \
  --input tests/data/comprehensive.osm.pbf \
  --output target/comparison/deferred --threads 1 4 --rounds 3 --requests 256
```

The basic matrix includes entity and block reads with metadata disabled and enabled.
It also includes random reads with and without an index.
The extended matrix adds all 15 nonzero count masks, selected-field workloads, strings, full values, and random count reads.
Its `full-values` case includes optional way locations and string bytes.
This case differs from the earlier `compare.py` workload named `full`.

Use `--only TEXT` to select case names that contain `TEXT`.
Each case has one warmup per implementation and thread count.
The runners alternate implementation order for subsequent rounds.
The default is three measured rounds with 1 and 32 workers.
Each run accepts one input file.
Use a separate output directory for each map.

Random requests use consecutive indexes from 1 through the maximum data-block index.
The runners reject random workloads when unknown blocks interrupt this sequence.
Use a sequential case filter for such files.
They also reject random workloads for files without data blocks.

### Timing boundaries

| CSV field | Scope |
|---|---|
| `setup_seconds` | Reader construction and random-worker preparation. |
| `index_build_seconds` | Concurrent index construction and random-worker startup; also included in setup time. |
| `read_seconds` | Selected read operations and callback work. Random workers complete preparation before this interval. |
| `teardown_seconds` | Explicit reader destruction after memory sampling. |
| `total_seconds` | Setup, reading, and teardown, excluding live-memory sampling. |

The `entities` mode constructs and destroys its reader inside `input_file()`.
Its read time therefore includes that complete operation, and its setup time is zero.
Bulk block reads include worker startup and shutdown within their read time.
Random modes use one reader per worker and retain those readers through live-memory sampling.
Bulk workers can release their buffers before that sample.
The Rust reader retains separate indexes; C++ readers can share an index.

These boundaries replace the earlier Rust benchmark's combined read and destruction interval.
Historical result files remain unchanged.
Do not combine historical and current `read_seconds` values without accounting for the different boundaries.

### Resource counters

The Rust executable obtains Linux counters from `/proc/self/stat`, `/proc/self/status`, and `/proc/self/smaps_rollup`.
It uses `getconf CLK_TCK` once before the measured repetitions.
CPU time uses process user and system ticks, so short runs can report zero.
C++ uses `getrusage`, which has finer CPU-time resolution.
The Rust `max_rss_kib` value comes from `VmHWM`; C++ uses `ru_maxrss`.
Both are process-wide maxima and can include earlier repetitions.

Live-memory columns report RSS, PSS, anonymous PSS, and page-table memory in KiB.
Fault columns report process-counter differences for each repetition.
Resource collection occurs outside the wall-clock read interval.
CPU and fault differences include live-memory sampling.
If a counter is unavailable, the Rust CSV field is empty.
This includes unsupported platforms and restricted `/proc` files.

The runners also sample process RSS, anonymous RSS, and page-table memory every 50 milliseconds on Linux.
These peaks can miss short-lived allocations.
The executable publishes its procfs PID through `INPUTOSM_BENCH_PID_FILE` for namespace-aware sampling.
If no sample exists, the summary reports `null` for that sampled metric.
For available metrics, the summary records the sample count.

Python checks validate argument failures, repeated rows, effective field masks, timing arithmetic, manifests, and retained reports.
The fixture matrix validates execution and output parity; its timings do not establish large-map performance.
