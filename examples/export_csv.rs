//! Export the original CSV fields with buffered writes.
mod common;
#[path = "common/export.rs"]
mod export;
use inputosm::*;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    sync::Mutex,
};

type Positions = HashMap<i64, (i64, i64)>;
struct Nodes {
    state: Mutex<(BufWriter<File>, Positions)>,
}
impl EntityHandler for Nodes {
    fn nodes(&self, _: Context, nodes: &[Node<'_>]) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let (out, positions) = &mut *state;
        for node in nodes {
            positions.insert(node.id, (node.raw_latitude, node.raw_longitude));
            write!(
                out,
                "{};0;0;{};{};",
                node.id, node.metadata.raw_timestamp, node.metadata.changeset
            )?;
            export::csv_tags(out, node.tags)?;
            writeln!(
                out,
                ";POINT({:.7} {:.7})",
                node.raw_latitude as f64 / 1e7,
                node.raw_longitude as f64 / 1e7
            )?;
        }
        Ok(())
    }
}
struct WaysAndRelations {
    positions: Positions,
    ways: Mutex<(BufWriter<File>, BufWriter<File>)>,
    relations: Mutex<(BufWriter<File>, BufWriter<File>)>,
}
impl EntityHandler for WaysAndRelations {
    fn ways(&self, _: Context, ways: &[Way<'_>]) -> Result<()> {
        let mut outputs = self.ways.lock().unwrap();
        let (out, refs) = &mut *outputs;
        for way in ways {
            write!(
                out,
                "{};0;0;{};{};",
                way.id, way.metadata.raw_timestamp, way.metadata.changeset
            )?;
            export::csv_tags(out, way.tags)?;
            write!(out, ";{{")?;
            for (sequence, id) in way.node_refs.iter().enumerate() {
                writeln!(refs, "{};{id};{sequence}", way.id)?;
                if sequence != 0 {
                    write!(out, ",")?;
                }
                write!(out, "{id}")?;
            }
            write!(out, "}};BBOX();LINESTRING(")?;
            for (sequence, id) in way.node_refs.iter().enumerate() {
                if sequence != 0 {
                    write!(out, ",")?;
                }
                let (lat, lon) = self.positions.get(id).copied().unwrap_or_default();
                write!(out, "{:.7} {:.7}", lat as f64 / 1e7, lon as f64 / 1e7)?;
            }
            writeln!(out, ")")?;
        }
        Ok(())
    }
    fn relations(&self, _: Context, relations: &[Relation<'_>]) -> Result<()> {
        let mut outputs = self.relations.lock().unwrap();
        let (out, members) = &mut *outputs;
        for relation in relations {
            write!(
                out,
                "{};0;0;{};{};",
                relation.id, relation.metadata.raw_timestamp, relation.metadata.changeset
            )?;
            export::csv_tags(out, relation.tags)?;
            writeln!(out)?;
            for (sequence, member) in relation.members.iter().enumerate() {
                let kind = match member.member_type {
                    MemberType::Node => 'N',
                    MemberType::Way => 'W',
                    MemberType::Relation => 'R',
                };
                write!(members, "{};{};{kind};\"", relation.id, member.id)?;
                export::csv_text(members, member.role)?;
                writeln!(members, "\";{sequence}")?;
            }
        }
        Ok(())
    }
}
fn main() -> Result<()> {
    let args = common::arguments(
        2,
        2,
        "Usage: export_csv <input-file> <new-output-directory>",
    )?;
    export::directory(&args[1])?;
    let nodes = Nodes {
        state: Mutex::new((export::output(&args[1], "nodes.csv")?, HashMap::new())),
    };
    let options = InputOptions {
        decode_metadata: true,
        threads: common::parallelism(),
    };
    input_file(&args[0], options, &nodes)?;
    let (mut output, positions) = nodes.state.into_inner().unwrap();
    export::finish(&mut output)?;
    let other = WaysAndRelations {
        positions,
        ways: Mutex::new((
            export::output(&args[1], "ways.csv")?,
            export::output(&args[1], "way_node.csv")?,
        )),
        relations: Mutex::new((
            export::output(&args[1], "relations.csv")?,
            export::output(&args[1], "relation_members.csv")?,
        )),
    };
    input_file(&args[0], options, &other)?;
    for pair in [other.ways, other.relations] {
        let (mut first, mut second) = pair.into_inner().unwrap();
        export::finish(&mut first)?;
        export::finish(&mut second)?;
    }
    Ok(())
}
