use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::class::Namespace;
use merman_model::presentation::RenderSemanticModel;

use merman_render_core::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
};

fn unexpected_interface(
    _: merman_render_core::text::create_text_html::FlowchartLabelMetricsRequest<'_>,
) -> TextMetrics {
    panic!("this label-retention fixture contains no interfaces")
}

#[test]
fn class_html_and_fallback_plans_enforce_retained_limits_and_release_partial_preparation() {
    use merman_render_core::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
    };
    use std::sync::Arc;

    for (source, prepared_html) in [
        (
            "classDiagram\nclass A {\n<<interface>>\n+value: String\n+call()\n}\nclass B\n",
            true,
        ),
        (
            "classDiagram\nclass A[\"$$x$$\"]\nclass A {\n+value\n+call()\n}\nclass B[\"$$y$$\"]\n",
            false,
        ),
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let config =
            crate::test_support::presentation_config(serde_json::json!({"htmlLabels":true}));
        let typography = super::ClassTextThemePlan::resolve(None, &config);
        let render_config = super::ClassRenderConfig::resolve(&config, &typography);
        let measurer = DeterministicTextMeasurer::default();
        for uses_elk_adapter in [false, true] {
            let measure = |policy| {
                let meter = Arc::new(OperationWorkMeter::new(policy));
                let work = super::OperationLayoutWorkControl::new(Arc::clone(&meter));
                let result = super::measured::measure_class_diagram(
                    model,
                    &config,
                    &measurer,
                    None,
                    &render_config,
                    uses_elk_adapter,
                    unexpected_interface,
                    Some(&work),
                );
                (result, meter)
            };
            let policy = RenderResourcePolicy::unbounded_for_trusted_input();
            let (measured, meter) = measure(policy);
            let measured = measured.unwrap();
            assert!(measured.prepared_labels.titles_by_id.is_empty());
            assert!(measured.prepared_labels.edges_by_id.is_empty());
            assert_eq!(measured.class_label_plans_by_id.len(), 2);
            assert_eq!(
                matches!(
                    measured.class_label_plans_by_id["A"].as_ref(),
                    super::ClassNodeLabelPlan::PreparedHtml(_)
                ),
                prepared_html
            );
            let retained = meter.prepared_text_retained_bytes();
            assert!(retained > 0);
            drop(measured);
            assert_eq!(meter.prepared_text_retained_bytes(), 0);

            let (exact, meter) = measure(
                policy
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained)
                    .unwrap(),
            );
            assert!(exact.is_ok());
            assert_eq!(meter.prepared_text_retained_bytes(), retained);
            drop(exact);
            assert_eq!(meter.prepared_text_retained_bytes(), 0);

            let (rejected, meter) = measure(
                policy
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained - 1)
                    .unwrap(),
            );
            let Some(merman_render_core::Error::ResourceLimitExceeded(limit)) = rejected.err()
            else {
                panic!("expected partial Class plan preparation to exceed the retained limit");
            };
            assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
            assert!(meter.prepared_text_retained_bytes_peak() > 0);
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }
}

