//! Experimental views for the profiling consumer. These types are not a public API.

use inputosm::*;

fn string<'a>(strings: &[&'a [u8]], sid: u32) -> Result<&'a [u8]> {
    strings
        .get(sid as usize)
        .copied()
        .ok_or(Error::Invalid("Invalid PBF string ID"))
}

fn tags<'a>(
    ids: &'a [TagIds],
    strings: &'a [&'a [u8]],
) -> impl Iterator<Item = Result<Tag<'a>>> + 'a {
    ids.iter().map(|t| {
        Ok(Tag {
            key: string(strings, t.key)?,
            value: string(strings, t.value)?,
        })
    })
}

pub struct NodeView<'a> {
    pub batch: &'a NodeBatch<'a>,
    pub strings: &'a [&'a [u8]],
    pub index: usize,
}
impl<'a> NodeView<'a> {
    pub fn id(&self) -> i64 {
        self.batch.ids[self.index]
    }
    pub fn coordinates(&self) -> Result<(i64, i64)> {
        let p = self.batch.context.parameters;
        Ok((
            p.latitude_nanodegrees(self.batch.raw_latitudes[self.index])? / 100,
            p.longitude_nanodegrees(self.batch.raw_longitudes[self.index])? / 100,
        ))
    }
    pub fn tags(&self) -> impl Iterator<Item = Result<Tag<'a>>> + 'a {
        tags(&self.batch.tags[self.index], self.strings)
    }
}

pub struct WayView<'a> {
    pub batch: &'a WayBatch<'a>,
    pub strings: &'a [&'a [u8]],
    pub index: usize,
}
impl<'a> WayView<'a> {
    pub fn id(&self) -> i64 {
        self.batch.ids[self.index]
    }
    pub fn node_refs(&self) -> &'a [i64] {
        &self.batch.node_refs[self.index]
    }
    pub fn tags(&self) -> impl Iterator<Item = Result<Tag<'a>>> + 'a {
        tags(&self.batch.tags[self.index], self.strings)
    }
}

pub struct RelationView<'a> {
    pub batch: &'a RelationBatch<'a>,
    pub strings: &'a [&'a [u8]],
    pub index: usize,
}
impl<'a> RelationView<'a> {
    pub fn id(&self) -> i64 {
        self.batch.ids[self.index]
    }
    pub fn members(&self) -> impl Iterator<Item = Result<RelationMember<'a>>> + 'a {
        let b = self.batch;
        let start = b.member_offsets[self.index] as usize;
        let end = b.member_offsets[self.index + 1] as usize;
        let strings = self.strings;
        (start..end).map(move |i| {
            Ok(RelationMember {
                id: b.member_ids[i],
                member_type: b.member_types[i],
                role: string(strings, b.member_roles[i])?,
            })
        })
    }
    pub fn tags(&self) -> impl Iterator<Item = Result<Tag<'a>>> + 'a {
        tags(&self.batch.tags[self.index], self.strings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_strings_return_errors() {
        let strings: &[&[u8]] = &[b"", b"name"];
        let ids = [TagIds { key: 1, value: 2 }];
        assert!(tags(&ids, strings).next().unwrap().is_err());
    }

    #[test]
    fn coordinates_use_parameters_and_check_overflow() {
        let mut batch = NodeBatch {
            context: BatchContext {
                kind: GroupKind::Nodes,
                block_index: 0,
                group_index: 0,
                count: 1,
                parameters: Parameters {
                    granularity: 200,
                    lat_offset: 100,
                    lon_offset: -100,
                    ..Default::default()
                },
            },
            fields: EntityOptions::all(false).nodes.unwrap(),
            ids: &[42],
            raw_latitudes: &[3],
            raw_longitudes: &[-4],
            tags: ListView {
                offsets: &[0, 0],
                values: &[],
            },
            metadata: &[],
        };
        assert_eq!(
            NodeView {
                batch: &batch,
                strings: &[],
                index: 0
            }
            .coordinates()
            .unwrap(),
            (7, -9)
        );
        batch.raw_latitudes = &[i64::MAX];
        assert!(
            NodeView {
                batch: &batch,
                strings: &[],
                index: 0
            }
            .coordinates()
            .is_err()
        );
    }
}
