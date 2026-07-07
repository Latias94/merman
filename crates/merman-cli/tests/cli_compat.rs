use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread;

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    run_with_stdin_in_dir(args, input, None)
}

fn run_with_stdin_input(args: &[&str], stdin: &[u8]) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn cli");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(stdin)
        .expect("write stdin");

    child.wait_with_output().expect("wait cli")
}

fn run_with_stdin_in_dir(args: &[&str], input: &str, cwd: Option<&Path>) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command.spawn().expect("spawn cli");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait cli")
}

fn run_with_closed_stdout(args: &[&str], input: Option<&[u8]>) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }

    let mut child = command.spawn().expect("spawn cli");
    drop(child.stdout.take().expect("stdout pipe"));
    if let Some(input) = input {
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(input)
            .expect("write stdin");
        drop(child.stdin.take());
    }

    child.wait_with_output().expect("wait cli")
}

fn pdf_media_box(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let marker = text.find("/MediaBox")?;
    let after_marker = &text[marker..];
    let start = after_marker.find('[')?;
    let end = after_marker[start..].find(']')? + start;
    Some(
        after_marker[start + 1..end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn serve_icon_json_once(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http server");
    let addr = listener.local_addr().expect("local addr");
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    });
    format!("http://{addr}/icons.json")
}

#[cfg(unix)]
fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(-1)
}

#[cfg(windows)]
fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

fn task_by_id<'a>(model: &'a Value, id: &str) -> &'a Value {
    model["tasks"]
        .as_array()
        .expect("gantt tasks should be an array")
        .iter()
        .find(|task| task["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("missing Gantt task {id} in {model}"))
}

#[test]
fn cli_prints_help_successfully() {
    let exe = assert_cmd::cargo_bin!("merman-cli");

    for arg in ["--help", "-h"] {
        let output = Command::new(exe).arg(arg).output().expect("run cli");

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
fn cli_help_groups_top_level_surfaces() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe).arg("--help").output().expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    for heading in [
        "mmdc-compatible export:",
        "Markdown batch export:",
        "Raster and PDF export:",
        "Mermaid configuration:",
        "Rust renderer controls:",
        "Accepted browser compatibility flags:",
    ] {
        assert!(
            stdout.contains(heading),
            "help should include `{heading}` heading:\n{stdout}"
        );
    }

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
            "top-level help should include {flag}:\n{stdout}"
        );
    }
}

#[test]
fn render_help_excludes_top_level_only_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["render", "--help"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    for absent in [
        "--artefacts",
        "--artifacts",
        "--jobs",
        "--puppeteerConfigFile",
        "--pdfFit",
    ] {
        assert!(
            !stdout.contains(absent),
            "render help should not include top-level-only {absent}:\n{stdout}"
        );
    }

    for present in [
        "--output",
        "--format",
        "--cssFile",
        "--raster-max-width",
        "--iconPacks",
        "--sequence-mirror-actors",
    ] {
        assert!(
            stdout.contains(present),
            "render help should include direct rendering option {present}:\n{stdout}"
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

    for (flag, value, expected) in [
        ("--scale", "0", "expected a positive number"),
        ("--raster-fit-width", "0", "expected a positive integer"),
        ("--raster-fit-height", "0", "expected a positive integer"),
        ("--raster-max-width", "0", "expected a positive integer"),
        ("--raster-max-height", "0", "expected a positive integer"),
        ("--raster-max-pixels", "0", "expected a positive integer"),
    ] {
        let output = Command::new(exe)
            .args(["-i", "-", "-o", "-", flag, value])
            .output()
            .expect("run cli");

        assert!(
            !output.status.success(),
            "expected {flag} {value} to be rejected"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains(expected),
            "unexpected stderr for {flag} {value}:\n{stderr}"
        );
    }
}

#[test]
fn cli_rejects_non_positive_jobs() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["-i", "-", "-o", "-", "--jobs", "0"])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected --jobs 0 to be rejected");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("expected a positive integer"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn cli_rejects_invalid_fixed_time_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");

    for (flag, value, expected) in [
        (
            "--fixed-today",
            "2026/02/15",
            "expected a date in YYYY-MM-DD format",
        ),
        (
            "--fixed-local-offset-minutes",
            "1440",
            "expected a timezone offset in minutes between -1439 and 1439",
        ),
    ] {
        let output = Command::new(exe)
            .args(["parse", flag, value, "-"])
            .output()
            .expect("run cli");

        assert!(
            !output.status.success(),
            "expected {flag} {value} to be rejected"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains(expected),
            "unexpected stderr for {flag} {value}:\n{stderr}"
        );
    }
}

