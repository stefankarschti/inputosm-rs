# inputosm for Rust

This project ports `~/inputosm` to Rust.
It reads OpenStreetMap PBF, XML (`.osm`), and change files (`.osc`).
The source modules retain the original component boundaries.
Rust callbacks borrow decoded slices without copying their contents.

## Build

Install a current stable Rust toolchain.
The default build also requires a C compiler for bundled libdeflate.
Cargo downloads the selected dependencies.

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release --example count_entities -- /mnt/maps/berlin-260514.osm.pbf
```

For the pure Rust decompressor, select `rust-zlib`.
This build does not require a C compiler.

```sh
cargo test --no-default-features --features rust-zlib
cargo build --release --no-default-features --features rust-zlib
```

The default backend uses `libdeflate-sys`.
The alternative uses `flate2` with `zlib-rs`.
Both builds use the `xml` crate.
The backend features control compression dependencies.
If both features are enabled, Cargo builds both dependencies, and the reader uses `rust-zlib`.

The task-local toolchain resides in `.toolchain/` on this machine.
Its files are not part of the source distribution.
For this installation, configure the shell as follows.

```sh
export RUSTUP_HOME="$PWD/.toolchain/rustup"
export CARGO_HOME="$PWD/.toolchain/cargo"
export PATH="$CARGO_HOME/bin:$PATH"
```

## Read selected fields

```rust
use inputosm::{NodeOptions, PbfReader, Result};

fn main() -> Result<()> {
    let mut reader = PbfReader::open("map.osm.pbf")?;
    reader.read_blocks(|block| {
        block.decode_nodes(NodeOptions::default(), |nodes| {
            for id in nodes.ids {
                println!("{id}");
            }
            Ok(())
        })
    })
}
```

`NodeOptions::default()` selects node IDs.
The reader does not decode other node fields for this request.
`Result` carries either the requested output or an error.
The `?` operator returns an error to the caller.

Use `block.counts()` to count entities without constructing entity arrays.
Use `decode_ways()` or `decode_relations()` to select their fields.
Use `decode_entities()` to request several entity types in one traversal.
Use `decode_entities_with_strings()` to resolve string IDs inside the same callback.

## Read entities

`input_file()` selects the reader from the file extension.
An `EntityHandler` receives node, way, and relation batches.
Its default methods ignore the corresponding entity type.
The PBF entity path still decodes all entity types and resolves their strings.
When unwanted fields affect performance, use selected block decoding.

Callbacks receive `Context` with the file type, OSC operation, block index, and worker index.
The library has no global parser state.
Set `InputOptions.threads` for parallel PBF entity reads.
XML and OSC callbacks execute sequentially.

## Read PBF entity views

`read_entity_views()` avoids construction of entity, tag, and member arrays.
The callback can read IDs, coordinates, tags, references, and members through borrowed accessors.
The boolean argument selects metadata decoding.
This interface supports PBF files.

```rust
use inputosm::{EntityViews, PbfReader, Result};

