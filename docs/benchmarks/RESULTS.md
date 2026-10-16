# C++ and Rust benchmark results

Rust has lower full-column median times with 32 threads on all four maps.
Performance varies by API and thread count.
The planet entity API takes about 24% more elapsed time in Rust.
All experiments use three measured runs after warmup.

The later [entity workflow profile](ENTITIES_PROFILE.md) examines CPU costs and experimental performance improvements.
It includes a consumer that reads tags, coordinates, and references.
Its workload differs from the ID-consuming entity measurements below.
That investigation also measured a large planet cache-residency change after a C++ scan.
Its cache discussion distinguishes warmed scans from scans with incomplete cache residency.

## Conditions

The tests ran on October 16, 2026, on an AMD Ryzen 9 7950X3D with 32 logical CPUs.
The system reported 124.9 GiB of RAM.
The map filesystem uses ext4 on a loop device.
The physical storage was not characterized.

Each implementation received one warmup and three measured runs for each map, thread count, and workload.
Each run used a fresh process.
The runner alternated implementation order.
Regional tests used one and 32 threads.
Planet tests used 32 threads.

The runner did not clear the file cache or fix CPU frequency and scheduling.
These are uncontrolled-cache measurements.
A warmup does not prove that the complete planet file remains cached.
No builds or other benchmark processes ran during the final measurements.

The main timing is the executable's `total_seconds` value.
It includes reader setup, reading, and teardown.
C++ excludes its live-memory sampling from that value.
Process wall and CPU measurements include that sampling.
Rust does not perform that sampling.

## Builds and inputs

The C++ source revision is `d93762b63398afee948ec0e1ca4e4328bba25177`.
The Rust implementation revision is `8d06b45`.
The [build record](builds.json) and [Rust source hashes](rust-source.json) identify the measured source.
Each experiment manifest records binary hashes, input sizes, modification times, and CPU information.
The sample records contain commands.

| Component | Configuration |
|---|---|
| C++ reader | Clang 19.1.1, `-O3 -DNDEBUG`, no LTO, default target CPU |
| Rust reader | Rust 1.99.1, release optimization, thin LTO, one code-generation unit, default target CPU |
| C++ libdeflate | Version 1.26, Clang 19, `-O3 -DNDEBUG` |
| Rust libdeflate | Version 1.26 through `libdeflate-sys` 1.26.1, GCC 13.3.0, `cc` crate release defaults |
| Pure Rust decompression | `flate2` 1.1.10 with `zlib-rs` 0.6.8 |

These measurements compare these build configurations.
Compiler versions, LTO settings, native C compilers, and file-access methods differ.
The results do not isolate the effect of the programming language.

| Map | File bytes |
|---|---:|
| Berlin | 97,937,692 |
| Romania | 323,127,585 |
| Germany | 4,847,577,526 |
| Planet | 94,746,421,723 |

## Workloads and validation

| Workload | Selected work | Compared output |
|---|---|---|
| `counts` | Count nodes, ways, and relations | Entity totals and block information |
| `nodes` | Decode node IDs and coordinates | Counts, summed IDs, and selected-value sums |
| `full` | Decode node mask 31, way mask 7, and relation mask 31 | Counts, summed IDs, and selected-value sums |
| `entities` | Construct resolved entities without metadata | Entity totals and summed IDs |

The `full` workload includes metadata, tags as string IDs, way references, and relation members.
It excludes optional way locations and string bytes.
Callbacks consume every selected column value.
The entity comparison does not verify resolved coordinates, tags, members, or metadata.
Additive checksums do not verify order or entity associations.
Regression fixtures provide additional correctness checks.

The [procedure](PROCEDURE.md) contains reproduction commands.
The [review record](../development/REVIEW.md) contains test and review coverage.

## Regional timing

Each timing cell gives the median in seconds, followed by the minimum and maximum in brackets.
The ratio is C++ median time divided by Rust median time.
A ratio above one means that Rust has the lower median time.
Small differences can fall within the recorded ranges.

Rust has lower median times in all regional 32-thread workloads.
Several ranges overlap.
Single-thread results vary by workload.
Single-thread entity decoding is slower in Rust on all three regional maps.

