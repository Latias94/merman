use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn render(source: &str, theme: &str, look: &str) -> String {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "theme": theme,
        "look": look,
        "layout": "dagre",
        "themeVariables": {
            "THEME_COLOR_LIMIT": 1,
            "borderColorArray": ["#112233", "#223344", "#334455"],
            "bkgColorArray": ["#ddeeff", "#ccddee"]
        }
    })));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("Flowchart model");
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare Flowchart");
    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Flowchart SVG")
        .svg()
        .to_owned()
}

#[test]
fn flowchart_container_palette_follows_preorder_across_collapsed_and_reordered_roots() {
    let outer =
        "subgraph Outer\nsubgraph ChildA\nA[First]\nend\nsubgraph ChildB\nB[Second]\nend\nend\n";
    let sibling = "subgraph Sibling\nC[Third]\nend\n";
    for (source, expected) in [
        (
            format!("flowchart TB\n{outer}{sibling}ChildA@{{view: collapsed}}\n"),
            [
                ("Outer", "color-0"),
                ("ChildA", "color-1"),
                ("ChildB", "color-2"),
                ("Sibling", "color-0"),
            ],
        ),
        (
            format!("flowchart TB\n{sibling}{outer}ChildA@{{view: collapsed}}\n"),
            [
                ("Sibling", "color-0"),
                ("Outer", "color-1"),
                ("ChildA", "color-2"),
                ("ChildB", "color-0"),
            ],
        ),
    ] {
        let svg = render(&source, "redux-color", "classic");
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        for (id, slot) in expected {
            let scoped = format!("merman-{id}");
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some(scoped.as_str()))
                .unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(node.attribute("data-color-id"), Some(slot), "{id}");
        }
        for node in document.descendants().filter(|node| {
            node.attribute("id")
                .is_some_and(|id| id.starts_with("merman-flowchart-"))
        }) {
            assert_eq!(
                node.attribute("data-color-id"),
                None,
                "ordinary steps are not palette participants"
            );
        }
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .unwrap();
        assert!(css.contains(r##"[data-color-id="color-2"].cluster:not(.swimlane) path{stroke:#334455;fill:#ddeeff;}"##));
        assert!(!css.contains(r#"data-color-id="color-3""#));
        let root_end = css.find("#merman p{margin:0;}").unwrap();
        let palette_start = css
            .find(r#"#merman [data-look="classic"][data-color-id="color-0"]"#)
            .unwrap();
        let label_start = css.find("#merman .label{font-family:").unwrap();
        assert!(
            root_end < palette_start && palette_start < label_start,
            "container palette belongs at the family stylesheet boundary"
        );
        assert!(css.contains(r##".cluster:not(.swimlane) rect{stroke:#112233;fill:#ddeeff;}#merman [data-look="classic"][data-color-id="color-0"].cluster:not(.swimlane) path{stroke:#112233;fill:#ddeeff;}"##),
            "rect and path rules retain upstream order and separate bodies");
    }
}

#[test]
fn flowchart_palette_styles_cover_collapsed_rough_shapes_without_overriding_author_styles() {
    let source = "flowchart TB\nsubgraph Outer\nA[Worker]\nend\nOuter@{view: collapsed}\nclassDef own fill:#abcdef,stroke:#fedcba\nclass Outer own\n";
    let svg = render(source, "redux-color", "handDrawn");
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let container = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-Outer"))
        .unwrap();
    assert_eq!(container.attribute("data-color-id"), Some("color-0"));
    assert!(
        container
            .attribute("class")
            .unwrap()
            .split_whitespace()
            .any(|class| class == "rough-node")
    );
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    for suffix in [
        ".collapsed-group",
        ".collapsed-group path",
        ".collapsed-indicator",
        ".collapsed-separator",
    ] {
        assert!(css.contains(&format!(
            r#"[data-look="handDrawn"][data-color-id="color-0"].rough-node {suffix}"#
        )));
    }
    let shape = container
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("#abcdef"))
        })
        .expect("explicit container styles survive the palette");
    assert!(shape.attribute("style").unwrap().contains("#fedcba"));

    let plain = render(source, "default", "classic");
    assert!(
        !plain.contains("data-color-id"),
        "custom palette arrays alone do not opt a plain theme into categorical colors"
    );
}
