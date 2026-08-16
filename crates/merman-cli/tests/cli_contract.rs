mod support;

use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::process::Command;
use std::time::Duration;
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
        #[cfg(feature = "rustdoc")]
        assert!(
            stdout.contains("rustdoc"),
            "compiled help should expose rustdoc:\n{stdout}"
        );
        #[cfg(not(feature = "rustdoc"))]
        assert!(
            !stdout.contains("rustdoc"),
            "a build without the rustdoc feature must not advertise rustdoc:\n{stdout}"
        );
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
    assert_eq!(
        stdout,
        format!("merman-cli {}\n", env!("CARGO_PKG_VERSION")),
        "the version line is a machine-readable release contract"
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

#[cfg(unix)]
#[test]
fn informational_commands_do_not_require_a_live_working_directory() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    for args in [
        vec!["--help"],
        vec!["--version"],
        vec!["capabilities", "--json"],
        vec!["completion", "bash"],
        vec!["lint-rules", "--format", "json"],
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let removed = tmp.path().join("removed-cwd");
        fs::create_dir(&removed).expect("create cwd");
        let output = Command::new("/bin/sh")
            .args([
                "-c",
                "cd \"$REMOVED_CWD\" && rmdir \"$REMOVED_CWD\" && exec \"$MERMAN_EXE\" \"$@\"",
                "merman-cli-info-test",
            ])
            .args(&args)
            .env("REMOVED_CWD", &removed)
            .env("MERMAN_EXE", exe)
            .output()
            .expect("run informational command from a removed cwd");
        assert!(
            output.status.success(),
            "{args:?} should not inspect cwd: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !output.stdout.is_empty(),
            "{args:?} should retain its stdout contract"
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
    #[cfg(feature = "rustdoc")]
    assert!(
        stdout.contains("Documentation:"),
        "root help should group the compiled Rustdoc command:\n{stdout}"
    );
    #[cfg(not(feature = "rustdoc"))]
    assert!(
        !stdout.contains("Documentation:"),
        "root help must omit an empty Documentation group:\n{stdout}"
    );
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
        "--presentation-profile",
        "--operation-timeout-ms",
    ] {
        assert!(
            stdout.contains(flag),
            "mmdc help should expose compatibility option {flag}:\n{stdout}"
        );
    }
}

#[test]
fn zero_operation_timeout_returns_structured_cancellation_without_output() {
    for (args, input) in [
        (
            vec!["render", "-", "--operation-timeout-ms", "0"],
            "flowchart LR\nA-->B\n",
        ),
        (
            vec!["mmdc", "-i", "-", "-o", "-", "--operation-timeout-ms", "0"],
            "flowchart LR\nA-->B\n",
        ),
    ] {
        let output = run_with_stdin(&args, input);
        assert_eq!(support::exit_code(output.status), 1, "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?} wrote a partial payload");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains("operation cancelled during admission: deadline_exceeded"),
            "unexpected cancellation for {args:?}:\n{stderr}"
        );
    }
}

