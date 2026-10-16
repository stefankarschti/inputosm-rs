//! Count OSMData blocks without decoding their contents.
mod common;
use inputosm::Result;

fn main() -> Result<()> {
    let args = common::arguments(1, 1, "Usage: count_blocks <file.osm.pbf>")?;
    let mut reader = common::reader(&args[0])?;
    let workers = common::workers(reader.thread_count(), || 0u64);
    reader.read_blocks(|block| {
        *workers[block.thread_index()].0.lock().unwrap() += 1;
        Ok(())
    })?;
    let count = workers.into_iter().map(|w| w.0.into_inner().unwrap()).sum();
    println!("blocks: {}", common::grouped(count));
    Ok(())
}
