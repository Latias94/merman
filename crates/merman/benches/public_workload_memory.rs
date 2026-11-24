#[path = "native_memory/allocator.rs"]
mod allocator;
#[path = "workloads/mod.rs"]
mod workloads;

#[global_allocator]
static ALLOCATOR: allocator::CountingSystemAllocator = allocator::CountingSystemAllocator::new();

fn main() -> Result<(), String> {
    workloads::run("memory", |trial| {
        let start = ALLOCATOR.begin_measurement();
        let result = trial.batch();
        let heap = ALLOCATOR.finish_measurement(start);
        trial.release();
        let after_release = ALLOCATOR.begin_measurement();
        ALLOCATOR.stop_measurement();
        result?;
        if heap.counter_overflowed || heap.counter_underflowed {
            return Err("invalid allocation counters".into());
        }
        debug_assert_eq!(
            heap.peak_live_bytes - heap.snapshot_live_bytes,
            heap.peak_growth_bytes
        );
        let delta = |value: u64| i128::from(value) - i128::from(heap.snapshot_live_bytes);
        Ok(serde_json::json!({
            "allocations": heap.allocation_count, "allocated_bytes": heap.allocated_bytes,
            "snapshot_live_bytes": heap.snapshot_live_bytes, "peak_live_bytes": heap.peak_live_bytes,
            "peak_growth_bytes": heap.peak_growth_bytes,
            "live_delta_bytes": delta(heap.live_bytes_after),
            "after_release_delta_bytes": delta(after_release),
        }))
    })
}
