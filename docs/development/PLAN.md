# Port plan

## Accepted requirements

- Preserve the original module structure and supported OSM data types.
- Use Rust names, borrowed views, and `Result` errors.
- Use a small dependency set for portable builds.
- Preserve selective decoding, counts, metadata, block indexes, and parallel reads.
- Compare C++ and Rust on all four maps in `/mnt/maps`.
- Repeat the measurements, including planet measurements.
- Require technical language review and independent implementation review.

## Implementation sequence

1. Add public types and checked protobuf primitives.
2. Add PBF framing, decompression, and selected field decoding.
3. Add reader indexing and parallel block processing.
4. Add XML, OSC, and entity callbacks.
5. Add fixtures, examples, and parity checks.
6. Run independent reviews and correct confirmed defects.
7. Run repeated benchmarks and record the conditions and results.
8. Review the final documents and implementation.

## Benchmark contract

Use release builds with comparable optimization settings.
Use equivalent field selections and callback work.
Compare counts and checksums before accepting timings.
Record wall time, CPU time, peak resident memory, compiler versions, and dependency versions.

Alternate process order across measured rounds.
Keep warmup results separate.
Report medians and ranges.
Identify cache conditions and differences between implementations.
