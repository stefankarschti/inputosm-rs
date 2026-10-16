# Entity implementation measurements

This change adds borrowed PBF entity views, exact-capacity arrays, sequential buffering, and optional memory mapping.
The default reader retains deferred file access.
The regional measurements favor views with that default.
Neither new file access option improves the measured Germany workload.

The [previous investigation](ENTITIES_PROFILE.md) describes the prototypes and profiling evidence.
This report measures the public implementation.

## Implemented interfaces

- `PbfBlock::decode_entity_views` decodes fields from the entity API without constructing entity arrays.
- `PbfReader::read_entity_views` adds parallel scans with callback context.
- `PbfReadMode::Sequential` reads compressed blocks through a bounded one-MiB buffer.
- `PbfReader::open_mmap` maps an unchanged file when the `mmap` feature is enabled.
- The existing entity adapter reserves the known tag and member counts.

Views support metadata and username access.
They omit optional way locations; the column API supplies those values.
String lookups and normalized numeric conversions can fail when accessors read those fields.
Private constructors prevent incomplete column selections and invalid entity indices.
The [contract](../development/ENTITY_IMPLEMENTATION.md) records these limits.

Mapped readers borrow compressed bytes directly.
The operating system loads file pages as necessary.
The unsafe constructor requires an unchanged backing file until the reader drops.
This follows the [memmap2 safety contract](https://docs.rs/memmap2/0.9.11/memmap2/struct.MmapOptions.html).
Mapping needs sufficient virtual address space.

Sequential buffering reads each complete compressed block under the framing lock.
Other workers decode previous blocks outside that lock.
The shared buffer uses one MiB; each worker retains its compressed buffer.
This simple design avoids an additional producer thread and queue.
It does not remove serialized file access.

## Method

All timings use 32 workers on the same Ryzen 9 7950X3D host as the previous investigation.
Each configuration has one preliminary process and three measured processes.
The runner rotates and reverses candidate order.
The ordinary comparison does not evict cached pages.
Preliminary processes precede measured processes but do not establish identical cache residency.

The consumer reads IDs, normalized coordinates, all tag bytes, ordered way references, and relation members.
It leaves metadata and optional way locations disabled.
All compared counts and complete-consumer checksums must match.
The checksum supplies comparison evidence, not a proof of identical data.
Fixture tests compare complete field values, including metadata.

The baseline binary contains the implementation before these changes.
The current entity candidate includes exact-capacity arrays and the new reader scheduling branches.
Thus, its timing difference does not isolate allocation changes alone.
All view candidates use the same production binary and differ only in file access mode.
The C++ binary uses the same consumer as the previous investigation.

Rust 1.99.1 uses release optimization, thin LTO, one code generation unit, and line debug information.
The build enables `mmap` and the default libdeflate backend.
Rust uses libdeflate 1.26 through `libdeflate-sys` 1.26.1.
C++ uses Clang 19, `-O3`, and libdeflate 1.26 without LTO.

The [source manifest](entity-implementation/source-manifest.json) and per-experiment manifests record provenance.
The source manifest also identifies later comment, formatting, and historical-builder changes.
Those changes do not alter the measured production behavior.

The input filesystem uses ext4 on a loop device.
Underlying storage caches and unrelated host activity remain uncontrolled.
Do not interpret these measurements as universal storage rankings.

## Regional results

Values are median elapsed seconds from three measured processes.
The [regional records](entity-implementation/regions/results.json) retain individual timings, CPU time, and peak resident memory.

| Candidate | Berlin | Romania | Germany |
|---|---:|---:|---:|
| Previous Rust entities | 0.0638 | 0.1236 | 1.5751 |
| Current Rust entities | 0.0569 | 0.1212 | 1.5825 |
| Views, deferred reads | 0.0469 | 0.1097 | 1.4235 |
| Views, sequential buffer | 0.0627 | 0.1717 | 2.3555 |
| Views, memory mapping | 0.0475 | 0.1205 | 1.6566 |
| C++ entities | 0.0657 | 0.1305 | 1.6168 |

The Germany deferred-view range is 1.3342–1.5755 seconds.
The previous entity range is 1.5736–1.5828 seconds.
These ranges overlap slightly; the median alone does not describe every run.
The current entity median has no clear Germany improvement.

## Germany after file-cache eviction requests

Before every process, the runner requests `POSIX_FADV_DONTNEED` for that input file.
It does not change file contents or clear global caches.
This request does not control caches below the mapped filesystem.
A separate privileged `mincore` probe confirmed zero resident pages immediately after the same request.
The probe checked 1,183,491 pages; it did not check every timed process.

| Candidate | Median seconds | Range |
|---|---:|---:|
| Views, deferred reads | 2.7112 | 2.6949–2.9659 |
| Views, sequential buffer | 3.3913 | 3.2501–3.4976 |
| Views, memory mapping | 3.7001 | 3.4696–4.1128 |
| C++ entities | 3.3998 | 3.3741–3.4488 |

Deferred views remain the fastest measured option in this set.
The result does not support a default switch to memory mapping or sequential buffering.
The [evicted-input records](entity-implementation/germany-evicted/results.json) contain all preliminary and measured processes.

## Planet results

The planet comparison uses the 94,746,421,723-byte input file.
All candidates produce matching entity counts and complete-consumer checksums.

| Candidate | Median seconds | Range | Time change against previous Rust |
|---|---:|---:|---:|
| Previous Rust entities | 33.4499 | 33.1563–34.0487 | — |
| Current Rust entities | 32.0618 | 31.8153–33.4201 | -4.1% |
| Views, deferred reads | 27.1501 | 26.9640–27.2471 | -18.8% |
| Views, sequential buffer | 35.4207 | 33.1587–35.5540 | +5.9% |
| Views, memory mapping | 29.3651 | 29.2900–30.2557 | -12.2% |
| C++ entities | 28.7484 | 28.1408–28.9940 | -14.1% |

Deferred views reduce median elapsed time by 18.8% against the previous Rust entity implementation.
They reduce the median by 15.3% against current entity objects.
Their median is 5.6% below C++, with nonoverlapping measured ranges in this experiment.
Mapped views take 8.2% more time than deferred views.
Sequential buffering takes 30.5% more time than deferred views.

The preliminary C++ and mapped-view processes take 80.27 and 70.20 seconds, respectively.
Later preliminary processes range from 27.37 to 34.61 seconds.
Measured processes range from 26.96 to 35.55 seconds across candidates.
These differences reinforce the cache and run-order limitations.
The planet comparison does not request cache eviction.

The [planet records](entity-implementation/planet/results.json) retain all 24 processes.
Together, the three experiments contain 112 processes: 84 measured processes and 28 preliminary processes.
All counts and checksums match within each map and across the two Germany experiments.


## Memory interpretation

Mapped-reader resident memory includes mapped input pages.
It cannot serve as a direct comparison of Rust heap allocation.
Sequential and deferred readers retain reusable worker buffers.
The raw records contain peak resident memory for each process.
Planet median peak resident memory is approximately 679 MiB for previous entities and 606 MiB for deferred views.
Mapped views reach approximately 88.7 GiB, including resident input pages.

## Reproduction

Use the task-local toolchain or another supported Rust installation.
Run commands from the repository root.

```sh
cargo test --features mmap
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --features mmap --bin entity_performance --target-dir target/entity-implementation
python3 test/profile/compare.py \
  --candidate views target/entity-implementation/release/entity_performance public-views \
  --candidate sequential target/entity-implementation/release/entity_performance sequential-views \
  --candidate mapped target/entity-implementation/release/entity_performance mapped-views \
  --maps /mnt/maps/germany-260925.osm.pbf --threads 32 --rounds 3 \
  --output target/entity-implementation/reproduction
```

For file-cache eviction requests, add `--evict-file-cache` and use a new output directory.
Keep mapped input files unchanged for the complete benchmark process.
The runner checks input size and modification time after each map comparison.
Those checks detect some changes; they do not enforce mapping safety.

The historical prototype builder now extracts baseline source from Git revision `fe524f7`.
This preserves applicability of the original profiling patches after the production changes.
Its output directories remain isolated under `target/entity-profile/variants`.

## Decision

Use borrowed views when the consumer reads fields during its callback.
Use the existing entity API when the application requires assembled entity slices.
Retain deferred reads as the default on this host.
Select mapping only when the application can guarantee an unchanged file.
Measure each file access option on the deployment system before selecting it for performance.
