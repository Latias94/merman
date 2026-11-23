use std::time::Instant;

#[path = "workloads/mod.rs"]
mod workloads;

fn main() -> Result<(), String> {
    workloads::run("clock", |trial| {
        let start = Instant::now();
        trial.batch()?;
        let elapsed = start.elapsed();
        trial.release();
        Ok(serde_json::json!({"elapsed_ns": elapsed.as_nanos() as u64}))
    })
}
