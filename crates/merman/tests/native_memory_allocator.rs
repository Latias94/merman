#[path = "../benches/native_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use std::alloc::{GlobalAlloc, Layout, System};

#[test]
fn measurement_resets_cumulative_counters_but_preserves_live_setup() {
    let allocator = CountingSystemAllocator::new();
    let setup_layout = Layout::from_size_align(16, 8).expect("valid setup layout");
    let initial_layout = Layout::from_size_align(32, 8).expect("valid initial layout");
    let grown_layout = Layout::from_size_align(64, 8).expect("valid grown layout");
    let final_layout = Layout::from_size_align(8, 8).expect("valid final layout");

    // SAFETY: each allocation uses a valid layout and is released exactly once with its current
    // layout after every successful reallocation.
    unsafe {
        let setup = allocator.alloc(setup_layout);
        assert!(!setup.is_null());

        let snapshot = allocator.begin_measurement();
        assert_eq!(snapshot, 16);

        let zeroed = allocator.alloc_zeroed(initial_layout);
        assert!(!zeroed.is_null());
        assert!((0..initial_layout.size()).all(|index| zeroed.add(index).read() == 0));

        let grown = allocator.realloc(zeroed, initial_layout, grown_layout.size());
        assert!(!grown.is_null());
        let shrunk = allocator.realloc(grown, grown_layout, final_layout.size());
        assert!(!shrunk.is_null());

        let metrics = allocator.finish_measurement(snapshot);
        assert_eq!(metrics.snapshot_live_bytes, 16);
        assert_eq!(metrics.allocation_count, 3);
        assert_eq!(metrics.allocated_bytes, 32 + 64 + 8);
        assert_eq!(metrics.live_bytes_after, 16 + 8);
        assert_eq!(metrics.peak_live_bytes, 16 + 64);
        assert_eq!(metrics.peak_growth_bytes, 64);
        assert!(!metrics.counter_overflowed);
        assert!(!metrics.counter_underflowed);

        allocator.dealloc(shrunk, final_layout);
        allocator.dealloc(setup, setup_layout);
        let final_snapshot = allocator.begin_measurement();
        assert_eq!(final_snapshot, 0);
        let frozen = allocator.finish_measurement(final_snapshot);
        assert_eq!(frozen.allocation_count, 0);
        assert_eq!(frozen.allocated_bytes, 0);
    }
}

#[test]
fn counter_damage_is_checked_and_sticky_across_measurement_reset() {
    let overflowed = CountingSystemAllocator::new();
    overflowed.force_live_bytes_for_test(u64::MAX);
    let layout = Layout::from_size_align(1, 1).expect("valid byte layout");

    // SAFETY: the allocation uses a valid layout and the returned pointer is released once.
    unsafe {
        let snapshot = overflowed.begin_measurement();
        let pointer = overflowed.alloc(layout);
        assert!(!pointer.is_null());
        let first = overflowed.finish_measurement(snapshot);
        assert!(first.counter_overflowed);

        let second_snapshot = overflowed.begin_measurement();
        let second = overflowed.finish_measurement(second_snapshot);
        assert!(second.counter_overflowed);
        overflowed.dealloc(pointer, layout);
    }

    let underflowed = CountingSystemAllocator::new();
    // Allocate directly through System so the counting allocator legitimately observes a
    // deallocation whose setup allocation was not charged to its live counter.
    // SAFETY: the pointer and layout are passed unchanged to the matching System deallocator.
    unsafe {
        let pointer = System.alloc(layout);
        assert!(!pointer.is_null());
        underflowed.dealloc(pointer, layout);
    }
    let snapshot = underflowed.begin_measurement();
    let metrics = underflowed.finish_measurement(snapshot);
    assert!(metrics.counter_underflowed);
}

#[test]
fn finish_freezes_cumulative_evidence_before_later_serialization_allocations() {
    let allocator = CountingSystemAllocator::new();
    let layout = Layout::from_size_align(24, 8).expect("valid layout");

    // SAFETY: each pointer is paired with one deallocation using the same layout.
    unsafe {
        let snapshot = allocator.begin_measurement();
        let measured = allocator.alloc(layout);
        assert!(!measured.is_null());
        let frozen = allocator.finish_measurement(snapshot);

        let serialization = allocator.alloc(layout);
        assert!(!serialization.is_null());
        assert_eq!(frozen.allocation_count, 1);
        assert_eq!(frozen.allocated_bytes, 24);
        assert_eq!(frozen.peak_growth_bytes, 24);

        allocator.dealloc(serialization, layout);
        allocator.dealloc(measured, layout);
    }
}
