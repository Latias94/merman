#![cfg(feature = "diagram-treemap")]
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::{
    LayoutOptions, family,
    svg::{SvgDebugOptions, SvgRenderOptions},
};
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct TitleProbe(Mutex<Vec<(&'static str, String)>>);
impl TitleProbe {
    fn record(&self, operation: &'static str, text: &str) {
        if text.contains("<br") {
            self.0.lock().unwrap().push((operation, text.to_owned()));
        }
    }
}
impl TextMeasurer for TitleProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.record("label", text);
        TextMetrics {
            width: 20.0,
            height: style.font_size,
            line_count: 1,
        }
    }
    fn measure_svg_simple_text_bbox_width_px(&self, text: &str, _: &TextStyle) -> f64 {
        self.record("simple-width", text);
        20.0
    }
    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, _: &TextStyle) -> f64 {
        self.record("simple-height", text);
        10.0
    }
    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, _: &TextStyle) -> f64 {
        self.record("raw-width", text);
        if text.contains("<br") { 4000.0 } else { 20.0 }
    }
    fn measure_svg_raw_text_bbox_height_px(&self, text: &str, _: &TextStyle) -> f64 {
        self.record("raw-height", text);
        24.0
    }
}
#[test]
fn treemap_literal_title_uses_raw_bbox_and_expands_the_root() {
    let source = "treemap-beta\ntitle WWWWWWWW<br/>WWWWWWWW\n\"Leaf\": 1";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let probe = Arc::new(TitleProbe::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("treemap-literal-title").unwrap(),
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
    let root = document.root_element();
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text") && node.text().is_some_and(|text| text.contains("<br"))
        })
        .unwrap();
    assert!(!title.children().any(|node| node.has_tag_name("tspan")));
    let visible = title.text().unwrap();
    let calls = probe.0.lock().unwrap();
    for operation in ["raw-width", "raw-height"] {
        assert!(
            calls
                .iter()
                .any(|(op, text)| *op == operation && text == visible),
            "{calls:?}"
        );
    }
    assert!(
        !calls.iter().any(|(op, _)| op.starts_with("simple-")),
        "{calls:?}"
    );
    let width = root
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!(
        width >= 4000.0,
        "raw title bbox must enter root bounds: {width}"
    );
}
