#![cfg(feature = "diagram-xychart")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::{
    LayoutOptions,
    svg::{SvgDebugOptions, SvgRenderOptions},
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct LiteralProbe(Mutex<Vec<(&'static str, String)>>);

impl TextMeasurer for LiteralProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0.lock().unwrap().push(("label", text.to_owned()));
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size,
            height: style.font_size,
            line_count: 1,
        }
    }
    fn measure_svg_text_bounding_client_rect_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        assert_eq!(style.font_family.as_deref(), Some("ThemeChart,serif"));
        self.0
            .lock()
            .unwrap()
            .push(("client-width", text.to_owned()));
        text.chars().count() as f64 * style.font_size
    }
    fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        assert_eq!(style.font_family.as_deref(), Some("ThemeChart,serif"));
        self.0.lock().unwrap().push(("raw-height", text.to_owned()));
        if text.is_empty() {
            0.0
        } else {
            style.font_size
        }
    }
}

#[test]
fn xychart_direct_text_and_dimension_probes_keep_literal_break_markers() {
    let source = "xychart\ntitle \"AAAAAA<br/>BBBBBB\"\nx-axis [A, B]\ny-axis 0 --> 2\nbar [1, 2]";
    let parsed = MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({"fontFamily":"RootChart, serif","themeVariables":{"fontFamily":"ThemeChart, serif"}}))).parse_diagram_for_render_model_sync(source, ParseOptions::strict()).unwrap().unwrap().into_presentation();
    let probe = Arc::new(LiteralProbe::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("xychart-literal-text").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, probe.clone())),
    );
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let css = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(
        css.contains("font-family:ThemeChart,serif;"),
        "stylesheet must share the measurement font winner"
    );
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("chart-title"))
        .unwrap()
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    let visible = title.text().unwrap();
    assert!(visible.starts_with("AAAAAA<br"), "{visible:?}");
    assert!(visible.ends_with(">BBBBBB"), "{visible:?}");
    assert!(!title.children().any(|node| node.has_tag_name("tspan")));
    let calls = probe.0.lock().unwrap();
    for operation in ["client-width", "raw-height"] {
        assert!(
            calls
                .iter()
                .any(|(actual, text)| *actual == operation && text == visible),
            "{calls:?}"
        );
    }
    assert!(
        !calls
            .iter()
            .any(|(operation, text)| *operation == "label" && text == visible),
        "direct SVG text must not enter label normalization: {calls:?}"
    );
}

#[allow(dead_code)]
mod common;
