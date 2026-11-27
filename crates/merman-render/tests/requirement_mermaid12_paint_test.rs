use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

#[cfg(feature = "layout-shoal")]
#[test]
fn requirement_shoal_dark_edge_labels_and_look_definitions_survive_rebasing() {
    use merman_render::svg::rebase_svg_ids;
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    let source = r#"requirementDiagram
direction LR
element gradient {
  type: hardware
}
element drop-shadow {
  type: software
}
gradient - satisfies -> drop-shadow
"#;
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "layout":"shoal", "look":"neo", "htmlLabels":true,
            "themeVariables":{"useGradient":true},
        })),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{"schema_version":2,"extends":"cyberpunk","roles":{"text.primary":"#E1E2E3"}}"##,
            None,
        )
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let render = |session| {
        family::prepare(
            parsed.presentation(),
            &LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap()
    };
    let rendered = render(
        environment
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap(),
    );
    let original = roxmltree::Document::parse(rendered.svg()).unwrap();
    let css = original
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains("fill:#E1E2E3;}"));
    assert!(css.contains(".edgeLabel span{color:#E1E2E3;}"));
    let rendered = render(environment.begin_session().unwrap());
    let rebased = rebase_svg_ids(
        rendered.svg(),
        "preview",
        &environment.begin_session().unwrap(),
    )
    .unwrap();
    let document = roxmltree::Document::parse(&rebased).unwrap();
    let ids = document
        .descendants()
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids.iter().collect::<std::collections::HashSet<_>>().len(),
        ids.len()
    );
    let gradient_id = document
        .descendants()
        .find(|node| node.has_tag_name("linearGradient"))
        .unwrap()
        .attribute("id")
        .unwrap();
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(&format!("url(#{gradient_id})")));
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    let marker_id = edge
        .attribute("marker-end")
        .unwrap()
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
    );
    assert!(!edge.attribute("style").unwrap().contains(";;;"));
}

#[cfg(feature = "layout-shoal")]
#[test]
fn requirement_shoal_paints_sketch_edges_and_native_arrowheads() {
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: shoal\n  look: neo\n---\nrequirementDiagram\ndirection LR\nelement A {\ntype: hardware\n}\nelement B {\ntype: software\n}\nA - satisfies -> B\nA - contains -> B\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let render = |edge_style, marker_style| {
        let source = serde_json::json!({"schema_version": 2, "extends": "paper", "form": {
            "edge_style": edge_style, "marker_style": marker_style
        }});
        let theme = ThemeCompiler::new()
            .compile_json(source.to_string().as_bytes(), None)
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap()
    };
    let arrow_paths = [
        ("open", "M1 1L10 5L1 9", Some("none")),
        ("filled", "M0 0.75L10 5L0 9.25Z", None),
        ("stealth", "M0 1L10 5L0 9L2.5 5Z", None),
    ];
    for (style, path, fill) in arrow_paths {
        let rendered = render("sketch", style);
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let edge = doc
            .descendants()
            .find(|node| node.attribute("marker-end").is_some())
            .unwrap();
        let marker_id = edge
            .attribute("marker-end")
            .unwrap()
            .trim_start_matches("url(#")
            .trim_end_matches(')');
        let marker = doc
            .descendants()
            .find(|node| node.attribute("id") == Some(marker_id))
            .unwrap();
        let head = marker.first_element_child().unwrap();
        assert_eq!(head.attribute("d"), Some(path), "{style}");
        assert_eq!(head.attribute("fill"), fill, "{style}");
        assert!(edge.attribute("d").unwrap().contains('C'));
        assert!(doc.descendants().any(|node| {
            node.attribute("marker-start")
                .is_some_and(|value| value.contains("containsStart"))
        }));
    }
    let rendered = render("straight", "open");
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        doc.descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .all(|node| !node.attribute("d").unwrap().contains('C'))
    );
}

fn render_requirement(layout: &str, look: &str) -> String {
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: {look}\n  themeVariables:\n    strokeWidth: 3\n    mainBkg: '#123456'\n    requirementBackground: '#abcdef'\n---\nrequirementDiagram\nrequirement Req {{\n  id: 1\n  text: body\n  risk: high\n  verifymethod: analysis\n}}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

#[test]
fn requirement_neo_markers_and_divider_use_source_paint_rules() {
    for look in ["neo", "classic"] {
        let svg = render_requirement("dagre", look);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let arrow = document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.ends_with("requirement_arrowEnd"))
            })
            .unwrap();
        assert_eq!(
            arrow.attribute("markerUnits"),
            (look == "neo").then_some("userSpaceOnUse")
        );
        assert_eq!(
            arrow.attribute("stroke-width"),
            (look == "neo").then_some("3")
        );
        assert_eq!(
            arrow.attribute("viewBox"),
            (look == "neo").then_some("0 0 25 20")
        );
        assert_eq!(
            arrow
                .first_element_child()
                .unwrap()
                .attribute("stroke-linejoin"),
            (look == "neo").then_some("miter")
        );
        let contains = document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.ends_with("requirement_containsStart"))
            })
            .unwrap();
        for shape in contains
            .descendants()
            .filter(|node| node.has_tag_name("circle") || node.has_tag_name("line"))
        {
            assert_eq!(
                shape.attribute("stroke-width"),
                (look == "neo").then_some("3")
            );
        }
        let outer = document
            .descendants()
            .find(|node| node.attribute("class") == Some("basic label-container outer-path"))
            .unwrap();
        assert_eq!(
            outer.first_element_child().unwrap().attribute("fill"),
            Some("#123456")
        );
        let divider = document
            .descendants()
            .find(|node| node.attribute("class") == Some("divider"))
            .unwrap();
        assert_eq!(
            divider.children().filter(|node| node.is_element()).count(),
            if look == "neo" { 2 } else { 1 }
        );
        if look == "neo" {
            let fill = divider.first_element_child().unwrap();
            assert_eq!(fill.attribute("fill"), Some("#123456"));
            assert_eq!(fill.attribute("fill-rule"), Some("evenodd"));
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn requirement_elk_uses_common_edge_layer_class() {
    let svg = render_requirement("elk", "neo");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("edges edgePaths"))
    );
}