#[test]
fn class_html_plan_accounts_for_spare_xhtml_and_container_capacity() {
    use merman_render_core::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
    };
    use std::sync::Arc;

    let label = || {
        let mut xhtml = String::with_capacity(4096);
        xhtml.push_str("<p>x</p>");
        super::ClassPreparedHtmlLabel {
            metrics: TextMetrics {
                width: 1.0,
                height: 1.0,
                line_count: 1,
            },
            max_width_px: 100,
            xhtml,
        }
    };
    let mut labels = super::ClassPreparedHtmlNodeLabels {
        title: label(),
        annotation: Some(label()),
        members: Vec::with_capacity(32),
        methods: Vec::with_capacity(64),
        _retained_reservation: None,
    };
    labels.members.push(label());
    labels.methods.push(label());
    let xhtml_capacity = std::iter::once(&labels.title)
        .chain(labels.annotation.iter())
        .chain(labels.members.iter())
        .chain(labels.methods.iter())
        .map(|label| label.xhtml.capacity())
        .sum::<usize>();
    let row_capacity = (labels.members.capacity() + labels.methods.capacity())
        * std::mem::size_of::<super::ClassPreparedHtmlLabel>();
    let mut id = String::with_capacity(2048);
    id.push('A');
    let payload_bytes = xhtml_capacity + row_capacity + id.capacity();
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, payload_bytes)
            .unwrap(),
    ));
    let work = super::OperationLayoutWorkControl::new(Arc::clone(&meter));
    let mut plan = super::ClassNodeLabelPlan::PreparedHtml(labels);
    let Err(merman_render_core::Error::ResourceLimitExceeded(limit)) =
        plan.reserve_retained_bytes(id.capacity(), &work)
    else {
        panic!("the plan and Arc/map owners must also fit after admitting payload capacities");
    };
    assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
    assert!(limit.actual > payload_bytes);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn class_html_prepared_plans_release_on_mid_preparation_cancellation() {
    use merman_render_core::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    struct CancellingMeasurer {
        meter: Arc<OperationWorkMeter>,
        control: merman_model::OperationControl,
    }

    impl TextMeasurer for CancellingMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            if self.meter.prepared_text_retained_bytes() > 0 {
                self.control.cancel();
            }
            DeterministicTextMeasurer::default().measure_wrapped(text, style, max_width, wrap_mode)
        }
    }

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "classDiagram\nclass A\nclass B\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let config = crate::test_support::presentation_config(serde_json::json!({"htmlLabels":true}));
    let typography = super::ClassTextThemePlan::resolve(None, &config);
    let render_config = super::ClassRenderConfig::resolve(&config, &typography);
    let control = merman_model::OperationControl::new();
    let meter = Arc::new(OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        control.clone(),
    ));
    let work = super::OperationLayoutWorkControl::new(Arc::clone(&meter));
    let measurer = CancellingMeasurer {
        meter: Arc::clone(&meter),
        control,
    };
    let result = super::measured::measure_class_diagram(
        model,
        &config,
        &measurer,
        None,
        &render_config,
        false,
        unexpected_interface,
        Some(&work),
    );
    assert!(matches!(
        result,
        Err(merman_render_core::Error::Cancelled(_))
    ));
    assert!(meter.prepared_text_retained_bytes_peak() > 0);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn class_svg_prepared_labels_own_retained_bytes_and_release_partial_preparation() {
    use merman_render_core::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
    };

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "classDiagram\nclass A[\"**OwnershipTitle**\"]\nclass B\nA --> B : **OwnershipEdge**\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let config = crate::test_support::presentation_config(serde_json::json!({"htmlLabels":false}));
    let typography = super::ClassTextThemePlan::resolve(None, &config);
    let render_config = super::ClassRenderConfig::resolve(&config, &typography);
    let measurer = DeterministicTextMeasurer::default();
    for uses_elk_adapter in [false, true] {
        let measure = |policy| {
            let meter = std::sync::Arc::new(OperationWorkMeter::new(policy));
            let work = super::OperationLayoutWorkControl::new(std::sync::Arc::clone(&meter));
            let result = super::measured::measure_class_diagram(
                model,
                &config,
                &measurer,
                None,
                &render_config,
                uses_elk_adapter,
                unexpected_interface,
                Some(&work),
            );
            (result, meter)
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let (measured, meter) = measure(policy);
        let mut measured = measured.unwrap();
        let retained = meter.prepared_text_retained_bytes();
        assert!(retained > 0);
        let labels = std::mem::take(&mut measured.prepared_labels);
        assert_eq!(labels.titles_by_id.len(), 2);
        assert_eq!(
            labels.titles_by_id["A"].label.plain_text(),
            "OwnershipTitle"
        );
        assert_eq!(labels.edges_by_id.len(), 1);
        assert_eq!(
            labels
                .edges_by_id
                .values()
                .next()
                .unwrap()
                .label
                .plain_text(),
            "OwnershipEdge"
        );
        drop(measured);
        assert_eq!(meter.prepared_text_retained_bytes(), retained);
        drop(labels);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);

        let (exact, meter) = measure(
            policy
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained)
                .unwrap(),
        );
        assert!(exact.is_ok());
        assert_eq!(meter.prepared_text_retained_bytes(), retained);
        drop(exact);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);

        let (rejected, meter) = measure(
            policy
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained - 1)
                .unwrap(),
        );
        let Some(merman_render_core::Error::ResourceLimitExceeded(limit)) = rejected.err() else {
            panic!("expected the prepared-text ceiling to reject partial Class preparation");
        };
        assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}

