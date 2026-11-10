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
    render_class_source_with_font_size(layout, source, 24)
}

fn render_class_source_with_font_size(
    layout: &str,
    source: &str,
    font_size: u32,
) -> (String, Arc<FontRequests>) {
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
                    "fontSize": format!("{font_size}px")
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
        for (label, expected_size, line_size) in [
            ("AccountAccount", 24.0, 24.0),
            (
                "<big><big><big>AccountAccount</big></big></big>",
                41.472,
                41.472,
            ),
            ("<small>AccountAccount</small>", 20.0, 24.0),
            ("<big><small>AccountAccount</small></big>", 24.0, 24.0 * 1.2),
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
                (height - line_size * 1.5).abs() < 1e-6,
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

fn html_title_dimensions(svg: &str, text: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("foreignObject")
                && node.descendants().any(|child| child.text() == Some(text))
        })
        .expect("Class title foreignObject");
    (
        title.attribute("width").unwrap().parse().unwrap(),
        title.attribute("height").unwrap().parse().unwrap(),
    )
}

#[test]
fn class_html_keyword_and_absolute_sizes_measure_and_emit_like_numeric_controls() {
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        for (authored, pixels) in [
            ("xx-small", 9.0),
            ("x-small", 10.0),
            ("small", 13.0),
            ("medium", 16.0),
            ("large", 18.0),
            ("x-large", 24.0),
            ("xx-large", 32.0),
            ("smaller", 20.0),
            ("larger", 24.0 * 1.2),
            ("24pt", 32.0),
            ("2PC", 32.0),
            (r"24p\74", 32.0),
        ] {
            let source = |size: &str| {
                format!(
                    "classDiagram\nclass E[\"<span style='font-size:{size}'>AccountAccount</span>\"]\n"
                )
            };
            let (actual, requests) = render_class_source(layout, &source(authored));
            let (numeric, _) = render_class_source(layout, &source(&format!("{pixels}px")));
            let actual_dimensions = html_title_dimensions(&actual, "AccountAccount");
            let numeric_dimensions = html_title_dimensions(&numeric, "AccountAccount");
            assert_eq!(actual_dimensions, numeric_dimensions, "{layout}/{authored}");
            assert!(
                requests
                    .displayed
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(text, style)| {
                        text == "AccountAccount" && (style.font_size - pixels).abs() < 1e-9
                    }),
                "{layout}/{authored}"
            );

            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            for svg in [&actual, &numeric] {
                let safe = merman_render::svg::finalize_resvg_svg(svg, &session).unwrap();
                let document = roxmltree::Document::parse(safe.as_str()).unwrap();
                let size = document
                    .descendants()
                    .find(|node| {
                        (node.has_tag_name("text") || node.has_tag_name("tspan"))
                            && node.text() == Some("AccountAccount")
                    })
                    .and_then(|node| node.attribute("style"))
                    .and_then(|style| {
                        style
                            .split(';')
                            .find_map(|declaration| declaration.trim().strip_prefix("font-size:"))
                    })
                    .and_then(|value| {
                        value
                            .trim()
                            .trim_end_matches("!important")
                            .trim()
                            .strip_suffix("px")
                    })
                    .and_then(|value| value.parse::<f64>().ok())
                    .expect("resolved native font size");
                assert!((size - pixels).abs() < 1e-9, "{layout}/{authored}: {size}");
            }
        }
    }
}

