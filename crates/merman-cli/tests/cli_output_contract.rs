use std::process::{Command, Stdio};

#[test]
fn cli_rejects_conflicting_raster_unbounded_and_limits() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .stdin(Stdio::null())
        .args([
            "render",
            "--format",
            "png",
            "--raster-unbounded",
            "--raster-max-width",
            "128",
            "-",
        ])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "expected raster unbounded/max conflict to fail"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--raster-unbounded")
            && stderr.contains("--raster-max-width")
            && (stderr.contains("cannot be combined") || stderr.contains("cannot be used with")),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn cli_rejects_conflicting_pdf_filter_budget_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .stdin(Stdio::null())
        .args([
            "render",
            "--format",
            "pdf",
            "--pdf-filter-images-unbounded",
            "--pdf-max-filter-image-pixels",
            "128",
            "-",
        ])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected PDF filter conflict");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--pdf-filter-images-unbounded")
            && stderr.contains("--pdf-max-filter-image-pixels")
            && (stderr.contains("cannot be combined") || stderr.contains("cannot be used with")),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn cli_rejects_conflicting_embedded_image_budget_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .stdin(Stdio::null())
        .args([
            "render",
            "--format",
            "png",
            "--embedded-images-unbounded",
            "--embedded-image-max-pixels",
            "128",
            "-",
        ])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected embedded image conflict");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("--embedded-images-unbounded")
            && stderr.contains("--embedded-image-max-pixels")
            && (stderr.contains("cannot be combined") || stderr.contains("cannot be used with")),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn completion_subcommand_generates_bash_script() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["completion", "bash"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("merman-cli")
            && stdout.contains("--input")
            && stdout.contains("--runtime")
            && stdout.contains("--system-timing")
            && stdout.contains("render"),
        "unexpected completion output:\n{stdout}"
    );
}
