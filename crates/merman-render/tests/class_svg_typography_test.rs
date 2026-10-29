#![cfg(feature = "diagram-class")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct SvgFontMeasurer {
    requests: Mutex<Vec<(String, TextStyle)>>,
    bbox_y_requests: Mutex<Vec<(String, TextStyle)>>,
}

impl TextMeasurer for SvgFontMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.requests
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
        let line_count = text.lines().count().max(1);
        let italic_scale = if style.font_style.as_deref() == Some("italic") {
            2.0
        } else {
            1.0
        };
        let weight_scale = if matches!(style.font_weight.as_deref(), Some("700" | "bold")) {
            1.5
        } else {
            1.0
        };
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size / 2.0
                * italic_scale
                * weight_scale,
            height: style.font_size * italic_scale * line_count as f64,
            line_count,
        }
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        _max_width: Option<f64>,
        _wrap: WrapMode,
    ) -> TextMetrics {
        self.measure(text, style)
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        _text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        TextMetrics {
            width: 1024.0,
            height: style.font_size,
            line_count: 1,
        }
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.bbox_y_requests
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
        self.measure(text, style);
        0.0
    }
}

#[test]
fn class_svg_abstract_method_measurement_matches_the_terminal_tspan_reset() {
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        let measurer = Arc::new(SvgFontMeasurer::default());
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("class-svg-font-contract").unwrap(),
                "test",
            )
            .unwrap(),
            measurer.clone(),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let parsed = Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "layout": layout,
                "htmlLabels": false,
                "fontSize": 16,
                "themeVariables": { "fontSize": "16px" }
            })))
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass Vehicle {\n+drive()*\n+stop()\n}",
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap();
        let artifact = family::prepare(
            parsed,
            &LayoutOptions::default(),
            environment.begin_session().unwrap(),
        )
        .unwrap();
        {
            let requests = measurer.requests.lock().unwrap();
            assert!(
                requests.iter().any(|(text, style)| {
                    text == "+drive()" && style.font_style.as_deref() == Some("normal")
                }),
                "{layout}: {requests:?}"
            );
            assert!(
                requests
                    .iter()
                    .filter(|(text, _)| text == "+drive()")
                    .all(|(_, style)| {
                        style.font_style.as_deref() == Some("normal")
                            && style
                                .font_weight
                                .as_deref()
                                .is_none_or(|value| value == "normal")
                    }),
                "{layout}: {requests:?}"
            );
        }
        measurer.requests.lock().unwrap().clear();

        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let requests = measurer.requests.lock().unwrap();
        assert!(
            requests.iter().any(|(text, style)| {
                text == "+drive()" && style.font_style.as_deref() == Some("normal")
            }),
            "{layout}: {requests:?}"
        );
        assert!(
            requests
                .iter()
                .filter(|(text, _)| text == "+drive()")
                .all(|(_, style)| {
                    style.font_style.as_deref() == Some("normal")
                        && style
                            .font_weight
                            .as_deref()
                            .is_none_or(|value| value == "normal")
                }),
            "{layout}: {requests:?}"
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let methods = document
            .descendants()
            .find(|node| node.attribute("class") == Some("methods-group text"))
            .unwrap();
        let labels: Vec<_> = methods
            .children()
            .filter(|node| node.is_element())
            .collect();
        assert_eq!(labels.len(), 2, "{layout}: {}", rendered.svg());
        assert_eq!(labels[0].attribute("transform"), Some("translate(0,-8)"));
        assert_eq!(labels[1].attribute("transform"), Some("translate(0,11)"));
        for (label, font_style) in labels.iter().zip(["normal", "normal"]) {
            let word = label
                .descendants()
                .find(|node| node.attribute("class") == Some("text-inner-tspan"))
                .unwrap();
            assert_eq!(word.attribute("font-style"), Some(font_style));
            assert_eq!(word.attribute("font-weight"), Some("normal"));
        }
    }
}

