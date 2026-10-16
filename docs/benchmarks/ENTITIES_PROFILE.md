# Entity workflow profile

This report records the prototype investigation before the public implementation.
See [the implementation measurements](ENTITY_IMPLEMENTATION.md) for current interfaces and later comparisons.

This investigation compares the existing entity API with experimental borrowed views.
The application reads IDs, coordinates, tags, way references, and relation members during each callback.
Metadata decoding remains disabled.
Production library source remains unchanged.

On the planet map, views reduce median Rust elapsed time from 33.59 seconds to 24.98 seconds.
That is 25.7% less time, or 1.34 times the throughput.
The exact-capacity experiment saves 4.6% without changing the API.
Views with the fused decoder reach 24.26 seconds.
The equivalent C++ consumer takes 28.31 seconds.

## Workloads

The original `entities` benchmark constructs entity objects but consumes only IDs and counts.
The original `full` benchmark consumes numeric columns, including metadata.
It does not resolve tag strings for each entity.
These workloads measure different operations.

The [new consumer](../../test/profile/entity_profile.rs) reads these fields:

- Node IDs, normalized coordinates, tag keys, and tag values.
- Way IDs, tag keys, tag values, and ordered node references.
- Relation IDs, tags, member IDs, member types, and role strings.

The consumer calculates one ordered checksum per entity.
It adds these checksums across entities because parallel callback order can change.
The checksum includes entity IDs and string lengths.
It supplies comparison evidence, but it is not a proof of identical data.
The consumer does not read metadata or optional way node locations.

The [C++ consumer](../../test/profile/entity_profile.cpp) performs the same calculation.
The measured maps produce matching counts and checksums.
The C++ entity API does not normalize arbitrary PBF coordinate parameters.
Thus, this cross-language comparison applies to the measured maps.

## Data flow and object size

The C++ entity decoder fills reusable entity buffers directly.
Its dense-node decoder reads IDs and coordinates together for the common packed representation.
It clears buffers between groups and retains their capacity.

The Rust entity adapter first decodes reusable numeric columns.
It then allocates tag views and entity objects for each group.
Relations also require a member-object array.
Tag strings borrow decompressed bytes; the adapter does not copy their contents.
The additional cost comes from array construction, string-ID resolution, and object fields.

| Object | C++ bytes | Rust bytes |
|---|---:|---:|
| Node | 56 | 72 |
| Way | 56 | 72 |
| Relation | 56 | 72 |

These sizes apply to the measured Linux x86-64 builds.
Rust entities include a 32-byte `Metadata` value, even when metadata decoding is disabled.
The Rust metadata model supports more fields and wider values than the C++ entity model.
Removing those fields would change the public contract.

The [view prototype](../../test/profile/views.rs) refers to a batch, a string table, and an entity index.
Accessors read the numeric columns directly.
Tag and member iterators resolve string IDs as the consumer reads them.
The prototype does not allocate entity, tag-view, or member-object arrays.
It still allocates decoded columns and the block string table.

## Profiling method

The host is the AMD Ryzen 9 7950X3D used for the [original benchmark](RESULTS.md).
It has 32 logical CPUs and 124.9 GiB of RAM.
The map filesystem uses ext4 on a loop device.
Physical storage performance and unrelated host activity remain uncontrolled.

Passwordless sudo permits process-scoped `perf` measurements.
The profiling commands leave kernel profiling settings unchanged.
The executable is `/usr/lib/linux-tools-6.8.0-147/perf`.
Rust profile builds use release optimization, thin LTO, one code generation unit, and line debug information.
The baseline uses the default libdeflate backend.

The CPU profiles sample user-space cycles.
Romania uses one worker, three iterations, and a 499 Hz sampling frequency.
Germany uses 32 workers, three iterations, and a 299 Hz sampling frequency.
Both profiles use the original ID-consuming entity benchmark.
Neither profile lost samples.

| Sampled function group | Romania, one worker | Germany, 32 workers |
|---|---:|---:|
| Main DEFLATE decompressor | 41.81% | 34.27% |
| Ordinary entity column decoder | 16.03% | 22.20% |
| Dense delta decoder | 14.61% | 10.50% |
| Remaining dense decoder | 4.57% | 4.35% |
| Inlined block/entity adapter function | 16.44% | 17.41% |
| Tag-view collection function | 2.13% | 4.51% |

These are sampled function costs, not independently timed stages.
The inlined adapter function includes several operations.
It cannot establish an exact object-construction percentage.
DEFLATE table construction adds cost outside the main decompressor row.
The [raw reports](entities-profile/profiles/) retain other functions and sampling totals.

