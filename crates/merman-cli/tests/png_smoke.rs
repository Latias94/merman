mod support;

use assert_cmd::prelude::*;
use png::ColorType;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

#[test]
fn png_canvas_uses_export_policy_instead_of_svg_css() {
    let source = "flowchart LR\n A --> B";
    for (args, expected) in [
        (vec!["render", "--format", "png", "-"], [0, 0, 0, 0]),
        (
            vec!["mmdc", "-i", "-", "-o", "-", "-e", "png"],
            [255, 255, 255, 255],
        ),
        (
            vec![
                "mmdc",
                "-i",
                "-",
                "-o",
                "-",
                "-e",
                "png",
                "-b",
                "transparent",
            ],
            [0, 0, 0, 0],
        ),
        (
            vec!["render", "--format", "png", "--background", "#112233", "-"],
            [17, 34, 51, 255],
        ),
        (
            vec![
                "render",
                "--format",
                "png",
                "--theme",
                "studio",
                "--background",
                "#112233",
                "-",
            ],
            [17, 34, 51, 255],
        ),
        (
            vec![
                "render",
                "--format",
                "png",
                "--theme",
                "studio",
                "--background",
                "transparent",
                "-",
            ],
            [0, 0, 0, 0],
        ),
    ] {
        let output = support::run_with_stdin(&args, source);
        assert!(output.status.success(), "{args:?}: {:?}", output.stderr);
        let mut reader = png::Decoder::new(Cursor::new(output.stdout))
            .read_info()
            .expect("PNG info");
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).expect("PNG frame");
        assert_eq!(info.color_type, ColorType::Rgba);
        assert_eq!(pixels[..4], expected, "canvas pixel for {args:?}");
    }
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
            "--output",
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
fn cli_png_preserves_the_width_and_height_of_flowchart_html_label_boxes() {
    for (style, expected) in [
        ("width:188px;height:66px", (236, 106)),
        ("width:220px;height:90px", (268, 130)),
    ] {
        let source = format!(
            "---\nconfig:\n  look: neo\n  htmlLabels: true\n---\nflowchart TB\nA@{{ shape: rect, label: \"<div style='{style}'>short</div>\" }}\n"
        );
        let output = support::run_with_stdin(
            &[
                "render",
                "--format",
                "png",
                "--font-measurement",
                "deterministic",
                "-",
            ],
            &source,
        );
        assert!(output.status.success(), "{style}: {:?}", output.stderr);
        assert_eq!(png_dimensions(&output.stdout), expected, "{style}");
    }
}

#[test]
fn cli_png_paints_equal_flowchart_label_boxes_with_default_fonts() {
    let source = r#"---
config:
  look: neo
  htmlLabels: true
---
flowchart TB
  A["<div style='width:188px;height:66px'>short</div>"]
  B["<div style='width:188px;height:66px'>a much longer label<br/>with three<br/>lines of text</div>"]
  A --> B
  style A fill:#0055cc,stroke:none
  style B fill:#22a055,stroke:none
"#;
    let output = support::run_with_stdin(&["render", "--format", "png", "-"], source);
    assert!(output.status.success(), "{:?}", output.stderr);
    let mut reader = png::Decoder::new(Cursor::new(output.stdout))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, ColorType::Rgba);
    let stride = info.width as usize * 4;
    for fill in [[0, 85, 204, 255], [34, 160, 85, 255]] {
        assert_eq!(
            solid_fill_size(&pixels, stride, fill),
            (220, 90),
            "{fill:?}"
        );
    }
}

