mod support;

use serde_json::Value;
use std::process::{Command, Stdio};
use support::{run_with_stdin, run_with_stdin_bytes as run_with_stdin_input};

fn task_by_id<'a>(model: &'a Value, id: &str) -> &'a Value {
    model["tasks"]
        .as_array()
        .expect("gantt tasks should be an array")
        .iter()
        .find(|task| task["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("missing Gantt task {id} in {model}"))
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
fn cli_lint_defaults_to_text_while_json_remains_explicit() {
    let source = "flowchart TD\nA[Hello] --> B[World]\n";
    let text = run_with_stdin(&["lint", "-"], source);
    assert!(text.status.success(), "stderr: {:?}", text.stderr);
    assert_eq!(
        String::from_utf8(text.stdout).expect("lint stdout should be utf8"),
        "No Mermaid diagnostics.\n"
    );

    let json = run_with_stdin(&["lint", "--format", "json", "-"], source);
    assert!(json.status.success(), "stderr: {:?}", json.stderr);
    let payload: Value =
        serde_json::from_slice(&json.stdout).expect("explicit JSON should remain machine-readable");
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["valid"], true);
}

#[test]
fn lint_pretty_error_explains_how_to_enable_json() {
    let output = run_with_stdin(&["lint", "--pretty", "missing.mmd"], "");

    assert_eq!(support::exit_code(output.status), 2);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--format json --pretty") && !stderr.contains("missing.mmd:"),
        "pretty validation must be actionable and precede input acquisition:\n{stderr}"
    );
}

#[test]
fn lint_explicit_json_pretty_output_succeeds() {
    let output = run_with_stdin(
        &["lint", "--format", "json", "--pretty", "-"],
        "flowchart TD\nA-->B\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("\n  \"version\": 1,"));
    serde_json::from_str::<Value>(&stdout).expect("pretty lint output should remain valid JSON");
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
    assert_eq!(flowchart_html_labels["fixable"], false);
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
        let output = Command::new(exe)
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
            "--resource-limit",
            "max_source_bytes=8",
            "--disable-rule",
            "merman.resource.source_bytes_exceeded",
            "-",
        ],
        vec![
            "lint",
            "--format",
            "json",
            "--resource-limit",
            "max_source_bytes=8",
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
fn cli_parse_meta_reports_javascript_compatible_railroad_repeat_bounds() {
    let huge = "9".repeat(400);
    let source = format!(
        "railroad-abnf-beta\nrounded = 9007199254740993\"a\" ;\ninfinite = {huge}\"b\" ;\n"
    );
    let output = run_with_stdin(&["parse", "--meta", "-"], &source);

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("parse --meta stdout should be JSON");
    assert_eq!(payload["meta"]["diagram_type"], "railroadAbnf");

    let rules = payload["model"]["rules"]
        .as_array()
        .expect("Railroad rules should be an array");
    assert_eq!(rules.len(), 2);
    for field in ["min", "max"] {
        assert_eq!(
            rules[0]["definition"][field].as_f64(),
            Some(9_007_199_254_740_992.0),
            "rounded {field}"
        );
        assert!(rules[1]["definition"][field].is_null(), "infinite {field}");
    }
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
fn cli_commands_reject_out_of_range_fixed_local_midnight_consistently() {
    let source = "flowchart TD\nA-->B\n";
    for command in ["parse", "layout"] {
        let output = run_with_stdin(
            &[
                command,
                "--fixed-today=-2147483648-01-01",
                "--fixed-local-offset-minutes",
                "1439",
                "-",
            ],
            source,
        );
        assert!(
            !output.status.success(),
            "{command} unexpectedly accepted an out-of-range local midnight"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("local datetime"),
            "unexpected {command} error: {stderr}"
        );
    }
}