| Map | Threads | Workload | C++ seconds | Rust seconds | C++ / Rust |
|---|---:|---|---:|---:|---:|
| Berlin | 1 | `counts` | 0.232 [0.232, 0.233] | 0.228 [0.226, 0.229] | 1.02 |
| Berlin | 1 | `nodes` | 0.276 [0.275, 0.291] | 0.280 [0.277, 0.317] | 0.99 |
| Berlin | 1 | `full` | 0.592 [0.585, 0.619] | 0.573 [0.554, 0.604] | 1.03 |
| Berlin | 1 | `entities` | 0.553 [0.522, 0.561] | 0.572 [0.569, 0.578] | 0.97 |
| Berlin | 32 | `counts` | 0.026 [0.025, 0.026] | 0.024 [0.023, 0.024] | 1.09 |
| Berlin | 32 | `nodes` | 0.025 [0.025, 0.025] | 0.025 [0.025, 0.026] | 1.02 |
| Berlin | 32 | `full` | 0.051 [0.050, 0.053] | 0.046 [0.045, 0.047] | 1.09 |
| Berlin | 32 | `entities` | 0.065 [0.065, 0.067] | 0.058 [0.057, 0.060] | 1.12 |
| Romania | 1 | `counts` | 0.769 [0.764, 0.819] | 0.801 [0.792, 0.808] | 0.96 |
| Romania | 1 | `nodes` | 1.021 [0.994, 1.055] | 1.088 [1.022, 1.096] | 0.94 |
| Romania | 1 | `full` | 1.897 [1.733, 1.974] | 1.875 [1.838, 1.950] | 1.01 |
| Romania | 1 | `entities` | 1.486 [1.476, 1.509] | 1.788 [1.585, 1.805] | 0.83 |
| Romania | 32 | `counts` | 0.084 [0.077, 0.088] | 0.078 [0.076, 0.083] | 1.08 |
| Romania | 32 | `nodes` | 0.069 [0.067, 0.106] | 0.064 [0.063, 0.066] | 1.07 |
| Romania | 32 | `full` | 0.131 [0.130, 0.135] | 0.118 [0.118, 0.119] | 1.11 |
| Romania | 32 | `entities` | 0.122 [0.119, 0.130] | 0.117 [0.116, 0.118] | 1.04 |
| Germany | 1 | `counts` | 11.639 [11.532, 11.723] | 11.849 [11.766, 12.110] | 0.98 |
| Germany | 1 | `nodes` | 14.529 [14.428, 14.875] | 15.198 [14.377, 15.647] | 0.96 |
| Germany | 1 | `full` | 24.320 [24.086, 25.045] | 24.392 [24.191, 24.439] | 1.00 |
| Germany | 1 | `entities` | 21.042 [20.788, 21.526] | 23.319 [23.181, 23.841] | 0.90 |
| Germany | 32 | `counts` | 0.767 [0.762, 0.795] | 0.710 [0.694, 0.711] | 1.08 |
| Germany | 32 | `nodes` | 0.874 [0.842, 0.875] | 0.696 [0.690, 0.713] | 1.26 |
| Germany | 32 | `full` | 1.520 [1.518, 1.552] | 1.310 [1.303, 1.318] | 1.16 |
| Germany | 32 | `entities` | 1.372 [1.359, 1.401] | 1.355 [1.355, 1.464] | 1.01 |

Regional records: [manifest](regions/manifest.json), [samples](regions/results.json), and [summary](regions/summary.json).

## Planet timing

These measurements use 32 threads.
All planet count and checksum comparisons passed.
Both readers reported 10,846,489,004 nodes, 1,216,465,009 ways, and 14,708,384 relations.
The table uses the same timing format as the regional table.

| Workload | C++ seconds | Rust seconds | C++ / Rust |
|---|---:|---:|---:|
| `nodes` | 16.689 [16.552, 16.892] | 16.824 [16.733, 17.431] | 0.99 |
| `full` | 29.954 [29.379, 30.287] | 28.470 [28.191, 28.621] | 1.05 |
| `entities` | 26.561 [26.475, 26.757] | 32.925 [32.797, 33.822] | 0.81 |

Rust uses about 5% less elapsed time for full column decoding.
Its entity API uses about 24% more elapsed time.
Node-decoding ranges overlap.
These results support different choices for different workloads.

Planet records: [manifest](planet/manifest.json), [samples](planet/results.json), and [summary](planet/summary.json).

### Counting variation and repeat

