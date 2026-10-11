#![cfg(all(feature = "svg", feature = "all-diagrams"))]

use merman::svg::RenderResourcePolicy;
use merman::{
    OperationControl, ParseOptions, RenderOutput, RenderTarget, Renderer, SvgEnvironment,
    SvgRequest,
};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Write as _};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CASE_ENV: &str = "MERMAN_FACADE_BOUNDED_LIFECYCLE_CASE";
const WORKER_STACK_BYTES: usize = 2 * 1024 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(60);
const LOCAL_DEPTH: usize = 256;

fn source(case: &str) -> String {
    match case {
        "railroad" => format!(
            "railroad-beta\nentry = {}terminal(\"a\"){} ;\n",
            "optional(".repeat(LOCAL_DEPTH),
            ")".repeat(LOCAL_DEPTH)
        ),
        "railroadEbnf" => format!(
            "railroad-ebnf-beta\nentry = \"a\"{} ;\n",
            "?".repeat(LOCAL_DEPTH)
        ),
        "railroadAbnf" => format!(
            "railroad-abnf-beta\nentry = {}\"a\"{} ;\n",
            "[".repeat(LOCAL_DEPTH),
            "]".repeat(LOCAL_DEPTH)
        ),
        "railroadPeg" => format!(
            "railroad-peg-beta\nentry <- {}\"a\"{} ;\n",
            "(".repeat(LOCAL_DEPTH),
            ")?".repeat(LOCAL_DEPTH)
        ),
        "zenuml" => format!(
            "zenuml\n{}A.call()\n{}",
            "opt {\n".repeat(LOCAL_DEPTH),
            "}\n".repeat(LOCAL_DEPTH)
        ),
        "treeView" => {
            let mut source = String::from("treeView-beta\n");
            for depth in 0..LOCAL_DEPTH {
                writeln!(source, "{}\"n{depth}\"", " ".repeat(depth)).unwrap();
            }
            source
        }
        "usecase" => format!(
            "usecase-beta\njson Data@{{\"value\":{}1{}}}\n",
            "[".repeat(126),
            "]".repeat(126)
        ),
        _ => panic!("unknown bounded facade case {case}"),
    }
}

fn marker(phase: &str, status: &str) {
    println!("phase={phase} status={status}");
    io::stdout().flush().expect("flush bounded facade marker");
}

fn run_worker(case: &str) {
    let source = source(case);
    println!(
        "case={case} source_bytes={} worker_stack_bytes={WORKER_STACK_BYTES} path=Renderer.prepare_semantic+SemanticArtifact.render profile=unbounded-for-trusted-input build_mode={}",
        source.len(),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let renderer = Renderer::new()
        .with_parse_options(ParseOptions::strict())
        .with_resource_policy(*policy.input_policy());
    marker("parse", "begin");
    let semantic = renderer
        .prepare_semantic(&source, OperationControl::new())
        .expect("accepted bounded source prepares through the facade")
        .unwrap();
    assert_eq!(semantic.diagram_type(), case);
    marker("parse", "done");
    marker("export", "begin");
    let json = semantic
        .compatibility_json()
        .expect("accepted bounded facade projection");
    let mut encoded = Vec::new();
    json.write_json(&mut encoded)
        .expect("bounded facade iterative export");
    assert!(!encoded.is_empty());
    let cloned = json.clone();
    assert_eq!(cloned, json);
    drop(cloned);
    drop(json);
    marker("export", "done");
    marker("render", "begin");
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic().with_resource_policy(policy),
        ..Default::default()
    };
    let output = semantic
        .render(RenderTarget::Svg(request))
        .expect("maximum accepted bounded model renders native SVG");
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("bounded family supports native SVG")
    };
    assert!(svg.svg().contains("<svg"));
    println!("svg_bytes={}", svg.svg().len());
    marker("render", "done");
    drop(svg);
    marker("drop", "done");
    drop(
        renderer
            .prepare_semantic(
                "railroad-beta\nafter = terminal(\"ok\") ;\n",
                OperationControl::new(),
            )
            .unwrap()
            .unwrap(),
    );
    marker("after-small-operation", "done");
}

fn run_child(case: &str) {
    let started = Instant::now();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("diagnostic timestamp follows the Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "merman-facade-bounded-{}-{case}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap_or_else(|error| {
        panic!(
            "create bounded facade diagnostic directory {}: {error}",
            directory.display()
        )
    });
    let stdout_path = directory.join("stdout.txt");
    let stderr_path = directory.join("stderr.txt");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "bounded_facade_lifecycle_child", "--nocapture"])
        .env(CASE_ENV, case)
        .stdout(Stdio::from(File::create(&stdout_path).unwrap()))
        .stderr(Stdio::from(File::create(&stderr_path).unwrap()))
        .spawn()
        .expect("spawn bounded facade lifecycle child");
    let (status, timed_out, process_error) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status, false, None),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                break (
                    child.wait().expect("reap failed facade child"),
                    false,
                    Some(error.to_string()),
                );
            }
        }
        if started.elapsed() >= CHILD_TIMEOUT {
            let error = child.kill().err().map(|error| error.to_string());
            break (
                child.wait().expect("reap timed-out facade child"),
                true,
                error,
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = fs::read_to_string(&stdout_path).unwrap();
    let stderr = fs::read_to_string(&stderr_path).unwrap();
    let summary = format!(
        "case={case} status={status} timed_out={timed_out} elapsed_ms={} process_error={process_error:?}",
        started.elapsed().as_millis()
    );
    fs::write(directory.join("status.txt"), &summary).unwrap();
    println!(
        "{summary}\ndiagnostics={}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        directory.display()
    );
    assert!(!timed_out, "bounded facade child timed out: {summary}");
    assert!(
        process_error.is_none(),
        "bounded facade observation failed: {summary}"
    );
    assert!(status.success(), "bounded facade child failed: {summary}");
    assert!(stdout.contains("phase=render status=done"));
    assert!(stdout.contains("phase=drop status=done"));
    assert!(stdout.contains("phase=after-small-operation status=done"));
}

#[test]
fn bounded_facade_lifecycle_child() {
    let Ok(case) = std::env::var(CASE_ENV) else {
        return;
    };
    std::thread::Builder::new()
        .name(format!("bounded-facade-{case}"))
        .stack_size(WORKER_STACK_BYTES)
        .spawn(move || run_worker(&case))
        .expect("spawn 2 MiB bounded facade worker")
        .join()
        .expect("bounded facade worker completes");
}

#[test]
fn railroad_boundary_native_svg_lifecycle_on_host_stack() {
    for case in ["railroad", "railroadEbnf", "railroadAbnf", "railroadPeg"] {
        run_child(case);
    }
}

#[test]
fn zenuml_boundary_native_svg_lifecycle_on_host_stack() {
    run_child("zenuml");
}

#[test]
fn tree_view_boundary_native_svg_lifecycle_on_host_stack() {
    run_child("treeView");
}

#[test]
fn usecase_json_boundary_native_svg_lifecycle_on_host_stack() {
    run_child("usecase");
}
