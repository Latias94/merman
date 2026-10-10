#![cfg(feature = "diagram-flowchart")]

use merman::{OperationControl, ParseOptions, Renderer};
use std::fs::{self, File};
use std::io::{self, Write as _};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "MERMAN_FACADE_DEEP_LIFECYCLE_CHILD";
const WORKER_STACK_BYTES: usize = 2 * 1024 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(60);
const SOURCE: &str = "flowchart TD\nsubgraph outer\nsubgraph inner\nleaf[Leaf]\nend\nend\n";

fn marker(started: Instant, phase: &str, status: &str) {
    println!(
        "phase={phase} status={status} worker_elapsed_ms={}",
        started.elapsed().as_millis()
    );
    io::stdout().flush().expect("flush facade lifecycle marker");
}

#[test]
fn facade_lifecycle_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    std::thread::Builder::new()
        .name("facade-lifecycle".to_owned())
        .stack_size(WORKER_STACK_BYTES)
        .spawn(|| {
            let started = Instant::now();
            println!(
                "case=facade-semantic-smoke depth=2 source_bytes={} \
                 path=Renderer.prepare_semantic profile=default-input-policy-strict \
                 worker_stack_bytes={WORKER_STACK_BYTES} build_mode={}",
                SOURCE.len(),
                if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }
            );
            io::stdout().flush().expect("flush facade case metadata");
            let renderer = Renderer::new().with_parse_options(ParseOptions::strict());
            marker(started, "parse", "begin");
            let artifact = renderer
                .prepare_semantic(SOURCE, OperationControl::new())
                .expect("maintained facade prepares semantics")
                .expect("nested flowchart is detected");
            marker(started, "parse", "done");
            assert_eq!(artifact.diagram_type(), "flowchart-v2");
            marker(started, "model", "done");
            marker(started, "export", "begin");
            let json = artifact
                .compatibility_json()
                .expect("small compatibility projection");
            let encoded = serde_json::to_vec(&json).expect("small compatibility export");
            assert!(!encoded.is_empty());
            marker(started, "export", "done");
            marker(started, "clone", "begin");
            let cloned = json.clone();
            marker(started, "clone", "done");
            marker(started, "drop", "begin");
            drop(cloned);
            drop(json);
            drop(encoded);
            drop(artifact);
            marker(started, "drop", "done");
            marker(started, "after-small-operation", "begin");
            let small = renderer
                .prepare_semantic("flowchart TD\na --> b\n", OperationControl::new())
                .expect("facade remains usable")
                .expect("small flowchart is detected");
            drop(small);
            marker(started, "after-small-operation", "done");
        })
        .expect("spawn 2 MiB facade worker")
        .join()
        .expect("facade lifecycle worker completes");
}

#[test]
fn small_facade_semantic_lifecycle_completes_on_host_stack() {
    let started = Instant::now();
    let directory = std::env::temp_dir().join(format!(
        "merman-facade-deep-lifecycle-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create facade child diagnostic directory");
    let stdout_path = directory.join("stdout.txt");
    let stderr_path = directory.join("stderr.txt");
    let mut child = Command::new(std::env::current_exe().expect("current test executable"))
        .args(["--exact", "facade_lifecycle_child", "--nocapture"])
        .env(CHILD_ENV, "semantic-smoke")
        .stdout(Stdio::from(
            File::create(&stdout_path).expect("create child stdout file"),
        ))
        .stderr(Stdio::from(
            File::create(&stderr_path).expect("create child stderr file"),
        ))
        .spawn()
        .expect("spawn facade lifecycle child");
    let (status, timed_out, process_error) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status, false, None),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let status = child.wait().expect("reap facade child after wait failure");
                break (status, false, Some(error.to_string()));
            }
        }
        if started.elapsed() >= CHILD_TIMEOUT {
            let kill_error = child.kill().err().map(|error| error.to_string());
            let status = child.wait().expect("reap timed-out facade child");
            break (status, true, kill_error);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = fs::read_to_string(&stdout_path).expect("read facade child stdout");
    let stderr = fs::read_to_string(&stderr_path).expect("read facade child stderr");
    let summary = format!(
        "case=facade-semantic-smoke status={status} timed_out={timed_out} \
         parent_elapsed_ms={} timeout_ms={} process_error={process_error:?}",
        started.elapsed().as_millis(),
        CHILD_TIMEOUT.as_millis()
    );
    fs::write(directory.join("status.txt"), &summary).expect("preserve facade child exit status");
    println!(
        "{summary}\ndiagnostics={}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        directory.display()
    );
    assert!(
        !timed_out,
        "facade child timed out; diagnostics={}",
        directory.display()
    );
    assert!(
        process_error.is_none(),
        "facade child process observation failed; {summary}"
    );
    assert!(
        status.success(),
        "facade child failed; diagnostics={}",
        directory.display()
    );
    assert!(
        stdout.contains("phase=export status=done"),
        "small facade export completes"
    );
    assert!(
        stdout.contains("phase=drop status=done"),
        "facade child completes ordinary disposal"
    );
    assert!(
        stdout.contains("phase=after-small-operation status=done"),
        "facade child completes a subsequent small operation"
    );
}