[Callgrind](https://valgrind.org/docs/manual/cl-manual.html) counted 7,033,515,582 instructions for the Berlin ID-consuming run.
Its count and checksum match the normal benchmark.
Its elapsed time is not a native performance measurement.
Valgrind reported a `brk` growth warning, then completed with exit status zero.

## Hardware counters and allocations

Three process runs measured each complete callback-data consumer on Romania.
Each process used CPU 0 and one worker.
These diagnostic runs are separate from the timing comparisons.

| Counter, mean of three runs | Rust entities | Rust views | C++ entities |
|---|---:|---:|---:|
| Instructions | 25,680,661,276 | 22,559,657,801 | 23,172,250,039 |
| Cycles | 9,386,991,666 | 8,225,268,748 | 9,267,365,993 |
| Cache misses | 15,860,533 | 6,663,394 | 7,877,620 |
| Branch misses | 103,922,200 | 101,928,515 | 102,802,673 |

Views execute 12.2% fewer instructions and report 58.0% fewer cache misses than Rust entities.
Branch misses change little.
These counters support reducing array construction and memory traffic.
They do not measure DRAM bandwidth or establish a memory-bandwidth limit.
The [counter reports](entities-profile/profiles/) include run variation.

The [original ID-consuming workload repeat](entities-profile/id-reproduction/summary.json) remains separate from the complete-consumer measurements.
It uses Romania, one and 32 workers, one warmup, and three measured processes per implementation.
All counts and ID checksums match.

The [allocation instrument](../../test/profile/allocations.rs) wraps Rust's system allocator.
It counts successful Rust allocation requests during a separate one-worker Romania run.
It excludes direct C allocator calls inside libdeflate and other native libraries.
Do not use its timing results for speed comparisons.

| Allocation measure | Rust entities | Rust views |
|---|---:|---:|
| Allocation calls | 41,649 | 31,176 |
| Reallocation calls | 67,260 | 30,290 |
| Cumulative requested bytes | 4,891,601,588 | 155,813,761 |
| Peak live requested bytes | 36,867,350 | 25,805,587 |

Cumulative requested bytes include the complete new size of each successful reallocation.
They do not represent copied bytes, resident memory, or DRAM traffic.
Peak live requested bytes exclude allocator overhead and native allocations.
The measurements cover program setup and output allocations as well as reading.

## Stage measurements

The [stage patch](../../test/profile/patches/stages.patch) adds thread-local wall-clock timers to an isolated source copy.
The [stage runner](../../test/profile/stages.py) checks worker records and entity/view checksums.
It keeps these diagnostic runs separate from native timing comparisons.
Instrumentation and changing cache conditions prevent direct speed claims from these runs.

For Romania with one worker, the existing entity path has this breakdown:

| Stage | Seconds |
|---|---:|
| Header processing under the mutex | 0.0121 |
| Compressed-buffer resize and payload read | 0.0982 |
| Blob parsing, output-buffer resize, and decompression | 0.6604 |
| String-table construction | 0.0063 |
| Column decoding and remaining decode overhead | 0.5211 |
| Entity adapter, excluding application callback | 0.2715 |
| Application callback | 0.1992 |

Total instrumented elapsed time is 1.7711 seconds.
The adapter accounts for about 15.3% of that time.
It includes object construction, string resolution, coordinate normalization, and temporary-array destruction.
Views still perform string resolution and coordinate normalization when the application reads those fields.

The corresponding view callback takes 0.2818 seconds.
The old adapter and application callback together take 0.4707 seconds.
Thus, removing the adapter does not remove all its work.
Some work moves into view accessors.
The [raw stage records](entities-profile/stages/results.json) retain the full breakdown.

The first planet stage pair has a different limiting factor:

| Diagnostic mode, 32 workers | Elapsed seconds | Summed mutex-wait seconds | Summed header-processing seconds |
|---|---:|---:|---:|
| Entities | 56.03 | 782.52 | 44.46 |
| Views | 67.09 | 1,203.35 | 60.43 |

Wait durations add across workers and can exceed elapsed time.
Header processing holds one shared mutex, so those intervals serialize.
These runs locate a substantial delay in that serialized operation.
They do not establish its exact cause.
Payload reads, scheduling, and cache conditions also vary.
This diagnostic pair is not evidence that views improve every scan.

The current reader holds the mutex across seeking, prefix reads, and header reads.
A delayed header read can therefore prevent other workers from obtaining their next block.
The profile makes bulk read-ahead and header scheduling a separate optimization candidate.

### Cache-residency probe

The [Linux cache probe](../../test/profile/cache_residency.py) calls `mincore` without reading mapped file contents.
It uses a private mapping and leaves the input file unchanged.
The probe ran with the authorized passwordless sudo.
Each result describes residency at that instant; it does not pin pages in memory.

| Probe point | Planet pages resident |
|---|---:|
| Before the additional C++ scan | 58.1794% |
| After the C++ scan | 99.9656% |
| After the following instrumented Rust scans | 100.0000% |

The [probe records](entities-profile/cache-probe/) establish a large residency change across the C++ scan.
The following Rust stage measurements then show much less serialized delay:

| Diagnostic mode after C++ | Elapsed seconds | Summed mutex-wait seconds | Summed header-processing seconds |
|---|---:|---:|---:|
| Entities | 33.62 | 1.71 | 1.32 |
| Views | 25.65 | 1.14 | 0.80 |

The [stage records after C++](entities-profile/stages-after-cpp/results.json) retain the remaining durations and matching checksums.
This sequence supports cache-sensitive header delays as a performance concern.
It does not isolate every storage or scheduler effect.
The experiment does not establish why the readers leave different cache residency.
Cross-implementation warmup can therefore affect later measurements.
The main timing table describes the recorded warmup sequence, rather than a guaranteed standalone Rust scan time.

## Experiments

The [variant builder](../../test/profile/build_variant.py) copies source into `target/entity-profile/variants/`.
It applies isolated patches and runs the existing tests before release compilation.
It does not change production source.

| Candidate | Change | Public API effect |
|---|---|---|
| `baseline` | Existing Rust entity adapter. | None. |
| `reserve` | Reserve exact tag and member capacity before insertion. | None. |
| `fused` | Decode three dense numeric streams in one loop. | None. |
| `views` | Read columns through entity accessors and iterators. | Experimental additional interface. |
| `fused-views` | Combine the dense-loop patch with views. | Experimental additional interface. |
| `cpp` | Original C++ reader with the equivalent consumer. | None. |

The dense-loop experiment retains checked varints and checked delta addition.
It retains the existing fallback for fragmented or unpacked representations and partial field selections.
Its error wording can differ when an array ends early.
The exact-capacity experiment retains string-ID checks.

The view experiment supports this PBF workload with metadata disabled.
Its constructors require complete column selection and valid indices.
It is not a supported public API or an XML implementation.
Unlike the current adapter, views can report string or coordinate errors during field access.
Unaccessed strings can therefore remain unchecked.
A production design must specify validation timing before it exposes these accessors.

## Timing results

### Planet

The planet file contains 94,746,421,723 bytes.
These measurements use 32 workers and three measured processes after one warmup per candidate.
Times are in seconds.

| Candidate | Median | Range | Time reduction against Rust entities |
|---|---:|---:|---:|
| C++ entities | 28.3054 | 27.7070–28.7889 | 15.7% |
| Rust entities | 33.5925 | 32.6661–36.6308 | — |
| Rust exact capacity | 32.0510 | 31.7888–32.0835 | 4.6% |
| Rust views | 24.9760 | 24.6684–25.2494 | 25.7% |
| Rust fused decoder with views | 24.2603 | 24.1302–24.3838 | 27.8% |

All candidates count 10,846,489,004 nodes and produce matching complete-consumer checksums.
The [planet records](entities-profile/planet/results.json) also retain way and relation counts.
The fused decoder reduces the view median by another 2.9% on this map.
That smaller result does not establish a universal improvement.

Warmup times range from 47.18 to 65.17 seconds.
Measured runs range from 24.13 to 36.63 seconds across candidates.
This large change shows that warmup and cache conditions matter.
It prevents direct comparison with an arbitrary cold scan.
Warmup CPU time indicates about 10 to 14 busy CPUs on average, despite 32 workers.
Later measured runs use substantially more CPU capacity.

The recorded cgroup hierarchy has no CPU quota and reports no quota throttling.
These observations suggest waiting during warmup, but they do not identify its exact cause.
The experiment does not isolate disk waits, cache effects, or unrelated host activity.

Median peak resident memory is about 682 MiB for Rust entities and 606 MiB for views.
The exact-capacity candidate reaches about 722 MiB.
Fewer allocation requests do not guarantee lower process resident memory.
Allocator retention, fragmentation, worker scheduling, and batch sizes can affect this measure.

### Regions

The following regional medians use 32 workers and three measured processes after warmup.
Times are in seconds.

| Map | C++ | Rust entities | Exact capacity | Views | Fused decoder with views |
|---|---:|---:|---:|---:|---:|
| Berlin | 0.0697 | 0.0670 | 0.0582 | 0.0461 | 0.0460 |
| Romania | 0.1255 | 0.1221 | 0.1170 | 0.1041 | 0.1026 |
| Germany | 1.5680 | 1.5783 | 1.5101 | 1.2785 | 1.2436 |

Views reduce these Rust median times by 31.2%, 14.7%, and 19.0%, respectively.
Exact-capacity allocation reduces them by 13.1%, 4.2%, and 4.3%.
The fused decoder supplies only a small additional change in these view comparisons.
Three repetitions do not establish that small changes will persist.

| Map, 32 workers | Rust entity range | View range |
|---|---:|---:|
| Berlin | 0.0655–0.0673 | 0.0451–0.0502 |
| Romania | 0.1212–0.1248 | 0.1038–0.1049 |
| Germany | 1.5620–1.6159 | 1.2676–1.6210 |

The Germany view range includes one slow run.
The median improvement therefore needs this qualification.
The [regional records](entities-profile/controlled-regions/summary.json) and [Germany records](entities-profile/germany/summary.json) retain every candidate's range.

Single-worker comparisons use CPU 0.
Romania medians are 1.8340 seconds for Rust entities and 1.6452 seconds for views.
Berlin medians are 0.7012 seconds and 0.7087 seconds, respectively.
Berlin has wide overlapping ranges, so this experiment does not establish a single-worker view improvement there.
CPU affinity alone did not remove timing variation.

| Germany workers | Rust entities | Views |
|---|---:|---:|
| 8 | 3.7260 | 3.3756 |
| 16 | 2.1005 | 1.7520 |
| 32 | 1.5783 | 1.2785 |

Thirty-two workers supplied the lowest measured median in this comparison.
At that count, median Rust peak resident memory fell from about 709 MiB to 306 MiB with views.
C++ resident memory also includes mapped input pages.
Do not compare its resident total directly with Rust heap use.

The [initial screening records](entities-profile/regions/) remain available.
The timing tables above use the later regional set and the Germany set.
Source-address processing overlapped the beginning of the initial screening set.
These conclusions exclude its timings.

## Performance options

### Add borrowed entity views

This is the strongest measured direction for the requested consumer.
Keep the existing entity API.
Add entity accessors over the existing column API.
The application can read fields during the callback without creating complete entity arrays.
Rust lifetimes prevent borrowed data from surviving buffer reuse.

The prototype already uses the public `decode_entities_with_strings` operation.
An additional view interface can therefore use the current decoder.
Metadata access, XML support, validation timing, and constructor checks still require design work.
Applications that retain entities must copy their required data into owned storage.

### Reduce serialized header delays

This option preserves the entity interface and addresses a different limit from object construction.
The stage measurements show that header processing can serialize much of a slower scan.
Candidate designs include bounded sequential read-ahead and a producer that supplies prepared block descriptors.
A bulk entity reader can reuse compressed buffers while worker threads decode earlier blocks.

The design must preserve bounded memory use, cancellation, and malformed-input checks.
Framing-only callbacks must retain their deferred-payload behavior.
Indexed random reads need separate handling from sequential bulk reads.
This investigation includes no implementation or measured speedup for the proposed scheduling changes.
For a large scan with incomplete cache residency, this option can deserve priority over decoder changes.

### Decode directly into reusable entity buffers

This option preserves the current public entity model.
It would remove the intermediate numeric columns from the entity path.
The C++ implementation demonstrates this architecture.
The Rust implementation must preserve its wider metadata, coordinate normalization, and validation rules.

Buffer reuse needs a sound lifetime design because entity fields borrow strings and auxiliary arrays.
Do not extend references beyond their backing buffers.
A separate direct decoder also duplicates some column-decoder logic.
This investigation includes no prototype or speed measurement for this option.

### Read encoded dense nodes through a forward iterator

This option could avoid both entity arrays and decoded numeric columns.
A forward iterator would decode ID, latitude, longitude, and tag data as the caller advances.
It fits a consumer that processes every entity once.

Delta encoding makes arbitrary backward access expensive without stored state.
Repeated field reads need stable values for the current entity.
Fragmented protobuf fields and malformed data still need correct handling.
This investigation includes no measurement for this option.
It requires a larger API change than views over decoded columns.

### Specialize the numeric decoder

Ordinary entity decoding consumes 16% to 22% of the sampled cycles.
Dense delta decoding consumes another 11% to 15%.
Candidate changes include specialized common field selections and faster checked varint decoding.
SIMD decoding would need scalar handling for tails, overflow, and unusual representations.

The fused-loop experiment alone does not establish a large improvement.
Further work must measure the hot instructions before adding another decoder implementation.
Code size and instruction-cache effects can offset fewer branches.

### Change decompression or worker count

Decompression remains a large cost after entity-array removal.
The [previous backend comparison](RESULTS.md) favored libdeflate over the tested pure Rust backend.
Unless a new comparison establishes an improvement, retain that measured default.
The application can still select the pure Rust backend for deployment needs.

For illustration, halving a stage that occupies 43% of runtime saves 21.5% of total runtime.
The corresponding speedup is about 1.27 times, assuming other costs remain fixed.
Eliminating that stage entirely would still leave 57% of runtime.
These calculations illustrate a limit; sampled cycle shares are not exact elapsed-time fractions.

Thread count also needs measurement on the intended machine and consumer.
More workers can increase memory pressure and scheduling cost.
The Germany experiment compares 8, 16, and 32 workers.
It does not establish an optimal count for another CPU or application.

Changing the allocator alone is a lower priority.
Allocator functions did not dominate the sampled profiles.
Fewer temporary arrays reduce construction work and memory traffic as well as allocation calls.

## Reproduction

Run commands from the repository root.
If necessary, use the task-local Rust installation:

```sh
export RUSTUP_HOME="$PWD/.toolchain/rustup"
export CARGO_HOME="$PWD/.toolchain/cargo"
export PATH="$CARGO_HOME/bin:$PATH"
CARGO_PROFILE_RELEASE_DEBUG=1 python3 test/profile/build_variant.py baseline
sh test/profile/build_cpp.sh
python3 test/profile/build_variant.py reserve
python3 test/profile/build_variant.py fused
python3 test/profile/build_variant.py stages
```

The variant builder extracts the historical production source from Git revision `fe524f7`.
It requires a destination that does not exist.
Preserve previous experiment directories before another build.
The C++ build uses existing dependencies from `~/inputosm/build`.
It uses Clang 19, `-O3`, debug information, and no LTO.
The original benchmark report describes the compiler and native backend differences.

Use the instrumented variant only for stage analysis:

```sh
python3 test/profile/stages.py \
  --binary target/entity-profile/variants/stages/target/release/entity_profile \
  --case /mnt/maps/romania-260701.osm.pbf 1 \
  --output target/entity-profile/stage-reproduction
```

The stage records contain nanoseconds for each worker.
`decode_total` includes string-table construction and the group consumer.
For entities, the group consumer includes both the adapter and application callback.
The runner subtracts nested durations to derive column and adapter totals.
Elapsed durations include scheduling delays and waits; they are not CPU-time counters.

On Linux, use the read-only cache probe when needed:

```sh
sudo -n python3 test/profile/cache_residency.py /mnt/maps/planet-260907.osm.pbf
```

Elevated permission avoids kernel restrictions that can conceal residency for files owned by another user.

Use a new result directory for each timing experiment:

```sh
python3 test/profile/compare.py \
  --candidate cpp target/entity-profile/cpp entities \
  --candidate baseline target/entity-profile/variants/baseline/target/release/entity_profile entities \
  --candidate views target/entity-profile/variants/baseline/target/release/entity_profile views \
  --maps /mnt/maps/romania-260701.osm.pbf \
  --threads 1 32 --single-cpu 0 --rounds 3 \
  --output target/entity-profile/reproduction
```

The runner uses one warmup and three measured processes per candidate and configuration.
It rotates and reverses candidate order.
It checks counts and checksums across candidates, repetitions, and worker counts.
It records binary hashes, input sizes, timestamps, process CPU time, and peak resident memory.
It does not clear the page cache.

Use the original workload for the sampled CPU profile:

```sh
sudo -n /usr/lib/linux-tools-6.8.0-147/perf record \
  -F 499 -e cycles:u --call-graph dwarf,8192 \
  -o target/entity-profile/reproduction.perf -- \
  target/entity-profile/variants/baseline/target/release/pbf_benchmark \
  /mnt/maps/romania-260701.osm.pbf entities 1 3
sudo -n /usr/lib/linux-tools-6.8.0-147/perf report \
  -i target/entity-profile/reproduction.perf \
  --stdio --no-children -g none
```
