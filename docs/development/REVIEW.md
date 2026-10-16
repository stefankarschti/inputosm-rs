# Review record

## Implementation review, cycle one

The independent agent `implementation_review` examined all source modules, tests, benchmark tools, and relevant original source.
The reviewer reproduced three XML defects and identified one missing PBF API capability.
The author confirmed all four findings.

| Severity and finding | Evidence and consequence | Disposition and correction |
|---|---|---|
| P2: XML attribute whitespace | Reproduced: literal whitespace produced tag values that differed from Expat. | Confirmed defect. Use the stricter `xml` event parser. |
| P2: Invisible nodes without coordinates | Reproduced: the reader rejected valid invisible nodes without coordinates. | Confirmed defect. Permit absent coordinates when `visible="false"`. |
| P2: Malformed attributes and comments | Reproduced: the reader accepted invalid attributes, character references, and comments. | Confirmed defect. Use parser validation and add malformed-input tests. |
| P2: Missing group-kind inspection | Source-supported: callers could not identify ordinary, dense, or unselected groups. | Confirmed defect. Expose `BatchContext.kind` and `GroupBatch.kind()`. |

The XML findings concern [the XML reader](../../src/inputosmxml.rs).
The group-kind finding concerns [the public types](../../src/inputosm.rs) and [PBF decoding](../../src/inputosmpbf.rs).

The regression tests cover literal whitespace, character references, invisible nodes with both metadata settings, invalid XML, and unselected groups.
The correction tests pass.
Additional coverage now includes 20 tests and one compile-fail example.
Native Clippy validation passes with warnings treated as errors.

## Implementation review, cycle two

The independent agent `correction_review` examined the corrected implementation with fresh context.
The scope included all source modules, tests, dependencies, examples, benchmark tools, API documents, and relevant C++ source.
The reviewer examined the four corrections and the later performance changes.
These changes include packed metadata decoding, deferred Unix reads, and protobuf function inlining.
The reviewer found no new blocking implementation defect.

The reviewer independently ran both dependency configurations.
Each configuration passed 20 tests and one compile-fail example.
Clippy passed for all targets and features with warnings denied.
Additional isolated probes covered metadata encodings, malformed metadata, deferred payload truncation, XML encodings, and malformed XML.
The reviewer also checked local document links and Git whitespace.

The reviewer identified inaccurate workload wording and paragraphs with more than six sentences.
The author corrected these findings.
The reviewer examined the corrections and found no additional actionable issue.
Large-map measurements were outside this review scope.

## Author validation

Both final backends passed 208 comparisons against the C++ executable.
These comparisons cover field masks, counts, strings, entity callbacks, and indexed and unindexed reads.
The README Rust example compiled.
The benchmark shell script passed syntax validation.
The pure Rust configuration passed a Windows GNU cross-compilation check for all targets.
No Windows execution test was available.

Both configurations also passed 20 tests and one compile-fail example on Rust 1.85.0.
This verifies the declared minimum compiler version on Linux.
The final default release executable matches the measured native executable's SHA-256 hash.
The original C++ repository remains unchanged.

The repeated C++/Rust map experiments completed 232 processes, including warmups and the separate planet counting repeat.
The final backend comparison completed another 64 processes.
All count and checksum comparisons passed.
The [benchmark report](../benchmarks/RESULTS.md) records measurement ranges, build differences, memory interpretation, and workload limitations.

## Final document review

The independent agent `final_document_review` examined the benchmark method and all project documents.
It verified all 296 final process records, all summaries, and all 41 result-table rows.
It checked local links, source hashes, dependency versions, compiler settings, timing boundaries, and field selections.
The reviewer ran no builds, tests, or large-map operations during timed measurements.

The author corrected counter-update descriptions, checksum claims, condition placement, paragraph lengths, and manifest descriptions.
The reviewer verified those corrections.
The final arithmetic and source checks found no numerical or methodological blocker.
The final review also verified three binary hashes and 20 source hashes.
All 12 Markdown files passed local link and anchor checks.

## Language review limitations

The official ASD-STE100 website was unavailable during this task.
The document review applied the project writing rules and defined technical terms.
It could not verify every general word against the complete Issue 9 dictionary.
Sentence-length checks do not establish full compliance.

## Entity profile implementation review

The independent agent `profile_implementation_review` examined the complete profiling change with fresh context.
The scope included Rust and C++ consumers, experimental views, allocation accounting, build scripts, comparison scripts, and decoder patches.
The reviewer also examined the existing Rust and C++ readers.
Production library source remains unchanged.