#[test]
fn class_svg_note_measures_visible_markdown_in_layout_and_emission() {
    let measurer = Arc::new(SvgFontMeasurer::default());
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("class-svg-note-contract").unwrap(),
            "test",
        )
        .unwrap(),
        measurer.clone(),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(json!({
            "layout": "dagre",
            "htmlLabels": false,
            "themeVariables": { "fontSize": "16px" }
        })))
        .parse_diagram_for_render_model_sync(
            "classDiagram\nnote \"**MMMM**\"\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    {
        let requests = measurer.requests.lock().unwrap();
        assert!(
            requests.iter().any(|(text, style)| {
                text == "MMMM" && style.font_weight.as_deref() == Some("700")
            }),
            "layout must measure the visible bold note: {requests:?}"
        );
        assert!(
            !requests.iter().any(|(text, _)| text.contains("**")),
            "{requests:?}"
        );
    }
    measurer.requests.lock().unwrap().clear();

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let label = document
        .descendants()
        .find(|node| node.attribute("class") == Some("label noteLabel"))
        .unwrap();
    let word = label
        .descendants()
        .find(|node| node.attribute("class") == Some("text-inner-tspan"))
        .unwrap();
    assert_eq!(word.text(), Some("MMMM"));
    assert_eq!(word.attribute("font-weight"), Some("bold"));
    assert_eq!(word.attribute("font-style"), Some("normal"));
    assert_eq!(label.attribute("transform"), Some("translate(-24, -8)"));
    let requests = measurer.requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|(text, style)| { text == "MMMM" && style.font_weight.as_deref() == Some("700") }),
        "emission must measure the visible bold note: {requests:?}"
    );
    assert!(
        !requests.iter().any(|(text, _)| text.contains("**")),
        "{requests:?}"
    );
}

#[test]
fn class_svg_note_preserves_entity_depth_and_measures_visible_baseline_words() {
    for (source, visible, transform, expected_words, weight) in [
        (
            "ASCII `omitted`",
            "ASCII",
            "translate(-20, -8)",
            &["ASCII"][..],
            "normal",
        ),
        (
            "**A &amp;amp; B**",
            "A &amp; B",
            "translate(-54, -8)",
            &["A", " &amp;", " B"][..],
            "bold",
        ),
    ] {
        let measurer = Arc::new(SvgFontMeasurer::default());
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("class-svg-note-projection").unwrap(),
                "test",
            )
            .unwrap(),
            measurer.clone(),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let parsed = Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "layout": "dagre",
                "htmlLabels": false,
                "themeVariables": { "fontSize": "16px" }
            })))
            .parse_diagram_for_render_model_sync(
                &format!("classDiagram\nnote \"{source}\"\n"),
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap();
        let artifact = family::prepare(
            parsed,
            &LayoutOptions::default(),
            environment.begin_session().unwrap(),
        )
        .unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = document
            .descendants()
            .find(|node| node.attribute("class") == Some("label noteLabel"))
            .unwrap();
        let words = label
            .descendants()
            .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
            .collect::<Vec<_>>();
        let terminal_words = words
            .iter()
            .filter_map(|node| node.text())
            .collect::<Vec<_>>();
        assert_eq!(terminal_words, expected_words, "{source}");
        for word in &words {
            assert_eq!(word.attribute("font-style"), Some("normal"), "{source}");
            assert_eq!(word.attribute("font-weight"), Some(weight), "{source}");
        }
        let terminal = terminal_words.concat();
        assert_eq!(terminal, visible, "{source}");
        assert_eq!(label.attribute("transform"), Some(transform), "{source}");
        let requests = measurer.requests.lock().unwrap();
        assert!(
            requests.iter().any(|(text, _)| text == visible),
            "{requests:?}"
        );
        assert!(
            !requests.iter().any(|(text, _)| {
                text.contains("omitted") || text.contains("**") || text == "A & B"
            }),
            "{requests:?}"
        );
        let bbox_y_requests = measurer.bbox_y_requests.lock().unwrap();
        let mut measured_words = bbox_y_requests
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<Vec<_>>();
        measured_words.sort_unstable();
        measured_words.dedup();
        // Baseline probes use source words; retained tspan probes own their leading spaces.
        let mut visible_words = visible
            .split(' ')
            .chain(expected_words.iter().copied())
            .collect::<Vec<_>>();
        visible_words.sort_unstable();
        visible_words.dedup();
        assert_eq!(measured_words, visible_words, "{source}");
        // Matching probes inherit the base face; bold probes materialize the reset.
        // The emitted tspan attributes above must remain explicit in both cases.
        let (probe_font_style, probe_font_weight) = if weight == "bold" {
            (Some("normal"), Some("700"))
        } else {
            (None, None)
        };
        for (_, style) in bbox_y_requests.iter() {
            assert_eq!(style.font_style.as_deref(), probe_font_style, "{source}");
            assert_eq!(style.font_weight.as_deref(), probe_font_weight, "{source}");
        }
    }
}