#[test]
fn cli_lint_valid_mermaid_returns_zero_and_json_payload() {
    let output = run_with_stdin(
        &["lint", "--format", "json", "-"],
        "flowchart TD\nA[Hello] --> B[World]\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["valid"], true);
    assert_eq!(payload["summary"]["errors"], 0);
    assert!(payload["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn cli_lint_rules_lists_rule_catalog_json() {
    let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .args(["lint-rules", "--format", "json"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let catalog: Value =
        serde_json::from_slice(&output.stdout).expect("lint-rules stdout should be JSON");
    assert_eq!(catalog["version"], 1);
    let rules = catalog["rules"]
        .as_array()
        .expect("rule catalog response should include a rules array");
    let authoring = rules
        .iter()
        .find(|rule| rule["id"] == "merman.authoring.flowchart.explicit_direction")
        .expect("authoring flowchart rule");

    assert_eq!(authoring["origin"], "merman_authoring");
    assert_eq!(authoring["default_profile"], "recommended");
    assert_eq!(authoring["default_severity"], "hint");
    assert_eq!(authoring["fixable"], true);
    assert!(
        authoring["evidence"]
            .as_array()
            .expect("evidence array")
            .iter()
            .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
    );
    let frontmatter = rules
        .iter()
        .find(|rule| rule["id"] == "merman.authoring.config.prefer_frontmatter_config")
        .expect("frontmatter config authoring rule");
    assert_eq!(frontmatter["origin"], "merman_authoring");
    assert_eq!(frontmatter["default_profile"], "recommended");
    assert_eq!(frontmatter["default_severity"], "hint");
    assert_eq!(frontmatter["fixable"], true);
    assert!(
        frontmatter["evidence"]
            .as_array()
            .expect("evidence array")
            .iter()
            .any(|value| value == "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md")
    );
    let external_loading = rules
        .iter()
        .find(|rule| {
            rule["id"] == "merman.compatibility.config.deprecated_external_diagram_loading"
        })
        .expect("deprecated external diagram loading rule");
    assert_eq!(external_loading["origin"], "mermaid_compatibility");
    assert_eq!(external_loading["default_profile"], "core");
    assert_eq!(external_loading["default_severity"], "warning");
    assert_eq!(external_loading["fixable"], false);
    assert!(
        external_loading["evidence"]
            .as_array()
            .expect("evidence array")
            .iter()
            .any(|value| value == "https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts")
    );
    let flowchart_html_labels = rules
        .iter()
        .find(|rule| rule["id"] == "merman.compatibility.config.deprecated_flowchart_html_labels")
        .expect("deprecated flowchart htmlLabels rule");
    assert_eq!(flowchart_html_labels["fixable"], true);
}

#[test]
fn cli_lint_rules_configurable_filter_excludes_internal_and_resource_rules() {
    let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .args(["lint-rules", "--format", "json", "--configurable"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let catalog: Value =
        serde_json::from_slice(&output.stdout).expect("lint-rules stdout should be JSON");
    assert_eq!(catalog["version"], 1);
    let rules = catalog["rules"]
        .as_array()
        .expect("rule catalog response should include a rules array");

    assert!(rules.iter().all(|rule| rule["category"] != "internal"
        && rule["category"] != "resource"
        && rule["configurable"] == true));
    assert!(
        rules
            .iter()
            .all(|rule| rule["id"] != "merman.resource.source_bytes_exceeded")
    );
}

#[test]
fn cli_lint_can_disable_rule_diagnostics() {
    let output = run_with_stdin(
        &[
            "lint",
            "--format",
            "json",
            "--lint-profile",
            "recommended",
            "--disable-rule",
            "merman.authoring.config.prefer_init_directive",
            "--disable-rule",
            "merman.authoring.config.prefer_frontmatter_config",
            "-",
        ],
        "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["valid"], true);
    assert_eq!(payload["summary"]["hints"], 0);
    assert!(payload["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn cli_lint_can_enable_authoring_rule_diagnostics() {
    let output = run_with_stdin(
        &[
            "lint",
            "--format",
            "json",
            "--enable-rule",
            "merman.authoring.config.prefer_init_directive",
            "-",
        ],
        "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["valid"], true);
    assert_eq!(payload["summary"]["hints"], 1);
    assert_eq!(
        payload["diagnostics"][0]["id"].as_str(),
        Some("merman.authoring.config.prefer_init_directive")
    );
}

#[test]
fn cli_lint_can_disable_no_diagram_rule() {
    let output = run_with_stdin(
        &[
            "lint",
            "--format",
            "json",
            "--disable-rule",
            "merman.parse.no_diagram",
            "-",
        ],
        "",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["valid"], true);
    assert_eq!(payload["summary"]["errors"], 0);
    assert!(payload["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn cli_lint_can_override_rule_severity() {
    let output = run_with_stdin(
        &[
            "lint",
            "--format",
            "json",
            "--lint-profile",
            "recommended",
            "--rule-severity",
            "merman.authoring.config.prefer_init_directive=warning",
            "--disable-rule",
            "merman.authoring.config.prefer_frontmatter_config",
            "-",
        ],
        "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["valid"], true);
    assert_eq!(payload["summary"]["hints"], 0);
    assert_eq!(payload["summary"]["warnings"], 1);
    assert_eq!(
        payload["diagnostics"][0]["id"].as_str(),
        Some("merman.authoring.config.prefer_init_directive")
    );
    assert_eq!(
        payload["diagnostics"][0]["severity"].as_str(),
        Some("warning")
    );
}

#[test]
fn cli_lint_rejects_unknown_rule_ids() {
    let exe = assert_cmd::cargo_bin!("merman-cli");

    for (args, expected) in [
        (
            vec![
                "lint",
                "--format",
                "json",
                "--disable-rule",
                "merman.unknown.rule",
                "-",
            ],
            "unknown or non-configurable lint rule id `merman.unknown.rule`",
        ),
        (
            vec![
                "lint",
                "--format",
                "json",
                "--rule-severity",
                "merman.internal.panic=warning",
                "-",
            ],
            "unknown or non-configurable lint rule id `merman.internal.panic`",
        ),
    ] {
        let output = Command::new(&exe)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn cli")
            .wait_with_output()
            .expect("wait cli");

        assert!(
            !output.status.success(),
            "expected lint args to be rejected"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(stderr.contains(expected), "unexpected stderr:\n{stderr}");
    }
}

#[test]
fn cli_lint_rejects_resource_limit_rule_configuration() {
    for args in [
        vec![
            "lint",
            "--format",
            "json",
            "--max-source-bytes",
            "8",
            "--disable-rule",
            "merman.resource.source_bytes_exceeded",
            "-",
        ],
        vec![
            "lint",
            "--format",
            "json",
            "--max-source-bytes",
            "8",
            "--rule-severity",
            "merman.resource.source_bytes_exceeded=hint",
            "-",
        ],
    ] {
        let output = run_with_stdin(&args, "flowchart TD\nA-->B\n");

        assert!(
            !output.status.success(),
            "expected resource rule config to be rejected"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains(
                "unknown or non-configurable lint rule id `merman.resource.source_bytes_exceeded`"
            ),
            "unexpected stderr:\n{stderr}"
        );
    }
}

#[test]
fn cli_lint_can_disable_block_warning_rules() {
    let output = run_with_stdin(
        &[
            "lint",
            "--format",
            "json",
            "--disable-rule",
            "merman.block.width_exceeds_columns",
            "-",
        ],
        "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["valid"], true);
    assert!(payload["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn cli_lint_can_override_block_warning_severity() {
    let output = run_with_stdin(
        &[
            "lint",
            "--format",
            "json",
            "--rule-severity",
            "merman.block.width_exceeds_columns=hint",
            "-",
        ],
        "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["valid"], true);
    assert_eq!(payload["summary"]["hints"], 2);
    assert_eq!(
        payload["diagnostics"][0]["id"].as_str(),
        Some("merman.block.width_exceeds_columns")
    );
    assert_eq!(payload["diagnostics"][0]["severity"].as_str(), Some("hint"));
}

#[test]
fn cli_lint_reports_markdown_fence_path_from_stdin_file_name() {
    let output = run_with_stdin_input(
        &[
            "lint",
            "--markdown",
            "--stdin-file-name",
            "notes.md",
            "--format",
            "text",
            "-",
        ],
        b"before\n```mermaid\nflowchart TD\nA -->\n```\nafter\n",
    );

    assert!(
        !output.status.success(),
        "lint should fail on invalid markdown"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("notes.md:4:6"),
        "unexpected lint output:\n{stdout}"
    );
    assert!(
        stdout.contains("merman.parse.diagram_parse"),
        "unexpected lint output:\n{stdout}"
    );
    assert!(
        stdout.contains("1 error(s)"),
        "unexpected lint summary:\n{stdout}"
    );
}

#[test]
fn cli_lint_reports_markdown_fence_failure_as_json_from_stdin_file_name() {
    let output = run_with_stdin_input(
        &[
            "lint",
            "--markdown",
            "--stdin-file-name",
            "notes.md",
            "--format",
            "json",
            "-",
        ],
        b"before\n```mermaid\nflowchart TD\nA -->\n```\nafter\n",
    );

    assert!(
        !output.status.success(),
        "lint should fail on invalid markdown"
    );
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("lint stdout should be JSON");
    assert_eq!(payload["valid"], false);
    assert_eq!(payload["source"]["path"], "notes.md");
    assert_eq!(payload["summary"]["errors"], 1);
    let diagnostic = &payload["diagnostics"][0];
    assert_eq!(diagnostic["id"], "merman.parse.diagram_parse");
    assert_eq!(diagnostic["span"]["line"], 4);
    assert_eq!(diagnostic["span"]["column"], 6);
}

#[test]
fn cli_parse_gantt_fixed_today_makes_missing_year_dates_deterministic() {
    let output = run_with_stdin(
        &[
            "parse",
            "--fixed-today",
            "2026-02-15",
            "--fixed-local-offset-minutes",
            "0",
            "-",
        ],
        r#"gantt
dateFormat MM-DD
section Demo
Missing year: id1,03-01,1d
Missing ref: id2,after missing,1d
"#,
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let model: Value = serde_json::from_slice(&output.stdout).expect("parse stdout should be JSON");

    assert_eq!(
        task_by_id(&model, "id1")["startTime"].as_i64(),
        Some(1_772_323_200_000),
        "MM-DD dates should use the fixed local year"
    );
    assert_eq!(
        task_by_id(&model, "id2")["startTime"].as_i64(),
        Some(1_771_113_600_000),
        "missing relative IDs should fall back to fixed local today"
    );
}

#[test]
fn cli_parse_gantt_fixed_local_offset_controls_local_midnight() {
    let output = run_with_stdin(
        &["parse", "--fixed-local-offset-minutes", "120", "-"],
        r#"gantt
dateFormat YYYY-MM-DD
section Demo
Shifted midnight: id1,2013-01-01,1d
"#,
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let model: Value = serde_json::from_slice(&output.stdout).expect("parse stdout should be JSON");

    assert_eq!(
        task_by_id(&model, "id1")["startTime"].as_i64(),
        Some(1_356_991_200_000),
        "UTC timestamp should reflect 2013-01-01T00:00 at +02:00"
    );
}

#[test]
fn top_level_gantt_fixed_today_is_carried_through_export_args() {
    let diagram = r#"gantt
dateFormat YYYY-MM-DD
section Demo
Anchor: id1,2026-01-01,1d
Missing ref: id2,after missing,1d
"#;
    let first = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "fixed-gantt",
            "--fixed-today",
            "2026-02-15",
            "--fixed-local-offset-minutes",
            "0",
        ],
        diagram,
    );
    let second = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "fixed-gantt",
            "--fixed-today",
            "2026-03-15",
            "--fixed-local-offset-minutes",
            "0",
        ],
        diagram,
    );

    assert!(first.status.success(), "stderr: {:?}", first.stderr);
    assert!(second.status.success(), "stderr: {:?}", second.stderr);
    assert!(
        String::from_utf8_lossy(&first.stdout)
            .trim_start()
            .starts_with("<svg")
    );
    assert!(
        String::from_utf8_lossy(&second.stdout)
            .trim_start()
            .starts_with("<svg")
    );
    assert_ne!(
        first.stdout, second.stdout,
        "top-level render must carry fixed Gantt time options through ExportArgs"
    );
}

#[test]
fn cli_rejects_conflicting_raster_unbounded_and_limits() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .stdin(Stdio::null())
        .args([
            "render",
            "--format",
            "png",
            "--raster-unbounded",
            "--raster-max-width",
            "128",
            "-",
        ])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "expected raster unbounded/max conflict to fail"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--raster-unbounded")
            && stderr.contains("--raster-max-width")
            && (stderr.contains("cannot be combined") || stderr.contains("cannot be used with")),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn completion_subcommand_generates_bash_script() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["completion", "bash"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("merman-cli") && stdout.contains("--input") && stdout.contains("render"),
        "unexpected completion output:\n{stdout}"
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
fn top_level_missing_input_file_reports_path() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "missing.mmd", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected missing input failure");
    assert_eq!(
        exit_code(output.status),
        2,
        "missing input should be usage/input error"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Input file \"missing.mmd\" doesn't exist"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!tmp.path().join("out.svg").exists());
}

#[test]
fn top_level_missing_output_directory_uses_output_exit_code() {
    let output = run_with_stdin(
        &["-i", "-", "-o", "missing-dir/out.svg"],
        "flowchart LR\nA-->B\n",
    );

    assert!(
        !output.status.success(),
        "expected missing output directory failure"
    );
    assert_eq!(
        exit_code(output.status),
        2,
        "invalid output path should use usage/output exit code"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Output directory") && stderr.contains("missing-dir"),
        "unexpected stderr:\n{stderr}"
    );
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
fn stdout_output_does_not_mix_non_error_logs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    fs::write(&input, "# No diagrams\n\nPlain text.\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.md", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stdout.is_empty(),
        "non-payload logs must not be written to stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("No mermaid charts found in Markdown input"),
        "diagnostic should be written to stderr:\n{stderr}"
    );
}

#[test]
fn stdout_broken_pipe_exits_success_without_diagnostic() {
    let output = run_with_closed_stdout(&["-i", "-", "-o", "-"], Some(b"flowchart LR\nA-->B\n"));
    assert!(
        output.status.success(),
        "broken stdout pipe should be treated as normal pipe termination: {:?}",
        output.stderr
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        !stderr.contains("I/O error") && !stderr.contains("Broken pipe"),
        "broken pipe should not print a generic diagnostic:\n{stderr}"
    );
}

#[test]
fn parse_stdout_broken_pipe_exits_success_without_panic() {
    let output = run_with_closed_stdout(&["parse", "-"], Some(b"flowchart LR\nA-->B\n"));
    assert!(
        output.status.success(),
        "parse broken stdout pipe should be treated as normal pipe termination: {:?}",
        output.stderr
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "broken pipe should not panic or print a diagnostic:\n{stderr}"
    );
}

#[test]
fn completion_stdout_broken_pipe_exits_success_without_panic() {
    let output = run_with_closed_stdout(&["completion", "bash"], None);
    assert!(
        output.status.success(),
        "completion broken stdout pipe should be treated as normal pipe termination: {:?}",
        output.stderr
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "broken pipe should not panic or print a diagnostic:\n{stderr}"
    );
}

#[test]
fn lint_rules_stdout_broken_pipe_exits_success_without_panic() {
    let output = run_with_closed_stdout(&["lint-rules"], None);
    assert!(
        output.status.success(),
        "lint-rules broken stdout pipe should be treated as normal pipe termination: {:?}",
        output.stderr
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "broken pipe should not panic or print a diagnostic:\n{stderr}"
    );
}

#[test]
fn lint_text_stdout_broken_pipe_exits_success_without_panic() {
    let output = run_with_closed_stdout(
        &["lint", "--markdown", "--format", "text", "-"],
        Some(b"before\n```mermaid\nflowchart TD\nA -->\n```\nafter\n"),
    );
    assert!(
        output.status.success(),
        "lint text broken stdout pipe should be treated as normal pipe termination: {:?}",
        output.stderr
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "broken pipe should not panic or print a diagnostic:\n{stderr}"
    );
}

#[test]
fn top_level_svg_pipeline_resvg_safe_outputs_export_safe_svg() {
    let diagram = "flowchart TD
A[Start] --> B{Is it working?}
B -->|Yes| C[Ship it]
B -->|No| D[Debug]
";
    let parity = run_with_stdin(&["-i", "-", "-o", "-"], diagram);
    let resvg_safe = run_with_stdin(
        &["-i", "-", "-o", "-", "--svg-pipeline", "resvg-safe"],
        diagram,
    );

    assert!(parity.status.success(), "stderr: {:?}", parity.stderr);
    assert!(
        resvg_safe.status.success(),
        "stderr: {:?}",
        resvg_safe.stderr
    );

    let parity_svg = String::from_utf8(parity.stdout).expect("parity stdout should be utf8");
    let safe_svg = String::from_utf8(resvg_safe.stdout).expect("resvg-safe stdout should be utf8");
    assert!(
        parity_svg.contains("<foreignObject"),
        "default SVG output should preserve parity HTML labels:\n{parity_svg}"
    );
    assert!(
        !safe_svg.contains("<foreignObject"),
        "resvg-safe SVG output should not rely on foreignObject:\n{safe_svg}"
    );
    assert!(
        safe_svg.contains(r#"data-merman-foreignobject="fallback""#),
        "resvg-safe SVG output should keep generated text fallbacks:\n{safe_svg}"
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
fn top_level_rejects_unknown_output_extension() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "out.unknown"])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected extension failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Output file must end"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!tmp.path().join("out.unknown").exists());
}

#[test]
fn top_level_output_format_does_not_bypass_extension_validation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "out.unknown", "-e", "svg"])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "explicit format should not bypass mmdc output extension validation"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Output file must end"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn top_level_rejects_unknown_output_format() {
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "-e", "gif"],
        "flowchart LR\nA-->B\n",
    );

    assert!(!output.status.success(), "expected output format failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("invalid value") && stderr.contains("gif"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn top_level_pdf_fit_controls_page_size() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "default.pdf", "-q"])
        .assert()
        .success();
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "fit.pdf", "--pdfFit", "-q"])
        .assert()
        .success();

    let default_pdf = fs::read(tmp.path().join("default.pdf")).expect("read default pdf");
    let fit_pdf = fs::read(tmp.path().join("fit.pdf")).expect("read fit pdf");
    assert!(default_pdf.starts_with(b"%PDF-"));
    assert!(fit_pdf.starts_with(b"%PDF-"));

    let default_media_box = pdf_media_box(&default_pdf).expect("default media box");
    let fit_media_box = pdf_media_box(&fit_pdf).expect("fit media box");
    assert!(
        default_media_box.contains("612") && default_media_box.contains("792"),
        "default mmdc PDF should use a Letter-sized page, got {default_media_box}"
    );
    assert_ne!(
        default_media_box, fit_media_box,
        "--pdfFit should produce a chart-sized page distinct from default PDF output"
    );
}

#[test]
fn top_level_default_output_for_input_file_appends_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

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
fn top_level_default_output_for_stdin_writes_out_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = run_with_stdin_in_dir(&["-q"], "flowchart LR\nA-->B\n", Some(tmp.path()));

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let out = tmp.path().join("out.svg");
    assert!(out.exists(), "stdin default output should be out.svg");
    let svg = fs::read_to_string(out).expect("read svg");
    assert!(svg.trim_start().starts_with("<svg"));
}

#[test]
fn config_file_theme_overrides_cli_theme() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("mermaid.json");
    fs::write(&config, r#"{"theme":"default"}"#).expect("write config");

    let diagram = "flowchart LR\nA-->B\n";
    let default_svg = run_with_stdin(&["-i", "-", "-o", "-", "-t", "default"], diagram);
    let dark_svg = run_with_stdin(&["-i", "-", "-o", "-", "-t", "dark"], diagram);
    let config_svg = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "-t",
            "dark",
            "-c",
            config.to_string_lossy().as_ref(),
        ],
        diagram,
    );

    assert!(
        default_svg.status.success(),
        "stderr: {:?}",
        default_svg.stderr
    );
    assert!(dark_svg.status.success(), "stderr: {:?}", dark_svg.stderr);
    assert!(
        config_svg.status.success(),
        "stderr: {:?}",
        config_svg.stderr
    );
    assert_ne!(
        default_svg.stdout, dark_svg.stdout,
        "dark theme should differ from default theme"
    );
    assert_eq!(
        default_svg.stdout, config_svg.stdout,
        "config theme should override CLI theme like official mmdc"
    );
}

#[test]
fn config_file_theme_variables_and_theme_css_affect_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("mermaid.json");
    fs::write(
        &config,
        r##"{
  "theme": "base",
  "themeVariables": {
    "mainBkg": "#111827",
    "nodeTextColor": "#f8fafc",
    "nodeBorder": "#38bdf8"
  },
  "themeCSS": ".node rect { filter: drop-shadow(1px 1px 1px #000); }"
}"##,
    )
    .expect("write config");

    let output = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "-I",
            "cli-theme-config",
            "-c",
            config.to_string_lossy().as_ref(),
        ],
        "flowchart TD\nA[Plain source]\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(svg.contains("#111827"), "unexpected SVG:\n{svg}");
    assert!(svg.contains("#f8fafc"), "unexpected SVG:\n{svg}");
    assert!(svg.contains("#38bdf8"), "unexpected SVG:\n{svg}");
    assert!(
        svg.contains("#cli-theme-config .node rect { filter: drop-shadow(1px 1px 1px #000); }"),
        "unexpected SVG:\n{svg}"
    );
    assert!(
        svg.contains(r#"data-merman-postprocess="scoped-css""#),
        "unexpected SVG:\n{svg}"
    );
}

#[test]
fn non_object_config_file_fails_before_rendering() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("mermaid.json");
    fs::write(&config, r#""dark""#).expect("write config");

    let output = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "-c",
            config.to_string_lossy().as_ref(),
        ],
        "flowchart TD\nA[Plain source]\n",
    );

    assert!(!output.status.success(), "expected config file failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("configuration file") && stderr.contains("JSON object"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn markdown_input_writes_numbered_svg_artefacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output = tmp.path().join("out.svg");
    fs::write(
        &input,
        "before\n```mermaid\nflowchart LR\nA-->B\n```\nafter\n:::mermaid\nsequenceDiagram\nA->>B: Hi\n:::\n",
    )
    .expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "-q",
        ])
        .assert()
        .success();

    let first = fs::read_to_string(tmp.path().join("out-1.svg")).expect("read first svg");
    let second = fs::read_to_string(tmp.path().join("out-2.svg")).expect("read second svg");
    assert!(first.trim_start().starts_with("<svg"));
    assert!(second.trim_start().starts_with("<svg"));
    assert!(
        !output.exists(),
        "template image output should not be written for Markdown input"
    );
}

#[test]
fn markdown_output_rewrites_mermaid_blocks_to_images() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output = tmp.path().join("out.md");
    fs::write(&input, "# Doc\n\n```mermaid\nflowchart LR\nA-->B\n```\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "-q",
        ])
        .assert()
        .success();

    let rewritten = fs::read_to_string(&output).expect("read rewritten markdown");
    assert!(
        rewritten.contains("![diagram](./out-1.svg)"),
        "unexpected rewritten markdown:\n{rewritten}"
    );
    assert!(!rewritten.contains("```mermaid"));
    assert!(tmp.path().join("out-1.svg").exists());
}

#[test]
fn markdown_artefacts_directory_controls_image_location() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let docs = tmp.path().join("docs");
    let assets = tmp.path().join("assets");
    fs::create_dir(&docs).expect("create docs dir");
    let input = docs.join("input.md");
    let output = docs.join("out.md");
    fs::write(&input, "```mermaid\nflowchart LR\nA-->B\n```\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "-a",
            assets.to_string_lossy().as_ref(),
            "-q",
        ])
        .assert()
        .success();

    assert!(assets.join("out-1.svg").exists());
    let rewritten = fs::read_to_string(&output).expect("read rewritten markdown");
    assert!(
        rewritten.contains("![diagram](./../assets/out-1.svg)"),
        "unexpected rewritten markdown:\n{rewritten}"
    );
}