The initial counting measurements varied widely.
Later runs took less time.
The tests did not measure cache residency, so the cause remains unconfirmed.
A second counting experiment followed the other planet workloads.
It used the same binaries and one warmup plus three measured runs per implementation.
Both experiments remain available.

| Counting experiment | C++ seconds | Rust seconds | C++ / Rust |
|---|---:|---:|---:|
| Initial | 14.134 [13.653, 35.708] | 34.373 [13.544, 35.796] | 0.41 |
| Later repeat | 13.565 [13.499, 14.125] | 13.687 [13.543, 13.868] | 0.99 |

The repeat has substantially narrower ranges.
Its ranges overlap.
The original median ratio is not a reliable measure of the implementation difference.

Repeat records: [manifest](planet-counts-repeat/manifest.json), [samples](planet-counts-repeat/results.json), and [summary](planet-counts-repeat/summary.json).

## Planet process CPU time and peak memory

CPU time is the median sum of user and system CPU time across all threads.
Peak RSS is the maximum resident set size across the three measured processes.
The counting row uses the later repeat.
CPU time includes C++ memory sampling.

| Workload | C++ CPU seconds | Rust CPU seconds | C++ peak RSS, MiB | Rust peak RSS, MiB |
|---|---:|---:|---:|---:|
| `counts` | 304.36 | 393.35 | 90719.1 | 272.8 |
| `nodes` | 397.78 | 492.41 | 90707.9 | 286.0 |
| `full` | 797.09 | 845.95 | 90995.7 | 499.8 |
| `entities` | 665.74 | 966.79 | 91244.4 | 680.5 |

C++ maps the input file into its address space.
Its RSS includes resident mapped file pages.
Rust uses reusable input buffers.
The operating system can retain file pages outside the Rust process RSS.
These measurements do not establish an equivalent reduction in total system RAM use.

The JSON summaries also contain CPU time and peak RSS for every regional workload.
The sample records contain whole-process wall times.

## Decompressor comparison

This experiment compares two Rust builds from the same source.
It uses Berlin and Romania, one and 32 threads, and the counts and full workloads.
Each configuration has one warmup and three measured runs per backend.
All 64 process runs passed their count and checksum comparisons.

The runner labels remain `cpp` and `rust` in these raw records.
Here, `cpp` identifies Rust with libdeflate, and `rust` identifies Rust with `zlib-rs`.
The ratio below is the pure Rust median divided by the libdeflate median.
A ratio above one means that libdeflate has the lower median time.

| Map | Threads | Workload | libdeflate seconds | zlib-rs seconds | zlib-rs / libdeflate |
|---|---:|---|---:|---:|---:|
| Berlin | 1 | `counts` | 0.229 [0.228, 0.230] | 0.356 [0.355, 0.358] | 1.55 |
| Berlin | 1 | `full` | 0.552 [0.518, 0.573] | 0.620 [0.617, 0.760] | 1.12 |
| Berlin | 32 | `counts` | 0.030 [0.028, 0.032] | 0.035 [0.032, 0.038] | 1.14 |
| Berlin | 32 | `full` | 0.059 [0.058, 0.060] | 0.069 [0.066, 0.071] | 1.17 |
| Romania | 1 | `counts` | 0.728 [0.725, 0.730] | 1.115 [1.105, 1.118] | 1.53 |
| Romania | 1 | `full` | 1.746 [1.694, 1.925] | 2.069 [2.028, 2.071] | 1.18 |
| Romania | 32 | `counts` | 0.059 [0.057, 0.062] | 0.067 [0.063, 0.068] | 1.13 |
| Romania | 32 | `full` | 0.117 [0.116, 0.117] | 0.136 [0.136, 0.137] | 1.17 |

Libdeflate has the lower median in all eight tested configurations.
The default therefore remains libdeflate.
The `rust-zlib` feature remains available for builds without a C compiler.
These results do not compare every available Rust decompressor.

Backend records: [manifest](decompressors-final/manifest.json), [samples](decompressors-final/results.json), and [summary](decompressors-final/summary.json).

## Record completeness

The C++/Rust experiments contain 232 processes: 58 warmups and 174 measured runs.
These include the separate planet counting repeat.
The backend experiment adds 16 warmups and 48 measured runs.
All comparisons passed.
Input size, modification time, and inode checks passed after each experiment.

The [preliminary records](preliminary/README.md) document earlier development measurements.
The tables above do not use those records.