#[test]
fn class_prepared_reservation_observes_cancellation_before_its_byte_ceiling() {
    use merman_render_core::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
    };

    let control = merman_model::OperationControl::new();
    let meter = std::sync::Arc::new(OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
            .unwrap(),
        control.clone(),
    ));
    let work = super::OperationLayoutWorkControl::new(std::sync::Arc::clone(&meter));
    control.cancel();
    assert!(matches!(
        work.reserve_prepared_text_retained_bytes(2),
        Err(merman_render_core::Error::Cancelled(_))
    ));
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn metrics_only_html_measurement_matches_prepared_labels() {
    let measurer = DeterministicTextMeasurer::default();
    let style = TextStyle::default();
    for (label, css, width) in [
        ("plain", "", 200),
        (
            "**bold** and <span style='color:red'>owned</span>",
            "font-size:20px",
            32,
        ),
        ("multiple<br/>lines", "font-family:monospace", 100),
    ] {
        let expected =
            super::class_prepare_html_label(&measurer, &style, label, width, css).metrics;
        let actual = super::class_html_measure_label_metrics(&measurer, &style, label, width, css);
        assert_eq!(
            (actual.width, actual.height, actual.line_count),
            (expected.width, expected.height, expected.line_count)
        );
    }
}

#[test]
fn class_member_display_preserves_authored_colon_spaces_and_classifiers() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "classDiagram\nclass Box {\n+left :String\n+right: String\n+both : String$\n+tight:String\n+convert(value: String) Result\n}",
            ParseOptions::default(),
        )
        .expect("parse Class member whitespace")
        .expect("detect Class diagram").into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let class = model.class("Box").expect("Box class");
    let members = class
        .members
        .iter()
        .map(super::class_member_display_text)
        .collect::<Vec<_>>();
    assert_eq!(
        members,
        [
            "+left :String",
            "+right: String",
            "+both : String",
            "+tight:String"
        ]
    );
    assert_eq!(class.members[2].classifier, "$");
    assert_eq!(class.members[2].css_style, "text-decoration:underline;");
    assert_eq!(
        super::class_member_display_text(&class.methods[0]),
        "+convert(value: String) : Result"
    );
    assert_eq!(
        super::class_member_create_text_input(&class.members[1]),
        "+right: String"
    );
}

struct ClassProbeMeasurer;

struct ClassPrecisionMeasurer;

struct ClassMathGeometryMeasurer;

#[derive(Debug)]
struct ClassMathGeometryRenderer;

impl TextMeasurer for ClassProbeMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 1,
        }
    }

    fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, style: &TextStyle) -> f64 {
        if style.font_family.as_deref() == Some("sans-serif") {
            120.0
        } else {
            80.0
        }
    }

    fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        17.0
    }
}

impl TextMeasurer for ClassPrecisionMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: 73.123_456_789,
            height: 17.25,
            line_count: 1,
        }
    }

    fn measure_svg_text_bbox_x(&self, _text: &str, _style: &TextStyle) -> (f64, f64) {
        (31.0, 42.123_456_789)
    }

    fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        panic!("formatted Class labels must not use the single-tspan operation")
    }
}

impl TextMeasurer for ClassMathGeometryMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: 29.0,
            height: 19.0,
            line_count: 1,
        }
    }

    fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        17.0
    }
}

