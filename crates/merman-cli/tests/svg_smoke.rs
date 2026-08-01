#![cfg(feature = "svg")]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
#[cfg(feature = "parallel-markdown")]
use std::time::{Duration, Instant};

const SOURCE: &str = "flowchart TD\nA[Start] --> B{Ready?}\nB -->|Yes| C[Ship]\n";

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for CLI")
}

#[test]
fn cli_renders_svg_from_stdin_to_stdout() {
    let output = run_with_stdin(&["render", "--format", "svg", "-"], SOURCE);

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = String::from_utf8(output.stdout).expect("SVG should be UTF-8");
    assert!(svg.trim_start().starts_with("<svg"), "{svg}");
    assert!(svg.contains("<foreignObject"), "{svg}");
}

#[test]
fn cli_renders_named_input_to_a_sibling_svg_by_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("diagram.mmd");
    fs::write(&input, SOURCE).expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(repo_root())
        .args(["render", input.to_string_lossy().as_ref()])
        .output()
        .expect("run CLI");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = fs::read_to_string(input.with_extension("svg")).expect("read sibling SVG");
    assert!(svg.trim_start().starts_with("<svg"), "{svg}");
}

#[test]
fn cli_selects_the_resvg_safe_svg_pipeline() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "svg",
            "--svg-pipeline",
            "resvg-safe",
            "-",
        ],
        SOURCE,
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = String::from_utf8(output.stdout).expect("SVG should be UTF-8");
    assert!(!svg.contains("<foreignObject"), "{svg}");
    assert!(
        svg.contains(r#"data-merman-foreignobject="fallback""#),
        "{svg}"
    );
}

#[test]
fn svg_only_help_does_not_offer_raw_svg_conversion() {
    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    {
        let exe = assert_cmd::cargo_bin!("merman-cli");
        let output = Command::new(exe)
            .args(["render", "--help"])
            .output()
            .expect("run CLI help");

        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let help = String::from_utf8(output.stdout).expect("help should be UTF-8");
        assert!(!help.contains("--input-kind"), "{help}");
    }
}

#[test]
fn working_set_is_rejected_before_the_svg_backend_runs() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "svg",
            "--resource-limit",
            "max_scheduling_weight_bytes=1",
            "-",
        ],
        "this is not Mermaid",
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("max_scheduling_weight_bytes"), "{stderr}");
    assert!(!stderr.contains("UnknownDiagram"), "{stderr}");
}

#[cfg(not(feature = "math"))]
#[test]
fn svg_only_build_does_not_advertise_ratex() {
    let output = run_with_stdin(
        &["render", "--math-renderer", "ratex", "--format", "svg", "-"],
        SOURCE,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("invalid value 'ratex'"), "{stderr}");
    assert!(stderr.contains("possible values: none"), "{stderr}");
}

#[cfg(feature = "parallel-markdown")]
#[test]
fn parallel_markdown_progresses_with_capacity_for_one_backend() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output_dir = tmp.path().join("rendered");
    fs::write(
        &input,
        "```mermaid\nflowchart LR\nA-->B\n```\n\n```mermaid\nflowchart LR\nB-->C\n```\n",
    )
    .expect("write Markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command.args([
        "batch",
        input.to_string_lossy().as_ref(),
        "--output-dir",
        output_dir.to_string_lossy().as_ref(),
        "--jobs",
        "2",
        "--resource-profile",
        "constrained",
        "--resource-limit",
        // The constrained SVG estimate fits once, but not twice, in 48 MiB.
        "max_scheduling_weight_bytes=50331648",
    ]);
    let output = run_with_timeout(command, Duration::from_secs(10));

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg_count = fs::read_dir(&output_dir)
        .expect("read output directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "svg"))
        .count();
    assert_eq!(svg_count, 2);
}

#[cfg(feature = "parallel-markdown")]
fn run_with_timeout(mut command: Command, timeout: Duration) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn CLI");
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().expect("poll CLI").is_some() {
            return child.wait_with_output().expect("collect CLI output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().expect("collect timed-out CLI");
            panic!("CLI did not finish within {timeout:?}: {:?}", output.stderr);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
