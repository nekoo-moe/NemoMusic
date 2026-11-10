//! The render path performs zero heap allocations, on both the normal and the underrun path.
//! This binary installs a counting global allocator, which needs `unsafe`.
#![allow(unsafe_code)] // reason: GlobalAlloc is an unsafe trait; test-only counting wrapper

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use nemo_audio::render::render;
use nemo_audio::ring::{new_ring, push_slice};

struct Counting;

thread_local! {
    // Per-thread count, so parallel tests do not see each other's allocations. A const-initialised
    // `Cell` has no destructor and never allocates.
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    ALLOCS.with(|c| c.set(c.get() + 1));
}

fn count() -> usize {
    ALLOCS.with(Cell::get)
}

// SAFETY: forwards every call unchanged to the system allocator and only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

const CALLBACK_SAMPLES: usize = 512 * 2;
const CALLBACKS_10S: usize = 48_000 * 10 / 512 + 1;

#[test]
fn counter_sees_allocations() {
    let before = count();
    let boxed = std::hint::black_box(Box::new([0_u8; 64]));
    assert!(count() > before);
    drop(boxed);
}

#[test]
fn ten_seconds_of_render_allocate_nothing() {
    let (mut producer, mut consumer) = new_ring(CALLBACK_SAMPLES * CALLBACKS_10S);
    let block: Vec<f32> = (0..u16::try_from(CALLBACK_SAMPLES).unwrap_or(u16::MAX))
        .map(|i| f32::from(i) * 1e-4)
        .collect();
    for _ in 0..CALLBACKS_10S {
        assert_eq!(push_slice(&mut producer, &block), CALLBACK_SAMPLES);
    }
    let underruns = AtomicU64::new(0);
    let mut out = vec![0.0_f32; CALLBACK_SAMPLES];

    let before = count();
    for _ in 0..CALLBACKS_10S {
        render(&mut consumer, &mut out, &underruns);
    }
    let after = count();

    assert_eq!(after - before, 0, "render allocated on the normal path");
    assert_eq!(underruns.load(Ordering::SeqCst), 0);
}

#[test]
fn underrun_path_allocates_nothing() {
    let (mut producer, mut consumer) = new_ring(CALLBACK_SAMPLES);
    let half = vec![0.5_f32; CALLBACK_SAMPLES / 2];
    push_slice(&mut producer, &half);
    let underruns = AtomicU64::new(0);
    let mut out = vec![1.0_f32; CALLBACK_SAMPLES];

    let before = count();
    for _ in 0..1_000 {
        render(&mut consumer, &mut out, &underruns); // partial, then empty
    }
    let after = count();

    assert_eq!(after - before, 0, "render allocated on the underrun path");
    assert_eq!(underruns.load(Ordering::SeqCst), 1_000);
    assert!(out.iter().all(|s| *s == 0.0));
}
