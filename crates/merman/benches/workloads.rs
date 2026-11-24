use std::time::Instant;

#[path = "workloads/mod.rs"]
mod workloads;

fn main() -> Result<(), String> {
    // `cargo test --all-targets` runs harness-free benches without a job.
    if !std::env::args().any(|arg| arg == "--workload-stdin") {
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--stages") {
        #[cfg(feature = "benchmark-stages")]
        return run_stage_workload();
        #[cfg(not(feature = "benchmark-stages"))]
        return Err("stage attribution requires the benchmark-stages feature".into());
    }
    workloads::run("clock", |trial| {
        let start = Instant::now();
        trial.batch()?;
        let elapsed = start.elapsed();
        trial.release();
        Ok(serde_json::json!({"elapsed_ns": elapsed.as_nanos() as u64}))
    })
}

#[cfg(feature = "benchmark-stages")]
fn run_stage_workload() -> Result<(), String> {
    use merman_model::benchmark_stages::{Stage, capture};
    use serde_json::{Map, Value, json};

    let trace = std::env::args().any(|arg| arg == "--stage-trace");
    workloads::run("stages", |trial| {
        let (result, timing) = capture(trace, || trial.batch());
        result?;
        trial.release();
        let mut accounted_ns = 0;
        let mut stages = Map::new();
        for stage in Stage::ALL {
            let sample = timing.stages[stage as usize];
            let elapsed_ns = sample.elapsed.as_nanos() as u64;
            accounted_ns += elapsed_ns;
            stages.insert(
                stage.id().to_owned(),
                json!({"elapsed_ns": elapsed_ns, "calls": sample.calls}),
            );
        }
        let elapsed_ns = timing.end_to_end.as_nanos() as u64;
        stages.insert(
            "end_to_end".to_owned(),
            json!({"elapsed_ns": elapsed_ns, "calls": 1}),
        );
        let mut metrics = json!({
            "elapsed_ns": elapsed_ns,
            "unattributed_ns": elapsed_ns - accounted_ns,
            "stages": stages,
        });
        if trace {
            metrics["trace"] = Value::Array(timing.trace.iter().map(|interval| {
                json!({"name": interval.stage.id(), "offset_ns": interval.offset.as_nanos() as u64,
                    "duration_ns": interval.elapsed.as_nanos() as u64})
            }).collect());
        }
        Ok(metrics)
    })
}
