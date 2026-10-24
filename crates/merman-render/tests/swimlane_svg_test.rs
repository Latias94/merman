use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::SwimlaneLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_swimlane_with_layout(source: &str, diagram_id: &str) -> (String, SwimlaneLayout) {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse swimlane")
        .expect("detect swimlane");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let layout = serde_json::from_value(
        artifact.layout_json().unwrap()["layout"]["SwimlaneDiagram"].clone(),
    )
    .unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_string();
    (svg, layout)
}

fn edge_label<'a>(
    document: &'a roxmltree::Document<'a>,
    layout: &SwimlaneLayout,
) -> roxmltree::Node<'a, 'a> {
    let label_id = layout.edges[0].label_node_id.as_deref().unwrap();
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("label edgeLabel")
                && node.attribute("id") == Some(label_id)
        })
        .expect("swimlane edge label")
}

fn text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect()
}

#[test]
fn long_html_edge_labels_wrap_with_the_measured_content_box() {
    for width in [96, 144] {
        let source = format!(
            r#"---
config:
  htmlLabels: true
  themeVariables:
    fontSize: 22px
  flowchart:
    wrappingWidth: {width}
---
swimlane-beta LR
A -->|First <b>review</b> the complete request before sending the final reply| B
"#
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "wrapped-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = edge_label(&document, &layout);
        let foreign_object = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let style = div.attribute("style").unwrap();
        assert!(
            style.contains("white-space: break-spaces"),
            "long HTML labels must wrap at {width}px: {style}"
        );
        assert!(style.contains(&format!("max-width: {width}px;")));
        assert!(style.contains(&format!("width: {width}px;")));
        assert!(
            foreign_object
                .attribute("style")
                .is_some_and(|style| style.contains("overflow: visible"))
        );
        let measured = layout.nodes.iter().find(|node| node.is_edge_label).unwrap();
        assert_eq!(
            foreign_object
                .attribute("width")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            measured.label_width
        );
        assert_eq!(
            foreign_object
                .attribute("height")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            measured.label_height
        );
        assert!(
            measured.label_height > 33.0,
            "22px labels must span multiple lines"
        );
        assert_eq!(
            text_content(foreign_object),
            "First review the complete request before sending the final reply"
        );
    }
}

#[test]
fn html_edge_label_wrapping_uses_only_the_first_mermaid_label_style() {
    for (styles, font_size, needs_wrap) in [
        (
            "linkStyle default font-size:24px,font-weight:bold\nlinkStyle 0 font-size:12px",
            24,
            true,
        ),
        ("linkStyle 0 font-size:12px,font-weight:bold", 12, false),
        (
            "linkStyle default fill:blue,font-size:24px\nlinkStyle 0 font-size:24px",
            8,
            false,
        ),
    ] {
        let source = format!(
            r#"---
config:
  htmlLabels: true
  themeVariables:
    fontSize: 8px
  flowchart:
    wrappingWidth: 144
---
swimlane-beta LR
A styled@-->|Review request| B
{styles}
"#
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "styled-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = edge_label(&document, &layout);
        let foreign_object = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let style = div.attribute("style").unwrap();
        assert_eq!(
            style.contains("white-space: break-spaces"),
            needs_wrap,
            "{styles}: {style}"
        );
        assert_eq!(style.contains("white-space: nowrap"), !needs_wrap);
        assert!(
            !style.contains("font-weight"),
            "only the first style is copied"
        );
        if font_size == 8 {
            assert!(style.contains("color:blue"), "{style}");
            assert!(!style.contains("font-size"));
        } else {
            assert!(style.contains(&format!("font-size:{font_size}px")));
        }
        let measured = layout.nodes.iter().find(|node| node.is_edge_label).unwrap();
        assert_eq!(
            measured.label_height > f64::from(font_size) * 1.5,
            needs_wrap,
            "{styles}: {measured:?}"
        );
    }
}
