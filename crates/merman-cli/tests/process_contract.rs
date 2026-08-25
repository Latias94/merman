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
        .args(["mmdc", "-i", "missing.mmd", "-o", "out.svg"])
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
fn root_requires_an_explicit_subcommand_without_side_effects() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .output()
        .expect("run cli");

    assert_eq!(exit_code(output.status), 2);
    assert!(
        output.stdout.is_empty(),
        "missing command help belongs on stderr"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Usage:")
            && stderr.contains("render")
            && stderr.contains("batch")
            && stderr.contains("mmdc"),
        "root usage should direct users to an explicit workflow:\n{stderr}"
    );
    assert!(
        !tmp.path().join("out.svg").exists(),
        "root argument validation must precede creating output"
    );
}

#[test]
fn root_mmdc_options_forward_without_compatibility_noise() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), "flowchart LR\nA-->B\n").expect("write input");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe.as_os_str())
        .current_dir(tmp.path())
        .args(["-i", "diagram.mmd", "-o", "legacy.svg"])
        .output()
        .expect("run root compatibility invocation");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stdout.is_empty(),
        "file output must not write a payload"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(!stderr.contains("deprecated"), "{stderr}");
    assert!(!stderr.contains("v0.9.0"), "{stderr}");
    let svg = fs::read_to_string(tmp.path().join("legacy.svg")).expect("read legacy SVG");
    assert!(svg.trim_start().starts_with("<svg"));
}

#[test]
fn root_mmdc_options_are_an_exact_alias_for_explicit_mmdc() {
    let source = "flowchart LR\nA-->B\n";
    let root = run_with_stdin(&["-i", "-", "-o", "-", "-e", "svg", "--quiet"], source);
    let explicit = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "-", "-e", "svg", "--quiet"],
        source,
    );

    assert_eq!(exit_code(root.status), exit_code(explicit.status));
    assert_eq!(root.stdout, explicit.stdout);
    assert_eq!(root.stderr, explicit.stderr);
}

#[test]
fn root_mmdc_quiet_remains_silent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), "flowchart LR\nA-->B\n").expect("write input");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["--input=diagram.mmd", "--output=quiet.svg", "--quiet"])
        .output()
        .expect("run quiet root compatibility invocation");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "quiet root compatibility invocation must remain silent: {:?}",
        output.stderr
    );
    assert!(tmp.path().join("quiet.svg").exists());
}

#[test]
fn compact_root_options_use_the_mmdc_parser_without_compatibility_noise() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), "flowchart LR\nA-->B\n").expect("write input");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-idiagram.mmd", "-ocompact.svg"])
        .output()
        .expect("run compact root compatibility invocation");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(tmp.path().join("compact.svg").exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("deprecated"), "{stderr}");
    assert!(!stderr.contains("v0.9.0"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn attached_non_utf8_input_reaches_the_mmdc_parser_unchanged() {
    use std::os::unix::ffi::OsStringExt;

    let tmp = tempfile::tempdir().expect("tempdir");
    let input = std::ffi::OsString::from_vec(b"diagram-\xff.mmd".to_vec());
    let mut attached_input = std::ffi::OsString::from("-i");
    attached_input.push(&input);
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args([
            attached_input,
            std::ffi::OsString::from("-o"),
            std::ffi::OsString::from("non-utf8.svg"),
        ])
        .output()
        .expect("run root compatibility invocation with a non-UTF-8 path");

    assert_eq!(exit_code(output.status), 2);
    assert!(!tmp.path().join("non-utf8.svg").exists());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        !stderr.contains("v0.9.0")
            && !stderr.contains("deprecated")
            && stderr.contains("Input file")
            && stderr.contains("\\xFF"),
        "the original non-UTF-8 argument must reach mmdc unchanged:\n{stderr}"
    );
}

#[test]
fn non_mmdc_root_render_syntax_points_to_explicit_workflows() {
    for args in [
        vec!["--id", "diagram-root"],
        vec!["--suppress-errors"],
        vec!["--sequence-mirror-actors"],
        vec!["--ascii-charset"],
        vec!["--ascii-width-profile"],
        vec!["--ascii-direction"],
        vec!["--ascii-color"],
        vec!["--xychart-vertical-plot-height"],
        vec!["--xychart-category-band-width"],
        vec!["--xychart-horizontal-plot-width"],
        vec!["--ascii-max-grid-cells"],
        vec!["diagram.mmd"],
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = assert_cmd::cargo_bin!("merman-cli");
        let output = Command::new(exe)
            .current_dir(tmp.path())
            .args(&args)
            .output()
            .expect("run cli");

        assert_eq!(exit_code(output.status), 2, "args: {args:?}");
        assert!(output.stdout.is_empty(), "usage errors belong on stderr");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains(args[0])
                && stderr.contains("merman-cli mmdc")
                && stderr.contains("merman-cli render")
                && stderr.contains("merman-cli batch"),
            "removed root rendering needs directional migration hints for {args:?}:\n{stderr}"
        );
        assert!(
            !stderr.contains("Input file") && !tmp.path().join("out.svg").exists(),
            "the migration diagnostic must not execute the legacy render path for {args:?}:\n{stderr}"
        );
    }
}

