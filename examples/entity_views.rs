//! Read node tags and coordinates through borrowed PBF views.

use inputosm::{EntityViews, Error, PbfReadMode, PbfReader, Result};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let path = args.get(1).ok_or(Error::Invalid("Supply a PBF file"))?;
    let mut reader = if args.get(2).is_some_and(|arg| arg == "--mmap") {
        #[cfg(feature = "mmap")]
        {
            // SAFETY: With --mmap, the user must keep the input file unchanged until this program exits.
            unsafe { PbfReader::open_mmap(path)? }
        }
        #[cfg(not(feature = "mmap"))]
        {
            return Err(Error::Invalid("Enable the mmap feature"));
        }
    } else {
        PbfReader::open(path)?
    };
    if args.get(2).is_some_and(|arg| arg == "--sequential") {
        reader.set_read_mode(PbfReadMode::Sequential);
    }
    reader.read_entity_views(false, |_, group| {
        if let EntityViews::Nodes(nodes) = group {
            for node in nodes.iter() {
                let coordinates = node.coordinates()?;
                for tag in node.tags() {
                    let tag = tag?;
                    println!(
                        "{} {:?} {:?}={:?}",
                        node.id(),
                        coordinates,
                        tag.key,
                        tag.value
                    );
                }
            }
        }
        Ok(())
    })
}
