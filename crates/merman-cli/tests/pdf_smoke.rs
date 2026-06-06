use assert_cmd::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

fn large_svg_input() -> &'static str {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 9000"><rect width="9000" height="9000" fill="black"/></svg>"#
}

#[test]
fn cli_renders_pdf_smoke() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.pdf");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "pdf",
            "--out",
            out.to_string_lossy().as_ref(),
            fixture.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-"), "output is not a PDF");
}

#[test]
fn cli_renders_pdf_with_default_out_path_for_file_input() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let tmp_fixture = tmp.path().join("basic.mmd");
    fs::copy(&fixture, &tmp_fixture).expect("copy fixture");

    let expected_out = tmp_fixture.with_extension("pdf");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "pdf",
            tmp_fixture.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&expected_out).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-"), "output is not a PDF");
}

#[test]
fn cli_pdf_rejects_large_intrinsic_svg_by_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("large.svg");
    let out = tmp.path().join("large.pdf");
    fs::write(&input, large_svg_input()).expect("write svg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args([
            "render",
            "--format",
            "pdf",
            "--out",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "expected oversized PDF input to fail"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("PDF output exceeds configured size_limit"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!out.exists(), "failed PDF export should not create output");
}

#[test]
fn cli_pdf_unbounded_allows_large_intrinsic_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("large.svg");
    let out = tmp.path().join("large.pdf");
    fs::write(&input, large_svg_input()).expect("write svg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .args([
            "render",
            "--format",
            "pdf",
            "--raster-unbounded",
            "--out",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-"), "output is not a PDF");
}
