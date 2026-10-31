use merman_core::{MermaidEngine, ParseOptions};
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
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size / 2.0 * italic_scale,
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
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(json!({
                    "layout": layout,
                    "htmlLabels": false,
                    "fontSize": 16,
                    "themeVariables": { "fontSize": "16px" }
                })),
            )
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass Vehicle {\n+drive()*\n+stop()\n}",
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::default(),
            environment.begin_session().unwrap(),
        )
        .unwrap();
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
                .all(|(_, style)| { style.font_style.as_deref() == Some("normal") }),
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
        }
    }
}

#[test]
fn class_svg_note_measures_visible_markdown_instead_of_source_delimiters() {
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
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"htmlLabels": false}),
            ),
        )
        .parse_diagram_for_render_model_sync(
            "classDiagram\nnote \"**MMMM**\"\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let requests = measurer.requests.lock().unwrap();
    assert!(
        !requests.iter().any(|(text, _)| text.contains("**")),
        "{requests:?}"
    );
    assert!(
        requests
            .iter()
            .any(|(text, style)| { text == "MMMM" && style.font_weight.as_deref() == Some("700") }),
        "{requests:?}"
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(document.descendants().any(|node| {
        node.attribute("font-weight") == Some("bold") && node.text() == Some("MMMM")
    }));
}

#[test]
fn class_svg_note_preserves_terminal_entity_depth_and_visible_baseline() {
    for (source, visible) in [
        ("ASCII `omitted`", "ASCII"),
        ("**A &amp;amp; B**", "A &amp; B"),
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
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    json!({"htmlLabels": false}),
                ),
            )
            .parse_diagram_for_render_model_sync(
                &format!("classDiagram\nnote \"{source}\"\n"),
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::default(),
            environment.begin_session().unwrap(),
        )
        .unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
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
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let terminal = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
            .flat_map(|node| node.text())
            .collect::<String>();
        assert_eq!(terminal, visible);
    }
}

#[allow(dead_code)]
mod common;
