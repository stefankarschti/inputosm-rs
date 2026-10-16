//! Exercise block layout, capacity growth, and numeric boundaries from the C++ tests.
mod common;
use common::*;
use inputosm::*;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

fn node(id: i64) -> Vec<u8> {
    [int(1, zz(id)), int(8, 0), int(9, 0)].concat()
}
fn strings(values: &[&[u8]]) -> Vec<u8> {
    msg(
        1,
        &values.iter().flat_map(|v| msg(1, v)).collect::<Vec<_>>(),
    )
}

#[test]
fn layout_preserves_bytes_order_and_string_borrows() {
    let embedded = b"A\0B";
    let ordinary = [node(-8), int(2, 1), packed(3, &[2]), msg(4, &int(1, 4))].concat();
    let dense = [
        packed(9, &[zz(400), zz(-100)]),
        packed(1, &[zz(20)]),
        int(1, zz(-2)),
        packed(8, &[zz(-200), zz(100)]),
        packed(10, &[1, 3, 0, 0]),
    ]
    .concat();
    let way = [
        int(1, 99),
        packed(8, &[zz(20), zz(-2)]),
        int(2, 1),
        int(3, 2),
    ]
    .concat();
    let relation = [
        int(1, 101),
        packed(8, &[3]),
        packed(9, &[zz(-8)]),
        packed(10, &[0]),
    ]
    .concat();
    let payload = [
        msg(2, &msg(1, &ordinary)),
        msg(2, &msg(2, &dense)),
        msg(2, &msg(3, &way)),
        msg(2, &msg(4, &relation)),
        msg(2, &msg(1, &node(123))),
        strings(&[b"", b"name", b"value", embedded, b"unused", b"value", b""]),
        int(17, 10),
        int(18, 100),
        int(19, (-5i64) as u64),
        int(20, 7),
        int(100, 88),
    ]
    .concat();
    for zip in [false, true] {
        let data = if zip {
            compressed(b"OSMData", &payload)
        } else {
            block(b"OSMData", &payload)
        };
        let prefix = [header(), block(b"Unknown", b"opaque")].concat();
        let temp = Temp::bytes(
            &[prefix.clone(), data, block(b"OSMData", &table())].concat(),
            "pbf",
        );
        let mut reader = PbfReader::open(&temp.0).unwrap();
        reader
            .read_block(2, |b| {
                assert_eq!(b.file_offset(), prefix.len() as u64);
                assert_eq!(
                    b.parameters()?,
                    Parameters {
                        granularity: 10,
                        date_granularity: 100,
                        lat_offset: -5,
                        lon_offset: 7
                    }
                );
                let mut ids = Vec::<i64>::new();
                b.decode_entities_with_strings(EntityOptions::all(true), |g, table| {
                    assert_eq!(table.len(), 7);
                    assert_eq!(table[2], table[5]);
                    assert_ne!(table[2].as_ptr(), table[5].as_ptr());
                    assert_eq!(table[3], embedded);
                    match g {
                        GroupBatch::Nodes(n) => {
                            ids.extend(n.ids);
                            if n.ids == [20, 18] {
                                assert_eq!(n.raw_latitudes, [-200, -100]);
                                assert_eq!(n.raw_longitudes, [400, 300]);
                                assert_eq!(
                                    table[n.tags.get(0).unwrap()[0].value as usize],
                                    embedded
                                );
                                assert!(n.tags.get(1).unwrap().is_empty());
                            }
                        }
                        GroupBatch::Ways(w) => assert_eq!(w.node_refs.values, [20, 18]),
                        GroupBatch::Relations(r) => {
                            assert_eq!(r.member_ids, [-8]);
                            assert_eq!(table[r.member_roles[0] as usize], embedded);
                        }
                        GroupBatch::Empty(_) => (),
                    }
                    Ok(())
                })?;
                assert_eq!(ids, [-8, 20, 18, 123]);
                Ok(())
            })
            .unwrap();
        reader
            .read_block(3, |b| {
                assert_eq!(b.parameters()?, Parameters::default());
                assert_eq!(b.counts()?, Counts::default());
                Ok(())
            })
            .unwrap();
    }
}
#[test]
fn dense_extreme_values_empty_groups_and_varint_boundaries() {
    let dense = [
        packed(9, &[0, zz(i64::MAX), zz(-i64::MAX)]),
        packed(1, &[zz(i64::MAX), zz(-i64::MAX), zz(i64::MIN)]),
        packed(8, &[zz(i64::MIN), zz(i64::MAX), zz(1)]),
    ]
    .concat();
    let empty = [packed(1, &[]), packed(8, &[]), packed(9, &[])].concat();
    let zero = [packed(1, &[0]), packed(8, &[0]), packed(9, &[0])].concat();
    let temp = Temp::pbf(
        &[
            table(),
            msg(2, &msg(2, &empty)),
            msg(2, &msg(2, &dense)),
            msg(2, &msg(2, &zero)),
        ]
        .concat(),
    );
    let mut reader = PbfReader::open(&temp.0).unwrap();
    let mut values = Vec::new();
    reader
        .read_block(1, |b| {
            b.decode_nodes(
                NodeOptions {
                    id: true,
                    latitude: true,
                    longitude: true,
                    tags: true,
                    metadata: true,
                },
                |n| {
                    values.extend(
                        n.ids
                            .iter()
                            .zip(n.raw_latitudes)
                            .zip(n.raw_longitudes)
                            .map(|((&id, &lat), &lon)| (id, lat, lon)),
                    );
                    Ok(())
                },
            )
        })
        .unwrap();
    assert_eq!(
        values,
        [
            (i64::MAX, i64::MIN, 0),
            (0, -1, i64::MAX),
            (i64::MIN, 0, 0),
            (0, 0, 0)
        ]
    );
    for bits in (0..=63).step_by(7) {
        let expected = if bits == 63 { i64::MIN } else { 1i64 << bits };
        let temp = Temp::pbf(&[table(), msg(2, &msg(1, &node(expected)))].concat());
        PbfReader::open(&temp.0)
            .unwrap()
            .read_block(1, |b| {
                b.decode_nodes(NodeOptions::default(), |n| {
                    assert_eq!(n.ids, [expected]);
                    Ok(())
                })
            })
            .unwrap();
    }
}
#[test]
fn buffers_grow_across_many_entities_and_long_strings() {
    let long = vec![b'x'; 8192];
    let tags = [packed(2, &[1]), packed(3, &[2])].concat();
    let mut payload = strings(&[b"", b"key", &long]);
    for group in 0..12 {
        let nodes: Vec<_> = (0..300)
            .flat_map(|i| msg(1, &[node(300 * group + i), tags.clone()].concat()))
            .collect();
        payload.extend(msg(2, &nodes));
        payload.extend(msg(
            2,
            &msg(
                3,
                &[int(1, group as u64), packed(8, &[2; 500]), tags.clone()].concat(),
            ),
        ));
        payload.extend(msg(
            2,
            &msg(
                4,
                &[
                    int(1, group as u64),
                    packed(8, &[2; 500]),
                    packed(9, &[2; 500]),
                    packed(10, &(0..500).map(|i| i % 3).collect::<Vec<_>>()),
                    tags.clone(),
                ]
                .concat(),
            ),
        ));
    }
    let temp = Temp::bytes(
        &[header(), compressed(b"OSMData", &payload)].concat(),
        "pbf",
    );
    let mut reader = PbfReader::open(&temp.0).unwrap();
    let mut totals = [0; 3];
    reader
        .read_block(1, |b| {
            b.decode_entity_views(false, |g| {
                match g {
                    EntityViews::Nodes(nodes) => {
                        for node in nodes.iter() {
                            assert_eq!(node.id(), totals[0] as i64);
                            assert_eq!(node.tags().next().unwrap()?.value, long);
                            totals[0] += 1;
                        }
                    }
                    EntityViews::Ways(ways) => {
                        for way in ways.iter() {
                            assert_eq!(way.node_refs(), (1..=500).collect::<Vec<_>>());
                            assert_eq!(way.tags().next().unwrap()?.value, long);
                            totals[1] += 1;
                        }
                    }
                    EntityViews::Relations(relations) => {
                        for relation in relations.iter() {
                            for (i, member) in relation.members().enumerate() {
                                let m = member?;
                                assert_eq!((m.id, m.member_type as usize), (i as i64 + 1, i % 3));
                                assert_eq!(m.role, long);
                            }
                            assert_eq!(relation.members().len(), 500);
                            totals[2] += 1;
                        }
                    }
                    EntityViews::Empty(_) => (),
                }
                Ok(())
            })
        })
        .unwrap();
    assert_eq!(totals, [3600, 12, 12]);
    let mut xml = String::from("<osm><relation id=\"1\">");
    for i in 0..500 {
        xml.push_str(&format!(
            "<tag k=\"k{i}\" v=\"{}\"/>",
            "x".repeat(if i % 2 == 0 { 3 } else { 200 })
        ));
    }
    xml.push_str("<member type=\"node\" ref=\"1\"/><member type=\"way\" ref=\"2\" role=\"\"/></relation></osm>");
    let xml = Temp::bytes(xml.as_bytes(), "osm");
    struct Handler(Mutex<bool>);
    impl EntityHandler for Handler {
        fn relations(&self, _: Context, r: &[Relation<'_>]) -> Result<()> {
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].tags.len(), 500);
            for (i, tag) in r[0].tags.iter().enumerate() {
                assert_eq!(tag.key, format!("k{i}").as_bytes());
                assert_eq!(tag.value, vec![b'x'; if i % 2 == 0 { 3 } else { 200 }]);
            }
            assert_eq!(r[0].members.len(), 2);
            assert!(r[0].members.iter().all(|m| m.role.is_empty()));
            *self.0.lock().unwrap() = true;
            Ok(())
        }
    }
    let handler = Handler(Mutex::new(false));
    input_file(&xml.0, InputOptions::default(), &handler).unwrap();
    assert!(*handler.0.lock().unwrap());
}
#[test]
fn compressed_buffers_grow_and_shrink_between_blocks() {
    let sizes = [65536, 0, 8, 131072, 1, 1024];
    for header_data in [&b""[..], &msg(4, b"OsmSchema-V0.6")] {
        let mut bytes = compressed(b"OSMHeader", header_data);
        for (i, size) in sizes.into_iter().enumerate() {
            let value = vec![b'a' + i as u8; size];
            let payload = [
                strings(&[b"", b"key", &value]),
                msg(
                    2,
                    &msg(
                        1,
                        &[node(i as i64 + 1), packed(2, &[1]), packed(3, &[2])].concat(),
                    ),
                ),
            ]
            .concat();
            bytes.extend(compressed(b"OSMData", &payload));
        }
        let temp = Temp::bytes(&bytes, "pbf");
        for threads in [1, 4] {
            let mut reader = PbfReader::open(&temp.0).unwrap();
            reader.set_thread_count(threads);
            let calls = AtomicUsize::new(0);
            reader
                .read_entity_views(false, |_, g| {
                    let EntityViews::Nodes(nodes) = g else {
                        panic!("Expected nodes");
                    };
                    assert_eq!(nodes.len(), 1);
                    let n = nodes.get(0).unwrap();
                    let i = (n.id() - 1) as usize;
                    assert_eq!(
                        n.tags().next().unwrap()?.value,
                        vec![b'a' + i as u8; sizes[i]]
                    );
                    calls.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })
                .unwrap();
            assert_eq!(calls.load(Ordering::Relaxed), sizes.len());
        }
    }
}

