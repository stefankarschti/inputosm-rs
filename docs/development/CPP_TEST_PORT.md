# C++ examples, tests, and benchmarks

This document maps the original `inputosm/test` programs to this repository.
The port retains the original executable names.
Cargo discovers the Rust examples in `examples/`.
The new examples, tests, and benchmark tools add no Cargo dependencies.

## Runnable examples

Build the examples before direct execution.

```sh
cargo build --release --examples
cargo run --release --example count_entity -- tests/data/comprehensive.osm.pbf
cargo run --release --example statistics -- tests/data/comprehensive.osm.pbf 1
cargo run --release --example read_block -- tests/data/comprehensive.osm.pbf 1 --index
```

| C++ program | Rust example | Operation |
|---|---|---|
| `count_blocks.cpp` | [count_blocks.rs](../../examples/count_blocks.rs) | Count data blocks without decoding their contents. |
| `count_entity.cpp` | [count_entity.rs](../../examples/count_entity.rs) | Count nodes, ways, and relations through selective decoding. |
| `read_block.cpp` | [read_block.rs](../../examples/read_block.rs) | Read one block, with optional index construction. |
| `custom_log.cpp` | [custom_log.rs](../../examples/custom_log.rs) | Install a reader diagnostic callback. |
| `lat_stat.cpp` | [lat_stat.rs](../../examples/lat_stat.rs) | Count nodes by absolute latitude in whole degrees. |
| `statistics.cpp` | [statistics.rs](../../examples/statistics.rs) | Calculate entity totals and complete-block maxima. |
| `extract_ferries.cpp` | [extract_ferries.rs](../../examples/extract_ferries.rs) | Find ferry ways, then collect referenced node coordinates. |
| `export_csv.cpp` | [export_csv.rs](../../examples/export_csv.rs) | Write the original five CSV files. |
| `export_db.cpp` | [export_db.rs](../../examples/export_db.rs) | Write the original three binary files. |

The existing `count_entities` example uses the complete entity API and also accepts XML.
The imported `count_entity` example uses PBF count operations.
The two examples demonstrate different levels of decoding.

The seven PBF examples accept one input path.
`read_block` also requires a block index and accepts `--index`.
`statistics` enables metadata when a second argument is present, as the C++ example does.
The exporters accept PBF, XML, or OSC input and require a new output directory.
They reject an existing output directory.

```sh
cargo run --release --example export_csv -- tests/data/sample.osm target/sample-csv
cargo run --release --example export_db -- tests/data/sample.osm target/sample-binary
```

These commands require unused output directory names.
The programs can leave partial output after an error.
Before a successful exit, the program flushes each output buffer and synchronizes its file.
The output order can differ between runs because PBF workers execute concurrently.
Rust creates empty output files when a record type is absent.
OSC exports retain the original behavior: they serialize entities without an operation column.

### Rust adaptations

PBF callbacks can run concurrently.
The count and statistics examples store separate results for each worker.
A `Mutex` supplies exclusive access to each worker's state.
The programs combine those results after all callbacks finish.
They acquire locks once per block, rather than once per entity.

Both exporters use `BufWriter`, which combines small writes into larger file writes.
They acquire an output mutex once per callback batch.
They do not retain complete output files in memory or require temporary worker files.
CSV export still retains node coordinates for way geometry.
This coordinate table can consume substantial memory on large maps.

The ferry example owns only selected way references and node coordinates.
Borrowed PBF slices cannot outlive their callback.
The example copies references only for matching ferry ways.
It normalizes coordinates through the block parameters and reports absent referenced nodes.
It records each ferry once, including ways with duplicate matching tags.

The latitude example selects latitude without IDs.
It uses checked integer conversion before assigning a degree bucket.
The statistics example combines all primitive groups before calculating per-block maxima.
Its timestamp conversion uses `i128` arithmetic before conversion to `i64` seconds.
This avoids intermediate millisecond overflow.

The log example uses a reader-specific callback and an ISO-style UTC date.
It accepts PBF because the Rust XML reader has no diagnostic callback API.
The original global C++ logger also supported XML operations.

### Export format contracts

The binary files are `node`, `way`, and `relation`.
Integers use explicit little-endian encoding, which matches the original format on the tested x86-64 machine.
The original C++ program uses native byte order.
Neither format contains a version header.

| Record | Fields in order |
|---|---|
| Node | `i64` ID; two `i32` coordinates; `i16` tag count; tag strings. |
| Way | `i64` ID; `i16` reference count; `i64` references; `i16` tag count; tag strings. |
| Relation | `i64` ID; `i16` member count; members; `i16` tag count; tag strings. |
| Member | `i64` ID; NUL-terminated role; one type byte: node 0, way 1, relation 2. |
| Tag | NUL-terminated key followed by NUL-terminated value. |

Coordinates use units of 100 nanodegrees.
Binary export rejects lengths above 32767, coordinates outside `i32`, and embedded NUL bytes in strings.
The C++ program can truncate these values.
Binary export omits metadata, so the Rust example disables metadata decoding.

CSV export preserves the original filenames, fields, punctuation, and seven decimal coordinate places.
The files are `nodes.csv`, `ways.csv`, `way_node.csv`, `relations.csv`, and `relation_members.csv`.
The format places latitude before longitude in geometry text.
Missing referenced nodes produce `0.0000000 0.0000000`, as in C++.
The format retains the original unescaped tag and role text.
It is a custom format, not a general CSV interchange specification.

