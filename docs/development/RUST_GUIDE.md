# Rust concepts for this port

## Ownership and borrowed views

A Rust value normally has one owner.
When the owner leaves its scope, Rust releases the value and its resources.
For example, dropping `PbfReader` closes its file.
No garbage collector controls this lifetime.

A reference borrows a value without copying it.
A slice such as `&[i64]` borrows consecutive integers.
The reader uses slices for IDs, coordinates, and entity lists.
The compiler prevents a callback from keeping these slices after their buffer becomes unavailable.

When the application must retain data, copy it into a `Vec`.
This copy is explicit and has a cost.
When the application can process each batch immediately, use borrowed views.

| C++ concept | Rust representation in this project |
|---|---|
| `std::vector<T>` | `Vec<T>` owns a variable-length array. |
| `std::span<const T>` | `&[T]` borrows an array. |
| `std::string_view` | `&[u8]` borrows PBF bytes. |
| `std::optional<T>` | `Option<T>` contains a value or `None`. |
| Boolean success and logging | `Result<T, Error>` returns a value or an error. |
| Destructor | `Drop` releases owned resources. |
| Thread-local parser globals | Reader settings and callback `Context`. |

## Error handling

`Result<T, Error>` has two forms: `Ok(value)` and `Err(error)`.
The `?` operator returns an error from the current function.
It does not discard the error.
When a callback must stop reading, return `Error::Cancelled`.

Rust panics indicate failed assumptions or an explicit `panic!()` call.
Use returned errors for malformed files and expected application failures.
The parallel reader joins its workers before it returns a callback error.

## Parallel callbacks

`&mut PbfReader` gives one caller exclusive access to a reader operation.
The block callback must support shared access because several workers can call it concurrently.
The `Sync` bound expresses this requirement in the type system.

When each worker processes many entities, use worker-local totals.
Combine the totals after reading.
A lock or atomic update for every individual node can limit throughput.
Sequential block benchmarks use one counter per worker and update it once per block.

## Performance

Use release builds for performance measurements.
Debug builds prioritize diagnostics and compilation speed.
Rust does not automatically outperform optimized C++.
Data layout, allocation, decompression, file access, and compiler decisions affect both implementations.

This port uses selected columns, reusable buffers, checked arithmetic, and parallel decompression.
The common protobuf parser path permits compiler inlining.
Rare group handling uses a separate function.
The benchmark compares counts and checksums before it compares elapsed times.

## Dependencies

Cargo resolves packages and records exact versions in `Cargo.lock`.
A direct dependency can require additional transitive dependencies.
The default library selects `xml` and `libdeflate-sys` directly.
The native build also needs the C build helper dependencies.

The alternative build selects `xml` and `flate2` directly.
`flate2` selects the pure Rust `zlib-rs` backend.
Use `--no-default-features --features rust-zlib` to avoid the native backend.

The current default follows measured performance.
The alternative remains available for builds without a C compiler.
