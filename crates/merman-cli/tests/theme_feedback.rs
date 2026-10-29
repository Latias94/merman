#![cfg(all(
    feature = "svg",
    feature = "diagram-sequence",
    feature = "diagram-flowchart"
))]

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

const SOURCE: &str = "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n";

fn setup(directory: &Path) {
    fs::write(directory.join("input.mmd"), SOURCE).unwrap();
    fs::write(
        directory.join("theme.json"),
        serde_json::to_vec(&json!({
            "schema_version": 1,
            "kind": "complete_spec",
            "complete_spec": {"styles": [
                {"kind":"ordinal-palette", "target":"node", "colors":["#123456"]},
                {"kind":"rule", "family":"sequence", "target":"lifeline",
                 "style":{"stroke":{"paint":"#2563eb"}, "radius":6}}
            ]}
        }))
        .unwrap(),
    )
    .unwrap();
}

fn run(directory: &Path, args: &[&str]) -> Output {
    Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn read_report(directory: &Path) -> Value {
    let value: Value =
        serde_json::from_slice(&fs::read(directory.join("report.json")).unwrap()).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["stage"], "render");
    value
}

fn formats() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("svg", b"<svg"),
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n"),
        #[cfg(feature = "jpeg")]
        ("jpg", b"\xff\xd8"),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF-"),
    ]
}

#[test]
fn every_graphical_format_reports_actual_theme_residuals_without_polluting_stdout() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    for (format, signature) in formats() {
        let output = run(
            directory.path(),
            &[
                "render",
                "input.mmd",
                "--format",
                format,
                "-o",
                "-",
                "--theme-file",
                "theme.json",
                "--theme-report",
                "report.json",
            ],
        );
        success(&output);
        assert!(
            output.stdout.starts_with(signature),
            "invalid {format} output"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("theme (sequence/"), "{stderr}");
        assert!(stderr.contains("unsupported-geometry"), "{stderr}");
        let report = read_report(directory.path());
        let result = &report["results"][0];
        assert!(result.get("error").is_none());
        assert_eq!(result["evidence"]["family"], "sequence");
        assert_eq!(
            result["target"]["kind"],
            if format == "jpg" { "jpeg" } else { format }
        );
        let diagnostics = result["evidence"]["diagnostics"].as_array().unwrap();
        let rule = diagnostics
            .iter()
            .find(|diagnostic| diagnostic["code"] == "unsupported-geometry")
            .unwrap();
        assert_eq!(rule["subject"], "rule");
        assert_eq!(rule["target"], "lifeline");
        assert_eq!(rule["source_document"], "complete_spec");
        assert_eq!(rule["source_paths"], json!(["/styles/1"]));
        assert_eq!(rule["generated"], false);
        assert!(
            rule.get("property").is_none(),
            "rule evidence must not invent a failing property"
        );
        if format == "svg" {
            let svg = std::str::from_utf8(&output.stdout).unwrap();
            roxmltree::Document::parse(svg).unwrap();
            assert!(svg.contains("#2563eb"), "supported stroke must still apply");
        }
    }
}

#[test]
fn quiet_keeps_machine_report_and_the_default_policy_preserves_output() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    let baseline = run(
        directory.path(),
        &[
            "render",
            "input.mmd",
            "-o",
            "-",
            "--theme-file",
            "theme.json",
        ],
    );
    success(&baseline);
    let output = run(
        directory.path(),
        &[
            "render",
            "input.mmd",
            "-o",
            "-",
            "--theme-file",
            "theme.json",
            "--theme-portability",
            "best-effort",
            "--theme-report",
            "report.json",
            "--quiet",
        ],
    );
    success(&output);
    assert!(
        output.stderr.is_empty(),
        "quiet must suppress theme feedback"
    );
    assert_eq!(baseline.stdout, output.stdout);
    assert!(
        !read_report(directory.path())["results"][0]["evidence"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn plain_diagrams_do_not_print_theme_feedback_and_can_still_request_a_report() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("input.mmd"), "flowchart LR\nA-->B\n").unwrap();
    let output = run(
        directory.path(),
        &[
            "render",
            "input.mmd",
            "-o",
            "-",
            "--theme-report",
            "report.json",
        ],
    );
    success(&output);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("theme ("));
    assert_eq!(
        read_report(directory.path())["results"][0]["target"]["status"],
        "unverified"
    );
}

