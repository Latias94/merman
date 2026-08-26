#![cfg(feature = "ascii")]

use std::fs;
#[cfg(unix)]
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::fd::FromRawFd;
use std::process::{Command, Output, Stdio};

use merman_ascii_test_contracts::ascii_resource_boundary_contract;

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

fn run_ascii_with_resource_limit(limit_id: &str, max: u64, source: &str) -> Output {
    let override_arg = format!("{limit_id}={max}");
    run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-limit",
            &override_arg,
            "-",
        ],
        source,
    )
}

fn assert_plain_ascii_report(bytes: &[u8]) -> serde_json::Value {
    assert!(
        !bytes.windows(2).any(|window| window == b"\x1b["),
        "report bytes must not contain ANSI escapes: {bytes:?}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(bytes).expect("ASCII report should be one JSON artifact");
    assert_eq!(report["kind"], "ascii");
    assert_eq!(report["encoding"], "plain");
    assert_eq!(report["schema_version"], 2);
    let text = report["text"].as_str().expect("report text");
    assert!(!text.contains('\u{1b}'), "report text must be escape-free");
    report
}

fn assert_ascii_error_report(
    bytes: &[u8],
    expected_code: &str,
    expected_category: &str,
) -> serde_json::Value {
    assert!(
        !bytes.windows(2).any(|window| window == b"\x1b["),
        "error report bytes must not contain ANSI escapes: {bytes:?}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(bytes).expect("ASCII error report should be one JSON artifact");
    assert_eq!(report["kind"], "ascii_error");
    assert_eq!(report["encoding"], "plain");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["error"]["code"], expected_code);
    assert_eq!(report["error"]["category"], expected_category);
    assert!(
        report["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.contains('\u{1b}')),
        "error report message must be terminal-safe: {report}"
    );
    report
}

#[cfg(unix)]
fn open_pty() -> (fs::File, fs::File) {
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes both descriptors on success; each descriptor
    // is immediately transferred into exactly one owned File.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(
        result,
        0,
        "openpty failed: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: successful openpty returned two fresh, owned descriptors.
    unsafe { (fs::File::from_raw_fd(master), fs::File::from_raw_fd(slave)) }
}

#[cfg(unix)]
fn read_pty(mut master: fs::File) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => output.extend_from_slice(&buffer[..read]),
            Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
            Err(error) => panic!("read PTY output: {error}"),
        }
    }
    output
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
fn cli_ascii_diagnostic_does_not_echo_source_or_terminal_controls() {
    let source = "not-a-diagram\u{1b}]8;;https://example.invalid\u{7}link\u{202e}";

    let output = run_with_stdin(&["render", "--format", "ascii", "-"], source);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("No Mermaid diagram type detected"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains(source),
        "source leaked into stderr: {stderr:?}"
    );
    for control in ['\u{1b}', '\u{7}', '\u{202e}'] {
        assert!(
            !stderr.contains(control),
            "raw control {control:?} leaked into stderr: {stderr:?}"
        );
    }
}

#[test]
fn cli_clap_diagnostic_normalizes_untrusted_terminal_controls() {
    for value in ["12\rOWNED", "12\u{202e}34"] {
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                "ascii",
                "--ascii-max-width",
                value,
                "-",
            ],
            "flowchart TD\nA --> B",
        );

        assert!(!output.status.success());
        assert!(output.stdout.is_empty(), "failure must not write a payload");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains("invalid value"),
            "unexpected stderr:\n{stderr}"
        );
        for control in ['\r', '\u{202e}'] {
            assert!(
                !stderr.contains(control),
                "raw control {control:?} leaked into stderr: {stderr:?}"
            );
        }
    }
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
            "--output",
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

#[test]
fn ascii_report_is_plain_for_pipe_file_and_host_color_overrides() {
    let source = "flowchart LR\nA[Hello] --> B[World]";
    let piped = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-color",
            "auto",
            "-",
        ],
        source,
    );
    assert!(piped.status.success(), "stderr: {:?}", piped.stderr);
    assert_plain_ascii_report(&piped.stdout);

    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.txt");
    let report_arg = report_path.to_string_lossy().into_owned();
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args([
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--ascii-color",
            "auto",
            "--output",
            report_arg.as_str(),
            "-",
        ])
        .env("CLICOLOR_FORCE", "1")
        .env("COLORTERM", "truecolor")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(source.as_bytes())
        .expect("write stdin");
    let file_output = child.wait_with_output().expect("wait for CLI");
    assert!(
        file_output.status.success(),
        "stderr: {:?}",
        file_output.stderr
    );
    assert_plain_ascii_report(&fs::read(report_path).expect("read report file"));
}

