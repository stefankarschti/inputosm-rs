//! Read PBF blocks and decode selected columns.

use crate::inputosm::require;
use crate::*;

pub(crate) mod wire {
    use super::*;
    #[derive(Clone, Copy, Debug, Default)]
    pub(crate) struct Field<'a> {
        pub number: u32,
        pub wire: u8,
        pub value: u64,
        pub bytes: &'a [u8],
    }
    impl<'a> Field<'a> {
        pub fn integer(self) -> Result<u64> {
            require(self.wire == 0, "Invalid integer wire type")?;
            Ok(self.value)
        }
        pub fn message(self) -> Result<&'a [u8]> {
            require(self.wire == 2, "Invalid message wire type")?;
            Ok(self.bytes)
        }
        pub fn repeated(self, mut visit: impl FnMut(u64) -> Result<()>) -> Result<()> {
            if self.wire == 0 {
                return visit(self.value);
            }
            let mut r = Wire::new(self.message()?);
            while !r.bytes.is_empty() {
                visit(r.varint()?)?;
            }
            Ok(())
        }
        pub fn count(self) -> Result<usize> {
            if self.wire == 0 {
                return Ok(1);
            }
            let mut r = Wire::new(self.message()?);
            let mut count = 0;
            while !r.bytes.is_empty() {
                if r.bytes.len() >= 8 && r.bytes[7] < 128 {
                    let word = u64::from_ne_bytes(r.bytes[..8].try_into().unwrap());
                    count += (!word & 0x8080808080808080).count_ones() as usize;
                    r.bytes = &r.bytes[8..];
                } else {
                    r.varint()?;
                    count += 1;
                }
            }
            Ok(count)
        }
    }
    pub(crate) struct Wire<'a> {
        pub bytes: &'a [u8],
    }
    impl<'a> Wire<'a> {
        pub fn new(bytes: &'a [u8]) -> Self {
            Self { bytes }
        }
        pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
            require(n <= self.bytes.len(), "Truncated PBF field")?;
            let (head, rest) = self.bytes.split_at(n);
            self.bytes = rest;
            Ok(head)
        }
        #[inline(always)]
        pub fn varint(&mut self) -> Result<u64> {
            let bytes = self.bytes;
            let first = *bytes
                .first()
                .ok_or(Error::Invalid("Truncated PBF varint"))?;
            if first < 128 {
                self.bytes = &bytes[1..];
                return Ok(first as u64);
            }
            let mut result = u64::from(first & 127);
            for i in 1..10 {
                let byte = *bytes.get(i).ok_or(Error::Invalid("Truncated PBF varint"))?;
                require(i != 9 || byte <= 1, "PBF varint overflow")?;
                result |= u64::from(byte & 127) << (i * 7);
                if byte < 128 {
                    self.bytes = &bytes[i + 1..];
                    return Ok(result);
                }
            }
            Err(Error::Invalid("PBF varint overflow"))
        }
        #[inline]
        pub fn next(&mut self) -> Result<Option<Field<'a>>> {
            self.next_depth(0)
        }
        #[inline(always)]
        fn next_depth(&mut self, depth: usize) -> Result<Option<Field<'a>>> {
            if self.bytes.is_empty() {
                return Ok(None);
            }
            let key = self.varint()?;
            require(
                key >> 3 != 0 && key >> 3 <= 0x1fffffff,
                "Invalid PBF field number",
            )?;
            let mut f = Field {
                number: (key >> 3) as u32,
                wire: (key & 7) as u8,
                ..Field::default()
            };
            match f.wire {
                0 => f.value = self.varint()?,
                1 => f.bytes = self.take(8)?,
                2 => {
                    let n = usize::try_from(self.varint()?)
                        .map_err(|_| Error::Invalid("PBF length overflow"))?;
                    f.bytes = self.take(n)?;
                }
                3 => self.skip_group(f.number, depth)?,
                4 => require(depth != 0, "Unexpected PBF group end")?,
                5 => f.bytes = self.take(4)?,
                _ => return Err(Error::Invalid("Unsupported PBF wire type")),
            }
            Ok(Some(f))
        }
        #[cold]
        #[inline(never)]
        fn skip_group(&mut self, number: u32, depth: usize) -> Result<()> {
            require(depth < 64, "PBF group nesting limit exceeded")?;
            loop {
                let nested = self
                    .next_depth(depth + 1)?
                    .ok_or(Error::Invalid("Missing PBF group end"))?;
                if nested.wire == 4 {
                    return require(nested.number == number, "Invalid PBF group end");
                }
            }
        }
    }
    #[inline]
    pub fn zigzag(v: u64) -> i64 {
        ((v >> 1) as i64) ^ -((v & 1) as i64)
    }
    pub fn narrow(v: i64) -> Result<i32> {
        i32::try_from(v).map_err(|_| Error::Invalid("PBF value exceeds 32 bits"))
    }
    pub fn sid(v: u64) -> Result<u32> {
        u32::try_from(v).map_err(|_| Error::Invalid("PBF string ID exceeds 32 bits"))
    }
    #[inline]
    pub fn delta(previous: i64, value: i64) -> Result<i64> {
        previous
            .checked_add(value)
            .ok_or(Error::Invalid("PBF delta overflow"))
    }
    pub fn visit_group<'a>(
        bytes: &'a [u8],
        mut visit: impl FnMut(Field<'a>) -> Result<()>,
    ) -> Result<u32> {
        let mut kind = 0;
        let mut r = Wire::new(bytes);
        while let Some(f) = r.next()? {
            if (1..=4).contains(&f.number) {
                require(
                    kind == 0 || kind == f.number,
                    "Mixed entity types in primitive group",
                )?;
                kind = f.number;
                f.message()?;
                visit(f)?;
            } else if f.number == 5 {
                return Err(Error::Invalid("Unsupported ChangeSet entity"));
            }
        }
        Ok(kind)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn malformed_wire() {
            for bytes in [
                &[0x80][..],
                &[0xff; 10],
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 2],
            ] {
                assert!(Wire::new(bytes).varint().is_err());
            }
            for bytes in [&[0][..], &[10, 4, 0], &[12], &[11, 20]] {
                assert!(Wire::new(bytes).next().is_err());
            }
            assert_eq!(
                Wire::new(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1])
                    .varint()
                    .unwrap(),
                u64::MAX
            );
            assert_eq!(zigzag(u64::MAX), i64::MIN);
            assert!(delta(i64::MAX, 1).is_err());
            let mut r = Wire::new(&[11, 16, 1, 12, 24, 2]);
            assert_eq!(r.next().unwrap().unwrap().number, 1);
            assert_eq!(r.next().unwrap().unwrap().integer().unwrap(), 2);
        }
    }
}

