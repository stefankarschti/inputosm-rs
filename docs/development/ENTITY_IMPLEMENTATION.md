# Entity performance implementation

## Contract

The existing entity and column interfaces retain their signatures and validation behavior.
The entity adapter reserves known tag and member counts before collection.

`PbfBlock::decode_entity_views` supplies fields from the entity API through borrowed group views.
The caller selects metadata decoding with a boolean argument.
Private constructors prevent incomplete columns and invalid entity indices.
Views expose IDs, normalized coordinates, tags, references, members, and optional metadata.
Coordinate, timestamp, and string accessors return errors when conversion or lookup fails.
Unaccessed values can remain unchecked.

Use `PbfBlock::validate` for encoded-field and string-reference validation.
Accessors still check normalized coordinate and timestamp overflow after block validation.
These views do not support XML.
They omit optional way locations; the column API supplies those values.

`PbfReader::read_entity_views` supplies context and group views during a parallel scan.
Views cannot outlive their callback buffers.
Applications must copy values that they retain after a callback.

`PbfReadMode::Deferred` preserves the existing block scheduling behavior.
`PbfReadMode::Sequential` reads complete compressed blocks through a bounded buffer before worker decoding.
The sequential mode applies to bulk reads; indexed reads retain their existing scheduling behavior.
Deferred payload access requires Unix; other platforms read payloads before callbacks.
Both modes preserve error propagation, cancellation, block indices, and worker indices.

The optional `mmap` feature adds `PbfReader::open_mmap`.
This unsafe constructor requires an unchanged backing file until the reader drops.
Mapped reads borrow compressed bytes directly and retain deferred decompression.
The mapping applies to indexed and bulk reads.
The mapping constructor does not alter the default safe file API.

## Validation

Compare complete field values across entity objects, views, and read modes.
Cover metadata, malformed strings, coordinate overflow, cancellation, repeated scans, and indexed reads.
Compile callback lifetime rejection examples.
Run both decompressor configurations and the mapping feature.
Measure the complete consumer on all four maps with repeated runs.
Separate observed cache conditions from assumptions about storage performance.
