use super::{
    ClassElkLayoutSettings, ClassRenderConfig, MeasuredClassDiagram, class_elk_layout_options,
    class_layout_from_elk, class_measured_to_elk_graph, elk, measure_class_diagram,
    orient_class_elk_graph, validate_class_elk_output_ids,
};
use crate::class::ClassTextThemePlan;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use merman_render_core::text::create_text_html::FlowchartLabelMetricsRequest;
use merman_render_core::text::literal_label::{FlowchartSvgLabelSource, FlowchartSvgWidthMode};
use merman_render_core::text::{DeterministicTextMeasurer, TextMetrics, WrapMode};

// These fixtures use plain interface names. The facade's integration tests exercise its
// createText callback with the real HTML and SVG measurement contract.
fn measure_fixture_interface(request: FlowchartLabelMetricsRequest<'_>) -> TextMetrics {
    assert_eq!(request.label_type, "text");
    assert!(
        request
            .raw_label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
    );
    if request.wrap_mode == WrapMode::HtmlLike {
        request.measurer.measure_wrapped(
            request.raw_label,
            request.style,
            request.max_width_px,
            request.wrap_mode,
        )
    } else {
        FlowchartSvgLabelSource::new(request.raw_label).metrics(
            request.measurer,
            request.style,
            request.max_width_px,
            FlowchartSvgWidthMode::Bbox,
        )
    }
}

fn class_elk_graph_for_test(source: &str, algorithm: elk::Algorithm) -> elk::Graph {
    class_elk_measurements_for_test(source, algorithm).0
}

fn class_elk_measurements_for_test(
    source: &str,
    algorithm: elk::Algorithm,
) -> (elk::Graph, MeasuredClassDiagram) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Class source")
        .expect("detect Class source")
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let config = &parsed.metadata().effective_config;
    let typography_theme = ClassTextThemePlan::resolve(None, config);
    let render_config = ClassRenderConfig::resolve(config, &typography_theme);
    let settings = render_config.layout_settings();
    let measured = measure_class_diagram(
        model,
        config,
        &DeterministicTextMeasurer::default(),
        None,
        &render_config,
        true,
        measure_fixture_interface,
        None,
    )
    .expect("measure Class diagram");
    let graph = class_measured_to_elk_graph(
        &measured,
        &ClassElkLayoutSettings {
            algorithm,
            namespace_padding: settings.namespace_padding,
            title_margin_top: settings.title_margin_top,
            title_margin_bottom: settings.title_margin_bottom,
            effective_config: config.as_value(),
        },
        &mut None,
    )
    .expect("Class ELK graph");
    (graph, measured)
}

#[test]
fn class_elk_spreads_crowded_extension_markers_at_their_own_end() {
    for (third_arrow, height, expected) in [
        ("--|>", 48.0, Some(elk::PortAlignment::Center)),
        ("<|--", 48.0, None),
        ("--|>", 64.0, None),
    ] {
        let source =
            format!("classDiagram\ndirection LR\nA --|> Hub\nB --|> Hub\nC {third_arrow} Hub\n");
        let (mut input, measured) =
            class_elk_measurements_for_test(&source, elk::Algorithm::Layered);
        assert_eq!(input.direction, elk::Direction::Right);
        let hub = input
            .nodes
            .iter()
            .position(|node| node.id == "Hub")
            .expect("Hub");
        // Match the side lengths of the pinned ELK relation-marker witnesses.
        input.nodes[hub].height = height;
        let original = input.clone();
        let oriented =
            orient_class_elk_graph(&mut input, &measured, &serde_json::Value::Null, &mut None)
                .expect("prepare Class ELK ports");
        assert_eq!(
            oriented.graph().nodes[hub].port_alignment,
            expected,
            "{third_arrow} on a {height}px side"
        );
        drop(oriented);
        assert_eq!(input, original);
    }
}

#[test]
fn class_elk_interfaces_remain_root_nodes_outside_nested_namespaces() {
    for relation in ["A --() ProvidedInterface", "ProvidedInterface ()-- A"] {
        let graph = class_elk_graph_for_test(
            &format!(
                "classDiagram\nnamespace Outer {{\nnamespace Inner {{\nclass A\n}}\n}}\n{relation}\n"
            ),
            elk::Algorithm::Layered,
        );
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
        let graph = class_elk_graph_for_test(
            &format!(
                "---\nconfig:\n  {config}\n---\nclassDiagram\nnamespace Outer {{\nnamespace Inner {{\nclass A\n}}\n}}\n"
            ),
            elk::Algorithm::Layered,
        );
        for id in ["Outer", "Outer.Inner"] {
            let container = graph.nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(container.container.padding, expected, "{config}: {id}");
        }
        assert_eq!(graph.spacing.group_padding_x, expected, "{config}");
        assert_eq!(graph.spacing.group_padding_y, expected, "{config}");
    }
}

