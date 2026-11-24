#![cfg(feature = "diagram-flowchart")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::json;

#[test]
fn structural_edge_rules_drop_external_paints_and_preserve_safe_declarations() {
    let source = "flowchart LR\nA --> B\n\
        classDef root stroke:url(https://example.com/paint),stroke-width:3px\n\
        classDef edgePaths stroke:var(--edge-color),stroke-width:5px\n";
    for html in [false, true] {
        let parsed = MermaidEngine::new()
            .with_site_config(SourcePresentationConfig::from_value(json!({
                "layout":"dagre", "look":"classic", "htmlLabels":html,
            })))
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let rendered = family::prepare(
            &parsed,
            &LayoutOptions::headless_svg_defaults(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .unwrap();
        let suffix = if html { ">*" } else { " path" };
        let root = format!(".root{suffix}{{stroke-width:3px!important;}}");
        let edge_paths = format!(
            ".edgePaths{suffix}{{stroke:var(--edge-color)!important;stroke-width:5px!important;}}"
        );
        assert!(!rendered.svg().contains("https://example.com/paint"));
        assert!(css.find(&root).unwrap() < css.find(&edge_paths).unwrap());
        let edge = document
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .unwrap();
        assert_eq!(
            edge.parent_element().unwrap().attribute("class"),
            Some("edgePaths")
        );
        assert!(edge.attribute("marker-end").is_some());
    }
}
