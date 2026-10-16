# C++ and Rust comparison

The Rust executable accepts the original benchmark arguments.
Its output contains entity counts, ID checksums, selected-value checksums, and elapsed times.
The Python runner measures process CPU time and peak resident memory with `/usr/bin/time`.

The main timing uses `total_seconds` from each executable.
It includes reader setup, reading, and teardown.
C++ excludes its live-memory sampling from this timing.
Process wall and CPU measurements include that sampling.
Rust does not perform that sampling.

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