fn reject_group(group: &[u8]) {
    let temp = Temp::pbf(&[table(), msg(2, group)].concat());
    let mut reader = PbfReader::open(&temp.0).unwrap();
    assert!(
        reader.read_block(1, |b| b.validate()).is_err(),
        "Accepted {group:?}"
    );
}
#[test]
fn malformed_column_lengths_and_delta_overflows() {
    for count in [0, 1, 3] {
        let extra = vec![1; count];
        for field in [2, 3] {
            let tags = [
                msg(field, &extra),
                packed(if field == 2 { 3 } else { 2 }, &[1, 2]),
            ]
            .concat();
            for kind in [1, 3, 4] {
                let base = if kind == 1 { node(1) } else { int(1, 1) };
                reject_group(&msg(kind, &[base, tags.clone()].concat()));
            }
        }
        for field in [8, 9, 10] {
            let mut relation = [int(1, 1), msg(field, &extra)].concat();
            for other in [8, 9, 10] {
                if other != field {
                    relation.extend(packed(other, &[1, 2]));
                }
            }
            reject_group(&msg(4, &relation));
        }
        for field in [1, 8, 9] {
            let mut dense = msg(field, &extra);
            for other in [1, 8, 9] {
                if other != field {
                    dense.extend(packed(other, &[2, 2]));
                }
            }
            reject_group(&msg(2, &dense));
        }
        if count != 0 {
            for field in 1..=6 {
                let dense = [
                    msg(5, &msg(field, &extra)),
                    packed(1, &[2, 2]),
                    packed(8, &[2, 2]),
                    packed(9, &[2, 2]),
                ]
                .concat();
                reject_group(&msg(2, &dense));
            }
        }
    }
    for (base, step) in [(i64::MAX, 1), (i64::MIN, -1)] {
        for fragmented in [false, true] {
            let deltas = |field| {
                if fragmented {
                    [packed(field, &[zz(base)]), int(field, zz(step))].concat()
                } else {
                    packed(field, &[zz(base), zz(step)])
                }
            };
            reject_group(&msg(3, &[int(1, 1), deltas(8)].concat()));
            reject_group(&msg(
                4,
                &[
                    int(1, 1),
                    packed(8, &[0, 0]),
                    packed(10, &[0, 0]),
                    deltas(9),
                ]
                .concat(),
            ));
            for field in [1, 8, 9] {
                let mut dense = deltas(field);
                for other in [1, 8, 9] {
                    if other != field {
                        dense.extend(packed(other, &[0, 0]));
                    }
                }
                reject_group(&msg(2, &dense));
            }
        }
    }
    for length in 1..=10 {
        reject_group(&msg(3, &[int(1, 1), msg(8, &vec![0x80; length])].concat()));
    }
    for field in [1, 8, 9] {
        for invalid in [vec![0x80], [vec![0xff; 9], vec![2]].concat()] {
            let mut dense = msg(field, &invalid);
            for other in [1, 8, 9] {
                if other != field {
                    dense.extend(packed(other, &[0]));
                }
            }
            reject_group(&msg(2, &dense));
        }
    }
}
#[test]
fn every_file_truncation_and_unknown_group_nesting() {
    let raw = [table(), msg(2, &msg(1, &node(1)))].concat();
    let bytes = [header(), block(b"OSMData", &raw)].concat();
    for length in 0..bytes.len() {
        if length == header().len() {
            continue;
        }
        let temp = Temp::bytes(&bytes[..length], "pbf");
        let result = PbfReader::open(&temp.0).and_then(|mut r| r.read_blocks(|b| b.validate()));
        assert!(result.is_err(), "Truncation length {length}");
    }
    let valid = [var(99 << 3 | 3), int(1, 7), var(99 << 3 | 4)].concat();
    let temp = Temp::pbf(&[raw.clone(), valid].concat());
    PbfReader::open(&temp.0)
        .unwrap()
        .read_blocks(|b| b.validate())
        .unwrap();
    for suffix in [
        var(99 << 3 | 4),
        [var(99 << 3 | 3), var(98 << 3 | 4)].concat(),
        [var(99 << 3 | 3).repeat(65), var(99 << 3 | 4).repeat(65)].concat(),
    ] {
        let temp = Temp::pbf(&[raw.clone(), suffix].concat());
        assert!(
            PbfReader::open(&temp.0)
                .unwrap()
                .read_blocks(|b| b.validate())
                .is_err()
        );
    }
}
