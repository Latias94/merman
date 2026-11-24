use merman_core::{MermaidEngine, ParseOptions};

use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPolicy,
    TextMeasurementProfile, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use std::sync::Arc;
use std::sync::Mutex;

// These fixtures exercise the classic terminal contract; Mermaid 12 defaults are tested separately.
fn classic_engine() -> MermaidEngine {
    MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "look": "classic",
            "layout": "dagre"
        })),
    )
}

fn render_requirement_without_theme(source: &str) -> family::RenderedFamilySvg {
    render_requirement_without_theme_and_engine(source, classic_engine())
}

fn render_requirement_without_theme_and_engine(
    source: &str,
    engine: MermaidEngine,
) -> family::RenderedFamilySvg {
    render_requirement_with_config(
        source,
        engine.with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"theme": "default"}),
            ),
        ),
        RenderEnvironment::deterministic(),
    )
}

fn render_requirement_with_config(
    source: &str,
    engine: MermaidEngine,
    environment: RenderEnvironment,
) -> family::RenderedFamilySvg {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Requirement")
        .expect("detect Requirement")
        .into_presentation();
    let session = environment
        .begin_session()
        .expect("begin Requirement session");

    family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Requirement")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("requirement-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Requirement")
}

#[test]
fn requirement_multiline_accessibility_reaches_svg_and_keeps_suffix_nodes() {
    for newline in ["\n", "\r\n"] {
        for (title, description) in [("\u{85}Accessible;#\u{85}", "Help"), ("\u{85}", "\u{85}")] {
            let source = format!(
                "requirementDiagram{newline}accTitle{newline}%% Removed before parsing{newline}\u{feff}:{newline}{title}{newline}accDescr{newline}{{{newline}{description}{newline}}}requirement Feature {{{newline} id: 1{newline} text: \"Body\"{newline}}}{newline}"
            );
            let rendered = render_requirement_without_theme(&source);
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let root = document.root_element();
            for (attribute, tag, expected) in [
                ("aria-labelledby", "title", title),
                ("aria-describedby", "desc", description),
            ] {
                let id = root.attribute(attribute).unwrap();
                let target = root
                    .children()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap();
                assert!(target.has_tag_name(tag));
                assert_eq!(target.text(), Some(expected));
            }
            for expected in ["Feature", "Body"] {
                assert!(document.descendants().any(|node| {
                    node.is_text() && node.text().is_some_and(|text| text.contains(expected))
                }));
            }
        }
    }
}

#[test]
fn requirement_root_width_uses_the_state_config_owner() {
    for (config, responsive) in [
        (serde_json::json!({}), true),
        (serde_json::json!({"state": {"useMaxWidth": false}}), false),
        (
            serde_json::json!({"requirement": {"useMaxWidth": false}}),
            true,
        ),
        (
            serde_json::json!({"state": {"useMaxWidth": false}, "requirement": {"useMaxWidth": true}}),
            false,
        ),
    ] {
        let rendered = render_requirement_without_theme_and_engine(
            requirement_source(),
            classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let root = document.root_element();
        assert_eq!(root.attribute("width") == Some("100%"), responsive);
        assert_eq!(
            root.attribute("style")
                .is_some_and(|style| style.contains("max-width")),
            responsive
        );
    }
}

fn terminal_fill_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("stroke") == Some("none")
                && node.attribute("stroke-width") == Some("0")
        })
        .and_then(|path| path.attribute("fill"))
        .unwrap_or_else(|| panic!("missing terminal fill path for {node_id}"))
        .to_string()
}

fn requirement_source() -> &'static str {
    r#"requirementDiagram
  requirement req1 {
    id: 1
    text: Themed requirement
    risk: high
    verifymethod: analysis
  }
"#
}

fn requirement_svg_label_source() -> &'static str {
    r#"requirementDiagram
  requirement req1 {
    id: "R-1"
    text: "**Strong** and *emphasis* &amp; entity"
    risk: high
    verifymethod: analysis
  }
  element elem1 {
    type: simulation
    docref: "*Document* &amp; reference"
  }
  req1 - satisfies -> elem1
  req1 - traces -> req1
  style req1 color:#123456,fill:#abcdef