use crate::pbfcolumns::{Columns, Fields, collect_dense};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    ops::Range,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use wire::*;

const MAX_HEADER: usize = 64 * 1024;
const MAX_RAW: usize = 32 * 1024 * 1024;
const MAX_BLOB: usize = MAX_RAW + MAX_HEADER;

#[cfg(not(feature = "rust-zlib"))]
struct Inflater(std::ptr::NonNull<libdeflate_sys::libdeflate_decompressor>);
#[cfg(not(feature = "rust-zlib"))]
impl Inflater {
    fn new() -> Result<Self> {
        // SAFETY: The allocator takes no arguments and returns an owned decoder.
        let pointer = unsafe { libdeflate_sys::libdeflate_alloc_decompressor() };
        Ok(Self(
            std::ptr::NonNull::new(pointer)
                .ok_or(Error::Invalid("Cannot allocate decompressor"))?,
        ))
    }
    fn decompress(&mut self, input: &[u8], output: &mut [u8]) -> Result<()> {
        let mut consumed = 0;
        // SAFETY: Both slices remain valid for this call. This worker owns the decoder.
        let status = unsafe {
            libdeflate_sys::libdeflate_zlib_decompress_ex(
                self.0.as_ptr(),
                input.as_ptr().cast(),
                input.len(),
                output.as_mut_ptr().cast(),
                output.len(),
                &mut consumed,
                std::ptr::null_mut(),
            )
        };
        require(
            status == 0 && consumed == input.len(),
            "Invalid Zlib Blob data",
        )
    }
}
#[cfg(not(feature = "rust-zlib"))]
impl Drop for Inflater {
    fn drop(&mut self) {
        // SAFETY: This pointer came from the allocator and has one owner.
        unsafe {
            libdeflate_sys::libdeflate_free_decompressor(self.0.as_ptr());
        }
    }
}
// SAFETY: A decoder has one owner. Libdeflate has no thread-local decoder state.
#[cfg(not(feature = "rust-zlib"))]
unsafe impl Send for Inflater {}

#[cfg(feature = "rust-zlib")]
struct Inflater(flate2::Decompress);
#[cfg(feature = "rust-zlib")]
impl Inflater {
    fn new() -> Result<Self> {
        Ok(Self(flate2::Decompress::new(true)))
    }
    fn decompress(&mut self, input: &[u8], output: &mut [u8]) -> Result<()> {
        self.0.reset(true);
        let status = self
            .0
            .decompress(input, output, flate2::FlushDecompress::Finish)
            .map_err(|_| Error::Invalid("Invalid Zlib Blob data"))?;
        require(
            status == flate2::Status::StreamEnd
                && self.0.total_in() == input.len() as u64
                && self.0.total_out() == output.len() as u64,
            "Invalid Zlib Blob data",
        )
    }
}

