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
fn native_source_geometry_guards_follow_normalized_css_properties() {
    let families = [
        "%%{init: {\"htmlLabels\":false}}%%\nflowchart LR\nA[Measured label]\n",
        #[cfg(feature = "diagram-class")]
        "classDiagram\nclass A\n",
    ];
    for family in families {
        for style in [
            "letter-spacing /* comment */ :20px",
            "/* comment */letter-spacing:20px",
            "letter-spacing\u{2003}:20px",
        ] {
            let source = format!("{family}style A {style}\n");
            let error = Renderer::new()
                .render(RenderRequest::new(
                    &source,
                    RenderTarget::Svg(request(true)),
                    OperationControl::new(),
                ))
                .expect_err("comments cannot admit unmeasured source geometry");
            assert!(
                error.to_string().contains("letter-spacing"),
                "{source}: {error}"
            );
            let compatibility = Renderer::new().render(RenderRequest::new(
                &source,
                RenderTarget::Svg(request(false)),
                OperationControl::new(),
            ));
            let merman::RenderOutput::Svg(Some(output)) =
                compatibility.unwrap_or_else(|error| panic!("{source}: {error}"))
            else {
                panic!("expected compatibility SVG");
            };
            let document = roxmltree::Document::parse(output.svg()).unwrap();
            assert!(
                document.descendants().any(|node| node
                    .attribute("style")
                    .is_some_and(|style| style.contains("letter-spacing:20px"))),
                "source normalization emits the rejected geometry style: {source}"
            );
        }
    }
}

#[test]
fn native_source_font_names_do_not_create_geometry_declarations() {
    let source = r#"flowchart LR
A[Measured label]
style A font-family:"Face;letter-spacing:20px",monospace
"#;
    let result = Renderer::new().render(RenderRequest::new(
        source,
        RenderTarget::Svg(request(true)),
        OperationControl::new(),
    ));
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn native_flowchart_measures_static_html_label_typography() {
    let source = "%%{init: {\"fontFamily\":\"Cousine\",\"flowchart\":{\"minNodeWidth\":0,\"wrappingWidth\":400}}}%%\nflowchart TB\nA[\"Alpha Beta<br/>Gamma Delta\"] -->|Alpha Beta| B[End]\n";
    let svg_request = request(true);
    let render = |source: &str| {
        let merman::RenderOutput::Svg(Some(output)) = Renderer::new()
            .render(RenderRequest::svg(
                source,
                OperationControl::new(),
                svg_request.clone(),
            ))
            .unwrap_or_else(|error| panic!("{source}: {error}"))
        else {
            panic!("expected SVG");
        };
        let document = roxmltree::Document::parse(output.svg()).unwrap();
        let size = |class| {
            let label = document
                .descendants()
                .find(|element| {
                    element.has_tag_name("foreignObject")
                        && element.ancestors().any(|parent| {
                            parent.attribute("class").is_some_and(|classes| {
                                classes.split_ascii_whitespace().any(|name| name == class)
                            })
                        })
                })
                .unwrap();
            let number = |name| label.attribute(name).unwrap().parse::<f64>().unwrap();
            (number("width"), number("height"))
        };
        (size("node"), size("edgeLabel"))
    };
    let (unstyled_node, unstyled_edge) = render(source);
    let (styled_node, styled_edge) = render(&format!(
        "{source}style A letter-spacing:4px,word-spacing:8px,line-height:3,text-transform:uppercase\nlinkStyle 0 letter-spacing:4px\n"
    ));
    assert_eq!(styled_node.0 - unstyled_node.0, 52.0);
    assert_eq!(styled_node.1, 84.0);
    assert!((styled_edge.0 - unstyled_edge.0 - 40.0).abs() < 1.0 / 64.0);
    assert_eq!(
        render(&format!(
            "{source}classDef measured letter-spacing:4px,word-spacing:8px,line-height:3,text-transform:uppercase\nclass A measured\nlinkStyle default letter-spacing:4px\n"
        )),
        (styled_node, styled_edge),
    );
}

#[test]
fn native_flowchart_checks_only_active_source_typography() {
    let source = "flowchart LR\nA[\"Alpha<br/>Beta\"] -->|Measured label| B[End]\n";
    let render = |source: &str| {
        Renderer::new().render(RenderRequest::new(
            source,
            RenderTarget::Svg(request(true)),
            OperationControl::new(),
        ))
    };
    for styles in [
        "classDef unused line-height:2rem\n",
        "classDef tall line-height:2rem\nclass A tall\nstyle A line-height:3\n",
        "linkStyle default line-height:2rem\nlinkStyle 0 line-height:3\n",
    ] {
        let result = render(&format!("{source}{styles}"));
        assert!(result.is_ok(), "{styles}: {:?}", result.err());
    }
    for styles in [
        "classDef tall line-height:2rem\nclass A tall\n",
        "classDef root line-height:2rem\n",
        "classDef tall line-height:2rem !important\nclass A tall\nstyle A line-height:3\n",
        "linkStyle default line-height:2rem\n",
    ] {
        let error = render(&format!("{source}{styles}")).expect_err("unmeasured active style");
        assert!(
            error.to_string().contains("line-height"),
            "{styles}: {error}"
        );
    }
    assert!(render("flowchart LR\nA --> B\nlinkStyle default line-height:2rem\n").is_ok());
}

#[test]
fn native_source_font_sizes_reject_rem_only_when_it_wins_within_the_style_list() {
    let families = [
        "flowchart LR\nA[Measured label]\n",
        #[cfg(feature = "diagram-class")]
        "classDiagram\nclass A\n",
    ];
    for family in families {
        for (style, accepted) in [
            ("font-size:2rem", false),
            ("font-size:2rem,font-size:24px", true),
            ("font-size:2rem !important,font-size:24px", false),
            ("font-size:2rem !important,font-size:24px !important", true),
        ] {
            let source = format!("{family}style A {style}\n");
            let result = Renderer::new().render(RenderRequest::new(
                &source,
                RenderTarget::Svg(request(true)),
                OperationControl::new(),
            ));
            if accepted {
                assert!(result.is_ok(), "{source}: {:?}", result.err());
            } else {
                let error = result.expect_err("native measurement cannot infer the document root");
                assert!(error.to_string().contains("font-size"), "{error}");
                assert!(error.to_string().contains("document-root"), "{error}");
            }
            let compatibility = Renderer::new().render(RenderRequest::new(
                &source,
                RenderTarget::Svg(request(false)),
                OperationControl::new(),
            ));
            assert!(compatibility.is_ok(), "{source}: {:?}", compatibility.err());
        }
    }
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
            "Literal: &#96;<span style='font-size:64px'>AVAVAV</span>&#96;",
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
