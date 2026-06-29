#![cfg(feature = "ascii")]

use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

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
fn cli_renders_unicode_ascii_output_to_stdout() {
    let output = run_with_stdin(
        &["render", "--format", "unicode", "-"],
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("┌"));
    assert!(stdout.contains("Hello"));
    assert!(stdout.contains("►"));
}

#[test]
fn cli_renders_sequence_mirrored_actors_when_requested() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--sequence-mirror-actors",
            "-",
        ],
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("┌─┴─┐     ┌─┴─┐"),
        "expected mirrored bottom participant boxes:\n{stdout}"
    );
}

#[test]
fn cli_allows_charset_override_for_text_output() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--ascii-charset",
            "ascii",
            "-",
        ],
        "flowchart TD\nA[Hello] --> B[World]",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("+"), "{stdout}");
    assert!(!stdout.contains("┌"), "{stdout}");
}

#[test]
fn cli_allows_xychart_plot_dimension_overrides() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--xychart-vertical-plot-height",
            "8",
            "--xychart-category-band-width",
            "5",
            "-",
        ],
        r#"xychart
title "Sales"
x-axis [Jan, Feb]
y-axis 0 --> 10
bar [2, 8]
"#,
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Sales"), "{stdout}");
    assert!(stdout.contains("Jan"), "{stdout}");
}

#[test]
fn cli_renders_shipped_reference_diagram_families_to_stdout() {
    let cases = [
        ("classDiagram\nclass Animal", "Animal"),
        ("erDiagram\nCUSTOMER", "CUSTOMER"),
        (
            r#"xychart
title "Sales"
x-axis [Jan, Feb]
y-axis 0 --> 10
bar [2, 8]
"#,
            "Sales",
        ),
    ];

    for (input, expected) in cases {
        let output = run_with_stdin(&["render", "--format", "ascii", "-"], input);

        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(
            stdout.contains(expected),
            "expected {expected:?} in stdout:\n{stdout}"
        );
    }
}

#[test]
fn cli_renders_plain_ascii_output_to_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("diagram.txt");
    let out_arg = out.to_string_lossy().into_owned();

    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--out",
            out_arg.as_str(),
            "-",
        ],
        "flowchart TB\nsubgraph one\nA((Start)) -- go --> B[(DB)]\nend",
    );
    assert!(output.status.success(), "stderr: {:?}", output.stderr);

    let text = fs::read_to_string(out).expect("read ascii output");
    assert!(text.contains("one"));
    assert!(text.contains("Start"));
    assert!(text.contains("go"));
    assert!(text.contains("DB"));
    assert!(text.contains("/"));
}