#[derive(Default)]
struct Scratch {
    raw: Vec<u8>,
    inflater: Option<Inflater>,
    groups: Vec<Range<usize>>,
    tables: Vec<Range<usize>>,
    columns: Columns,
}
fn blob_data(blob: &[u8], scratch: &mut Scratch) -> Result<()> {
    let mut r = Wire::new(blob);
    let (mut encoding, mut payload, mut size) = (0, &[][..], None);
    while let Some(f) = r.next()? {
        if f.number == 2 {
            let v = narrow(f.integer()? as i64)?;
            require(
                v >= 0 && (v as usize) < MAX_RAW,
                "Invalid uncompressed Blob size",
            )?;
            size = Some(v as usize);
        }
        if f.number == 1 || (3..=7).contains(&f.number) {
            require(encoding == 0, "Multiple Blob encodings")?;
            encoding = f.number;
            payload = f.message()?;
        }
    }
    match encoding {
        1 => {
            require(
                payload.len() < MAX_RAW && size.is_none_or(|s| s == payload.len()),
                "Invalid raw Blob size",
            )?;
            scratch.raw.clear();
            scratch.raw.extend_from_slice(payload);
        }
        3 => {
            let size = size.ok_or(Error::Invalid("Missing uncompressed Blob size"))?;
            scratch.raw.resize(size, 0);
            if scratch.inflater.is_none() {
                scratch.inflater = Some(Inflater::new()?);
            }
            scratch
                .inflater
                .as_mut()
                .unwrap()
                .decompress(payload, &mut scratch.raw)?;
        }
        _ => return Err(Error::Invalid("Unsupported or missing Blob encoding")),
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockKind {
    Header,
    Data,
    Unknown,
}

#[derive(Clone, Debug)]
struct Descriptor {
    index: usize,
    offset: u64,
    payload_offset: u64,
    size: usize,
    kind: BlockKind,
}
fn descriptor(
    file: &mut File,
    index: usize,
    offset: u64,
    length: u64,
) -> Result<Option<Descriptor>> {
    if offset == length {
        return Ok(None);
    }
    require(
        offset < length && length - offset >= 4,
        "Truncated Blob prefix",
    )?;
    file.seek(SeekFrom::Start(offset))?;
    read_descriptor(file, index, offset, length)
}
fn read_descriptor(
    file: &mut impl Read,
    index: usize,
    offset: u64,
    length: u64,
) -> Result<Option<Descriptor>> {
    if offset == length {
        return Ok(None);
    }
    require(
        offset < length && length - offset >= 4,
        "Truncated Blob prefix",
    )?;
    let mut prefix = [0; 4];
    file.read_exact(&mut prefix)?;
    let size = u32::from_be_bytes(prefix) as usize;
    require(size > 0 && size < MAX_HEADER, "Invalid BlobHeader size")?;
    require(length - offset - 4 >= size as u64, "Truncated BlobHeader")?;
    let mut bytes = [0u8; MAX_HEADER];
    file.read_exact(&mut bytes[..size])?;
    let mut r = Wire::new(&bytes[..size]);
    let mut kind = None;
    let mut blob_size = None;
    while let Some(f) = r.next()? {
        match f.number {
            1 => {
                let bytes = f.message()?;
                kind = Some(bytes);
            }
            3 => {
                blob_size = Some(f.integer()?);
            }
            _ => {}
        }
    }
    let kind = kind.ok_or(Error::Invalid("Missing Blob type"))?;
    require(!kind.is_empty(), "Empty Blob type")?;
    let kind = match kind {
        b"OSMHeader" => BlockKind::Header,
        b"OSMData" => BlockKind::Data,
        _ => BlockKind::Unknown,
    };
    let size = blob_size.ok_or(Error::Invalid("Missing Blob size"))?;
    require(size <= MAX_BLOB as u64, "Invalid Blob size")?;
    let size = size as usize;
    let payload_offset = offset + 4 + u64::from(u32::from_be_bytes(prefix));
    require(size as u64 <= length - payload_offset, "Truncated Blob")?;
    require(
        index == 0 || kind != BlockKind::Header,
        "Unexpected second OSMHeader",
    )?;
    Ok(Some(Descriptor {
        index,
        offset,
        payload_offset,
        size,
        kind,
    }))
}
#[cfg(feature = "mmap")]
fn mapped_descriptor(map: &[u8], index: usize, offset: u64) -> Result<Option<Descriptor>> {
    let start = usize::try_from(offset).map_err(|_| Error::Invalid("PBF offset overflow"))?;
    let mut bytes = map
        .get(start..)
        .ok_or(Error::Invalid("PBF offset out of range"))?;
    read_descriptor(&mut bytes, index, offset, map.len() as u64)
}

/// Select payload scheduling for bulk file reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PbfReadMode {
    /// On Unix, read payloads when callbacks request decoded data. Other platforms read payloads before callbacks.
    #[default]
    Deferred,
    /// Read complete compressed blocks through a bounded sequential buffer.
    Sequential,
}

enum BlockSource<'a> {
    Buffered,
    #[cfg(unix)]
    File(&'a File),
    Slice(&'a [u8]),
}

fn read_payload(file: &mut File, d: &Descriptor, out: &mut Vec<u8>) -> Result<()> {
    file.seek(SeekFrom::Start(d.payload_offset))?;
    out.resize(d.size, 0);
    file.read_exact(out)?;
    Ok(())
}
fn parse_header(bytes: &[u8]) -> Result<HeaderMetadata> {
    let mut out = HeaderMetadata::default();
    let mut bbox_mask = 0;
    let mut r = Wire::new(bytes);
    while let Some(f) = r.next()? {
        match f.number {
            1 => {
                let bbox = out.bbox.get_or_insert_default();
                let mut r = Wire::new(f.message()?);
                while let Some(v) = r.next()? {
                    if !(1..=4).contains(&v.number) {
                        continue;
                    }
                    let value = zigzag(v.integer()?);
                    bbox_mask |= 1 << (v.number - 1);
                    match v.number {
                        1 => bbox.left = value,
                        2 => bbox.right = value,
                        3 => bbox.top = value,
                        4 => bbox.bottom = value,
                        _ => {}
                    }
                }
            }
            4 => {
                let feature = f.message()?;
                require(
                    feature == b"OsmSchema-V0.6" || feature == b"DenseNodes",
                    "Unsupported required PBF feature",
                )?;
                out.required_features.push(feature.to_vec());
            }
            5 => out.optional_features.push(f.message()?.to_vec()),
            16 => out.writingprogram = Some(f.message()?.to_vec()),
            17 => out.source = Some(f.message()?.to_vec()),
            32 => out.osmosis_replication_timestamp = Some(f.integer()? as i64),
            33 => out.osmosis_replication_sequence_number = Some(f.integer()? as i64),
            34 => out.osmosis_replication_base_url = Some(f.message()?.to_vec()),
            _ => {}
        }
    }
    require(
        out.bbox.is_none() || bbox_mask == 15,
        "Missing bounding box coordinate",
    )?;
    Ok(out)
}

/// Borrow one block during a reader callback.
pub struct PbfBlock<'a> {
    descriptor: &'a Descriptor,
    compressed: &'a mut Vec<u8>,
    source: BlockSource<'a>,
    scratch: &'a mut Scratch,
    prepared: bool,
    parameters: Parameters,
    locations_allowed: bool,
    thread_index: usize,
    cached_counts: Counts,
    count_mask: u8,
}
impl PbfBlock<'_> {
    pub fn index(&self) -> usize {
        self.descriptor.index
    }
    pub fn file_offset(&self) -> u64 {
        self.descriptor.offset
    }
    pub fn thread_index(&self) -> usize {
        self.thread_index
    }
    fn prepare(&mut self) -> Result<()> {
        if self.prepared {
            return Ok(());
        }
        let bytes = match self.source {
            #[cfg(unix)]
            BlockSource::File(file) => {
                use std::os::unix::fs::FileExt;
                self.compressed.resize(self.descriptor.size, 0);
                file.read_exact_at(self.compressed, self.descriptor.payload_offset)?;
                &self.compressed[..]
            }
            BlockSource::Buffered => &self.compressed[..],
            BlockSource::Slice(bytes) => bytes,
        };
        blob_data(bytes, self.scratch)?;
        self.scratch.groups.clear();
        self.scratch.tables.clear();
        let raw = &self.scratch.raw;
        let range = |bytes: &[u8]| {
            let start = bytes.as_ptr() as usize - raw.as_ptr() as usize;
            start..start + bytes.len()
        };
        let mut r = Wire::new(raw);
        let mut parameters = Parameters::default();
        while let Some(f) = r.next()? {
            match f.number {
                1 => self.scratch.tables.push(range(f.message()?)),
                2 => self.scratch.groups.push(range(f.message()?)),
                17 => parameters.granularity = narrow(f.integer()? as i64)?,
                18 => parameters.date_granularity = narrow(f.integer()? as i64)?,
                19 => parameters.lat_offset = f.integer()? as i64,
                20 => parameters.lon_offset = f.integer()? as i64,
                _ => {}
            }
        }
        require(!self.scratch.tables.is_empty(), "Missing PBF string table")?;
        require(
            parameters.granularity > 0 && parameters.date_granularity > 0,
            "Invalid PBF granularity",
        )?;
        self.parameters = parameters;
        self.prepared = true;
        Ok(())
    }
    pub fn parameters(&mut self) -> Result<Parameters> {
        self.prepare()?;
        Ok(self.parameters)
    }
    pub fn decode_strings(&mut self, handler: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        self.prepare()?;
        visit_strings(&self.scratch.raw, &self.scratch.tables, handler)
    }
    pub fn string_table_size(&mut self) -> Result<usize> {
        let mut count = 0;
        self.decode_strings(|_| {
            count += 1;
            Ok(())
        })?;
        Ok(count)
    }
    fn count(&mut self, mask: u8) -> Result<Counts> {
        let mask = mask & !self.count_mask;
        if mask == 0 {
            return Ok(self.cached_counts);
        }
        self.prepare()?;
        let mut counts = self.cached_counts;
        for range in &self.scratch.groups {
            visit_group(&self.scratch.raw[range.clone()], |f| {
                match f.number {
                    1 if mask & 1 != 0 => counts.nodes += 1,
                    3 if mask & 2 != 0 => counts.ways += 1,
                    4 if mask & 4 != 0 => counts.relations += 1,
                    2 if mask & 1 != 0 => {
                        let mut r = Wire::new(f.message()?);
                        while let Some(c) = r.next()? {
                            if c.number == 1 {
                                counts.nodes += c.count()?;
                            }
                        }
                    }
                    _ => {}
                }
                Ok(())
            })?;
        }
        self.cached_counts = counts;
        self.count_mask |= mask;
        Ok(counts)
    }
    pub fn counts(&mut self) -> Result<Counts> {
        self.count(7)
    }
    pub fn node_count(&mut self) -> Result<usize> {
        Ok(self.count(1)?.nodes)
    }
    pub fn way_count(&mut self) -> Result<usize> {
        Ok(self.count(2)?.ways)
    }
    pub fn relation_count(&mut self) -> Result<usize> {
        Ok(self.count(4)?.relations)
    }
    pub fn decode_entities(
        &mut self,
        options: EntityOptions,
        mut handler: impl FnMut(GroupBatch<'_>) -> Result<()>,
    ) -> Result<()> {
        self.decode(options, false, |batch, _| handler(batch))
    }
    /// String slices and batch slices remain valid during this callback.
    pub fn decode_entities_with_strings(
        &mut self,
        options: EntityOptions,
        handler: impl FnMut(GroupBatch<'_>, &[&[u8]]) -> Result<()>,
    ) -> Result<()> {
        self.decode(options, true, handler)
    }
    fn decode(
        &mut self,
        options: EntityOptions,
        strings: bool,
        mut handler: impl FnMut(GroupBatch<'_>, &[&[u8]]) -> Result<()>,
    ) -> Result<()> {
        self.prepare()?;
        let mut table = Vec::new();
        if strings {
            visit_strings(&self.scratch.raw, &self.scratch.tables, |s| {
                table.push(s);
                Ok(())
            })?;
        }
        let mut dense: [Fields<'_>; 10] = std::array::from_fn(|_| Fields::default());
        for (group_index, range) in self.scratch.groups.iter().enumerate() {
            let columns = &mut self.scratch.columns;
            columns.clear();
            for f in &mut dense {
                f.clear();
            }
            let kind = visit_group(&self.scratch.raw[range.clone()], |f| {
                if f.number == 2 {
                    if let Some(n) = options.nodes {
                        collect_dense(f.message()?, &mut dense, n)?;
                    }
                } else if (f.number == 1 && options.nodes.is_some())
                    || (f.number == 3 && options.ways.is_some())
                    || (f.number == 4 && options.relations.is_some())
                {
                    columns.ordinary(f.message()?, f.number, options, self.locations_allowed)?;
                }
                Ok(())
            })?;
            if kind == 2 {
                if let Some(n) = options.nodes {
                    columns.dense(&dense, n)?;
                }
            }
            let context = BatchContext {
                kind: match kind {
                    1 => GroupKind::Nodes,
                    2 => GroupKind::DenseNodes,
                    3 => GroupKind::Ways,
                    4 => GroupKind::Relations,
                    _ => GroupKind::Empty,
                },
                block_index: self.descriptor.index,
                group_index,
                count: columns.count,
                parameters: self.parameters,
            };
            handler(columns.batch(context, kind, options), &table)?;
        }
        Ok(())
    }
    /// Decode PBF entity fields without constructing entity arrays. Optional way locations require the column API.
    /// Accessors check string IDs and numeric conversions when the caller reads them.
    /// Unaccessed values can remain unchecked. Metadata decoding is optional.
    pub fn decode_entity_views(
        &mut self,
        decode_metadata: bool,
        mut handler: impl FnMut(EntityViews<'_>) -> Result<()>,
    ) -> Result<()> {
        self.decode_entities_with_strings(EntityOptions::all(decode_metadata), |group, strings| {
            handler(EntityViews::new(group, strings))
        })
    }
    pub fn decode_nodes(
        &mut self,
        fields: NodeOptions,
        mut handler: impl FnMut(NodeBatch<'_>) -> Result<()>,
    ) -> Result<()> {
        self.decode_entities(
            EntityOptions {
                nodes: Some(fields),
                ..Default::default()
            },
            |group| {
                if let GroupBatch::Nodes(batch) = group {
                    handler(batch)?;
                }
                Ok(())
            },
        )
    }
    pub fn decode_ways(
        &mut self,
        fields: WayOptions,
        mut handler: impl FnMut(WayBatch<'_>) -> Result<()>,
    ) -> Result<()> {
        self.decode_entities(
            EntityOptions {
                ways: Some(fields),
                ..Default::default()
            },
            |group| {
                if let GroupBatch::Ways(batch) = group {
                    handler(batch)?;
                }
                Ok(())
            },
        )
    }
    pub fn decode_relations(
        &mut self,
        fields: RelationOptions,
        mut handler: impl FnMut(RelationBatch<'_>) -> Result<()>,
    ) -> Result<()> {
        self.decode_entities(
            EntityOptions {
                relations: Some(fields),
                ..Default::default()
            },
            |group| {
                if let GroupBatch::Relations(batch) = group {
                    handler(batch)?;
                }
                Ok(())
            },
        )
    }
    pub fn validate(&mut self) -> Result<()> {
        let mut options = EntityOptions::all(true);
        options.ways.as_mut().unwrap().node_locations = true;
        self.decode_entities_with_strings(options, |group, strings| {
            let (tags, metadata) = match group {
                GroupBatch::Nodes(ref b) => (&b.tags, b.metadata),
                GroupBatch::Ways(ref b) => (&b.tags, b.metadata),
                GroupBatch::Relations(ref b) => {
                    require(
                        b.member_roles.iter().all(|&s| (s as usize) < strings.len()),
                        "Invalid member role string ID",
                    )?;
                    (&b.tags, b.metadata)
                }
                GroupBatch::Empty(_) => return Ok(()),
            };
            require(
                tags.values.iter().all(|t| {
                    (t.key as usize) < strings.len() && (t.value as usize) < strings.len()
                }),
                "Invalid tag string ID",
            )?;
            require(
                metadata.iter().all(|m| {
                    m.present & Metadata::USER == 0 || (m.user_sid as usize) < strings.len()
                }),
                "Invalid user string ID",
            )
        })
    }
}
fn visit_strings<'a>(
    raw: &'a [u8],
    tables: &[Range<usize>],
    mut visit: impl FnMut(&'a [u8]) -> Result<()>,
) -> Result<()> {
    let mut count = 0;
    for range in tables {
        let mut r = Wire::new(&raw[range.clone()]);
        while let Some(f) = r.next()? {
            if f.number == 1 {
                let s = f.message()?;
                require(count != 0 || s.is_empty(), "PBF string zero must be empty")?;
                count += 1;
                visit(s)?;
            }
        }
    }
    require(count != 0, "Empty PBF string table")
}

/// Own a PBF file and its optional offset index.
pub struct PbfReader {
    file: File,
    #[cfg(feature = "mmap")]
    mapping: Option<memmap2::Mmap>,
    read_mode: PbfReadMode,
    length: u64,
    header: HeaderMetadata,
    locations_allowed: bool,
    threads: usize,
    offsets: Option<Vec<u64>>,
    scratch: Scratch,
    compressed: Vec<u8>,
    log_level: crate::inputosmlog::LogLevel,
    log_callback: Option<crate::inputosmlog::LogCallback>,
}
impl PbfReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_file(File::open(path)?)
    }
    /// Map a stable PBF file for indexed and bulk reads.
    ///
    /// # Safety
    /// The backing file must remain unchanged until this reader drops.
    /// This includes writes and truncation through other handles or processes.
    #[cfg(feature = "mmap")]
    pub unsafe fn open_mmap(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        // SAFETY: The caller guarantees an unchanged backing file for this reader's lifetime.
        let mapping = unsafe { memmap2::MmapOptions::new().map(&file)? };
        let mut reader = Self::from_file(file)?;
        reader.mapping = Some(mapping);
        Ok(reader)
    }
    fn from_file(mut file: File) -> Result<Self> {
        let length = file.metadata()?.len();
        let d = descriptor(&mut file, 0, 0, length)?.ok_or(Error::Invalid("Empty PBF file"))?;
        require(d.kind == BlockKind::Header, "Missing initial OSMHeader")?;
        let mut scratch = Scratch::default();
        let mut compressed = Vec::new();
        read_payload(&mut file, &d, &mut compressed)?;
        blob_data(&compressed, &mut scratch)?;
        let header = parse_header(&scratch.raw)?;
        let locations_allowed = header
            .optional_features
            .iter()
            .any(|s| s == b"LocationsOnWays");
        Ok(Self {
            file,
            #[cfg(feature = "mmap")]
            mapping: None,
            read_mode: PbfReadMode::Deferred,
            length,
            header,
            locations_allowed,
            threads: 1,
            offsets: None,
            scratch,
            compressed,
            log_level: crate::inputosmlog::LogLevel::Info,
            log_callback: None,
        })
    }
    /// Set bulk file scheduling. Memory mappings ignore this setting.
    pub fn set_read_mode(&mut self, mode: PbfReadMode) {
        self.read_mode = mode;
    }
    pub fn read_mode(&self) -> PbfReadMode {
        self.read_mode
    }
    fn mapped_bytes(&self) -> Option<&[u8]> {
        #[cfg(feature = "mmap")]
        {
            self.mapping.as_deref()
        }
        #[cfg(not(feature = "mmap"))]
        {
            None
        }
    }
    fn descriptor(&mut self, index: usize, offset: u64) -> Result<Option<Descriptor>> {
        #[cfg(feature = "mmap")]
        if let Some(map) = &self.mapping {
            return mapped_descriptor(map, index, offset);
        }
        descriptor(&mut self.file, index, offset, self.length)
    }
    pub fn header(&self) -> &HeaderMetadata {
        &self.header
    }
    pub fn set_log_level(&mut self, level: crate::inputosmlog::LogLevel) {
        self.log_level = level;
    }
    pub fn set_log_callback(&mut self, callback: crate::inputosmlog::LogCallback) {
        self.log_callback = Some(callback);
    }
    fn log(&self, level: crate::inputosmlog::LogLevel, message: &str) {
        if level >= self.log_level {
            if let Some(callback) = &self.log_callback {
                callback(level, message);
            }
        }
    }

    pub fn set_thread_count(&mut self, count: usize) {
        self.threads = count.clamp(1, 256);
    }
    pub fn set_max_thread_count(&mut self) {
        self.set_thread_count(std::thread::available_parallelism().map_or(1, usize::from));
    }
    pub fn thread_count(&self) -> usize {
        self.threads
    }
    pub fn has_index(&self) -> bool {
        self.offsets.is_some()
    }
    pub fn index_memory_bytes(&self) -> usize {
        self.offsets.as_ref().map_or(0, |v| v.capacity() * 8)
    }
    pub fn build_index(&mut self) -> Result<()> {
        if self.has_index() {
            return Ok(());
        }
        let mut offsets = Vec::new();
        let mut offset = 0;
        while let Some(d) = self.descriptor(offsets.len(), offset)? {
            offsets.push(offset);
            offset = d.payload_offset + d.size as u64;
        }
        self.offsets = Some(offsets);
        Ok(())
    }
    /// The header has index zero. Unknown block types retain their file indexes.
    pub fn read_block(
        &mut self,
        index: usize,
        handler: impl FnOnce(&mut PbfBlock<'_>) -> Result<()>,
    ) -> Result<()> {
        let d = if let Some(offsets) = &self.offsets {
            let offset = *offsets
                .get(index)
                .ok_or(Error::Invalid("Block index out of range"))?;
            self.descriptor(index, offset)?
                .ok_or(Error::Invalid("Missing indexed block"))?
        } else {
            let mut offset = 0;
            let mut current = 0;
            loop {
                let d = self
                    .descriptor(current, offset)?
                    .ok_or(Error::Invalid("Block index out of range"))?;
                if current == index {
                    break d;
                }
                offset = d.payload_offset + d.size as u64;
                current += 1;
            }
        };
        require(d.kind == BlockKind::Data, "Requested block is not OSMData")?;
        #[cfg(feature = "mmap")]
        let mapping = self.mapping.as_deref();
        #[cfg(not(feature = "mmap"))]
        let mapping: Option<&[u8]> = None;
        let source = if let Some(map) = mapping {
            BlockSource::Slice(&map[d.payload_offset as usize..][..d.size])
        } else {
            #[cfg(unix)]
            {
                BlockSource::File(&self.file)
            }
            #[cfg(not(unix))]
            {
                read_payload(&mut self.file, &d, &mut self.compressed)?;
                BlockSource::Buffered
            }
        };
        handler(&mut PbfBlock {
            descriptor: &d,
            compressed: &mut self.compressed,
            source,
            scratch: &mut self.scratch,
            prepared: false,
            parameters: Parameters::default(),
            locations_allowed: self.locations_allowed,
            thread_index: 0,
            cached_counts: Counts::default(),
            count_mask: 0,
        })
    }
    /// Read PBF entity views with this reader's current scheduling mode.
    /// Workers can call the handler concurrently and out of file order.
    pub fn read_entity_views(
        &mut self,
        decode_metadata: bool,
        handler: impl Fn(Context, EntityViews<'_>) -> Result<()> + Sync,
    ) -> Result<()> {
        self.read_blocks(|block| {
            let context = Context {
                file_type: FileType::Pbf,
                mode: Mode::Bulk,
                block_index: block.index(),
                thread_index: block.thread_index(),
            };
            block.decode_entity_views(decode_metadata, |group| handler(context, group))
        })
    }
    /// Workers can call the handler concurrently and out of file order.
    pub fn read_blocks(
        &mut self,
        handler: impl Fn(&mut PbfBlock<'_>) -> Result<()> + Sync,
    ) -> Result<()> {
        self.log(crate::inputosmlog::LogLevel::Trace, "Start PBF block read");
        #[cfg(unix)]
        let payload_file = self.file.try_clone()?;
        let mut framing_file = self.file.try_clone()?;
        framing_file.seek(SeekFrom::Start(0))?;
        let sequential = self.read_mode == PbfReadMode::Sequential;
        let mapping = self.mapped_bytes();
        let capacity = if sequential && mapping.is_none() {
            1024 * 1024
        } else {
            0
        };
        let state = Mutex::new((
            BufReader::with_capacity(capacity, framing_file),
            0u64,
            0usize,
        ));
        let stopped = AtomicBool::new(false);
        let failure = Mutex::new(None);
        let worker = |thread_index| {
            let work = || -> Result<()> {
                let mut scratch = Scratch::default();
                let mut compressed = Vec::new();
                loop {
                    if stopped.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let d = {
                        let mut state = state.lock().map_err(|_| Error::WorkerPanic)?;
                        let (file, offset, index) = &mut *state;
                        loop {
                            let next = if let Some(_map) = mapping {
                                #[cfg(feature = "mmap")]
                                {
                                    mapped_descriptor(_map, *index, *offset)?
                                }
                                #[cfg(not(feature = "mmap"))]
                                {
                                    unreachable!()
                                }
                            } else if sequential {
                                read_descriptor(file, *index, *offset, self.length)?
                            } else {
                                descriptor(file.get_mut(), *index, *offset, self.length)?
                            };
                            let Some(d) = next else {
                                return Ok(());
                            };
                            *offset = d.payload_offset + d.size as u64;
                            *index += 1;
                            if d.kind == BlockKind::Data {
                                if mapping.is_none() {
                                    if sequential {
                                        compressed.resize(d.size, 0);
                                        file.read_exact(&mut compressed)?;
                                    } else {
                                        #[cfg(not(unix))]
                                        read_payload(file.get_mut(), &d, &mut compressed)?;
                                    }
                                }
                                break d;
                            }
                            if mapping.is_none() && sequential {
                                file.seek_relative(d.size as i64)?;
                            }
                        }
                    };
                    if stopped.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    handler(&mut PbfBlock {
                        descriptor: &d,
                        compressed: &mut compressed,
                        source: if let Some(map) = mapping {
                            BlockSource::Slice(&map[d.payload_offset as usize..][..d.size])
                        } else if sequential {
                            BlockSource::Buffered
                        } else {
                            #[cfg(unix)]
                            {
                                BlockSource::File(&payload_file)
                            }
                            #[cfg(not(unix))]
                            {
                                BlockSource::Buffered
                            }
                        },
                        scratch: &mut scratch,
                        prepared: false,
                        parameters: Parameters::default(),
                        locations_allowed: self.locations_allowed,
                        thread_index,
                        cached_counts: Counts::default(),
                        count_mask: 0,
                    })?;
                }
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or(Err(Error::WorkerPanic));
            if let Err(error) = result {
                stopped.store(true, Ordering::Relaxed);
                let mut first = failure.lock().unwrap_or_else(|e| e.into_inner());
                if first.is_none() {
                    *first = Some(error);
                }
            }
        };
        if self.threads == 1 {
            worker(0);
        } else {
            std::thread::scope(|scope| {
                let mut handles = Vec::new();
                for i in 0..self.threads {
                    let worker = &worker;
                    match std::thread::Builder::new().spawn_scoped(scope, move || worker(i)) {
                        Ok(handle) => handles.push(handle),
                        Err(e) => {
                            stopped.store(true, Ordering::Relaxed);
                            *failure.lock().unwrap() = Some(Error::Io(e));
                            break;
                        }
                    }
                }
                for handle in handles {
                    if handle.join().is_err() {
                        *failure.lock().unwrap() = Some(Error::WorkerPanic);
                    }
                }
            });
        }
        let result = match failure.into_inner().unwrap_or_else(|e| e.into_inner()) {
            Some(error) => Err(error),
            None => Ok(()),
        };
        match &result {
            Ok(()) => self.log(
                crate::inputosmlog::LogLevel::Info,
                "PBF block read complete",
            ),
            Err(error) => self.log(crate::inputosmlog::LogLevel::Error, &error.to_string()),
        }
        result
    }
}

pub(crate) fn input_pbf(
    path: &Path,
    options: InputOptions,
    handler: &impl EntityHandler,
) -> Result<()> {
    let mut reader = PbfReader::open(path)?;
    reader.set_thread_count(options.threads);
    reader.read_blocks(|block| {
        let context = Context {
            file_type: FileType::Pbf,
            mode: Mode::Bulk,
            block_index: block.index(),
            thread_index: block.thread_index(),
        };
        block.decode_entities_with_strings(
            EntityOptions::all(options.decode_metadata),
            |group, strings| {
                let string = |sid: u32| -> Result<&[u8]> {
                    strings
                        .get(sid as usize)
                        .copied()
                        .ok_or(Error::Invalid("Invalid PBF string ID"))
                };
                let tags = |source: &[TagIds]| -> Result<Vec<Tag<'_>>> {
                    let mut result = Vec::with_capacity(source.len());
                    for t in source {
                        result.push(Tag {
                            key: string(t.key)?,
                            value: string(t.value)?,
                        });
                    }
                    Ok(result)
                };
                let metadata = |source: &[Metadata],
                                index: usize,
                                parameters: Parameters|
                 -> Result<Metadata> {
                    let mut m = source.get(index).copied().unwrap_or_default();
                    if m.present & Metadata::TIMESTAMP != 0 {
                        m.raw_timestamp =
                            parameters.timestamp_milliseconds(m.raw_timestamp)? / 1000;
                    }
                    if m.present & Metadata::USER != 0 {
                        string(m.user_sid)?;
                    }
                    Ok(m)
                };
                match group {
                    GroupBatch::Nodes(b) if b.context.count != 0 => {
                        let all_tags = tags(b.tags.values)?;
                        let mut nodes = Vec::with_capacity(b.context.count);
                        for i in 0..b.context.count {
                            nodes.push(Node {
                                id: b.ids[i],
                                raw_latitude: b
                                    .context
                                    .parameters
                                    .latitude_nanodegrees(b.raw_latitudes[i])?
                                    / 100,
                                raw_longitude: b
                                    .context
                                    .parameters
                                    .longitude_nanodegrees(b.raw_longitudes[i])?
                                    / 100,
                                tags: &all_tags
                                    [b.tags.offsets[i] as usize..b.tags.offsets[i + 1] as usize],
                                metadata: metadata(b.metadata, i, b.context.parameters)?,
                            });
                        }
                        handler.nodes(context, &nodes)?;
                    }
                    GroupBatch::Ways(b) if b.context.count != 0 => {
                        let all_tags = tags(b.tags.values)?;
                        let mut ways = Vec::with_capacity(b.context.count);
                        for i in 0..b.context.count {
                            ways.push(Way {
                                id: b.ids[i],
                                node_refs: &b.node_refs[i],
                                tags: &all_tags
                                    [b.tags.offsets[i] as usize..b.tags.offsets[i + 1] as usize],
                                metadata: metadata(b.metadata, i, b.context.parameters)?,
                            });
                        }
                        handler.ways(context, &ways)?;
                    }
                    GroupBatch::Relations(b) if b.context.count != 0 => {
                        let all_tags = tags(b.tags.values)?;
                        let mut members = Vec::with_capacity(b.member_ids.len());
                        for (i, &id) in b.member_ids.iter().enumerate() {
                            members.push(RelationMember {
                                id,
                                member_type: b.member_types[i],
                                role: string(b.member_roles[i])?,
                            });
                        }
                        let mut relations = Vec::with_capacity(b.context.count);
                        for i in 0..b.context.count {
                            relations.push(Relation {
                                id: b.ids[i],
                                members: &members[b.member_offsets[i] as usize
                                    ..b.member_offsets[i + 1] as usize],
                                tags: &all_tags
                                    [b.tags.offsets[i] as usize..b.tags.offsets[i + 1] as usize],
                                metadata: metadata(b.metadata, i, b.context.parameters)?,
                            });
                        }
                        handler.relations(context, &relations)?;
                    }
                    _ => {}
                }
                Ok(())
            },
        )
    })
}
