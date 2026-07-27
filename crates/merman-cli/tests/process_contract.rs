mod support;

use std::fs;
use std::process::Command;
use support::{exit_code, run_with_closed_stdout, run_with_stdin, run_with_stdin_in_dir};

#[test]
fn missing_input_is_a_usage_error_without_output_side_effects() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "missing.mmd", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert_eq!(exit_code(output.status), 2);
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Input file \"missing.mmd\" doesn't exist"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!tmp.path().join("out.svg").exists());
}

#[test]
fn process_exit_classes_remain_distinct() {
    let success = run_with_stdin(&["detect", "-"], "flowchart LR\nA-->B\n");
    assert_eq!(exit_code(success.status), 0, "stderr: {:?}", success.stderr);

    let content_failure = run_with_stdin(&["parse", "-"], "not a Mermaid diagram\n");
    assert_eq!(
        exit_code(content_failure.status),
        1,
        "stderr: {:?}",
        content_failure.stderr
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let operational_failure = Command::new(exe)
        .args(["parse", tmp.path().to_string_lossy().as_ref()])
        .output()
        .expect("run cli");
    assert_eq!(
        exit_code(operational_failure.status),
        3,
        "stderr: {:?}",
        operational_failure.stderr
    );
}

#[test]
fn informational_diagnostics_do_not_mix_with_stdout() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join("input.md"),
        "# No diagrams\n\nPlain text.\n",
    )
    .expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.md", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stdout.is_empty(),
        "non-payload logs must not be written to stdout"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("No mermaid charts found in Markdown input"),
        "diagnostic should be written to stderr:\n{stderr}"
    );
}

#[test]
fn closed_stdout_is_successful_for_every_payload_writer() {
    let cases: &[(&[&str], Option<&[u8]>)] = &[
        (&["-i", "-", "-o", "-"], Some(b"flowchart LR\nA-->B\n")),
        (&["parse", "-"], Some(b"flowchart LR\nA-->B\n")),
        (&["completion", "bash"], None),
        (&["lint-rules"], None),
        (
            &["lint", "--markdown", "--format", "text", "-"],
            Some(b"before\n```mermaid\nflowchart TD\nA -->\n```\nafter\n"),
        ),
    ];

    for (args, input) in cases {
        let output = run_with_closed_stdout(args, *input);
        assert!(
            output.status.success(),
            "closed stdout should be normal pipe termination: {:?}",
            output.stderr
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            !stderr.contains("panicked")
                && !stderr.contains("I/O error")
                && !stderr.contains("Broken pipe"),
            "closed stdout should not print a diagnostic:\n{stderr}"
        );
    }
}

#[test]
fn remote_icon_loading_requires_explicit_network_authorization() {
    let icon_arg = "remote#https://example.invalid/icons.json";
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "--iconPacksNamesAndUrls", icon_arg],
        "flowchart TD\nA@{ icon: \"remote:cloud\", label: \"Cloud\" }\n",
    );

    assert_eq!(exit_code(output.status), 2);
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--allow-network") && stderr.to_ascii_lowercase().contains("icon pack"),
        "unexpected stderr:\n{stderr}"
    );

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
    assert_eq!(exit_code(output.status), 2);
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("@iconify-json/missing-test-pack")
            && stderr.contains("--allow-network")
            && stderr.contains("node_modules"),
        "local miss must not silently fetch:\n{stderr}"
    );
}

#[test]
fn quiet_parallel_markdown_preserves_source_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join("input.md"),
        "one\n```mermaid\nflowchart LR\nA1-->B1\n```\n\
         two\n```mermaid\nflowchart LR\nA2-->B2\n```\n\
         three\n```mermaid\nflowchart LR\nA3-->B3\n```\n",
    )
    .expect("write markdown");

    let output = run_with_stdin_in_dir(
        &["-i", "input.md", "-o", "out.md", "--jobs", "2", "-q"],
        "",
        Some(tmp.path()),
    );
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "quiet mode must suppress informational diagnostics: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let rewritten = fs::read_to_string(tmp.path().join("out.md")).expect("read markdown");
    let first = rewritten.find("./out-1.svg").expect("first image");
    let second = rewritten.find("./out-2.svg").expect("second image");
    let third = rewritten.find("./out-3.svg").expect("third image");
    assert!(
        first < second && second < third,
        "Markdown image order must follow source order:\n{rewritten}"
    );
}
