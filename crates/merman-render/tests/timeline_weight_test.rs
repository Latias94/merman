mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::TimelineDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use std::sync::{Arc, Mutex};

const LABEL: &str = "Alpha Bravo Charlie Delta Echo";

#[derive(Debug)]
struct WeightCall {
    operation: &'static str,
    text: String,
    weight: Option<String>,
}

#[derive(Clone, Default)]
struct SectionWeightMeasurer(Arc<Mutex<Vec<WeightCall>>>);

impl SectionWeightMeasurer {
    fn record(&self, operation: &'static str, text: &str, style: &TextStyle) {
        self.0.lock().unwrap().push(WeightCall {
            operation,
            text: text.to_owned(),
            weight: style.font_weight.clone(),
        });
    }

    fn width(text: &str, style: &TextStyle) -> f64 {
        let advance = match style.font_weight.as_deref() {
            Some("600") => 10.0,
            Some("900") => 15.0,
            _ => 5.0,
        };
        if text == "Overflow" {
            advance * 60.0
        } else {
            text.chars().count() as f64 * advance
        }
    }
}

impl TextMeasurer for SectionWeightMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: Self::width(text, style),
            height: 10.0,
            line_count: 1,
        }
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("wrap", text, style);
        Self::width(text, style)
    }

    fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("bbox-width", text, style);
        Self::width(text, style)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("bbox-height", text, style);
        match style.font_weight.as_deref() {
            Some("600" | "900") => 40.0,
            _ => 10.0,
        }
    }
}

fn render_weighted_timeline(
    theme: &str,
    look: &str,
    direction: &str,
    weight: Option<&str>,
    with_section: bool,
) -> (TimelineDiagramLayout, String, Vec<WeightCall>) {
    let mut config = serde_json::json!({
        "theme": theme, "look": look, "fontSize": "16px",
        "themeVariables": {"fontSize": "16px"}
    });
    if let Some(weight) = weight {
        config["themeVariables"]["fontWeight"] = serde_json::json!(weight);
    }
    let section = if with_section {
        format!("section {LABEL}\n")
    } else {
        String::new()
    };
    let next_section = if with_section {
        "section Next\nSmall : Done\n"
    } else {
        ""
    };
    let source = format!(
        "timeline {direction}\n{section}{LABEL} : {LABEL} : Overflow\nTiny\n{next_section}"
    );
    render_source(config, &source)
}

fn render_source(
    config: serde_json::Value,
    source: &str,
) -> (TimelineDiagramLayout, String, Vec<WeightCall>) {
    let parsed = legacy_init_theme_compat_engine()
        .with_site_config(merman_core::MermaidConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Timeline")
        .expect("detect Timeline");
    let measurer = SectionWeightMeasurer::default();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.timeline-section-weight").unwrap(),
        "1",
    )
    .unwrap();
    let profile = TextMeasurementProfile::new(identity, Arc::new(measurer.clone()));
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Timeline");
    let projection = artifact.layout_json().expect("Timeline projection");
    let layout = serde_json::from_value(projection["layout"]["TimelineDiagram"].clone())
        .expect("Timeline layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Timeline SVG")
        .svg()
        .to_owned();
    let calls = measurer.0.lock().unwrap().drain(..).collect();
    (layout, svg, calls)
}

fn assert_normal_probes_and_final_weight(calls: &[WeightCall], weight: &str) {
    for text in [LABEL, "[object Object]", "Overflow"] {
        assert!(
            calls.iter().any(|call| {
                call.operation == "wrap" && call.text == text && call.weight.is_none()
            }),
            "missing normal virtual probe for {text}: {calls:?}"
        );
    }
    for operation in ["wrap", "bbox-height", "bbox-width"] {
        assert!(
            calls.iter().any(|call| {
                call.operation == operation && call.weight.as_deref() == Some(weight)
            }),
            "missing final section {operation} at weight {weight}: {calls:?}"
        );
    }
    let first_final_wrap = calls
        .iter()
        .position(|call| call.operation == "wrap" && call.weight.as_deref() == Some(weight))
        .expect("final wrap");
    let last_virtual_wrap = calls
        .iter()
        .rposition(|call| call.operation == "wrap" && call.weight.is_none())
        .expect("virtual wrap");
    assert!(last_virtual_wrap < first_final_wrap);
}