#[test]
fn operation_deadline_interrupts_an_open_stdin_pipe() {
    let output = support::run_before_stdin_close_in_dir(
        &["render", "-", "--operation-timeout-ms", "20"],
        None,
        Duration::from_secs(2),
    );

    assert_eq!(support::exit_code(output.status), 1);
    assert!(
        output.stdout.is_empty(),
        "deadline cancellation must not publish a payload"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("operation cancelled during admission: deadline_exceeded"),
        "unexpected cancellation:\n{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn sigint_requests_cooperative_cancellation_before_publication() {
    use std::io::{BufRead, BufReader, Read};
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Instant;

    let tmp = tempfile::tempdir().expect("tempdir");
    let output_path = tmp.path().join("out.svg");
    let mut child = Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .arg("mmdc")
        .arg("-o")
        .arg(&output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn controlled CLI");

    let stderr = child.stderr.take().expect("stderr pipe");
    let (ready_sender, ready_receiver) = mpsc::channel();
    let stderr_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut text = String::new();
        let first_line = reader.read_line(&mut text).map(|_| text.clone());
        let _ = ready_sender.send(first_line);
        let _ = reader.read_to_string(&mut text);
        text
    });

    let first_line = match ready_receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(first_line) => first_line.expect("read readiness warning"),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stderr_reader.join();
            panic!("CLI did not reach controlled input acquisition: {error}");
        }
    };
    assert!(
        first_line.contains("No input file specified"),
        "unexpected readiness warning: {first_line:?}"
    );

    let child_pid = libc::pid_t::try_from(child.id()).expect("child pid should fit pid_t");
    // SAFETY: `child_pid` names the live child process and SIGINT is a valid signal number.
    let signal_result = unsafe { libc::kill(child_pid, libc::SIGINT) };
    assert_eq!(signal_result, 0, "send SIGINT to controlled CLI");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if child.try_wait().expect("poll controlled CLI").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill blocked controlled CLI");
            let _ = child.wait();
            let stderr = stderr_reader.join().expect("join stderr reader");
            panic!("SIGINT did not interrupt the open stdin pipe:\n{stderr}");
        }
        thread::sleep(Duration::from_millis(10));
    }

    let output = child.wait_with_output().expect("wait for controlled CLI");
    let stderr = stderr_reader.join().expect("join stderr reader");
    assert_eq!(support::exit_code(output.status), 1, "stderr:\n{stderr}");
    assert!(output.stdout.is_empty(), "SIGINT must not emit a payload");
    assert!(
        !output_path.exists(),
        "SIGINT must be observed before atomic publication"
    );
    assert!(
        stderr.contains("operation cancelled during") && stderr.contains(": requested"),
        "SIGINT must retain structured cancellation details:\n{stderr}"
    );
}

#[test]
fn batch_zero_deadline_precedes_numbered_preflight_and_preserves_existing_output() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let generated = tmp.path().join("generated");
    fs::create_dir(&generated).expect("create existing output directory");
    fs::write(generated.join("README.md"), "complete previous document")
        .expect("write previous document");
    fs::write(
        generated.join(".merman-manifest.json"),
        "complete previous manifest",
    )
    .expect("write previous manifest");
    fs::create_dir(generated.join("README-1.svg")).expect("create invalid numbered output entry");

    let output = support::run_with_stdin_in_dir(
        &[
            "batch",
            "-",
            "--stdin-file-name",
            "README.md",
            "--output-dir",
            "generated",
            "--operation-timeout-ms",
            "0",
        ],
        "```mermaid\nflowchart LR\nA-->B\n```\n",
        Some(tmp.path()),
    );

    assert_eq!(support::exit_code(output.status), 1);
    assert!(output.stdout.is_empty(), "deadline must not write stdout");
    assert_eq!(
        fs::read_to_string(generated.join("README.md")).expect("read previous document"),
        "complete previous document"
    );
    assert_eq!(
        fs::read_to_string(generated.join(".merman-manifest.json"))
            .expect("read previous manifest"),
        "complete previous manifest"
    );
    assert!(generated.join("README-1.svg").is_dir());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("operation cancelled during admission: deadline_exceeded"),
        "unexpected cancellation:\n{stderr}"
    );
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
fn native_theme_values_match_the_compiled_runtime_catalog() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let rejected = Command::new(exe)
        .args(["render", "missing.mmd", "--theme", "not-a-runtime-theme"])
        .output()
        .expect("run cli");

    assert_eq!(support::exit_code(rejected.status), 2);
    assert!(
        rejected.stdout.is_empty(),
        "failure must not write a payload"
    );
    let stderr = String::from_utf8(rejected.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("not-a-runtime-theme") && !stderr.contains("missing.mmd:"),
        "theme validation must precede input acquisition:\n{stderr}"
    );
    for theme in merman::supported_themes() {
        assert!(
            stderr.contains(theme),
            "native theme validation should list `{theme}`:\n{stderr}"
        );
    }

    let accepted = run_with_stdin(
        &["render", "--theme", "redux", "-"],
        "flowchart LR\nA-->B\n",
    );
    assert!(
        accepted.status.success(),
        "compiled runtime theme should be accepted: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
}