#[test]
fn markdown_jobs_preserve_rewrite_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output = tmp.path().join("out.md");
    fs::write(
        &input,
        "one\n```mermaid\nflowchart LR\nA1-->B1\n```\ntwo\n```mermaid\nflowchart LR\nA2-->B2\n```\nthree\n```mermaid\nflowchart LR\nA3-->B3\n```\n",
    )
    .expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "--jobs",
            "2",
            "-q",
        ])
        .assert()
        .success();

    for index in 1..=3 {
        assert!(
            tmp.path().join(format!("out-{index}.svg")).exists(),
            "missing numbered artefact {index}"
        );
    }

    let rewritten = fs::read_to_string(&output).expect("read rewritten markdown");
    let first = rewritten.find("./out-1.svg").expect("first image");
    let second = rewritten.find("./out-2.svg").expect("second image");
    let third = rewritten.find("./out-3.svg").expect("third image");
    assert!(
        first < second && second < third,
        "Markdown image order must follow source order:\n{rewritten}"
    );
}

#[test]
fn markdown_input_rejects_stdout_output() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    fs::write(&input, "```mermaid\nflowchart LR\nA-->B\n```\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", input.to_string_lossy().as_ref(), "-o", "-"])
        .assert()
        .failure();
}

#[test]
fn markdown_without_charts_logs_and_writes_no_artefacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    fs::write(&input, "# No diagrams\n\nPlain text.\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.md", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stdout.is_empty(),
        "Markdown diagnostics must not be written to stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("No mermaid charts found in Markdown input"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!tmp.path().join("out.svg").exists());
    assert!(!tmp.path().join("out-1.svg").exists());
}

