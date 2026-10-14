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
        let error = native.expect_err("native typography mismatch must fail admission");
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

#[test]
fn native_flowchart_rejects_unmeasured_inline_html_typography() {
    for (style, property) in [
        ("font-size:64px", "font-size"),
        ("letter-spacing:20px", "letter-spacing"),
        ("font:italic 64px monospace", "font"),
        ("font-family:monospace", "font-family"),
        ("font-style:italic", "font-style"),
        ("font-weight:700", "font-weight"),
        (
            "color:red; FONT-SIZE /* comment */ :64px !important",
            "font-size",
        ),
    ] {
        let source = format!("flowchart LR\nA[\"<span style='{style}'>AVAVAV</span>\"]-->B\n");
        let error = Renderer::new()
            .render(RenderRequest::new(
                &source,
                RenderTarget::Svg(request(true)),
                OperationControl::new(),
            ))
            .expect_err("unmeasured inline typography must fail native admission");
        assert!(
            error
                .to_string()
                .contains("inline HTML text geometry style"),
            "{style}: {error}"
        );
        assert!(error.to_string().contains(property), "{style}: {error}");
        assert!(error.to_string().contains("deterministic"), "{error}");
        let compatibility = Renderer::new().render(RenderRequest::new(
            &source,
            RenderTarget::Svg(request(false)),
            OperationControl::new(),
        ));
        assert!(compatibility.is_ok(), "{style}: {:?}", compatibility.err());
    }
}

#[test]
fn native_flowchart_allows_paint_styles_and_literal_typography_text() {
    for label in [
        "<span style='color:red;background-color:blue'>AVAVAV</span>",
        "font-size:64px and letter-spacing:20px",
        "<span title='font-size:64px;letter-spacing:20px'>AVAVAV</span>",
        "<span style='color:red' title='style=font-size:64px'>AVAVAV</span>",
        "&lt;span style='font-size:64px'&gt;AVAVAV&lt;/span&gt;",
        "<span style='font-size:;color:red'>AVAVAV</span>",
    ] {
        let source = format!("flowchart LR\nA[\"{label}\"]-->B\n");
        let result = Renderer::new().render(RenderRequest::new(
            &source,
            RenderTarget::Svg(request(true)),
            OperationControl::new(),
        ));
        assert!(result.is_ok(), "{label}: {:?}", result.err());
    }
}

#[test]
fn native_flowchart_does_not_inspect_svg_literal_tags() {
    let source = "%%{init: {\"htmlLabels\":false}}%%\nflowchart LR\nA[\"<span style='font-size:64px;letter-spacing:20px'>AVAVAV</span>\"]-->B\n";
    let result = Renderer::new().render(RenderRequest::new(
        source,
        RenderTarget::Svg(request(true)),
        OperationControl::new(),
    ));
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn native_flowchart_admission_follows_effective_markdown_html() {
    for (label, property) in [
        (
            "Literal: `<span style='font-size:64px'>AVAVAV</span>`",
            "font-size",
        ),
        (
            r"Literal: \<span style='letter-spacing:20px'>AVAVAV\</span>",
            "letter-spacing",
        ),
        (
            "Before <span style='font-size:64px'>AVAVAV</span> after",
            "font-size",
        ),
    ] {
        let source = format!("flowchart LR\nA[\"`{label}`\"]-->B\n");
        let error = Renderer::new()
            .render(RenderRequest::new(
                &source,
                RenderTarget::Svg(request(true)),
                OperationControl::new(),
            ))
            .expect_err("effective HTML typography must fail native admission");
        assert!(error.to_string().contains(property), "{label}: {error}");

        // Mermaid outputs unsupported Markdown tokens as raw HTML, including code spelling.
        let merman::RenderOutput::Svg(Some(output)) = Renderer::new()
            .render(RenderRequest::new(
                &source,
                RenderTarget::Svg(request(false)),
                OperationControl::new(),
            ))
            .unwrap()
        else {
            panic!("expected compatibility SVG");
        };
        let svg = output.into_parts().0;
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            document.descendants().any(|node| {
                node.has_tag_name("span")
                    && node
                        .attribute("style")
                        .is_some_and(|style| style.contains(property))
            }),
            "the guard must match actual HTML emission: {label}"
        );
    }
}

#[test]
fn native_flowchart_admits_inherited_inline_typography() {
    for property in [
        "font-family",
        "font-size",
        "font-weight",
        "font-style",
        "letter-spacing",
    ] {
        let source = format!(
            "flowchart LR\nA[\"<span style='{property}:inherit !important'>AVAVAV</span>\"]-->B\n"
        );
        let result = Renderer::new().render(RenderRequest::new(
            &source,
            RenderTarget::Svg(request(true)),
            OperationControl::new(),
        ));
        assert!(result.is_ok(), "{property}: {:?}", result.err());
    }
}

#[test]
fn native_flowchart_rejects_inheritance_that_overrides_element_typography() {
    for (tag, property) in [
        ("strong", "font-weight"),
        ("b", "font-weight"),
        ("em", "font-style"),
        ("i", "font-style"),
        ("code", "font-family"),
        ("strong", "font"),
        ("em", "font"),
        ("code", "font"),
    ] {
        let source = format!(
            "flowchart LR\nA[\"<{tag} style='{property}:inherit !important'>AVAVAV</{tag}>\"]-->B\n"
        );
        let error = Renderer::new()
            .render(RenderRequest::new(
                &source,
                RenderTarget::Svg(request(true)),
                OperationControl::new(),
            ))
            .expect_err("intrinsic element typography must remain consistent with measurement");
        assert!(
            error.to_string().contains(property),
            "{tag}/{property}: {error}"
        );
    }
}
