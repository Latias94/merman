#![cfg(feature = "diagram-flowchart")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{BuiltinPreset, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

fn render(source: &str, config: Value, elevation: Option<Value>) -> String {
    let theme = elevation.map(|elevation| {
        let document = json!({
            "schema_version": 2, "extends": "paper", "form": {"elevation": elevation},
        });
        NativeTheme::new(
            ThemeCompiler::new()
                .compile_json(&serde_json::to_vec(&document).unwrap(), None)
                .unwrap(),
        )
    });
    render_with_theme(source, config, theme)
}

fn render_with_theme(source: &str, config: Value, theme: Option<NativeTheme>) -> String {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic();
    let session = match theme {
        Some(theme) => environment.begin_session_with_native_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn node<'a, 'input>(document: &'a Document<'input>, id: &str) -> Node<'a, 'input> {
    document
        .descendants()
        .find(|element| {
            element.attribute("data-et") == Some("node") && element.attribute("data-id") == Some(id)
        })
        .unwrap()
}

fn rect<'a, 'input>(document: &'a Document<'input>, id: &str) -> Node<'a, 'input> {
    node(document, id)
        .children()
        .find(|child| child.has_tag_name("rect"))
        .expect("the node still owns its direct shape child")
}

#[test]
fn native_theme_v2_flowchart_elevation_keeps_foreground_styles_and_expands_the_viewport() {
    let source =
        "flowchart LR\nA[Alpha]:::dim --> B[Beta]\nclassDef dim opacity:0.5\nstyle B filter:none\n";
    let config = json!({"layout": "dagre", "look": "classic", "htmlLabels": true});
    let flat = render(source, config.clone(), Some(json!("flat")));
    let raised = render(
        source,
        config,
        Some(json!({"kind":"glow","color":"#4A5B6C","blur":8,"opacity":0.5})),
    );
    let flat = Document::parse(&flat).unwrap();
    let raised = Document::parse(&raised).unwrap();
    for id in ["A", "B"] {
        assert_eq!(
            node(&flat, id).attribute("transform"),
            node(&raised, id).attribute("transform")
        );
    }
    let view_box_size = |document: &Document<'_>| {
        let dimensions: Vec<f64> = document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|dimension| dimension.parse().unwrap())
            .collect();
        (dimensions[2], dimensions[3])
    };
    let (flat_width, flat_height) = view_box_size(&flat);
    let (raised_width, raised_height) = view_box_size(&raised);
    assert!(raised_width > flat_width && raised_height > flat_height);

    let depth: Vec<_> = raised
        .descendants()
        .filter(|node| node.attribute("data-merman-theme-depth") == Some("node"))
        .collect();
    assert_eq!(depth.len(), 1, "B's authored filter owns its shape");
    assert_eq!(
        depth[0].attribute("transform"),
        node(&raised, "A").attribute("transform")
    );
    assert!(
        depth[0]
            .descendants()
            .all(|child| !child.has_tag_name("foreignObject"))
    );
    let path = depth[0]
        .children()
        .find(|child| child.has_tag_name("path"))
        .unwrap();
    assert_eq!(path.attribute("fill"), Some("#4A5B6C"));
    assert!(path.attribute("filter").is_some());
    let edge = raised
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    assert!(depth[0].range().end <= edge.range().start);
    assert!(
        node(&raised, "A")
            .descendants()
            .any(|child| child.has_tag_name("foreignObject"))
    );
    let shape = rect(&raised, "A");
    assert!(shape.attribute("filter").is_none());
    assert!(
        shape
            .attribute("style")
            .unwrap()
            .contains("opacity:0.5 !important")
    );
}

#[test]
fn native_theme_v2_uses_the_blueprint_border_for_neo_cylinders() {
    let theme = NativeTheme::new(
        ThemeCompiler::new()
            .compile_preset(BuiltinPreset::Blueprint, None)
            .unwrap(),
    );
    let svg = render_with_theme(
        "flowchart LR\nA[(Default)] --> B[(Authored)]\nstyle B stroke:#123456,stroke-width:3px",
        json!({"layout":"dagre", "look":"neo"}),
        Some(theme),
    );
    let document = Document::parse(&svg).unwrap();
    let css = document
        .descendants()
        .find(|element| element.has_tag_name("style"))
        .and_then(|element| element.text())
        .unwrap();
    assert!(css.contains(r#"[data-look="neo"].node path{stroke:#C5E4FF;stroke-width:1.25px;}"#));
    let authored = node(&document, "B")
        .children()
        .find(|element| element.has_tag_name("path"))
        .unwrap();
    let style = authored.attribute("style").unwrap();
    assert!(style.contains("stroke:#123456") || style.contains("stroke:rgb(18, 52, 86)"));
    assert!(style.contains("stroke-width:3px"));
}

#[test]
fn neo_rectangle_defaults_keep_shape_specific_radius() {
    for (palette, default_radius) in [("default", "5"), ("neo", "3")] {
        for (shape, radius) in [
            ("[Rectangle]", None),
            ("(Rounded rectangle)", Some(default_radius)),
        ] {
            let source = format!("---\nconfig:\n  theme: {palette}\n---\nflowchart LR\nA{shape}\n");
            let svg = render(&source, json!({"layout":"dagre","look":"neo"}), None);
            let document = Document::parse(&svg).unwrap();
            let rect = rect(&document, "A");
            assert_eq!(rect.attribute("rx"), radius, "{source}");
            assert_eq!(rect.attribute("ry"), radius, "{source}");
        }
    }
}

#[test]
fn rectangle_source_radius_keeps_each_axis_and_declaration_precedence() {
    for look in ["classic", "neo"] {
        for shape in ["[Rectangle]", "(Rounded rectangle)"] {
            for (declarations, rx, ry) in [
                ("style A rx:4px", Some("4"), None),
                ("style A rx:4px!important,RX:6px", Some("4"), None),
                ("style A rx:4px,RX:6px,rx:8px", Some("8"), None),
                ("style A ry:6px", None, Some("6")),
                (
                    "classDef corner rx:4px,ry:6px\nclass A corner\nstyle A ry:2px",
                    Some("4"),
                    Some("2"),
                ),
                ("style A rx:4px!important,rx:8px,ry:0", Some("4"), Some("0")),
            ] {
                let source = format!(
                    "---\nconfig:\n  theme: default\n---\nflowchart LR\nA{shape}\n{declarations}"
                );
                let svg = render(&source, json!({"layout":"dagre","look":look}), None);
                let document = Document::parse(&svg).unwrap();
                let rect = rect(&document, "A");
                let fallback = shape.starts_with('(').then_some("5");
                assert_eq!(rect.attribute("rx"), rx.or(fallback), "{look} {source}");
                assert_eq!(rect.attribute("ry"), ry.or(fallback), "{look} {source}");
            }
        }
    }
}
