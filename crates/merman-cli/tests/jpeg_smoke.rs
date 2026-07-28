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

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u16, u16)> {
    let mut offset = 2;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xff {
            offset += 1;
            continue;
        }
        let marker = bytes[offset + 1];
        offset += 2;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]));
        if length < 2 || offset.checked_add(length)? > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            let height = u16::from_be_bytes([*bytes.get(offset + 3)?, *bytes.get(offset + 4)?]);
            let width = u16::from_be_bytes([*bytes.get(offset + 5)?, *bytes.get(offset + 6)?]);
            return Some((width, height));
        }
        offset += length;
    }
    None
}

#[test]
fn cli_renders_jpg_smoke() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.jpg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "jpg",
            "--output",
            out.to_string_lossy().as_ref(),
            fixture.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read jpg");
    assert!(
        bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "output is not a JPG"
    );
    assert!(bytes.ends_with(&[0xFF, 0xD9]), "output is not a JPG");
}

#[test]
fn cli_renders_jpg_with_default_out_path_for_file_input() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let tmp_fixture = tmp.path().join("basic.mmd");
    fs::copy(&fixture, &tmp_fixture).expect("copy fixture");

    let expected_out = tmp_fixture.with_extension("jpg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "jpg",
            tmp_fixture.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&expected_out).expect("read jpg");
    assert!(
        bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "output is not a JPG"
    );
    assert!(bytes.ends_with(&[0xFF, 0xD9]), "output is not a JPG");
}

#[test]
fn cli_applies_jpeg_pixel_limits_before_encoding() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.svg");
    let out = tmp.path().join("out.jpg");
    fs::write(
        &input,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100"><rect width="200" height="100"/></svg>"#,
    )
    .expect("write SVG");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .args([
            "render",
            "--format",
            "jpg",
            "--raster-max-width",
            "20",
            "--output",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read JPEG");
    assert_eq!(jpeg_dimensions(&bytes), Some((20, 10)));
}
