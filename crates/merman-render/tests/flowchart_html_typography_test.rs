#![cfg(feature = "diagram-flowchart")]

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
struct FontMeasurer(Mutex<Vec<f64>>);

impl TextMeasurer for FontMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        if text.trim() == "Payload" {
            self.0.lock().unwrap().push(style.font_size);
        }
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size / 2.0,
            height: style.font_size * 1.5,
            line_count: 1,
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
}

#[test]
fn flowchart_html_layout_and_svg_use_the_same_resolved_root_font_size() {
    for (value, expected) in [
        (json!(24), 24.0),
        (json!(24.5), 24.5),
        (json!("24"), 24.0),
        (json!("24.5px"), 24.0),
    ] {
        let measurer = Arc::new(FontMeasurer::default());
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("flowchart-html-font-contract").unwrap(),
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
                "htmlLabels": true,
                "themeVariables": {"fontSize": value}
            })))
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[Payload]",
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
            let requests = measurer.0.lock().unwrap();
            assert!(!requests.is_empty(), "{value}");
            assert!(
                requests.iter().all(|size| *size == expected),
                "{value}: {requests:?}"
            );
        }
        measurer.0.lock().unwrap().clear();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let requests = measurer.0.lock().unwrap();
        assert!(!requests.is_empty(), "{value}");
        assert!(
            requests.iter().all(|size| *size == expected),
            "{value}: {requests:?}"
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let root = document.root_element();
        let id = root.attribute("id").unwrap();
        let root_rule = format!("#{id}{{font-family:");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        let rule = css
            .split('}')
            .find(|rule| rule.starts_with(&root_rule))
            .unwrap();
        assert!(
            rule.contains(&format!(";font-size:{expected}px;")),
            "{value}: {rule}"
        );
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
    }
}
