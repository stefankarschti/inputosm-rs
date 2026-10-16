//! Count entities through the selective PBF API.
mod common;
use inputosm::Result;

fn main() -> Result<()> {
    let args = common::arguments(1, 1, "Usage: count_entity <file.osm.pbf>")?;
    let mut reader = common::reader(&args[0])?;
    println!("{}\nrunning on {} threads", args[0], reader.thread_count());
    let workers = common::workers(reader.thread_count(), || [0u64; 3]);
    reader.read_blocks(|block| {
        let counts = block.counts()?;
        let mut worker = workers[block.thread_index()].0.lock().unwrap();
        for (total, count) in worker
            .iter_mut()
            .zip([counts.nodes, counts.ways, counts.relations])
        {
            *total += count as u64;
        }
        Ok(())
    })?;
    let mut totals = [0u64; 3];
    for worker in workers {
        for (total, count) in totals.iter_mut().zip(worker.0.into_inner().unwrap()) {
            *total += count;
        }
    }
    for (name, count) in ["nodes", "ways", "relations"].into_iter().zip(totals) {
        println!("{name}: {}", common::grouped(count));
    }
    Ok(())
}
