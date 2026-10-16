//! Borrow complete PBF entities without constructing entity arrays.

use crate::*;

fn string<'a>(strings: &[&'a [u8]], sid: u32) -> Result<&'a [u8]> {
    strings
        .get(sid as usize)
        .copied()
        .ok_or(Error::Invalid("Invalid PBF string ID"))
}
fn tags<'a>(
    ids: &'a [TagIds],
    strings: &'a [&'a [u8]],
) -> impl ExactSizeIterator<Item = Result<Tag<'a>>> + 'a {
    ids.iter().map(|t| {
        Ok(Tag {
            key: string(strings, t.key)?,
            value: string(strings, t.value)?,
        })
    })
}
fn metadata(
    source: &[Metadata],
    index: usize,
    parameters: Parameters,
    strings: &[&[u8]],
) -> Result<Option<Metadata>> {
    let Some(mut m) = source.get(index).copied() else {
        return Ok(None);
    };
    if m.present & Metadata::TIMESTAMP != 0 {
        m.raw_timestamp = parameters.timestamp_milliseconds(m.raw_timestamp)? / 1000;
    }
    if m.present & Metadata::USER != 0 {
        string(strings, m.user_sid)?;
    }
    Ok(Some(m))
}
fn user<'a>(source: &[Metadata], index: usize, strings: &[&'a [u8]]) -> Result<Option<&'a [u8]>> {
    match source.get(index) {
        Some(m) if m.present & Metadata::USER != 0 => Ok(Some(string(strings, m.user_sid)?)),
        _ => Ok(None),
    }
}

/// Entity columns with borrowed accessors. Constructors remain internal.
#[derive(Debug)]
pub enum EntityViews<'a> {
    Nodes(NodeViews<'a>),
    Ways(WayViews<'a>),
    Relations(RelationViews<'a>),
    Empty(BatchContext),
}
impl<'a> EntityViews<'a> {
    pub(crate) fn new(group: GroupBatch<'a>, strings: &'a [&'a [u8]]) -> Self {
        match group {
            GroupBatch::Nodes(batch) => Self::Nodes(NodeViews { batch, strings }),
            GroupBatch::Ways(batch) => Self::Ways(WayViews { batch, strings }),
            GroupBatch::Relations(batch) => Self::Relations(RelationViews { batch, strings }),
            GroupBatch::Empty(context) => Self::Empty(context),
        }
    }
    pub fn context(&self) -> BatchContext {
        match self {
            Self::Nodes(b) => b.context(),
            Self::Ways(b) => b.context(),
            Self::Relations(b) => b.context(),
            Self::Empty(c) => *c,
        }
    }
}

/// Borrow a node group and its string table.
#[derive(Debug)]
pub struct NodeViews<'a> {
    batch: NodeBatch<'a>,
    strings: &'a [&'a [u8]],
}
impl NodeViews<'_> {
    pub fn context(&self) -> BatchContext {
        self.batch.context
    }
    pub fn len(&self) -> usize {
        self.batch.context.count
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<NodeView<'_>> {
        (index < self.len()).then_some(NodeView { group: self, index })
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = NodeView<'_>> + '_ {
        (0..self.len()).map(|index| NodeView { group: self, index })
    }
}
/// Borrow one node from a decoded group.
#[derive(Clone, Copy, Debug)]
pub struct NodeView<'a> {
    group: &'a NodeViews<'a>,
    index: usize,
}
impl<'a> NodeView<'a> {
    pub fn id(&self) -> i64 {
        self.group.batch.ids[self.index]
    }
    /// Resolve tag strings as the iterator advances.
    pub fn tags(&self) -> impl ExactSizeIterator<Item = Result<Tag<'a>>> + 'a {
        tags(&self.group.batch.tags[self.index], self.group.strings)
    }
    /// Return metadata with timestamps in seconds, or `None` when decoding is disabled.
    pub fn metadata(&self) -> Result<Option<Metadata>> {
        let b = &self.group.batch;
        metadata(
            b.metadata,
            self.index,
            b.context.parameters,
            self.group.strings,
        )
    }
    /// Resolve the username when metadata contains a user string ID.
    pub fn user(&self) -> Result<Option<&'a [u8]>> {
        user(self.group.batch.metadata, self.index, self.group.strings)
    }
    /// Return latitude and longitude in units of 100 nanodegrees.
    pub fn coordinates(&self) -> Result<(i64, i64)> {
        let b = &self.group.batch;
        let p = b.context.parameters;
        Ok((
            p.latitude_nanodegrees(b.raw_latitudes[self.index])? / 100,
            p.longitude_nanodegrees(b.raw_longitudes[self.index])? / 100,
        ))
    }
}