"#
}

#[test]
fn requirement_svg_labels_cover_nodes_relationships_and_self_loop_anchors() {
    let rendered = render_requirement_with_config(
        requirement_svg_label_source(),
        classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "htmlLabels": false
            })),
        ),
        RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid SVG label document");
    for (id, expected_lines) in [
        ("requirement-theme-req1", 6),
        ("requirement-theme-elem1", 4),
    ] {
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .expect("semantic node");
        assert_eq!(
            node.descendants()
                .filter(|node| node.has_tag_name("foreignObject"))
                .count(),
            0,
            "{id}"
        );
        let labels: Vec<_> = node
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect();
        assert_eq!(
            labels.len(),
            expected_lines,
            "each populated line of {id} is SVG text"
        );
        assert!(
            labels
                .iter()
                .all(|label| label.descendants().any(|node| node.has_tag_name("tspan")))
        );
    }
    let req = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-req1"))
        .unwrap();
    let text = req
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(text.contains("Strong and emphasis & entity"), "{text}");
    for (word, property, value) in [
        ("Strong", "font-weight", "bold"),
        ("emphasis", "font-style", "italic"),
    ] {
        assert!(
            req.descendants().any(|node| node.has_tag_name("tspan")
                && node.text().is_some_and(|text| text.trim() == word)
                && node.attribute(property) == Some(value)),
            "missing Markdown {word}"
        );
    }
    assert!(
        req.descendants()
            .filter(|node| node.has_tag_name("text"))
            .all(|label| {
                label
                    .attribute("style")
                    .is_some_and(|style| style.contains("fill:#123456"))
                    && label.descendants().any(|node| {
                        node.has_tag_name("tspan")
                            && node
                                .attribute("style")
                                .is_some_and(|style| style.contains("color:#123456"))
                    })
            })
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#abcdef"
    );
    let header = req
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("req1"))
        .unwrap();
    assert!(
        header
            .attribute("style")
            .is_some_and(|style| style.contains("font-weight: bold"))
    );
    assert!(
        req.descendants()
            .filter(|node| node.has_tag_name("text"))
            .all(|node| node.attribute("text-anchor").is_none())
    );
    let edge_labels: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("data-id").is_some())
        .collect();
    assert!(!edge_labels.is_empty());
    for label in edge_labels {
        assert!(
            !label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        assert!(label.descendants().any(|node| node.has_tag_name("text")));
    }
    for id in ["req1---req1---1", "req1---req1---2"] {
        let anchor = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .expect("self-loop anchor");
        assert!(
            !anchor
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        assert!(anchor.descendants().any(|node| node.has_tag_name("text")));
    }
}

#[test]
fn requirement_default_labels_match_explicit_html_labels() {
    let default = render_requirement_with_config(
        requirement_svg_label_source(),
        classic_engine(),
        RenderEnvironment::deterministic(),
    );
    let explicit = render_requirement_with_config(
        requirement_svg_label_source(),
        classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"htmlLabels": true}),
            ),
        ),
        RenderEnvironment::deterministic(),
    );
    assert_eq!(default.svg(), explicit.svg());
    let document = roxmltree::Document::parse(default.svg()).expect("valid default HTML labels");
    let req = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-req1"))
        .unwrap();
    assert_eq!(
        req.descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
            .count(),
        6
    );
}

#[derive(Debug)]
struct RequirementMeasurementCall {
    operation: TextMeasurementOperation,
    text: String,
    style: TextStyle,
    wrap_mode: Option<WrapMode>,
}

impl RequirementMeasurementCall {
    fn is_terminal(&self, html_labels: bool) -> bool {
        if html_labels {
            self.operation == TextMeasurementOperation::Wrapped
                && self.wrap_mode == Some(WrapMode::HtmlLike)
        } else {
            matches!(
                self.operation,
                TextMeasurementOperation::TspanBBoxHeight
                    | TextMeasurementOperation::CreateTextBBoxYOffset
            )
        }
    }
}

#[derive(Debug)]
struct RequirementSvgModeProbe {
    calls: Arc<Mutex<Vec<RequirementMeasurementCall>>>,
    width_per_byte: Option<f64>,
}

