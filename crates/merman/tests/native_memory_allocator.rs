#[path = "../benches/native_memory/allocator.rs"]
mod allocator;
#[cfg(feature = "benchmark-stages")]
#[path = "../benches/memory/stages.rs"]
mod stages;

use allocator::CountingSystemAllocator;
use std::alloc::{GlobalAlloc, Layout, System};

#[cfg(feature = "benchmark-stages")]
type ProbeAllocator = CountingSystemAllocator;

#[test]
fn measurement_resets_cumulative_counters_but_preserves_live_setup() {
    let allocator = CountingSystemAllocator::new(std::alloc::System);
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
    let overflowed = CountingSystemAllocator::new(std::alloc::System);
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

    let underflowed = CountingSystemAllocator::new(std::alloc::System);
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
    let allocator = CountingSystemAllocator::new(std::alloc::System);
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

#[cfg(feature = "benchmark-stages")]
#[test]
fn phase_allocations_are_exclusive_and_keep_interval_and_batch_peaks() {
    use merman_model::benchmark_stages::{Stage, enter};

    static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new(std::alloc::System);
    let outside = Layout::from_size_align(8, 8).expect("valid unscoped layout");
    let layout = Layout::from_size_align(32, 8).expect("valid layout allocation");
    let label = Layout::from_size_align(64, 8).expect("valid label layout");
    let end = Layout::from_size_align(16, 8).expect("valid final layout");

    // SAFETY: each pointer is paired with one deallocation using the same layout.
    let ((), heap, report) = stages::capture(&ALLOCATOR, || unsafe {
        let retained = ALLOCATOR.alloc(outside);
        assert!(!retained.is_null());
        let _layout = enter(Stage::LayoutSolve);
        let buffer = ALLOCATOR.alloc(layout);
        assert!(!buffer.is_null());
        {
            let _measure = enter(Stage::Measure);
            let text = ALLOCATOR.alloc_zeroed(label);
            assert!(!text.is_null());
            ALLOCATOR.dealloc(text, label);
        }
        ALLOCATOR.dealloc(buffer, layout);
        let path = ALLOCATOR.alloc(end);
        assert!(!path.is_null());
        ALLOCATOR.dealloc(path, end);
        ALLOCATOR.dealloc(retained, outside);
    });

    let layout = report.stages[Stage::LayoutSolve as usize];
    assert_eq!((layout.allocations, layout.allocated_bytes), (2, 48));
    assert_eq!(layout.peak_growth_bytes, 32);
    let measure = report.stages[Stage::Measure as usize];
    assert_eq!((measure.allocations, measure.allocated_bytes), (1, 64));
    assert_eq!(measure.peak_growth_bytes, 64);
    assert_eq!(report.unattributed.allocations, 1);
    assert_eq!(report.unattributed.allocated_bytes, 8);
    assert_eq!(report.unattributed.peak_growth_bytes, 8);

    assert_eq!((heap.allocation_count, heap.allocated_bytes), (4, 120));
    assert_eq!(heap.peak_growth_bytes, 104);
    assert_eq!(heap.live_bytes_after, 0);
    assert!(!heap.counter_overflowed && !heap.counter_underflowed);
}

#[cfg(feature = "benchmark-stages")]
#[test]
fn stage_capture_rejects_a_counter_reset_during_an_interval() {
    use merman_model::benchmark_stages::{Stage, enter};

    static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new(System);
    let layout = Layout::from_size_align(8, 8).expect("valid test allocation");
    let ((), heap, _) = stages::capture(&ALLOCATOR, || {
        // SAFETY: the allocation and deallocation use the same layout and allocator.
        unsafe {
            let retained = ALLOCATOR.alloc(layout);
            assert!(!retained.is_null());
            {
                let _stage = enter(Stage::Measure);
                assert_eq!(ALLOCATOR.begin_measurement(), 8);
            }
            ALLOCATOR.dealloc(retained, layout);
        }
    });
    assert!(heap.counter_underflowed);
}
