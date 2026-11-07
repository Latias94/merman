#![cfg(feature = "diagram-mindmap")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::MindmapDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use svgtypes::{PathParser, PathSegment};

fn render(source: &str, diagram_id: &str) -> (String, MindmapDiagramLayout) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = serde_json::from_value(projection["layout"]["MindmapDiagram"].clone()).unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned();
    (svg, layout)
}

fn node<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    dom_id: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|element| element.has_tag_name("g") && element.attribute("id") == Some(dom_id))
        .unwrap()
}

#[test]
fn mindmap_mermaid_12_3_neo_default_has_a_square_bottom() {
    // MM-001: defaultMindmapNode.ts selects this topology before browser measurement.
    for (look, quadratic_count) in [("neo", 2), ("classic", 4)] {
        let source = format!(
            "---\nconfig:\n  theme: default\n  look: {look}\n  layout: dagre\n  htmlLabels: false\n---\nmindmap\n  Root\n"
        );
        let (svg, layout) = render(&source, "mindmap-12-3-bottom");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = node(&document, "mindmap-12-3-bottom-node_0");
        let path = root
            .children()
            .find(|element| element.attribute("class") == Some("node-bkg node-0"))
            .unwrap();
        let segments = PathParser::from(path.attribute("d").unwrap())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            segments
                .iter()
                .filter(|segment| matches!(segment, PathSegment::Quadratic { .. }))
                .count(),
            quadratic_count
        );
        if look == "neo" {
            assert_eq!(root.attribute("data-look"), Some("neo"));
            let tail = &segments[segments.len() - 3..];
            let [
                PathSegment::VerticalLineTo { abs: false, y },
                PathSegment::HorizontalLineTo { abs: true, x },
                PathSegment::ClosePath { .. },
            ] = tail
            else {
                panic!("Neo must end with a straight vertical, absolute H and Z: {tail:?}");
            };
            assert!((*y - (layout.nodes[0].height - 5.0)).abs() < 0.001);
            assert!((*x + layout.nodes[0].width / 2.0).abs() < 0.001);
        }
    }
}

#[test]
fn mindmap_mermaid_12_3_fractional_padding_reaches_circle_geometry() {
    // MM-002: compare each render with its own label diagonal, independent of font metrics.
    for html_labels in [false, true] {
        let mut integer_label = None;
        let mut integer_radius = None;
        for padding in [10.0, 10.5] {
            let source = format!(
                "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: {html_labels}\n  mindmap:\n    padding: {padding}\n---\nmindmap\n  root((R))\n"
            );
            let (svg, layout) = render(&source, "mindmap-12-3-padding");
            let document = roxmltree::Document::parse(&svg).unwrap();
            let root = node(&document, "mindmap-12-3-padding-node_0");
            let circle = root
                .children()
                .find(|element| element.has_tag_name("circle"))
                .unwrap();
            let radius: f64 = circle.attribute("r").unwrap().parse().unwrap();
            let label = root
                .children()
                .find(|element| element.attribute("class") == Some("label"))
                .unwrap();
            let (x, y) = label
                .attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split_once(',')
                .unwrap();
            let label_center = (
                x.trim().parse::<f64>().unwrap(),
                y.trim().parse::<f64>().unwrap(),
            );
            // Radius and both translations are independently serialized to subpixel precision.
            assert!((radius - label_center.0.hypot(label_center.1) - padding).abs() < 0.002);
            let dimensions = &layout.nodes[0];
            assert!((dimensions.width - 2.0 * radius).abs() < 0.002);
            assert!((dimensions.height - 2.0 * radius).abs() < 0.002);
            if padding == 10.0 {
                integer_label = Some(label_center);
                integer_radius = Some(radius);
            } else {
                assert_eq!(Some(label_center), integer_label);
                assert!((radius - integer_radius.unwrap() - 0.5).abs() < 0.001);
            }
        }
    }
}

#[test]
fn mindmap_mermaid_12_3_bare_percent_text_survives_both_label_modes() {
    // MM-003: NODE_ID consumes the complete bare line, including its internal percent pair.
    for html_labels in [false, true] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  htmlLabels: {html_labels}\n---\nmindmap\n  Root %% literal\n"
        );
        let (svg, _) = render(&source, "mindmap-12-3-percent");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = node(&document, "mindmap-12-3-percent-node_0");
        let label = root
            .children()
            .find(|element| element.attribute("class") == Some("label"))
            .unwrap();
        let text: String = label
            .descendants()
            .filter(roxmltree::Node::is_text)
            .filter_map(|element| element.text())
            .collect();
        assert_eq!(text, "Root %% literal");
    }
}

#[test]
fn mindmap_mermaid_12_3_mixed_closer_renders_a_cloud_with_safe_label_content() {
    // MM-004: the generic closing token selects CLOUD for an opening '(' and ending ']'.
    for html_labels in [false, true] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  htmlLabels: {html_labels}\n  securityLevel: strict\n---\nmindmap\n  root(Label]\n    child(\"Text %% &lt;unsafe&gt; &amp; value<script>bad</script>\"]\n"
        );
        let (svg, _) = render(&source, "mindmap-12-3-cloud");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = node(&document, "mindmap-12-3-cloud-node_0");
        let path = root
            .descendants()
            .find(|element| element.has_tag_name("path"))
            .unwrap();
        assert!(
            PathParser::from(path.attribute("d").unwrap())
                .map(Result::unwrap)
                .any(|segment| matches!(segment, PathSegment::EllipticalArc { .. }))
        );
        let child = node(&document, "mindmap-12-3-cloud-node_1");
        let text: String = child
            .descendants()
            .filter(roxmltree::Node::is_text)
            .filter_map(|element| element.text())
            .collect();
        assert!(text.contains("Text %% "));
        assert!(text.contains("unsafe"));
        assert!(text.contains("value"));
        assert!(
            !document.descendants().any(|element| {
                element.has_tag_name("script") || element.has_tag_name("unsafe")
            })
        );
        assert!(!text.contains("bad"));
    }
}