impl RequirementSvgModeProbe {
    fn record(
        &self,
        operation: TextMeasurementOperation,
        text: &str,
        style: &TextStyle,
        wrap_mode: Option<WrapMode>,
    ) {
        self.calls.lock().unwrap().push(RequirementMeasurementCall {
            operation,
            text: text.to_owned(),
            style: style.clone(),
            wrap_mode,
        });
    }
}

impl TextMeasurer for RequirementSvgModeProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: text.len() as f64 * self.width_per_byte.unwrap_or(style.font_size),
            height: style.font_size,
            line_count: 1,
        }
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        _width: Option<f64>,
        mode: WrapMode,
    ) -> TextMetrics {
        self.record(TextMeasurementOperation::Wrapped, text, style, Some(mode));
        let mut metrics = self.measure(text, style);
        // HTML reports its CSS line box; SVG probes retain the tight font-sized bbox.
        if mode == WrapMode::HtmlLike {
            metrics.height = style.font_size * 1.5;
        }
        metrics
    }

    fn measure_html_line_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record(
            TextMeasurementOperation::HtmlLineHeight,
            text,
            style,
            Some(WrapMode::HtmlLike),
        );
        self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
            .height
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record(TextMeasurementOperation::ComputedLength, text, style, None);
        self.measure(text, style).width
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record(TextMeasurementOperation::TspanBBoxHeight, text, style, None);
        self.measure(text, style).height
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record(
            TextMeasurementOperation::CreateTextBBoxYOffset,
            text,
            style,
            None,
        );
        0.0
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        self.record(
            TextMeasurementOperation::MermaidCalculateTextDimensions,
            text,
            style,
            None,
        );
        self.measure(text, style)
    }
}

fn requirement_foreign_object_for_text<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    text: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("foreignObject")
                && node
                    .descendants()
                    .any(|child| child.is_text() && child.text() == Some(text))
        })
        .unwrap_or_else(|| panic!("missing Requirement HTML label containing {text:?}"))
}

#[test]
fn requirement_html_terminal_projection_preserves_repeated_break_rows() {
    for breaks in 1..=4 {
        let source = format!(
            "requirementDiagram\nrequirement req1 {{\ntext: \"First{}Last\"\nrisk: low\n}}\n",
            "<br/>".repeat(breaks)
        );
        let rendered = render_requirement_with_config(
            &source,
            classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": true, "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
                ),
            ),
            RenderEnvironment::deterministic(),
        );
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Requirement XHTML");
        let label = requirement_foreign_object_for_text(&document, "Last");
        assert_eq!(
            label
                .descendants()
                .filter(|node| node.has_tag_name("br"))
                .count(),
            breaks,
        );
        assert_eq!(
            label.attribute("height").unwrap().parse::<f64>().unwrap(),
            (breaks + 1) as f64 * 36.0,
            "every HTML break retains its line box: {source}"
        );
    }
}

#[test]
fn requirement_html_terminal_projection_measures_code_without_literal_markup() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.requirement-html-code-projection").unwrap(),
            "test",
        )
        .unwrap(),
        Arc::new(RequirementSvgModeProbe {
            calls: Arc::clone(&calls),
            width_per_byte: Some(1.0),
        }),
    );
    let rendered = render_requirement_with_config(
        "requirementDiagram\nrequirement req1 {\ntext: \"<code>Account</code>\"\n}\n",
        classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"htmlLabels": true, "fontSize": 10, "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
            ),
        ),
        RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Requirement code XHTML");
    let label = requirement_foreign_object_for_text(&document, "Account");
    assert!(label.descendants().any(|node| node.has_tag_name("code")));
    assert_eq!(label.attribute("height"), Some("36"));
    let calls = calls.lock().unwrap();
    assert!(
        calls.iter().any(|call| {
            call.is_terminal(true)
                && call.text == "Account"
                && call.style.font_family.as_deref() == Some("monospace")
                && call.style.font_size == 24.0
        }),
        "code must be measured with its actual terminal font: {calls:?}"
    );
    assert!(
        calls
            .iter()
            .filter(|call| call.is_terminal(true))
            .all(|call| { !call.text.contains("<code>") && !call.text.contains("</code>") }),
        "terminal measurement must consume XHTML text rather than source markup: {calls:?}"
    );
    assert!(
        calls.iter().any(|call| {
            call.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
                && call.text == "Text: <code>Account</code>"
                && call.style.font_size == 10.0
        }),
        "the independent calculation probe must retain its input and size: {calls:?}"
    );
}