#[test]
fn ascii_report_rejects_explicit_styled_output() {
    for color in ["ansi16", "ansi256", "truecolor", "html"] {
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                "unicode",
                "--ascii-report",
                "--ascii-color",
                color,
                "-",
            ],
            "flowchart LR\nA[Hello] --> B[World]",
        );

        assert!(
            !output.status.success(),
            "{color} report unexpectedly succeeded"
        );
        assert!(
            output.stdout.is_empty(),
            "failed report must emit no payload"
        );
        let report = assert_ascii_error_report(
            &output.stderr,
            "merman.cli.ascii_report.requires_plain",
            "usage",
        );
        assert_eq!(report["error"]["details"]["field"], "ascii_color");
    }
}

#[test]
fn ascii_report_width_failure_is_typed_json_without_partial_stdout() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-max-width",
            "5",
            "--ascii-overflow",
            "error",
            "-",
        ],
        "flowchart LR\nA[Hello] --> B[World]",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "failed report must not publish a partial success artifact"
    );
    let report =
        assert_ascii_error_report(&output.stderr, "merman.ascii.width_overflow", "content");
    assert_eq!(report["error"]["details"]["requested_max_width"], 5);
    assert!(
        report["error"]["details"]["actual_width"]
            .as_u64()
            .is_some_and(|actual| actual > 5)
    );
    assert_eq!(report["error"]["details"]["width_profile"], "unicode");
}

#[cfg(unix)]
#[test]
fn ascii_report_is_plain_on_a_real_terminal_stdout() {
    let (master, slave) = open_pty();
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args([
            "render",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-color",
            "auto",
            "-",
        ])
        .env("COLORTERM", "truecolor")
        .env("TERM", "xterm-256color")
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI with PTY stdout");
    let reader = std::thread::spawn(move || read_pty(master));
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"flowchart LR\nA[Hello] --> B[World]")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for CLI");
    let stdout = reader.join().expect("join PTY reader");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert_plain_ascii_report(&stdout);
}

#[test]
fn auto_color_honors_process_environment_before_rendering() {
    let source = "flowchart LR\nA[Hello] --> B[World]";
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let run = |no_color: &str| {
        let mut child = Command::new(exe)
            .args([
                "render",
                "--format",
                "unicode",
                "--ascii-color",
                "auto",
                "-",
            ])
            .env("CLICOLOR_FORCE", "1")
            .env("TERM", "xterm-256color")
            .env("NO_COLOR", no_color)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn CLI");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(source.as_bytes())
            .expect("write stdin");
        child.wait_with_output().expect("wait for CLI")
    };

    let empty_no_color = run("");
    assert!(
        empty_no_color.status.success(),
        "stderr: {:?}",
        empty_no_color.stderr
    );
    assert!(
        empty_no_color
            .stdout
            .windows(2)
            .any(|bytes| bytes == b"\x1b["),
        "an empty NO_COLOR value must not disable forced color"
    );

    let set_no_color = run("1");
    assert!(
        set_no_color.status.success(),
        "stderr: {:?}",
        set_no_color.stderr
    );
    assert!(
        !set_no_color
            .stdout
            .windows(2)
            .any(|bytes| bytes == b"\x1b["),
        "a non-empty NO_COLOR value must disable color"
    );
}

#[test]
fn ascii_report_resource_failure_preserves_stable_limit_details() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--resource-limit",
            "max_ascii_output_bytes=1",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let report =
        assert_ascii_error_report(&output.stderr, "merman.ascii.resource_limit", "content");
    let resource = &report["error"]["details"]["resource_limit"];
    assert_eq!(resource["id"], "max_ascii_output_bytes");
    assert_eq!(resource["phase"], "ascii_output");
    assert_eq!(resource["maximum"], 1);
    assert_eq!(resource["cause"], "ceiling");
    assert_eq!(resource["profile"], "trusted-native");
    assert!(resource["actual"].as_u64().is_some_and(|actual| actual > 1));
}

