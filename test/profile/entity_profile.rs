//! Compare entity objects with experimental views under the same consumer.

mod views;

use inputosm::*;
use std::{hint::black_box, sync::Mutex, time::Instant};
use views::{NodeView, RelationView, WayView};

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
struct Totals {
    nodes: u64,
    ways: u64,
    relations: u64,
    checksum: u64,
}

struct Digest(u64);
impl Digest {
    fn new(kind: u64, id: i64) -> Self {
        let mut result = Self(kind);
        result.number(id as u64);
        result
    }
    fn number(&mut self, value: u64) {
        self.0 = self.0.rotate_left(7).wrapping_add(value);
    }
    fn bytes(&mut self, bytes: &[u8]) {
        self.number(bytes.len() as u64);
        for &byte in bytes {
            self.number(u64::from(byte));
        }
    }
    fn tags<'a>(&mut self, tags: impl Iterator<Item = Result<Tag<'a>>>) -> Result<()> {
        for tag in tags {
            let tag = tag?;
            self.bytes(tag.key);
            self.bytes(tag.value);
        }
        Ok(())
    }
}

#[repr(align(64))]
struct Counter(Mutex<Totals>);

struct Handler<'a>(&'a [Counter]);
impl EntityHandler for Handler<'_> {
    fn nodes(&self, context: Context, nodes: &[Node<'_>]) -> Result<()> {
        let mut total = self.0[context.thread_index].0.lock().unwrap();
        total.nodes += nodes.len() as u64;
        for n in nodes {
            let mut h = Digest::new(1, n.id);
            h.number(n.raw_latitude as u64);
            h.number(n.raw_longitude as u64);
            h.tags(n.tags.iter().copied().map(Ok))?;
            total.checksum = total.checksum.wrapping_add(h.0);
        }
        Ok(())
    }
    fn ways(&self, context: Context, ways: &[Way<'_>]) -> Result<()> {
        let mut total = self.0[context.thread_index].0.lock().unwrap();
        total.ways += ways.len() as u64;
        for w in ways {
            let mut h = Digest::new(2, w.id);
            h.tags(w.tags.iter().copied().map(Ok))?;
            h.number(w.node_refs.len() as u64);
            for &id in w.node_refs {
                h.number(id as u64);
            }
            total.checksum = total.checksum.wrapping_add(h.0);
        }
        Ok(())
    }
    fn relations(&self, context: Context, relations: &[Relation<'_>]) -> Result<()> {
        let mut total = self.0[context.thread_index].0.lock().unwrap();
        total.relations += relations.len() as u64;
        for r in relations {
            let mut h = Digest::new(3, r.id);
            h.tags(r.tags.iter().copied().map(Ok))?;
            h.number(r.members.len() as u64);
            for m in r.members {
                h.number(m.id as u64);
                h.number(m.member_type as u64);
                h.bytes(m.role);
            }
            total.checksum = total.checksum.wrapping_add(h.0);
        }
        Ok(())
    }
}

fn consume_views(group: GroupBatch<'_>, strings: &[&[u8]], total: &mut Totals) -> Result<()> {
    match group {
        GroupBatch::Nodes(b) => {
            total.nodes += b.context.count as u64;
            for index in 0..b.context.count {
                let n = NodeView {
                    batch: &b,
                    strings,
                    index,
                };
                let mut h = Digest::new(1, n.id());
                let (lat, lon) = n.coordinates()?;
                h.number(lat as u64);
                h.number(lon as u64);
                h.tags(n.tags())?;
                total.checksum = total.checksum.wrapping_add(h.0);
            }
        }
        GroupBatch::Ways(b) => {
            total.ways += b.context.count as u64;
            for index in 0..b.context.count {
                let w = WayView {
                    batch: &b,
                    strings,
                    index,
                };
                let mut h = Digest::new(2, w.id());
                h.tags(w.tags())?;
                h.number(w.node_refs().len() as u64);
                for &id in w.node_refs() {
                    h.number(id as u64);
                }
                total.checksum = total.checksum.wrapping_add(h.0);
            }
        }
        GroupBatch::Relations(b) => {
            total.relations += b.context.count as u64;
            for index in 0..b.context.count {
                let r = RelationView {
                    batch: &b,
                    strings,
                    index,
                };
                let mut h = Digest::new(3, r.id());
                h.tags(r.tags())?;
                h.number((b.member_offsets[index + 1] - b.member_offsets[index]) as u64);
                for m in r.members() {
                    let m = m?;
                    h.number(m.id as u64);
                    h.number(m.member_type as u64);
                    h.bytes(m.role);
                }
                total.checksum = total.checksum.wrapping_add(h.0);
            }
        }
        GroupBatch::Empty(_) => {}
    }
    Ok(())
}

pub(crate) fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || !matches!(args[2].as_str(), "entities" | "views") {
        return Err(Error::Invalid(
            "Usage: entity_profile MAP entities|views THREADS",
        ));
    }
    let threads: usize = args[3]
        .parse()
        .map_err(|_| Error::Invalid("Invalid thread count"))?;
    if !(1..=256).contains(&threads) {
        return Err(Error::Invalid("Invalid thread count"));
    }
    let counters: Vec<_> = (0..threads)
        .map(|_| Counter(Mutex::new(Totals::default())))
        .collect();
    let start = Instant::now();
    if args[2] == "entities" {
        input_file(
            &args[1],
            InputOptions {
                threads,
                decode_metadata: false,
            },
            &Handler(&counters),
        )?;
    } else {
        let mut reader = PbfReader::open(&args[1])?;
        reader.set_thread_count(threads);
        reader.read_blocks(|block| {
            let thread = block.thread_index();
            block.decode_entities_with_strings(EntityOptions::all(false), |group, strings| {
                consume_views(group, strings, &mut counters[thread].0.lock().unwrap())
            })
        })?;
    }
    let seconds = start.elapsed().as_secs_f64();
    let mut total = Totals::default();
    for counter in counters {
        let t = counter.0.into_inner().unwrap();
        total.nodes += t.nodes;
        total.ways += t.ways;
        total.relations += t.relations;
        total.checksum = total.checksum.wrapping_add(t.checksum);
    }
    black_box(total);
    println!(
        "mode,threads,seconds,nodes,ways,relations,checksum,node_bytes,way_bytes,relation_bytes"
    );
    println!(
        "{},{},{:.9},{},{},{},{},{},{},{}",
        args[2],
        threads,
        seconds,
        total.nodes,
        total.ways,
        total.relations,
        total.checksum,
        size_of::<Node<'_>>(),
        size_of::<Way<'_>>(),
        size_of::<Relation<'_>>()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_match_entities_on_complete_fixtures() {
        for name in ["ordinary.osm.pbf", "comprehensive.osm.pbf"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(name);
            for threads in [1, 4] {
                let entities: Vec<_> = (0..threads)
                    .map(|_| Counter(Mutex::new(Totals::default())))
                    .collect();
                input_file(
                    &path,
                    InputOptions {
                        threads,
                        decode_metadata: false,
                    },
                    &Handler(&entities),
                )
                .unwrap();
                let views = Mutex::new(Totals::default());
                let mut reader = PbfReader::open(&path).unwrap();
                reader.set_thread_count(threads);
                reader
                    .read_blocks(|block| {
                        block.decode_entities_with_strings(
                            EntityOptions::all(false),
                            |group, strings| {
                                consume_views(group, strings, &mut views.lock().unwrap())
                            },
                        )
                    })
                    .unwrap();
                let mut expected = Totals::default();
                for counter in entities {
                    let t = counter.0.into_inner().unwrap();
                    expected.nodes += t.nodes;
                    expected.ways += t.ways;
                    expected.relations += t.relations;
                    expected.checksum = expected.checksum.wrapping_add(t.checksum);
                }
                assert_eq!(views.into_inner().unwrap(), expected);
            }
        }
    }

    #[test]
    fn digest_includes_order_and_entity_association() {
        let mut a = Digest::new(2, 1);
        a.number(10);
        a.number(20);
        let mut b = Digest::new(2, 1);
        b.number(20);
        b.number(10);
        assert_ne!(a.0, b.0);
        let mut c = Digest::new(2, 2);
        c.number(10);
        c.number(20);
        assert_ne!(a.0, c.0);
    }
}
