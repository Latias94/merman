mod support;

use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;
use support::{exit_code, run_with_stdin, run_with_stdin_bytes, run_with_stdin_in_dir};

const SOURCE: &str = "flowchart LR\nA-->B\n";

#[test]
fn primary_stdin_accepts_exact_source_limit_and_rejects_one_more_byte() {
    let limit = SOURCE.len().to_string();
    let exact = run_with_stdin(
        &[
            "parse",
            "-",
            "--resource-limit",
            &format!("max_source_bytes={limit}"),
        ],
        SOURCE,
    );
    assert!(exact.status.success(), "stderr: {:?}", exact.stderr);

    let oversized_source = format!("{SOURCE}\n");
    let oversized = run_with_stdin(
        &[
            "parse",
            "-",
            "--resource-limit",
            &format!("max_source_bytes={limit}"),
        ],
        &oversized_source,
    );
    assert_eq!(exit_code(oversized.status), 1);
    assert!(oversized.stdout.is_empty());
    let stderr = utf8(&oversized.stderr);
    assert!(stderr.contains("max_source_bytes"), "{stderr}");
    assert!(stderr.contains(&(SOURCE.len() + 1).to_string()), "{stderr}");
}

#[test]
fn primary_file_length_hint_rejects_before_reading_content() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("diagram.mmd");
    fs::write(&input, SOURCE).expect("write source");

    let output = Command::new(cargo_bin!("merman-cli"))
        .args([
            "parse",
            input.to_str().expect("UTF-8 temp path"),
            "--resource-limit",
            "max_source_bytes=4",
        ])
        .output()
        .expect("run cli");

    assert_eq!(exit_code(output.status), 1);
    let stderr = utf8(&output.stderr);
    assert!(stderr.contains(&SOURCE.len().to_string()), "{stderr}");
    assert!(!stderr.contains("at least"), "{stderr}");
}

#[test]
fn invalid_utf8_primary_input_is_a_content_failure() {
    let output = run_with_stdin_bytes(&["parse", "-"], &[b'f', 0xff, b'o']);

    assert_eq!(exit_code(output.status), 1);
    assert!(output.stdout.is_empty());
    let stderr = utf8(&output.stderr);
    assert!(stderr.contains("valid UTF-8"), "{stderr}");
    assert!(stderr.contains("offset 1"), "{stderr}");
}

#[test]
fn lint_json_projects_streaming_source_overflow_as_a_resource_diagnostic() {
    let output = run_with_stdin(
        &[
            "lint",
            "-",
            "--format",
            "json",
            "--resource-limit",
            "max_source_bytes=4",
        ],
        "12345",
    );

    assert_eq!(exit_code(output.status), 1);
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    let payload: Value = serde_json::from_slice(&output.stdout).expect("lint JSON");
    let diagnostic = &payload["diagnostics"][0];
    assert_eq!(diagnostic["id"], "merman.resource.source_bytes_exceeded");
    assert_eq!(diagnostic["category"], "resource");
    assert_eq!(diagnostic["code_name"], "MERMAN_RESOURCE_LIMIT_EXCEEDED");
    assert_eq!(payload["valid"], false);
}

#[test]
fn configuration_file_is_bounded_at_acquisition() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("config.json");
    fs::write(&config, "{}").expect("write exact config");

    let exact = run_with_stdin(
        &[
            "parse",
            "-",
            "--config-file",
            config.to_str().expect("UTF-8 temp path"),
            "--resource-limit",
            "max_config_bytes=2",
        ],
        SOURCE,
    );
    assert!(exact.status.success(), "stderr: {:?}", exact.stderr);

    fs::write(&config, "{ }\n").expect("write oversized config");
    let oversized = run_with_stdin(
        &[
            "parse",
            "-",
            "--config-file",
            config.to_str().expect("UTF-8 temp path"),
            "--resource-limit",
            "max_config_bytes=3",
        ],
        SOURCE,
    );
    assert_eq!(exit_code(oversized.status), 2);
    assert!(oversized.stdout.is_empty());
    let stderr = utf8(&oversized.stderr);
    assert!(stderr.contains("configuration file"), "{stderr}");
    assert!(stderr.contains("3-byte limit"), "{stderr}");
}

