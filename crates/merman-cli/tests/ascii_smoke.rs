#![cfg(feature = "ascii")]

use std::fs;
#[cfg(unix)]
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::fd::FromRawFd;
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
