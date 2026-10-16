use inputosm::*;
use std::{sync::Mutex, time::Instant};

#[derive(Default, Clone, Copy)]
struct Totals {
    blocks: u64,
    max_index: usize,
    nodes: u64,
    ways: u64,
    relations: u64,
    checksum: u64,
    values_checksum: u64,
    strings: u64,
}
impl Totals {
    fn merge(&mut self, b: Self) {
        self.blocks += b.blocks;
        self.max_index = self.max_index.max(b.max_index);
        self.nodes += b.nodes;
        self.ways += b.ways;
        self.relations += b.relations;
        self.checksum = self.checksum.wrapping_add(b.checksum);
        self.values_checksum = self.values_checksum.wrapping_add(b.values_checksum);
        self.strings += b.strings;
    }
    fn values(&mut self, values: impl IntoIterator<Item = u64>) {
        for v in values {
            self.values_checksum = self.values_checksum.wrapping_add(v);
        }
    }
    fn common(
        &mut self,
        ids: &[i64],
        tags: &[TagIds],
        metadata: &[Metadata],
        selection: &Selection,
    ) {
        if selection.consume != "count" {
            for &id in ids {
                self.checksum = self.checksum.wrapping_add(id as u64);
            }
        }
        if selection.consume != "all" {
            return;
        }
        self.values(
            tags.iter()
                .map(|t| u64::from(t.key) * 1000003 + u64::from(t.value)),
        );
        for m in metadata {
            self.values([
                m.version as u64,
                m.raw_timestamp as u64,
                m.changeset as u64,
                m.uid as u64,
                u64::from(m.user_sid),
                u64::from(m.present),
                u64::from(m.visible),
            ]);
        }
    }
    fn consume(&mut self, b: &mut PbfBlock<'_>, s: &Selection) -> Result<()> {
        self.blocks += 1;
        self.max_index = self.max_index.max(b.index());
        let mut counted = Counts::default();
        if s.counts & 14 == 14 {
            counted = b.counts()?;
        } else {
            if s.counts & 2 != 0 {
                counted.nodes = b.node_count()?;
            }
            if s.counts & 4 != 0 {
                counted.ways = b.way_count()?;
            }
            if s.counts & 8 != 0 {
                counted.relations = b.relation_count()?;
            }
        }
        if s.strings {
            b.decode_strings(|text| {
                self.strings += 1;
                if s.consume == "all" {
                    self.values(text.iter().map(|&v| u64::from(v)));
                }
                Ok(())
            })?;
        }
        let before = (self.nodes, self.ways, self.relations);
        if s.options.nodes.is_some() || s.options.ways.is_some() || s.options.relations.is_some() {
            b.decode_entities(s.options, |group| {
                match group {
                    GroupBatch::Nodes(n) => {
                        self.nodes += n.context.count as u64;
                        self.common(n.ids, n.tags.values, n.metadata, s);
                        if s.consume == "all" {
                            self.values(
                                n.raw_latitudes
                                    .iter()
                                    .chain(n.raw_longitudes)
                                    .map(|&v| v as u64),
                            );
                        }
                    }
                    GroupBatch::Ways(w) => {
                        self.ways += w.context.count as u64;
                        self.common(w.ids, w.tags.values, w.metadata, s);
                        if s.consume == "all" {
                            self.values(w.node_refs.values.iter().map(|&v| v as u64));
                            for l in w.node_locations.values {
                                self.values([l.raw_latitude as u64, l.raw_longitude as u64]);
                            }
                        }
                    }
                    GroupBatch::Relations(r) => {
                        self.relations += r.context.count as u64;
                        self.common(r.ids, r.tags.values, r.metadata, s);
                        if s.consume == "all" {
                            self.values(r.member_ids.iter().map(|&v| v as u64));
                            self.values(r.member_types.iter().map(|&v| v as u64));
                            self.values(r.member_roles.iter().map(|&v| u64::from(v)));
                        }
                    }
                    _ => {}
                }
                Ok(())
            })?;
        }
        for (mask, enabled, old, total, count) in [
            (
                2,
                s.options.nodes.is_some(),
                before.0,
                &mut self.nodes,
                counted.nodes,
            ),
            (
                4,
                s.options.ways.is_some(),
                before.1,
                &mut self.ways,
                counted.ways,
            ),
            (
                8,
                s.options.relations.is_some(),
                before.2,
                &mut self.relations,
                counted.relations,
            ),
        ] {
            if s.counts & mask != 0 {
                if enabled && *total - old != count as u64 {
                    return Err(Error::Invalid("Count and decode results differ"));
                }
                *total = old + count as u64;
            }
        }
        Ok(())
    }
}
#[repr(align(64))]
struct Counter(Mutex<Totals>);
struct Selection {
    options: EntityOptions,
    counts: u8,
    strings: bool,
    consume: String,
}
fn number(text: &str) -> Result<usize> {
    if let Some(s) = text.strip_prefix("0x") {
        usize::from_str_radix(s, 16)
    } else {
        text.parse()
    }
    .map_err(|_| Error::Invalid("Invalid numeric argument"))
}
fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut v = *state;
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 5 {
        return Err(Error::Invalid(
            "Usage: pbf_benchmark FILE MODE THREADS REPETITIONS [METADATA REQUESTS MAX_INDEX] [--nodes=N --ways=N --relations=N --counts=N --strings --consume=ids|all|count]",
        ));
    }
    let (path, mode) = (&args[1], args[2].as_str());
    let threads = number(&args[3])?;
    let repetitions = number(&args[4])?;
    if !(1..=256).contains(&threads) || repetitions == 0 {
        return Err(Error::Invalid("Invalid benchmark arguments"));
    }
    let mut masks = [None, None, None];
    let mut positional = vec![];
    let mut explicit = false;
    let mut selection = Selection {
        options: EntityOptions::default(),
        counts: 0,
        strings: false,
        consume: "ids".into(),
    };
    for arg in &args[5..] {
        if !arg.starts_with("--") {
            positional.push(number(arg)?);
            continue;
        }
        if arg == "--strings" {
            selection.strings = true;
            explicit = true;
            continue;
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or(Error::Invalid("Invalid benchmark option"))?;
        if key == "--consume" {
            if !matches!(value, "ids" | "all" | "count") {
                return Err(Error::Invalid("Invalid consumption mode"));
            }
            selection.consume = value.into();
            continue;
        }
        let n = number(value)?;
        let (index, max) = match key {
            "--nodes" => (0, 31),
            "--ways" => (1, 15),
            "--relations" => (2, 31),
            "--counts" => (3, 15),
            _ => return Err(Error::Invalid("Unknown benchmark option")),
        };
        if n > max {
            return Err(Error::Invalid("Invalid field mask"));
        }
        explicit = true;
        if index == 3 {
            selection.counts = n as u8;
        } else {
            masks[index] = Some(n);
        }
    }
    if positional.len() > 3 {
        return Err(Error::Invalid("Too many positional arguments"));
    }
    let metadata = positional.first().copied().unwrap_or(0) != 0;
    let requests = positional.get(1).copied().unwrap_or(256);
    let max_index = positional.get(2).copied().unwrap_or(0);
    if !explicit {
        masks = [
            Some(if metadata { 31 } else { 15 }),
            Some(if metadata { 7 } else { 3 }),
            Some(if metadata { 31 } else { 15 }),
        ];
    }
    selection.options = EntityOptions {
        nodes: masks[0].map(|m| NodeOptions {
            id: m & 1 != 0,
            latitude: m & 2 != 0,
            longitude: m & 4 != 0,
            tags: m & 8 != 0,
            metadata: m & 16 != 0,
        }),
        ways: masks[1].map(|m| WayOptions {
            tags: m & 1 != 0,
            node_refs: m & 2 != 0,
            metadata: m & 4 != 0,
            node_locations: m & 8 != 0,
        }),
        relations: masks[2].map(|m| RelationOptions {
            tags: m & 1 != 0,
            member_ids: m & 2 != 0,
            member_types: m & 4 != 0,
            member_roles: m & 8 != 0,
            metadata: m & 16 != 0,
        }),
    };
    if !matches!(mode, "blocks" | "entities" | "random" | "random-index") {
        return Err(Error::Invalid("Invalid benchmark mode"));
    }
    if mode == "entities" && (explicit || selection.consume == "all") {
        return Err(Error::Invalid(
            "Entity mode requires ID or count consumption without field selection",
        ));
    }
    if mode.starts_with("random") && (requests == 0 || max_index == 0) {
        return Err(Error::Invalid("Invalid random requests"));
    }
    let mut seed = 0x494e5055544f534d;
    let indexes: Vec<_> = if mode.starts_with("random") {
        (0..requests)
            .map(|_| 1 + (random(&mut seed) % (max_index as u64)) as usize)
            .collect()
    } else {
        vec![]
    };
    println!(
        "mode,threads,metadata,iteration,file_bytes,setup_seconds,read_seconds,total_seconds,index_build_seconds,blocks,max_index,nodes,ways,relations,checksum,values_checksum,strings,index_bytes"
    );
    for iteration in 0..repetitions {
        let start = Instant::now();
        let mut index_seconds = 0.;
        let mut index_bytes = 0;
        let counters: Vec<_> = (0..threads)
            .map(|_| Counter(Mutex::new(Totals::default())))
            .collect();
        let ready;
        if mode == "entities" {
            struct Handler<'a> {
                counters: &'a [Counter],
                consume: bool,
            }
            impl EntityHandler for Handler<'_> {
                fn nodes(&self, c: Context, n: &[Node<'_>]) -> Result<()> {
                    let mut t = self.counters[c.thread_index].0.lock().unwrap();
                    t.nodes += n.len() as u64;
                    if self.consume {
                        for n in n {
                            t.checksum = t.checksum.wrapping_add(n.id as u64);
                        }
                    }
                    Ok(())
                }
                fn ways(&self, c: Context, w: &[Way<'_>]) -> Result<()> {
                    let mut t = self.counters[c.thread_index].0.lock().unwrap();
                    t.ways += w.len() as u64;
                    if self.consume {
                        for w in w {
                            t.checksum = t.checksum.wrapping_add(w.id as u64);
                        }
                    }
                    Ok(())
                }
                fn relations(&self, c: Context, r: &[Relation<'_>]) -> Result<()> {
                    let mut t = self.counters[c.thread_index].0.lock().unwrap();
                    t.relations += r.len() as u64;
                    if self.consume {
                        for r in r {
                            t.checksum = t.checksum.wrapping_add(r.id as u64);
                        }
                    }
                    Ok(())
                }
            }
            ready = Instant::now();
            input_file(
                path,
                InputOptions {
                    decode_metadata: metadata,
                    threads,
                },
                &Handler {
                    counters: &counters,
                    consume: selection.consume != "count",
                },
            )?;
        } else if mode == "blocks" {
            let mut reader = PbfReader::open(path)?;
            reader.set_thread_count(threads);
            ready = Instant::now();
            reader.read_blocks(|b| {
                counters[b.thread_index()]
                    .0
                    .lock()
                    .unwrap()
                    .consume(b, &selection)
            })?;
        } else {
            let mut readers = (0..threads)
                .map(|_| PbfReader::open(path))
                .collect::<Result<Vec<_>>>()?;
            let index_start = Instant::now();
            if mode == "random-index" {
                std::thread::scope(|scope| -> Result<()> {
                    let handles: Vec<_> = readers
                        .iter_mut()
                        .map(|r| scope.spawn(|| r.build_index()))
                        .collect();
                    for h in handles {
                        h.join().map_err(|_| Error::WorkerPanic)??;
                    }
                    Ok(())
                })?;
                index_seconds = index_start.elapsed().as_secs_f64();
                index_bytes = readers.iter().map(PbfReader::index_memory_bytes).sum();
            }
            ready = Instant::now();
            std::thread::scope(|scope| -> Result<()> {
                let handles: Vec<_> = readers
                    .iter_mut()
                    .enumerate()
                    .map(|(worker, r)| {
                        let indexes = &indexes;
                        let selection = &selection;
                        let counters = &counters;
                        scope.spawn(move || -> Result<()> {
                            let mut t = Totals::default();
                            for &index in indexes.iter().skip(worker).step_by(threads) {
                                r.read_block(index, |b| t.consume(b, selection))?;
                            }
                            counters[worker].0.lock().unwrap().merge(t);
                            Ok(())
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().map_err(|_| Error::WorkerPanic)??;
                }
                Ok(())
            })?;
        }
        let end = Instant::now();
        let mut total = Totals::default();
        for c in counters {
            total.merge(c.0.into_inner().unwrap());
        }
        println!(
            "{mode},{threads},{},{iteration},{},{:.9},{:.9},{:.9},{index_seconds:.9},{},{},{},{},{},{},{},{},{index_bytes}",
            u8::from(metadata),
            std::fs::metadata(path)?.len(),
            (ready - start).as_secs_f64(),
            (end - ready).as_secs_f64(),
            (end - start).as_secs_f64(),
            total.blocks,
            total.max_index,
            total.nodes,
            total.ways,
            total.relations,
            total.checksum,
            total.values_checksum,
            total.strings
        );
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