#[test]
fn class_elk_cycle_entry_follows_relations_independently_of_node_declarations() {
    for declarations in ["class B\nclass A\nclass C", "class C\nclass B\nclass A"] {
        for enabled in [false, true] {
            let graph = class_elk_graph_for_test(
                &format!(
                    "---\nconfig:\n  elk:\n    keepEntryNodeOnTop: {enabled}\n    cycleBreakingStrategy: GREEDY_MODEL_ORDER\n---\nclassDiagram\n{declarations}\nA --> B\nB --> C\nC --> A\n"
                ),
                elk::Algorithm::Layered,
            );
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
                let typography_theme = ClassTextThemePlan::resolve(None, config);
                let render_config = ClassRenderConfig::resolve(config, &typography_theme);
                let layout_settings = render_config.layout_settings();
                let measured = measure_class_diagram(
                    model,
                    config,
                    &DeterministicTextMeasurer::default(),
                    None,
                    &render_config,
                    true,
                    measure_fixture_interface,
                    None,
                )
                .unwrap();
                let (label_width, label_height) = measured.node_label_metrics_by_id["interface0"];
                let (padding_x, padding_y) = if look == "neo" {
                    (32.0, 24.0)
                } else {
                    (0.0, 0.0)
                };
                let settings = ClassElkLayoutSettings {
                    algorithm: elk::Algorithm::Layered,
                    namespace_padding: layout_settings.namespace_padding,
                    title_margin_top: layout_settings.title_margin_top,
                    title_margin_bottom: layout_settings.title_margin_bottom,
                    effective_config: config.as_value(),
                };
                let graph = class_measured_to_elk_graph(&measured, &settings, &mut None).unwrap();
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
                let layout =
                    class_layout_from_elk(model, measured, &graph, provider, settings, &mut None)
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
        let typography_theme = ClassTextThemePlan::resolve(None, config);
        let render_config = ClassRenderConfig::resolve(config, &typography_theme);
        let layout_settings = render_config.layout_settings();
        let mut measured = measure_class_diagram(
            model,
            config,
            &DeterministicTextMeasurer::default(),
            None,
            &render_config,
            true,
            measure_fixture_interface,
            None,
        )
        .unwrap();
        measured
            .namespace_label_metrics_by_id
            .get_mut("Frame")
            .unwrap()
            .0 = title_width;
        let settings = ClassElkLayoutSettings {
            algorithm: elk::Algorithm::Layered,
            namespace_padding: layout_settings.namespace_padding,
            title_margin_top: layout_settings.title_margin_top,
            title_margin_bottom: layout_settings.title_margin_bottom,
            effective_config: config.as_value(),
        };
        let graph = class_measured_to_elk_graph(&measured, &settings, &mut None).unwrap();
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
        let layout =
            class_layout_from_elk(model, measured, &graph, provider, settings, &mut None).unwrap();
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

#[test]
fn class_elk_terminal_provider_padding_only_reserves_real_markers() {
    use merman_layout_elk::TerminalLabelKey;
    let plain = class_elk_graph_for_test(
        "classDiagram\nA \"many\" -- \"many\" B\n",
        elk::Algorithm::Layered,
    );
    let marked = class_elk_graph_for_test(
        "classDiagram\nA \"many\" <|-- \"many\" B\n",
        elk::Algorithm::Layered,
    );
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
        elk::Algorithm::MrTree,
    );
    assert!(non_layered.edges[0].terminal_labels.is_empty());
}

#[test]
fn class_elk_keeps_the_source_default_nonzero_seed() {
    assert_eq!(
        class_elk_layout_options(&serde_json::Value::Null, elk::Algorithm::Layered)
            .layered
            .random_seed,
        1
    );
}

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
    let typography_theme = ClassTextThemePlan::resolve(None, config);
    let render_config = ClassRenderConfig::resolve(config, &typography_theme);
    let measured = measure_class_diagram(
        model,
        config,
        &DeterministicTextMeasurer::default(),
        None,
        &render_config,
        true,
        measure_fixture_interface,
        None,
    )
    .expect("measure Class diagram");
    let settings = ClassElkLayoutSettings {
        algorithm: elk::Algorithm::Layered,
        namespace_padding: 8.0,
        title_margin_top: 0.0,
        title_margin_bottom: 0.0,
        effective_config: &serde_json::Value::Null,
    };
    let mut graph =
        class_measured_to_elk_graph(&measured, &settings, &mut None).expect("Class ELK graph");
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

#[test]
fn class_elk_output_inventory_rejects_missing_duplicate_and_unknown_ids() {
    validate_class_elk_output_ids("node", ["A", "B"], ["A", "B"])
        .expect("matching ELK node inventory");

    for (observed, expected_message) in [
        (vec!["A"], "ELK layout omitted class node B"),
        (vec!["A", "A"], "ELK layout returned duplicate class node A"),
        (vec!["A", "C"], "ELK layout returned unknown class node C"),
    ] {
        let error = validate_class_elk_output_ids("node", ["A", "B"], observed)
            .expect_err("invalid ELK node inventory");
        assert_eq!(
            error.to_string(),
            format!("invalid semantic model: {expected_message}")
        );
    }
}
