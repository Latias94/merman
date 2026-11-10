#![cfg(feature = "diagram-git-graph")]

use std::sync::Arc;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions, debug_svg_emitted_bounds};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::{LayoutOptions, family};
use serde_json::json;

struct BaselineMeasurer;

impl TextMeasurer for BaselineMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size * 0.5,
            height: style.font_size,
            line_count: 1,
        }
    }

    fn measure_svg_raw_text_bbox_y_offset_px(&self, _: &str, style: &TextStyle) -> Option<f64> {
        Some(-style.font_size * 0.875)
    }

    fn measure_svg_raw_text_bbox_height_px(&self, _: &str, style: &TextStyle) -> f64 {
        style.font_size
    }
}

#[test]
fn large_commit_and_tag_ink_reaches_the_viewport_after_its_emitted_transforms() {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.gitgraph-label-baseline").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(BaselineMeasurer),
        )),
    );
    for direction in ["LR", "TB", "BT"] {
        for rotate in [false, true] {
            for class in ["commit-label", "tag-label"] {
                let tag = if class == "tag-label" {
                    " tag:\"BigTag\""
                } else {
                    ""
                };
                let source = format!("gitGraph {direction}:\ncommit id:\"BIG\"{tag}\n");
                let parsed = MermaidEngine::new()
                    .with_site_config(SourcePresentationConfig::from_value(json!({
                        "gitGraph": {
                            "rotateCommitLabel": rotate,
                            "showCommitLabel": class == "commit-label",
                            "useMaxWidth": false,
                            "diagramPadding": 8
                        },
                        "themeVariables": {
                            "fontFamily": "monospace",
                            "commitLabelFontSize": "96px",
                            "tagLabelFontSize": "96px"
                        }
                    })))
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let rendered = family::prepare(
                    &parsed,
                    &LayoutOptions::headless_svg_defaults(),
                    environment.begin_session().unwrap(),
                )
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let label = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .unwrap();
                let x = label.attribute("x").unwrap().parse::<f64>().unwrap();
                let y = label.attribute("y").unwrap().parse::<f64>().unwrap();
                let style = TextStyle {
                    font_size: 96.0,
                    ..Default::default()
                };
                let text = label.text().unwrap();
                let metrics = BaselineMeasurer.measure(text, &style);
                let top = BaselineMeasurer
                    .measure_svg_raw_text_bbox_y_offset_px(text, &style)
                    .unwrap();
                // Let the independent SVG primitive scanner apply the actual serialized
                // transforms to the supplied text bbox, including the horizontal wrapper.
                let ink_svg = format!(
                    r#"<svg><g transform="{}"><g transform="{}"><rect x="{x}" y="{}" width="{}" height="{}"/></g></g></svg>"#,
                    label.parent().unwrap().attribute("transform").unwrap_or(""),
                    label.attribute("transform").unwrap_or(""),
                    y + top,
                    metrics.width,
                    metrics.height,
                );
                let ink = debug_svg_emitted_bounds(&ink_svg).unwrap().bounds;
                let viewport = document
                    .root_element()
                    .attribute("viewBox")
                    .unwrap()
                    .split_ascii_whitespace()
                    .map(|value| value.parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                let epsilon = 1e-6;
                assert!(
                    viewport[0] + 8.0 <= ink.min_x + epsilon
                        && viewport[1] + 8.0 <= ink.min_y + epsilon
                        && viewport[0] + viewport[2] - 8.0 >= ink.max_x - epsilon
                        && viewport[1] + viewport[3] - 8.0 >= ink.max_y - epsilon,
                    "{direction}/{rotate}/{class}: viewport={viewport:?}, ink={ink:?}"
                );
            }
        }
    }
}
