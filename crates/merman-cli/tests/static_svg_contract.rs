#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

mod support;

use std::process::Output;
use support::run_with_stdin;

const FLAT: &str =
    "---\nconfig:\n  theme: redux-color\n  look: classic\n---\nflowchart LR\nA --> B\n";

fn render(format: &str, extra: &[&str], source: &str) -> Output {
    let mut args = vec!["render", "-", "--format", format, "--output", "-"];
    args.extend(extra);
    run_with_stdin(&args, source)
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr),
    );
    output.stdout
}

#[test]
fn static_svg_is_explicit_and_mmdc_retains_the_full_palette() {
    let full = String::from_utf8(success(render("svg", &[], FLAT))).unwrap();
    let optimized =
        String::from_utf8(success(render("svg", &["--svg-pipeline", "static"], FLAT))).unwrap();
    let mmdc = String::from_utf8(success(run_with_stdin(
        &["mmdc", "-i", "-", "-e", "svg", "-o", "-"],
        FLAT,
    )))
    .unwrap();

    let selector = r#"[data-color-id="color-1"]"#;
    assert!(full.contains(selector));
    assert!(mmdc.contains(selector));
    assert!(!optimized.contains(selector));
    assert!(optimized.len() < full.len());
    assert_eq!(
        full.split_once("</style>").unwrap().1,
        optimized.split_once("</style>").unwrap().1
    );
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn native_cli_encoders_default_to_static_generation() {
    let limits = ["--resource-limit", "max_svg_bytes=18000"];
    let full = render("svg", &limits, FLAT);
    assert!(!full.status.success());
    assert!(full.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&full.stderr);
    let setting = stderr
        .split_once("--resource-limit ")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .expect("retry option");
    assert!(setting.starts_with("max_svg_bytes="), "{stderr}");
    success(render("svg", &["--resource-limit", setting], FLAT));
    success(render(
        "svg",
        &[limits[0], limits[1], "--svg-pipeline", "static"],
        FLAT,
    ));

    for (format, signature) in [
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n".as_slice()),
        #[cfg(feature = "jpeg")]
        ("jpeg", b"\xff\xd8\xff".as_slice()),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF-".as_slice()),
    ] {
        let bytes = success(render(format, &limits, FLAT));
        assert!(bytes.starts_with(signature), "{format}");
    }

    #[cfg(feature = "png")]
    {
        let full = run_with_stdin(
            &[
                "mmdc", "-i", "-", "-e", "png", "-o", "-", limits[0], limits[1],
            ],
            FLAT,
        );
        assert!(!full.status.success());
        assert!(String::from_utf8_lossy(&full.stderr).contains("max_svg_bytes"));
    }
}

#[cfg(any(feature = "png", feature = "pdf"))]
#[test]
fn static_generation_preserves_target_quotas() {
    #[cfg(feature = "png")]
    {
        let bytes = success(render("png", &["--raster-max-pixels", "1"], FLAT));
        assert_eq!(&bytes[16..24], &[0, 0, 0, 1, 0, 0, 0, 1]);
    }
    #[cfg(feature = "pdf")]
    {
        let source = FLAT.replace("look: classic", "look: neo");
        let result = render("pdf", &["--pdf-max-filter-image-pixels", "1"], &source);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("--pdf-max-filter-image-pixels "),
            "{stderr}"
        );
        assert!(!stderr.contains("--resource-limit"), "{stderr}");
    }
}

#[cfg(feature = "png")]
#[test]
fn embedded_image_byte_limit_suggests_a_working_export_option() {
    const BLUE_PIXEL: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNgYPgPAAEDAQAIicLsAAAAAElFTkSuQmCC";
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="20"><image xlink:href="data:image/png;base64,{BLUE_PIXEL}" width="20" height="20"/></svg>"#
    );
    let flag = "--embedded-image-max-bytes";
    let limited = render("png", &["--input-kind=svg", flag, "3"], &source);
    assert_eq!(limited.status.code(), Some(1));
    assert!(limited.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&limited.stderr);
    let value = stderr
        .split_once("--embedded-image-max-bytes ")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .expect("embedded image retry value");
    assert!(
        success(render("png", &["--input-kind=svg", flag, value], &source)).starts_with(b"\x89PNG")
    );
}
