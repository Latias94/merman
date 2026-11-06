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

struct EdgeLabelMeasurer;

impl TextMeasurer for EdgeLabelMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: text.chars().count() as f64 * 8.0,
            height: style.font_size
                + if style.font_style.as_deref() == Some("italic") {
                    8.0
                } else {
                    0.0
                },
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

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure(text, style).width
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        let half = self.measure(text, style).width / 2.0;
        (half + 3.0, half - 1.0)
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, style: &TextStyle) -> f64 {
        if style.font_style.as_deref() == Some("italic") {
            -3.0
        } else {
            2.0
        }
    }
}

fn render_class_with_edge_label_measurer(source: &str, layout: &str) -> String {
    render_class_with_edge_label_measurer_and_html(source, layout, false)
}

fn render_class_with_edge_label_measurer_and_html(
    source: &str,
    layout: &str,
    html: bool,
) -> String {
    render_and_layout_class_with_edge_label_measurer(source, layout, html).0
}

fn render_and_layout_class_with_edge_label_measurer(
    source: &str,
    layout: &str,
    html: bool,
) -> (String, merman_render::model::ClassDiagramLayout) {
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("class-edge-label-geometry").unwrap(),
            "test",
        )
        .unwrap(),
        Arc::new(EdgeLabelMeasurer),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "layout": layout,
                "htmlLabels": html,
                "look": "classic",
                "themeVariables": { "fontSize": "16px" }
            })),
        )
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = serde_json::from_value(projection["layout"]["ClassDiagramV2"].clone()).unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (svg, layout)
}

#[test]
fn class_html_cardinalities_preserve_breaks_and_entity_depth_in_measured_content() {
    let source = "classDiagram\nA \"1<br/>many\" --> \"&amp;lt;owner&amp;gt;\\n**literal**\" B\n";
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(json!({
                    "layout": layout,
                    "htmlLabels": true
                })),
            )
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("class-cardinality-visible-text").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(merman_render::text::DeterministicTextMeasurer {
                char_width_factor: 1.0,
                line_height_factor: 0.0,
            }),
        );
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::headless_svg_defaults(),
            RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
                .begin_session()
                .unwrap(),
        )
        .unwrap();
        let projection = artifact.layout_json().unwrap();
        let projection: merman_render::model::ClassDiagramLayout =
            serde_json::from_value(projection["layout"]["ClassDiagramV2"].clone()).unwrap();
        let edge = &projection.edges[0];
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let terminals: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("edgeTerminals"))
            .collect();
        assert_eq!(terminals.len(), 2);
        for (((terminal, label), expected_text), expected_width) in terminals
            .into_iter()
            .zip([
                edge.start_label_right.as_ref().unwrap(),
                edge.end_label_left.as_ref().unwrap(),
            ])
            .zip(["1many", "&lt;owner&gt;**literal**"])
            .zip([44.0, 143.0])
        {
            let object = terminal
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            assert_eq!(
                object
                    .descendants()
                    .filter(|node| node.has_tag_name("br"))
                    .count(),
                1
            );
            let text: String = object
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect();
            assert_eq!(text, expected_text);
            assert!(
                !object
                    .descendants()
                    .any(|node| { ["owner", "strong", "em"].contains(&node.tag_name().name()) })
            );
            assert_eq!(
                object.attribute("height").unwrap().parse::<f64>().unwrap(),
                label.height
            );
            assert_eq!(label.height, 33.0);
            assert_eq!(label.width, expected_width);
            assert_eq!(
                object.attribute("width").unwrap().parse::<f64>().unwrap(),
                expected_width
            );
        }
    }
}

#[cfg(feature = "math")]
#[test]
fn class_svg_math_cardinalities_measure_the_emitted_literal_text() {
    let source = "classDiagram\nA \"$$x$$\" --> \"$$x^2$$\" B\n";
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        for html in [false, true] {
            let (svg, projection) =
                render_and_layout_class_with_edge_label_measurer(source, layout, html);
            let edge = &projection.edges[0];
            let labels = [
                edge.start_label_right.as_ref().unwrap(),
                edge.end_label_left.as_ref().unwrap(),
            ];
            let document = roxmltree::Document::parse(&svg).unwrap();
            let terminals: Vec<_> = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("edgeTerminals"))
                .collect();
            assert_eq!(terminals.len(), 2);
            for ((label, terminal), text) in
                labels.into_iter().zip(terminals).zip(["$$x$$", "$$x^2$$"])
            {
                if html {
                    assert!(terminal.descendants().any(|node| node.has_tag_name("svg")));
                    assert!(label.width < text.len() as f64 * 8.0);
                } else {
                    let emitted: String = terminal
                        .descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect();
                    assert_eq!(emitted, text);
                    assert_eq!(label.width, text.len() as f64 * 8.0);
                    assert_eq!(label.height, 11.0);
                    assert!(
                        !terminal
                            .descendants()
                            .any(|node| node.has_tag_name("foreignObject"))
                    );
                }
            }
        }
    }
}