#[test]
fn requirement_html_sized_icons_keep_their_advance_in_node_measurement() {
    let width = |text: &str| {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.requirement-sized-icon").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(RequirementSvgModeProbe {
                calls: Arc::clone(&calls),
                width_per_byte: Some(1.0),
            }),
        );
        let rendered = render_requirement_with_config(
            &format!("requirementDiagram\nrequirement req1 {{\ntext: \"{text}\"\n}}\n"),
            classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": true, "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
                ),
            ),
            RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
        );
        assert!(
            calls
                .lock()
                .unwrap()
                .iter()
                .all(|call| !call.text.contains('\u{fffc}'))
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let width = requirement_foreign_object_for_text(&document, "Account")
            .attribute("width")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        width
    };
    let ordinary = width("<big>Account</big>");
    let with_icon = width("<big><i class='fa fa-car'></i>Account</big>");
    assert_eq!(with_icon - ordinary, 36.0);
}

#[test]
fn requirement_html_declared_font_height_requests_are_linear_in_each_render_phase() {
    for word_count in [64, 256] {
        let visible = vec!["x"; word_count].join(" ");
        let source = format!(
            "requirementDiagram\nrequirement req1 {{\ntext: \"<span style='font-size:16px'>{visible}</span>\"\n}}\n"
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.requirement-html-height-payload").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(RequirementSvgModeProbe {
                calls: Arc::clone(&calls),
                width_per_byte: Some(1.0),
            }),
        );
        let engine = classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": true, "securityLevel": "loose", "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
                ),
            );
        let parsed = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
            .begin_session()
            .unwrap();
        let prepared = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let prepared_call_count = calls.lock().unwrap().len();
        let rendered = prepared
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let calls = calls.lock().unwrap();
        for (phase, phase_calls) in [
            ("prepare", &calls[..prepared_call_count]),
            ("SVG replay", &calls[prepared_call_count..]),
        ] {
            let declared_font: Vec<_> = phase_calls
                .iter()
                .filter(|call| {
                    call.operation == TextMeasurementOperation::HtmlLineHeight
                        && call.style.font_size == 16.0
                })
                .collect();
            assert_eq!(
                declared_font.len(),
                1,
                "{phase}: the final 16px group needs one height request for {word_count} words"
            );
            assert_eq!(
                declared_font
                    .iter()
                    .map(|call| call.text.len())
                    .sum::<usize>(),
                visible.len(),
                "{phase}: submitted height payload must grow linearly with the visible text"
            );
            assert_eq!(declared_font[0].text, visible);
        }

        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = requirement_foreign_object_for_text(&document, &visible);
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
fn requirement_html_inline_sizes_use_terminal_parent_and_css_root_contexts() {
    for (size, terminal_size) in [("48px", 48.0), ("2em", 64.0), ("2rem", 32.0)] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.requirement-html-inline-size").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(RequirementSvgModeProbe {
                calls: Arc::clone(&calls),
                width_per_byte: Some(1.0),
            }),
        );
        let source = format!(
            "requirementDiagram\nrequirement req1 {{\ntext: \"<span style='font-size:{size}'>Account</span>\"\n}}\nstyle req1 font-size:32px\n"
        );
        let rendered = render_requirement_with_config(
            &source,
            classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": true, "fontSize": 10, "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
                ),
            ),
            RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
        );
        let document = roxmltree::Document::parse(rendered.svg())
            .expect("valid Requirement inline-size XHTML");
        let label = requirement_foreign_object_for_text(&document, "Account");
        assert!(label.descendants().any(|node| {
            node.has_tag_name("span")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains(size))
        }));
        assert_eq!(
            label.attribute("height").unwrap().parse::<f64>().unwrap(),
            terminal_size * 1.5,
            "the prepared label must retain the enlarged terminal line box: {size}"
        );
        let calls = calls.lock().unwrap();
        assert!(
            calls.iter().any(|call| {
                call.is_terminal(true)
                    && call.text.contains("Account")
                    && call.style.font_size == terminal_size
            }),
            "inline size {size} must use the terminal context: {calls:?}"
        );
        assert!(
            calls.iter().any(|call| {
                call.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
                    && call.text.contains("font-size:")
                    && call.style.font_size == 10.0
            }),
            "inline sizing must not replace the independent calculation probe: {calls:?}"
        );
    }
}

