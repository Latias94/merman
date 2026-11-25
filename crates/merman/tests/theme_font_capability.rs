#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman::svg::{SvgPipeline, ThemePortabilityRequirement};
use merman::theme::{BuiltinPreset, ThemeCompiler, ThemeSelection};
use merman::{
    OperationControl, RenderError, RenderRequest, Renderer, SvgRequest, TargetAdmissionReason,
};

const SOURCE: &str = "flowchart LR\nA[Alpha] --> B[Beta]";

fn theme_with_font() -> ThemeSelection {
    let theme = ThemeCompiler::new()
        .compile_json(
            br#"{"schema_version":2,"extends":"studio","typography":{"sans":["A&B","sans-serif"]}}"#,
            None,
        )
        .unwrap();
    ThemeSelection::compiled(theme)
}

#[test]
fn resource_free_cyberpunk_renders_through_the_public_facade() {
    let request = SvgRequest::default()
        .with_native_theme(ThemeSelection::preset(BuiltinPreset::Cyberpunk, None));
    let svg = Renderer::new()
        .render_svg_data(RenderRequest::svg(SOURCE, OperationControl::new(), request))
        .unwrap()
        .unwrap();
    assert!(svg.contains("<pattern"), "public canvas must render");
    assert!(svg.contains("<filter"), "public glow must render");
    assert!(!svg.contains("@font-face"), "preset has no font resources");
}

#[test]
fn native_font_stack_stays_valid_in_svg_and_has_a_separate_portability_result() {
    let renderer = Renderer::new();
    let mut request = SvgRequest {
        pipeline: Some(SvgPipeline::resvg_safe()),
        ..SvgRequest::default()
    }
    .with_native_theme(theme_with_font());
    let svg = renderer
        .render_svg_data(RenderRequest::svg(
            SOURCE,
            OperationControl::new(),
            request.clone(),
        ))
        .unwrap()
        .unwrap();
    let document = roxmltree::Document::parse(&svg).expect("SVG must escape the font stack");
    assert!(document.descendants().any(|node| {
        node.has_tag_name("style") && node.text().is_some_and(|css| css.contains("A&B"))
    }));

    request.environment = request
        .environment
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable);
    let error = renderer
        .render(RenderRequest::svg(SOURCE, OperationControl::new(), request))
        .expect_err("unembedded named fonts must retain standalone font admission");
    let RenderError::TargetAdmission(error) = error else {
        panic!("expected font admission after valid XML, got {error}");
    };
    assert!(
        error
            .receipt()
            .reasons()
            .contains(&TargetAdmissionReason::SvgFontsNotSelfContained)
    );
    assert!(
        !error
            .receipt()
            .reasons()
            .contains(&TargetAdmissionReason::SvgTerminalValidationFailed)
    );
}
