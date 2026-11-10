#![cfg(feature = "diagram-requirement")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::RequirementDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
};
use serde_json::json;
use std::sync::Arc;

const SOURCE: &str = concat!(
    "requirementDiagram\nrequirement WWWWWWWWWWWWWWWWWWWW {\nid: DOC-12345\ntext: \"",
    "Every report retains its reviewed source and archived reference. ",
    "Every report retains its reviewed source and archived reference. ",
    "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW\"\nrisk: low\nverifymethod: test\n}\n",
);

fn render(
    html: bool,
    size: u32,
    environment: RenderEnvironment,
) -> (RequirementDiagramLayout, String) {
    let diagram = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "htmlLabels": html,
                "layout": "dagre",
                "look": "classic",
                "fontFamily": "Arial",
                "fontSize": 10,
                "themeVariables": {"fontFamily": "Arial", "fontSize": format!("{size}px")}
            })),
        )
        .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &diagram,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let layout = serde_json::from_value(
        artifact.layout_json().unwrap()["layout"]["RequirementDiagram"].clone(),
    )
    .unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    (layout, rendered.svg().to_owned())
}

fn view_box(document: &roxmltree::Document<'_>) -> [f64; 4] {
    document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn translation(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let (x, y) = node
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split_once(',')
        .unwrap();
    (x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}

struct SignedOriginMeasurer {
    bbox_y: f64,
}

impl TextMeasurer for SignedOriginMeasurer {
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

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        DeterministicTextMeasurer::default().measure_mermaid_calculate_text_dimensions(text, style)
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        DeterministicTextMeasurer::default().measure_svg_text_computed_length_px(text, style)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        DeterministicTextMeasurer::default().measure_svg_tspan_text_bbox_height_px(text, style)
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        self.bbox_y
    }
}

#[test]
fn requirement_svg_viewport_contains_multiline_stereotype_at_the_measured_bbox_origin() {
    for size in [32, 48] {
        for bbox_y in [-7.25, 5.0] {
            let profile = TextMeasurementProfile::new(
                TextMeasurementProfileIdentity::new(
                    MeasurementProfileId::new("test.requirement-signed-origin").unwrap(),
                    "1",
                )
                .unwrap(),
                Arc::new(SignedOriginMeasurer { bbox_y }),
            );
            let (layout, svg) = render(
                false,
                size,
                RenderEnvironment::deterministic()
                    .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let viewport = view_box(&document);
            let node = &layout.nodes[0];
            let mut top = node.y;
            let mut bottom = node.y + node.height;
            let texts = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            let stereotype = texts[0];
            assert!(stereotype.children().filter(|row| row.is_element()).count() > 1);
            for text in texts {
                let group = text.parent().unwrap().parent().unwrap();
                let (_, label_y) = translation(group);
                let first_top = node.y + node.height / 2.0 + label_y + bbox_y;
                for (index, row) in text.children().filter(|row| row.is_element()).enumerate() {
                    let content = row
                        .descendants()
                        .filter(|child| child.is_text())
                        .filter_map(|child| child.text())
                        .collect::<String>();
                    let row_top = first_top + index as f64 * f64::from(size) * 1.1;
                    let height = DeterministicTextMeasurer::default()
                        .measure_svg_tspan_text_bbox_height_px(
                            &content,
                            &TextStyle {
                                font_family: Some("Arial".to_owned()),
                                font_size: f64::from(size),
                                ..TextStyle::default()
                            },
                        );
                    top = top.min(row_top);
                    bottom = bottom.max(row_top + height);
                }
            }
            assert!(top < node.y - 8.0, "exercise text outside the old viewport");
            close(viewport[1], top - 8.0);
            close(viewport[1] + viewport[3], bottom + 8.0);
            close(viewport[0], node.x - 8.0);
            close(viewport[2], node.width + 16.0);
        }
    }
}

#[test]
fn requirement_html_viewport_contains_foreign_objects_outside_the_node() {
    let (layout, svg) = render(true, 64, RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    let viewport = view_box(&document);
    let node = &layout.nodes[0];
    let mut top = node.y;
    let mut bottom = node.y + node.height;
    for foreign_object in document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
    {
        let (label_x, label_y) = translation(foreign_object.parent().unwrap());
        let x = node.x + node.width / 2.0 + label_x;
        let y = node.y + node.height / 2.0 + label_y;
        let width = foreign_object
            .attribute("width")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        let height = foreign_object
            .attribute("height")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!(viewport[0] <= x && viewport[0] + viewport[2] >= x + width);
        top = top.min(y);
        bottom = bottom.max(y + height);
    }
    assert!(top < node.y - 8.0, "exercise overflowing HTML stereotype");
    close(viewport[1], top - 8.0);
    close(viewport[1] + viewport[3], bottom + 8.0);
}

#[test]
fn requirement_small_labels_keep_the_node_based_viewport() {
    for html in [false, true] {
        let (layout, svg) = render(html, 10, RenderEnvironment::deterministic());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let viewport = view_box(&document);
        let node = &layout.nodes[0];
        close(viewport[0], node.x - 8.0);
        close(viewport[1], node.y - 8.0);
        close(viewport[2], node.width + 16.0);
        close(viewport[3], node.height + 16.0);
    }
}