#[test]
fn css_and_puppeteer_files_use_their_owned_limits() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let css = tmp.path().join("style.css");
    fs::write(&css, "a{}").expect("write CSS");
    let css_exact = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--css-file",
            css.to_str().expect("UTF-8 temp path"),
            "--resource-limit",
            "max_css_bytes=3",
        ],
        SOURCE,
    );
    assert!(css_exact.status.success(), "stderr: {:?}", css_exact.stderr);

    fs::write(&css, "a{ }").expect("write oversized CSS");
    let css_oversized = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--css-file",
            css.to_str().expect("UTF-8 temp path"),
            "--resource-limit",
            "max_css_bytes=3",
        ],
        SOURCE,
    );
    assert_eq!(exit_code(css_oversized.status), 2);
    assert!(utf8(&css_oversized.stderr).contains("CSS file"));

    let puppeteer = tmp.path().join("puppeteer.json");
    fs::write(&puppeteer, "{}").expect("write Puppeteer config");
    let puppeteer_exact = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--puppeteerConfigFile",
            puppeteer.to_str().expect("UTF-8 temp path"),
            "--resource-limit",
            "max_puppeteer_config_bytes=2",
        ],
        SOURCE,
    );
    assert!(
        puppeteer_exact.status.success(),
        "stderr: {:?}",
        puppeteer_exact.stderr
    );

    fs::write(&puppeteer, "{ }\n").expect("write oversized Puppeteer config");
    let puppeteer_oversized = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--puppeteerConfigFile",
            puppeteer.to_str().expect("UTF-8 temp path"),
            "--resource-limit",
            "max_puppeteer_config_bytes=3",
        ],
        SOURCE,
    );
    assert_eq!(exit_code(puppeteer_oversized.status), 2);
    assert!(utf8(&puppeteer_oversized.stderr).contains("Puppeteer configuration file"));
}

#[test]
fn local_icon_bodies_enforce_individual_and_aggregate_limits() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let first_body = icon_body("first");
    let second_body = icon_body("second");
    let first = tmp.path().join("first.json");
    let second = tmp.path().join("second.json");
    fs::write(&first, &first_body).expect("write first icon pack");
    fs::write(&second, &second_body).expect("write second icon pack");

    let exact_limit = format!("max_local_icon_body_bytes={}", first_body.len());
    let exact_aggregate = format!("max_aggregate_icon_bytes={}", first_body.len());
    let exact_source = format!("first#{}", first.display());
    let exact = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--icon-pack-source",
            &exact_source,
            "--resource-limit",
            &exact_limit,
            "--resource-limit",
            &exact_aggregate,
        ],
        SOURCE,
    );
    assert!(exact.status.success(), "stderr: {:?}", exact.stderr);

    let individual_limit = format!("max_local_icon_body_bytes={}", first_body.len() - 1);
    let individual = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--icon-pack-source",
            &exact_source,
            "--resource-limit",
            &individual_limit,
        ],
        SOURCE,
    );
    assert_eq!(exit_code(individual.status), 2);
    assert!(utf8(&individual.stderr).contains("max_local_icon_body_bytes"));

    let combined = first_body.len() + second_body.len();
    let second_source = format!("second#{}", second.display());
    let aggregate_limit = format!("max_aggregate_icon_bytes={}", combined - 1);
    let aggregate = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--icon-pack-source",
            &exact_source,
            "--icon-pack-source",
            &second_source,
            "--resource-limit",
            &aggregate_limit,
        ],
        SOURCE,
    );
    assert_eq!(exit_code(aggregate.status), 2);
    assert!(utf8(&aggregate.stderr).contains("max_aggregate_icon_bytes"));
}

#[test]
fn icon_count_is_rejected_before_opening_sources() {
    let output = run_with_stdin(
        &[
            "render",
            "missing.mmd",
            "--icon-pack-source",
            "one#missing-one.json",
            "--icon-pack-source",
            "two#missing-two.json",
            "--resource-limit",
            "max_icon_packs=1",
        ],
        "",
    );

    assert_eq!(exit_code(output.status), 2);
    let stderr = utf8(&output.stderr);
    assert!(stderr.contains("max_icon_packs"), "{stderr}");
    assert!(!stderr.contains("missing.mmd"), "{stderr}");
    assert!(!stderr.contains("missing-one.json"), "{stderr}");
}

#[test]
fn network_authorization_is_two_stage_and_diagnostics_are_sanitized() {
    let private_url = "private#http://127.0.0.1:9/secret/path?token=do-not-print";
    let denied = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--icon-pack-source",
            private_url,
            "--allow-network",
        ],
        SOURCE,
    );
    assert_eq!(exit_code(denied.status), 2);
    let denied_stderr = utf8(&denied.stderr);
    assert!(denied_stderr.contains("private network authorization"));
    assert!(!denied_stderr.contains("secret/path"), "{denied_stderr}");
    assert!(!denied_stderr.contains("do-not-print"), "{denied_stderr}");

    let credentials = "private#http://user:password@127.0.0.1:9/secret/path?token=do-not-print";
    let credential_error = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--icon-pack-source",
            credentials,
            "--allow-network",
            "--allow-private-network",
        ],
        SOURCE,
    );
    assert_eq!(exit_code(credential_error.status), 2);
    let credential_stderr = utf8(&credential_error.stderr);
    assert!(credential_stderr.contains("credentials are not allowed"));
    for secret in ["user", "password", "secret/path", "do-not-print"] {
        assert!(!credential_stderr.contains(secret), "{credential_stderr}");
    }
}