#[test]
fn ascii_report_source_limit_failure_preserves_stable_limit_details() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--resource-limit",
            "max_source_bytes=5",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let report =
        assert_ascii_error_report(&output.stderr, "merman.input.resource_limit", "content");
    let resource = &report["error"]["details"]["resource_limit"];
    assert_eq!(resource["id"], "max_source_bytes");
    assert_eq!(resource["phase"], "source");
    assert_eq!(resource["maximum"], 5);
    assert_eq!(resource["cause"], "ceiling");
    assert_eq!(resource["profile"], "trusted-native");
    assert!(resource["actual"].as_u64().is_some_and(|actual| actual > 5));
}

#[test]
fn ascii_report_model_limit_failure_preserves_stable_limit_details() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--resource-limit",
            "max_model_items=1",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let report =
        assert_ascii_error_report(&output.stderr, "merman.input.resource_limit", "content");
    let resource = &report["error"]["details"]["resource_limit"];
    assert_eq!(resource["id"], "max_model_items");
    assert_eq!(resource["phase"], "layout_model");
    assert_eq!(resource["maximum"], 1);
    assert_eq!(resource["cause"], "ceiling");
    assert_eq!(resource["profile"], "trusted-native");
    assert!(resource["actual"].as_u64().is_some_and(|actual| actual > 1));
}

#[cfg(unix)]
#[test]
fn auto_color_observes_a_real_terminal_stdout() {
    let (master, slave) = open_pty();
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args([
            "render",
            "--format",
            "unicode",
            "--ascii-color",
            "auto",
            "-",
        ])
        .env("TERM", "xterm-256color")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI with PTY stdout");
    let reader = std::thread::spawn(move || read_pty(master));
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"flowchart LR\nA[Hello] --> B[World]")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for CLI");
    let stdout = reader.join().expect("join PTY reader");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        stdout.windows(2).any(|bytes| bytes == b"\x1b["),
        "terminal stdout should enable ANSI color: {stdout:?}"
    );
}

#[test]
fn cli_applies_the_selected_resource_profile_to_text_output() {
    let input = format!(
        "%% {}\nflowchart TD\nA[Hello] --> B[World]\n",
        "x".repeat(1024 * 1024)
    );

    let constrained = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-profile",
            "constrained",
            "-",
        ],
        &input,
    );
    assert!(!constrained.status.success());
    let stderr = String::from_utf8(constrained.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("max_source_bytes"), "{stderr}");

    let trusted = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-profile",
            "trusted-native",
            "-",
        ],
        &input,
    );
    assert!(trusted.status.success(), "stderr: {:?}", trusted.stderr);
}

#[test]
fn cli_reports_a_representative_ascii_resource_boundary_with_stable_typed_context() {
    let case = ascii_resource_boundary_contract()
        .transport_representatives
        .cli_trusted_native;
    let exact = case.exact;

    let exact_output = run_ascii_with_resource_limit(&case.id, exact, &case.source);
    assert!(
        exact_output.status.success(),
        "exact {} boundary failed: {}",
        case.id,
        String::from_utf8_lossy(&exact_output.stderr),
    );
    assert!(
        !exact_output.stdout.is_empty(),
        "{} produced no output",
        case.id
    );

    let below_output = run_ascii_with_resource_limit(&case.id, exact - 1, &case.source);
    assert!(
        !below_output.status.success(),
        "one-below {} boundary unexpectedly succeeded",
        case.id
    );
    let stderr = String::from_utf8(below_output.stderr).expect("stderr should be utf8");
    let expected_prefix = format!(
        "ASCII resource limit `{}` exceeded during `{}`: actual {}",
        case.id, case.phase, exact
    );
    assert!(stderr.contains(&expected_prefix), "{stderr}");
    assert!(
        stderr.contains(&format!("maximum {} (profile `trusted-native`)", exact - 1)),
        "{stderr}"
    );
}

#[test]
fn cli_rejects_duplicate_generic_and_legacy_ascii_grid_overrides() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-limit",
            "max_ascii_grid_cells=100",
            "--ascii-max-grid-cells",
            "100",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("specified by both"), "{stderr}");
}

#[cfg(feature = "svg")]
#[test]
fn text_admission_ignores_svg_layout_limits_in_combined_builds() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-profile",
            "constrained",
            "--resource-limit",
            "max_layout_work_units=10000000",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
}
