mod support;

use std::process::Command;
use support::{exit_code, run_with_stdin};

#[test]
fn render_help_excludes_mmdc_and_batch_only_options() {
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
        "--iconPacks",
        "--iconPacksNamesAndUrls",
        "--outputFormat",
        "--configFile",
        "--cssFile",
    ] {
        assert!(
            !stdout.contains(absent),
            "render help should not include mmdc- or batch-only {absent}:\n{stdout}"
        );
    }

    for present in [
        "--output",
        "--format",
        "--css-file",
        "--presentation-profile",
        "--raster-max-width",
        "--icon-pack",
        "--icon-pack-source",
        "--runtime",
        "--system-timing",
        "--sequence-mirror-actors",
    ] {
        assert!(
            stdout.contains(present),
            "render help should include direct rendering option {present}:\n{stdout}"
        );
    }
}

#[test]
fn render_short_help_prioritizes_the_common_workflow() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let short = Command::new(exe)
        .args(["render", "-h"])
        .output()
        .expect("run short help");
    let long = Command::new(exe)
        .args(["render", "--help"])
        .output()
        .expect("run long help");

    assert!(short.status.success(), "stderr: {:?}", short.stderr);
    assert!(long.status.success(), "stderr: {:?}", long.stderr);
    let short = String::from_utf8(short.stdout).expect("stdout should be utf8");
    let long = String::from_utf8(long.stdout).expect("stdout should be utf8");

    for common in ["--output", "--format", "--theme", "--quiet"] {
        assert!(
            short.contains(common),
            "short help should retain common option {common}:\n{short}"
        );
    }
    for advanced in [
        "--resource-limit",
        "--raster-max-width",
        "--pdf-filter-scale",
        "--embedded-image-max-bytes",
        "--text-measurer",
        "--presentation-profile",
        "--system-timing",
        "--allow-private-network",
    ] {
        assert!(
            !short.contains(advanced),
            "short help should hide advanced option {advanced}:\n{short}"
        );
        assert!(
            long.contains(advanced),
            "long help should retain advanced option {advanced}:\n{long}"
        );
    }
    assert!(
        short.contains("merman-cli render") && short.contains("--help"),
        "short help should end with a copyable example and long-help cue:\n{short}"
    );
}

#[test]
fn batch_help_exposes_only_graphical_batch_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["batch", "--help"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");

    for absent in [
        "--sequence-mirror-actors",
        "--ascii-charset",
        "--ascii-direction",
        "--ascii-color",
        "--xychart-vertical-plot-height",
        "--xychart-category-band-width",
        "--xychart-horizontal-plot-width",
        "--ascii-max-grid-cells",
    ] {
        assert!(
            !stdout.contains(absent),
            "batch help should not advertise unsupported text option {absent}:\n{stdout}"
        );
    }

    for present in [
        "--output-dir",
        "--format",
        "--jobs",
        "--svg-pipeline",
        "--presentation-profile",
    ] {
        assert!(
            stdout.contains(present),
            "batch help should expose graphical batch option {present}:\n{stdout}"
        );
    }

    let rejected = Command::new(exe)
        .args(["batch", "missing.md", "--format", "unicode"])
        .output()
        .expect("run cli");
    assert_eq!(exit_code(rejected.status), 2);
    assert!(rejected.stdout.is_empty());
    let stderr = String::from_utf8(rejected.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("invalid value 'unicode'") && !stderr.contains("missing.md:"),
        "batch must reject text formats during argument parsing:\n{stderr}"
    );
}

