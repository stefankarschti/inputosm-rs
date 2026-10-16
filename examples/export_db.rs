//! Export the original binary records with checked lengths and writes.
mod common;
#[path = "common/export.rs"]
mod export;
use inputosm::*;
use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::Mutex,
};

struct Exporter {
    files: [Mutex<BufWriter<File>>; 3],
}
impl EntityHandler for Exporter {
    fn nodes(&self, _: Context, nodes: &[Node<'_>]) -> Result<()> {
        let mut out = self.files[0].lock().unwrap();
        for node in nodes {
            let lat = i32::try_from(node.raw_latitude)
                .map_err(|_| Error::Invalid("Latitude exceeds int32"))?;
            let lon = i32::try_from(node.raw_longitude)
                .map_err(|_| Error::Invalid("Longitude exceeds int32"))?;
            out.write_all(&node.id.to_le_bytes())?;
            out.write_all(&lat.to_le_bytes())?;
            out.write_all(&lon.to_le_bytes())?;
            export::binary_tags(&mut *out, node.tags)?;
        }
        Ok(())
    }
    fn ways(&self, _: Context, ways: &[Way<'_>]) -> Result<()> {
        let mut out = self.files[1].lock().unwrap();
        for way in ways {
            out.write_all(&way.id.to_le_bytes())?;
            export::count(&mut *out, way.node_refs.len())?;
            for id in way.node_refs {
                out.write_all(&id.to_le_bytes())?;
            }
            export::binary_tags(&mut *out, way.tags)?;
        }
        Ok(())
    }
    fn relations(&self, _: Context, relations: &[Relation<'_>]) -> Result<()> {
        let mut out = self.files[2].lock().unwrap();
        for relation in relations {
            out.write_all(&relation.id.to_le_bytes())?;
            export::count(&mut *out, relation.members.len())?;
            for member in relation.members {
                out.write_all(&member.id.to_le_bytes())?;
                export::terminated(&mut *out, member.role)?;
                out.write_all(&[member.member_type as u8])?;
            }
            export::binary_tags(&mut *out, relation.tags)?;
        }
        Ok(())
    }
}
fn main() -> Result<()> {
    let args = common::arguments(2, 2, "Usage: export_db <input-file> <new-output-directory>")?;
    export::directory(&args[1])?;
    let exporter = Exporter {
        files: [
            Mutex::new(export::output(&args[1], "node")?),
            Mutex::new(export::output(&args[1], "way")?),
            Mutex::new(export::output(&args[1], "relation")?),
        ],
    };
    input_file(
        &args[0],
        InputOptions {
            threads: common::parallelism(),
            ..Default::default()
        },
        &exporter,
    )?;
    for file in exporter.files {
        export::finish(&mut file.into_inner().unwrap())?;
    }
    Ok(())
}
