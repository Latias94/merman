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
            "--resource-profile",
            "constrained",
            "--output",
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
fn scoped_unbounded_pdf_and_images_keep_other_profile_limits() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.svg");
    let out = tmp.path().join("out.pdf");
    fs::write(
        &input,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#,
    )
    .expect("write SVG");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .args([
            "render",
            "--format",
            "pdf",
            "--resource-profile",
            "constrained",
            "--pdf-filter-images-unbounded",
            "--embedded-images-unbounded",
            "--output",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read PDF");
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn cli_pdf_preserves_large_intrinsic_vector_page_by_default() {
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
            "--output",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-"), "output is not a PDF");
    assert!(
        String::from_utf8_lossy(&bytes).contains("9000"),
        "the vector PDF page should keep the source dimensions"
    );
}

#[cfg(not(any(feature = "png", feature = "jpeg")))]
#[test]
fn cli_pdf_profile_does_not_expose_raster_pixel_limit_flags() {
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
            "--raster-max-width",
            "1",
            "--raster-max-height",
            "1",
            "--raster-max-pixels",
            "1",
            "--output",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "PDF-only builds must reject PNG/JPEG-only raster limits"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("unexpected argument '--raster-max-width'"),
        "unexpected error: {stderr}"
    );
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[test]
fn cli_pdf_rejects_raster_pixel_limit_flags_when_raster_is_compiled() {
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
            "--raster-max-width",
            "1",
            "--raster-max-height",
            "1",
            "--raster-max-pixels",
            "1",
            "--output",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "native PDF rendering must reject PNG/JPEG-only raster limits"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("raster options require --format png or --format jpg"),
        "unexpected error: {stderr}"
    );
    assert!(
        !out.exists(),
        "validation must fail before creating the output file"
    );
}

#[cfg(feature = "parallel-markdown")]
#[test]
fn parallel_markdown_renders_ordered_pdf_artifacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output_dir = tmp.path().join("rendered");
    fs::write(
        &input,
        "```mermaid\nflowchart LR\nA-->B\n```\n\n```mermaid\nflowchart LR\nB-->C\n```\n",
    )
    .expect("write Markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .args([
            "batch",
            input.to_string_lossy().as_ref(),
            "--format",
            "pdf",
            "--output-dir",
            output_dir.to_string_lossy().as_ref(),
            "--jobs",
            "2",
        ])
        .assert()
        .success();

    for name in ["input-1.pdf", "input-2.pdf"] {
        let bytes = fs::read(output_dir.join(name)).expect("read PDF artifact");
        assert!(bytes.starts_with(b"%PDF-"), "{name} is not a PDF");
    }
    let rewritten =
        fs::read_to_string(output_dir.join("input.md")).expect("read rewritten Markdown");
    let first = rewritten.find("./input-1.pdf").expect("first PDF link");
    let second = rewritten.find("./input-2.pdf").expect("second PDF link");
    assert!(first < second, "rewritten links must retain source order");
}