impl merman_render_core::math::MathRenderer for ClassMathGeometryRenderer {
    fn render_html_label(
        &self,
        text: &str,
        _config: &merman_model::presentation::PresentationConfig,
    ) -> Option<String> {
        text.contains("$$").then(|| text.to_string())
    }

    fn measure_html_label(
        &self,
        text: &str,
        _config: &merman_model::presentation::PresentationConfig,
        _style: &TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> Option<TextMetrics> {
        text.contains("$$").then_some(TextMetrics {
            width: 123.0,
            height: 37.0,
            line_count: 1,
        })
    }
}

fn default_style() -> TextStyle {
    TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    }
}

fn class_model_with_namespace_chain(count: usize) -> super::ClassDiagramModel {
    let mut model: merman_model::families::class::ClassDiagramModel =
        serde_json::from_value(serde_json::json!({
            "type": "classDiagram",
            "direction": "TB",
            "classes": {},
            "constants": {
                "lineType": { "line": 0, "dottedLine": 1 },
                "relationType": {
                    "none": 0,
                    "aggregation": 1,
                    "extension": 2,
                    "composition": 3,
                    "dependency": 4,
                    "lollipop": 5
                }
            }
        }))
        .expect("minimal Class diagram model");
    for index in 0..count {
        let id = format!("ns{index}");
        model.namespaces.insert(
            id.clone(),
            Namespace {
                id,
                label: String::new(),
                class_ids: Vec::new(),
                note_ids: Vec::new(),
                parent: index.checked_sub(1).map(|parent| format!("ns{parent}")),
                explicit: true,
            },
        );
    }
    super::ClassDiagramModel::new(model, &merman_model::MermaidConfig::default())
}

#[test]
fn class_namespace_hierarchy_accepts_the_shared_depth_boundary() {
    let model = class_model_with_namespace_chain(merman_model::MAX_DIAGRAM_NESTING_DEPTH + 1);

    super::validate_class_namespace_hierarchy(&model)
        .expect("the shared maximum namespace nesting depth is valid");
}

#[test]
fn class_namespace_hierarchy_rejects_excessive_depth_without_recursion() {
    let model = class_model_with_namespace_chain(merman_model::MAX_DIAGRAM_NESTING_DEPTH + 2);

    let error = super::validate_class_namespace_hierarchy(&model)
        .expect_err("namespace nesting beyond the shared limit must be rejected");

    assert_eq!(
        error.to_string(),
        format!(
            "invalid semantic model: class namespace nesting depth exceeds {} at ns{}",
            merman_model::MAX_DIAGRAM_NESTING_DEPTH,
            merman_model::MAX_DIAGRAM_NESTING_DEPTH + 1
        )
    );
}

#[test]
fn class_namespace_hierarchy_rejects_parent_cycles() {
    let mut model = class_model_with_namespace_chain(3).into_model();
    model.namespaces["ns0"].parent = Some("ns2".to_string());
    let model = super::ClassDiagramModel::new(model, &merman_model::MermaidConfig::default());

    let error = super::validate_class_namespace_hierarchy(&model)
        .expect_err("namespace parent cycles must be rejected");

    assert!(error.to_string().contains("namespace parent cycle"));
}