#[test]
fn cli_png_paints_flowchart_rectangle_css_dimensions() {
    let source = "---\nconfig:\n  layout: dagre\n  look: neo\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nA[Short]\n";
    for style in [
        "style A width:220px,height:90px,fill:#0055cc,stroke:none",
        "classDef sized width:220px,height:90px,fill:#0055cc,stroke:none\nclass A sized",
        "classDef sized width:140px,height:80px,fill:#0055cc,stroke:none\nclass A sized\nstyle A WIDTH:220,height:90",
        "classDef sized width:220px !important,height:90px !important,fill:#0055cc,stroke:none\nclass A sized\nstyle A WIDTH:140px,HEIGHT:80px",
    ] {
        let output = support::run_with_stdin(
            &[
                "render",
                "--format",
                "png",
                "--font-measurement",
                "deterministic",
                "-",
            ],
            &format!("{source}{style}\n"),
        );
        assert!(output.status.success(), "{style}: {:?}", output.stderr);
        let mut reader = png::Decoder::new(Cursor::new(output.stdout))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (236, 106), "{style}");
        assert_eq!(info.color_type, ColorType::Rgba);
        let stride = info.width as usize * 4;
        assert_eq!(
            solid_fill_size(&pixels, stride, [0, 85, 204, 255]),
            (220, 90),
            "{style}"
        );
    }
}

#[test]
fn cli_png_paints_the_intrinsic_flowchart_image_aspect_ratio() {
    let image = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAHgAAAA8CAIAAAAiz+n/AAAAn0lEQVR4nO3QMQ0AIADAMEAF8hCJQFSwqzWwZHOfO/hvBQ2M7hgdMTpidMToiNERoyNGR4yOGB0xOmJ0xOiI0RGjI0ZHjI4YHTE6YnTE6IjREaMjRkeMjhgdMTpidMToiNERoyNGR4yOGB0xOmJ0xOiI0RGjI0ZHjI4YHTE6YnTE6IjREaMjRkeMjhgdMTpidMToiNERoyNGR4yOGD0aD1QBAaT/CgOSAAAAAElFTkSuQmCC";
    for (constraint, expected_width) in [("off", 120), ("on", 80)] {
        let source =
            format!("flowchart TB\nA@{{ img: \"{image}\", h: 40, constraint: {constraint} }}\n");
        let output = support::run_with_stdin(
            &[
                "render",
                "--format",
                "png",
                "--font-measurement",
                "deterministic",
                "-",
            ],
            &source,
        );
        assert!(output.status.success(), "{constraint}: {:?}", output.stderr);
        let mut reader = png::Decoder::new(Cursor::new(output.stdout))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, ColorType::Rgba);
        let stride = info.width as usize * 4;
        let row = info.height as usize / 2 * stride;
        let center = info.width as usize / 2 * 4;
        let color = [24, 96, 180, 255];
        let width = pixels[row..row + stride]
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel == color)
            .count();
        let height = pixels
            .chunks_exact(stride)
            .filter(|row| row[center..center + 4] == color)
            .count();
        assert_eq!((width, height), (expected_width, 40), "{constraint}");
    }
}

#[test]
fn cli_png_paints_the_styled_flowchart_subgraph_frame() {
    let source = "---\nconfig:\n  layout: dagre\n  look: neo\n  flowchart:\n    minNodeWidth: 0\n---\nflowchart TB\nsubgraph G[Group]\nA[Short] --> B[End]\nend\nC --> A\nstyle G width:260px,height:180px,fill:#0055cc,stroke:none\n";
    let output = support::run_with_stdin(
        &[
            "render",
            "--format",
            "png",
            "--font-measurement",
            "deterministic",
            "-",
        ],
        source,
    );
    assert!(output.status.success(), "{:?}", output.stderr);
    let mut reader = png::Decoder::new(Cursor::new(output.stdout))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, ColorType::Rgba);
    assert_eq!((info.width, info.height), (276, 291));
    let stride = info.width as usize * 4;
    assert_eq!(
        solid_fill_size(&pixels, stride, [0, 85, 204, 255]),
        (260, 180)
    );
}

fn solid_fill_size(pixels: &[u8], stride: usize, fill: [u8; 4]) -> (usize, usize) {
    let painted_row_widths = pixels.chunks_exact(stride).map(|row| {
        row.as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel == fill)
            .count()
    });
    let painted_width = painted_row_widths.clone().max().unwrap();
    let painted_height = painted_row_widths.filter(|width| *width > 0).count();
    (painted_width, painted_height)
}

