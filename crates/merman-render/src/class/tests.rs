use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::class::Namespace;
use merman_model::presentation::RenderSemanticModel;

use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};

#[test]
fn class_html_and_fallback_plans_enforce_retained_limits_and_release_partial_preparation() {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
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
                    Some(&work),
                );
                (result, meter)
            };
            let policy = RenderResourcePolicy::unbounded_for_trusted_input();
            let (measured, meter) = measure(policy);
            let measured = measured.unwrap();
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
            let Some(crate::Error::ResourceLimitExceeded(limit)) = rejected.err() else {
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
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
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
    let Err(crate::Error::ResourceLimitExceeded(limit)) =
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
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
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
        Some(&work),
    );
    assert!(matches!(result, Err(crate::Error::Cancelled(_))));
    assert!(meter.prepared_text_retained_bytes_peak() > 0);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[cfg(feature = "layout-dagre")]
#[test]
fn class_html_family_releases_plans_after_svg_failure_or_cancellation() {
    use crate::environment::RenderEnvironment;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use std::sync::Arc;

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  htmlLabels: true\n---\nclassDiagram\nclass A[\"MMMM\"]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    for cancel in [false, true] {
        let control = merman_model::OperationControl::new();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                    .unwrap(),
            )
            .begin_session_with_control(control.clone())
            .unwrap();
        let meter = Arc::clone(session.work_meter());
        let artifact = crate::family::prepare(
            &parsed,
            &crate::LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap();
        assert!(meter.prepared_text_retained_bytes() > 0);
        if cancel {
            control.cancel();
        }
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Err(error) => error,
                Ok(_) => panic!("expected SVG failure or cancellation"),
            };
        if cancel {
            assert!(matches!(error, crate::Error::Cancelled(_)));
        } else {
            let crate::Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected the SVG output ceiling");
            };
            assert_eq!(limit.limit, "max_svg_bytes");
        }
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}

#[test]
fn class_svg_prepared_labels_own_retained_bytes_and_release_partial_preparation() {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

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
        let Some(crate::Error::ResourceLimitExceeded(limit)) = rejected.err() else {
            panic!("expected the prepared-text ceiling to reject partial Class preparation");
        };
        assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}

