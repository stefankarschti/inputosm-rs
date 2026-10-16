//! Compute entity totals and maxima for complete PBF blocks.
mod common;
use inputosm::*;

#[derive(Clone, Copy, Default)]
struct BlockTotals {
    count: u64,
    tags: u64,
    references: u64,
    with_tags: u64,
    max_id: i64,
    max_raw_timestamp: i64,
}
impl BlockTotals {
    fn add(&mut self, ids: &[i64], tags: ListView<'_, TagIds>, metadata: &[Metadata]) {
        self.count += ids.len() as u64;
        self.tags += tags.values.len() as u64;
        self.with_tags += tags.offsets.windows(2).filter(|w| w[0] != w[1]).count() as u64;
        self.max_id = self.max_id.max(ids.iter().copied().max().unwrap_or(0));
        for value in metadata {
            if value.present & Metadata::TIMESTAMP != 0 {
                self.max_raw_timestamp = self.max_raw_timestamp.max(value.raw_timestamp);
            }
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Statistics {
    count: u64,
    with_tags: u64,
    max_count: u64,
    max_tags: u64,
    max_references: u64,
    max_id: i64,
    max_timestamp: i64,
}
impl Statistics {
    fn add(&mut self, block: BlockTotals, granularity: i32) -> Result<()> {
        let timestamp =
            i64::try_from(i128::from(block.max_raw_timestamp) * i128::from(granularity) / 1000)
                .map_err(|_| Error::Invalid("Timestamp exceeds the supported range"))?;
        self.merge(Self {
            count: block.count,
            with_tags: block.with_tags,
            max_count: block.count,
            max_tags: block.tags,
            max_references: block.references,
            max_id: block.max_id,
            max_timestamp: timestamp,
        });
        Ok(())
    }
    fn merge(&mut self, other: Self) {
        self.count += other.count;
        self.with_tags += other.with_tags;
        self.max_count = self.max_count.max(other.max_count);
        self.max_tags = self.max_tags.max(other.max_tags);
        self.max_references = self.max_references.max(other.max_references);
        self.max_id = self.max_id.max(other.max_id);
        self.max_timestamp = self.max_timestamp.max(other.max_timestamp);
    }
}
fn main() -> Result<()> {
    let args = common::arguments(1, 2, "Usage: statistics <file.osm.pbf> [read-metadata]")?;
    let metadata = args.len() == 2;
    let mut reader = common::reader(&args[0])?;
    println!("{}", args[0]);
    if metadata {
        println!("reading metadata");
    }
    println!("running on {} threads", reader.thread_count());
    let workers = common::workers(reader.thread_count(), || {
        ([Statistics::default(); 3], 0usize)
    });
    let options = EntityOptions {
        nodes: Some(NodeOptions {
            id: true,
            tags: true,
            metadata,
            ..Default::default()
        }),
        ways: Some(WayOptions {
            tags: true,
            node_refs: true,
            metadata,
            ..Default::default()
        }),
        relations: Some(RelationOptions {
            tags: true,
            member_types: true,
            metadata,
            ..Default::default()
        }),
    };
    reader.read_blocks(|block| {
        let parameters = block.parameters()?;
        let mut totals = [BlockTotals::default(); 3];
        block.decode_entities(options, |group| {
            match group {
                GroupBatch::Nodes(b) => totals[0].add(b.ids, b.tags, b.metadata),
                GroupBatch::Ways(b) => {
                    totals[1].add(b.ids, b.tags, b.metadata);
                    totals[1].references += b.node_refs.values.len() as u64;
                }
                GroupBatch::Relations(b) => {
                    totals[2].add(b.ids, b.tags, b.metadata);
                    totals[2].references += b.member_types.len() as u64;
                }
                GroupBatch::Empty(_) => (),
            }
            Ok(())
        })?;
        let mut worker = workers[block.thread_index()].0.lock().unwrap();
        for (stat, total) in worker.0.iter_mut().zip(totals) {
            stat.add(total, parameters.date_granularity)?;
        }
        worker.1 = worker.1.max(block.index());
        Ok(())
    })?;
    let mut totals = [Statistics::default(); 3];
    let mut max_index = 0;
    for worker in workers {
        let (stats, index) = worker.0.into_inner().unwrap();
        for (total, stat) in totals.iter_mut().zip(stats) {
            total.merge(stat);
        }
        max_index = max_index.max(index);
    }
    let singular = ["node", "way", "relation"];
    let plural = ["nodes", "ways", "relations"];
    for (name, stat) in plural.iter().zip(&totals) {
        println!("{name}: {}", common::grouped(stat.count));
    }
    for (i, stat) in totals.iter().enumerate() {
        println!(
            "max {} per block: {}",
            plural[i],
            common::grouped(stat.max_count)
        );
        println!(
            "max {} tags per block: {}",
            singular[i],
            common::grouped(stat.max_tags)
        );
        if i != 0 {
            println!(
                "max {} {} per block: {}",
                singular[i],
                if i == 1 { "nodes" } else { "members" },
                common::grouped(stat.max_references)
            );
        }
    }
    for (name, stat) in singular.iter().zip(&totals) {
        println!(
            "max {name} timestamp: {} GMT",
            timeutil::timestamp_to_string(stat.max_timestamp)
        );
    }
    println!("max file block index: {max_index}");
    for (name, stat) in plural.iter().zip(&totals) {
        println!("{name} with tags: {}", common::grouped(stat.with_tags));
    }
    for (name, stat) in singular.iter().zip(&totals) {
        println!("max {name} id: {}", stat.max_id);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamp_conversion_does_not_require_milliseconds_to_fit() {
        let mut stats = Statistics::default();
        stats
            .add(
                BlockTotals {
                    max_raw_timestamp: i64::MAX,
                    ..Default::default()
                },
                500,
            )
            .unwrap();
        assert_eq!(stats.max_timestamp, i64::MAX / 2);
        assert!(
            stats
                .add(
                    BlockTotals {
                        max_raw_timestamp: i64::MAX,
                        ..Default::default()
                    },
                    2000
                )
                .is_err()
        );
    }
}
