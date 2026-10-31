#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(body: &str, html_labels: bool) -> (serde_json::Value, String) {
    let source = format!(
        "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: {html_labels}\n  flowchart:\n    wrappingWidth: 1000\n  themeVariables:\n    fontSize: 16px\n---\nerDiagram\n{body}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ER text-stage fixture")
        .expect("detect ER text-stage fixture")
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare ER text-stage fixture");
    let layout = artifact.layout_json().unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render ER text-stage fixture")
        .svg()
        .to_owned();
    (serde_json::json!(layout), svg)
}

fn text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_whitespace().any(|part| part == class))
}

fn number(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
    node.attribute(attribute)
        .unwrap_or_else(|| panic!("missing terminal {attribute}"))
        .parse()
        .unwrap_or_else(|_| panic!("invalid terminal {attribute}"))
}

#[test]
fn er_generic_html_entity_name_measures_its_literal_terminal() {
    // Pinned erBox.ts:addText restores generic angle brackets through textContent, then
    // measures that plain terminal. The escaped source control reaches the same visible text.
    for (generic, escaped, expected) in [
        ("A~br~B", "A&lt;br&gt;B", "A<br>B"),
        ("*A~br~B*", "A&lt;br&gt;B", "A<br>B"),
        (
            "A~br~B&amp;lt;br&amp;gt;C",
            "A&lt;br&gt;B&lt;br&gt;C",
            "A<br>B<br>C",
        ),
        (
            "A~br~B&amp;amp;lt;br&amp;amp;gt;C",
            "A&lt;br&gt;B&amp;amp;lt;br&amp;amp;gt;C",
            "A<br>B&amp;lt;br&amp;gt;C",
        ),
    ] {
        let mut dimensions = Vec::new();
        for label in [generic, escaped] {
            let body = format!("  \"{label}\" {{\n    string id\n  }}");
            let (layout, svg) = render(&body, true);
            let document = roxmltree::Document::parse(&svg).expect("final ER SVG is XML");
            let terminal = document
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .expect("HTML entity name terminal");
            assert_eq!(text(terminal), expected, "{label}");
            assert!(
                !terminal.descendants().any(|node| node.has_tag_name("br")),
                "literal BR must remain text: {label}"
            );
            assert!(
                !terminal.descendants().any(|node| node.has_tag_name("em")),
                "the generic textContent workaround removes inline markup: {label}"
            );
            let node = &layout["layout"]["ErDiagram"]["nodes"][0];
            dimensions.push((
                number(terminal, "width"),
                number(terminal, "height"),
                node["width"].as_f64().unwrap(),
                node["height"].as_f64().unwrap(),
            ));
        }
        let (generic, escaped) = (dimensions[0], dimensions[1]);
        // The rich HTML planner rounds widths to 1/64px; literal host line probes retain precision.
        assert!(
            (generic.0 - escaped.0).abs() <= 1.0 / 64.0,
            "{generic:?} != {escaped:?}"
        );
        assert_eq!(generic.1, escaped.1, "{expected}");
        assert!(
            (generic.2 - escaped.2).abs() <= 1.0 / 64.0,
            "{generic:?} != {escaped:?}"
        );
        assert_eq!(generic.3, escaped.3, "{expected}");
    }
}

#[test]
fn er_attribute_free_entity_uses_draw_rect_generic_source_semantics() {
    use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};

    let style = TextStyle {
        font_size: 16.0,
        ..TextStyle::default()
    };
    let svg_ink_height =
        DeterministicTextMeasurer::default().measure_svg_tspan_text_bbox_height_px("A", &style);
    // drawRect/labelHelper bypasses erBox.addText and its generic textContent workaround.
    for html_labels in [false, true] {
        for (label, expected, emphasis) in [
            ("A~br~B", "A~br~B", false),
            ("*A~br~B*", "A~br~B", true),
            ("A~br~B&amp;lt;br&amp;gt;C", "A~br~B&lt;br&gt;C", false),
        ] {
            let (layout, svg) = render(&format!("  E[\"{label}\"]"), html_labels);
            let document = roxmltree::Document::parse(&svg).expect("final ER SVG is XML");
            let terminal = document
                .descendants()
                .find(|node| node.attribute("class") == Some("label"))
                .expect("attribute-free entity label");
            assert_eq!(text(terminal), expected, "{html_labels}/{label}");
            assert!(!terminal.descendants().any(|node| node.has_tag_name("br")));
            if html_labels {
                assert_eq!(
                    terminal.descendants().any(|node| node.has_tag_name("em")),
                    emphasis,
                    "{label}"
                );
                let foreign_object = terminal
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .unwrap();
                assert_eq!(number(foreign_object, "height"), 24.0);
            } else {
                let rows = terminal
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("tspan")
                            && node
                                .parent()
                                .is_some_and(|parent| parent.has_tag_name("text"))
                    })
                    .count();
                assert_eq!(rows, 1, "{label}");
            }
            let label_height = if html_labels {
                16.0 * 1.5
            } else {
                svg_ink_height
            };
            let expected_height = label_height + 2.0 * 20.0 * 1.5;
            assert_eq!(
                layout["layout"]["ErDiagram"]["nodes"][0]["height"]
                    .as_f64()
                    .unwrap(),
                expected_height,
                "{html_labels}/{label}"
            );
        }
    }
}

