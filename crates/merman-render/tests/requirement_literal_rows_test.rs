#![cfg(feature = "diagram-requirement")]

use merman_core::{Engine as MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use std::sync::Arc;

fn render(label: &str, environment: RenderEnvironment) -> (f64, String) {
    let source = format!(
        "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: false\n  themeVariables:\n    fontSize: 16px\n---\nrequirementDiagram\nrequirement req {{\ntext: \"{label}\"\nrisk: low\nverifymethod: test\n}}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let layout = artifact.layout_json().unwrap();
    let height = layout["layout"]["RequirementDiagram"]["nodes"][0]["height"]
        .as_f64()
        .unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (height, svg)
}

#[test]
fn requirement_svg_terminal_literal_break_is_not_reparsed_as_a_row() {
    let mut heights = Vec::new();
    for (label, expected) in [
        ("A&lt;br&gt;B", vec!["Text: A<br>B"]),
        ("AXXXXB", vec!["Text: AXXXXB"]),
        ("A<br/>B", vec!["Text: A", "B"]),
    ] {
        let (height, svg) = render(label, RenderEnvironment::deterministic());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let rows = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| {
                node.children()
                    .filter(|row| row.has_tag_name("tspan"))
                    .map(|row| {
                        row.descendants()
                            .filter(|node| node.is_text())
                            .filter_map(|node| node.text())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
            })
            .find(|rows| rows.first().is_some_and(|row| row.starts_with("Text:")))
            .expect("Requirement body rows");
        assert_eq!(rows, expected, "{label}");
        heights.push(height);
    }
    assert_eq!(heights[0], heights[1], "literal BR has one terminal row");
    assert!((heights[2] - heights[1] - 16.0 * 1.1).abs() < 1e-9);
}

#[test]
fn requirement_svg_first_row_height_uses_tspan_bbox_instead_of_generic_measure() {
    struct TerminalMeasurer {
        fallback_height: f64,
    }

    impl TextMeasurer for TerminalMeasurer {
        fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: text.len() as f64 * 5.0,
                height: self.fallback_height,
                line_count: 1,
            }
        }

        fn measure_wrapped(
            &self,
            _text: &str,
            _style: &TextStyle,
            _width: Option<f64>,
            _mode: WrapMode,
        ) -> TextMetrics {
            panic!("Requirement SVG must measure retained words without generic wrapping")
        }

        fn measure_mermaid_calculate_text_dimensions(
            &self,
            text: &str,
            _style: &TextStyle,
        ) -> TextMetrics {
            TextMetrics {
                width: text.len() as f64 * 5.0,
                height: 10.0,
                line_count: 1,
            }
        }

        fn measure_svg_tspan_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            19.0
        }

        fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            -3.0
        }
    }

    let mut heights = Vec::new();
    for fallback_height in [17.0, 91.0] {
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.requirement-terminal-bbox").unwrap(),
                "1",
            )
            .unwrap(),
            Arc::new(TerminalMeasurer { fallback_height }),
        );
        let (height, _) = render(
            "ok",
            RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
        );
        heights.push(height);
    }
    assert_eq!(
        heights[0], heights[1],
        "the first-row face adapter must forward the dedicated tspan bbox height"
    );
}