#[test]
fn presentation_profile_values_match_the_compiled_runtime_catalog() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    for command in ["render", "mmdc"] {
        let mut rejected = Command::new(exe);
        rejected.args([command, "--presentation-profile", "not-a-runtime-profile"]);
        if command == "mmdc" {
            rejected.args(["-i", "missing.mmd"]);
        } else {
            rejected.arg("missing.mmd");
        }
        let rejected = rejected.output().expect("run cli");

        assert_eq!(support::exit_code(rejected.status), 2);
        assert!(
            rejected.stdout.is_empty(),
            "failure must not write a payload"
        );
        let stderr = String::from_utf8(rejected.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains("not-a-runtime-profile") && !stderr.contains("missing.mmd:"),
            "profile validation must precede input acquisition:\n{stderr}"
        );
        for descriptor in merman::svg::presentation_profile_descriptors() {
            assert!(
                stderr.contains(descriptor.id()),
                "profile validation should list `{}`:\n{stderr}",
                descriptor.id()
            );
        }
    }

    let accepted = run_with_stdin(
        &[
            "render",
            "--presentation-profile",
            "merman-modern",
            "--format",
            "svg",
            "-",
        ],
        "flowchart LR\nA-->B\n",
    );
    assert!(
        accepted.status.success(),
        "compiled presentation profile should be accepted: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let svg = String::from_utf8(accepted.stdout).expect("stdout should be utf8");
    assert!(
        svg.contains(
            r#".flowchart-link[data-look="neo"]{stroke-linecap:round;stroke-linejoin:round;}"#
        ),
        "the profile should activate its typed Flowchart presentation policy: {svg}"
    );

    let mmdc = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--presentation-profile",
            "merman-modern",
        ],
        "flowchart LR\nA-->B\n",
    );
    assert!(
        mmdc.status.success(),
        "mmdc presentation profile should be accepted: {}",
        String::from_utf8_lossy(&mmdc.stderr)
    );
    let svg = String::from_utf8(mmdc.stdout).expect("stdout should be utf8");
    assert!(
        svg.contains("fill:#F8FAFC")
            && svg.contains(
                r#".flowchart-link[data-look="neo"]{stroke-linecap:round;stroke-linejoin:round;}"#
            ),
        "implicit mmdc defaults must not override an explicit presentation profile: {svg}"
    );
}