#[test]
fn batch_and_mmdc_use_progressive_help_without_losing_long_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    for (command, common, advanced) in [
        (
            "batch",
            &["--output-dir", "--format", "--jobs"][..],
            &[
                "--resource-limit",
                "--raster-max-width",
                "--pdf-filter-scale",
                "--svg-pipeline",
                "--presentation-profile",
            ][..],
        ),
        (
            "mmdc",
            &["--input", "--output", "--outputFormat"][..],
            &[
                "--artefacts",
                "--jobs",
                "--resource-limit",
                "--raster-max-width",
                "--pdf-filter-scale",
                "--svg-pipeline",
                "--presentation-profile",
                "--puppeteerConfigFile",
            ][..],
        ),
    ] {
        let short = Command::new(exe)
            .args([command, "-h"])
            .output()
            .expect("run short help");
        let long = Command::new(exe)
            .args([command, "--help"])
            .output()
            .expect("run long help");
        assert!(short.status.success(), "{command}: {:?}", short.stderr);
        assert!(long.status.success(), "{command}: {:?}", long.stderr);
        let short = String::from_utf8(short.stdout).expect("stdout should be utf8");
        let long = String::from_utf8(long.stdout).expect("stdout should be utf8");
        for option in common {
            assert!(
                short.contains(option),
                "{command} short help should retain {option}:\n{short}"
            );
        }
        for option in advanced {
            assert!(
                !short.contains(option),
                "{command} short help should hide {option}:\n{short}"
            );
            assert!(
                long.contains(option),
                "{command} long help should retain {option}:\n{long}"
            );
        }
        assert!(
            short.contains(&format!("merman-cli {command}")) && short.contains("--help"),
            "{command} short help needs an example and long-help cue:\n{short}"
        );
    }
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
        ("--pdf-filter-scale", "0", "expected a positive number"),
        (
            "--pdf-max-filter-image-pixels",
            "0",
            "expected a positive integer",
        ),
        (
            "--embedded-image-max-bytes",
            "0",
            "expected a positive integer",
        ),
        (
            "--embedded-image-max-total-bytes",
            "0",
            "expected a positive integer",
        ),
        (
            "--embedded-image-max-pixels",
            "0",
            "expected a positive integer",
        ),
        (
            "--embedded-image-max-total-pixels",
            "0",
            "expected a positive integer",
        ),
    ] {
        let output = Command::new(exe)
            .args(["mmdc", "-i", "-", "-o", "-", flag, value])
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
        .args(["mmdc", "-i", "-", "-o", "-", "--jobs", "0"])
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
            "expected a canonical civil date such as YYYY-MM-DD or +10000-MM-DD",
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

#[cfg(all(
    feature = "system-clock",
    feature = "system-timezone",
    feature = "system-random"
))]
#[test]
fn cli_accepts_an_explicit_native_runtime() {
    let output = run_with_stdin(
        &["parse", "--runtime", "native", "-"],
        "flowchart TD\nA --> B\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "native selection should not emit diagnostics unless timing is requested: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(not(all(
    feature = "system-clock",
    feature = "system-timezone",
    feature = "system-random"
)))]
#[test]
fn cli_reports_an_unavailable_native_runtime_as_invalid_configuration() {
    let output = run_with_stdin(
        &["parse", "--runtime", "native", "-"],
        "flowchart TD\nA --> B\n",
    );

    assert_eq!(exit_code(output.status), 2);
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--runtime native is unavailable")
            && stderr.contains("runtime capability `system-")
            && stderr.contains("is not compiled into this artifact"),
        "unexpected stderr:\n{stderr}"
    );
}

#[cfg(feature = "system-timing")]
#[test]
fn cli_system_timing_is_explicit_and_reaches_each_runtime_configuration_path() {
    let source = "flowchart TD\nA --> B\n";
    let parse = run_with_stdin(
        &[
            "parse",
            "--runtime",
            "deterministic",
            "--system-timing",
            "-",
        ],
        source,
    );
    assert!(parse.status.success(), "stderr: {:?}", parse.stderr);
    assert!(
        String::from_utf8_lossy(&parse.stderr).contains("[parse-timing]"),
        "parse timing diagnostics were not emitted: {}",
        String::from_utf8_lossy(&parse.stderr)
    );

    let render = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--runtime",
            "deterministic",
            "--system-timing",
        ],
        source,
    );
    assert!(render.status.success(), "stderr: {:?}", render.stderr);
    assert!(
        String::from_utf8_lossy(&render.stderr).contains("[parse-render-timing]"),
        "mmdc did not carry timing through compatibility arguments: {}",
        String::from_utf8_lossy(&render.stderr)
    );

    let lint = run_with_stdin(
        &["lint", "--runtime", "deterministic", "--system-timing", "-"],
        source,
    );
    assert!(lint.status.success(), "stderr: {:?}", lint.stderr);
    assert!(
        String::from_utf8_lossy(&lint.stderr).contains("[parse-timing]"),
        "lint did not carry timing through AnalysisCliArgs: {}",
        String::from_utf8_lossy(&lint.stderr)
    );
}

#[cfg(not(feature = "system-timing"))]
#[test]
fn cli_reports_unavailable_system_timing_as_invalid_configuration() {
    let output = run_with_stdin(
        &["parse", "--system-timing", "-"],
        "flowchart TD\nA --> B\n",
    );

    assert_eq!(exit_code(output.status), 2);
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--system-timing is unavailable")
            && stderr.contains("runtime capability `system-timing`")
            && stderr.contains("is not compiled into this artifact"),
        "unexpected stderr:\n{stderr}"
    );
}
