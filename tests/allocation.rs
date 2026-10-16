//! Check retained buffers after one complete decode.
mod common;
use common::*;
use inputosm::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
};
struct Allocator;
static MEASURE: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
fn record() {
    if MEASURE.load(Relaxed) {
        ALLOCATIONS.fetch_add(1, Relaxed);
    }
}
// SAFETY: Every operation delegates to System with the original allocation arguments.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: The caller supplies a valid layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: The caller supplies a valid layout.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        // SAFETY: The caller supplies the original pointer and layout with a valid new size.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: System allocated this pointer with the supplied layout.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn repeated_indexed_reads_do_not_allocate_after_warmup() {
    let dense = [
        packed(1, &[2, 2]),
        packed(8, &[2, 2]),
        packed(9, &[2, 2]),
        packed(10, &[1, 2, 0, 0]),
    ]
    .concat();
    let raw = [
        table(),
        msg(2, &msg(2, &dense)),
        msg(2, &msg(3, &[int(1, 3), packed(8, &[2, 2])].concat())),
        msg(
            2,
            &msg(
                4,
                &[
                    int(1, 4),
                    packed(8, &[1]),
                    packed(9, &[2]),
                    packed(10, &[0]),
                ]
                .concat(),
            ),
        ),
    ]
    .concat();
    let temp = Temp::bytes(&[header(), compressed(b"OSMData", &raw)].concat(), "pbf");
    let mut reader = PbfReader::open(&temp.0).unwrap();
    reader.build_index().unwrap();
    let decode = |block: &mut PbfBlock<'_>| {
        assert_eq!(
            block.counts()?,
            Counts {
                nodes: 2,
                ways: 1,
                relations: 1
            }
        );
        assert_eq!(block.string_table_size()?, 3);
        block.decode_strings(|_| Ok(()))?;
        block.decode_entities(EntityOptions::all(true), |_| Ok(()))
    };
    reader.read_block(1, decode).unwrap();
    MEASURE.store(true, Relaxed);
    let result: Result<()> = (0..100).try_for_each(|_| reader.read_block(1, decode));
    MEASURE.store(false, Relaxed);
    result.unwrap();
    assert_eq!(ALLOCATIONS.load(Relaxed), 0);
}