#[test]
fn class_html_zero_sizes_preserve_parent_struts_visible_siblings_and_host_requests() {
    let source = |label: &str| format!("classDiagram\nclass E[\"{label}\"]\n");
    let (visible, _) = render_class_source("dagre", &source("Visible"));
    let expected_dimensions = html_title_dimensions(&visible, "Visible");
    for zero in [
        "<span style='font-size:0px'>Hidden</span>",
        "<span style='font-size:0pt!important;font-size:32px'>Hidden</span>",
        "<span style='font-size:0px'><big><span style='font-size:150%'>Hidden</span></big></span>",
    ] {
        let (svg, requests) = render_class_source("dagre", &source(&format!("{zero}Visible")));
        assert_eq!(
            html_title_dimensions(&svg, "Visible"),
            expected_dimensions,
            "{zero}"
        );
        let displayed = requests.displayed.lock().unwrap();
        assert!(
            displayed
                .iter()
                .any(|(text, style)| text == "Hidden" && style.font_size == 0.0),
            "{zero}: {displayed:?}"
        );
        assert!(
            displayed
                .iter()
                .any(|(text, style)| text == "Visible" && style.font_size == 24.0),
            "{zero}: {displayed:?}"
        );
        assert!(
            displayed
                .iter()
                .any(|(text, style)| text == "HiddenVisible" && style.font_size == 24.0),
            "the opaque host retains its complete paragraph request: {displayed:?}"
        );
        drop(displayed);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let safe = merman_render::svg::finalize_resvg_svg(&svg, &session).unwrap();
        let document = roxmltree::Document::parse(safe.as_str()).unwrap();
        let hidden = document
            .descendants()
            .find(|node| node.has_tag_name("tspan") && node.text() == Some("Hidden"))
            .unwrap();
        assert!(
            hidden
                .attribute("style")
                .unwrap()
                .replace(' ', "")
                .contains("font-size:0px!important")
        );
    }
    let (zero_only, requests) = render_class_source(
        "dagre",
        &source("<span style='font-size:0px'>Hidden</span>"),
    );
    let (width, height) = html_title_dimensions(&zero_only, "Hidden");
    assert!(
        width <= 1.0,
        "a native viewport may keep its minimum extent: {width}"
    );
    assert_eq!(
        height, 36.0,
        "the 24px parent still supplies its HTML strut"
    );
    assert!(
        requests
            .html_line_heights
            .lock()
            .unwrap()
            .iter()
            .any(|(text, style)| text == "Hidden" && style.font_size == 0.0)
    );
}

#[test]
fn class_html_nested_zero_text_keeps_ancestor_line_boxes_and_terminal_requests() {
    let source = |label: &str| format!("classDiagram\nclass E[\"{label}\"]\n");
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        let (control, _) = render_class_source(layout, &source("Visible"));
        let (visible_width, visible_height) = html_title_dimensions(&control, "Visible");
        for tail in ["", "Visible"] {
            let (svg, requests) = render_class_source(
                layout,
                &source(&format!(
                    "<span style='font-size:64px'><span style='font-size:0px'>Hidden</span></span>{tail}"
                )),
            );
            let (width, height) = html_title_dimensions(&svg, "Hidden");
            assert_eq!(height, 96.0, "{layout}/{tail}");
            if !tail.is_empty() {
                assert_eq!(width, visible_width, "{layout}");
                assert!(height > visible_height);
            }
            let requests = requests.displayed.lock().unwrap();
            let hidden_request = requests
                .iter()
                .position(|(text, style)| text == "Hidden" && style.font_size == 0.0)
                .expect("zero-sized terminal text request");
            let paragraph_request = requests
                .iter()
                .position(|(text, style)| {
                    text == &format!("Hidden{tail}") && style.font_size == 24.0
                })
                .expect("complete opaque paragraph request");
            assert!(
                paragraph_request < hidden_request,
                "the opaque paragraph request precedes terminal probes: {layout}: {requests:?}"
            );
        }
    }
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

#[test]
fn class_html_notes_resolve_rem_against_the_document_root_before_sizing() {
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        for font_size in [14, 24] {
            let render = |size: &str| {
                render_class_source_with_font_size(
                    layout,
                    &format!(
                        "classDiagram\nnote \"<span style='font-size:{size}'>AccountAccount</span>\"\n"
                    ),
                    font_size,
                )
            };
            let (relative, requests) = render("2rem");
            let (pixels, _) = render("32px");
            assert_eq!(
                html_title_dimensions(&relative, "AccountAccount"),
                html_title_dimensions(&pixels, "AccountAccount"),
                "{layout}, diagram font size {font_size}px"
            );
            let displayed = requests.displayed.lock().unwrap();
            assert!(
                displayed
                    .iter()
                    .any(|(text, style)| { text == "AccountAccount" && style.font_size == 32.0 }),
                "{layout}/{font_size}: {displayed:?}"
            );
        }
    }
}

#[allow(dead_code)]
mod common;
