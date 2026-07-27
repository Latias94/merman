mod support;

use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::process::Command;
use support::{repo_root, run_with_stdin};

fn task_by_id<'a>(model: &'a Value, id: &str) -> &'a Value {
    model["tasks"]
        .as_array()
        .expect("gantt tasks should be an array")
        .iter()
        .find(|task| task["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("missing Gantt task {id} in {model}"))
}

#[test]
fn compiled_help_and_version_are_available() {
    let exe = assert_cmd::cargo_bin!("merman-cli");

    for arg in ["--help", "-h"] {
        let output = Command::new(exe).arg(arg).output().expect("run cli");
        assert!(output.status.success(), "stderr: {:?}", output.stderr);

        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("Usage:"), "unexpected help:\n{stdout}");
        for command in ["capabilities", "detect", "parse", "render"] {
            assert!(
                stdout.contains(command),
                "compiled help should expose {command}:\n{stdout}"
            );
        }
    }

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
fn compiled_capabilities_match_the_full_test_artifact() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["capabilities", "--json"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr: {:?}",
        output.stderr
    );
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("capabilities should be JSON");
    assert_eq!(payload["schema_version"], 1);
    assert_eq!(payload["target"], "native");
    assert!(
        payload["descriptor_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:")),
        "missing descriptor digest: {payload}"
    );

    let capabilities = payload["capabilities"]
        .as_array()
        .expect("capabilities should be an array");
    for id in [
        "analysis",
        "ascii",
        "icons",
        "jpeg",
        "layout-cytoscape",
        "layout-elk",
        "markdown",
        "math",
        "network-icons",
        "parallel-markdown",
        "pdf",
        "png",
        "shell-completions",
        "svg",
        "system-clock",
        "system-random",
        "system-timezone",
        "system-timing",
    ] {
        assert!(
            capabilities
                .iter()
                .any(|capability| capability["id"].as_str() == Some(id)),
            "missing compiled capability {id}: {payload}"
        );
    }
}

#[test]
fn compiled_system_adapters_do_not_change_the_deterministic_default() {
    let output = run_with_stdin(
        &["parse", "-"],
        "gantt\n\
         dateFormat YYYY-MM-DD\n\
         section Demo\n\
         Missing ref: id1,after missing,1d\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let model: Value = serde_json::from_slice(&output.stdout).expect("parse stdout should be JSON");
    assert_eq!(
        task_by_id(&model, "id1")["startTime"].as_i64(),
        Some(0),
        "compiling system adapters must not change the deterministic CLI default"
    );
}

#[test]
fn compatibility_export_renders_a_svg_file() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let output = tmp.path().join("out.svg");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "-i",
            fixture.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let svg = fs::read_to_string(output).expect("read svg");
    assert!(svg.trim_start().starts_with("<svg"), "output is not SVG");
    assert!(svg.contains("flowchart"), "expected rendered flowchart SVG");
}

#[test]
fn compatibility_default_output_appends_svg_to_the_input_name() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("input.mmd"), "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-q"])
        .assert()
        .success();

    let output = tmp.path().join("input.mmd.svg");
    assert!(
        output.exists(),
        "default mmdc output should append .svg to the input path"
    );
    let svg = fs::read_to_string(output).expect("read svg");
    assert!(svg.trim_start().starts_with("<svg"));
}

#[test]
fn developer_commands_keep_payload_only_stdout() {
    let output = run_with_stdin(&["detect", "-"], "sequenceDiagram\nA->>B: Hello\n");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr: {:?}",
        output.stderr
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert_eq!(stdout.trim(), "sequence");
}