fn same_named_requirement_and_element_source() -> &'static str {
    r#"requirementDiagram
  requirement shared {
    id: R-1
    text: Requirement owner
    risk: low
    verifymethod: analysis
  }
  element shared {
    type: simulation
    docref: Element owner
  }
"#
}

#[test]
fn requirement_source_points_reach_terminal_measurement_and_keep_css_ownership() {
    for html_labels in [false, true] {
        for declaration in [
            "style req1 font-size:24pt",
            "classDef sourceSize font-size:24pt\nclass req1 sourceSize",
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let profile = TextMeasurementProfile::new(
                TextMeasurementProfileIdentity::new(
                    MeasurementProfileId::new("test.requirement-source-points").unwrap(),
                    "test",
                )
                .unwrap(),
                Arc::new(RequirementSvgModeProbe {
                    calls: Arc::clone(&calls),
                    width_per_byte: None,
                }),
            );
            let rendered = render_requirement_with_config(
                &format!("{}\n{declaration}\n", requirement_source()),
                classic_engine().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"htmlLabels": html_labels, "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
                    ),
                ),
                RenderEnvironment::deterministic()
                    .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
            );
            assert!(rendered.svg().contains("font-size:24pt !important"));
            let calls = calls.lock().unwrap();
            let terminal = calls.iter().filter(|call| call.is_terminal(html_labels));
            assert!(terminal.clone().next().is_some());
            assert!(
                terminal.clone().all(|call| call.style.font_size == 32.0),
                "{calls:?}"
            );
            assert!(
                calls.iter().any(|call| {
                    call.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
                        && call.style.font_size == 16.0
                }),
                "the independent root/config wrapping probe must stay unchanged: {calls:?}"
            );
        }
    }
}