#[cfg(feature = "math")]
#[test]
fn class_rejected_math_relationship_uses_html_measurement_even_when_svg_labels_are_selected() {
    let source = format!(
        "classDiagram\nA --> B : {} $${}$$\n",
        std::iter::repeat_n("word", 15)
            .collect::<Vec<_>>()
            .join(" "),
        r"\frac{1}{"
    );
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        let central_html = |html| {
            let svg = render_class_with_edge_label_measurer_and_html(&source, layout, html);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let label = document
                .descendants()
                .find(|node| {
                    node.attribute("class") == Some("label") && node.attribute("data-id").is_some()
                })
                .unwrap();
            let object = label
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            let div = object.first_element_child().unwrap();
            let style = div.attribute("style").unwrap().to_owned();
            assert!(
                style.contains("white-space: break-spaces"),
                "{layout}, htmlLabels={html}: {style}"
            );
            assert!(object.attribute("width").unwrap().parse::<f64>().unwrap() >= 200.0);
            assert!(label.descendants().all(|node| !node.has_tag_name("text")));
            (
                object.attribute("width").unwrap().to_owned(),
                object.attribute("height").unwrap().to_owned(),
                style,
            )
        };
        assert_eq!(central_html(false), central_html(true), "{layout}");
    }
}

#[test]
fn class_relationship_label_centers_text_bbox_and_pads_background_in_namespaces() {
    let source = "classDiagram\ndirection TB\nclass Resource {\n<<abstract>>\n+String id\n}\nnamespace Publishing {\nclass Document {\n+String title\n+addSection(title: String) void\n+renderSummary() String\n}\nclass Section {\n+String heading\n+wordCount() int\n}\n}\nResource <|-- Document\nDocument \"1\" *-- \"1..*\" Section : contains\n";
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        for source in [
            source.to_owned(),
            source.replace("Resource <|-- Document\n", ""),
        ] {
            for (title, transform, x, y, width, height) in [
                ("contains", "translate(2, -10)", "-37", "0", "70", 20.0),
                ("*contains*", "translate(2, -9)", "-37", "-5", "70", 28.0),
                (
                    "contains *sections*",
                    "translate(2, -9)",
                    "-73",
                    "-5",
                    "142",
                    28.0,
                ),
                (
                    "contains<br/>contains",
                    "translate(2, -18.8)",
                    "-37",
                    "0",
                    "70",
                    37.6,
                ),
                (
                    "contains<br/>contains *sections*",
                    "translate(2, -20.3)",
                    "-73",
                    "0",
                    "142",
                    40.6,
                ),
            ] {
                let source = source.replace(": contains\n", &format!(": {title}\n"));
                let svg = render_class_with_edge_label_measurer(&source, layout);
                let document = roxmltree::Document::parse(&svg).unwrap();
                let text = document
                    .descendants()
                    .find(|node| node.has_tag_name("tspan") && node.text() == Some("contains"))
                    .unwrap();
                let label = text
                    .ancestors()
                    .find(|node| node.attribute("class") == Some("label"))
                    .unwrap();
                assert_eq!(
                    label.attribute("transform"),
                    Some(transform),
                    "{layout}: {title}"
                );
                let background = label
                    .descendants()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                assert_eq!(background.attribute("x"), Some(x), "{layout}: {title}");
                assert_eq!(background.attribute("y"), Some(y), "{layout}: {title}");
                assert_eq!(
                    background.attribute("width"),
                    Some(width),
                    "{layout}: {title}"
                );
                assert!(
                    (background
                        .attribute("height")
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                        - height)
                        .abs()
                        < 1e-8,
                    "{layout}: {title}"
                );
                for cardinality in ["1", "1..*"] {
                    assert!(
                        document.descendants().any(|node| {
                            node.has_tag_name("tspan")
                                && node.text() == Some(cardinality)
                                && node.ancestors().any(|ancestor| {
                                    ancestor.attribute("class") == Some("edgeTerminals")
                                })
                        }),
                        "{layout}: missing {cardinality}"
                    );
                }
            }
        }
    }
}

#[test]
fn class_relationship_svg_labels_wrap_prepared_words_without_losing_styles_or_graphemes() {
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        let long_word = "👩🏽‍💻".repeat(18);
        for title in [
            "contains **published** sections with a long relationship label<br/>and *draft* sections".to_owned(),
            long_word.clone(),
        ] {
            let svg = render_class_with_edge_label_measurer(
                &format!("classDiagram\nA --> B : {title}\n"),
                layout,
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let label = document
                .descendants()
                .find(|node| node.attribute("class") == Some("label") && node.attribute("data-id").is_some())
                .unwrap();
            let rows = label.descendants().filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|classes| classes.contains("text-outer-tspan"))
            }).collect::<Vec<_>>();
            assert!(rows.len() > 1, "{layout}: the default 200px width must wrap {title}");
            let words = rows.iter().map(|row| {
                row.children().filter_map(|node| node.text()).collect::<String>()
            }).collect::<Vec<_>>();
            for row in &words {
                assert!(row.chars().count() as f64 * 8.0 <= 200.0, "{layout}: {row}");
            }
            if title == long_word {
                assert_eq!(words.concat(), long_word, "{layout}");
                for row in &words {
                    assert!(row.replace("👩🏽‍💻", "").is_empty(), "{layout}: split grapheme {row}");
                }
            } else {
                assert_eq!(words.join(" "), "contains published sections with a long relationship label and draft sections", "{layout}");
                assert!(label.descendants().any(|node| node.text() == Some(" published") && node.attribute("font-weight") == Some("bold")));
                assert!(label.descendants().any(|node| node.text() == Some(" draft") && node.attribute("font-style") == Some("italic")));
            }
        }
    }
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
