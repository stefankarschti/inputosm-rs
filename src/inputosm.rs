use std::{fmt, io, path::Path};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(&'static str),
    Message(String),
    Cancelled,
    WorkerPanic,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Invalid(s) => f.write_str(s),
            Self::Message(s) => f.write_str(s),
            Self::Cancelled => f.write_str("Read cancelled"),
            Self::WorkerPanic => f.write_str("Reader worker panicked"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
pub(crate) fn require(condition: bool, message: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(message))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Bulk,
    Create,
    Modify,
    Delete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    Pbf,
    Xml,
}
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub file_type: FileType,
    pub mode: Mode,
    pub block_index: usize,
    pub thread_index: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tag<'a> {
    pub key: &'a [u8],
    pub value: &'a [u8],
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Node<'a> {
    pub id: i64,
    /// Coordinates use units of 100 nanodegrees.
    pub raw_latitude: i64,
    pub raw_longitude: i64,
    pub tags: &'a [Tag<'a>],
    pub metadata: Metadata,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Way<'a> {
    pub id: i64,
    pub node_refs: &'a [i64],
    pub tags: &'a [Tag<'a>],
    pub metadata: Metadata,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct RelationMember<'a> {
    pub member_type: MemberType,
    pub id: i64,
    pub role: &'a [u8],
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Relation<'a> {
    pub id: i64,
    pub members: &'a [RelationMember<'a>],
    pub tags: &'a [Tag<'a>],
    pub metadata: Metadata,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parameters {
    pub granularity: i32,
    pub lat_offset: i64,
    pub lon_offset: i64,
    pub date_granularity: i32,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            granularity: 100,
            lat_offset: 0,
            lon_offset: 0,
            date_granularity: 1000,
        }
    }
}
impl Parameters {
    pub fn latitude_nanodegrees(self, raw: i64) -> Result<i64> {
        self.coordinate(raw, self.lat_offset)
    }
    pub fn longitude_nanodegrees(self, raw: i64) -> Result<i64> {
        self.coordinate(raw, self.lon_offset)
    }
    fn coordinate(self, raw: i64, offset: i64) -> Result<i64> {
        raw.checked_mul(i64::from(self.granularity))
            .and_then(|v| v.checked_add(offset))
            .ok_or(Error::Invalid("Coordinate overflow"))
    }
    pub fn timestamp_milliseconds(self, raw: i64) -> Result<i64> {
        raw.checked_mul(i64::from(self.date_granularity))
            .ok_or(Error::Invalid("Timestamp overflow"))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub nodes: usize,
    pub ways: usize,
    pub relations: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderBbox {
    pub left: i64,
    pub right: i64,
    pub top: i64,
    pub bottom: i64,
}
#[derive(Clone, Debug, Default)]
pub struct HeaderMetadata {
    pub bbox: Option<HeaderBbox>,
    pub required_features: Vec<Vec<u8>>,
    pub optional_features: Vec<Vec<u8>>,
    pub writingprogram: Option<Vec<u8>>,
    pub source: Option<Vec<u8>>,
    pub osmosis_replication_timestamp: Option<i64>,
    pub osmosis_replication_sequence_number: Option<i64>,
    pub osmosis_replication_base_url: Option<Vec<u8>>,
}
#[derive(Clone, Copy, Debug)]
pub struct NodeOptions {
    pub id: bool,
    pub latitude: bool,
    pub longitude: bool,
    pub tags: bool,
    pub metadata: bool,
}
impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            id: true,
            latitude: false,
            longitude: false,
            tags: false,
            metadata: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct WayOptions {
    pub tags: bool,
    pub node_refs: bool,
    pub metadata: bool,
    pub node_locations: bool,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct RelationOptions {
    pub tags: bool,
    pub member_ids: bool,
    pub member_types: bool,
    pub member_roles: bool,
    pub metadata: bool,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct EntityOptions {
    pub nodes: Option<NodeOptions>,
    pub ways: Option<WayOptions>,
    pub relations: Option<RelationOptions>,
}
impl EntityOptions {
    pub fn all(metadata: bool) -> Self {
        Self {
            nodes: Some(NodeOptions {
                id: true,
                latitude: true,
                longitude: true,
                tags: true,
                metadata,
            }),
            ways: Some(WayOptions {
                tags: true,
                node_refs: true,
                metadata,
                node_locations: false,
            }),
            relations: Some(RelationOptions {
                tags: true,
                member_ids: true,
                member_types: true,
                member_roles: true,
                metadata,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TagIds {
    pub key: u32,
    pub value: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Location {
    pub raw_latitude: i64,
    pub raw_longitude: i64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum MemberType {
    #[default]
    Node,
    Way,
    Relation,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupKind {
    #[default]
    Empty,
    Nodes,
    DenseNodes,
    Ways,
    Relations,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub raw_timestamp: i64,
    pub changeset: i64,
    pub version: i32,
    pub uid: i32,
    pub user_sid: u32,
    pub present: u8,
    pub visible: bool,
}
impl Default for Metadata {
    fn default() -> Self {
        Self {
            raw_timestamp: 0,
            changeset: 0,
            version: -1,
            uid: 0,
            user_sid: 0,
            present: 0,
            visible: true,
        }
    }
}
impl Metadata {
    pub const VERSION: u8 = 1;
    pub const TIMESTAMP: u8 = 2;
    pub const CHANGESET: u8 = 4;
    pub const UID: u8 = 8;
    pub const USER: u8 = 16;
    pub const VISIBLE: u8 = 32;
}
#[derive(Clone, Copy, Debug)]
pub struct ListView<'a, T> {
    pub offsets: &'a [u32],
    pub values: &'a [T],
}
impl<T> ListView<'_, T> {
    pub fn get(&self, index: usize) -> Option<&[T]> {
        let end = index.checked_add(1)?;
        self.values
            .get(*self.offsets.get(index)? as usize..*self.offsets.get(end)? as usize)
    }
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
impl<T> std::ops::Index<usize> for ListView<'_, T> {
    type Output = [T];
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("List index out of bounds")
    }
}
#[derive(Clone, Copy, Debug)]
pub struct BatchContext {
    pub kind: GroupKind,
    pub block_index: usize,
    pub group_index: usize,
    pub count: usize,
    pub parameters: Parameters,
}
#[derive(Debug)]
pub struct NodeBatch<'a> {
    pub context: BatchContext,
    pub fields: NodeOptions,
    pub ids: &'a [i64],
    pub raw_latitudes: &'a [i64],
    pub raw_longitudes: &'a [i64],
    pub tags: ListView<'a, TagIds>,
    pub metadata: &'a [Metadata],
}
#[derive(Debug)]
pub struct WayBatch<'a> {
    pub context: BatchContext,
    pub fields: WayOptions,
    pub ids: &'a [i64],
    pub tags: ListView<'a, TagIds>,
    pub node_refs: ListView<'a, i64>,
    pub node_locations: ListView<'a, Location>,
    pub locations_present: &'a [bool],
    pub metadata: &'a [Metadata],
}
#[derive(Debug)]
pub struct RelationBatch<'a> {
    pub context: BatchContext,
    pub fields: RelationOptions,
    pub ids: &'a [i64],
    pub tags: ListView<'a, TagIds>,
    pub member_offsets: &'a [u32],
    pub member_ids: &'a [i64],
    pub member_types: &'a [MemberType],
    pub member_roles: &'a [u32],
    pub metadata: &'a [Metadata],
}
#[derive(Debug)]
pub enum GroupBatch<'a> {
    Nodes(NodeBatch<'a>),
    Ways(WayBatch<'a>),
    Relations(RelationBatch<'a>),
    Empty(BatchContext),
}
impl GroupBatch<'_> {
    pub fn context(&self) -> BatchContext {
        match self {
            Self::Nodes(b) => b.context,
            Self::Ways(b) => b.context,
            Self::Relations(b) => b.context,
            Self::Empty(c) => *c,
        }
    }
    pub fn kind(&self) -> GroupKind {
        self.context().kind
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InputOptions {
    pub decode_metadata: bool,
    pub threads: usize,
}
impl Default for InputOptions {
    fn default() -> Self {
        Self {
            decode_metadata: false,
            threads: 1,
        }
    }
}
/// Return `Error::Cancelled` to stop an operation.
pub trait EntityHandler: Sync {
    fn nodes(&self, _context: Context, _nodes: &[Node<'_>]) -> Result<()> {
        Ok(())
    }
    fn ways(&self, _context: Context, _ways: &[Way<'_>]) -> Result<()> {
        Ok(())
    }
    fn relations(&self, _context: Context, _relations: &[Relation<'_>]) -> Result<()> {
        Ok(())
    }
}
pub fn input_file(
    path: impl AsRef<Path>,
    options: InputOptions,
    handler: &impl EntityHandler,
) -> Result<()> {
    let path = path.as_ref();
    match path.extension().and_then(|s| s.to_str()) {
        Some("pbf") => crate::inputosmpbf::input_pbf(path, options, handler),
        Some("osm" | "osc") => crate::inputosmxml::input_xml(path, options, handler),
        _ => Err(Error::Invalid("Unknown file extension")),
    }
}
