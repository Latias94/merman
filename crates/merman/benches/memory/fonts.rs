#[path = "../support/font_workloads.rs"]
mod font_workloads;

use super::ALLOCATOR;
use font_workloads::Observer;
use serde_json::json;
use std::hint::black_box;

struct Allocations;

impl Observer for Allocations {
    fn single<T>(&mut self, name: &str, operation: impl FnMut() -> T) {
        record(name, operation);
    }

    fn batched<I, T>(
        &mut self,
        name: &str,
        mut setup: impl FnMut() -> I,
        mut operation: impl FnMut(I) -> T,
    ) {
        let input = setup();
        record(name, || operation(input));
    }
}

fn record<T>(name: &str, operation: impl FnOnce() -> T) {
    let snapshot = ALLOCATOR.begin_measurement();
    let output = black_box(operation());
    let metrics = ALLOCATOR.finish_measurement(snapshot);
    drop(output);
    assert!(!metrics.counter_overflowed && !metrics.counter_underflowed);
    println!(
        "{}",
        json!({
            "benchmark": name,
            "memory_scope": "single-operation-counting-allocator",
            "snapshot_live_bytes": metrics.snapshot_live_bytes,
            "allocation_count": metrics.allocation_count,
            "allocated_bytes": metrics.allocated_bytes,
            "live_bytes_after": metrics.live_bytes_after,
            "peak_live_bytes": metrics.peak_live_bytes,
            "peak_growth_bytes": metrics.peak_growth_bytes,
        })
    );
}

pub(super) fn observe() {
    font_workloads::observe(&mut Allocations);
}