#[test]
fn er_html_entity_name_preserves_source_depth_and_real_breaks() {
    let mut heights = Vec::new();
    for (label, expected, breaks) in [
        ("A&lt;br&gt;B", "A<br>B", 0),
        ("A&amp;lt;br&amp;gt;B", "A&lt;br&gt;B", 0),
        ("A&amp;amp;lt;br&amp;amp;gt;B", "A&amp;lt;br&amp;gt;B", 0),
        ("A#lt;br#gt;B", "A<br>B", 0),
        ("AXXXXB", "AXXXXB", 0),
        ("A<br>B", "AB", 1),
        ("A<br/>B", "AB", 1),
    ] {
        let body = format!("  \"{label}\" {{\n    string id\n  }}");
        let (layout, svg) = render(&body, true);
        let document = roxmltree::Document::parse(&svg).expect("final ER SVG is XML");
        let name = document
            .descendants()
            .find(|node| has_class(*node, "label") && has_class(*node, "name"))
            .unwrap();
        assert_eq!(text(name), expected, "{label}");
        assert_eq!(
            name.descendants()
                .filter(|node| node.has_tag_name("br"))
                .count(),
            breaks,
            "{label}"
        );
        heights.push(
            layout["layout"]["ErDiagram"]["nodes"][0]["height"]
                .as_f64()
                .unwrap(),
        );
    }
    assert!(
        heights[..5].iter().all(|height| *height == heights[0]),
        "entity depth and Mermaid spelling must preserve one-line geometry: {heights:?}"
    );
    assert_eq!(heights[5], heights[6], "authored BR spellings must agree");
    assert!((heights[5] - heights[0] - 16.0 * 1.5).abs() < 1e-9);
}

#[test]
fn er_svg_relationship_measures_emitted_rows_and_background() {
    use merman_render::text::{
        DeterministicTextMeasurer, TextMeasurer, TextStyle, round_to_1_64_px,
    };

    let style = TextStyle {
        font_size: 14.0,
        ..TextStyle::default()
    };
    let measurer = DeterministicTextMeasurer::default();
    let one_row_ink = measurer.measure_svg_tspan_text_bbox_height_px("A", &style);
    let mut heights = Vec::new();
    for (label, expected_rows) in [
        ("A&lt;br&gt;B", vec!["A<br>B"]),
        ("A&amp;lt;br&amp;gt;B", vec!["A&lt;br&gt;B"]),
        ("AXXXXB", vec!["AXXXXB"]),
        ("A<br/>B", vec!["A", "B"]),
    ] {
        let (layout, svg) = render(&format!("  A ||--|| B : \"{label}\""), false);
        let document = roxmltree::Document::parse(&svg).expect("final ER SVG is XML");
        let edge = document
            .descendants()
            .find(|node| has_class(*node, "edgeLabel"))
            .unwrap();
        assert!(
            !edge
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        let rows = edge
            .descendants()
            .filter(|node| has_class(*node, "text-outer-tspan"))
            .map(text)
            .collect::<Vec<_>>();
        assert_eq!(rows, expected_rows, "{label}");
        let expected_width = round_to_1_64_px(
            rows.iter()
                .map(|row| measurer.measure_svg_text_computed_length_px(row, &style))
                .fold(0.0_f64, f64::max),
        );
        let expected_height = one_row_ink + (rows.len() - 1) as f64 * 14.0 * 1.1;
        let geometry = &layout["layout"]["ErDiagram"]["edges"][0]["label"];
        assert!(
            (geometry["width"].as_f64().unwrap() - expected_width).abs() < 1e-9,
            "{label}"
        );
        assert!(
            (geometry["height"].as_f64().unwrap() - expected_height).abs() < 1e-9,
            "{label}"
        );
        let background = edge
            .descendants()
            .find(|node| node.has_tag_name("rect") && has_class(*node, "background"))
            .expect("relationship label background");
        assert!(
            (number(background, "width") - expected_width).abs() < 1e-9,
            "{label}"
        );
        assert!(
            (number(background, "height") - expected_height).abs() < 1e-9,
            "{label}"
        );
        heights.push(geometry["height"].as_f64().unwrap());
    }
    assert_eq!(
        heights[0], heights[1],
        "entity depth cannot create an SVG row"
    );
    assert_eq!(
        heights[0], heights[2],
        "literal BR must have one-row geometry"
    );
    assert!((heights[3] - heights[0] - 14.0 * 1.1).abs() < 1e-9);
}
