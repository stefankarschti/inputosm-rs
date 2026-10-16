# Preliminary measurements

These measurements supported implementation decisions.
The final report does not use them for performance claims.

`decompressors/` contains measurements from an earlier implementation.
The final source includes later XML and PBF changes.

`decompressors-with-review-overlap/` uses the final source.
A short reviewer fixture check might have overlapped its first measurements.
The final comparison repeats this experiment without reviewer commands.

Both directories use the comparison runner labels `cpp` and `rust`.
In these experiments, `cpp` means Rust with libdeflate.
The `rust` label means Rust with `zlib-rs`.
The recorded binary hashes distinguish these builds.