#[test]
fn presentation_profile_composes_with_config_regardless_of_argument_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("mermaid.json");
    fs::write(
        &config,
        r##"{
  "theme": "base",
  "look": "classic",
  "flowchart": {"defaultRenderer": "dagre-wrapper"},
  "themeVariables": {
    "mainBkg": "#123456",
    "nodeBorder": "#654321"
  }
}"##,
    )
    .expect("write config");
    let path = config.to_string_lossy();
    let source = "flowchart LR\nA-->|label|B\n";

    let profile_first = run_with_stdin(
        &[
            "render",
            "--presentation-profile",
            "merman-modern",
            "--config-file",
            path.as_ref(),
            "--format",
            "svg",
            "-",
        ],
        source,
    );
    let config_first = run_with_stdin(
        &[
            "render",
            "--config-file",
            path.as_ref(),
            "--presentation-profile",
            "merman-modern",
            "--format",
            "svg",
            "-",
        ],
        source,
    );

    assert!(
        profile_first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&profile_first.stderr)
    );
    assert!(
        config_first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&config_first.stderr)
    );
    assert_eq!(profile_first.stdout, config_first.stdout);
    let svg = String::from_utf8(profile_first.stdout).expect("stdout should be utf8");
    assert!(svg.contains("#123456"), "explicit config should win: {svg}");
    assert!(
        !svg.contains(r#"data-look="neo""#),
        "explicit Mermaid look should override the profile default: {svg}"
    );
    assert!(
        svg.contains(r#"rx="4" ry="4""#),
        "the independent private Flowchart aspect should remain active: {svg}"
    );
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
fn invalid_render_configuration_precedes_primary_input_acquisition() {
    const WIDTH_OUTSIDE_SUPPORTED_INTEGER_RANGE: &str = "18446744073709551616";
    let tmp = tempfile::tempdir().expect("tempdir");
    let cases: &[(&str, &[&str], &str)] = &[
        (
            "boundary local midnight",
            &[
                "render",
                "missing.mmd",
                "--fixed-today=-2147483648-01-01",
                "--fixed-local-offset-minutes",
                "1439",
            ],
            "local datetime",
        ),
        (
            "XYChart vertical plot height",
            &[
                "render",
                "missing.mmd",
                "--format",
                "ascii",
                "--xychart-vertical-plot-height",
                "1",
            ],
            "at least 2",
        ),
        (
            "XYChart horizontal plot width",
            &[
                "render",
                "missing.mmd",
                "--format",
                "ascii",
                "--xychart-horizontal-plot-width",
                "1",
            ],
            "at least 2",
        ),
        (
            "pdfFit viewport width",
            &[
                "mmdc",
                "-i",
                "missing.mmd",
                "-o",
                "out.pdf",
                "--pdfFit",
                "--width",
                WIDTH_OUTSIDE_SUPPORTED_INTEGER_RANGE,
            ],
            "expected a positive integer",
        ),
    ];

    for (label, args, expected) in cases {
        let output = support::run_with_stdin_in_dir(args, "", Some(tmp.path()));
        assert_eq!(
            support::exit_code(output.status),
            2,
            "{label}: stderr: {:?}",
            output.stderr
        );
        assert!(output.stdout.is_empty(), "{label}");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains(expected),
            "{label} should report `{expected}` before opening input:\n{stderr}"
        );
        assert!(
            !stderr.contains("missing.mmd"),
            "{label} must precede missing input acquisition:\n{stderr}"
        );
    }
}

#[test]
fn invalid_render_configuration_does_not_wait_for_stdin() {
    const WIDTH_OUTSIDE_SUPPORTED_INTEGER_RANGE: &str = "18446744073709551616";
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("puppeteer.json"), "{not json")
        .expect("write invalid Puppeteer config");
    let cases: &[(&str, &[&str], &str)] = &[
        (
            "boundary local midnight",
            &[
                "render",
                "-",
                "--fixed-today=-2147483648-01-01",
                "--fixed-local-offset-minutes",
                "1439",
            ],
            "local datetime",
        ),
        (
            "XYChart dimensions",
            &[
                "render",
                "-",
                "--format",
                "ascii",
                "--xychart-vertical-plot-height",
                "1",
            ],
            "at least 2",
        ),
        (
            "Puppeteer configuration JSON",
            &["mmdc", "-i", "-", "-o", "-", "-p", "puppeteer.json"],
            "JSON error",
        ),
        (
            "pdfFit viewport width",
            &[
                "mmdc",
                "-i",
                "-",
                "-o",
                "out.pdf",
                "--pdfFit",
                "--width",
                WIDTH_OUTSIDE_SUPPORTED_INTEGER_RANGE,
            ],
            "expected a positive integer",
        ),
    ];

    for (label, args, expected) in cases {
        let output =
            support::run_before_stdin_close_in_dir(args, Some(tmp.path()), Duration::from_secs(2));
        assert_eq!(
            support::exit_code(output.status),
            2,
            "{label}: stderr: {:?}",
            output.stderr
        );
        assert!(output.stdout.is_empty(), "{label}");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains(expected),
            "{label} should report `{expected}` before reading stdin:\n{stderr}"
        );
    }
}

