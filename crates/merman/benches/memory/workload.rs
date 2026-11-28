use super::{ALLOCATOR, AllocationMetrics, WorkloadMode, workloads};

#[cfg(feature = "benchmark-stages")]
use super::stages;

pub(super) fn measure(mode: WorkloadMode) -> Result<(), String> {
    match mode {
        WorkloadMode::Counters => workloads::run("memory", |trial| {
            let start = ALLOCATOR.begin_measurement();
            let result = trial.batch();
            let heap = ALLOCATOR.finish_measurement(start);
            workload_metrics(trial, result, heap)
        }),
        #[cfg(feature = "benchmark-stages")]
        WorkloadMode::Stages => measure_stages(),
        #[cfg(not(feature = "benchmark-stages"))]
        WorkloadMode::Stages => {
            Err("stage allocation attribution requires the benchmark-stages feature".into())
        }
        #[cfg(all(feature = "dhat-heap", not(target_arch = "wasm32")))]
        WorkloadMode::HeapProfile(path) => {
            if path.exists() {
                return Err("choose a new heap profile output path".into());
            }
            workloads::run("memory", |trial| {
                let profiler = dhat::Profiler::builder().file_name(&path).build();
                let result = trial.batch();
                drop(profiler);
                trial.release();
                result?;
                Ok(serde_json::json!({"heap_profile": path}))
            })
        }
    }
}

#[cfg(feature = "benchmark-stages")]
fn measure_stages() -> Result<(), String> {
    use merman_model::benchmark_stages::Stage;
    use serde_json::{Map, json};

    workloads::run("stage-memory", |trial| {
        let (result, heap, report) = stages::capture(&ALLOCATOR, || trial.batch());
        let mut metrics = workload_metrics(trial, result, heap)?;
        let stage_row = |heap: stages::StageHeap| {
            json!({"allocations": heap.allocations, "allocated_bytes": heap.allocated_bytes,
                   "peak_growth_bytes": heap.peak_growth_bytes})
        };
        let mut rows: Map<_, _> = Stage::ALL
            .into_iter()
            .map(|stage| {
                (
                    stage.id().to_owned(),
                    stage_row(report.stages[stage as usize]),
                )
            })
            .collect();
        rows.insert("unattributed".into(), stage_row(report.unattributed));
        rows.insert("end_to_end".into(), json!({
            "allocations": metrics["allocations"], "allocated_bytes": metrics["allocated_bytes"],
            "peak_growth_bytes": metrics["peak_growth_bytes"],
        }));
        metrics["stages"] = rows.into();
        Ok(metrics)
    })
}

fn workload_metrics(
    trial: &mut workloads::Trial<'_>,
    result: Result<(), String>,
    heap: AllocationMetrics,
) -> Result<serde_json::Value, String> {
    trial.release();
    let after_release = ALLOCATOR.begin_measurement();
    ALLOCATOR.stop_measurement();
    result?;
    if heap.counter_overflowed || heap.counter_underflowed {
        return Err("invalid allocation counters".into());
    }
    debug_assert_eq!(
        heap.peak_live_bytes.checked_sub(heap.snapshot_live_bytes),
        Some(heap.peak_growth_bytes)
    );
    let delta = |value: u64| i128::from(value) - i128::from(heap.snapshot_live_bytes);
    Ok(serde_json::json!({
        "allocations": heap.allocation_count, "allocated_bytes": heap.allocated_bytes,
        "snapshot_live_bytes": heap.snapshot_live_bytes, "peak_live_bytes": heap.peak_live_bytes,
        "peak_growth_bytes": heap.peak_growth_bytes,
        "live_delta_bytes": delta(heap.live_bytes_after),
        "after_release_delta_bytes": delta(after_release),
    }))
}
