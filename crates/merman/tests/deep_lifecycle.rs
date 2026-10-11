#![cfg(feature = "diagram-flowchart")]

use merman::{OperationControl, ParseOptions, Renderer};
use std::fs::{self, File};
use std::io::{self, Write as _};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("diagnostic timestamp follows the Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "merman-facade-deep-lifecycle-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap_or_else(|error| {
        panic!(
            "create facade child diagnostic directory {}: {error}",
            directory.display()
        )
    });
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

fn nested_source(family: &str, depth: usize, repeated: bool) -> String {
    use std::fmt::Write as _;
    let (header, closer, leaf) = match family {
        "flowchart" => ("flowchart TD\n", "end\n", "leaf[Leaf]\n"),
        "state" => ("stateDiagram-v2\n", "}\n", "leaf\n"),
        "block" => ("block-beta\n", "end\n", "leaf\n"),
        _ => unreachable!(),
    };
    let mut source = header.to_owned();
    for index in 0..depth {
        let id = if repeated {
            "same".to_owned()
        } else {
            format!("n{index}")
        };
        match family {
            "flowchart" => writeln!(source, "subgraph {id}").unwrap(),
            "state" => writeln!(source, "state {id} {{").unwrap(),
            "block" => writeln!(source, "block:{id}").unwrap(),
            _ => unreachable!(),
        }
    }
    source.push_str(leaf);
    source.push_str(&closer.repeat(depth));
    source
}

#[test]
fn integrated_facade_lifecycle_child() {
    let Ok(case) = std::env::var("MERMAN_INTEGRATED_FACADE_LIFECYCLE") else {
        return;
    };
    std::thread::Builder::new()
        .stack_size(WORKER_STACK_BYTES)
        .spawn(move || {
            let started = Instant::now();
            println!("case={case} path=maintained-facade worker_stack_bytes={WORKER_STACK_BYTES}");
            io::stdout().flush().unwrap();
            if let Some(family) = case.strip_prefix("budget-") {
                let source = nested_source(family, 3_000, false);
                println!("source_bytes={} depth=3000", source.len());
                marker(started, "model", "begin");
                let direct = merman::Engine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                let expected = merman::resources::InputResourcePolicy::default()
                    .check_render_model(direct.model())
                    .unwrap_err();
                drop(direct);
                marker(started, "model", "done");
                let control = OperationControl::new();
                let renderer = Renderer::new().with_parse_options(ParseOptions::lenient());
                let error = renderer
                    .prepare_semantic(&source, control.clone())
                    .unwrap_err();
                let merman::RenderError::ResourceLimitExceeded(limit) = &error else {
                    panic!("expected resource terminal with suppression: {error:?}");
                };
                assert_eq!(limit.id, expected.limit);
                assert_eq!(limit.actual, expected.actual as u64);
                assert_eq!(limit.maximum, expected.max as u64);
                assert_eq!(limit.phase, "layout_model");
                assert!(limit.provenance.is_some());
                control.cancel();
                let replayed = renderer
                    .prepare_semantic("flowchart TD\na-->b", control)
                    .unwrap_err();
                let merman::RenderError::ResourceLimitExceeded(replayed) = replayed else {
                    panic!("the earlier resource terminal must remain sticky");
                };
                assert_eq!(&replayed, limit);
                marker(started, "budget-terminal", "done");
            } else if case == "repeated-id-targets" {
                let source = nested_source("flowchart", 3_000, true);
                let renderer = Renderer::new().with_parse_options(ParseOptions::strict());
                let direct = merman::Engine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                let merman::RenderSemanticModel::Flowchart(model) = direct.model() else {
                    panic!("Flowchart model");
                };
                assert!(
                    merman::resources::FlowchartComplexity::from_model(model).subgraph_depth <= 1
                );
                drop(direct);
                let artifact = renderer
                    .prepare_semantic(&source, OperationControl::new())
                    .unwrap()
                    .unwrap();
                let json = artifact.compatibility_json().unwrap();
                json.write_json(io::sink()).unwrap();
                drop(json.clone());
                drop(json);
                drop(artifact);
                #[cfg(feature = "svg")]
                {
                    let output = renderer
                        .render(merman::RenderRequest::svg(
                            &source,
                            OperationControl::new(),
                            merman::SvgRequest::default(),
                        ))
                        .unwrap();
                    assert!(matches!(output, merman::RenderOutput::Svg(Some(_))));
                    drop(output);
                }
                #[cfg(feature = "ascii")]
                {
                    let output = renderer
                        .render(merman::RenderRequest::ascii(
                            &source,
                            OperationControl::new(),
                            merman::AsciiRequest::default(),
                        ))
                        .unwrap();
                    assert!(matches!(output, merman::RenderOutput::Ascii(Some(_))));
                    drop(output);
                }
                marker(started, "deep-supported-targets", "done");
            } else if case == "flat-families" {
                for source in [
                    "architecture-beta\n  service api(server)[API]\n",
                    "agentflow-beta\nA --> B\n",
                    "kanban\n  Todo\n    item1\n",
                    "sequenceDiagram\nAlice->>Bob: Hello\n",
                    "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2024-01-01, 1d\n",
                ] {
                    let artifact = Renderer::new()
                        .with_parse_options(ParseOptions::strict())
                        .prepare_semantic(source, OperationControl::new())
                        .unwrap()
                        .unwrap();
                    let json = artifact.compatibility_json().unwrap();
                    json.write_json(io::sink()).unwrap();
                    drop(json.clone());
                    drop(json);
                    drop(artifact);
                }
                marker(started, "retained-flat-families", "done");
            } else {
                panic!("unknown integrated facade case");
            }
            let small = Renderer::new()
                .prepare_semantic("flowchart TD\na-->b\n", OperationControl::new())
                .unwrap()
                .unwrap();
            drop(small);
            marker(started, "after-small-operation", "done");
        })
        .unwrap()
        .join()
        .unwrap();
}

fn run_integrated_facade_case(case: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("diagnostic timestamp follows the Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "merman-integrated-facade-{}-{case}-{timestamp}",
        std::process::id(),
    ));
    fs::create_dir(&directory).unwrap_or_else(|error| {
        panic!(
            "create integrated facade diagnostic directory {}: {error}",
            directory.display()
        )
    });
    let stdout_path = directory.join("stdout.txt");
    let stderr_path = directory.join("stderr.txt");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "integrated_facade_lifecycle_child",
            "--nocapture",
        ])
        .env("MERMAN_INTEGRATED_FACADE_LIFECYCLE", case)
        .stdout(Stdio::from(File::create(&stdout_path).unwrap()))
        .stderr(Stdio::from(File::create(&stderr_path).unwrap()))
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < CHILD_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(10))
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "integrated facade timeout/error {result:?}: {}",
                    directory.display()
                );
            }
        }
    };
    let stdout = fs::read_to_string(stdout_path).unwrap();
    let stderr = fs::read_to_string(stderr_path).unwrap();
    assert!(
        status.success(),
        "{case} status={status}; diagnostics={}; stdout={stdout}; stderr={stderr}",
        directory.display()
    );
    assert!(stdout.contains("phase=after-small-operation status=done"));
}

#[test]
fn deep_flowchart_budget_rejection_preserves_terminal_and_cleanup() {
    run_integrated_facade_case("budget-flowchart");
}

#[cfg(feature = "diagram-state")]
#[test]
fn deep_state_budget_rejection_preserves_terminal_and_cleanup() {
    run_integrated_facade_case("budget-state");
}

#[cfg(feature = "diagram-block")]
#[test]
fn deep_block_budget_rejection_preserves_terminal_and_cleanup() {
    run_integrated_facade_case("budget-block");
}

#[test]
fn deep_repeated_ids_keep_canonical_depth_and_supported_targets() {
    run_integrated_facade_case("repeated-id-targets");
}

#[cfg(feature = "all-diagrams")]
#[test]
fn representative_flat_families_keep_the_managed_lifecycle() {
    run_integrated_facade_case("flat-families");
}