The Rust entity API normalizes coordinates and timestamps when PBF block parameters differ from their defaults.
This existing API difference also applies to exported values.
The comparison fixtures use default parameters.

## Unit-test coverage

The port combines existing Rust coverage with additional tests.
Tests compare observable behavior instead of C++ allocation layouts or global variables.
The following table accounts for every original unit-test source file.

| C++ unit-test source | Rust coverage |
|---|---|
| `pbf_block_test.cpp` | [pbf_block.rs](../../tests/pbf_block.rs) covers layout, string bytes, growth, compression buffers, numeric boundaries, column errors, and truncation. [read.rs](../../tests/read.rs) covers fixtures, selected columns, compression errors, cancellation, and normalized values. |
| `pbf_reader_test.cpp` | [pbf_reader.rs](../../tests/pbf_reader.rs) covers ownership, indexes, descriptors, failed-read recovery, nesting, concurrency, file replacement, large offsets, and many groups. Existing reader and entity tests cover access and adapter results. |
| `pbf_deferred_test.cpp` | `read.rs` covers every field mask, lazy payloads, malformed selected fields, optional way locations, wide metadata, and batch kinds. `pbf_reader.rs` covers complete header fields and callback nesting. |
| `pbf_allocation_test.cpp` | [allocation.rs](../../tests/allocation.rs) checks 100 repeated indexed compressed-block decodes after buffer preparation. |
| `read_osm_test.cpp` | [xml.rs](../../tests/xml.rs) checks every entity field from `sample.osm`, with metadata enabled and disabled. |
| `read_osc_test.cpp` | `xml.rs` checks every entity field and operation from `sample.osc`. |
| `thread_config_test.cpp` | `time_output_and_thread_bounds` in `read.rs` checks the Rust worker limits. |
| `output_format_test.cpp` | `read.rs` checks UTC output, duration boundaries, and reader logging. Example helper tests check decimal grouping without locale dependence. |

The allocation test counts Rust allocator calls in an isolated test executable.
It does not count allocations inside the native decompressor.
The tested block uses one packed fragment for each dense column.
Fragmented dense fields and string-view tables can still allocate.
The test does not assert zero allocations for complete entity materialization.

The imported tests exposed two implementation differences that this task corrected.
Blob payload size validation now uses the final `BlobHeader.datasize` field value.
Block kinds and single-fragment dense fields no longer require heap allocations.
The compressed-block allocation test changed from five allocator calls per read to zero after buffer preparation.

The Rust API prevents use of a moved reader or expired callback borrow at compile time.
Compile-fail examples verify callback lifetime restrictions.
It returns an error when opening a file fails; it has no separately usable closed-reader state.
The large-offset test uses a sparse file with bounded unknown blocks instead of two oversized blocks.
The reader retains its compressed-block size limit.

Rust preserves supported 64-bit metadata values and checks normalized conversion overflow.
It does not reproduce older C++ metadata narrowing.
Thread configuration remains local to each reader, with a supported range of 1 through 256.
Header access returns a borrow instead of invoking a callback.
Rust diagnostic callbacks have no fixed 511-byte message limit.

Separate readers retain separate file handles and indexes.
These contracts replace C++ tests for global state, closed handles, shared pointers, and header callback re-entry.

### Fixtures and executable checks

[generate_pbf_fixture.py](../../tests/data/generate_pbf_fixture.py) retains the original fixture generator.
It requires the external `osmium` executable and replaces the two checked-in PBF fixtures when run directly.
Normal tests use the checked-in fixtures and do not invoke this generator.
[pbf_test_data.py](../../tests/data/pbf_test_data.py) generates small integration fixtures with Python standard-library modules.

The original `statistics_fixture.cpp` and CMake assertions now run through the Python integration checks.
They verify multiple groups, empty blocks, unknown blocks, nondefault timestamp units, and metadata selection.
The locale checks verify fixed comma grouping.
Optional C++ comparisons check counts, statistics, latitude tables, ferry counts, block reads, and exported records.
They compare unordered records where worker order can differ.

```sh
cargo test --all-targets
cargo test --doc
cargo test --all-targets --no-default-features --features rust-zlib,mmap
cargo build --release --examples --bin pbf_benchmark
python3 test/integration/examples_test.py
python3 test/integration/examples_test.py --cpp-examples "$HOME/inputosm/build/test/integration"
python3 test/benchmark/benchmark_test.py
python3 test/benchmark/parity.py --cpp target/comparison/cpp --rust target/release/pbf_benchmark
```

The optional C++ commands require existing C++ executables.
The [benchmark procedure](../benchmarks/PROCEDURE.md) gives the benchmark build command.
The Python benchmark runners require Python 3.11 or later.

## Benchmark ports

[pbf_benchmark.rs](../../test/benchmark/pbf_benchmark.rs) retains the original modes, field masks, checksums, and random request sequence.
It now also reports the original timing and resource columns.
[run_comparison.py](../../test/benchmark/run_comparison.py) compares six sequential and random cases.
[run_deferred.py](../../test/benchmark/run_deferred.py) extends that matrix to 28 cases.
The port compares the current C++ and Rust implementations instead of three historical C++ revisions.

See the [benchmark procedure](../benchmarks/PROCEDURE.md) for commands, timer boundaries, memory sampling, and platform limitations.
The earlier `compare.py` remains available for the recorded multi-map workloads.