#[test]
fn mmdc_distinguishes_implicit_stdin_from_explicit_stdin() {
    let implicit = run_with_stdin(&["mmdc", "-o", "-", "-e", "svg"], "flowchart LR\nA-->B\n");
    assert!(implicit.status.success(), "stderr: {:?}", implicit.stderr);
    let stderr = String::from_utf8(implicit.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("No input file specified"),
        "implicit compatibility stdin should retain the pinned warning:\n{stderr}"
    );

    let explicit = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "-", "-e", "svg"],
        "flowchart LR\nA-->B\n",
    );
    assert!(explicit.status.success(), "stderr: {:?}", explicit.stderr);
    assert!(
        explicit.stderr.is_empty(),
        "explicit compatibility stdin should be quiet: {:?}",
        explicit.stderr
    );

    let implicit_quiet = run_with_stdin(
        &["mmdc", "--quiet", "-o", "-", "-e", "svg"],
        "flowchart LR\nA-->B\n",
    );
    assert!(implicit_quiet.status.success());
    assert!(
        String::from_utf8_lossy(&implicit_quiet.stderr).contains("No input file specified"),
        "upstream compatibility warning must not be suppressed by --quiet"
    );

    let implicit_format = run_with_stdin(&["mmdc", "-i", "-", "-o", "-"], "flowchart LR\nA-->B\n");
    assert!(implicit_format.status.success());
    assert!(
        String::from_utf8_lossy(&implicit_format.stderr)
            .contains("No output format specified, using svg"),
        "stdout format inference must retain the pinned warning"
    );
}

#[test]
fn native_render_uses_piped_stdin_and_stdout_by_default() {
    let output = run_with_stdin(&["render"], "flowchart LR\nA-->B\n");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "unexpected diagnostic: {:?}",
        output.stderr
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.trim_start().starts_with("<svg"),
        "native piped rendering should default to an SVG payload on stdout:\n{stdout}"
    );
}

#[test]
fn native_render_uses_f_for_format_and_warns_for_the_temporary_e_spelling() {
    let source = "flowchart LR\nA-->B\n";
    let current = run_with_stdin(&["render", "-f", "svg"], source);
    assert!(current.status.success(), "stderr: {:?}", current.stderr);
    assert!(
        current.stderr.is_empty(),
        "the native spelling should not warn: {}",
        String::from_utf8_lossy(&current.stderr)
    );

    let legacy = run_with_stdin(&["render", "-e", "svg"], source);
    assert!(legacy.status.success(), "stderr: {:?}", legacy.stderr);
    let stderr = String::from_utf8(legacy.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("deprecated")
            && stderr.contains("v0.9.0")
            && stderr.contains("merman-cli render -f svg"),
        "the temporary spelling needs exact migration guidance:\n{stderr}"
    );

    let attached_legacy = run_with_stdin(&["render", "-esvg"], source);
    assert!(
        attached_legacy.status.success(),
        "stderr: {:?}",
        attached_legacy.stderr
    );
    assert!(
        String::from_utf8_lossy(&attached_legacy.stderr).contains("merman-cli render -f svg"),
        "the attached legacy spelling must retain migration guidance"
    );

    let compatible = run_with_stdin(&["mmdc", "-i", "-", "-o", "-", "-e", "svg"], source);
    assert!(
        compatible.status.success(),
        "stderr: {:?}",
        compatible.stderr
    );
    assert!(
        !String::from_utf8_lossy(&compatible.stderr).contains("deprecated"),
        "explicit mmdc -e is a permanent compatibility contract"
    );

    let quiet_legacy = run_with_stdin(&["render", "--quiet", "-e", "svg"], source);
    assert!(quiet_legacy.status.success());
    assert!(
        String::from_utf8_lossy(&quiet_legacy.stderr).contains("deprecated"),
        "quiet must not hide a bounded public-contract migration warning"
    );
}