#[test]
fn strict_renderer_rejections_write_failure_reports_and_preserve_existing_images() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    for (format, _) in formats() {
        let image = format!("image.{format}");
        fs::write(directory.path().join(&image), b"previous image").unwrap();
        let output = run(
            directory.path(),
            &[
                "render",
                "input.mmd",
                "-o",
                &image,
                "--theme-file",
                "theme.json",
                "--theme-portability",
                "require-portable",
                "--theme-report",
                "report.json",
                "--quiet",
            ],
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read(directory.path().join(&image)).unwrap(),
            b"previous image"
        );
        let report = read_report(directory.path());
        let result = &report["results"][0];
        let error = result["error"].as_str().expect("renderer error");
        assert!(error.to_lowercase().contains("portable"), "{error}");
        assert!(
            result.get("evidence").is_none(),
            "failed rendering must not fabricate completed evidence"
        );
    }
}

#[test]
fn invalid_mermaid_writes_a_failure_report_without_truncating_an_image() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("input.mmd"),
        "invalid diagram syntax\n",
    )
    .unwrap();
    fs::write(directory.path().join("image.svg"), b"previous image").unwrap();
    let output = run(
        directory.path(),
        &[
            "render",
            "input.mmd",
            "-o",
            "image.svg",
            "--theme-report",
            "report.json",
        ],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read(directory.path().join("image.svg")).unwrap(),
        b"previous image"
    );
    assert!(read_report(directory.path())["results"][0]["error"].is_string());
}

#[test]
fn strict_svg_target_errors_preserve_the_renderer_receipt_in_the_report() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    let output = run(
        directory.path(),
        &[
            "render",
            "input.mmd",
            "-o",
            "-",
            "--theme-portability",
            "require-portable",
            "--theme-report",
            "report.json",
            "--quiet",
        ],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let report = read_report(directory.path());
    let result = &report["results"][0];
    assert!(result["error"].is_string());
    assert_eq!(result["target"]["kind"], "svg");
    assert_ne!(result["target"]["status"], "portable");
    assert!(!result["target"]["reasons"].as_array().unwrap().is_empty());
    assert!(result.get("evidence").is_none());
}

#[test]
fn report_paths_reject_stdout_and_image_aliases_before_acquiring_stdin() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("image.svg"), b"previous image").unwrap();
    fs::hard_link(
        directory.path().join("image.svg"),
        directory.path().join("alias.json"),
    )
    .unwrap();
    for path in ["-", "image.svg", "alias.json", "missing/report.json"] {
        let output = support::run_before_stdin_close_in_dir(
            &["render", "-", "-o", "image.svg", "--theme-report", path],
            Some(directory.path()),
            Duration::from_secs(5),
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read(directory.path().join("image.svg")).unwrap(),
            b"previous image"
        );
    }
    fs::write(directory.path().join("input.mmd"), "flowchart LR\nA-->B\n").unwrap();
    let output = run(
        directory.path(),
        &[
            "render",
            "input.mmd",
            "-o",
            "image.svg",
            "--theme-report",
            "input.mmd",
        ],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("aliases protected"));
    assert_eq!(
        fs::read_to_string(directory.path().join("input.mmd")).unwrap(),
        "flowchart LR\nA-->B\n"
    );
}

#[test]
fn new_report_and_image_names_cannot_differ_only_by_case() {
    let directory = tempfile::tempdir().unwrap();
    let output = support::run_before_stdin_close_in_dir(
        &[
            "render",
            "-",
            "-o",
            "image.svg",
            "--theme-report",
            "IMAGE.svg",
        ],
        Some(directory.path()),
        Duration::from_secs(5),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("aliases an image"));
    assert!(!directory.path().join("image.svg").exists());
    assert!(!directory.path().join("IMAGE.svg").exists());
}

#[test]
fn compatibility_file_render_does_not_emit_native_theme_feedback() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    let output = run(
        directory.path(),
        &["mmdc", "-i", "input.mmd", "-o", "image.svg"],
    );
    success(&output);
    assert!(directory.path().join("image.svg").exists());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("theme ("));
}

