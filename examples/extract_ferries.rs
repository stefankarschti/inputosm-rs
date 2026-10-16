//! Find ferry ways, then read the coordinates of their referenced nodes.
mod common;
use inputosm::*;
use std::collections::{HashMap, HashSet};

fn main() -> Result<()> {
    let args = common::arguments(1, 1, "Usage: extract_ferries <file.osm.pbf>")?;
    let mut reader = common::reader(&args[0])?;
    let workers = common::workers(reader.thread_count(), Vec::<(i64, Vec<i64>)>::new);
    reader.read_blocks(|block| {
        if block.way_count()? == 0 {
            return Ok(());
        }
        let mut ferries = workers[block.thread_index()].0.lock().unwrap();
        block.decode_entities_with_strings(
            EntityOptions {
                ways: Some(WayOptions {
                    tags: true,
                    node_refs: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |group, strings| {
                if let GroupBatch::Ways(ways) = group {
                    for (i, &id) in ways.ids.iter().enumerate() {
                        let mut ferry = false;
                        for tag in ways.tags.get(i).unwrap() {
                            let key = strings
                                .get(tag.key as usize)
                                .ok_or(Error::Invalid("Invalid tag key"))?;
                            let value = strings
                                .get(tag.value as usize)
                                .ok_or(Error::Invalid("Invalid tag value"))?;
                            ferry |= *key == b"route" && *value == b"ferry";
                        }
                        if ferry {
                            ferries.push((id, ways.node_refs.get(i).unwrap().to_vec()));
                        }
                    }
                }
                Ok(())
            },
        )
    })?;
    let ferries: Vec<_> = workers
        .into_iter()
        .flat_map(|w| w.0.into_inner().unwrap())
        .collect();
    let wanted: HashSet<_> = ferries
        .iter()
        .flat_map(|(_, refs)| refs.iter().copied())
        .collect();
    println!("{} ferries", common::grouped(ferries.len() as u64));
    println!(
        "{} unique nodes used by ferries",
        common::grouped(wanted.len() as u64)
    );
    println!("retrieving ferry node coordinates...");
    let workers = common::workers(reader.thread_count(), HashMap::<i64, (i64, i64)>::new);
    reader.read_blocks(|block| {
        let mut positions = workers[block.thread_index()].0.lock().unwrap();
        block.decode_nodes(
            NodeOptions {
                id: true,
                latitude: true,
                longitude: true,
                ..Default::default()
            },
            |nodes| {
                for (i, &id) in nodes.ids.iter().enumerate() {
                    if wanted.contains(&id) {
                        let p = nodes.context.parameters;
                        positions.insert(
                            id,
                            (
                                p.latitude_nanodegrees(nodes.raw_latitudes[i])? / 100,
                                p.longitude_nanodegrees(nodes.raw_longitudes[i])? / 100,
                            ),
                        );
                    }
                }
                Ok(())
            },
        )
    })?;
    let positions: HashMap<_, _> = workers
        .into_iter()
        .flat_map(|w| w.0.into_inner().unwrap())
        .collect();
    let missing = wanted.len() - positions.len();
    if missing != 0 {
        eprintln!("{missing} referenced nodes are absent from the input");
    }
    println!("done.");
    Ok(())
}
