//! Count Rust allocator requests. Do not use this binary for timing comparisons.

#[allow(dead_code)]
#[path = "entity_profile.rs"]
mod consumer;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

struct Allocator;
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REQUESTED_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_BYTES: AtomicU64 = AtomicU64::new(0);

fn allocated(bytes: usize) {
    REQUESTED_BYTES.fetch_add(bytes as u64, Relaxed);
    let live = LIVE_BYTES.fetch_add(bytes as u64, Relaxed) + bytes as u64;
    PEAK_BYTES.fetch_max(live, Relaxed);
}

// SAFETY: Each operation delegates to System with the original pointer and layout.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            ALLOCATIONS.fetch_add(1, Relaxed);
            allocated(layout.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            ALLOCATIONS.fetch_add(1, Relaxed);
            allocated(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size() as u64, Relaxed);
        // SAFETY: System created this allocation with the supplied layout.
        unsafe { System.dealloc(p, layout) };
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: The caller supplies the original layout and a valid new size.
        let result = unsafe { System.realloc(p, layout, new_size) };
        if !result.is_null() {
            REALLOCATIONS.fetch_add(1, Relaxed);
            LIVE_BYTES.fetch_sub(layout.size() as u64, Relaxed);
            allocated(new_size);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn main() {
    let result = consumer::run();
    eprintln!(
        "{{\"allocations\":{},\"reallocations\":{},\"requested_bytes\":{},\"peak_live_bytes\":{}}}",
        ALLOCATIONS.load(Relaxed),
        REALLOCATIONS.load(Relaxed),
        REQUESTED_BYTES.load(Relaxed),
        PEAK_BYTES.load(Relaxed)
    );
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
