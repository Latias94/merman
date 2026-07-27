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
        for command in [
            "render",
            "batch",
            "mmdc",
            "lint",
            "fix",
            "capabilities",
            "detect",
            "parse",
        ] {
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
    assert!(
        output.stderr.is_empty(),
        "explicit version output must stay on stdout: {:?}",
        output.stderr
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "unexpected version output:\n{stdout}"
    );

    for arg in ["--version", "-V"] {
        let output = Command::new(exe)
            .args(["mmdc", arg])
            .output()
            .expect("run mmdc version");
        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(
            stdout.contains(env!("CARGO_PKG_VERSION")),
            "mmdc compatibility command should inherit the Merman version:\n{stdout}"
        );
    }
}

#[test]
fn root_help_groups_commands_by_user_task() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe).arg("--help").output().expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "explicit help must stay on stdout: {:?}",
        output.stderr
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    for heading in [
        "Native rendering:",
        "Analysis:",
        "Compatibility:",
        "Capabilities and tooling:",
    ] {
        assert!(
            stdout.contains(heading),
            "root help should group commands under `{heading}`:\n{stdout}"
        );
    }
    for flag in ["--input", "--output", "--outputFormat", "--pdfFit"] {
        assert!(
            !stdout.contains(flag),
            "root help must not advertise command-owned option {flag}:\n{stdout}"
        );
    }
}

#[test]
fn mmdc_help_owns_the_pinned_compatibility_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["mmdc", "--help"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "explicit help must stay on stdout: {:?}",
        output.stderr
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    for flag in [
        "--input",
        "--output",
        "--outputFormat",
        "--configFile",
        "--cssFile",
        "--pdfFit",
        "--iconPacks",
        "--iconPacksNamesAndUrls",
    ] {
        assert!(
            stdout.contains(flag),
            "mmdc help should expose compatibility option {flag}:\n{stdout}"
        );
    }
}

#[test]
fn mmdc_theme_values_match_the_pinned_upstream_contract() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    for rejected in ["base", "not-an-upstream-theme"] {
        let output = Command::new(exe)
            .args(["mmdc", "-i", "-", "-o", "-", "--theme", rejected])
            .output()
            .expect("run cli");

        assert_eq!(support::exit_code(output.status), 2);
        assert!(output.stdout.is_empty(), "failure must not write a payload");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        for theme in ["default", "forest", "dark", "neutral"] {
            assert!(
                stderr.contains(theme),
                "theme validation should list `{theme}`:\n{stderr}"
            );
        }
        assert!(
            stderr.contains(rejected),
            "theme validation should identify the rejected value:\n{stderr}"
        );
    }
}

#[test]
fn native_render_rejects_options_for_a_different_output_kind() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = support::run_with_stdin_in_dir(
        &[
            "render",
            "--format",
            "svg",
            "--pdf-filter-scale",
            "2",
            "missing.mmd",
        ],
        "",
        Some(tmp.path()),
    );

    assert_eq!(support::exit_code(output.status), 2);
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--pdf-filter-scale") && stderr.contains("svg"),
        "format-specific validation should explain the conflict before opening input:\n{stderr}"
    );
    assert!(
        !stderr.contains("missing.mmd"),
        "argument validation must precede input acquisition:\n{stderr}"
    );
}

#[test]
fn native_text_render_rejects_svg_and_network_only_options() {
    for args in [
        [
            "render",
            "missing.mmd",
            "--format",
            "ascii",
            "--text-measurer",
            "deterministic",
        ]
        .as_slice(),
        [
            "render",
            "missing.mmd",
            "--format",
            "ascii",
            "--allow-network",
        ]
        .as_slice(),
    ] {
        let output = run_with_stdin(args, "");
        assert_eq!(support::exit_code(output.status), 2, "args: {args:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            !stderr.contains("missing.mmd"),
            "irrelevant option rejection must precede input acquisition: {args:?}\n{stderr}"
        );
    }
}

#[test]
fn analysis_commands_reject_irrelevant_output_and_input_options() {
    for args in [
        ["lint", "missing.mmd", "--format", "text", "--pretty"].as_slice(),
        ["lint", "missing.mmd", "--stdin-file-name", "stdin.mmd"].as_slice(),
        ["lint-rules", "--format", "text", "--pretty"].as_slice(),
    ] {
        let output = run_with_stdin(args, "");
        assert_eq!(support::exit_code(output.status), 2, "args: {args:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            !stderr.contains("doesn't exist") && !stderr.contains("I/O error"),
            "argument rejection must precede input acquisition: {args:?}\n{stderr}"
        );
    }
}

#[test]
fn mmdc_accepts_jobs_as_an_upstream_single_render_noop() {
    let output = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "-", "--jobs", "2"],
        "flowchart LR\nA-->B\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.trim_start().starts_with("<svg"), "{stdout}");
}

#[test]
fn native_render_rejects_markdown_inputs_in_favor_of_batch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join("input.md"),
        "```mermaid\nflowchart LR\nA-->B\n```\n",
    )
    .expect("write Markdown");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["render", "input.md", "--output", "out.svg"])
        .output()
        .expect("run cli");

    assert_eq!(support::exit_code(output.status), 2);
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("batch") && stderr.to_ascii_lowercase().contains("markdown"),
        "native single rendering should direct Markdown users to batch:\n{stderr}"
    );
    assert!(!tmp.path().join("out.svg").exists());
}

#[test]
fn developer_commands_reject_render_only_options() {
    for args in [
        ["detect", "--suppress-errors", "-"].as_slice(),
        ["layout", "--svg-id", "diagram", "-"].as_slice(),
        ["layout", "--hand-drawn-seed", "7", "-"].as_slice(),
    ] {
        let output = run_with_stdin(args, "flowchart LR\nA-->B\n");
        assert_eq!(
            support::exit_code(output.status),
            2,
            "irrelevant options must be usage errors: {args:?}"
        );
        assert!(
            output.stdout.is_empty(),
            "argument rejection must precede reading stdin: {args:?}"
        );
    }
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
            "mmdc",
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
        .args(["mmdc", "-i", "input.mmd", "-q"])
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