#[test]
fn missing_puppeteer_config_file_fails_before_rendering() {
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "-p", "missing-puppeteer-config.json"],
        "flowchart LR\nA-->B\n",
    );

    assert!(!output.status.success(), "expected missing file failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Puppeteer configuration file")
            && stderr.contains("missing-puppeteer-config.json"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn invalid_puppeteer_config_file_fails_before_rendering() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("puppeteer.json");
    fs::write(&config, "{not json").expect("write invalid config");

    let output = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "-p",
            config.to_string_lossy().as_ref(),
        ],
        "flowchart LR\nA-->B\n",
    );

    assert!(!output.status.success(), "expected JSON failure");
    assert_eq!(
        exit_code(output.status),
        2,
        "invalid config should be usage/config error"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("JSON error"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn dynamic_icon_pack_url_file_renders_flowchart_icon() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let icons = tmp.path().join("icons.json");
    fs::write(
        &icons,
        r#"{
            "prefix": "test",
            "width": 16,
            "height": 16,
            "icons": {
                "rocket": {
                    "body": "<path data-icon=\"rocket\" fill=\"currentColor\" d=\"M1 1H15V15H1z\"/>"
                }
            }
        }"#,
    )
    .expect("write icons");

    let icon_arg = format!("test#{}", icons.display());
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "--iconPacksNamesAndUrls", &icon_arg],
        "flowchart TD\nA@{ icon: \"test:rocket\", label: \"Rocket\" }\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"data-icon="rocket""#),
        "expected custom icon body in SVG:\n{stdout}"
    );
    assert!(
        !stdout.contains(r#"<tspan x="0" y="0">?</tspan>"#),
        "custom icon should replace the placeholder SVG:\n{stdout}"
    );
}

