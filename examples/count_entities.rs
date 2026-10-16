use inputosm::{PbfReader, Result};
use std::sync::atomic::{AtomicUsize, Ordering};
fn main() -> Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .expect("Usage: count_entities FILE.pbf");
    let mut reader = PbfReader::open(path)?;
    reader.set_max_thread_count();
    let nodes = AtomicUsize::new(0);
    let ways = AtomicUsize::new(0);
    let relations = AtomicUsize::new(0);
    reader.read_blocks(|block| {
        let counts = block.counts()?;
        nodes.fetch_add(counts.nodes, Ordering::Relaxed);
        ways.fetch_add(counts.ways, Ordering::Relaxed);
        relations.fetch_add(counts.relations, Ordering::Relaxed);
        Ok(())
    })?;
    println!(
        "nodes={} ways={} relations={}",
        nodes.into_inner(),
        ways.into_inner(),
        relations.into_inner()
    );
    Ok(())
}
