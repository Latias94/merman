#![cfg(any(
    feature = "diagram-class",
    feature = "diagram-flowchart",
    feature = "diagram-mindmap"
))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(source: &str, environment: RenderEnvironment) -> (serde_json::Value, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let layout = artifact.layout_json().unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
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

#[cfg(feature = "diagram-class")]
#[test]
fn class_note_html_terminal_is_xml_and_preserves_entity_depth_and_breaks() {
    for (label, expected, breaks, images) in [
        ("A&nbsp;B", "A\u{a0}B", 0, 0),
        ("A<img src=x onerror=alert(1)>B", "AB", 0, 1),
        ("A\nB", "AB", 1, 0),
        ("A<br/>B", "AB", 1, 0),
        ("A&lt;b&gt;B&lt;/b&gt;", "A<b>B</b>", 0, 0),
        ("A&amp;lt;b&amp;gt;B", "A&lt;b&gt;B", 0, 0),
    ] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: true\n---\nclassDiagram\nnote \"{label}\"\n"
        );
        let (_, svg) = render(&source, RenderEnvironment::deterministic());
        let document = roxmltree::Document::parse(&svg).expect("final Class SVG must be XML");
        let terminal = document
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(text(terminal), expected, "{label}");
        assert_eq!(
            terminal
                .descendants()
                .filter(|node| node.has_tag_name("br"))
                .count(),
            breaks
        );
        assert_eq!(
            terminal
                .descendants()
                .filter(|node| node.has_tag_name("img"))
                .count(),
            images
        );
        assert!(!terminal.descendants().any(|node| node.has_tag_name("b")));
        assert!(
            !terminal
                .descendants()
                .any(|node| node.attribute("onerror").is_some())
        );
    }
}

#[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
#[test]
fn mindmap_html_terminal_restores_entities_before_serialization() {
    for (label, expected) in [
        ("A#60;B", "A<B"),
        ("A#lt;B", "A<B"),
        ("A#60;b#62;B#60;/b#62;C", "A<b>B</b>C"),
        ("A#quot;B", "A\"B"),
        ("A#nbsp;B", "A\u{a0}B"),
    ] {
        let source = format!("mindmap\n  root[\"{label}\"]\n");
        let (_, svg) = render(&source, RenderEnvironment::deterministic());
        let document = roxmltree::Document::parse(&svg).expect("final Mindmap SVG must be XML");
        let terminal = document
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(text(terminal), expected, "{label}");
        assert!(!terminal.descendants().any(|node| node.has_tag_name("b")));
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_plain_svg_terminal_literal_break_has_one_row_height() {
    let heights = ["A&lt;br&gt;B", "AXXXXB", "A<br/>B"].map(|label| {
        let source =
            format!("---\nconfig:\n  htmlLabels: false\n---\nflowchart LR\n  A[\"{label}\"]\n");
        let (layout, svg) = render(&source, RenderEnvironment::deterministic());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let rows = document
            .descendants()
            .filter(|node| {
                node.attribute("class").is_some_and(|class| {
                    class
                        .split_whitespace()
                        .any(|part| part == "text-outer-tspan")
                })
            })
            .map(text)
            .collect::<Vec<_>>();
        if label == "A&lt;br&gt;B" {
            assert_eq!(rows, ["A<br>B"]);
        }
        layout["layout"]["FlowchartV2"]["nodes"][0]["height"]
            .as_f64()
            .unwrap()
    });
    assert_eq!(heights[0], heights[1]);
    assert!(heights[2] > heights[1]);
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_svg_terminal_literal_break_has_one_row_and_one_row_height() {
    check_literal_rows("FlowchartV2", "flowchart LR\n  A");
}

#[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
#[test]
fn mindmap_svg_terminal_literal_break_has_one_row_and_one_row_height() {
    check_literal_rows("MindmapDiagram", "mindmap\n  root");
}

#[cfg(any(
    feature = "diagram-flowchart",
    all(feature = "diagram-mindmap", feature = "layout-cytoscape")
))]
fn check_literal_rows(family: &str, body: &str) {
    let row_step = 16.0 * 1.1;
    let mut heights = Vec::new();
    for (label, expected_rows) in [
        ("A&lt;br&gt;B", vec!["A<br>B"]),
        ("AXXXXB", vec!["AXXXXB"]),
        ("A<br/>B", vec!["A", "B"]),
    ] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: false\n  themeVariables:\n    fontSize: 16px\n---\n{body}[\"`{label}`\"]\n"
        );
        let (layout, svg) = render(&source, RenderEnvironment::deterministic());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let rows = document
            .descendants()
            .filter(|node| {
                node.attribute("class").is_some_and(|class| {
                    class
                        .split_whitespace()
                        .any(|part| part == "text-outer-tspan")
                })
            })
            .map(text)
            .collect::<Vec<_>>();
        assert_eq!(rows, expected_rows, "{family}: {label}");
        let node = &layout["layout"][family]["nodes"][0];
        heights.push(node["height"].as_f64().unwrap());
    }
    assert_eq!(heights[0], heights[1], "a literal BR cannot add a line");
    assert!((heights[2] - heights[1] - row_step).abs() < 1e-9);
    // The deterministic single-tspan bbox is font-size * 1.1875, rounded to pixels.
    // Family padding remains independent of the shared terminal measurement.
    let padding = if family == "FlowchartV2" { 30.0 } else { 20.0 };
    assert!((heights[1] - padding - 19.0).abs() < 1e-9);
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_svg_note_terminal_unions_styled_ink_at_retained_row_baselines() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{
        DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
    };

    struct TerminalMeasurer;

    impl TextMeasurer for TerminalMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            DeterministicTextMeasurer::default().measure(text, style)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            width: Option<f64>,
            mode: WrapMode,
        ) -> TextMetrics {
            DeterministicTextMeasurer::default().measure_wrapped(text, style, width, mode)
        }

        fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
            assert!(
                !text.contains('\n'),
                "terminal probe owns one retained word"
            );
            if style.font_style.as_deref() == Some("italic") {
                16.0
            } else {
                10.0
            }
        }

        fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, style: &TextStyle) -> f64 {
            if style.font_style.as_deref() == Some("italic") {
                -2.0
            } else {
                -4.0
            }
        }
    }

    for (label, rows, expected_ink) in [
        ("A", 1, 10.0),
        ("A *ii*", 1, 18.0),
        ("A<br/>*ii*", 2, 16.0 * 1.1 + 18.0),
        ("A<br/>`x`<br/>*ii*", 3, 2.0 * 16.0 * 1.1 + 18.0),
    ] {
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.terminal-note-baselines").unwrap(),
                "1",
            )
            .unwrap(),
            std::sync::Arc::new(TerminalMeasurer),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: false\n  themeVariables:\n    fontSize: 16px\n---\nclassDiagram\nnote \"{label}\"\n"
        );
        let (layout, svg) = render(&source, environment);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("class").is_some_and(|class| class
                    .split_whitespace()
                    .any(|part| part == "text-outer-tspan")))
                .count(),
            rows
        );
        let height = layout["layout"]["ClassDiagramV2"]["nodes"][0]["height"]
            .as_f64()
            .unwrap();
        assert!(
            (height - (expected_ink + 12.0)).abs() < 1e-9,
            "{label}: {height}"
        );
    }
}
