# Architecture

This library ports the C++ inputosm reader to Rust.
Module names retain the original component boundaries.

| Module | Responsibility |
|---|---|
| `inputosm` | Public entity types and file dispatch. |
| `inputosmpbf` | PBF framing, header parsing, block access, and worker scheduling. |
| `pbfcolumns` | Selected field decoding into reusable columns. |
| `inputosmxml` | XML and OSC entity decoding. |
| `timeutil` | UTC timestamp conversion. |
| `inputosmlog` | Optional diagnostic callbacks. |

The reader defers decompression until a block operation needs its payload.
Each worker retains its compressed buffer, decompression buffer, and column storage.
Entity callbacks borrow slices from those columns.
The compiler prevents these slices from outliving their buffers.
Each reader owns its configuration.
Callbacks receive context instead of reading global state.

File reads use bounded buffers and standard file operations.
This permits safe Rust APIs without a file memory mapping invariant.
On Unix, workers read payloads independently with positional file reads.
Framing-only callbacks do not read payload bytes on Unix.
Other platforms read payloads during serialized framing.

The C++ reader uses memory mapping.
Benchmark reports must identify this difference.

The `libdeflate-sys` crate builds libdeflate for compressed PBF blocks.
The `xml` crate supplies XML events.
The library implements protobuf decoding without generated code or a protobuf dependency.
Cargo records transitive dependencies in `Cargo.lock`.

Malformed selected fields return errors.
Unselected payload fields can remain unvalidated.
Full validation checks all supported entity fields and string references.
Callback errors stop an operation.
Concurrent callbacks can already be active when another callback stops the read.
