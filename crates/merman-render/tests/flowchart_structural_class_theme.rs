#![cfg(feature = "diagram-flowchart")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::json;

#[test]
fn structural_classdefs_keep_source_order_and_target_the_label_mode() {
    for classes in [
        [
            ("root", "#aabbcc", "rgb(170, 187, 204)"),
            ("nodes", "#ccbbaa", "rgb(204, 187, 170)"),
        ],
        [
            ("nodes", "#ccbbaa", "rgb(204, 187, 170)"),
            ("root", "#aabbcc", "rgb(170, 187, 204)"),
        ],
    ] {
        let [first, second] = classes;
        let source = format!(
            "flowchart LR\nA[Box]\nclassDef {} fill:{}\nclassDef {} fill:{}\n",
            first.0, first.1, second.0, second.1,
        );
        for html in [false, true] {
            let parsed = MermaidEngine::new()
                .with_site_config(SourcePresentationConfig::from_value(json!({
                    "layout":"dagre", "look":"classic", "htmlLabels":html,
                })))
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
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
            let selector_suffix = if html { ">*" } else { " rect" };
            let rules = classes.map(|(class, _, color)| {
                format!(" .{class}{selector_suffix}{{fill:{color}!important;}}")
            });
            assert!(css.find(&rules[0]).unwrap() < css.find(&rules[1]).unwrap());
            let node = document
                .descendants()
                .find(|node| node.attribute("data-id") == Some("A"))
                .unwrap();
            let rect = node
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            for class in ["root", "nodes"] {
                assert!(rect.ancestors().skip(2).any(|ancestor| {
                    ancestor.attribute("class").is_some_and(|value| {
                        value.split_ascii_whitespace().any(|part| part == class)
                    })
                }));
            }
            assert_eq!(
                node.descendants()
                    .any(|child| child.has_tag_name("foreignObject")),
                html
            );
        }
    }
}
