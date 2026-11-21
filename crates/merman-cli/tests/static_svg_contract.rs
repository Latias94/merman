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
    assert!(String::from_utf8_lossy(&full.stderr).contains("max_svg_bytes"));
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
        assert!(String::from_utf8_lossy(&result.stderr).contains("max_pdf_filter_image_pixels"));
    }
}