#[test]
fn requirement_source_font_faces_reach_measurement_and_override_name_defaults() {
    for html_labels in [false, true] {
        for (declarations, weight, face) in [
            ("font-weight:500,font-style:italic", "500", "italic"),
            ("font-weight:700,font-style:OBLIQUE", "700", "oblique"),
            (
                "font-weight:bold!important,font-weight:normal,font-style:italic,font-style:normal",
                "normal",
                "normal",
            ),
        ] {
            for source_style in [
                format!("style req1 font-size:32px,{declarations}"),
                format!("classDef sourceFace font-size:32px,{declarations}\nclass req1 sourceFace"),
                format!(
                    "classDef earlierFace font-weight:900,font-style:normal\nclass req1 earlierFace\nstyle req1 font-size:32px,{declarations}"
                ),
                format!(
                    "style req1 font-weight:900,font-style:normal\nclassDef laterFace font-size:32px,{declarations}\nclass req1 laterFace"
                ),
            ] {
                let calls = Arc::new(Mutex::new(Vec::new()));
                let profile = TextMeasurementProfile::new(
                    TextMeasurementProfileIdentity::new(
                        MeasurementProfileId::new("test.requirement-source-faces").unwrap(),
                        "test",
                    )
                    .unwrap(),
                    Arc::new(RequirementSvgModeProbe {
                        calls: Arc::clone(&calls),
                        width_per_byte: None,
                    }),
                );
                let rendered = render_requirement_with_config(
                    &format!("{}\n{source_style}\n", requirement_source()),
                    classic_engine().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({"htmlLabels": html_labels, "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
                        ),
                    ),
                    RenderEnvironment::deterministic()
                        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
                );
                let calls = calls.lock().unwrap();
                let node_calls = calls
                    .iter()
                    .filter(|call| call.is_terminal(html_labels) && call.style.font_size == 32.0)
                    .map(|call| &call.style);
                assert!(node_calls.clone().count() > 0);
                let matches_source = |style: &&TextStyle| {
                    style.font_weight.as_deref() == Some(weight)
                        && style.font_style.as_deref() == Some(face)
                };
                assert!(node_calls.clone().any(|style| matches_source(&style)));
                assert!(
                    node_calls.clone().all(|style| {
                        matches_source(&style)
                            || (!html_labels
                                && matches!(style.font_weight.as_deref(), None | Some("normal"))
                                && matches!(style.font_style.as_deref(), None | Some("normal")))
                    }),
                    "{html_labels}/{source_style}: {calls:?}"
                );
                assert!(
                    calls.iter().any(|call| {
                        call.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
                            && call.style.font_size == 16.0
                            && call.style.font_weight.is_none()
                            && call.style.font_style.is_none()
                    }),
                    "{html_labels}/{source_style}: {calls:?}"
                );
                if !html_labels {
                    assert!(calls.iter().any(|call| {
                        call.operation == TextMeasurementOperation::ComputedLength
                            && call.style.font_size == 32.0
                            && matches!(call.style.font_weight.as_deref(), None | Some("normal"))
                            && matches!(call.style.font_style.as_deref(), None | Some("normal"))
                    }));
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let multiline_labels = document
                        .descendants()
                        .filter(|node| {
                            node.has_tag_name("text")
                                && node
                                    .children()
                                    .filter(|row| row.has_tag_name("tspan"))
                                    .count()
                                    > 1
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        !multiline_labels.is_empty(),
                        "the probe must force SVG wrapping"
                    );
                    for label in multiline_labels {
                        for word in label
                            .descendants()
                            .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
                        {
                            let style = word.attribute("style").unwrap_or("").to_ascii_lowercase();
                            let first_row = word
                                .parent_element()
                                .unwrap()
                                .prev_sibling_element()
                                .is_none();
                            if first_row {
                                assert!(
                                    style.contains(&format!("font-weight:{weight} !important"))
                                        && style.contains(&format!("font-style:{face} !important")),
                                    "the first row receives the source face: {source_style}/{word:?}"
                                );
                            } else {
                                assert!(
                                    style.is_empty(),
                                    "later rows keep Markdown faces: {word:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn requirement_svg_source_faces_override_markdown_but_html_keeps_child_faces() {
    for html_labels in [false, true] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.requirement-markdown-source-faces").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(RequirementSvgModeProbe {
                calls: Arc::clone(&calls),
                width_per_byte: None,
            }),
        );
        let source = format!(
            "{}\nstyle req1 font-size:32px,font-weight:500,font-style:normal\n",
            requirement_source().replace("Themed requirement", "\"**Strong** and *emphasis*\"")
        );
        let rendered = render_requirement_with_config(
            &source,
            classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": html_labels, "themeVariables": {"fontFamily":"Arial","fontSize":"24px"}}),
                ),
            ),
            RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
        );
        let calls = calls.lock().unwrap();
        let node_calls = calls
            .iter()
            .filter(|call| call.is_terminal(html_labels) && call.style.font_size == 32.0)
            .map(|call| &call.style);
        assert!(node_calls.clone().count() > 0);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        if html_labels {
            assert!(
                node_calls
                    .clone()
                    .any(|style| style.font_weight.as_deref() == Some("700"))
            );
            assert!(
                node_calls
                    .clone()
                    .any(|style| style.font_style.as_deref() == Some("italic"))
            );
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("strong"))
            );
            assert!(document.descendants().any(|node| node.has_tag_name("em")));
        } else {
            let node_rows = node_calls.clone();
            assert!(node_rows.clone().count() > 0);
            assert!(
                node_rows
                    .clone()
                    .any(|style| style.font_weight.as_deref() == Some("500"))
            );
            for (word, weight, face) in [
                ("Strong", "700", "normal"),
                ("emphasis", "normal", "italic"),
            ] {
                assert!(
                    calls.iter().any(|call| {
                        call.operation == TextMeasurementOperation::ComputedLength
                            && call.text.trim() == word
                            && call.style.font_weight.as_deref() == Some(weight)
                            && call.style.font_style.as_deref() == Some(face)
                    }),
                    "Markdown wrapping keeps its own face for {word}: {calls:?}"
                );
            }
            let words = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
                .collect::<Vec<_>>();
            assert!(
                words
                    .iter()
                    .any(|word| word.attribute("font-weight") == Some("bold"))
            );
            assert!(
                words
                    .iter()
                    .any(|word| word.attribute("font-style") == Some("italic"))
            );
            for word in words {
                let first_row = word
                    .parent_element()
                    .unwrap()
                    .prev_sibling_element()
                    .is_none();
                if first_row {
                    assert!(word.attribute("style").is_some_and(|style| {
                        style.contains("font-weight:500 !important")
                            && style.contains("font-style:normal !important")
                    }));
                } else {
                    assert!(word.attribute("style").is_none());
                }
                let weight = if first_row {
                    "500"
                } else if word.attribute("font-weight") == Some("bold") {
                    "700"
                } else {
                    "normal"
                };
                let face = if first_row {
                    "normal"
                } else {
                    word.attribute("font-style").unwrap()
                };
                for operation in [
                    TextMeasurementOperation::TspanBBoxHeight,
                    TextMeasurementOperation::CreateTextBBoxYOffset,
                ] {
                    assert!(
                        calls.iter().any(|call| {
                            call.operation == operation
                                && call.text == word.text().unwrap()
                                && call.style.font_size == 32.0
                                && call.style.font_weight.as_deref() == Some(weight)
                                && call.style.font_style.as_deref() == Some(face)
                        }),
                        "{operation:?} must measure the emitted face: {word:?}/{calls:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn same_named_requirement_and_element_render_one_default_fill_terminal() {
    let rendered = render_requirement_without_theme(same_named_requirement_and_element_source());
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid default Requirement SVG");
    let node_id = "requirement-theme-shared";

    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some(node_id))
            .count(),
        1
    );
    assert_eq!(terminal_fill_for(&document, node_id), "#ECECFF");
}

#[test]
fn requirement_source_styles_reject_css_injection_tokens() {
    let source = r#"requirementDiagram
  requirement unsafe {
    id: 1
    text: Unsafe source style
    risk: low
    verifymethod: analysis
  }
  classDef unsafe fill:#123456;stroke:url(javascript:alert(1))
  class unsafe unsafe
  style unsafe stroke:#654321;fill:red
"#;
    let rendered = render_requirement_without_theme(source);
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Requirement SVG");
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-unsafe"))
        .expect("unsafe Requirement node");
    let emitted_styles = requirement
        .descendants()
        .filter_map(|node| node.attribute("style"))
        .collect::<Vec<_>>()
        .join(";");

    assert!(
        !emitted_styles.contains("url("),
        "unsafe URL reached SVG: {emitted_styles}"
    );
    assert!(!emitted_styles.contains("javascript:"));
    assert!(!emitted_styles.contains(";fill:red"));
}

#[test]
fn requirement_svg_multiline_name_applies_role_to_the_first_row_only() {
    let rendered = render_requirement_without_theme(
        r#"%%{init: {"htmlLabels": false}}%%
requirementDiagram
requirement "W<br/>WW WW" {
  text: ok
}
"#,
    );
    let svg = rendered.svg().to_owned();
    let document = roxmltree::Document::parse(&svg).expect("valid Requirement SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node
                    .children()
                    .filter(|row| row.has_tag_name("tspan"))
                    .count()
                    == 2
                && node
                    .descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>()
                    == "WWW WW"
        })
        .expect("two-row requirement name");
    let rows = label
        .children()
        .filter(|row| row.has_tag_name("tspan"))
        .collect::<Vec<_>>();
    let first_word = rows[0]
        .children()
        .find(|word| word.has_tag_name("tspan"))
        .unwrap();
    assert!(
        first_word.attribute("font-weight") == Some("bold")
            || first_word
                .attribute("style")
                .is_some_and(|style| style.contains("font-weight: bold"))
    );
    let later_words = rows[1]
        .children()
        .filter(|word| word.has_tag_name("tspan"))
        .collect::<Vec<_>>();
    assert_eq!(later_words.len(), 2);
    for word in later_words {
        assert_eq!(word.attribute("font-weight"), Some("normal"));
        assert_eq!(word.attribute("style"), None);
    }
}
