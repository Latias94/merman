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
    html_line_heights: Mutex<Vec<(String, TextStyle)>>,
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

    fn measure_html_line_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.0
            .html_line_heights
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
        self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
            .height
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
    render_class_source(
        layout,
        &format!(
            "classDiagram\nclass Vehicle[\"<i>Vehicle</i>\"] {{\n+owner\n+drive()*\n}}\nclass Driver\nVehicle --> Driver\n{node_style}\n"
        ),
    )
}

fn render_class_source(layout: &str, source: &str) -> (String, Arc<FontRequests>) {
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
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "layout": layout,
                "htmlLabels": true,
                "securityLevel": "loose",
                "fontFamily": "\"Budget; Face\", monospace;",
                "fontSize": 18,
                "themeVariables": {
                    "fontFamily": "\"Visible; Face\", serif;",
                    "fontSize": "24px"
                }
            })),
        )
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .unwrap()
        .unwrap()
        .into_presentation();
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
fn class_html_semantic_sizes_measure_the_displayed_face_and_keep_the_parent_strut() {
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        for (label, expected_size) in [
            ("AccountAccount", 24.0),
            ("<big><big><big>AccountAccount</big></big></big>", 41.472),
            ("<small>AccountAccount</small>", 20.0),
            ("<big><small>AccountAccount</small></big>", 24.0),
        ] {
            let (svg, requests) =
                render_class_source(layout, &format!("classDiagram\nclass E[\"{label}\"]\n"));
            let displayed = requests.displayed.lock().unwrap();
            assert!(
                displayed.iter().any(|(text, style)| {
                    text == "AccountAccount" && (style.font_size - expected_size).abs() < 1e-9
                }),
                "{layout}/{label}: {displayed:?}"
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let title = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("foreignObject")
                        && node
                            .descendants()
                            .any(|child| child.text() == Some("AccountAccount"))
                })
                .unwrap();
            let height = title.attribute("height").unwrap().parse::<f64>().unwrap();
            assert!(
                (height - expected_size.max(24.0) * 1.5).abs() < 1e-6,
                "{layout}/{label}: height={height}"
            );
            assert!(
                requests
                    .probes
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|style| style.font_size == 18.0)
            );
        }
    }
}

#[test]
fn class_html_sized_icons_keep_their_advance_in_node_measurement() {
    let width = |label: &str| {
        let (svg, requests) =
            render_class_source("dagre", &format!("classDiagram\nclass E[\"{label}\"]\n"));
        assert!(
            requests
                .displayed
                .lock()
                .unwrap()
                .iter()
                .all(|(text, _)| !text.contains('\u{fffc}'))
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .any(|child| child.text() == Some("AccountAccount"))
            })
            .unwrap()
            .attribute("width")
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let ordinary = width("<big>AccountAccount</big>");
    let with_icon = width("<big><i class='fa fa-car'></i>AccountAccount</big>");
    assert_eq!(with_icon - ordinary, 36.0);
}

#[test]
fn class_html_declared_font_height_requests_are_linear_in_the_visible_text() {
    for word_count in [64, 256] {
        let visible = vec!["x"; word_count].join(" ");
        // Keep the independent root-size budget wider than the final 16px title.
        let source = format!(
            "---\nconfig: {{fontSize: 64}}\n---\nclassDiagram\nclass E[\"<span style='font-size:16px'>{visible}</span>\"]\n"
        );
        let (svg, requests) = render_class_source("dagre", &source);
        let heights = requests.html_line_heights.lock().unwrap();
        let declared_font: Vec<_> = heights
            .iter()
            .filter(|(_, style)| style.font_size == 16.0)
            .collect();
        assert_eq!(
            declared_font.len(),
            1,
            "a single final font group needs one height request for {word_count} words"
        );
        assert_eq!(
            declared_font
                .iter()
                .map(|(text, _)| text.len())
                .sum::<usize>(),
            visible.len(),
            "height measurement must submit the visible group once, without repeated full-run payloads"
        );
        assert_eq!(declared_font[0].0, visible);

        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .any(|child| child.is_text() && child.text() == Some(visible.as_str()))
            })
            .expect("the Class title retains its complete visible text");
        assert_eq!(label.attribute("height"), Some("36"));
        assert!(label.descendants().all(|node| !node.has_tag_name("br")));
        assert!(label.descendants().any(|node| {
            node.has_tag_name("div")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("white-space: nowrap"))
        }));
        assert!(label.descendants().any(|node| {
            node.has_tag_name("span")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("font-size:16px"))
        }));
    }
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

#[allow(dead_code)]
mod common;