#[test]
fn cli_png_paints_flowchart_html_line_height() {
    let base = "---\nconfig:\n  look: neo\n  htmlLabels: true\n---\nflowchart TB\n";
    let font_options: &[&[&str]] = &[
        &[],
        &["--font-measurement", "deterministic"],
        #[cfg(feature = "native-fonts")]
        &[
            "--font-measurement",
            "native",
            "--font-file",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../merman-export/tests/fonts/Cousine-Regular.ttf"
            ),
        ],
    ];
    for (body, expected_height) in [
        (
            "A[\"AB<br/>AB\"]\nstyle A color:#e00022,line-height:3\n",
            Some(124),
        ),
        (
            "A[\"AB<br/>AB\"]:::tall\nclassDef tall color:#e00022,line-height:3\n",
            Some(124),
        ),
        (
            "A -->|AB<br/>AB| B\nlinkStyle default color:#e00022,line-height:3\n",
            None,
        ),
    ] {
        let source = format!("{base}{body}");
        for options in font_options {
            let mut args = vec!["render", "--format", "png", "-"];
            args.extend_from_slice(options);
            let output = support::run_with_stdin(&args, &source);
            assert!(
                output.status.success(),
                "{body} {options:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let mut reader = png::Decoder::new(Cursor::new(output.stdout))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut pixels).unwrap();
            assert_eq!(info.color_type, ColorType::Rgba);
            if let Some(height) = expected_height {
                assert_eq!(info.height, height);
            }
            let red_rows = pixels
                .chunks_exact(info.width as usize * 4)
                .enumerate()
                .filter_map(|(y, row)| {
                    row.as_chunks::<4>()
                        .0
                        .iter()
                        .any(|pixel| {
                            pixel[0] > 170 && pixel[1] < 90 && pixel[2] < 100 && pixel[3] > 200
                        })
                        .then_some(y)
                })
                .collect::<Vec<_>>();
            let first = red_rows[0];
            let second = red_rows
                .windows(2)
                .find(|rows| rows[1] > rows[0] + 1)
                .unwrap()[1];
            assert_eq!(second - first, 42, "{body} {options:?}");
        }
    }
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
            "--output",
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
fn cli_rasterizes_explicit_svg_stdin_to_png() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.png");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args([
            "render",
            "--format",
            "png",
            "--input-kind",
            "svg",
            "--output",
            out.to_string_lossy().as_ref(),
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(
            br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#,
        )
        .expect("write SVG stdin");
    let output = child.wait_with_output().expect("wait for CLI");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let bytes = fs::read(&out).expect("read png");
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn scoped_unbounded_raster_and_images_keep_other_profile_limits() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.svg");
    let out = tmp.path().join("out.png");
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
            "png",
            "--resource-profile",
            "constrained",
            "--raster-unbounded",
            "--embedded-images-unbounded",
            "--resource-limit",
            "max_model_items=10000000",
            "--output",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read PNG");
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn scoped_unbounded_raster_still_obeys_scheduling_budget() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("large.svg");
    let out = tmp.path().join("out.png");
    fs::write(
        &input,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10000 10000"><rect width="10000" height="10000"/></svg>"#,
    )
    .expect("write SVG");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args([
            "render",
            "--format",
            "png",
            "--resource-profile",
            "constrained",
            "--raster-unbounded",
            "--output",
            out.to_string_lossy().as_ref(),
            input.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run CLI");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("max_scheduling_weight_bytes"), "{stderr}");
    assert!(!out.exists());
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
            "--background",
            "#f8fafc",
            "--css-file",
            css.to_string_lossy().as_ref(),
            "--output",
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
            "--output",
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
    let output = Command::new(exe)
        .current_dir(&root)
        .args([
            "render",
            "--format",
            "png",
            "--raster-max-width",
            "128",
            "--output",
            out.to_string_lossy().as_ref(),
            svg_in.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run cli");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("constrained from 1000x500 to 128x64 pixels"),
        "{stderr}"
    );

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
            "--output",
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
        .as_chunks::<4>()
        .0
        .iter()
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
