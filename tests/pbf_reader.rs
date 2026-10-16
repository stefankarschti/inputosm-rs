//! Reader contract tests adapted from the C++ unit tests.
mod common;
use common::*;
use inputosm::*;
use std::{
    sync::{Arc, Barrier, Mutex},
    thread,
};

fn node(id: i64) -> Vec<u8> {
    [int(1, zz(id)), int(8, 0), int(9, 0)].concat()
}
fn payload(id: i64) -> Vec<u8> {
    [table(), msg(2, &msg(1, &node(id)))].concat()
}
fn id(block: &mut PbfBlock<'_>) -> Result<i64> {
    let mut value = 0;
    block.decode_nodes(
        NodeOptions {
            id: true,
            ..Default::default()
        },
        |nodes| {
            value = nodes.ids[0];
            Ok(())
        },
    )?;
    Ok(value)
}
#[test]
fn ownership_move_empty_file_and_index_idempotence() {
    let temp = Temp::pbf(&payload(17));
    let mut reader = PbfReader::open(&temp.0).unwrap();
    assert_eq!(reader.thread_count(), 1);
    assert!(!reader.has_index());
    assert_eq!(reader.index_memory_bytes(), 0);
    reader.build_index().unwrap();
    let bytes = reader.index_memory_bytes();
    reader.build_index().unwrap();
    assert_eq!(reader.index_memory_bytes(), bytes);
    let mut moved = reader;
    moved
        .read_block(1, |b| {
            assert_eq!(id(b)?, 17);
            Ok(())
        })
        .unwrap();
    drop(moved);
    let empty = Temp::bytes(&header(), "pbf");
    let mut reader = PbfReader::open(&empty.0).unwrap();
    reader
        .read_blocks(|_| panic!("Header-only input has no data blocks"))
        .unwrap();
    reader.build_index().unwrap();
    for index in [0, 1, usize::MAX] {
        assert!(reader.read_block(index, |_| Ok(())).is_err());
    }
    for length in 0..header().len() {
        let temp = Temp::bytes(&header()[..length], "pbf");
        assert!(
            PbfReader::open(&temp.0).is_err(),
            "Truncated header length {length}"
        );
    }
}
#[test]
fn failed_read_and_failed_index_leave_reader_usable() {
    let temp = Temp::bytes(
        &[
            header(),
            block(b"OSMData", b"invalid"),
            block(b"OSMData", &payload(23)),
        ]
        .concat(),
        "pbf",
    );
    for indexed in [false, true] {
        let mut reader = PbfReader::open(&temp.0).unwrap();
        if indexed {
            reader.build_index().unwrap();
        }
        assert!(reader.read_block(1, |b| b.validate()).is_err());
        assert!(reader.read_block(2, |_| Err(Error::Cancelled)).is_err());
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = reader.read_block(2, |_| panic!("Read callback panic"));
        }));
        assert!(unwind.is_err());
        reader
            .read_block(2, |b| {
                assert_eq!(id(b)?, 23);
                Ok(())
            })
            .unwrap();
    }
    for tail in [
        b"bad".to_vec(),
        header(),
        65536u32.to_be_bytes().to_vec(),
        vec![0; 4],
    ] {
        let temp = Temp::bytes(
            &[header(), block(b"OSMData", &payload(23)), tail].concat(),
            "pbf",
        );
        let mut reader = PbfReader::open(&temp.0).unwrap();
        assert!(reader.build_index().is_err());
        assert!(!reader.has_index());
        assert_eq!(reader.index_memory_bytes(), 0);
        reader
            .read_block(1, |b| {
                assert_eq!(id(b)?, 23);
                Ok(())
            })
            .unwrap();
    }
}
#[test]
fn descriptor_order_duplicates_unknown_fields_and_invalid_sizes() {
    let raw_blob = msg(1, &payload(42));
    let kind = msg(1, b"OSMData");
    let size = int(3, raw_blob.len() as u64);
    let valid = [
        [kind.clone(), size.clone()].concat(),
        [size.clone(), kind.clone()].concat(),
        [kind.clone(), size.clone(), msg(2, b"optional index data")].concat(),
        [kind.clone(), int(3, 0), size.clone()].concat(),
        [kind.clone(), int(3, u64::MAX), size.clone()].concat(),
        [msg(1, b"Unknown"), kind.clone(), size.clone()].concat(),
        [kind.clone(), size.clone(), int(99, 123)].concat(),
        [
            kind.clone(),
            size.clone(),
            var(99 << 3 | 3),
            int(1, 7),
            var(99 << 3 | 4),
        ]
        .concat(),
    ];
    let invalid = [
        kind.clone(),
        [kind.clone(), msg(3, b"bad")].concat(),
        [kind.clone(), int(3, 1 << 31)].concat(),
        [kind.clone(), int(3, u64::MAX)].concat(),
        [kind.clone(), vec![0x18, 0x80]].concat(),
        [kind.clone(), size, vec![0]].concat(),
    ];
    for (descriptors, success) in [(&valid[..], true), (&invalid[..], false)] {
        for descriptor in descriptors {
            let temp = Temp::bytes(
                &[
                    header(),
                    (descriptor.len() as u32).to_be_bytes().to_vec(),
                    descriptor.clone(),
                    raw_blob.clone(),
                ]
                .concat(),
                "pbf",
            );
            let mut reader = PbfReader::open(&temp.0).unwrap();
            let result = reader.read_block(1, |b| {
                assert_eq!(id(b)?, 42);
                Ok(())
            });
            assert_eq!(result.is_ok(), success, "{descriptor:?}: {result:?}");
            assert_eq!(reader.build_index().is_ok(), success);
        }
    }
}
#[test]
fn nested_readers_preserve_outer_borrowed_data_and_caller_thread() {
    let first = Temp::pbf(&payload(17));
    let second = Temp::pbf(&payload(99));
    let mut outer = PbfReader::open(&first.0).unwrap();
    let mut inner = PbfReader::open(&second.0).unwrap();
    let caller = thread::current().id();
    outer
        .read_block(1, |block| {
            block.decode_entities_with_strings(EntityOptions::all(true), |group, strings| {
                let GroupBatch::Nodes(nodes) = group else {
                    panic!("Expected nodes");
                };
                assert_eq!(thread::current().id(), caller);
                let pointer = nodes.ids.as_ptr();
                inner.read_block(1, |b| {
                    assert_eq!(id(b)?, 99);
                    Ok(())
                })?;
                assert_eq!(nodes.ids.as_ptr(), pointer);
                assert_eq!(nodes.ids, [17]);
                assert_eq!(strings[1], b"key");
                Ok(())
            })
        })
        .unwrap();
}
#[test]
fn independent_readers_do_not_share_cancellation_or_buffers() {
    let temp = Temp::pbf(&payload(77));
    for first_all in [false, true] {
        for second_all in [false, true] {
            let barrier = Arc::new(Barrier::new(2));
            thread::scope(|scope| {
                for (worker, all) in [first_all, second_all].into_iter().enumerate() {
                    let path = &temp.0;
                    let barrier = barrier.clone();
                    scope.spawn(move || {
                        let mut reader = PbfReader::open(path).unwrap();
                        if worker == 1 {
                            reader.build_index().unwrap();
                        }
                        let handler = |b: &mut PbfBlock<'_>| {
                            b.decode_nodes(
                                NodeOptions {
                                    id: true,
                                    ..Default::default()
                                },
                                |batch| {
                                    assert_eq!(batch.ids, [77]);
                                    barrier.wait();
                                    if worker == 0 {
                                        return Err(Error::Cancelled);
                                    }
                                    assert_eq!(batch.ids, [77]);
                                    Ok(())
                                },
                            )
                        };
                        let result = if all {
                            reader.read_blocks(handler)
                        } else {
                            reader.read_block(1, handler)
                        };
                        assert_eq!(result.is_ok(), worker == 1);
                        reader.read_block(1, |b| b.validate()).unwrap();
                    });
                }
            });
        }
    }
}
#[cfg(unix)]
#[test]
fn replacement_path_does_not_change_an_open_reader() {
    let temp = Temp::pbf(&payload(77));
    let replacement = Temp::pbf(&payload(99));
    let mut old = PbfReader::open(&temp.0).unwrap();
    std::fs::rename(&replacement.0, &temp.0).unwrap();
    let mut new = PbfReader::open(&temp.0).unwrap();
    old.read_block(1, |b| {
        assert_eq!(id(b)?, 77);
        Ok(())
    })
    .unwrap();
    new.read_block(1, |b| {
        assert_eq!(id(b)?, 99);
        Ok(())
    })
    .unwrap();
    drop(new);
    old.read_block(1, |b| {
        assert_eq!(id(b)?, 77);
        Ok(())
    })
    .unwrap();
}
#[cfg(all(unix, target_pointer_width = "64"))]
#[test]
fn sparse_file_offsets_exceed_four_gibibytes() {
    use std::io::{Seek, SeekFrom, Write};
    let temp = Temp::bytes(&header(), "pbf");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&temp.0)
        .unwrap();
    file.seek(SeekFrom::End(0)).unwrap();
    let skipped = 129usize;
    let blob_size = 32 * 1024 * 1024;
    for _ in 0..skipped {
        let descriptor = [msg(1, b"Unknown"), int(3, blob_size as u64)].concat();
        file.write_all(&(descriptor.len() as u32).to_be_bytes())
            .unwrap();
        file.write_all(&descriptor).unwrap();
        file.seek(SeekFrom::Current(blob_size)).unwrap();
    }
    let offset = file.stream_position().unwrap();
    assert!(offset > 1u64 << 32);
    file.write_all(&block(b"OSMData", &payload(42))).unwrap();
    drop(file);
    let mut reader = PbfReader::open(&temp.0).unwrap();
    for indexed in [false, true] {
        if indexed {
            reader.build_index().unwrap();
        }
        reader
            .read_block(skipped + 1, |b| {
                assert_eq!(b.file_offset(), offset);
                assert_eq!(id(b)?, 42);
                Ok(())
            })
            .unwrap();
    }
}
#[test]
fn dense_deltas_reset_between_many_groups() {
    let dense = [
        msg(1, &[2; 512]),
        msg(8, &[2; 512]),
        msg(9, &[2; 512]),
        msg(10, &[0; 512]),
    ]
    .concat();
    let mut raw = table();
    for _ in 0..24 {
        raw.extend(msg(2, &msg(2, &dense)));
    }
    let temp = Temp::pbf(&raw);
    struct Handler(Mutex<usize>);
    impl EntityHandler for Handler {
        fn nodes(&self, context: Context, nodes: &[Node<'_>]) -> Result<()> {
            assert_eq!(context.block_index, 1);
            assert_eq!(nodes.len(), 512);
            for (i, node) in nodes.iter().enumerate() {
                let expected = i as i64 + 1;
                assert_eq!(
                    (node.id, node.raw_latitude, node.raw_longitude),
                    (expected, expected, expected)
                );
                assert!(node.tags.is_empty());
            }
            *self.0.lock().unwrap() += 1;
            Ok(())
        }
    }
    let handler = Handler(Mutex::new(0));
    input_file(
        &temp.0,
        InputOptions {
            threads: 4,
            ..Default::default()
        },
        &handler,
    )
    .unwrap();
    assert_eq!(*handler.0.lock().unwrap(), 24);
}

#[test]
fn complete_header_metadata_and_split_bbox() {
    let raw = [
        msg(1, &[int(1, zz(-10)), int(2, zz(20))].concat()),
        msg(1, &[int(3, zz(30)), int(4, zz(-40))].concat()),
        msg(4, b"OsmSchema-V0.6"),
        msg(4, b"DenseNodes"),
        msg(5, b"LocationsOnWays"),
        msg(5, b"unknown optional feature"),
        msg(16, b""),
        msg(17, b"source"),
        int(32, 0),
        int(33, i64::MAX as u64),
        msg(34, b"https://example.test/replication"),
    ]
    .concat();
    let temp = Temp::bytes(&block(b"OSMHeader", &raw), "pbf");
    let mut reader = PbfReader::open(&temp.0).unwrap();
    let header = reader.header();
    let bbox = header.bbox.as_ref().unwrap();
    assert_eq!(
        (bbox.left, bbox.right, bbox.top, bbox.bottom),
        (-10, 20, 30, -40)
    );
    assert_eq!(
        header.required_features,
        [b"OsmSchema-V0.6".to_vec(), b"DenseNodes".to_vec()]
    );
    assert_eq!(header.optional_features.len(), 2);
    assert_eq!(header.writingprogram.as_deref(), Some(&b""[..]));
    assert_eq!(header.source.as_deref(), Some(&b"source"[..]));
    assert_eq!(header.osmosis_replication_timestamp, Some(0));
    assert_eq!(header.osmosis_replication_sequence_number, Some(i64::MAX));
    assert_eq!(
        header.osmosis_replication_base_url.as_deref(),
        Some(&b"https://example.test/replication"[..])
    );
    reader.read_blocks(|_| panic!("No data blocks")).unwrap();
}