Cycle one found one source-supported reporting defect.
The comparison runner combined summary records when distinct map paths had identical filenames.
Checksum comparisons used complete paths, so they did not detect this summary error.
The recorded maps have distinct filenames; the defect did not affect these measurements.

The author confirmed the defect and added argument validation that rejects duplicate map filenames.
The regression test fails without that guard and passes with it.
The guard runs before output creation or binary access.
Cycle two examined the correction and its interactions with the original change.
The reviewer found no remaining implementation defect in that scope.

## Entity profile validation

The baseline passed 28 test executions and one compile-fail example.
These include four consumer/view tests in each of two profiling binaries.
Clippy passed for all targets and features with warnings denied.
The isolated exact-capacity and fused-decoder variants passed their existing library and consumer tests before measurement.
The later stage-instrumented variant passed 28 test executions and one compile-fail example.

The main timing sets contain 132 measured processes and 44 warmups.
They cover Berlin, Romania, Germany, and the planet.
Counts and complete-consumer checksums match across candidates, worker counts, and repetitions.
The original ID-consuming workload also passed 16 process comparisons, including warmups.
Eight stage diagnostics passed worker-record, nested-timer, and checksum checks.

The cache probe passed focused checks for page counts, result bounds, unchanged input contents, and empty-file rejection.
Python syntax checks, shell syntax checks, and Git whitespace checks passed.
The source manifest records production source, patch, and profiling-tool hashes.
The experiment manifests record measured binary hashes.

## Entity profile document review

The same independent reviewer compared the draft profile report with source and recorded regional data.
It verified workload differences, sampled-function labels, allocation definitions, timing arithmetic, ranges, prototype limits, and local links.
The author corrected passive sentences, combined instructions, condition placement, imprecise memory rounding, and CPU-affinity wording.

The initial review used source and records only during active measurements.
It did not run builds, tests, or large-map reads.
The complete ASD-STE100 Issue 9 dictionary remains unavailable for exhaustive word verification.
The review applies the project writing rules and technical terms without claiming full dictionary compliance.

Final review found no remaining actionable defect in the profiling tools or report.
The reviewer verified all 176 main process records, their summaries, counts, checksums, arithmetic, and memory conversions.
It checked eight stage diagnostics, including worker identities, timer nesting, sums, and derived durations.
It also verified all 16 original-workload comparisons, source and tool hashes, and local document links.

Independent fixture checks covered four stage runs, the cache probe, and the duplicate-map regression.
The reviewer found no implementation defect in the stage patch, stage runner, cache probe, or builder changes.
The author corrected the warmup CPU range and decompressor recommendation wording.
The reviewer verified both corrections.

The reviewer did not repeat large-map measurements or the complete Rust test suite.
Full ASD-STE100 dictionary compliance remains unverified.

## Entity implementation review

The independent agent `entity_implementation_review` examined the production changes with fresh context.
The scope included borrowed views, capacity changes, file scheduling, memory mapping, dependencies, tests, examples, and benchmark tools.
The reviewer examined lifetimes, malformed inputs, cancellation, resource bounds, and compatibility.
The source review found no runtime defect.

The reviewer identified imprecise documentation about validation, optional way locations, and platform-specific deferred reads.
The author confirmed these findings and corrected the descriptions.
The author also split paragraphs that exceeded the project limit.
The reviewer checked the corrections and analogous architecture and API comments.

The reviewer identified historical reproduction commands that used the current implementation as the earlier baseline.
The author added an isolated baseline build from Git revision `fe524f7` and updated those commands.
The reviewer verified the correction.
No blocking source finding remains.

During active measurements, the reviewer used source inspection and small result records only.
It verified all 88 regional and file-cache-eviction processes, all 22 summary rows, checksums, ranges, and document links.
The reviewer then verified all 24 planet processes, six planet summary rows, arithmetic, memory conversions, and local document links.
All 112 process records have matching counts and checksums for each map.
The reviewer verified the recorded source changes after the benchmark build.
Those changes affect comments, formatting, and the historical builder.

The reviewer independently passed 32 native-backend test executions with mapping enabled and two compile-fail examples.
It also passed 12 complete-consumer comparisons on an isolated 2.34 MB fixture at 1, 4, and 32 workers.
That fixture includes unknown blocks larger than the sequential buffer.
The comparisons covered entity objects, deferred views, sequential views, and mapped views.
No project input or source map changed during these checks.

The reviewer did not repeat large-map timings, alternate-backend builds, or Windows execution.
Full ASD-STE100 dictionary compliance remains unverified.

## Entity implementation author validation

