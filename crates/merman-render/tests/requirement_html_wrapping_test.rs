#![cfg(feature = "diagram-requirement")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

#[test]
fn requirement_html_min_content_expansion_keeps_the_original_wrap_constraint() {
    for (body, size) in [
        ("W".repeat(120), 14),
        (
            format!(
                "Every report retains its reviewed source. {}",
                "W".repeat(40)
            ),
            48,
        ),
        (
            format!(
                "<span style='font-size:36px;font-weight:700'>Every report retains its reviewed source. {}</span>",
                "W".repeat(40)
            ),
            14,
        ),
    ] {
        let source = format!("requirementDiagram\nrequirement req {{\ntext: \"{body}\"\n}}\n");
        let diagram = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(json!({
                    "htmlLabels": true,
                    "layout": "dagre",
                    "look": "classic",
                    "fontFamily": "Arial",
                    "fontSize": 10,
                    "themeVariables": {"fontFamily": "Arial", "fontSize": format!("{size}px")}
                })),
            )
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .expect("parse Requirement wrapping case")
            .expect("detect Requirement")
            .into_presentation();
        let artifact = family::prepare(
            &diagram,
            &LayoutOptions::headless_svg_defaults(),
            RenderEnvironment::deterministic()
                .begin_session()
                .expect("begin Requirement session"),
        )
        .expect("prepare Requirement wrapping case");
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render Requirement wrapping case");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Requirement SVG");
        let foreign_object = document
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node.descendants().any(|child| {
                        child.is_text()
                            && child.text().is_some_and(|text| text.starts_with("Text:"))
                    })
            })
            .expect("Requirement body label");
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .expect("Requirement HTML wrapper");
        let style = div.attribute("style").expect("Requirement wrapper style");
        let max_width = style
            .split(';')
            .find_map(|declaration| declaration.trim().strip_prefix("max-width:"))
            .expect("original createText wrap constraint")
            .trim()
            .strip_suffix("px")
            .expect("pixel wrap constraint")
            .parse::<f64>()
            .expect("numeric wrap constraint");
        let width = foreign_object
            .attribute("width")
            .expect("measured HTML width")
            .parse::<f64>()
            .expect("numeric HTML width");
        let width_style = format!("width: {max_width}px");
        assert!(width > max_width, "long word expands min-content: {body}");
        assert!(
            style.contains("display: table;")
                && style.contains("white-space: break-spaces;")
                && style
                    .split(';')
                    .any(|declaration| declaration.trim() == width_style),
            "Requirement must retain wrapping when min-content exceeds the cap: {style}"
        );
    }
}
