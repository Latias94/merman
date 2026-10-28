use merman_core::presentation::PresentationConfig;
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
struct FontRequests {
    probes: Mutex<Vec<TextStyle>>,
    displayed: Mutex<Vec<(String, TextStyle)>>,
}

struct FontMeasurer(Arc<FontRequests>);

impl FontMeasurer {
    fn metrics(text: &str, style: &TextStyle) -> TextMetrics {
        let family_scale = if style.font_family.as_deref() == Some(r#""Visible; Face",serif"#) {
            2.0
        } else {
            1.0
        };
        let weight_scale = if style.font_weight.as_deref() == Some("700") {
            1.25
        } else {
            1.0
        };
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size / 8.0
                * family_scale
                * weight_scale,
            height: style.font_size * 1.5,
            line_count: 1,
        }
    }
}

impl TextMeasurer for FontMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0
            .displayed
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
        Self::metrics(text, style)
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
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        self.0.probes.lock().unwrap().push(style.clone());
        Self::metrics(text, style)
    }
}

fn render_class(layout: &str, node_style: &str) -> (String, Arc<FontRequests>) {
    let requests = Arc::new(FontRequests::default());
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("class-html-font-contract").unwrap(),
            "test",
        )
        .unwrap(),
        Arc::new(FontMeasurer(requests.clone())),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let parsed = MermaidEngine::new()
        .with_site_config(PresentationConfig::from_value(json!({
            "layout": layout,
            "htmlLabels": true,
            "securityLevel": "loose",
            "fontFamily": "\"Budget; Face\", monospace;",
            "fontSize": 18,
            "themeVariables": {
                "fontFamily": "\"Visible; Face\", serif;",
                "fontSize": "24px"
            }
        })))
        .parse_diagram_for_render_model_sync(
            &format!(
                "classDiagram\nclass Vehicle[\"<i>Vehicle</i>\"] {{\n+owner\n+drive()*\n}}\nclass Driver\nVehicle --> Driver\n{node_style}\n"
            ),
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (svg, requests)
}

#[test]
fn class_html_final_labels_use_displayed_css_fonts_while_budget_probe_stays_root_first() {
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        let (svg, requests) = render_class(layout, "");
        assert!(
            svg.contains(r#"font-family:"Visible; Face",serif;"#),
            "{svg}"
        );
        let probes = requests.probes.lock().unwrap();
        assert!(probes.iter().any(|style| {
            style.font_family.as_deref() == Some(r#""Budget; Face",monospace"#)
                && style.font_size == 18.0
        }));
        assert!(probes.iter().all(|style| {
            matches!(
                style.font_family.as_deref(),
                Some("sans-serif") | Some(r#""Budget; Face",monospace"#)
            ) && style.font_weight.is_none()
                && style.font_style.is_none()
        }));
        let displayed = requests.displayed.lock().unwrap();
        for (text, weight, font_style) in [
            ("Vehicle", Some("700"), Some("italic")),
            ("Driver", Some("700"), None),
            ("+owner", None, None),
            ("+drive()", None, Some("italic")),
        ] {
            assert!(
                displayed.iter().any(|(actual, style)| {
                    actual.trim() == text
                        && style.font_family.as_deref() == Some(r#""Visible; Face",serif"#)
                        && style.font_size == 24.0
                        && style.font_weight.as_deref() == weight
                        && style.font_style.as_deref() == font_style
                }),
                "{layout}: missing final font for {text}: {displayed:?}"
            );
        }
        let document = roxmltree::Document::parse(&svg).unwrap();
        let title = document
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .any(|child| child.text() == Some("Vehicle"))
            })
            .unwrap();
        // 7 glyphs at 24px, the visible family scale, and title weight: no probe font or padding.
        assert_eq!(title.attribute("width"), Some("52.5"));
        assert_eq!(title.attribute("height"), Some("36"));
        assert!(title.descendants().any(|node| node.has_tag_name("i")));
    }
}

#[test]
fn class_html_node_span_fonts_follow_group_inheritance_before_xhtml_measurement() {
    let (svg, requests) = render_class(
        "dagre",
        "style Vehicle font-family:monospace,font-size:150%,font-weight:lighter,font-style:italic",
    );
    let displayed = requests.displayed.lock().unwrap();
    for (text, weight) in [("Vehicle", "400"), ("+owner", "100"), ("+drive()", "100")] {
        assert!(
            displayed.iter().any(|(actual, style)| {
                actual.trim() == text
                    && style.font_family.as_deref() == Some("monospace")
                    && style.font_size == 36.0
                    && style.font_weight.as_deref() == Some(weight)
                    && style.font_style.as_deref() == Some("italic")
            }),
            "missing final span font for {text}: {displayed:?}"
        );
    }
    assert!(svg.contains("font-size:150%"));
    assert!(svg.contains("font-weight:lighter"));
}
