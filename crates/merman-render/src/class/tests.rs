use merman_core::{MermaidEngine, ParseOptions};
#[cfg(feature = "layout-elk")]
use merman_model::presentation::RenderSemanticModel;

#[cfg(feature = "layout-elk")]
use crate::text::DeterministicTextMeasurer;

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
