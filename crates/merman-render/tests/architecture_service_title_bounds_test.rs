#![cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]

use std::sync::Arc;

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::TextMetrics;

struct ServiceTitleFaceHost;

impl HostTextMeasurer for ServiceTitleFaceHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let characters = request.text.chars().count() as f64;
        // Wrapping advances fit within 120px; styled ink exceeds the 80px icon.
        Ok(Some(match request.operation {
            TextMeasurementOperation::Measure | TextMeasurementOperation::Wrapped => {
                HostTextMeasurement::Metrics(TextMetrics {
                    width: characters * 10.0,
                    height: 16.0,
                    line_count: 1,
                })
            }
            TextMeasurementOperation::ComputedLength | TextMeasurementOperation::TspanBBoxWidth => {
                HostTextMeasurement::Length(characters * 10.0)
            }
            TextMeasurementOperation::BBoxX => {
                let (left_per_character, right_per_character) =
                    if request.style.font_weight.as_deref() == Some("bold") {
                        (25.0, 15.0)
                    } else if request.style.font_style.as_deref() == Some("italic") {
                        (15.0, 35.0)
                    } else {
                        (5.0, 5.0)
                    };
                HostTextMeasurement::HorizontalExtents {
                    left: characters * left_per_character,
                    right: characters * right_per_character,
                }
            }
            TextMeasurementOperation::TspanBBoxHeight => HostTextMeasurement::Length(
                if request.style.font_weight.as_deref() == Some("bold") {
                    48.0
                } else if request.style.font_style.as_deref() == Some("italic") {
                    60.0
                } else {
                    16.0
                },
            ),
            TextMeasurementOperation::CreateTextMiddleBBoxYOffset => HostTextMeasurement::Length(
                if request.style.font_weight.as_deref() == Some("bold") {
                    -12.0
                } else if request.style.font_style.as_deref() == Some("italic") {
                    -20.0
                } else {
                    -8.0
                },
            ),
            _ => return Ok(None),
        }))
    }
}

fn render_service_title(title: &str, responsive: bool) -> String {
    let source = format!(
        "%%{{init: {{\"architecture\": {{\"padding\": 0, \"iconSize\": 80, \"useMaxWidth\": {responsive}, \"numIter\": 1, \"randomize\": false}}}}}}%%\narchitecture-beta\nservice api[{title}]\n",
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Architecture service title")
        .expect("detect Architecture")
        .into_presentation();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.architecture-service-title-faces").unwrap(),
        "1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            Arc::new(ServiceTitleFaceHost),
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .expect("begin face-sensitive Architecture session");
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Architecture service title")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("architecture-service-title-bounds".into()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Architecture service title")
        .svg()
        .to_owned()
}