#[test]
fn remote_icon_body_stream_stops_at_limit_plus_one() {
    let body = icon_body("remote");
    let url = serve_body_once(body.as_bytes());
    let source = format!("remote#{url}/secret-path?token=do-not-print");
    let limit = format!("max_remote_icon_body_bytes={}", body.len() - 1);
    let output = run_with_stdin(
        &[
            "render",
            "-",
            "--output",
            "-",
            "--icon-pack-source",
            &source,
            "--allow-network",
            "--allow-private-network",
            "--resource-limit",
            &limit,
        ],
        SOURCE,
    );

    assert_eq!(exit_code(output.status), 2);
    let stderr = utf8(&output.stderr);
    assert!(stderr.contains("body limit"), "{stderr}");
    assert!(!stderr.contains("secret-path"), "{stderr}");
    assert!(!stderr.contains("do-not-print"), "{stderr}");
}

#[test]
fn markdown_chart_staging_and_scheduler_budgets_reject_workloads() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let markdown = tmp.path().join("input.md");
    fs::write(
        &markdown,
        "```mermaid\nflowchart LR\nA-->B\n```\n\n```mermaid\nflowchart LR\nB-->C\n```\n",
    )
    .expect("write Markdown");

    let chart_output = run_command_in(
        tmp.path(),
        &[
            "batch",
            "input.md",
            "--output-dir",
            "chart-out",
            "--resource-limit",
            "max_markdown_charts=1",
        ],
    );
    assert_eq!(exit_code(chart_output.status), 1);
    assert!(utf8(&chart_output.stderr).contains("max_markdown_charts"));

    fs::write(&markdown, "```mermaid\nflowchart LR\nA-->B\n```\n")
        .expect("write single chart Markdown");
    let staged_output = run_command_in(
        tmp.path(),
        &[
            "batch",
            "input.md",
            "--output-dir",
            "staged-out",
            "--resource-limit",
            "max_staged_bytes=1",
        ],
    );
    assert_eq!(exit_code(staged_output.status), 1);
    assert!(utf8(&staged_output.stderr).contains("max_staged_bytes"));
    assert!(!tmp.path().join("staged-out/input-1.svg").exists());

    let raster_output = run_with_stdin_in_dir(
        &[
            "render",
            "-",
            "--format",
            "png",
            "--output",
            "out.png",
            "--resource-limit",
            "max_scheduling_weight_bytes=1",
        ],
        SOURCE,
        Some(tmp.path()),
    );
    assert_eq!(exit_code(raster_output.status), 1);
    assert!(utf8(&raster_output.stderr).contains("max_scheduling_weight_bytes"));
    assert!(!tmp.path().join("out.png").exists());
}

#[test]
fn jobs_and_resource_override_errors_precede_input_acquisition() {
    for args in [
        [
            "batch",
            "missing.md",
            "--jobs",
            "3",
            "--resource-limit",
            "max_jobs=2",
        ]
        .as_slice(),
        [
            "parse",
            "missing.mmd",
            "--resource-limit",
            "max_source_bytes=4",
            "--resource-limit",
            "max_source_bytes=5",
        ]
        .as_slice(),
        [
            "parse",
            "missing.mmd",
            "--resource-limit",
            "unknown_limit=1",
        ]
        .as_slice(),
    ] {
        let output = run_with_stdin(args, "");
        assert_eq!(exit_code(output.status), 2, "args: {args:?}");
        assert!(output.stdout.is_empty());
        let stderr = utf8(&output.stderr);
        assert!(!stderr.contains("doesn't exist"), "{stderr}");
    }
}

#[test]
fn superseded_resource_flags_are_rejected() {
    for flag in [
        "--max-source-bytes",
        "--encoding-parallel-budget-mib",
        "--encoding-memory-budget-mib",
    ] {
        let output = run_with_stdin(&["parse", "-", flag, "4"], SOURCE);
        assert_eq!(exit_code(output.status), 2, "flag: {flag}");
        let stderr = utf8(&output.stderr);
        assert!(stderr.contains(flag), "{stderr}");
        assert!(stderr.contains("--resource-limit"), "{stderr}");
    }
}

fn icon_body(prefix: &str) -> String {
    format!(r#"{{"prefix":"{prefix}","icons":{{"cloud":{{"body":"<path/>"}}}}}}"#)
}

fn serve_body_once(body: &[u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind HTTP fixture");
    let address = listener.local_addr().expect("fixture address");
    let body = body.to_vec();
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request);
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(headers.as_bytes());
        let _ = stream.write_all(&body);
    });
    format!("http://{address}")
}

fn run_command_in(directory: &Path, args: &[&str]) -> Output {
    Command::new(cargo_bin!("merman-cli"))
        .current_dir(directory)
        .args(args)
        .output()
        .expect("run cli")
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("UTF-8 process output")
}
