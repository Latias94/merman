use assert_cmd::prelude::*;
use png::ColorType;
use std::fs;
use std::io::{Cursor, Read};
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

#[test]
fn cli_renders_png_smoke() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.png");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            "--out",
            out.to_string_lossy().as_ref(),
            fixture.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read png");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "output is not a PNG"
    );
}

#[test]
fn cli_rasterizes_svg_input_to_png() {
    let root = repo_root();

    let tmp = tempfile::tempdir().expect("tempdir");
    let svg_in = tmp.path().join("in.svg");
    let out = tmp.path().join("out.png");

    fs::write(
        &svg_in,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect x="1" y="1" width="8" height="8" fill='#000'/></svg>"#,
    )
    .expect("write svg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            "--out",
            out.to_string_lossy().as_ref(),
            svg_in.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read png");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "output is not a PNG"
    );
}

#[test]
fn cli_rasterizes_raw_svg_after_resvg_safe_boundary() {
    let root = repo_root();

    let tmp = tempfile::tempdir().expect("tempdir");
    let svg_in = tmp.path().join("in.svg");
    let css = tmp.path().join("host.css");
    let out = tmp.path().join("out.png");

    fs::write(
        &svg_in,
        r##"<svg id="raw-boundary" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 40"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="60" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" x="5" y="5" width="20px" height="20px" fill="#000"/></svg>"##,
    )
    .expect("write svg");
    fs::write(&css, ".node { stroke: #ef4444; }").expect("write css");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            "--backgroundColor",
            "#f8fafc",
            "--cssFile",
            css.to_string_lossy().as_ref(),
            "--out",
            out.to_string_lossy().as_ref(),
            svg_in.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read png");
    assert_eq!(png_dimensions(&bytes), (80, 40));
}

#[test]
fn cli_rasterizes_svg_input_to_png_with_fit_width_and_scale() {
    let root = repo_root();

    let tmp = tempfile::tempdir().expect("tempdir");
    let svg_in = tmp.path().join("in.svg");
    let out = tmp.path().join("out.png");

    fs::write(
        &svg_in,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500"><rect width="1000" height="500" fill='#000'/></svg>"#,
    )
    .expect("write svg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            "--raster-fit-width",
            "250",
            "--scale",
            "2",
            "--out",
            out.to_string_lossy().as_ref(),
            svg_in.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read png");
    assert_eq!(png_dimensions(&bytes), (500, 250));
}

#[test]
fn cli_rasterizes_svg_input_to_png_with_max_width_limit() {
    let root = repo_root();

    let tmp = tempfile::tempdir().expect("tempdir");
    let svg_in = tmp.path().join("in.svg");
    let out = tmp.path().join("out.png");

    fs::write(
        &svg_in,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500"><rect width="1000" height="500" fill='#000'/></svg>"#,
    )
    .expect("write svg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            "--raster-max-width",
            "128",
            "--out",
            out.to_string_lossy().as_ref(),
            svg_in.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read png");
    assert_eq!(png_dimensions(&bytes), (128, 64));
}

#[test]
fn cli_renders_png_with_default_out_path_for_file_input() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let tmp_fixture = tmp.path().join("basic.mmd");
    fs::copy(&fixture, &tmp_fixture).expect("copy fixture");

    let expected_out = tmp_fixture.with_extension("png");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            tmp_fixture.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&expected_out).expect("read png");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "output is not a PNG"
    );
}

#[test]
fn cli_renders_png_for_negative_viewbox_diagrams() {
    // Regression test: our rasterizer must handle viewBox mins < 0.
    // If we double-apply the viewBox translation, diagrams like kanban/gitGraph render blank.
    let root = repo_root();
    let fixture = root.join("fixtures").join("kanban").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.png");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            "--out",
            out.to_string_lossy().as_ref(),
            fixture.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let mut bytes = Vec::new();
    fs::File::open(&out)
        .expect("open png")
        .read_to_end(&mut bytes)
        .expect("read png");
    let cursor = Cursor::new(bytes.as_slice());
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder.read_info().expect("png read_info");
    let size = reader
        .output_buffer_size()
        .expect("invalid png output buffer size");
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).expect("png next_frame");
    assert_eq!(info.color_type, ColorType::Rgba, "expected RGBA output");
    assert_eq!(
        info.bit_depth,
        png::BitDepth::Eight,
        "expected 8-bit output"
    );

    let has_any_ink = buf[..info.buffer_size()]
        .chunks_exact(4)
        .any(|px| px[3] != 0);
    assert!(has_any_ink, "rendered PNG is fully transparent");
}

fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    let cursor = Cursor::new(bytes);
    let decoder = png::Decoder::new(cursor);
    let reader = decoder.read_info().expect("png read_info");
    let info = reader.info();
    (info.width, info.height)
}