fn assert_service_title_viewport(
    title: &str,
    expected_rows: &[&[(&str, &str, &str)]],
    expected_bounds: [f64; 4],
    responsive: bool,
) {
    let svg = render_service_title(title, responsive);
    let document = roxmltree::Document::parse(&svg).expect("parse emitted SVG");
    let root = document.root_element();
    let service = document
        .descendants()
        .find(|node| node.attribute("class") == Some("architecture-service"))
        .expect("emitted service");
    let text = service
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .expect("emitted service title");
    let rows = text
        .children()
        .filter(|node| node.has_tag_name("tspan"))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        expected_rows.len(),
        "{title}: final ink bounds must not change wrapping"
    );
    for (row, expected_words) in rows.iter().zip(expected_rows) {
        let words = row
            .children()
            .filter(|node| node.has_tag_name("tspan"))
            .collect::<Vec<_>>();
        assert_eq!(words.len(), expected_words.len(), "{title}");
        for (word, &(content, weight, style)) in words.iter().zip(*expected_words) {
            assert_eq!(word.text(), Some(content), "{title}");
            assert_eq!(word.attribute("font-weight"), Some(weight), "{title}");
            assert_eq!(word.attribute("font-style"), Some(style), "{title}");
        }
    }

    // Pinned svgDraw centers the label at iconSize / 2 before the final root getBBox.
    let transform = service.attribute("transform").expect("service translation");
    let translation = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .expect("service translate transform");
    let service_xy = translation
        .split(',')
        .map(|value| value.trim().parse::<f64>().expect("service coordinate"))
        .collect::<Vec<_>>();
    assert_eq!(service_xy.len(), 2);
    let view_box = root
        .attribute("viewBox")
        .expect("root viewBox")
        .split_whitespace()
        .map(|value| value.parse::<f64>().expect("viewBox coordinate"))
        .collect::<Vec<_>>();
    assert_eq!(view_box.len(), 4);
    for (index, coordinate) in ["left", "top", "width", "height"].iter().enumerate() {
        let actual = if index < 2 {
            view_box[index] - service_xy[index]
        } else {
            view_box[index]
        };
        assert!(
            (actual - expected_bounds[index]).abs() < 1e-3,
            "{title}: zero-padding root {coordinate} must include the emitted faces; expected {}, got {actual}; viewBox={view_box:?}",
            expected_bounds[index],
        );
    }
    assert_eq!(root.attribute("height"), None);
    if responsive {
        assert_eq!(root.attribute("width"), Some("100%"));
        let max_width = root
            .attribute("style")
            .and_then(|style| style.strip_prefix("max-width: "))
            .and_then(|value| value.strip_suffix("px;"))
            .expect("responsive max-width")
            .parse::<f64>()
            .expect("numeric max-width");
        assert!((max_width - expected_bounds[2]).abs() < 1e-6, "{title}");
    } else {
        // Architecture's intrinsic sizing exports viewBox without width/height attributes.
        assert_eq!(root.attribute("width"), None);
    }
}

#[test]
fn architecture_standalone_service_normal_title_keeps_icon_width() {
    assert_service_title_viewport(
        "WWWW",
        &[&[("WWWW", "normal", "normal")]],
        [0.0, 0.0, 80.0, 88.0],
        false,
    );
}

#[test]
fn architecture_standalone_service_strong_title_expands_root_bounds() {
    assert_service_title_viewport(
        "**WWWW**",
        &[&[("WWWW", "bold", "normal")]],
        [-60.0, 0.0, 160.0, 116.0],
        false,
    );
}

#[test]
fn architecture_standalone_service_emphasis_title_expands_root_bounds() {
    assert_service_title_viewport(
        "*WWWW*",
        &[&[("WWWW", "normal", "italic")]],
        [-20.0, 0.0, 200.0, 120.0],
        false,
    );
}

#[test]
fn architecture_standalone_service_mixed_faces_position_ink_by_advances() {
    // The three advances are 20/30/30px. Their positioned ink union is [-80, 130]
    // around the row anchor, rather than the 290px sum of the separate ink widths.
    assert_service_title_viewport(
        "WW **WW** *WW*",
        &[&[
            ("WW", "normal", "normal"),
            (" WW", "bold", "normal"),
            (" WW", "normal", "italic"),
        ]],
        [-40.0, 0.0, 210.0, 120.0],
        false,
    );
    assert_service_title_viewport(
        "*WW* **WW** WW",
        &[&[
            ("WW", "normal", "italic"),
            (" WW", "bold", "normal"),
            (" WW", "normal", "normal"),
        ]],
        [-40.0, 0.0, 120.0, 120.0],
        false,
    );
}

#[test]
fn architecture_standalone_service_later_tall_row_expands_vertical_bounds() {
    assert_service_title_viewport(
        "WW<br/>**WW**",
        &[&[("WW", "normal", "normal")], &[("WW", "bold", "normal")]],
        [-10.0, 0.0, 90.0, 133.6],
        false,
    );
}

#[test]
fn architecture_standalone_service_short_tail_does_not_repeat_tall_first_row_height() {
    assert_service_title_viewport(
        "**WW**<br/>WW",
        &[&[("WW", "bold", "normal")], &[("WW", "normal", "normal")]],
        [-10.0, 0.0, 90.0, 116.0],
        false,
    );
}

#[test]
fn architecture_standalone_service_responsive_width_uses_final_face_bounds() {
    assert_service_title_viewport(
        "**WWWW**",
        &[&[("WWWW", "bold", "normal")]],
        [-60.0, 0.0, 160.0, 116.0],
        true,
    );
}