#[test]
fn native_format_spellings_conflict_before_input_acquisition() {
    for args in [
        ["render", "-e", "svg", "-f", "svg", "missing.mmd"].as_slice(),
        ["render", "-f", "svg", "-e", "svg", "missing.mmd"].as_slice(),
        ["render", "-e", "svg", "--format", "svg", "missing.mmd"].as_slice(),
        ["render", "--format", "svg", "-e", "svg", "missing.mmd"].as_slice(),
        ["batch", "-e", "svg", "-f", "svg", "missing.md"].as_slice(),
        ["batch", "-f", "svg", "-e", "svg", "missing.md"].as_slice(),
        ["batch", "-e", "svg", "--format", "svg", "missing.md"].as_slice(),
        ["batch", "--format", "svg", "-e", "svg", "missing.md"].as_slice(),
    ] {
        let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
            .args(args)
            .output()
            .expect("run cli");
        assert_eq!(exit_code(output.status), 2, "args: {args:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            (stderr.contains("cannot be used with")
                || stderr.contains("cannot be used multiple times"))
                && !stderr.contains("missing."),
            "format conflicts must fail before input acquisition: {args:?}\n{stderr}"
        );
    }
}

#[test]
fn native_batch_temporary_e_spelling_maps_and_warns() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join("input.md"),
        "```mermaid\nflowchart LR\nA-->B\n```\n",
    )
    .expect("write Markdown");
    let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .current_dir(tmp.path())
        .args(["batch", "input.md", "-e", "svg"])
        .output()
        .expect("run batch");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("merman-cli batch -f svg") && stderr.contains("v0.9.0"),
        "batch migration warning should include the exact replacement:\n{stderr}"
    );
    assert!(tmp.path().join("input.merman").join("input.md").exists());
}

#[test]
fn mmdc_native_only_formats_point_to_the_native_command() {
    for (format, replacement) in [
        ("jpg", "merman-cli render -f jpg"),
        ("ascii", "merman-cli render -f ascii"),
        ("unicode", "merman-cli render -f unicode"),
    ] {
        let output = run_with_stdin(
            &["mmdc", "-i", "-", "-o", "-", "-e", format],
            "flowchart LR\nA-->B\n",
        );
        assert_eq!(exit_code(output.status), 2, "format: {format}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains(replacement),
            "mmdc should point native-only {format} users to an executable replacement:\n{stderr}"
        );
    }
}

#[test]
fn mmdc_native_only_output_extension_points_to_the_native_command() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), "flowchart LR\nA-->B\n").expect("write input");
    let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .current_dir(tmp.path())
        .args(["mmdc", "-i", "diagram.mmd", "-o", "out.jpg"])
        .output()
        .expect("run cli");

    assert_eq!(exit_code(output.status), 2);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("merman-cli render -f jpg") && !tmp.path().join("out.jpg").exists(),
        "native-only extensions need an executable replacement before output effects:\n{stderr}"
    );
}

#[test]
fn native_render_replaces_the_named_input_extension_by_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["render", "diagram.mmd"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stdout.is_empty(),
        "named native input should default to a sibling file"
    );
    let svg = fs::read_to_string(tmp.path().join("diagram.svg")).expect("read native SVG");
    assert!(svg.trim_start().starts_with("<svg"));
    assert!(
        !tmp.path().join("diagram.mmd.svg").exists(),
        "native naming must remain distinct from mmdc compatibility naming"
    );
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
        .args(["mmdc", "-i", "input.md", "-o", "out.svg"])
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
        (
            &["mmdc", "-i", "-", "-o", "-"],
            Some(b"flowchart LR\nA-->B\n"),
        ),
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
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--iconPacksNamesAndUrls",
            icon_arg,
        ],
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
            "mmdc",
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
fn native_icon_options_consume_exactly_one_value_per_occurrence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("input.mmd"), "flowchart LR\nA-->B\n").expect("write input");
    let package = tmp
        .path()
        .join("node_modules")
        .join("@iconify-json")
        .join("test");
    fs::create_dir_all(&package).expect("create icon package");
    fs::write(
        package.join("icons.json"),
        r#"{"prefix":"test","icons":{}}"#,
    )
    .expect("write icon package");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "render",
            "--format",
            "svg",
            "--icon-pack",
            "@iconify-json/test",
            "input.mmd",
            "--output",
            "out.svg",
        ])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "unexpected diagnostic: {:?}",
        output.stderr
    );
    let svg = fs::read_to_string(tmp.path().join("out.svg")).expect("read native SVG");
    assert!(
        svg.trim_start().starts_with("<svg"),
        "the positional input must not be consumed as a second icon package"
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
        &[
            "mmdc", "-i", "input.md", "-o", "out.md", "--jobs", "2", "-q",
        ],
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

#[cfg(feature = "system-timing")]
#[test]
fn quiet_suppresses_explicit_system_timing_diagnostics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("input.mmd"), "flowchart LR\nA-->B\n").expect("write Mermaid source");

    let output = run_with_stdin_in_dir(
        &[
            "render",
            "input.mmd",
            "--output",
            "out.svg",
            "--system-timing",
            "--quiet",
        ],
        "",
        Some(tmp.path()),
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "quiet mode must suppress timing diagnostics: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
