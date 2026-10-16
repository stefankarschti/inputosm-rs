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
