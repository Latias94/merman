#![cfg(all(feature = "native-fonts", feature = "diagram-requirement"))]

use merman::svg::TextMeasurementSource;
use merman::svg::export::{NativeFontContext, NativeFontOptions};
use merman::{
    OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgOutput, SvgRequest,
};

fn context() -> NativeFontContext {
    let fonts =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../merman-export/tests/fonts");
    NativeFontOptions::default()
        .with_font_file(fonts.join("Cousine-Regular.ttf"))
        .with_font_file(fonts.join("Cousine-Bold.ttf"))
        .with_font_file(fonts.join("Cousine-Italic.ttf"))
        .load(&OperationControl::new())
        .unwrap()
}

fn request(context: NativeFontContext) -> SvgRequest {
    SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context)
            .unwrap(),
        ..Default::default()
    }
}

fn render(source: &str, request: SvgRequest) -> SvgOutput {
    match Renderer::new()
        .render(RenderRequest::svg(source, OperationControl::new(), request))
        .unwrap()
    {
        RenderOutput::Svg(Some(output)) => output,
        other => panic!("expected Requirement SVG, got {other:?}"),
    }
}

#[test]
fn native_requirement_rejects_retained_font_shorthand_before_measurement() {
    let request = request(context());
    for (style, property) in [
        ("font:italic 48px Cousine", "font"),
        ("font-size:64px;font:inherit", "font"),
        ("font-size:64px;font:unset", "font"),
        ("font-size:calc(48px)", "font-size (unresolved CSS value)"),
        ("letter-spacing:12px", "letter-spacing"),
        ("all:inherit", "all"),
    ] {
        let source = format!(
            "requirementDiagram\nrequirement r {{\ntext: \"<span style='{style}'>WWWW</span>\"\n}}\n",
        );
        let error = Renderer::new()
            .render(RenderRequest::svg(
                &source,
                OperationControl::new(),
                request.clone(),
            ))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("inline HTML text geometry style `{property}`")),
            "{error}"
        );
    }
}

#[test]
fn native_requirement_accepts_removed_styles_svg_labels_and_measured_longhands() {
    let request = request(context());
    for (config, style) in [
        (r#"{"htmlLabels":false}"#, "font:italic 48px Cousine"),
        (
            r#"{"htmlLabels":true,"dompurifyConfig":{"FORBID_ATTR":["style"]}}"#,
            "font:italic 48px Cousine",
        ),
        (
            r#"{"htmlLabels":true}"#,
            "font-style:italic;font-size:48px;font-family:Cousine",
        ),
    ] {
        let source = format!(
            "---\nconfig: {config}\n---\nrequirementDiagram\nrequirement r {{\ntext: \"<span style='{style}'>WWWW</span>\"\n}}\n",
        );
        let output = render(&source, request.clone());
        let entries = output.evidence().measurement().entries();
        assert!(!entries.is_empty());
        assert!(
            entries.iter().all(|entry| {
                entry.provenance().source == TextMeasurementSource::Host
                    && entry.provenance().fallback_reason.is_none()
            }),
            "{entries:?}"
        );
    }
}

#[test]
fn native_requirement_points_share_pixel_geometry_and_relative_sizes_fail_admission() {
    let request = request(context());
    let source = |size| {
        format!(
            "---\nconfig: {{htmlLabels: false, look: classic, themeVariables: {{fontFamily: Cousine, fontSize: 16px}}}}\n---\nrequirementDiagram\nrequirement r {{\ntext: WWWWWWWWWWWW\n}}\nstyle r font-size:{size}\n",
        )
    };
    let points = render(&source("24pt"), request.clone());
    let pixels = render(&source("32px"), request.clone());
    assert_eq!(
        points.svg().replace("font-size:24pt", "font-size:32px"),
        pixels.svg()
    );
    let baseline = render(&source("16px"), request.clone());
    assert_ne!(
        points.svg().replace("font-size:24pt", "font-size:16px"),
        baseline.svg()
    );
    for size in ["2em", "125%", "calc(24pt)"] {
        let error = Renderer::new()
            .render(RenderRequest::svg(
                &source(size),
                OperationControl::new(),
                request.clone(),
            ))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Requirement requires an absolute CSS length"),
            "{size}: {error}"
        );
    }
}
