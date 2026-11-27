#![cfg(any(
    all(feature = "diagram-architecture", feature = "layout-cytoscape"),
    feature = "diagram-block",
    feature = "diagram-c4",
    all(feature = "diagram-requirement", feature = "layout-dagre"),
))]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::{LayoutOptions, environment::RenderEnvironment, family, svg::SvgRenderOptions};
use merman_render_core::svg_css::{CssResourceUrl, visit_css_resource_urls};
use roxmltree::Document;
use serde_json::{Value, json};
use std::collections::HashSet;

fn render(source: &str, config: Value) -> String {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("defs-audit".to_owned()),
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap()
    .svg()
    .to_owned()
}

fn assert_fragment_closure_after_rebasing(svg: &str) {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let rebased = merman_render::svg::rebase_svg_ids(svg, "preview", &session).unwrap();
    for svg in [svg, rebased.as_str()] {
        let document = Document::parse(svg).unwrap();
        let mut ids = HashSet::new();
        for id in document
            .descendants()
            .filter_map(|node| node.attribute("id"))
        {
            assert!(ids.insert(id), "duplicate SVG ID {id}");
        }
        let check_urls = |css: &str| {
            visit_css_resource_urls(css, |url| {
                if let CssResourceUrl::SameDocumentFragment(id) = url {
                    assert!(ids.contains(id), "unresolved fragment {id}");
                }
            })
            .unwrap();
        };
        for node in document.descendants().filter(roxmltree::Node::is_element) {
            if node.has_tag_name("style") {
                check_urls(node.text().unwrap());
            }
            for attr in node.attributes() {
                if attr.value().contains("url(") {
                    check_urls(attr.value());
                }
                if attr.name() == "href"
                    && let Some(id) = attr.value().strip_prefix('#')
                {
                    assert!(ids.contains(id), "unresolved href {id}");
                }
            }
        }
    }
}

#[cfg(any(
    feature = "diagram-c4",
    all(feature = "diagram-architecture", feature = "layout-cytoscape")
))]
fn assert_builtin_shadow(svg: &str) {
    assert!(svg.contains("url(#defs-audit-drop-shadow)"));
    let document = Document::parse(svg).unwrap();
    let shadow = document
        .descendants()
        .find(|node| {
            node.has_tag_name("filter") && node.attribute("id") == Some("defs-audit-drop-shadow")
        })
        .unwrap();
    let effect = shadow.first_element_child().unwrap();
    assert!(effect.has_tag_name("feDropShadow"));
    assert_eq!(effect.attribute("flood-color"), Some("#FFFFFF"));
}

#[cfg(feature = "diagram-c4")]
#[test]
fn legacy_c4_keeps_authored_aliases_separate_from_symbols_markers_and_look_definitions() {
    let svg = render(
        r#"C4Context
System(database, "Database alias")
System("c4-node-0", "Authored fallback")
System(arrowhead, "Arrow alias")
System("drop-shadow", "Shadow alias")
System(gradient, "Gradient alias")
Rel(database, gradient, "Queries")
"#,
        json!({"look": "neo", "theme": "redux-dark-color", "themeVariables": {"useGradient": true}}),
    );
    let document = Document::parse(&svg).unwrap();
    for (suffix, tag) in [
        ("database", "symbol"),
        ("arrowhead", "marker"),
        ("gradient", "linearGradient"),
    ] {
        let id = format!("defs-audit-{suffix}");
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name(tag) && node.attribute("id") == Some(&id))
        );
    }
    for alias in ["database", "arrowhead", "drop-shadow", "gradient"] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("data-et") == Some("node")
                    && node.attribute("data-id") == Some(alias)),
            "missing node alias {alias}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("id") == Some("defs-audit-c4-node-0"))
    );
    assert_builtin_shadow(&svg);
    assert_fragment_closure_after_rebasing(&svg);
}

#[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
#[test]
fn legacy_architecture_emits_the_builtin_shadow_when_its_stylesheet_references_it() {
    let source =
        "architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R --> L:db\n";
    let svg = render(source, json!({"look": "classic", "theme": "redux-dark"}));
    assert_builtin_shadow(&svg);
    assert_fragment_closure_after_rebasing(&svg);

    let svg = render(source, json!({"look": "classic", "theme": "default"}));
    assert!(!svg.contains("defs-audit-drop-shadow"));
}

#[cfg(all(feature = "diagram-requirement", feature = "layout-dagre"))]
#[test]
fn legacy_requirement_keeps_authored_aliases_separate_from_look_definitions() {
    let source = r#"requirementDiagram
direction LR
element gradient {
  type: hardware
}
element drop-shadow {
  type: software
}
element requirement-node-0 {
  type: service
}
gradient - satisfies -> drop-shadow
"#;
    for layout in [
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ] {
        let svg = render(
            source,
            json!({"layout": layout, "look": "neo", "themeVariables": {"useGradient": true}}),
        );
        let document = Document::parse(&svg).unwrap();
        for alias in ["gradient", "drop-shadow"] {
            assert!(
                document
                    .descendants()
                    .any(|node| node.attribute("data-et") == Some("node")
                        && node.attribute("data-id") == Some(alias)),
                "{layout}: missing node alias {alias}"
            );
        }
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some("defs-audit-requirement-node-0"))
        );
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("linearGradient")
                    && node.attribute("id") == Some("defs-audit-gradient"))
        );
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("filter")
                    && node.attribute("id") == Some("defs-audit-drop-shadow"))
        );
        assert_fragment_closure_after_rebasing(&svg);
    }
}

#[cfg(feature = "diagram-block")]
#[test]
fn legacy_block_preserves_the_gradient_alias_when_the_definition_is_inactive() {
    let source = "block\ncolumns 2\ngradient[\"Gradient alias\"] B[\"Target\"]\ngradient --> B\n";
    for use_gradient in [false, true] {
        let svg = render(
            source,
            json!({"look": "neo", "themeVariables": {"useGradient": use_gradient}}),
        );
        let document = Document::parse(&svg).unwrap();
        let gradient_id = document
            .descendants()
            .find(|node| node.attribute("id") == Some("defs-audit-gradient"))
            .unwrap();
        assert!(gradient_id.has_tag_name(if use_gradient { "linearGradient" } else { "g" }));
        assert_eq!(
            document
                .descendants()
                .any(|node| node.attribute("data-et") == Some("node")
                    && node.attribute("data-id") == Some("gradient")),
            use_gradient
        );
        assert_fragment_closure_after_rebasing(&svg);
    }
}
