//! Count nodes by absolute latitude in whole degrees.
mod common;
use inputosm::{Error, NodeOptions, Result};

fn main() -> Result<()> {
    let args = common::arguments(1, 1, "Usage: lat_stat <file.osm.pbf>")?;
    let mut reader = common::reader(&args[0])?;
    println!("{}\nrunning on {} threads", args[0], reader.thread_count());
    let workers = common::workers(reader.thread_count(), || [0u64; 91]);
    reader.read_blocks(|block| {
        let mut counts = workers[block.thread_index()].0.lock().unwrap();
        block.decode_nodes(
            NodeOptions {
                id: false,
                latitude: true,
                ..Default::default()
            },
            |batch| {
                for &raw in batch.raw_latitudes {
                    let latitude = batch
                        .context
                        .parameters
                        .latitude_nanodegrees(raw)?
                        .unsigned_abs();
                    if latitude > 90_000_000_000 {
                        return Err(Error::Invalid("Latitude exceeds 90 degrees"));
                    }
                    counts[(latitude / 1_000_000_000) as usize] += 1;
                }
                Ok(())
            },
        )
    })?;
    let mut counts = [0u64; 91];
    for worker in workers {
        for (count, value) in counts.iter_mut().zip(worker.0.into_inner().unwrap()) {
            *count += value;
        }
    }
    let sum = counts.iter().sum();
    let total = common::grouped(sum);
    let width = 13.max(total.len());
    println!("|   degree | {:^width$} |   percent  |", "count");
    println!("| -------- | {:-<width$} | ---------- |", "");
    for (degree, count) in counts.into_iter().enumerate() {
        let percent = if sum == 0 {
            0.0
        } else {
            count as f64 * 100.0 / sum as f64
        };
        println!(
            "| {degree:8} | {:>width$} | {percent:9.2}% |",
            common::grouped(count)
        );
    }
    println!("|   total  | {total:>width$} |    100.00% |");
    Ok(())
}
