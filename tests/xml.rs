//! Verify all entity fields from the original XML and OSC fixtures.
use inputosm::*;
use std::{path::Path, sync::Mutex};

#[derive(Debug, PartialEq, Eq)]
struct Record {
    mode: Mode,
    kind: char,
    id: i64,
    coordinates: Option<(i64, i64)>,
    metadata: (i32, i64, i64),
    tags: Vec<(String, String)>,
    refs: Vec<i64>,
    members: Vec<(MemberType, i64, String)>,
}
fn strings(tags: &[Tag<'_>]) -> Vec<(String, String)> {
    tags.iter()
        .map(|t| {
            (
                String::from_utf8(t.key.to_vec()).unwrap(),
                String::from_utf8(t.value.to_vec()).unwrap(),
            )
        })
        .collect()
}
fn metadata(m: Metadata) -> (i32, i64, i64) {
    (m.version, m.raw_timestamp, m.changeset)
}
struct Collector(Mutex<Vec<Record>>);
impl EntityHandler for Collector {
    fn nodes(&self, c: Context, nodes: &[Node<'_>]) -> Result<()> {
        assert_eq!(c.file_type, FileType::Xml);
        for n in nodes {
            self.0.lock().unwrap().push(Record {
                mode: c.mode,
                kind: 'N',
                id: n.id,
                coordinates: Some((n.raw_latitude, n.raw_longitude)),
                metadata: metadata(n.metadata),
                tags: strings(n.tags),
                refs: vec![],
                members: vec![],
            });
        }
        Ok(())
    }
    fn ways(&self, c: Context, ways: &[Way<'_>]) -> Result<()> {
        for w in ways {
            self.0.lock().unwrap().push(Record {
                mode: c.mode,
                kind: 'W',
                id: w.id,
                coordinates: None,
                metadata: metadata(w.metadata),
                tags: strings(w.tags),
                refs: w.node_refs.to_vec(),
                members: vec![],
            });
        }
        Ok(())
    }
    fn relations(&self, c: Context, relations: &[Relation<'_>]) -> Result<()> {
        for r in relations {
            self.0.lock().unwrap().push(Record {
                mode: c.mode,
                kind: 'R',
                id: r.id,
                coordinates: None,
                metadata: metadata(r.metadata),
                tags: strings(r.tags),
                refs: vec![],
                members: r
                    .members
                    .iter()
                    .map(|m| {
                        (
                            m.member_type,
                            m.id,
                            String::from_utf8(m.role.to_vec()).unwrap(),
                        )
                    })
                    .collect(),
            });
        }
        Ok(())
    }
}
#[test]
fn original_osm_and_osc_fixture_values_with_metadata_on_and_off() {
    for (file, osc) in [("sample.osm", false), ("sample.osc", true)] {
        for with_metadata in [false, true] {
            let handler = Collector(Mutex::new(Vec::new()));
            input_file(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/data")
                    .join(file),
                InputOptions {
                    decode_metadata: with_metadata,
                    threads: 4,
                },
                &handler,
            )
            .unwrap();
            let actual = handler.0.into_inner().unwrap();
            let entries = if osc {
                vec![
                    (
                        'N',
                        100,
                        Mode::Create,
                        Some((407128000, -740060000)),
                        7,
                        "2021-01-02T03:04:05Z",
                        1234,
                        vec![("name", "Create Node"), ("note", "created")],
                    ),
                    (
                        'W',
                        200,
                        Mode::Modify,
                        None,
                        8,
                        "2021-02-03T04:05:06Z",
                        2345,
                        vec![("highway", "secondary"), ("status", "modified")],
                    ),
                    (
                        'R',
                        300,
                        Mode::Delete,
                        None,
                        9,
                        "2021-03-04T05:06:07Z",
                        3456,
                        vec![("type", "route"), ("route", "tram")],
                    ),
                ]
            } else {
                vec![
                    (
                        'N',
                        1,
                        Mode::Bulk,
                        Some((525200000, 134050000)),
                        3,
                        "2020-01-02T03:04:05Z",
                        111,
                        vec![("name", "Node One"), ("amenity", "cafe")],
                    ),
                    (
                        'N',
                        2,
                        Mode::Bulk,
                        Some((488566000, 23522000)),
                        2,
                        "2020-02-03T04:05:06Z",
                        222,
                        vec![("name", "Node Two")],
                    ),
                    (
                        'W',
                        10,
                        Mode::Bulk,
                        None,
                        4,
                        "2020-03-04T05:06:07Z",
                        333,
                        vec![("highway", "residential"), ("name", "A Street")],
                    ),
                    (
                        'R',
                        20,
                        Mode::Bulk,
                        None,
                        5,
                        "2020-04-05T06:07:08Z",
                        444,
                        vec![("type", "route"), ("route", "bus")],
                    ),
                ]
            };
            let expected: Vec<_> = entries
                .into_iter()
                .map(
                    |(kind, id, mode, coordinates, version, timestamp, changeset, tags)| Record {
                        kind,
                        id,
                        mode,
                        coordinates,
                        metadata: if with_metadata {
                            (
                                version,
                                timeutil::parse_timestamp(timestamp).unwrap(),
                                changeset,
                            )
                        } else {
                            metadata(Metadata::default())
                        },
                        tags: tags
                            .into_iter()
                            .map(|(k, v)| (k.into(), v.into()))
                            .collect(),
                        refs: if kind == 'W' {
                            if osc { vec![100, 101] } else { vec![1, 2] }
                        } else {
                            vec![]
                        },
                        members: if kind == 'R' {
                            vec![
                                (MemberType::Node, if osc { 100 } else { 1 }, "stop".into()),
                                (MemberType::Way, if osc { 200 } else { 10 }, "route".into()),
                            ]
                        } else {
                            vec![]
                        },
                    },
                )
                .collect();
            assert_eq!(actual, expected, "{file} metadata={with_metadata}");
        }
    }
}