/// Borrow a way group and its string table.
#[derive(Debug)]
pub struct WayViews<'a> {
    batch: WayBatch<'a>,
    strings: &'a [&'a [u8]],
}
impl WayViews<'_> {
    pub fn context(&self) -> BatchContext {
        self.batch.context
    }
    pub fn len(&self) -> usize {
        self.batch.context.count
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<WayView<'_>> {
        (index < self.len()).then_some(WayView { group: self, index })
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = WayView<'_>> + '_ {
        (0..self.len()).map(|index| WayView { group: self, index })
    }
}
/// Borrow one way from a decoded group.
#[derive(Clone, Copy, Debug)]
pub struct WayView<'a> {
    group: &'a WayViews<'a>,
    index: usize,
}
impl<'a> WayView<'a> {
    pub fn id(&self) -> i64 {
        self.group.batch.ids[self.index]
    }
    /// Resolve tag strings as the iterator advances.
    pub fn tags(&self) -> impl ExactSizeIterator<Item = Result<Tag<'a>>> + 'a {
        tags(&self.group.batch.tags[self.index], self.group.strings)
    }
    /// Return metadata with timestamps in seconds, or `None` when decoding is disabled.
    pub fn metadata(&self) -> Result<Option<Metadata>> {
        let b = &self.group.batch;
        metadata(
            b.metadata,
            self.index,
            b.context.parameters,
            self.group.strings,
        )
    }
    /// Resolve the username when metadata contains a user string ID.
    pub fn user(&self) -> Result<Option<&'a [u8]>> {
        user(self.group.batch.metadata, self.index, self.group.strings)
    }
    pub fn node_refs(&self) -> &'a [i64] {
        &self.group.batch.node_refs[self.index]
    }
}

/// Borrow a relation group and its string table.
#[derive(Debug)]
pub struct RelationViews<'a> {
    batch: RelationBatch<'a>,
    strings: &'a [&'a [u8]],
}
impl RelationViews<'_> {
    pub fn context(&self) -> BatchContext {
        self.batch.context
    }
    pub fn len(&self) -> usize {
        self.batch.context.count
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<RelationView<'_>> {
        (index < self.len()).then_some(RelationView { group: self, index })
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = RelationView<'_>> + '_ {
        (0..self.len()).map(|index| RelationView { group: self, index })
    }
}
/// Borrow one relation from a decoded group.
#[derive(Clone, Copy, Debug)]
pub struct RelationView<'a> {
    group: &'a RelationViews<'a>,
    index: usize,
}
impl<'a> RelationView<'a> {
    pub fn id(&self) -> i64 {
        self.group.batch.ids[self.index]
    }
    /// Resolve tag strings as the iterator advances.
    pub fn tags(&self) -> impl ExactSizeIterator<Item = Result<Tag<'a>>> + 'a {
        tags(&self.group.batch.tags[self.index], self.group.strings)
    }
    /// Return metadata with timestamps in seconds, or `None` when decoding is disabled.
    pub fn metadata(&self) -> Result<Option<Metadata>> {
        let b = &self.group.batch;
        metadata(
            b.metadata,
            self.index,
            b.context.parameters,
            self.group.strings,
        )
    }
    /// Resolve the username when metadata contains a user string ID.
    pub fn user(&self) -> Result<Option<&'a [u8]>> {
        user(self.group.batch.metadata, self.index, self.group.strings)
    }
    /// Resolve member roles as the iterator advances.
    pub fn members(&self) -> impl ExactSizeIterator<Item = Result<RelationMember<'a>>> + 'a {
        let b = &self.group.batch;
        let start = b.member_offsets[self.index] as usize;
        let end = b.member_offsets[self.index + 1] as usize;
        let strings = self.group.strings;
        (start..end).map(move |i| {
            Ok(RelationMember {
                id: b.member_ids[i],
                member_type: b.member_types[i],
                role: string(strings, b.member_roles[i])?,
            })
        })
    }
}

/// Entity views cannot escape their callback.
///
/// ```compile_fail
/// use inputosm::{EntityViews, PbfReader, Result};
/// fn retain(reader: &mut PbfReader) -> Result<()> {
///     let mut retained = None;
///     reader.read_block(1, |block| {
///         block.decode_entity_views(false, |group| {
///             if let EntityViews::Nodes(nodes) = group {
///                 retained = nodes.get(0);
///             }
///             Ok(())
///         })
///     })?;
///     println!("{:?}", retained);
///     Ok(())
/// }
/// ```
pub mod lifetime_contract {}
