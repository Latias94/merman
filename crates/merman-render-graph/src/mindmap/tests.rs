use merman_model::presentation::PresentationConfig;
use merman_model::resources::{InputResourcePolicy, ResourceProfile};
use merman_model::{MermaidConfig, OperationControl};

pub(super) fn presentation_config(value: serde_json::Value) -> PresentationConfig {
    PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(value),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

#[test]
fn mindmap_prepared_html_owns_the_sanitized_structure_and_releases_retained_bytes() {
    let meter = std::sync::Arc::new(merman_render_core::resources::OperationWorkMeter::new(
        merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let config = presentation_config(serde_json::json!({
        "securityLevel": "loose",
        "dompurifyConfig": {"FORBID_TAGS": ["strong"], "KEEP_CONTENT": false}
    }));
    let prepared = super::MindmapPreparedLabel::prepare(
        "root",
        "Alpha</div><strong>removed</strong>Beta",
        &config,
        &meter,
    )
    .expect("prepare the final HTML label");
    assert_eq!(prepared.xhtml, "<p>AlphaBeta</p>");
    let retained = meter.prepared_text_retained_bytes();
    assert!(retained >= prepared.xhtml.capacity());
    assert!(meter.used() > 0, "HTML parsing belongs to layout work");
    assert_eq!(meter.projected_svg_bytes(), 0);
    drop(prepared);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn mindmap_math_source_uses_its_separate_rendering_path() {
    let parsed = merman_core::MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "mindmap\n  root[\"$$x^2$$\"]\n",
            merman_core::ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let merman_model::presentation::RenderSemanticModel::Mindmap(model) = parsed.model() else {
        panic!("Mindmap fixture");
    };
    let meter = std::sync::Arc::new(merman_render_core::resources::OperationWorkMeter::new(
        merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let labels =
        super::MindmapPreparedLabels::prepare(model, &parsed.metadata().effective_config, &meter)
            .unwrap();
    assert!(labels.xhtml_for(&model.nodes[0].id).is_none());
}

#[test]
fn mindmap_prepared_html_preserves_retained_and_work_rejections() {
    use merman_render_core::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
    };
    let config = presentation_config(serde_json::json!({
        "securityLevel": "loose"
    }));
    for (limit, value, expected) in [
        (
            ResourceLimitId::MaxPreparedTextRetainedBytes,
            1,
            "max_prepared_text_retained_bytes",
        ),
        (
            ResourceLimitId::MaxLayoutWorkUnits,
            1,
            "max_layout_work_units",
        ),
    ] {
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(limit, value)
                .unwrap(),
        ));
        let error = super::MindmapPreparedLabel::prepare("root", "<b>label</b>", &config, &meter)
            .expect_err("prepared HTML must respect its operation budget");
        let merman_render_core::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected the original resource terminal");
        };
        assert_eq!(error.limit, expected);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
        assert_eq!(meter.projected_svg_bytes(), 0);
    }
}

#[test]
fn mindmap_prepared_html_preserves_layout_cancellation() {
    let control = merman_model::OperationControl::new();
    control.cancel();
    let meter = std::sync::Arc::new(
        merman_render_core::resources::OperationWorkMeter::new_with_control(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        ),
    );
    let config = presentation_config(serde_json::json!({
        "securityLevel": "loose"
    }));
    let error = super::MindmapPreparedLabel::prepare("root", "<b>label</b>", &config, &meter)
        .expect_err("HTML preparation must preserve cancellation");
    let merman_render_core::Error::Cancelled(error) = error else {
        panic!("expected cancellation");
    };
    assert_eq!(error.phase, merman_model::OperationPhase::Layout);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

struct FixedMeasurer;

impl merman_render_core::text::TextMeasurer for FixedMeasurer {
    fn measure(
        &self,
        _text: &str,
        _style: &merman_render_core::text::TextStyle,
    ) -> merman_render_core::text::TextMetrics {
        merman_render_core::text::TextMetrics {
            width: 73.0,
            height: 24.0,
            line_count: 1,
        }
    }
}

#[test]
fn mindmap_label_width_observes_projected_width_and_shape_resets() {
    let mut node: super::MindmapNodeModel = serde_json::from_value(serde_json::json!({
        "id": "0", "domId": "node_0", "label": "Label", "shape": "defaultMindmapNode",
        "cssClasses": "mindmap-node section-root section--1", "width": 80.5
    }))
    .unwrap();
    for flowchart_width in [serde_json::json!(160.5), serde_json::json!("160.5px")] {
        let config = serde_json::json!({
            "mindmap": {"maxNodeWidth": 240},
            "flowchart": {"wrappingWidth": flowchart_width}
        });
        for shape in ["defaultMindmapNode", "mindmapCircle", "cloud", "bang"] {
            node.shape = shape.to_string();
            assert_eq!(super::mindmap_label_wrapping_width_px(&node, &config), 80.5);
        }
        for shape in ["rect", "rounded", "hexagon"] {
            node.shape = shape.to_string();
            assert_eq!(
                super::mindmap_label_wrapping_width_px(&node, &config),
                160.5
            );
            assert_eq!(
                super::mindmap_label_wrapping_width_px(&node, &serde_json::json!({})),
                120.0
            );
        }
        node.shape = "defaultMindmapNode".to_string();
        node.width = 0.0;
        assert_eq!(
            super::mindmap_label_wrapping_width_px(&node, &config),
            160.5
        );
        node.width = 80.5;
    }
}

#[test]
fn mindmap_label_text_for_layout_trims_single_line_delimiter_text() {
    assert_eq!(
        super::mindmap_label_text_for_layout("\n      The root\n    "),
        "The root"
    );
    assert_eq!(
        super::mindmap_label_text_for_layout("\r\nThe root"),
        "The root"
    );
    assert_eq!(super::mindmap_label_text_for_layout("The root"), "The root");
    assert_eq!(
        super::mindmap_label_text_for_layout("\n      first\n      second\n    "),
        "\n      first\n      second\n    "
    );
}

#[test]
fn mindmap_plain_wrapping_label_uses_wrapped_container_width() {
    let measurer = merman_render_core::text::DeterministicTextMeasurer::default();
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let (width, height) = super::mindmap_label_bbox_px(
        "A root with a long text that wraps to keep the node size in check",
        &measurer,
        &style,
        200.0,
    );

    assert!(width > 0.0 && width <= 200.0);
    assert!(
        height > style.font_size,
        "long prose should wrap to multiple rows"
    );
}

#[test]
fn mindmap_markdown_wrapping_respects_max_node_width() {
    let measurer = merman_render_core::text::DeterministicTextMeasurer::default();
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let (width, height) = super::mindmap_label_bbox_px(
        "The dog in **the** hog... a *very long text* that wraps to a new line",
        &measurer,
        &style,
        200.0,
    );

    assert_eq!(width, 200.0);
    assert_eq!(height, 72.0);
}

#[test]
fn mindmap_break_spaces_preserves_trailing_indentation_line_box() {
    let measurer = merman_render_core::text::DeterministicTextMeasurer::default();
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let source = "\n    Multi-line root\n    with three lines\n  ";
    assert_eq!(
        merman_render_core::text::mermaid_markdown_to_html_label_fragment(source, true),
        "    Multi-line root\n    with three lines\n  "
    );
    let (width, height) = super::mindmap_label_bbox_px(source, &measurer, &style, 200.0);

    assert_eq!(width, 200.0);
    assert_eq!(height, 72.0);
}

#[test]
fn mindmap_html_labels_measure_visible_content_instead_of_markup() {
    let measurer = merman_render_core::text::DeterministicTextMeasurer::default();
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let expected = merman_render_core::text::TextMeasurer::measure_wrapped(
        &measurer,
        "docs",
        &style,
        Some(200.0),
        merman_render_core::text::WrapMode::HtmlLike,
    );

    let actual = super::mindmap_label_bbox_px(
        r#"<a href='https://mermaid.js.org/' rel="noopener" target="_blank">docs</a>"#,
        &measurer,
        &style,
        200.0,
    );

    assert_eq!(
        actual,
        (
            merman_render_core::text::round_to_1_64_px(expected.width),
            expected.height
        )
    );
}

#[test]
fn mindmap_layout_decodes_mermaid_entity_placeholders_before_measurement() {
    let measurer = merman_render_core::text::DeterministicTextMeasurer::default();
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let expected = super::mindmap_label_bbox_px("Circle: &♥ ∞", &measurer, &style, 200.0);

    let actual =
        super::mindmap_label_bbox_px("Circle: &ﬂ°°9829¶ß &infin;", &measurer, &style, 200.0);

    assert_eq!(actual, expected);
}

#[test]
fn mindmap_code_only_html_measures_visible_code_content() {
    let measurer = merman_render_core::text::DeterministicTextMeasurer::default();
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let mut code_style = style.clone();
    code_style.font_family = Some("monospace".to_owned());
    let expected = merman_render_core::text::TextMeasurer::measure_wrapped(
        &measurer,
        "note about mermaid",
        &code_style,
        Some(200.0),
        merman_render_core::text::WrapMode::HtmlLike,
    );

    let actual = super::mindmap_label_bbox_px(
        r#"<a href="https://mermaid.js.org/"><code>note about mermaid</code></a>"#,
        &measurer,
        &style,
        200.0,
    );

    assert_eq!(
        actual,
        (
            merman_render_core::text::round_to_1_64_px(expected.width),
            expected.height
        )
    );
}

#[test]
fn mindmap_plain_labels_use_the_selected_measurer_without_content_adjustment() {
    let measurer = FixedMeasurer;
    let style = super::mindmap_text_style(&serde_json::json!({}));

    for text in [
        "String containing []",
        "String containing ()",
        "Waterfall",
        "the root",
        "Root",
        "unseen label",
    ] {
        let (width, height) = super::mindmap_label_bbox_px(text, &measurer, &style, 200.0);
        assert_eq!(width, 73.0);
        assert_eq!(height, 24.0);
    }
}

#[test]
fn mindmap_circle_encloses_label_diagonal_with_actual_node_look() {
    let mut node = super::MindmapNodeModel {
        id: "0".to_string(),
        dom_id: "node_0".to_string(),
        label: "Circle".to_string(),
        label_type: "markdown".to_string(),
        is_group: false,
        shape: "mindmapCircle".to_string(),
        width: 0.0,
        height: 0.0,
        padding: 7.0,
        css_classes: "mindmap-node section-root section--1".to_string(),
        css_styles: Vec::new(),
        look: String::new(),
        icon: None,
        x: None,
        y: None,
        level: 0,
        node_id: "0".to_string(),
        node_type: 3,
        section: None,
    };
    // A 30x40 label has a 50px diagonal. mindmapCircle supplies all 7px padding;
    // Neo instead uses shared circle.ts's single 16px radius padding step.
    for (node_look, config_look, expected_diameter) in [
        ("", "neo", 82.0),
        ("default", "neo", 82.0),
        ("classic", "neo", 64.0),
        ("handDrawn", "neo", 64.0),
        ("neo", "classic", 82.0),
        ("classic", "classic", 64.0),
    ] {
        node.look = node_look.to_string();
        let config = serde_json::json!({ "look": config_look, "theme": "redux" });
        let dimensions = super::mindmap_node_dimensions_from_label_bbox(
            &node,
            30.0,
            40.0,
            super::mindmap_node_look(&node.look, &config),
        );
        assert_eq!(
            dimensions,
            (expected_diameter, expected_diameter, 30.0, 40.0),
            "node look={node_look}, config look={config_look}"
        );
    }
}

#[test]
fn mindmap_svg_text_style_matches_root_css_font_sources() {
    let config = serde_json::json!({
        "htmlLabels": false,
        "fontFamily": "Root font",
        "fontSize": 18,
        "themeVariables": { "fontFamily": "Theme font", "fontSize": "24px" }
    });
    let style = super::mindmap_text_style(&config);
    assert_eq!(style.font_family.as_deref(), Some("Theme font"));
    assert_eq!(style.font_size, 24.0);
    let typed_style =
        super::mindmap_text_style_with_font_family(&config, "Typed font,monospace".to_owned());
    assert_eq!(
        typed_style.font_family.as_deref(),
        Some("Typed font,monospace")
    );
    assert_eq!(typed_style.font_size, 24.0);
    let html_style = super::mindmap_text_style_with_font_family(
        &serde_json::json!({"htmlLabels": true, "themeVariables": {"fontSize": "24px"}}),
        "Typed font,monospace".to_owned(),
    );
    assert_eq!(html_style.font_family, typed_style.font_family);
    assert_eq!(html_style.font_size, 24.0);
    assert_eq!(
        super::mindmap_text_style(&serde_json::json!({ "htmlLabels": false, "fontSize": 18 }))
            .font_size,
        18.0
    );
    assert_eq!(
        super::mindmap_text_style(&serde_json::json!({ "htmlLabels": false })).font_size,
        16.0
    );
}

#[test]
fn mindmap_cloud_layout_uses_rendered_path_bbox_dimensions() {
    let measurer = FixedMeasurer;
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let node = super::MindmapNodeModel {
        id: "0".to_string(),
        dom_id: "node_0".to_string(),
        label: "arbitrary cloud label".to_string(),
        label_type: String::new(),
        is_group: false,
        shape: "cloud".to_string(),
        width: 0.0,
        height: 0.0,
        padding: 10.0,
        css_classes: "mindmap-node section-root section--1".to_string(),
        css_styles: Vec::new(),
        look: String::new(),
        icon: None,
        x: None,
        y: None,
        level: 0,
        node_id: "0".to_string(),
        node_type: 0,
        section: None,
    };

    let (width, height, label_width, label_height) =
        super::mindmap_node_dimensions_px(&node, &measurer, &style, 200.0);

    let shape_width = label_width + node.padding;
    let shape_height = label_height + node.padding;
    let expected = super::svg::mindmap_cloud_rendered_bbox_size_px(shape_width, shape_height)
        .expect("cloud path bounds");

    assert_eq!((label_width, label_height), (73.0, 24.0));
    assert_eq!((width, height), expected);
    assert!(width > shape_width && height > shape_height);
}

#[test]
fn mindmap_hexagon_layout_uses_mermaid_11_16_shape_geometry() {
    let measurer = FixedMeasurer;
    let style = super::mindmap_text_style(&serde_json::json!({}));
    let node = super::MindmapNodeModel {
        id: "0".to_string(),
        dom_id: "node_0".to_string(),
        label: "arbitrary hexagon label".to_string(),
        label_type: String::new(),
        is_group: false,
        shape: "hexagon".to_string(),
        width: 0.0,
        height: 0.0,
        padding: 20.0,
        css_classes: "mindmap-node section-root section--1".to_string(),
        css_styles: Vec::new(),
        look: String::new(),
        icon: None,
        x: None,
        y: None,
        level: 0,
        node_id: "0".to_string(),
        node_type: 0,
        section: None,
    };

    let dimensions = super::mindmap_node_dimensions_px(&node, &measurer, &style, 200.0);

    assert_eq!(dimensions, (115.0, 44.0, 73.0, 24.0));
}
