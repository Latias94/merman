use assert_cmd::prelude::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
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
        .expect("spawn cli");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait cli")
}

#[test]
fn cli_prints_help_successfully() {
    let exe = assert_cmd::cargo_bin!("merman-cli");

    for arg in ["--help", "-h"] {
        let output = Command::new(&exe).arg(arg).output().expect("run cli");

        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("Usage:"), "unexpected help:\n{stdout}");
        assert!(
            stdout.contains("-i, --input"),
            "help should include mmdc-compatible input flag:\n{stdout}"
        );
    }
}

#[test]
fn cli_prints_version_successfully() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .arg("--version")
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "unexpected version output:\n{stdout}"
    );
}

#[test]
fn cli_rejects_non_positive_numeric_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["-i", "-", "-o", "-", "--scale", "0"])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "expected --scale 0 to be rejected"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("expected a positive number"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn top_level_mmdc_flags_render_svg_file() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.svg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "-i",
            fixture.to_string_lossy().as_ref(),
            "-o",
            out.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let svg = fs::read_to_string(&out).expect("read svg");
    assert!(svg.trim_start().starts_with("<svg"), "output is not SVG");
    assert!(svg.contains("flowchart"), "expected rendered flowchart SVG");
}

#[test]
fn top_level_output_dash_writes_to_stdout() {
    let output = run_with_stdin(
        &["-i", "-", "-o", "-"],
        "flowchart LR\nA[Start] --> B[Done]\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.trim_start().starts_with("<svg"),
        "expected SVG on stdout:\n{stdout}"
    );

    let dash_file = repo_root().join("-");
    assert!(
        !dash_file.exists(),
        "stdout output must not create a file named '-'"
    );
}

#[test]
fn top_level_infers_png_from_output_extension() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.png");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "-i",
            fixture.to_string_lossy().as_ref(),
            "-o",
            out.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read png");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "output is not a PNG"
    );
}

#[test]
fn developer_subcommands_still_work() {
    let output = run_with_stdin(&["detect", "-"], "sequenceDiagram\nA->>B: Hello\n");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert_eq!(stdout.trim(), "sequence");
}