#[test]
fn class_create_text_width_uses_shared_mermaid_dimensions() {
    let style = TextStyle {
        font_family: Some("Arial".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    assert_eq!(
        super::class_html_create_text_width_px("unseen label", &ClassProbeMeasurer, &style),
        130
    );
}

#[test]
fn class_effective_text_style_uses_token_aware_declaration_boundaries() {
    let base = default_style();
    let style = super::class_effective_text_style(
        &base,
        r#"font-family:"a;b",sans-serif;font-size:20px !important"#,
    );

    assert_eq!(style.font_family.as_deref(), Some(r#""a;b",sans-serif"#));
    assert_eq!(style.font_size, 20.0);
}

#[test]
fn class_effective_text_style_applies_css_winners_against_the_inherited_base() {
    let base = default_style();
    let style = super::class_effective_text_style(
        &base,
        "font-family:Important !important;font-family:Later;\
         font-size:20px !important;font-size:2em;\
         font-weight:700 !important;font-weight:400;\
         font-style:italic !important;font-style:normal",
    );

    assert_eq!(style.font_family.as_deref(), Some("Important"));
    assert_eq!(style.font_size, 20.0);
    assert_eq!(style.font_weight.as_deref(), Some("700"));
    assert_eq!(style.font_style.as_deref(), Some("italic"));

    let relative = super::class_effective_text_style(&base, "font-size:2em;font-size:2em");
    assert_eq!(relative.font_size, base.font_size * 2.0);

    let same_priority = super::class_effective_text_style(
        &base,
        "font-family:First;font-family:Second;\
         font-size:2em;font-size:150%;\
         font-weight:400;font-weight:500;\
         font-style:italic;font-style:normal",
    );
    assert_eq!(same_priority.font_family.as_deref(), Some("Second"));
    assert_eq!(same_priority.font_size, base.font_size * 1.5);
    assert_eq!(same_priority.font_weight.as_deref(), Some("500"));
    assert_eq!(same_priority.font_style.as_deref(), Some("normal"));

    let invalid = super::class_effective_text_style(
        &base,
        "font-family:Valid;font-family:inherit junk !important;\
         font-size:20px;font-size:30px junk !important;\
         font-weight:700;font-weight:invalid !important;\
         font-style:italic;font-style:invalid !important",
    );
    assert_eq!(invalid.font_family.as_deref(), Some("Valid"));
    assert_eq!(invalid.font_size, 20.0);
    assert_eq!(invalid.font_weight.as_deref(), Some("700"));
    assert_eq!(invalid.font_style.as_deref(), Some("italic"));

    let inherited = super::class_effective_text_style(
        &base,
        "font-family:Other;font-family:unset;\
         font-size:20px;font-size:inherit;\
         font-weight:700;font-weight:unset;\
         font-style:italic;font-style:inherit",
    );
    assert!(matches!(inherited, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn class_html_row_relative_fonts_are_applied_once() {
    struct FontSizeMeasurer;
    impl TextMeasurer for FontSizeMeasurer {
        fn measure(&self, _text: &str, style: &TextStyle) -> merman_render_core::text::TextMetrics {
            merman_render_core::text::TextMetrics {
                width: style.font_size,
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
        ) -> merman_render_core::text::TextMetrics {
            self.measure(text, style)
        }
    }
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "classDiagram\nclass Vehicle {\n+owner\n+drive()\n}",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let mut node = model.class("Vehicle").expect("Vehicle class").node.clone();
    node.members[0].css_style = "font-size:2em".to_owned();
    node.methods[0].css_style = "font-size:150%".to_owned();
    let config = merman_model::presentation::PresentationConfig::default();
    let style = TextStyle {
        font_size: 24.0,
        ..default_style()
    };
    let context = super::ClassBoxMeasureCtx {
        measurer: &FontSizeMeasurer,
        mermaid_config: &config,
        math_renderer: None,
        text_style: &style,
        code_text_style: &style,
        native_title_style: None,
        html_calc_text_style: &style,
        wrap_probe_font_size: 16.0,
        wrap_mode: WrapMode::HtmlLike,
        padding: 12.0,
        hide_empty_members_box: false,
        capture_row_metrics: true,
    };
    let (_, _, plan, _) = super::class_box_dimensions(&node, &context);
    let Some(super::ClassNodeLabelPlan::PreparedHtml(labels)) = plan else {
        panic!("expected prepared HTML labels");
    };
    assert_eq!(labels.members[0].metrics.width, 48.0);
    assert_eq!(labels.members[0].metrics.height, 72.0);
    assert_eq!(labels.methods[0].metrics.width, 36.0);
    assert_eq!(labels.methods[0].metrics.height, 54.0);
}

#[test]
fn class_raw_code_and_anchor_metrics_measure_the_rendered_dom() {
    let measurer = DeterministicTextMeasurer::default();
    let style = default_style();
    let source = "<a href='https://example.com'><code>Entity</code></a>";
    let fragment = merman_render_core::text::mermaid_markdown_to_xhtml_label_fragment(source, true);
    let expected = merman_render_core::text::inline_html::measure_html_with_inline_styles(
        &measurer,
        &fragment,
        &style,
        Some(500.0),
        WrapMode::HtmlLike,
    );

    let actual = super::class_html_measure_label_metrics(&measurer, &style, source, 500, "");
    let literal = measurer.measure_wrapped(source, &style, Some(500.0), WrapMode::HtmlLike);

    assert_eq!(actual.width, expected.width);
    assert_eq!(actual.height, expected.height);
    assert_eq!(actual.line_count, expected.line_count);
    assert!(
        actual.width < literal.width,
        "actual={actual:?}, literal={literal:?}"
    );
}

#[test]
fn class_plain_underscore_label_preserves_host_precision() {
    let metrics = super::class_html_measure_label_metrics(
        &ClassPrecisionMeasurer,
        &default_style(),
        "driver_license",
        200,
        "",
    );

    assert_eq!(metrics.width, 73.123_456_789);
    assert_eq!(metrics.line_count, 1);
}

#[test]
fn class_svg_plain_underscore_uses_formatted_text_bbox_precision() {
    assert_eq!(
        super::ClassSvgLabel::new("driver_license")
            .single_line_plain_width(&ClassPrecisionMeasurer, &default_style()),
        Some(73.123_456_789)
    );
}

#[test]
fn class_svg_formatted_bbox_uses_semantic_markdown_analysis() {
    let style = default_style();
    for literal in [
        "driver_license",
        "*unclosed",
        "literal ` backtick",
        "plain title",
    ] {
        assert_eq!(
            super::ClassSvgLabel::new(literal)
                .single_line_plain_width(&ClassPrecisionMeasurer, &style),
            Some(73.123_456_789),
            "literal={literal:?}"
        );
    }

    for non_plain_single_line in ["*emphasis*", "__strong__", "first<br/>second"] {
        assert_eq!(
            super::ClassSvgLabel::new(non_plain_single_line)
                .single_line_plain_width(&ClassPrecisionMeasurer, &style),
            None,
            "label={non_plain_single_line:?}"
        );
    }
}

#[test]
fn class_svg_math_title_keeps_math_renderer_width_in_box_geometry() {
    let node: super::ClassNode = serde_json::from_value(serde_json::json!({
        "id": "Formula",
        "label": "Formula",
        "text": "$$x^2$$",
        "domId": "classId-Formula-0"
    }))
    .expect("Class node");
    let config = merman_model::presentation::PresentationConfig::default();
    let text_style = default_style();
    let context = super::ClassBoxMeasureCtx {
        measurer: &ClassMathGeometryMeasurer,
        mermaid_config: &config,
        math_renderer: Some(&ClassMathGeometryRenderer),
        text_style: &text_style,
        code_text_style: &text_style,
        native_title_style: None,
        html_calc_text_style: &text_style,
        wrap_probe_font_size: 10.0,
        wrap_mode: WrapMode::SvgLike,
        padding: 8.0,
        hide_empty_members_box: true,
        capture_row_metrics: false,
    };

    let (width, _, _, _) = super::class_box_dimensions(&node, &context);

    assert_eq!(width, 139.0);
}

#[test]
fn class_svg_math_note_keeps_math_renderer_width_in_note_geometry() {
    let config = merman_model::presentation::PresentationConfig::default();
    let style = default_style();

    let (width, height, metrics) = super::note_dimensions(
        "$$x^2$$",
        &ClassMathGeometryMeasurer,
        &style,
        WrapMode::SvgLike,
        8.0,
        Some(&config),
        Some(&ClassMathGeometryRenderer),
    );

    assert_eq!(metrics.width, 123.0);
    assert_eq!(metrics.height, 37.0);
    assert_eq!(width, 139.0);
    assert_eq!(height, 53.0);
}