#[test]
fn dynamic_icon_pack_http_url_renders_flowchart_icon() {
    let url = serve_icon_json_once(
        r#"{
            "prefix": "remote",
            "width": 16,
            "height": 16,
            "icons": {
                "cloud": {
                    "body": "<path data-icon=\"cloud\" fill=\"currentColor\" d=\"M1 8H15V14H1z\"/>"
                }
            }
        }"#,
    );

    let icon_arg = format!("remote#{url}");
    let output = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "--allow-network",
            "--iconPacksNamesAndUrls",
            &icon_arg,
        ],
        "flowchart TD\nA@{ icon: \"remote:cloud\", label: \"Cloud\" }\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"data-icon="cloud""#),
        "expected HTTP icon body in SVG:\n{stdout}"
    );
}

#[test]
fn dynamic_icon_pack_http_url_requires_network_opt_in() {
    let icon_arg = "remote#https://example.invalid/icons.json";
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "--iconPacksNamesAndUrls", icon_arg],
        "flowchart TD\nA@{ icon: \"remote:cloud\", label: \"Cloud\" }\n",
    );

    assert!(!output.status.success(), "expected network policy failure");
    assert_eq!(
        exit_code(output.status),
        2,
        "network policy failure should be usage/config error"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    let normalized = stderr.to_ascii_lowercase();
    assert!(
        stderr.contains("--allow-network") && normalized.contains("icon pack"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn dynamic_icon_pack_package_missing_local_copy_does_not_fetch_by_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = run_with_stdin_in_dir(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "--iconPacks",
            "@iconify-json/missing-test-pack",
        ],
        "flowchart TD\nA@{ icon: \"missing-test-pack:box\", label: \"Box\" }\n",
        Some(tmp.path()),
    );

    assert!(
        !output.status.success(),
        "expected missing local package failure"
    );
    assert_eq!(
        exit_code(output.status),
        2,
        "missing local icon package should be usage/config error"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("@iconify-json/missing-test-pack")
            && stderr.contains("--allow-network")
            && stderr.contains("node_modules"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn dynamic_icon_pack_package_renders_local_node_modules_icon() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let package = tmp
        .path()
        .join("node_modules")
        .join("@iconify-json")
        .join("test");
    fs::create_dir_all(&package).expect("create icon package");
    fs::write(
        package.join("icons.json"),
        r#"{
            "prefix": "ignored",
            "width": 20,
            "height": 20,
            "icons": {
                "box": {
                    "body": "<path data-icon=\"box\" fill=\"currentColor\" d=\"M2 2H18V18H2z\"/>"
                }
            }
        }"#,
    )
    .expect("write icons");

    let output = run_with_stdin_in_dir(
        &["-i", "-", "-o", "-", "--iconPacks", "@iconify-json/test"],
        "flowchart TD\nA@{ icon: \"test:box\", label: \"Box\" }\n",
        Some(tmp.path()),
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"data-icon="box""#),
        "expected local package icon body in SVG:\n{stdout}"
    );
}

#[test]
fn developer_subcommands_still_work() {
    let output = run_with_stdin(&["detect", "-"], "sequenceDiagram\nA->>B: Hello\n");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert_eq!(stdout.trim(), "sequence");
}