The default build and both decompressor configurations with mapping enabled each passed 32 test executions and two compile-fail examples.
Rust 1.85 passed the same native-backend suite with mapping enabled.
All-target, all-feature Clippy passed with warnings denied.
The pure Rust configuration with mapping enabled passed an all-target Windows GNU compilation check.
No Windows execution environment was available.

The usage example produced identical output with deferred reads, sequential buffering, and memory mapping.
The historical baseline builder passed its tests and release build.
Its consumer matched the retained baseline binary on a fixture.
All three historical patches passed application checks against that baseline.
The comparison-runner regression, Python syntax checks, formatting checks, and Git whitespace checks passed.

The measurements cover Berlin, Romania, Germany, and the planet with three repetitions for each configuration.
The experiments contain 84 measured processes and 28 preliminary processes.
Input sizes and modification times remain unchanged.
The original C++ repository remains unchanged.

## C++ example, test, and benchmark ports

The independent agent `cpp_port_review` examined the complete task changes against revision `e6095f2` with fresh context.
The review included all new files, surrounding decoder code, export formats, unit tests, fixtures, benchmark metrics, and comparison runners.
The reviewer compared the ports with the original C++ source files.
The task adds nine integration examples, additional unit-test coverage, and two benchmark runners without new Cargo dependencies.

The first review reproduced a benchmark deadlock after partial random-worker creation.
A fixed barrier left existing workers waiting when a later thread creation failed.
The author confirmed this defect with an injected `pthread_create` failure and a five-second timeout.
The correction uses fallible thread creation, a readiness channel, and a cancelable start signal.
The new regression test checks both random modes.
It requires Linux and a C compiler; other environments skip this specific fault-injection check.

The second review examined the correction and its interactions with the complete task change.
The reviewer independently injected failures after zero, one, two, and three successful worker starts in both random modes.
All eight cases returned errors without a panic or timeout.
A malformed trailing descriptor also returned an index error without waiting indefinitely.
The reviewer reran all four Python benchmark tests.
No implementation finding remains unresolved after two review cycles.

The author removed an initial CSV text restriction to preserve the original literal output format.
The reviewer verified this change against the C++ exporter.
Binary export retains checked lengths, coordinate conversions, NUL termination, buffered writes, and explicit output synchronization.
The documentation records original format limits and intentional Rust adaptations.

### Validation

The native decompressor with mapping enabled passed 62 test executions and two compile-fail examples.
The pure Rust decompressor with mapping enabled passed the same checks.
Rust 1.85 passed all 62 test executions with the native decompressor and mapping enabled.
All-target, all-feature Clippy passed with warnings denied.
The pure Rust configuration with mapping enabled passed an all-target Windows GNU compilation check.
Windows execution was unavailable.

The example checks passed all five test groups.
These checks include C++ comparisons on both original PBF fixtures and the generated statistics fixture.
They compare CSV lines and parsed binary records independently of worker order.
The benchmark parity script passed all 208 C++/Rust comparisons.
The benchmark checks passed all four test groups, including forced thread-creation failure.

The extended runner completed 336 processes across 28 cases, two worker counts, one warmup, and two measured rounds.
The author repeated the affected random cases after the startup correction.
Those checks produced 48 additional processes with matching counts and checksums.
These fixture runs validate the benchmark tools; they do not establish large-map performance.
Git excludes the local records under `target/port-validation/`.
The original C++ source and `/mnt/maps` inputs remain unchanged.

The imported allocation test initially counted five Rust allocator calls per repeated compressed-block decode.
The corrected implementation makes zero calls after buffer preparation for that fixture.
The test does not count native decompressor allocations or establish zero allocation for fragmented fields and entity materialization.
The imported descriptor test also verified the corrected final-value behavior for `BlobHeader.datasize`.

### Document review

The reviewer examined `README.md`, `docs/development/CPP_TEST_PORT.md`, and `docs/benchmarks/PROCEDURE.md` against the source.
It checked local links, export contracts, timer boundaries, runner cases, and documented commands.
It executed the documented example invocations with temporary output directories.
The author corrected ambiguous mutex terminology, payload-size terminology, passive wording, and the description of unavailable metric summaries.
The author also reviewed this review record against the validation logs and reviewer reports.

Python syntax, Rust formatting, local links, and Git whitespace checks passed.
The language review checked sentence lengths, paragraph limits, active voice, and consistent terms.
The complete ASD-STE100 dictionary was unavailable; an attempted official-site request returned HTTP 403.
Full dictionary compliance remains unverified.
The independent reviewer did not repeat large-map benchmarks or Windows execution.