fn main() -> Result<()> {
    let mut reader = PbfReader::open("map.osm.pbf")?;
    reader.set_max_thread_count();
    reader.read_entity_views(false, |_, group| {
        if let EntityViews::Nodes(nodes) = group {
            for node in nodes.iter() {
                let (latitude, longitude) = node.coordinates()?;
                for tag in node.tags() {
                    let tag = tag?;
                    println!("{} {latitude} {longitude} {:?}", node.id(), tag);
                }
            }
        }
        Ok(())
    })
}
```

View coordinates use units of 100 nanodegrees.
`metadata()` returns `None` when metadata decoding is disabled.
Otherwise, it returns metadata with timestamps in seconds.
`user()` resolves the optional username.

String and conversion errors occur when accessors read those fields.
Unaccessed values can remain unchecked.
Use block validation for encoded-field and string-reference validation.
Accessors still check normalized coordinate and timestamp overflow after block validation.
Views omit optional way locations; use the column API for those values.

## File access modes

`PbfReader::open()` uses safe file reads.
On Unix, its default mode defers payload access until a callback requests decoded data.
Other platforms read payloads before callbacks.
For sequential buffering, call `reader.set_read_mode(inputosm::PbfReadMode::Sequential)`.
This mode reads each compressed data block before its callback and uses a one-MiB shared buffer.

Sequential mode does not change indexed reads or decompression timing.
It can help large sequential scans, but it can slow cached scans.

Enable `mmap` to use `PbfReader::open_mmap()`.
This optional feature adds the `memmap2` dependency.

```sh
cargo build --release --features mmap
```

The mapping constructor is unsafe.
The caller must prevent changes to the backing file until the reader drops.
This includes writes and truncation through other handles or processes.
The operating system loads mapped pages as necessary.
Memory mapping does not load the complete file into the Rust heap.
Mapped readers ignore `PbfReadMode` and borrow compressed payloads directly.

The [usage example](examples/entity_views.rs) shows both file access methods.

## Data contracts

- Callback slices remain valid only during their callback.
- PBF string values are byte slices. When text is required, use `std::str::from_utf8()`.
- PBF batch coordinates retain their encoded values.
- Use `Parameters` to convert batch coordinates into nanodegrees.
- Entity coordinates use units of 100 nanodegrees.
- PBF batch timestamps retain their encoded units.
- Entity timestamps use seconds since the Unix epoch.
- `Metadata.present` identifies fields that exist in the input.
- Metadata integers retain their full supported width.

`validate()` checks all supported columns, string references, and optional way locations.
Selected decoding validates the selected fields and their structural dependencies.
Counts validate framing and the fields required for counting.
Framing-only callbacks do not decompress data blocks.

Return `Error::Cancelled` to stop a read.
Other callback errors also stop the operation.
Concurrent callbacks can already be active when another callback fails.
`read_blocks()` converts callback panics into `Error::WorkerPanic` when unwinding is enabled.
Single-block and XML callbacks use normal Rust panic propagation.

## Random access and threads

`build_index()` records file block offsets without decompressing data blocks.
`read_block(index, callback)` uses that index when available.
Without an index, it scans block headers from the start.
The header has index zero.
Unknown block types retain their indexes but produce no sequential data callback.

Use `set_thread_count()` to configure a reader.
The supported range is one through 256 workers.
The method clamps values outside that range.
Use `set_max_thread_count()` to select available parallelism.
Callbacks from different workers can execute concurrently and out of file order.

Use `GroupBatch.kind()` to identify ordinary, dense, or unselected groups.
One mutable reader operation can run at a time.
Independent readers can operate concurrently.

## Differences from C++

The Rust API uses owned reader handles, borrowed slices, enums, and `Result` errors.
Dropping a reader closes its file.
Open failures return an error instead of leaving a closed reader object.
Header metadata belongs to the reader and remains available through `header()`.

The entity API normalizes coordinates and timestamps using the PBF block parameters.
This corrects the original entity API behavior for nondefault parameters.
Full validation also checks string indexes.
The C++ block validation interface does not check every string reference.

The XML reader accepts UTF-8, UTF-16, ASCII, and ISO-8859-1.
It rejects document types and external entities.
It rejects unsupported encoding declarations.
Malformed integers and coordinates return errors.
The metadata option applies consistently to XML and PBF.

The Rust reader defaults to bounded file reads and offers optional memory mapping.
Separate readers own separate indexes.
The C++ reader can share mappings and indexes across readers.
These differences affect random-access performance and memory measurements.

## Project documents

- [Rust concepts](docs/development/RUST_GUIDE.md)
- [Architecture](docs/architecture/OVERVIEW.md)
- [Port plan](docs/development/PLAN.md)
- [Review record](docs/development/REVIEW.md)
- [Benchmark procedure](docs/benchmarks/PROCEDURE.md)
- [Benchmark results](docs/benchmarks/RESULTS.md)
- [Entity implementation measurements](docs/benchmarks/ENTITY_IMPLEMENTATION.md)
- [Project instructions](AGENTS.md)

The original source and this port use the Apache-2.0 license.
See [LICENSE](LICENSE).