#[test]
fn mindmap_mermaid_12_3_shape_resets_share_flowchart_wrapping_in_layout_and_svg() {
    // MM-006: mindmapRenderer.ts clears rect/rounded/hexagon widths, so labelHelper uses
    // flowchart.wrappingWidth (120px by default). Saved upstream controls confirm 160px.
    let text = "This long label needs several wrapped lines with words";
    for html_labels in [false, true] {
        for flowchart_width in [None, Some(160)] {
            let mut config = serde_json::json!({
                "layout": "dagre", "look": "classic", "htmlLabels": html_labels,
                "mindmap": {"maxNodeWidth": 80.5}
            });
            if let Some(width) = flowchart_width {
                config["flowchart"] = serde_json::json!({"wrappingWidth": width});
            }
            let source = format!(
                "---\nconfig: {config}\n---\nmindmap\n  {text}\n    rect[{text}]\n    rounded({text})\n    hexagon{{{{{text}}}}}\n"
            );
            let (svg, layout) = render(&source, "mindmap-12-3-wrapping");
            let expected_width = flowchart_width.unwrap_or(120) as f64;
            config["mindmap"]["maxNodeWidth"] = serde_json::json!(expected_width);
            let control = format!("---\nconfig: {config}\n---\nmindmap\n  {text}\n");
            let (control_svg, _) = render(&control, "mindmap-12-3-control");
            let document = roxmltree::Document::parse(&svg).unwrap();
            let control_document = roxmltree::Document::parse(&control_svg).unwrap();
            let control_node = node(&control_document, "mindmap-12-3-control-node_0");
            let control_label = control_node
                .children()
                .find(|element| element.attribute("class") == Some("label"))
                .unwrap();
            assert_eq!(layout.nodes.len(), 4);
            for index in 1..layout.nodes.len() {
                let group = node(&document, &format!("mindmap-12-3-wrapping-node_{index}"));
                let label = group
                    .children()
                    .find(|element| element.attribute("class") == Some("label"))
                    .unwrap();
                // Compare emitted bounds and label content in both HTML and SVG modes.
                assert_eq!(&svg[label.range()], &control_svg[control_label.range()]);
                if html_labels {
                    let foreign_object = group
                        .descendants()
                        .find(|element| element.has_tag_name("foreignObject"))
                        .unwrap();
                    assert_eq!(
                        foreign_object
                            .attribute("width")
                            .unwrap()
                            .parse::<f64>()
                            .unwrap(),
                        expected_width
                    );
                    let style = foreign_object
                        .children()
                        .find(roxmltree::Node::is_element)
                        .unwrap()
                        .attribute("style")
                        .unwrap();
                    assert!(style.contains(&format!("max-width: {expected_width}px;")));
                    assert!(style.contains(&format!("width: {expected_width}px;")));
                }
            }
            if html_labels {
                let root = node(&document, "mindmap-12-3-wrapping-node_0");
                let foreign_object = root
                    .descendants()
                    .find(|element| element.has_tag_name("foreignObject"))
                    .unwrap();
                assert_eq!(foreign_object.attribute("width"), Some("80.5"));
            }
        }
    }
}

#[test]
fn mindmap_mermaid_12_3_default_node_retains_fractional_wrapping_width() {
    // The saved controls retain fractional visible width in both runtimes; MM-006 must
    // not turn the earlier projection defect into a universal visible-width claim.
    let text = "This long label needs several wrapped lines with words";
    for html_labels in [false, true] {
        for width in [80.0, 80.5] {
            let source = format!(
                "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: {html_labels}\n  mindmap:\n    maxNodeWidth: {width}\n---\nmindmap\n  {text}\n"
            );
            let (svg, layout) = render(&source, "mindmap-12-3-default-width");
            // Rectangles reset the node width and obtain the same wrap limit from Flowchart.
            let control = format!(
                "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: {html_labels}\n  flowchart:\n    wrappingWidth: {width}\n---\nmindmap\n  root[{text}]\n"
            );
            let (control_svg, control_layout) = render(&control, "mindmap-12-3-width-control");
            let document = roxmltree::Document::parse(&svg).unwrap();
            let control_document = roxmltree::Document::parse(&control_svg).unwrap();
            let root = node(&document, "mindmap-12-3-default-width-node_0");
            let label = root
                .children()
                .find(|element| element.attribute("class") == Some("label"))
                .unwrap();
            let control_node = node(&control_document, "mindmap-12-3-width-control-node_0");
            let control_label = control_node
                .children()
                .find(|element| element.attribute("class") == Some("label"))
                .unwrap();
            assert_eq!(&svg[label.range()], &control_svg[control_label.range()]);
            // Classic default and rect nodes have 40px horizontal padding, but 10/20px vertical.
            assert!((layout.nodes[0].width - control_layout.nodes[0].width).abs() < 1e-10);
            assert!((layout.nodes[0].height + 10.0 - control_layout.nodes[0].height).abs() < 1e-10);
            if html_labels {
                let foreign_object = label
                    .descendants()
                    .find(|element| element.has_tag_name("foreignObject"))
                    .unwrap();
                assert_eq!(
                    foreign_object
                        .attribute("width")
                        .unwrap()
                        .parse::<f64>()
                        .unwrap(),
                    width
                );
            }
        }
    }
}