#[test]
fn timeline_redux_section_weight_drives_final_wrap_height_and_bbox() {
    for theme in ["redux", "redux-color", "redux-dark", "redux-dark-color"] {
        for look in ["classic", "neo"] {
            for direction in ["LR", "TD"] {
                for (weight, expected_weight) in [
                    (None, "600"),
                    (Some("900"), "900"),
                    (Some("900 !important"), "900"),
                ] {
                    let (layout, svg, calls) =
                        render_weighted_timeline(theme, look, direction, weight, true);
                    assert_normal_probes_and_final_weight(&calls, expected_weight);
                    let task = &layout.sections[0].tasks[0];
                    let expected: &[&str] = match (direction, expected_weight) {
                        ("LR", "600") => &["Alpha   Bravo", "Charlie   Delta", "Echo"],
                        ("LR", "900") => &["Alpha", "Bravo", "Charlie", "Delta   Echo"],
                        ("TD", "600") => &["Alpha   Bravo   Charlie", "Delta   Echo"],
                        ("TD", "900") => &["Alpha   Bravo", "Charlie   Delta", "Echo"],
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        task.node.label_lines, expected,
                        "{theme}/{look}/{direction}"
                    );
                    let expected_height = 40.0
                        + (expected.len() - 1) as f64 * 16.0 * 1.1
                        + 16.0 * 1.1 / 2.0
                        + task.node.padding;
                    assert!(
                        (task.node.height - expected_height).abs() < 1e-9,
                        "{theme}/{look}/{direction}: {} != {expected_height}",
                        task.node.height
                    );
                    assert_eq!(layout.sections[0].tasks[1].node.height, task.node.height);
                    assert!(layout.sections[1].tasks[0].node.height < task.node.height);
                    if direction == "LR" {
                        assert_eq!(task.connectors[0].y1, task.node.y + task.node.height);
                    }
                    assert!(layout.pre_title_box_width >= 600.0);
                    let bounds = layout.bounds.as_ref().expect("bounds");
                    assert!(bounds.max_x - bounds.min_x >= 700.0);
                    let css_weight = weight.unwrap_or("600");
                    assert!(svg.contains(&format!("font-weight:{css_weight};")));
                    assert!(!svg.contains(r#"font-weight="normal""#));
                    assert!(!svg.contains("font-weight:normal"));
                    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
                    let task_text = document
                        .descendants()
                        .find(|node| node.attribute("class") == Some("taskWrapper"))
                        .expect("task wrapper")
                        .descendants()
                        .find(|node| node.has_tag_name("text"))
                        .expect("task text");
                    let rows: Vec<_> = task_text
                        .children()
                        .filter(|node| node.has_tag_name("tspan"))
                        .map(|node| node.text().unwrap_or_default())
                        .collect();
                    assert_eq!(rows, expected);
                }
            }
        }
    }
}

#[test]
fn timeline_classic_and_neo_labels_ignore_redux_only_weight() {
    for look in ["classic", "neo"] {
        for direction in ["LR", "TD"] {
            let (layout, svg, calls) =
                render_weighted_timeline("default", look, direction, Some("900"), true);
            assert!(calls.iter().all(|call| call.weight.is_none()));
            assert_eq!(layout.sections[0].tasks[0].node.label_lines.len(), 1);
            assert!(!svg.contains("font-weight:900"));
        }
    }
}

#[test]
fn timeline_orphan_nodes_keep_their_redux_section_weight() {
    for direction in ["LR", "TD"] {
        let (layout, svg, calls) =
            render_weighted_timeline("redux", "classic", direction, Some("900"), false);
        assert_normal_probes_and_final_weight(&calls, "900");
        assert!(layout.sections.is_empty());
        assert!(layout.orphan_tasks[0].node.label_lines.len() > 1);
        assert_eq!(
            layout.orphan_tasks[1].node.height,
            layout.orphan_tasks[0].node.height
        );
        assert!(layout.pre_title_box_width >= 900.0);
        assert!(svg.contains("font-weight:900;"));
    }
}

#[test]
fn timeline_maintenance_weight_requires_the_emitted_section_rule() {
    for direction in ["LR", "TD"] {
        for limit in [1, 2] {
            for with_sections in [true, false] {
                let mut source = format!("timeline {direction}\n");
                for index in 0..=limit {
                    if with_sections {
                        source.push_str(&format!("section Group{index}\n"));
                    }
                    let label = if index == limit { LABEL } else { "Bold" };
                    source.push_str(&format!("{label} : Event{index}\n"));
                }
                let (layout, svg, calls) = render_source(
                    serde_json::json!({
                        "theme": "redux", "fontSize": "16px",
                        "themeVariables": {"fontSize": "16px", "fontWeight": 900, "THEME_COLOR_LIMIT": limit}
                    }),
                    &source,
                );
                let last = if with_sections {
                    &layout.sections[limit].tasks[0]
                } else {
                    &layout.orphan_tasks[limit]
                };
                assert_eq!(last.node.section_class, format!("section-{}", limit - 1));
                assert!(!svg.contains(&format!(".section-{} text{{", limit - 1)));
                assert_eq!(last.node.label_lines.len(), 1);
                assert!(
                    last.node.label_lines[0]
                        .split_whitespace()
                        .eq(LABEL.split_whitespace())
                );
                let is_label =
                    |call: &WeightCall| call.text.split_whitespace().eq(LABEL.split_whitespace());
                for operation in ["bbox-width", "bbox-height"] {
                    assert!(
                        calls.iter().any(|call| is_label(call)
                            && call.operation == operation
                            && call.weight.is_none()),
                        "{direction}/limit={limit}/sections={with_sections}: {calls:?}"
                    );
                    assert!(!calls.iter().any(|call| is_label(call)
                        && call.operation == operation
                        && call.weight.is_some()));
                    assert!(calls.iter().any(|call| call.text == "Bold"
                        && call.operation == operation
                        && call.weight.as_deref() == Some("900")));
                }
            }
        }
    }
}