#[test]
fn class_prepared_reservation_observes_cancellation_before_its_byte_ceiling() {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

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
        Err(crate::Error::Cancelled(_))
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

#[cfg(feature = "layout-elk")]
fn class_elk_graph_for_test(source: &str) -> merman_layout_elk::Graph {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Class source")
        .expect("detect Class source")
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let config = &parsed.metadata().effective_config;
    let typography_theme = super::ClassTextThemePlan::resolve(None, config);
    let render_config = super::ClassRenderConfig::resolve(config, &typography_theme);
    let settings = render_config.layout_settings();
    let measured = super::measured::measure_class_diagram(
        model,
        config,
        &DeterministicTextMeasurer::default(),
        None,
        &render_config,
        true,
        None,
    )
    .expect("measure Class diagram");
    super::elk_layout::class_measured_to_elk_graph(
        &measured,
        &super::elk_layout::ClassElkLayoutSettings {
            algorithm: crate::layout_backend::ElkRootAlgorithm::for_diagnostic(config.as_value()),
            namespace_padding: settings.namespace_padding,
            title_margin_top: settings.title_margin_top,
            title_margin_bottom: settings.title_margin_bottom,
            effective_config: config.as_value(),
        },
        &mut None,
    )
    .expect("Class ELK graph")
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_interfaces_remain_root_nodes_outside_nested_namespaces() {
    for relation in ["A --() ProvidedInterface", "ProvidedInterface ()-- A"] {
        let graph = class_elk_graph_for_test(&format!(
            "classDiagram\nnamespace Outer {{\nnamespace Inner {{\nclass A\n}}\n}}\n{relation}\n"
        ));
        let node = |id: &str| graph.nodes.iter().find(|node| node.id == id).expect("node");
        assert_eq!(node("A").parent.as_deref(), Some("Outer.Inner"));
        assert_eq!(node("Outer.Inner").parent.as_deref(), Some("Outer"));
        assert_eq!(node("interface0").parent, None);
        assert!(graph.edges.iter().any(|edge| {
            (edge.source == "A" && edge.target == "interface0")
                || (edge.source == "interface0" && edge.target == "A")
        }));
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_namespace_padding_reaches_every_container_without_flowchart_fallback() {
    for (config, expected) in [
        ("flowchart:\n    padding: 83", 16.0),
        (
            "class:\n    padding: 23\n  flowchart:\n    padding: 83",
            23.0,
        ),
        ("class:\n    padding: 0\n  flowchart:\n    padding: 83", 0.0),
    ] {
        let graph = class_elk_graph_for_test(&format!(
            "---\nconfig:\n  {config}\n---\nclassDiagram\nnamespace Outer {{\nnamespace Inner {{\nclass A\n}}\n}}\n"
        ));
        for id in ["Outer", "Outer.Inner"] {
            let container = graph.nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(container.container.padding, expected, "{config}: {id}");
        }
        assert_eq!(graph.spacing.group_padding_x, expected, "{config}");
        assert_eq!(graph.spacing.group_padding_y, expected, "{config}");
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_cycle_entry_follows_relations_independently_of_node_declarations() {
    for declarations in ["class B\nclass A\nclass C", "class C\nclass B\nclass A"] {
        for enabled in [false, true] {
            let graph = class_elk_graph_for_test(&format!(
                "---\nconfig:\n  elk:\n    keepEntryNodeOnTop: {enabled}\n    cycleBreakingStrategy: GREEDY_MODEL_ORDER\n---\nclassDiagram\n{declarations}\nA --> B\nB --> C\nC --> A\n"
            ));
            let constrained = graph
                .nodes
                .iter()
                .filter(|node| {
                    node.layer_constraint == Some(merman_layout_elk::LayerConstraint::First)
                })
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(constrained, if enabled { vec!["A"] } else { vec![] });
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_interface_padding_preserves_measured_label_bounds_in_both_modes() {
    for look in ["classic", "neo"] {
        for html in [false, true] {
            for font_size in [14, 23] {
                let source = format!(
                    "---\nconfig:\n  look: {look}\n  htmlLabels: {html}\n  themeVariables:\n    fontSize: {font_size}px\n---\nclassDiagram\nA --() CompleteInterface\n"
                );
                let parsed = MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let RenderSemanticModel::Class(model) = parsed.model() else {
                    panic!("Class model")
                };
                let config = &parsed.metadata().effective_config;
                let typography_theme = super::ClassTextThemePlan::resolve(None, config);
                let render_config = super::ClassRenderConfig::resolve(config, &typography_theme);
                let layout_settings = render_config.layout_settings();
                let measured = super::measured::measure_class_diagram(
                    model,
                    config,
                    &DeterministicTextMeasurer::default(),
                    None,
                    &render_config,
                    true,
                    None,
                )
                .unwrap();
                let (label_width, label_height) = measured.node_label_metrics_by_id["interface0"];
                let (padding_x, padding_y) = if look == "neo" {
                    (32.0, 24.0)
                } else {
                    (0.0, 0.0)
                };
                let settings = super::elk_layout::ClassElkLayoutSettings {
                    algorithm: crate::layout_backend::ElkRootAlgorithm::for_diagnostic(
                        config.as_value(),
                    ),
                    namespace_padding: layout_settings.namespace_padding,
                    title_margin_top: layout_settings.title_margin_top,
                    title_margin_bottom: layout_settings.title_margin_bottom,
                    effective_config: config.as_value(),
                };
                let graph =
                    super::elk_layout::class_measured_to_elk_graph(&measured, &settings, &mut None)
                        .unwrap();
                let interface = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == "interface0")
                    .unwrap();
                assert!((interface.width - label_width - padding_x).abs() < 1e-6);
                assert!((interface.height - label_height - padding_y).abs() < 1e-6);
                let provider = merman_layout_elk::LayoutResult {
                    nodes: graph
                        .nodes
                        .iter()
                        .map(|node| merman_layout_elk::NodeLayout {
                            id: node.id.clone(),
                            x: 0.0,
                            y: 0.0,
                            width: node.width,
                            height: node.height,
                        })
                        .collect(),
                    edges: graph
                        .edges
                        .iter()
                        .map(|edge| merman_layout_elk::EdgeLayout {
                            id: edge.id.clone(),
                            points: vec![
                                merman_layout_elk::Point { x: 0.0, y: 0.0 },
                                merman_layout_elk::Point { x: 40.0, y: 40.0 },
                            ],
                            labels: Vec::new(),
                        })
                        .collect(),
                };
                let layout = super::elk_layout::class_layout_from_elk(
                    model, measured, &graph, provider, settings, &mut None,
                )
                .unwrap();
                let painted = layout
                    .nodes
                    .iter()
                    .find(|node| node.id == "interface0")
                    .unwrap();
                assert_eq!(painted.label_width, Some(label_width));
                assert_eq!(painted.label_height, Some(label_height));
                assert_eq!(
                    (painted.width, painted.height),
                    (interface.width, interface.height)
                );
            }
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_contracts_namespace_frame_without_rebasing_provider_routes() {
    for title_width in [32.0, 300.0] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nnamespace Frame {\nclass A\n}\nA --> A : loop\n",
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("Class model")
        };
        let config = &parsed.metadata().effective_config;
        let typography_theme = super::ClassTextThemePlan::resolve(None, config);
        let render_config = super::ClassRenderConfig::resolve(config, &typography_theme);
        let layout_settings = render_config.layout_settings();
        let mut measured = super::measured::measure_class_diagram(
            model,
            config,
            &DeterministicTextMeasurer::default(),
            None,
            &render_config,
            true,
            None,
        )
        .unwrap();
        measured
            .namespace_label_metrics_by_id
            .get_mut("Frame")
            .unwrap()
            .0 = title_width;
        let settings = super::elk_layout::ClassElkLayoutSettings {
            algorithm: crate::layout_backend::ElkRootAlgorithm::for_diagnostic(config.as_value()),
            namespace_padding: layout_settings.namespace_padding,
            title_margin_top: layout_settings.title_margin_top,
            title_margin_bottom: layout_settings.title_margin_bottom,
            effective_config: config.as_value(),
        };
        let graph = super::elk_layout::class_measured_to_elk_graph(&measured, &settings, &mut None)
            .unwrap();
        let provider = merman_layout_elk::LayoutResult {
            nodes: vec![
                merman_layout_elk::NodeLayout {
                    id: "Frame".to_string(),
                    x: 100.0,
                    y: 74.0,
                    width: 200.0,
                    height: 148.0,
                },
                merman_layout_elk::NodeLayout {
                    id: "A".to_string(),
                    x: 90.0,
                    y: 86.0,
                    width: 100.0,
                    height: 76.0,
                },
            ],
            edges: vec![merman_layout_elk::EdgeLayout {
                id: graph.edges[0].id.clone(),
                points: vec![
                    merman_layout_elk::Point { x: 40.0, y: 60.0 },
                    merman_layout_elk::Point { x: 140.0, y: 70.0 },
                ],
                labels: vec![merman_layout_elk::EdgeLabelLayout {
                    terminal: None,
                    x: 100.0,
                    y: 20.0,
                    width: 30.0,
                    height: 20.0,
                }],
            }],
        };
        let layout = super::elk_layout::class_layout_from_elk(
            model, measured, &graph, provider, settings, &mut None,
        )
        .unwrap();
        let frame = &layout.clusters[0];
        let expected = if title_width < 200.0 {
            (90.0, 74.0, 148.0, 148.0)
        } else {
            (100.0, 74.0, 200.0, 148.0)
        };
        assert_eq!((frame.x, frame.y, frame.width, frame.height), expected);
        let node = layout.nodes.iter().find(|node| node.id == "Frame").unwrap();
        assert_eq!((node.x, node.y, node.width, node.height), expected);
        assert_eq!(
            layout.edges[0]
                .points
                .iter()
                .map(|p| (p.x, p.y))
                .collect::<Vec<_>>(),
            [(40.0, 60.0), (140.0, 70.0)]
        );
        let label = layout.edges[0].label.as_ref().unwrap();
        assert_eq!((label.x, label.y), (115.0, 30.0));
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_terminal_provider_padding_only_reserves_real_markers() {
    use merman_layout_elk::TerminalLabelKey;
    let plain = class_elk_graph_for_test("classDiagram\nA \"many\" -- \"many\" B\n");
    let marked = class_elk_graph_for_test("classDiagram\nA \"many\" <|-- \"many\" B\n");
    assert_eq!(plain.edges[0].terminal_labels.len(), 2);
    for key in [TerminalLabelKey::StartRight, TerminalLabelKey::EndLeft] {
        let before = plain.edges[0]
            .terminal_labels
            .iter()
            .find(|label| label.key == key)
            .unwrap()
            .label;
        let after = marked.edges[0]
            .terminal_labels
            .iter()
            .find(|label| label.key == key)
            .unwrap()
            .label;
        let padding = if key.at_start() { 12.0 } else { 0.0 };
        assert!((after.width - before.width - padding).abs() < 1e-9);
        assert!((after.height - before.height - padding).abs() < 1e-9);
        assert_eq!(before.height, 16.5, "measured terminal CSS font is 11px");
    }
    let non_layered = class_elk_graph_for_test(
        "---\nconfig:\n  layout: elk.mrtree\n---\nclassDiagram\nA \"many\" --> \"one\" B\n",
    );
    assert!(non_layered.edges[0].terminal_labels.is_empty());
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_keeps_the_source_default_nonzero_seed() {
    assert_eq!(
        crate::elk_options::layout_options(
            &serde_json::Value::Null,
            crate::layout_backend::ElkRootAlgorithm::Layered
        )
        .layered
        .random_seed,
        1
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_zero_seed_adapter_graph_requires_an_operation_seed() {
    use std::num::NonZeroU64;

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("classDiagram\nA --> B", ParseOptions::default())
        .expect("parse Class source")
        .expect("detect Class source")
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let config = &parsed.metadata().effective_config;
    let typography_theme = super::ClassTextThemePlan::resolve(None, config);
    let render_config = super::ClassRenderConfig::resolve(config, &typography_theme);
    let measured = super::measured::measure_class_diagram(
        model,
        config,
        &DeterministicTextMeasurer::default(),
        None,
        &render_config,
        true,
        None,
    )
    .expect("measure Class diagram");
    let settings = super::elk_layout::ClassElkLayoutSettings {
        algorithm: crate::layout_backend::ElkRootAlgorithm::Layered,
        namespace_padding: 8.0,
        title_margin_top: 0.0,
        title_margin_bottom: 0.0,
        effective_config: &serde_json::Value::Null,
    };
    let mut graph = super::elk_layout::class_measured_to_elk_graph(&measured, &settings, &mut None)
        .expect("Class ELK graph");
    graph.options.layered.random_seed = 0;

    assert!(merman_layout_elk::layout(&graph).is_err());

    let operation_seed = merman_layout_elk::ElkOperationSeed::from_operation_seed(
        NonZeroU64::new(0x636c_6173_7365_6c6b).expect("nonzero operation seed"),
    );
    let first = merman_layout_elk::layout_with_operation_seed(&graph, operation_seed)
        .expect("seeded Class layout");
    let replayed = merman_layout_elk::layout_with_operation_seed(&graph, operation_seed)
        .expect("replayed seeded Class layout");

    assert_eq!(first, replayed);
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

impl crate::math::MathRenderer for ClassMathGeometryRenderer {
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

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_output_inventory_rejects_missing_duplicate_and_unknown_ids() {
    super::elk_layout::validate_class_elk_output_ids("node", ["A", "B"], ["A", "B"])
        .expect("matching ELK node inventory");

    for (observed, expected_message) in [
        (vec!["A"], "ELK layout omitted class node B"),
        (vec!["A", "A"], "ELK layout returned duplicate class node A"),
        (vec!["A", "C"], "ELK layout returned unknown class node C"),
    ] {
        let error = super::elk_layout::validate_class_elk_output_ids("node", ["A", "B"], observed)
            .expect_err("invalid ELK node inventory");
        assert_eq!(
            error.to_string(),
            format!("invalid semantic model: {expected_message}")
        );
    }
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
        fn measure(&self, _text: &str, style: &TextStyle) -> crate::text::TextMetrics {
            crate::text::TextMetrics {
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
        ) -> crate::text::TextMetrics {
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
    let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(source, true);
    let expected = crate::text::measure_html_with_inline_styles(
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