#[test]
fn native_render_rejects_each_irrelevant_output_option_before_input_acquisition() {
    let cases: &[(&str, &str, &[&str])] = &[
        ("raster scale on SVG", "svg", &["--scale", "2"]),
        (
            "raster fit width on SVG",
            "svg",
            &["--raster-fit-width", "10"],
        ),
        (
            "raster fit height on SVG",
            "svg",
            &["--raster-fit-height", "10"],
        ),
        (
            "raster max width on SVG",
            "svg",
            &["--raster-max-width", "10"],
        ),
        (
            "raster max height on SVG",
            "svg",
            &["--raster-max-height", "10"],
        ),
        (
            "raster max pixels on SVG",
            "svg",
            &["--raster-max-pixels", "100"],
        ),
        ("unbounded raster on SVG", "svg", &["--raster-unbounded"]),
        (
            "PDF filter scale on SVG",
            "svg",
            &["--pdf-filter-scale", "2"],
        ),
        (
            "PDF filter pixels on SVG",
            "svg",
            &["--pdf-max-filter-image-pixels", "100"],
        ),
        (
            "unbounded PDF filters on SVG",
            "svg",
            &["--pdf-filter-images-unbounded"],
        ),
        (
            "embedded image bytes on SVG",
            "svg",
            &["--embedded-image-max-bytes", "100"],
        ),
        (
            "aggregate embedded bytes on SVG",
            "svg",
            &["--embedded-image-max-total-bytes", "100"],
        ),
        (
            "embedded image pixels on SVG",
            "svg",
            &["--embedded-image-max-pixels", "100"],
        ),
        (
            "aggregate embedded pixels on SVG",
            "svg",
            &["--embedded-image-max-total-pixels", "100"],
        ),
        (
            "unbounded embedded images on SVG",
            "svg",
            &["--embedded-images-unbounded"],
        ),
        (
            "sequence mirror on SVG",
            "svg",
            &["--sequence-mirror-actors"],
        ),
        (
            "ASCII charset on SVG",
            "svg",
            &["--ascii-charset", "unicode"],
        ),
        (
            "ASCII width profile on SVG",
            "svg",
            &["--ascii-width-profile", "cjk"],
        ),
        (
            "ASCII direction on SVG",
            "svg",
            &["--ascii-direction", "left-right"],
        ),
        ("ASCII color on SVG", "svg", &["--ascii-color", "plain"]),
        (
            "XYChart height on SVG",
            "svg",
            &["--xychart-vertical-plot-height", "10"],
        ),
        (
            "XYChart category width on SVG",
            "svg",
            &["--xychart-category-band-width", "10"],
        ),
        (
            "XYChart plot width on SVG",
            "svg",
            &["--xychart-horizontal-plot-width", "10"],
        ),
        (
            "ASCII grid limit on SVG",
            "svg",
            &["--ascii-max-grid-cells", "100"],
        ),
        ("SVG pipeline on PNG", "png", &["--svg-pipeline", "parity"]),
        ("background on text", "ascii", &["--background", "white"]),
        ("CSS on text", "ascii", &["--css-file", "missing.css"]),
        (
            "text measurer on text",
            "ascii",
            &["--text-measurer", "deterministic"],
        ),
        (
            "presentation profile on text",
            "ascii",
            &["--presentation-profile", "merman-modern"],
        ),
        (
            "math renderer on text",
            "ascii",
            &["--math-renderer", "none"],
        ),
        ("container width on text", "ascii", &["--width", "100"]),
        ("container height on text", "ascii", &["--height", "100"]),
        ("SVG id on text", "ascii", &["--svg-id", "diagram"]),
        (
            "hand-drawn seed on text",
            "ascii",
            &["--hand-drawn-seed", "1"],
        ),
        (
            "local icon pack on text",
            "ascii",
            &["--icon-pack", "missing.json"],
        ),
        (
            "named icon source on text",
            "ascii",
            &["--icon-pack-source", "test#missing.json"],
        ),
        ("network icons on text", "ascii", &["--allow-network"]),
        (
            "private network icons on text",
            "ascii",
            &["--allow-network", "--allow-private-network"],
        ),
    ];

    for (label, format, extra) in cases {
        let mut args = vec!["render", "missing.mmd", "--format", *format];
        args.extend_from_slice(extra);
        let output = run_with_stdin(&args, "");
        assert_eq!(
            support::exit_code(output.status),
            2,
            "{label}: stderr: {:?}",
            output.stderr
        );
        assert!(output.stdout.is_empty(), "{label}");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            !stderr.contains("missing.mmd") && !stderr.contains("missing.css"),
            "{label} must fail before input acquisition:\n{stderr}"
        );
    }
}