#[test]
fn report_cannot_replace_a_redirected_standard_stream() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    for stream in ["stdout", "stderr"] {
        let redirected = fs::File::create(directory.path().join("report.json")).unwrap();
        let mut command = Command::new(assert_cmd::cargo_bin!("merman-cli"));
        command
            .current_dir(directory.path())
            .args([
                "render",
                "input.mmd",
                "-o",
                "image.svg",
                "--theme-report",
                "report.json",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if stream == "stdout" {
            command.stdout(redirected);
        } else {
            command.stderr(redirected);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(!directory.path().join("image.svg").exists());
        let contents = fs::read_to_string(directory.path().join("report.json")).unwrap();
        if stream == "stdout" {
            assert!(contents.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("aliases stdout"));
        } else {
            assert!(contents.contains("aliases stderr"));
            assert!(!contents.contains("schema_version"));
        }
    }
}

#[cfg(feature = "markdown")]
#[test]
fn batch_reports_are_ordered_and_strict_failure_keeps_the_previous_generation() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    let document = format!("```mermaid\n{SOURCE}```\n\n```mermaid\n{SOURCE}```\n");
    fs::write(directory.path().join("input.md"), document).unwrap();
    for jobs in ["1", "2"] {
        let mut args = vec![
            "batch",
            "input.md",
            "-o",
            "generated",
            "--theme-file",
            "theme.json",
            "--theme-report",
            "report.json",
            "--quiet",
        ];
        #[cfg(feature = "parallel-markdown")]
        args.extend(["--jobs", jobs]);
        #[cfg(not(feature = "parallel-markdown"))]
        let _ = jobs;
        let output = run(directory.path(), &args);
        success(&output);
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
        let report = read_report(directory.path());
        let results = report["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        for (index, result) in results.iter().enumerate() {
            assert_eq!(result["chart"], index + 1);
            assert_eq!(result["evidence"]["family"], "sequence");
            assert!(
                result["output"]
                    .as_str()
                    .unwrap()
                    .ends_with(&format!("input-{}.svg", index + 1))
            );
        }
        let before = fs::read(directory.path().join("generated/input-1.svg")).unwrap();
        args.extend(["--theme-portability", "require-portable"]);
        let output = run(directory.path(), &args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read(directory.path().join("generated/input-1.svg")).unwrap(),
            before
        );
        assert!(
            read_report(directory.path())["results"]
                .as_array()
                .unwrap()
                .iter()
                .any(|result| result["error"].is_string())
        );
    }
}

#[cfg(feature = "markdown")]
#[test]
fn batch_requires_a_report_outside_its_managed_output_directory() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("generated")).unwrap();
    let output = support::run_before_stdin_close_in_dir(
        &[
            "batch",
            "-",
            "--stdin-file-name",
            "input.md",
            "-o",
            "generated",
            "--theme-report",
            "generated/report.json",
        ],
        Some(directory.path()),
        Duration::from_secs(5),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside the managed batch"));
    assert_eq!(
        fs::read_dir(directory.path().join("generated"))
            .unwrap()
            .count(),
        0
    );
}

#[cfg(feature = "markdown")]
#[test]
fn batch_without_charts_writes_an_empty_report() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("input.md"), "No diagrams.\n").unwrap();
    let output = run(
        directory.path(),
        &[
            "batch",
            "input.md",
            "-o",
            "generated",
            "--theme-report",
            "report.json",
            "--quiet",
        ],
    );
    success(&output);
    assert_eq!(read_report(directory.path())["results"], json!([]));
}

#[test]
fn compatibility_commands_reject_native_theme_feedback_options() {
    let directory = tempfile::tempdir().unwrap();
    for command in ["layout", "mmdc"] {
        for option in [
            ["--theme-report", "report.json"],
            ["--theme-portability", "require-portable"],
        ] {
            let output = support::run_before_stdin_close_in_dir(
                &[command, option[0], option[1]],
                Some(directory.path()),
                Duration::from_secs(5),
            );
            assert!(!output.status.success());
            assert!(!directory.path().join("report.json").exists());
        }
    }
    let output = run(directory.path(), &["render", "--help"]);
    success(&output);
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--theme-report") && help.contains("--theme-portability"));
}

#[cfg(feature = "png")]
#[test]
fn raw_svg_rejects_theme_feedback_before_acquiring_input() {
    let directory = tempfile::tempdir().unwrap();
    let output = support::run_before_stdin_close_in_dir(
        &[
            "render",
            "-",
            "--input-kind",
            "svg",
            "--format",
            "png",
            "--theme-report",
            "report.json",
        ],
        Some(directory.path()),
        Duration::from_secs(5),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!directory.path().join("report.json").exists());
}

#[cfg(feature = "ascii")]
#[test]
fn text_output_rejects_graphical_theme_feedback_before_acquiring_input() {
    let directory = tempfile::tempdir().unwrap();
    let output = support::run_before_stdin_close_in_dir(
        &[
            "render",
            "-",
            "--format",
            "ascii",
            "--theme-report",
            "report.json",
        ],
        Some(directory.path()),
        Duration::from_secs(5),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!directory.path().join("report.json").exists());
}
