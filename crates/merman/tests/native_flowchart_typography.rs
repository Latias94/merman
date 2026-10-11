#![cfg(all(feature = "native-fonts", feature = "diagram-flowchart"))]

use merman::svg::export::NativeFontOptions;
use merman::{OperationControl, RenderRequest, RenderTarget, Renderer, SvgEnvironment, SvgRequest};

fn request(native: bool) -> SvgRequest {
    let mut request = SvgRequest::default();
    if native {
        let context = NativeFontOptions::default()
            .with_font_file(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../merman-export/tests/fonts/Cousine-Regular.ttf"),
            )
            .load(&OperationControl::new())
            .unwrap();
        request.environment = SvgEnvironment::deterministic()
            .with_native_font_context(context)
            .unwrap();
    }
    request
}

#[test]
fn native_flowchart_rejects_configurations_with_different_layout_and_css_typography() {
    for (config, expected) in [
        (
            r#"{"themeVariables":{"fontSize":24}}"#,
            "themeVariables.fontSize",
        ),
        (
            r#"{"themeVariables":{"fontSize":"24em"}}"#,
            "themeVariables.fontSize",
        ),
        (r#"{"fontWeight":"700"}"#, "root fontWeight"),
    ] {
        let source = format!("%%{{init: {config}}}%%\nflowchart LR\nA[Measured label]-->B\n");
        let native = Renderer::new().render(RenderRequest::new(
            &source,
            RenderTarget::Svg(request(true)),
            OperationControl::new(),
        ));
        let error = native
            .err()
            .expect("native typography mismatch must fail admission");
        assert!(error.to_string().contains(expected), "{error}");
        let compatibility = Renderer::new().render(RenderRequest::new(
            &source,
            RenderTarget::Svg(request(false)),
            OperationControl::new(),
        ));
        assert!(compatibility.is_ok(), "{:?}", compatibility.err());
    }
}

#[test]
fn native_flowchart_admits_px_size_and_node_font_weight() {
    let source = "%%{init: {\"themeVariables\":{\"fontSize\":\"24px\"}}}%%\nflowchart LR\nA[Measured label]-->B\nstyle A font-weight:bold\n";
    let result = Renderer::new().render(RenderRequest::new(
        source,
        RenderTarget::Svg(request(true)),
        OperationControl::new(),
    ));
    assert!(result.is_ok(), "{:?}", result.err());
}