#[test]
fn raw_svg_rejects_each_mermaid_only_option_before_input_acquisition() {
    let cases: &[(&str, &[&str])] = &[
        ("suppressed Mermaid errors", &["--suppress-errors"]),
        ("Mermaid configuration", &["--config-file", "missing.json"]),
        ("Mermaid theme", &["--theme", "dark"]),
        ("native runtime", &["--runtime", "native"]),
        ("system clock", &["--system-clock"]),
        ("system timezone", &["--system-timezone"]),
        ("system random", &["--system-random"]),
        ("system timing", &["--system-timing"]),
        ("fixed date", &["--fixed-today", "2026-07-28"]),
        (
            "fixed local offset",
            &["--fixed-local-offset-minutes", "480"],
        ),
        ("text measurer", &["--text-measurer", "deterministic"]),
        (
            "presentation profile",
            &["--presentation-profile", "merman-modern"],
        ),
        ("math renderer", &["--math-renderer", "none"]),
        ("container width", &["--width", "100"]),
        ("container height", &["--height", "100"]),
        ("SVG id", &["--svg-id", "diagram"]),
        ("hand-drawn seed", &["--hand-drawn-seed", "1"]),
        ("local icon pack", &["--icon-pack", "missing.json"]),
        (
            "named icon source",
            &["--icon-pack-source", "test#missing.json"],
        ),
        ("network icons", &["--allow-network"]),
        (
            "private network icons",
            &["--allow-network", "--allow-private-network"],
        ),
    ];

    for (label, extra) in cases {
        let mut args = vec!["render", "missing.svg", "--format", "png"];
        args.extend_from_slice(extra);
        let output = run_with_stdin(&args, "");
        assert_eq!(
            support::exit_code(output.status),
            2,
            "{label}: stderr: {:?}",
            output.stderr
        );
        assert!(output.stdout.is_empty(), "{label}");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            !stderr.contains("missing.svg") && !stderr.contains("missing.json"),
            "{label} must fail before input acquisition:\n{stderr}"
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
        ["detect", "--config-file", "missing.json", "-"].as_slice(),
        ["detect", "--theme", "dark", "-"].as_slice(),
        ["detect", "--runtime", "deterministic", "-"].as_slice(),
        ["detect", "--resource-limit", "max_css_bytes=1", "-"].as_slice(),
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
fn developer_short_help_points_to_advanced_controls() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    for command in ["detect", "parse", "layout"] {
        let output = Command::new(exe)
            .args([command, "-h"])
            .output()
            .expect("run short help");
        assert!(output.status.success(), "{command}: {:?}", output.stderr);
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(
            stdout.contains(&format!("merman-cli {command} --help")),
            "{command} short help should expose the long-help path:\n{stdout}"
        );
        assert!(
            !stdout.contains("--resource-limit"),
            "{command} short help should hide resource controls:\n{stdout}"
        );
    }
}

#[test]
fn detect_still_handles_frontmatter_and_directives_without_configuration_flags() {
    let source =
        "---\ntitle: Directed\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart LR\nA-->B\n";
    let output = run_with_stdin(&["detect", "-"], source);

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be utf8"),
        "flowchart-v2\n"
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
    assert_eq!(payload["schema_version"], 2);
    assert_eq!(payload["cli_contract_version"], 4);
    assert_eq!(payload["package"]["name"], "merman-cli");
    assert_eq!(payload["package"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        payload["compatibility"]["mermaid"],
        merman::baseline::PINNED_MERMAID_BASELINE_VERSION
    );
    assert_eq!(
        payload["compatibility"]["mmdc"],
        merman::baseline::PINNED_MERMAID_CLI_VERSION
    );
    assert_eq!(payload["descriptor"]["schema_version"], 1);
    assert!(
        payload["descriptor"]["digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:")),
        "missing descriptor digest: {payload}"
    );
    let command_ids = payload["commands"]
        .as_array()
        .expect("commands should be an array")
        .iter()
        .map(|value| value.as_str().expect("command ids should be strings"))
        .collect::<Vec<_>>();
    assert!(
        command_ids.windows(2).all(|pair| pair[0] < pair[1]),
        "command ids must be sorted and unique: {command_ids:?}"
    );
    for command in ["batch", "capabilities", "completion", "mmdc", "render"] {
        assert!(
            command_ids.contains(&command),
            "missing compiled command {command}: {payload}"
        );
    }
    #[cfg(feature = "rustdoc")]
    assert!(
        command_ids.contains(&"rustdoc"),
        "missing compiled rustdoc command"
    );
    #[cfg(not(feature = "rustdoc"))]
    assert!(
        !command_ids.contains(&"rustdoc"),
        "a build without the rustdoc feature must not report rustdoc"
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
