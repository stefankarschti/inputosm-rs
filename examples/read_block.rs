//! Read one block, with an optional block index.
mod common;
use inputosm::{Error, Result};

fn main() -> Result<()> {
    let args = common::arguments(
        2,
        3,
        "Usage: read_block <file.osm.pbf> <block-index> [--index]",
    )?;
    let index = args[1]
        .parse::<usize>()
        .map_err(|_| Error::Invalid("Invalid block index"))?;
    let mut reader = common::reader(&args[0])?;
    if let Some(flag) = args.get(2) {
        if flag != "--index" {
            return Err(Error::Invalid("Expected --index"));
        }
        reader.build_index()?;
    }
    reader.read_block(index, |block| {
        let counts = block.counts()?;
        println!(
            "block={} offset={} nodes={} ways={} relations={}",
            block.index(),
            block.file_offset(),
            counts.nodes,
            counts.ways,
            counts.relations
        );
        Ok(())
    })
}
