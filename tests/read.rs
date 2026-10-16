use inputosm::*;
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

#[derive(Default)]
struct Collector(Mutex<(usize, usize, usize, u64)>);
impl EntityHandler for Collector {
    fn nodes(&self, _: Context, nodes: &[Node<'_>]) -> Result<()> {
        let mut totals = self.0.lock().unwrap();
        totals.0 += nodes.len();
        for n in nodes {
            totals.3 = totals.3.wrapping_add(n.id as u64);
        }
        Ok(())
    }
    fn ways(&self, _: Context, ways: &[Way<'_>]) -> Result<()> {
        let mut totals = self.0.lock().unwrap();
        totals.1 += ways.len();
        for w in ways {
            totals.3 = totals.3.wrapping_add(w.id as u64);
        }
        Ok(())
    }
    fn relations(&self, _: Context, relations: &[Relation<'_>]) -> Result<()> {
        let mut totals = self.0.lock().unwrap();
        totals.2 += relations.len();
        for r in relations {
            totals.3 = totals.3.wrapping_add(r.id as u64);
        }
        Ok(())
    }
}
#[test]
fn complete_fixtures_and_parallel_readers() {
    for (file, expected) in [("ordinary.osm.pbf", 7), ("comprehensive.osm.pbf", 17005)] {
        for threads in [1, 4] {
            let handler = Collector::default();
            input_file(
                fixture(file),
                InputOptions {
                    decode_metadata: true,
                    threads,
                },
                &handler,
            )
            .unwrap();
            let t = handler.0.into_inner().unwrap();
            assert_eq!((t.0, t.1, t.2), (expected, 12, 4));
            if expected == 17005 {
                assert_eq!(t.3, 225144593582);
            }
            let mut reader = PbfReader::open(fixture(file)).unwrap();
            reader.set_thread_count(threads);
            let nodes = AtomicUsize::new(0);
            reader
                .read_blocks(|b| {
                    nodes.fetch_add(b.node_count()?, Ordering::Relaxed);
                    b.validate()
                })
                .unwrap();
            assert_eq!(nodes.into_inner(), expected);
        }
    }
}
#[test]
fn index_and_repeated_random_reads() {
    let mut reader = PbfReader::open(fixture("comprehensive.osm.pbf")).unwrap();
    let mut before = Counts::default();
    reader
        .read_block(1, |b| {
            before = b.counts()?;
            Ok(())
        })
        .unwrap();
    assert!(!reader.has_index());
    reader.build_index().unwrap();
    assert!(reader.has_index());
    assert!(reader.index_memory_bytes() >= 48);
    for _ in 0..3 {
        reader
            .read_block(1, |b| {
                assert_eq!(b.counts()?, before);
                Ok(())
            })
            .unwrap();
    }
    assert!(reader.read_block(0, |_| Ok(())).is_err());
    assert!(reader.read_block(100000, |_| Ok(())).is_err());
    reader.read_blocks(|b| b.validate()).unwrap();
}
#[test]
fn cancellation_and_worker_panic() {
    let mut reader = PbfReader::open(fixture("comprehensive.osm.pbf")).unwrap();
    reader.set_thread_count(4);
    assert!(matches!(
        reader.read_blocks(|_| Err(Error::Cancelled)),
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        reader.read_blocks(|_| panic!("callback test")),
        Err(Error::WorkerPanic)
    ));
    reader.read_blocks(|b| b.validate()).unwrap();
}
#[test]
fn xml_and_osc_modes() {
    struct Handler(Mutex<Vec<(Mode, i64)>>);
    impl EntityHandler for Handler {
        fn nodes(&self, c: Context, n: &[Node<'_>]) -> Result<()> {
            self.0.lock().unwrap().push((c.mode, n[0].id));
            assert_eq!(n[0].raw_latitude, 407128000);
            assert_eq!(n[0].raw_longitude, -740060000);
            assert_eq!(n[0].metadata.raw_timestamp, 1609556645);
            assert_eq!(n[0].tags[0].value, b"Create Node");
            Ok(())
        }
        fn ways(&self, c: Context, w: &[Way<'_>]) -> Result<()> {
            self.0.lock().unwrap().push((c.mode, w[0].id));
            assert_eq!(w[0].node_refs, &[100, 101]);
            Ok(())
        }
        fn relations(&self, c: Context, r: &[Relation<'_>]) -> Result<()> {
            self.0.lock().unwrap().push((c.mode, r[0].id));
            assert_eq!(r[0].members[1].member_type, MemberType::Way);
            Ok(())
        }
    }
    let handler = Handler(Mutex::new(Vec::new()));
    input_file(
        fixture("sample.osc"),
        InputOptions {
            decode_metadata: true,
            threads: 4,
        },
        &handler,
    )
    .unwrap();
    assert_eq!(
        handler.0.into_inner().unwrap(),
        vec![
            (Mode::Create, 100),
            (Mode::Modify, 200),
            (Mode::Delete, 300)
        ]
    );
    let h = Collector::default();
    input_file(fixture("sample.osm"), InputOptions::default(), &h).unwrap();
    assert_eq!(*h.0.lock().unwrap(), (2, 1, 1, 33));
}

mod data {
    use super::*;
    pub fn var(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        while v >= 128 {
            out.push(v as u8 | 128);
            v >>= 7;
        }
        out.push(v as u8);
        out
    }
    pub fn int(n: u64, v: u64) -> Vec<u8> {
        [var(n << 3), var(v)].concat()
    }
    pub fn msg(n: u64, bytes: &[u8]) -> Vec<u8> {
        [var(n << 3 | 2), var(bytes.len() as u64), bytes.to_vec()].concat()
    }
    pub fn zz(v: i64) -> u64 {
        ((v as u64) << 1) ^ ((v >> 63) as u64)
    }
    pub fn packed(n: u64, values: &[u64]) -> Vec<u8> {
        msg(n, &values.iter().flat_map(|v| var(*v)).collect::<Vec<_>>())
    }
    pub fn block(kind: &[u8], raw: &[u8]) -> Vec<u8> {
        blob(kind, &msg(1, raw))
    }
    pub fn blob(kind: &[u8], blob: &[u8]) -> Vec<u8> {
        let header = [msg(1, kind), int(3, blob.len() as u64)].concat();
        [
            (header.len() as u32).to_be_bytes().to_vec(),
            header,
            blob.to_vec(),
        ]
        .concat()
    }
    pub fn header() -> Vec<u8> {
        block(
            b"OSMHeader",
            &[msg(4, b"OsmSchema-V0.6"), msg(5, b"LocationsOnWays")].concat(),
        )
    }
    pub fn table() -> Vec<u8> {
        msg(1, &[msg(1, b""), msg(1, b"key"), msg(1, b"value")].concat())
    }
    pub struct Temp(pub PathBuf);
    impl Temp {
        pub fn bytes(bytes: &[u8], extension: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "inputosm-test-{}-{}.{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
                extension
            ));
            std::fs::write(&path, bytes).unwrap();
            Self(path)
        }
        pub fn pbf(raw: &[u8]) -> Self {
            Self::bytes(&[header(), block(b"OSMData", raw)].concat(), "pbf")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}
use data::*;
fn selected_payload() -> Vec<u8> {
    let info = [
        int(1, 7),
        int(2, 5_000_000_000),
        int(3, 6_000_000_000),
        int(4, 42),
        int(5, 2),
        int(6, 0),
    ]
    .concat();
    let tags = [packed(2, &[1]), packed(3, &[2])].concat();
    let node = [
        int(1, zz(10)),
        int(8, zz(-100)),
        int(9, zz(200)),
        tags.clone(),
        msg(4, &info),
    ]
    .concat();
    let dense_info = [
        packed(1, &[7, 8]),
        packed(2, &[zz(5_000_000_000), zz(-1)]),
        packed(3, &[zz(6_000_000_000), zz(2)]),
        packed(4, &[zz(42), zz(-1)]),
        packed(5, &[zz(2), zz(-1)]),
        packed(6, &[0, 1]),
    ]
    .concat();
    let dense = [
        packed(1, &[zz(20)]),
        int(1, zz(-1)),
        packed(8, &[zz(-3), zz(4)]),
        packed(9, &[zz(5), zz(-2)]),
        packed(10, &[1, 2, 0, 0]),
        msg(5, &dense_info),
    ]
    .concat();
    let way = [
        int(1, 30),
        tags.clone(),
        msg(4, &info),
        packed(8, &[zz(10), zz(1), zz(-2)]),
        packed(9, &[zz(1), zz(2), zz(-4)]),
        packed(10, &[zz(2), zz(-1), zz(3)]),
    ]
    .concat();
    let relation = [
        int(1, 40),
        tags,
        msg(4, &info),
        packed(8, &[1, 2, 0]),
        packed(9, &[zz(10), zz(-3), zz(5)]),
        packed(10, &[0, 1, 2]),
    ]
    .concat();
    [
        table(),
        msg(2, &msg(1, &node)),
        msg(2, &msg(2, &dense)),
        msg(2, &[msg(3, &way), msg(3, &int(1, 31))].concat()),
        msg(2, &msg(4, &relation)),
        int(17, 10),
        int(18, 2000),
        int(19, 3),
        int(20, 4),
    ]
    .concat()
}
#[test]
fn every_field_mask_and_wide_metadata() {
    let file = Temp::pbf(&selected_payload());
    let mut reader = PbfReader::open(&file.0).unwrap();
    reader
        .read_block(1, |b| {
            assert_eq!(
                b.counts()?,
                Counts {
                    nodes: 3,
                    ways: 2,
                    relations: 1
                }
            );
            assert_eq!(b.string_table_size()?, 3);
            for mask in 0..32 {
                let mut ids = Vec::new();
                let mut count = 0;
                b.decode_nodes(
                    NodeOptions {
                        id: mask & 1 != 0,
                        latitude: mask & 2 != 0,
                        longitude: mask & 4 != 0,
                        tags: mask & 8 != 0,
                        metadata: mask & 16 != 0,
                    },
                    |batch| {
                        count += batch.context.count;
                        ids.extend_from_slice(batch.ids);
                        if mask & 2 != 0 {
                            assert_eq!(
                                batch.raw_latitudes,
                                if batch.context.group_index == 0 {
                                    &[-100][..]
                                } else {
                                    &[-3, 1][..]
                                }
                            );
                        }
                        if mask & 4 != 0 {
                            assert_eq!(
                                batch.raw_longitudes,
                                if batch.context.group_index == 0 {
                                    &[200][..]
                                } else {
                                    &[5, 3][..]
                                }
                            );
                        }
                        if mask & 8 != 0 {
                            assert_eq!(batch.tags[0], [TagIds { key: 1, value: 2 }]);
                        }
                        if mask & 16 != 0 {
                            assert_eq!(batch.metadata[0].raw_timestamp, 5_000_000_000);
                            assert_eq!(batch.metadata[0].changeset, 6_000_000_000);
                            assert_eq!(batch.metadata[0].present, 63);
                        }
                        Ok(())
                    },
                )?;
                assert_eq!(count, 3);
                assert_eq!(
                    ids,
                    if mask & 1 != 0 {
                        vec![10, 20, 19]
                    } else {
                        vec![]
                    }
                );
            }
            for mask in 0..16 {
                b.decode_ways(
                    WayOptions {
                        tags: mask & 1 != 0,
                        node_refs: mask & 2 != 0,
                        metadata: mask & 4 != 0,
                        node_locations: mask & 8 != 0,
                    },
                    |batch| {
                        assert_eq!(batch.ids, &[30, 31]);
                        if mask & 2 != 0 {
                            assert_eq!(batch.node_refs[0], [10, 11, 9]);
                            assert!(batch.node_refs[1].is_empty());
                        }
                        if mask & 8 != 0 {
                            assert_eq!(
                                batch.node_locations[0][2],
                                Location {
                                    raw_latitude: -1,
                                    raw_longitude: 4
                                }
                            );
                            assert_eq!(batch.locations_present, &[true, false]);
                        }
                        Ok(())
                    },
                )?;
            }
            for mask in 0..32 {
                b.decode_relations(
                    RelationOptions {
                        tags: mask & 1 != 0,
                        member_ids: mask & 2 != 0,
                        member_types: mask & 4 != 0,
                        member_roles: mask & 8 != 0,
                        metadata: mask & 16 != 0,
                    },
                    |batch| {
                        assert_eq!(batch.ids, &[40]);
                        if mask & 2 != 0 {
                            assert_eq!(batch.member_ids, &[10, 7, 12]);
                        }
                        if mask & 4 != 0 {
                            assert_eq!(
                                batch.member_types,
                                &[MemberType::Node, MemberType::Way, MemberType::Relation]
                            );
                        }
                        if mask & 8 != 0 {
                            assert_eq!(batch.member_roles, &[1, 2, 0]);
                        }
                        Ok(())
                    },
                )?;
            }
            b.validate()
        })
        .unwrap();
}
#[test]
fn lazy_payload_and_unknown_blocks() {
    let file = Temp::bytes(
        &[
            header(),
            block(b"Ignored", b"bad"),
            block(b"OSMData", b"bad"),
        ]
        .concat(),
        "pbf",
    );
    let mut reader = PbfReader::open(&file.0).unwrap();
    reader
        .read_blocks(|b| {
            assert_eq!(b.index(), 2);
            Ok(())
        })
        .unwrap();
    assert!(reader.read_blocks(|b| b.counts().map(|_| ())).is_err());
    reader.build_index().unwrap();
    assert!(reader.read_block(1, |_| Ok(())).is_err());
}
#[test]
fn malformed_selected_fields() {
    let cases = [
        msg(2, &msg(1, &int(1, zz(1)))),
        msg(
            2,
            &msg(
                2,
                &[
                    packed(1, &[zz(1), zz(2)]),
                    packed(8, &[0]),
                    packed(9, &[0, 0]),
                ]
                .concat(),
            ),
        ),
        msg(2, &msg(3, &[int(1, 1), packed(2, &[1])].concat())),
        msg(
            2,
            &msg(
                4,
                &[
                    int(1, 1),
                    packed(8, &[1]),
                    packed(9, &[0]),
                    packed(10, &[3]),
                ]
                .concat(),
            ),
        ),
        msg(
            2,
            &[
                msg(1, &[int(1, 0), int(8, 0), int(9, 0)].concat()),
                msg(3, &int(1, 1)),
            ]
            .concat(),
        ),
        msg(
            2,
            &msg(
                2,
                &[
                    packed(1, &[zz(i64::MAX), zz(1)]),
                    packed(8, &[0, 0]),
                    packed(9, &[0, 0]),
                ]
                .concat(),
            ),
        ),
    ];
    for case in cases {
        let file = Temp::pbf(&[table(), case].concat());
        let mut reader = PbfReader::open(&file.0).unwrap();
        assert!(reader.read_blocks(|b| b.validate()).is_err());
    }
}
#[test]
fn selective_validation_and_string_bounds() {
    let way = [
        int(1, 1),
        packed(2, &[999]),
        packed(3, &[2]),
        msg(8, &[128]),
    ]
    .concat();
    let file = Temp::pbf(&[table(), msg(2, &msg(3, &way))].concat());
    let mut reader = PbfReader::open(&file.0).unwrap();
    reader
        .read_blocks(|b| {
            b.decode_ways(WayOptions::default(), |w| {
                assert_eq!(w.ids, &[1]);
                Ok(())
            })
        })
        .unwrap();
    assert!(reader.read_blocks(|b| b.validate()).is_err());
}
#[test]
fn bad_framing_and_headers() {
    let files = [
        vec![],
        vec![0, 0, 0, 0],
        vec![0, 1, 0, 0],
        block(b"OSMData", &table()),
        block(b"OSMHeader", &msg(4, b"UnknownRequiredFeature")),
        block(b"OSMHeader", &msg(1, &int(1, 1))),
    ];
    for bytes in files {
        let file = Temp::bytes(&bytes, "pbf");
        assert!(PbfReader::open(&file.0).is_err());
    }
    let file = Temp::bytes(&[header(), header()].concat(), "pbf");
    let mut r = PbfReader::open(&file.0).unwrap();
    assert!(r.build_index().is_err());
    assert!(!r.has_index());
    let file = Temp::bytes(&[header(), vec![0, 0]].concat(), "pbf");
    let mut r = PbfReader::open(&file.0).unwrap();
    assert!(r.read_blocks(|_| Ok(())).is_err());
}
#[test]
fn malformed_xml_and_exact_decimal_coordinates() {
    for text in [
        "",
        "<osm>",
        "<osm/><osm/>",
        "<osm><node id='1' lat='a' lon='0'/></osm>",
        "<osm><node id='1' lat='0' lon='0'><tag k='a'/></node></osm>",
        "<!DOCTYPE osm><osm/>",
        "<osm><way id='1'><member type='way' ref='1'/></way></osm>",
    ] {
        let file = Temp::bytes(text.as_bytes(), "osm");
        assert!(
            input_file(&file.0, InputOptions::default(), &Collector::default()).is_err(),
            "{text}"
        );
    }
    struct Exact;
    impl EntityHandler for Exact {
        fn nodes(&self, _: Context, n: &[Node<'_>]) -> Result<()> {
            assert_eq!(n[0].raw_latitude, -1);
            assert_eq!(n[0].raw_longitude, 123456789);
            assert_eq!(n[0].tags[0].value, b"a&b");
            Ok(())
        }
    }
    let file=Temp::bytes(b"<osm><node id='1' lat='-0.0000001' lon='12.3456789'><tag v='a&amp;b' k='name'/></node></osm>","osm");
    input_file(&file.0, InputOptions::default(), &Exact).unwrap();
}

fn stored_zlib(raw: &[u8]) -> Vec<u8> {
    assert!(raw.len() < 65536);
    let length = raw.len() as u16;
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    [
        vec![0x78, 0x01, 1],
        length.to_le_bytes().to_vec(),
        (!length).to_le_bytes().to_vec(),
        raw.to_vec(),
        ((b << 16) | a).to_be_bytes().to_vec(),
    ]
    .concat()
}
#[test]
fn compressed_blob_validation() {
    let raw = selected_payload();
    let compressed = stored_zlib(&raw);
    let valid = [int(2, raw.len() as u64), msg(3, &compressed)].concat();
    let file = Temp::bytes(&[header(), blob(b"OSMData", &valid)].concat(), "pbf");
    PbfReader::open(&file.0)
        .unwrap()
        .read_blocks(|b| b.validate())
        .unwrap();
    let mut corrupt = compressed.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    let cases = [
        [
            int(2, raw.len() as u64),
            msg(3, &[compressed.clone(), vec![0]].concat()),
        ]
        .concat(),
        [int(2, raw.len() as u64 - 1), msg(3, &compressed)].concat(),
        [int(2, raw.len() as u64 + 1), msg(3, &compressed)].concat(),
        [int(2, raw.len() as u64), msg(3, &corrupt)].concat(),
        [int(2, 32 * 1024 * 1024), msg(3, &compressed)].concat(),
        [msg(1, &raw), msg(3, &compressed)].concat(),
        msg(3, &compressed),
        msg(4, &compressed),
    ];
    for case in cases {
        let file = Temp::bytes(&[header(), blob(b"OSMData", &case)].concat(), "pbf");
        assert!(
            PbfReader::open(&file.0)
                .unwrap()
                .read_blocks(|b| b.validate())
                .is_err()
        );
    }
}
#[test]
fn dense_fixture_field_values() {
    struct Values;
    impl EntityHandler for Values {
        fn nodes(&self, _: Context, nodes: &[Node<'_>]) -> Result<()> {
            for n in nodes {
                let i = if n.id < 5_000_000_000 {
                    n.id - 1
                } else {
                    n.id - 5_000_000_000
                };
                assert_eq!(n.raw_latitude, -450000000 + i % 9000 * 1000);
                assert_eq!(n.raw_longitude, 1200000000 - i % 10000 * 1000);
                assert_eq!(n.metadata.version, (i % 7 + 1) as i32);
                assert_eq!(n.metadata.changeset, 10000 + i % 17);
                assert_eq!(n.metadata.raw_timestamp, 1577923200 + i);
                if i % 3 == 0 {
                    assert!(
                        n.tags.iter().any(|t| t.key == b"name"
                            && t.value == format!("node {i} – café").as_bytes())
                    );
                }
            }
            Ok(())
        }
        fn ways(&self, _: Context, ways: &[Way<'_>]) -> Result<()> {
            for w in ways {
                if w.id == 10000000000 {
                    assert_eq!(w.node_refs.len(), 17005);
                } else {
                    assert_eq!(w.node_refs, &[5000017004, 1, 2, 1]);
                }
            }
            Ok(())
        }
        fn relations(&self, _: Context, rs: &[Relation<'_>]) -> Result<()> {
            for r in rs {
                assert_eq!(r.members.len(), 3);
                assert_eq!(r.members[2].role, "café".as_bytes());
            }
            Ok(())
        }
    }
    input_file(
        fixture("comprehensive.osm.pbf"),
        InputOptions {
            decode_metadata: true,
            threads: 4,
        },
        &Values,
    )
    .unwrap();
}
#[test]
fn normalized_coordinates_metadata_and_logging() {
    struct Values;
    impl EntityHandler for Values {
        fn nodes(&self, _: Context, n: &[Node<'_>]) -> Result<()> {
            if n[0].id == 10 {
                assert_eq!(n[0].raw_latitude, -9);
                assert_eq!(n[0].raw_longitude, 20);
                assert_eq!(n[0].metadata.raw_timestamp, 10_000_000_000);
            }
            Ok(())
        }
    }
    let file = Temp::pbf(&selected_payload());
    input_file(
        &file.0,
        InputOptions {
            decode_metadata: true,
            threads: 1,
        },
        &Values,
    )
    .unwrap();
    let messages = std::sync::Arc::new(Mutex::new(Vec::new()));
    let out = messages.clone();
    let mut reader = PbfReader::open(&file.0).unwrap();
    reader.set_log_callback(std::sync::Arc::new(move |level, message| {
        out.lock().unwrap().push((level, message.to_string()))
    }));
    reader.read_blocks(|b| b.validate()).unwrap();
    assert_eq!(messages.lock().unwrap().len(), 1);
    reader.set_log_level(inputosmlog::LogLevel::Disabled);
    reader.read_blocks(|b| b.validate()).unwrap();
    assert_eq!(messages.lock().unwrap().len(), 1);
}
#[test]
fn time_output_and_thread_bounds() {
    for text in [
        "1970-01-01T00:00:00Z",
        "1969-12-31T23:59:59Z",
        "2000-02-29T01:02:03Z",
        "2026-10-16T11:22:33Z",
    ] {
        assert_eq!(
            timeutil::timestamp_to_string(timeutil::parse_timestamp(text).unwrap()),
            text.replace('T', " ").replace('Z', "")
        );
    }
    assert_eq!(timeutil::duration_to_string(1001), "1.001 μs");
    let mut reader = PbfReader::open(fixture("ordinary.osm.pbf")).unwrap();
    reader.set_thread_count(0);
    assert_eq!(reader.thread_count(), 1);
    reader.set_thread_count(usize::MAX);
    assert_eq!(reader.thread_count(), 256);
}

#[test]
fn xml_review_regressions() {
    struct Normalized;
    impl EntityHandler for Normalized {
        fn nodes(&self, _: Context, n: &[Node<'_>]) -> Result<()> {
            assert_eq!(n[0].tags[0].value, b"a b c d e\nf\tg\rh");
            Ok(())
        }
    }
    let file=Temp::bytes(b"<osm><node id='1' lat='0' lon='0'><tag k='name' v='a\tb\rc\r\nd\ne&#10;f&#9;g&#13;h'/></node></osm>","osm");
    input_file(&file.0, InputOptions::default(), &Normalized).unwrap();
    for metadata in [false, true] {
        let file = Temp::bytes(b"<osm><node id='1' visible='false'/></osm>", "osm");
        let c = Collector::default();
        input_file(
            &file.0,
            InputOptions {
                decode_metadata: metadata,
                threads: 1,
            },
            &c,
        )
        .unwrap();
        assert_eq!(c.0.into_inner().unwrap().0, 1);
    }
    for text in [
        "<osm><node id='1' lat='0' lon='0'><tag k='name' v='x<y'/></node></osm>",
        "<osm><node id='1' lat='0' lon='0'><tag k='name' v='x&#1;y'/></node></osm>",
        "<osm><!-- a--b --></osm>",
        "<osm><way id='1' id='2'/></osm>",
    ] {
        let file = Temp::bytes(text.as_bytes(), "osm");
        assert!(
            input_file(&file.0, InputOptions::default(), &Collector::default()).is_err(),
            "{text}"
        );
    }
}
#[test]
fn group_kinds_survive_field_selection() {
    let file = Temp::pbf(&selected_payload());
    let mut r = PbfReader::open(&file.0).unwrap();
    for options in [EntityOptions::default(), EntityOptions::all(true)] {
        r.read_block(1, |b| {
            let mut kinds = Vec::new();
            b.decode_entities(options, |g| {
                kinds.push(g.kind());
                Ok(())
            })?;
            assert_eq!(
                kinds,
                vec![
                    GroupKind::Nodes,
                    GroupKind::DenseNodes,
                    GroupKind::Ways,
                    GroupKind::Relations
                ]
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn fragmented_dense_metadata_and_truncated_payload() {
    let meta = [
        int(1, 7),
        int(1, 8),
        packed(2, &[zz(100)]),
        int(2, zz(-1)),
        int(3, zz(6)),
        int(3, zz(2)),
    ]
    .concat();
    let dense = [
        packed(1, &[zz(1), zz(1)]),
        packed(8, &[0, 0]),
        packed(9, &[0, 0]),
        msg(5, &meta),
    ]
    .concat();
    let file = Temp::pbf(&[table(), msg(2, &msg(2, &dense))].concat());
    let mut r = PbfReader::open(&file.0).unwrap();
    r.read_block(1, |b| {
        b.decode_nodes(
            NodeOptions {
                metadata: true,
                ..Default::default()
            },
            |n| {
                assert_eq!(n.metadata[0].version, 7);
                assert_eq!(n.metadata[1].version, 8);
                assert_eq!(n.metadata[1].raw_timestamp, 99);
                assert_eq!(n.metadata[1].changeset, 8);
                Ok(())
            },
        )
    })
    .unwrap();
    let file = Temp::pbf(&selected_payload());
    let mut r = PbfReader::open(&file.0).unwrap();
    r.build_index().unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file.0)
        .unwrap()
        .set_len(header().len() as u64)
        .unwrap();
    assert!(r.read_block(1, |b| b.validate()).is_err());
}
#[test]
fn utf16_xml() {
    let text = "<?xml version='1.0' encoding='UTF-16'?><osm><node id='1' lat='0' lon='0'><tag k='name' v='café'/></node></osm>";
    let bytes: Vec<_> = [0xff, 0xfe]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let file = Temp::bytes(&bytes, "osm");
    let c = Collector::default();
    input_file(&file.0, InputOptions::default(), &c).unwrap();
    assert_eq!(c.0.into_inner().unwrap().0, 1);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EntityRecord {
    kind: u8,
    id: i64,
    coordinates: Option<(i64, i64)>,
    tags: Vec<(Vec<u8>, Vec<u8>)>,
    refs: Vec<i64>,
    members: Vec<(u8, i64, Vec<u8>)>,
    metadata: String,
}
fn owned_tags<'a>(tags: impl Iterator<Item = Tag<'a>>) -> Vec<(Vec<u8>, Vec<u8>)> {
    tags.map(|t| (t.key.to_vec(), t.value.to_vec())).collect()
}
#[derive(Default)]
struct RecordCollector(Mutex<Vec<EntityRecord>>);
impl EntityHandler for RecordCollector {
    fn nodes(&self, _: Context, nodes: &[Node<'_>]) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .extend(nodes.iter().map(|n| EntityRecord {
                kind: 1,
                id: n.id,
                coordinates: Some((n.raw_latitude, n.raw_longitude)),
                tags: owned_tags(n.tags.iter().copied()),
                refs: vec![],
                members: vec![],
                metadata: format!("{:?}", n.metadata),
            }));
        Ok(())
    }
    fn ways(&self, _: Context, ways: &[Way<'_>]) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .extend(ways.iter().map(|w| EntityRecord {
                kind: 2,
                id: w.id,
                coordinates: None,
                tags: owned_tags(w.tags.iter().copied()),
                refs: w.node_refs.to_vec(),
                members: vec![],
                metadata: format!("{:?}", w.metadata),
            }));
        Ok(())
    }
    fn relations(&self, _: Context, relations: &[Relation<'_>]) -> Result<()> {
        self.0.lock().unwrap().extend(relations.iter().map(|r| {
            EntityRecord {
                kind: 3,
                id: r.id,
                coordinates: None,
                tags: owned_tags(r.tags.iter().copied()),
                refs: vec![],
                members: r
                    .members
                    .iter()
                    .map(|m| (m.member_type as u8, m.id, m.role.to_vec()))
                    .collect(),
                metadata: format!("{:?}", r.metadata),
            }
        }));
        Ok(())
    }
}
fn view_records(
    group: EntityViews<'_>,
    records: &mut Vec<EntityRecord>,
    decode_metadata: bool,
) -> Result<()> {
    let checked_metadata = |m: Option<Metadata>| {
        assert_eq!(m.is_some(), decode_metadata);
        format!("{:?}", m.unwrap_or_default())
    };
    match group {
        EntityViews::Nodes(nodes) => {
            assert!(nodes.get(nodes.len()).is_none());
            assert!(nodes.get(usize::MAX).is_none());
            for n in nodes.iter() {
                n.user()?;
                records.push(EntityRecord {
                    kind: 1,
                    id: n.id(),
                    coordinates: Some(n.coordinates()?),
                    tags: owned_tags(n.tags().collect::<Result<Vec<_>>>()?.into_iter()),
                    refs: vec![],
                    members: vec![],
                    metadata: checked_metadata(n.metadata()?),
                });
            }
        }
        EntityViews::Ways(ways) => {
            assert!(ways.get(ways.len()).is_none());
            for w in ways.iter() {
                w.user()?;
                records.push(EntityRecord {
                    kind: 2,
                    id: w.id(),
                    coordinates: None,
                    tags: owned_tags(w.tags().collect::<Result<Vec<_>>>()?.into_iter()),
                    refs: w.node_refs().to_vec(),
                    members: vec![],
                    metadata: checked_metadata(w.metadata()?),
                });
            }
        }
        EntityViews::Relations(relations) => {
            assert!(relations.get(relations.len()).is_none());
            for r in relations.iter() {
                r.user()?;
                records.push(EntityRecord {
                    kind: 3,
                    id: r.id(),
                    coordinates: None,
                    tags: owned_tags(r.tags().collect::<Result<Vec<_>>>()?.into_iter()),
                    refs: vec![],
                    members: r
                        .members()
                        .map(|m| m.map(|m| (m.member_type as u8, m.id, m.role.to_vec())))
                        .collect::<Result<_>>()?,
                    metadata: checked_metadata(r.metadata()?),
                });
            }
        }
        EntityViews::Empty(_) => {}
    }
    Ok(())
}
fn io_readers(path: &Path) -> Vec<PbfReader> {
    let mut sequential = PbfReader::open(path).unwrap();
    sequential.set_read_mode(PbfReadMode::Sequential);
    let mut readers = vec![PbfReader::open(path).unwrap(), sequential];
    #[cfg(feature = "mmap")]
    // SAFETY: Tests leave these files unchanged until all readers drop.
    readers.push(unsafe { PbfReader::open_mmap(path).unwrap() });
    let _ = &mut readers;
    readers
}
#[test]
fn public_views_match_complete_entities_in_all_read_modes() {
    let custom = Temp::pbf(&selected_payload());
    for path in [
        fixture("ordinary.osm.pbf"),
        fixture("comprehensive.osm.pbf"),
        custom.0.clone(),
    ] {
        for decode_metadata in [false, true] {
            let collector = RecordCollector::default();
            input_file(
                &path,
                InputOptions {
                    decode_metadata,
                    threads: 1,
                },
                &collector,
            )
            .unwrap();
            let mut expected = collector.0.into_inner().unwrap();
            expected.sort();
            for mut reader in io_readers(&path) {
                for threads in [1, 4] {
                    reader.set_thread_count(threads);
                    let records = Mutex::new(Vec::new());
                    reader
                        .read_entity_views(decode_metadata, |context, group| {
                            assert!(context.thread_index < threads);
                            assert_eq!(context.block_index, group.context().block_index);
                            view_records(group, &mut records.lock().unwrap(), decode_metadata)
                        })
                        .unwrap();
                    let mut actual = records.into_inner().unwrap();
                    actual.sort();
                    assert_eq!(actual, expected);
                }
                reader.build_index().unwrap();
                for _ in 0..2 {
                    reader
                        .read_block(1, |block| {
                            block.decode_entity_views(decode_metadata, |g| {
                                view_records(g, &mut Vec::new(), decode_metadata)
                            })
                        })
                        .unwrap();
                }
                reader.read_blocks(|block| block.validate()).unwrap();
            }
        }
    }
}
#[test]
fn new_read_modes_preserve_errors_and_cancellation() {
    let invalid = Temp::bytes(&[header(), blob(b"OSMData", &[0xff])].concat(), "pbf");
    for mut reader in io_readers(&invalid.0) {
        reader.read_blocks(|_| Ok(())).unwrap();
        assert!(reader.read_blocks(|b| b.validate()).is_err());
    }
    let truncated = Temp::bytes(&[header(), vec![0, 0, 0, 5, 10]].concat(), "pbf");
    for mut reader in io_readers(&truncated.0) {
        assert!(reader.read_blocks(|_| Ok(())).is_err());
        assert!(reader.build_index().is_err());
    }
    let unknown = Temp::bytes(
        &[
            header(),
            block(b"Extension", &[]),
            block(b"OSMData", &selected_payload()),
        ]
        .concat(),
        "pbf",
    );
    for mut reader in io_readers(&unknown.0) {
        reader.set_thread_count(4);
        assert!(matches!(
            reader.read_entity_views(false, |_, _| Err(Error::Cancelled)),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            reader.read_blocks(|_| panic!("test panic")),
            Err(Error::WorkerPanic)
        ));
        reader
            .read_blocks(|b| {
                assert_eq!(b.index(), 2);
                b.validate()
            })
            .unwrap();
        reader.build_index().unwrap();
        assert!(reader.read_block(1, |_| Ok(())).is_err());
        reader.read_block(2, |b| b.validate()).unwrap();
    }
}
#[test]
fn view_accessors_check_strings_conversions_and_metadata() {
    let node = [
        int(1, zz(1)),
        int(8, zz(i64::MAX)),
        int(9, zz(0)),
        packed(2, &[1]),
        packed(3, &[9]),
        msg(4, &[int(2, i64::MAX as u64), int(5, 9)].concat()),
    ]
    .concat();
    let relation = [
        int(1, 2),
        packed(8, &[9]),
        packed(9, &[zz(1)]),
        packed(10, &[0]),
    ]
    .concat();
    let file = Temp::pbf(
        &[
            table(),
            msg(2, &msg(1, &node)),
            msg(2, &msg(4, &relation)),
            int(18, 2000),
        ]
        .concat(),
    );
    for mut reader in io_readers(&file.0) {
        reader
            .read_entity_views(true, |_, group| {
                match group {
                    EntityViews::Nodes(nodes) => {
                        let n = nodes.get(0).unwrap();
                        assert_eq!(n.id(), 1);
                        assert!(n.coordinates().is_err());
                        assert!(n.tags().next().unwrap().is_err());
                        assert!(n.metadata().is_err());
                        assert!(n.user().is_err());
                    }
                    EntityViews::Relations(relations) => {
                        assert!(relations.get(0).unwrap().members().next().unwrap().is_err());
                    }
                    _ => panic!("unexpected group"),
                }
                Ok(())
            })
            .unwrap();
        reader
            .read_entity_views(false, |_, group| {
                if let EntityViews::Nodes(nodes) = group {
                    let n = nodes.get(0).unwrap();
                    assert_eq!(n.metadata()?, None);
                    assert_eq!(n.user()?, None);
                }
                Ok(())
            })
            .unwrap();
    }
    let file = Temp::pbf(&selected_payload());
    PbfReader::open(&file.0)
        .unwrap()
        .read_entity_views(true, |_, group| {
            if let EntityViews::Nodes(nodes) = group {
                assert_eq!(nodes.get(0).unwrap().user()?, Some(&b"value"[..]));
            }
            Ok(())
        })
        .unwrap();
}
