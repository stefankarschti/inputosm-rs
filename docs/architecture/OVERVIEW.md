# Architecture

This library ports the C++ inputosm reader to Rust.
Module names retain the original component boundaries.

| Module | Responsibility |
|---|---|
| `inputosm` | Public entity types and file dispatch. |
| `inputosmpbf` | PBF framing, header parsing, block access, and worker scheduling. |
| `pbfcolumns` | Selected field decoding into reusable columns. |
| `pbfviews` | PBF entity access through borrowed column views. |
| `inputosmxml` | XML and OSC entity decoding. |
| `timeutil` | UTC timestamp conversion. |
| `inputosmlog` | Optional diagnostic callbacks. |

The reader defers decompression until a block operation needs its payload.
Each worker retains its compressed buffer, decompression buffer, and column storage.
Entity callbacks borrow slices from those columns.
The compiler prevents these slices from outliving their buffers.
Each reader owns its configuration.
Callbacks receive context instead of reading global state.

The default file path uses bounded buffers and standard file operations.
In deferred mode, Unix workers read payloads independently with positional file reads.
Framing-only callbacks do not read payload bytes in that mode on Unix.
Other platforms read payloads during serialized framing.

Sequential mode reads complete compressed blocks through a one-MiB buffer under the framing lock.
Workers then decode those blocks outside the lock.
Each bulk worker retains one compressed block, in addition to the shared buffer and reader header storage.

The optional `mmap` feature maps a stable file through `PbfReader::open_mmap`.
Its unsafe contract requires an unchanged backing file until the reader drops.
Mapped blocks borrow compressed bytes directly and defer decompression until requested.
The operating system controls page residency; mapping size does not equal resident memory.
Indexed and bulk operations share the reader's mapping.

Entity views reuse the existing column decoder and block string table.
Private constructors guarantee complete columns and valid entity indices.
Accessors check string references and normalized numeric conversions when the caller reads those values.
The existing entity adapter constructs arrays and checks those values before its callback.
Views omit optional way locations; the column API supplies those values.

The `libdeflate-sys` crate builds libdeflate for compressed PBF blocks.
The `xml` crate supplies XML events.
The library implements protobuf decoding without generated code or a protobuf dependency.
Cargo records transitive dependencies in `Cargo.lock`.
The optional mapping feature uses `memmap2` and its platform dependency.

Malformed selected fields return errors.
Unselected payload fields can remain unvalidated.
Block validation checks supported encoded fields and string references.
Entity accessors also check normalized coordinate and timestamp overflow.
Callback errors stop an operation.
Concurrent callbacks can already be active when another callback stops the read.
