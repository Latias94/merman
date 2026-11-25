use super::*;
use crate::resources::RenderResourcePolicy;
#[cfg(feature = "diagram-flowchart")]
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};
#[cfg(feature = "diagram-flowchart")]
use merman_model::presentation::RenderSemanticModel;
use merman_model::{CancelReason, OperationControl, OperationPhase};
use merman_render_flow::dagre::{
    charge_edge_endpoint_insertions, copy_cluster, extract_descendants,
    flowchart_find_non_cluster_child, remove_node_work_upper_bound,
};
use std::sync::mpsc;
use std::time::Duration;

const DEEP_SUBGRAPH_DEPTH: usize = 10_000;
#[cfg(feature = "diagram-flowchart")]
const NON_LATTICE_COMPUTED_LENGTH_PX: f64 = 73.123_456_789;

#[cfg(feature = "diagram-flowchart")]
struct NonLatticeComputedLengthMeasurer;

#[cfg(feature = "diagram-flowchart")]
impl TextMeasurer for NonLatticeComputedLengthMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> crate::text::TextMetrics {
        crate::text::TextMetrics {
            width: 40.0,
            height: 18.0,
            line_count: 1,
        }
    }

    fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        NON_LATTICE_COMPUTED_LENGTH_PX
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_wrapped_labels_fit_rectangle_and_diamond_nodes() {
    let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                r#"%%{init: {"flowchart": {"wrappingWidth": 120}}}%%
flowchart TD
    A["Receive a new deployment request from the release pipeline"] --> B{"Did every required validation and security check pass successfully?"}
    B -->|Yes| C["Publish the approved application version to the production environment"]
    B -->|No| D["Return the detailed validation errors to the requesting developer"]
"#,
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let measurer = crate::text::DeterministicTextMeasurer::default()
        .with_width_callback(|text, _| text.chars().count() as f64 * 10.0);
    let layout = layout_flowchart_typed(
        model,
        &parsed.metadata().effective_config,
        measurer.as_ref(),
        None,
    )
    .expect("layout ok");

    for id in ["A", "C", "D"] {
        let node = layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("node {id}"));
        let label_width = node.label_width.expect("rectangle label width");
        let label_height = node.label_height.expect("rectangle label height");
        assert!(
            label_height > 24.0,
            "node {id} label did not wrap: {node:?}"
        );
        assert!(label_width < node.width, "node {id}: {node:?}");
        assert!(label_height < node.height, "node {id}: {node:?}");
    }

    let diamond = layout
        .nodes
        .iter()
        .find(|node| node.id == "B")
        .expect("diamond node B");
    let label_width = diamond.label_width.expect("diamond label width");
    let label_height = diamond.label_height.expect("diamond label height");
    assert!(
        label_height > 24.0,
        "diamond label did not wrap: {diamond:?}"
    );
    assert_eq!(diamond.width, diamond.height);
    assert!(
        label_width + label_height < diamond.width,
        "diamond label does not fit inside its sloped sides: {diamond:?}"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_preserves_operation_computed_length_precision() {
    // Isolate host precision from Mermaid 12's minimum label width.
    let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"htmlLabels\": false, \"flowchart\": {\"htmlLabels\": false, \"minNodeWidth\": 0}}}%%\nflowchart TB\nA[alpha]\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let layout = layout_flowchart_typed(
        model,
        &parsed.metadata().effective_config,
        &NonLatticeComputedLengthMeasurer,
        None,
    )
    .expect("layout ok");
    let node = layout
        .nodes
        .iter()
        .find(|node| node.id == "A")
        .expect("node A");

    assert_eq!(node.label_width, Some(NON_LATTICE_COMPUTED_LENGTH_PX));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_nonempty_subgraph_title_reuses_its_prepared_layout_measurement() {
    let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nsubgraph S[service title]\n  A\nend\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let labels = FlowchartRenderContext::default();
    let builder = FlowchartSvgLabelSidecarBuilder::default();
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("render session");
    let measurer = session.text_measurer(crate::environment::TextMeasurementPhase::Layout);
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
        model,
        &parsed.metadata().effective_config,
        false,
        &meter,
    )
    .expect("prepare edge styles");
    let mut work_control = OperationLayoutWorkControl::new(meter);

    layout_flowchart_with_model(
        model,
        &labels,
        &super::super::FlowchartClassDefinitions::new(&model.class_defs),
        &parsed.metadata().effective_config,
        &measurer,
        None,
        Some(&builder),
        &edge_style_plan,
        &mut work_control,
    )
    .expect("layout ok");

    assert!(builder.prepared_count() > 0);
    assert!(
        builder.prepared_hit_count() > 0,
        "the cluster rect/title stages must reuse the same built-in measurement"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_html_nbsp_only_edge_labels_respect_source_provenance() {
    let nbsp = '\u{00A0}';
    let source = format!("flowchart LR\nA -- \"&nbsp;\" --> B\nC -- \"{nbsp}\" --> D\n");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let layout = layout_flowchart_typed(
        model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
    )
    .expect("layout ok");

    assert_eq!(layout.edges.len(), 2);
    let entity_label = layout
        .edges
        .iter()
        .find(|edge| edge.from == "A")
        .and_then(|edge| edge.label.as_ref())
        .expect("entity-authored NBSP label must reach Dagre");
    assert!(entity_label.width > 0.0, "{entity_label:?}");
    assert!(entity_label.height > 0.0, "{entity_label:?}");

    let direct_label = layout
        .edges
        .iter()
        .find(|edge| edge.from == "C")
        .and_then(|edge| edge.label.as_ref());
    assert!(direct_label.is_none(), "{direct_label:?}");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_operation_meter_honors_below_equal_and_above_boundaries() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TB\nsubgraph Outer\nsubgraph Inner\nA-->B\nend\nB-->C\nend\nC-->C\n",
            ParseOptions::default(),
        )
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let config = &parsed.metadata().effective_config;
    let measurer = crate::text::DeterministicTextMeasurer::default();

    let unbounded_meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let expected = layout_flowchart_typed_with_work_meter(
        model,
        config,
        &measurer,
        None,
        Arc::clone(&unbounded_meter),
    )
    .expect("unbounded Dagre layout succeeds");
    let exact = unbounded_meter.used();
    assert!(exact > 1);

    let below_meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .unwrap(),
    ));
    let error = layout_flowchart_typed_with_work_meter(
        model,
        config,
        &measurer,
        None,
        Arc::clone(&below_meter),
    )
    .expect_err("one unit below the exact Dagre work must fail");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(limit.actual, exact);
    assert_eq!(limit.max, exact - 1);
    assert!(below_meter.used() < exact);

    for limit in [exact, exact + 1] {
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        ));
        let actual = layout_flowchart_typed_with_work_meter(
            model,
            config,
            &measurer,
            None,
            Arc::clone(&meter),
        )
        .expect("equal and above Dagre work budgets succeed");
        assert_eq!(meter.used(), exact);
        assert_eq!(
            actual.dom_node_order_by_root,
            expected.dom_node_order_by_root
        );
        assert_eq!(actual.uses_elk_adapter_dom, expected.uses_elk_adapter_dom);
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
}

#[test]
fn dagre_adapter_rejects_before_a_tombstoned_edge_snapshot() {
    let mut graph = compound_graph();
    for id in ["a", "b", "c", "d"] {
        graph.set_node(id, NodeLabel::default());
    }
    for (from, to) in [("a", "b"), ("b", "c"), ("c", "d"), ("d", "a")] {
        graph.set_edge(from, to);
    }
    assert!(graph.remove_edge("b", "c", None));

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();
    let before_edge_snapshot = graph.node_order_slot_count() + node_count + node_count;
    let edge_snapshot_work = graph.edge_slot_count() + edge_count;
    assert!(
        graph.edge_slot_count() > edge_count,
        "the removed non-tail edge must leave a tombstone"
    );
    let attempted_after_edge_snapshot = before_edge_snapshot + edge_snapshot_work;
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                crate::ResourceLimitId::MaxLayoutWorkUnits,
                attempted_after_edge_snapshot - 1,
            )
            .unwrap(),
    ));
    let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));

    let error = adjust_flowchart_clusters_and_edges(&mut graph, &mut work_control)
        .expect_err("the complete edge snapshot must be admitted before it is cloned");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(meter.used(), before_edge_snapshot);
    assert_eq!(limit.actual, attempted_after_edge_snapshot);
    assert_eq!(limit.max, attempted_after_edge_snapshot - 1);

    let sticky = work_control
        .charge_adapter(1)
        .expect_err("a rejected owner tranche must remain sticky");
    let Error::ResourceLimitExceeded(sticky) = sticky else {
        panic!("expected sticky ResourceLimitExceeded");
    };
    assert_eq!(sticky.actual, attempted_after_edge_snapshot);
    assert_eq!(meter.used(), before_edge_snapshot);
}

#[test]
fn dagre_cluster_scans_charge_removed_edge_slots_on_each_pass() {
    fn adjust_cluster_with_tombstones(tombstones: bool) -> usize {
        let mut graph = compound_graph();
        for id in ["cluster", "inside", "outside"] {
            graph.set_node(id, NodeLabel::default());
        }
        graph.set_parent("inside", "cluster");
        if tombstones {
            graph.set_edge("inside", "outside");
        }
        graph.set_edge("inside", "inside");
        if tombstones {
            assert!(graph.remove_edge("inside", "outside", None));
            assert_eq!(graph.edge_slot_count(), graph.edge_count() + 1);
        }

        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let mut control = OperationLayoutWorkControl::new(Arc::clone(&meter));
        let external = adjust_flowchart_clusters_and_edges(&mut graph, &mut control).unwrap();
        assert_eq!(external.get("cluster"), Some(&false));
        meter.used()
    }

    // The external-connection scan, the outbound scan, and the rewrite snapshot each visit it.
    let dense = adjust_cluster_with_tombstones(false);
    assert_eq!(adjust_cluster_with_tombstones(true), dense + 3);
}

#[test]
fn dagre_adapter_checked_work_overflow_maps_to_the_resource_contract() {
    let mut work_control = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )));
    let error = work_control.checked_mul(usize::MAX, 2).unwrap_err();
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(limit.limit, "max_layout_work_units");
    let Error::ResourceLimitExceeded(sticky) = work_control
        .charge_adapter(1)
        .expect_err("arithmetic overflow must remain sticky")
    else {
        panic!("expected sticky ResourceLimitExceeded");
    };
    assert_eq!(sticky, limit);
}

#[test]
fn dagre_kernel_rejection_remains_sticky_after_error_mapping() {
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap(),
    ));
    let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));

    let interrupted = dugong::WorkControl::charge(&mut work_control, 2)
        .expect_err("the kernel charge must exceed the operation budget");
    let Error::ResourceLimitExceeded(mapped) = work_control.map_dugong_error(interrupted) else {
        panic!("expected ResourceLimitExceeded");
    };
    let Error::ResourceLimitExceeded(sticky) = work_control
        .charge_adapter(1)
        .expect_err("mapping the kernel error must not consume its sticky rejection")
    else {
        panic!("expected sticky ResourceLimitExceeded");
    };

    assert_eq!(sticky, mapped);
    assert_eq!(meter.used(), 0);
}

#[test]
fn dagre_kernel_interruption_preserves_operation_cancellation() {
    let control = OperationControl::new();
    control.cancel();
    let meter = Arc::new(OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        control,
    ));
    let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));

    let interrupted = dugong::WorkControl::charge(&mut work_control, 1)
        .expect_err("the cancelled operation must interrupt the kernel");
    let Error::Cancelled(cancelled) = work_control.map_dugong_error(interrupted) else {
        panic!("expected structured operation cancellation");
    };

    assert_eq!(cancelled.phase, OperationPhase::Layout);
    assert_eq!(cancelled.reason, CancelReason::Requested);
    assert_eq!(meter.used(), 0);
}

#[test]
fn dagre_kernel_invalid_tree_maps_to_invalid_model() {
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));

    let Error::InvalidModel { message } =
        work_control.map_dugong_error(dugong::LayoutError::InvalidNetworkSimplexTree)
    else {
        panic!("expected InvalidModel");
    };

    assert_eq!(
        message,
        "network simplex encountered an invalid mutable tree state"
    );
    assert_eq!(meter.used(), 0);
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn dagre_kernel_arithmetic_overflow_maps_to_the_resource_contract() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TB\nA-->B\n", ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let mut model = model.clone();
    model.edges[0].length = usize::MAX;

    let error = layout_flowchart_typed_with_work_meter(
        &model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
        Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )),
    )
    .expect_err("Dugong arithmetic overflow must fail closed");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(limit.limit, "max_layout_work_units");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn unknown_shape_is_rejected_instead_of_becoming_a_rectangle() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TB\nA[known]\n", ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let mut model = model.clone();
    model.nodes[0].layout_shape = Some("definitely-unknown".to_string());
    let error = layout_flowchart_typed(
        &model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
    )
    .expect_err("unknown Flowchart shapes must not silently become rectangles");
    let Error::InvalidModel { message } = error else {
        panic!("expected InvalidModel for unknown Flowchart shape");
    };
    assert_eq!(
        message,
        "No such shape: definitely-unknown. Please check your syntax."
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shape_validation_scan_is_admitted_before_invalid_model_reporting() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart TB\nA[known]\nB[also known]\n",
            ParseOptions::default(),
        )
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    let mut model = model.clone();
    model.nodes[1].layout_shape = Some("definitely-unknown".to_string());

    let error = layout_flowchart_typed_with_work_meter(
        &model,
        &parsed.metadata().effective_config,
        &crate::text::DeterministicTextMeasurer::default(),
        None,
        Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap(),
        )),
    )
    .expect_err("the node scan must be rejected before inspecting an invalid shape");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded");
    };
    assert_eq!(limit.actual, 2);
    assert_eq!(limit.max, 1);
}

#[derive(Debug)]
struct DeepTraversalOutcome {
    descendant_count: usize,
    anchor: Option<String>,
    root_dir: Option<String>,
    child_dir: Option<String>,
    copied_node_count: usize,
}

fn compound_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        directed: true,
    })
}

#[test]
fn dagre_cluster_copy_keeps_edge_owner_and_extras() {
    let mut graph = compound_graph();
    for id in ["root", "a", "b", "outside"] {
        graph.set_node(id, NodeLabel::default());
    }
    for child in ["a", "b"] {
        graph.set_parent(child, "root");
    }
    let edges = [
        ("a", "b", "inside", "a", "b", 0),
        ("a", "outside", "leaving", "root", "outside", 1),
        ("outside", "b", "entering", "outside", "root", 2),
    ];
    let mut labels = Vec::new();
    for (from, to, name, _, _, owner_index) in edges {
        let mut label = EdgeLabel {
            owner_index: Some(owner_index),
            ..Default::default()
        };
        label
            .extras
            .insert("caller".into(), serde_json::json!({ "ends": [from, to] }));
        graph.set_edge_named(from, to, Some(name), Some(label.clone()));
        labels.push(label);
    }
    let mut copied = compound_graph();
    let mut work_control = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )));
    let descendants = HashMap::from([("root".into(), vec!["a".into(), "b".into()])]);
    copy_cluster(
        "root",
        &mut graph,
        &mut copied,
        "root",
        &descendants,
        &mut work_control,
    )
    .unwrap();

    for ((_, _, name, from, to, _), label) in edges.into_iter().zip(&labels) {
        let graph = if name == "inside" { &copied } else { &graph };
        assert_eq!(graph.edge(from, to, Some(name)), Some(label));
    }
}

#[test]
fn dagre_cluster_copy_keeps_first_insertions_and_collision_fallback() {
    let mut graph = compound_graph();
    for id in ["root", "first", "gap", "late", "tail", "outside"] {
        graph.set_node(id, NodeLabel::default());
    }
    for child in ["first", "gap", "late", "tail"] {
        graph.set_parent(child, "root");
    }
    let edges = [
        ("late", "tail", "internal", 0),
        ("root", "outside", "same", 1),
        ("late", "outside", "same", 2),
        ("first", "outside", "same", 3),
        ("late", "outside", "parallel", 4),
    ];
    for (from, to, name, owner_index) in edges {
        let label = EdgeLabel {
            owner_index: Some(owner_index),
            ..Default::default()
        };
        graph.set_edge_named(from, to, Some(name), Some(label));
    }

    let mut copied = compound_graph();
    let mut work_control = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )));
    let descendants = extract_descendants("root", &graph, &mut work_control).unwrap();
    copy_cluster(
        "root",
        &mut graph,
        &mut copied,
        "root",
        &HashMap::from([("root".into(), descendants)]),
        &mut work_control,
    )
    .unwrap();

    assert_eq!(copied.node_ids(), ["first", "late", "tail", "gap"]);
    let rewritten = graph
        .edges()
        .map(|key| {
            assert_eq!((key.v.as_str(), key.w.as_str()), ("root", "outside"));
            (
                key.name.as_deref(),
                graph.edge_by_key(key).unwrap().owner_index,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rewritten,
        [(Some("same"), Some(2)), (Some("parallel"), Some(4))]
    );
}

#[cfg(target_pointer_width = "64")]
#[test]
fn dagre_edge_owner_index_overflow_is_a_sticky_layout_resource_error() {
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));
    let max = u32::MAX as usize;
    assert_eq!(
        checked_flowchart_edge_owner_index(0, &work_control).unwrap(),
        0
    );
    assert_eq!(
        checked_flowchart_edge_owner_index(max, &work_control).unwrap(),
        u32::MAX
    );
    let error = checked_flowchart_edge_owner_index(max + 1, &work_control).unwrap_err();
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ResourceLimitExceeded, got {error}");
    };
    assert_eq!(limit.limit, "max_layout_work_units");
    let Err(Error::ResourceLimitExceeded(sticky)) = work_control.charge_adapter(1) else {
        panic!("expected the same terminal layout resource error");
    };
    assert_eq!(sticky, limit);
    assert_eq!(meter.used(), 0);
}

#[cfg(target_pointer_width = "64")]
#[test]
fn dagre_edge_owner_index_overflow_preserves_cancellation() {
    let control = OperationControl::new();
    control.cancel();
    let meter = Arc::new(OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        control,
    ));
    let work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));
    let error =
        checked_flowchart_edge_owner_index(u32::MAX as usize + 1, &work_control).unwrap_err();
    let Error::Cancelled(cancelled) = error else {
        panic!("expected structured operation cancellation, got {error}");
    };
    assert_eq!(cancelled.phase, OperationPhase::Layout);
    assert_eq!(cancelled.reason, CancelReason::Requested);
    assert_eq!(meter.used(), 0);
}

#[test]
fn numeric_node_insertion_ordering_is_admitted_before_mutation() {
    let graph = compound_graph();
    let exact = 2usize;
    let below_meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .unwrap(),
    ));
    let mut below = OperationLayoutWorkControl::new(Arc::clone(&below_meter));
    let error = charge_node_insertion_ordering(&graph, "1", &mut below)
        .expect_err("the numeric node's two ordered-map insertions require admission");
    assert!(matches!(error, Error::ResourceLimitExceeded(_)));
    assert_eq!(below_meter.used(), 0);
    assert!(!graph.has_node("1"));

    for limit in [exact, exact + 1] {
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        ));
        let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));
        charge_node_insertion_ordering(&graph, "1", &mut work_control)
            .expect("equal and above numeric insertion budgets succeed");
        assert_eq!(meter.used(), exact);
        assert!(!graph.has_node("1"));
    }
}

#[test]
fn implicit_numeric_edge_endpoints_are_admitted_before_mutation() {
    let graph = compound_graph();
    let exact = 10usize;
    let below_meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .unwrap(),
    ));
    let mut below = OperationLayoutWorkControl::new(Arc::clone(&below_meter));
    let error = charge_edge_endpoint_insertions(&graph, "1", "2", &mut below)
        .expect_err("both missing numeric endpoints require admission before set_edge");
    assert!(matches!(error, Error::ResourceLimitExceeded(_)));
    assert_eq!(below_meter.used(), 0);
    assert_eq!(graph.node_count(), 0);

    for limit in [exact, exact + 1] {
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        ));
        let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&meter));
        charge_edge_endpoint_insertions(&graph, "1", "2", &mut work_control)
            .expect("equal and above endpoint budgets succeed");
        assert_eq!(meter.used(), exact);
        assert_eq!(graph.node_count(), 0);
    }
}

#[test]
fn child_snapshot_work_includes_ordinary_bucket_tombstones() {
    let mut graph = compound_graph();
    for id in ["parent", "other", "a", "b", "c"] {
        graph.set_node(id, NodeLabel::default());
    }
    for child in ["a", "b", "c"] {
        graph.set_parent_ref(child, "parent");
    }
    graph.set_parent_ref("b", "other");

    let work_control = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )));
    assert_eq!(graph.child_count("parent"), 2);
    assert_eq!(graph.child_order_slot_count("parent"), 3);
    assert_eq!(
        child_id_snapshot_work_upper_bound(&graph, "parent", &work_control).unwrap(),
        5
    );
}

#[test]
fn remove_node_numeric_child_promotion_curve_charges_the_logarithmic_term() {
    for width in (0..=10).map(|shift| 1usize << shift) {
        let mut numeric = compound_graph();
        let mut ordinary = compound_graph();
        numeric.set_node("parent", NodeLabel::default());
        ordinary.set_node("parent", NodeLabel::default());
        for index in 0..width {
            let numeric_id = index.to_string();
            numeric.set_node(&numeric_id, NodeLabel::default());
            numeric.set_parent_ref(&numeric_id, "parent");

            let ordinary_id = format!("child-{index}");
            ordinary.set_node(&ordinary_id, NodeLabel::default());
            ordinary.set_parent_ref(&ordinary_id, "parent");
        }

        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let work_control = OperationLayoutWorkControl::new(meter);
        let numeric_bound =
            remove_node_work_upper_bound(&numeric, "parent", &work_control).unwrap();
        let ordinary_bound =
            remove_node_work_upper_bound(&ordinary, "parent", &work_control).unwrap();
        let ordered_term =
            ordered_key_update_work_upper_bound(width, width, &work_control).unwrap();
        // Ordinary-key promotion leaves a root tombstone to scan, while array-index
        // promotion removes from the B-tree without that physical slot. Correct for the
        // representation difference before asserting that the logarithmic update term is
        // present in the numeric bound.
        let root_tombstone_savings = ordinary
            .root_child_order_slot_count()
            .saturating_sub(numeric.root_child_order_slot_count());
        assert_eq!(
            numeric_bound + root_tombstone_savings,
            ordinary_bound + ordered_term
        );
    }
}

#[test]
fn flowchart_third_self_loop_segment_uses_mermaid_11_16_graph_key() {
    let edge = FlowEdge {
        id: "A-cyclic-special-2".to_string(),
        from: "A---A---2".to_string(),
        to: "A".to_string(),
        label: Some(String::new()),
        label_type: None,
        edge_type: None,
        arrow: "-->".to_string(),
        start_marker: FlowEdgeMarker::None,
        end_marker: FlowEdgeMarker::Point,
        is_user_defined_id: false,
        stroke: None,
        stroke_kind: FlowEdgeStroke::Normal,
        visibility: FlowEdgeVisibility::Visible,
        interpolate: None,
        classes: Vec::new(),
        style: Vec::new(),
        animate: None,
        animation: None,
        step: None,
        length: 1,
    };
    let meta = FlowchartSelfLoopSegmentMeta {
        owner_index: 0,
        logical_edge_id: "L_A_A_0".to_string(),
        node_id: "A".to_string(),
        order: 2,
    };

    assert_eq!(
        flowchart_layout_edge_key(&edge, Some(&meta)),
        "A-cyclic-special-2"
    );
}

fn deep_compound_graph(depth: usize) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut graph = compound_graph();
    for i in (0..depth).rev() {
        graph.set_parent(format!("n{}", i + 1), format!("n{i}"));
    }
    graph
}

fn deep_subgraphs(depth: usize) -> Vec<FlowSubgraph> {
    let mut subgraphs = Vec::with_capacity(depth);
    for i in 0..depth {
        subgraphs.push(FlowSubgraph {
            metadata: None,
            id: format!("n{i}"),
            title: format!("n{i}"),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec![format!("n{}", i + 1)],
        });
    }
    subgraphs
}

#[test]
fn parent_batch_handles_a_long_acyclic_chain_without_parent_walks() {
    let count = 8_192usize;
    let mut graph = compound_graph();
    graph.set_node("root", NodeLabel::default());
    for index in 0..count {
        graph.set_node(format!("n{index}"), NodeLabel::default());
    }
    let root_ix = graph.node_ix("root").unwrap();
    let assignments = (0..count)
        .map(|index| {
            let child_ix = graph.node_ix(&format!("n{index}")).unwrap();
            let parent_ix = if index == 0 {
                root_ix
            } else {
                graph.node_ix(&format!("n{}", index - 1)).unwrap()
            };
            (child_ix, parent_ix)
        })
        .collect::<Vec<_>>();
    let mut work_control = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )));

    apply_unparented_parent_assignments(
        &mut graph,
        &assignments,
        UnparentedParentBatchShape::default(),
        &mut work_control,
    )
    .expect("the deep functional-parent chain is acyclic");
    assert_eq!(graph.parent("n0"), Some("root"));
    assert_eq!(graph.parent(&format!("n{}", count - 1)), Some("n8190"));
}

#[test]
fn parent_batch_matches_mermaid_graphlib_assignment_order() {
    // Mermaid applies the final FlowDB parents through Graphlib `setParent` in this order.
    // The atomic Graphlib batch must identify the same offending assignment so the
    // source-backed error names the same child and parent without a duplicate preflight.
    fn sequential_parent_walk_reference(
        order: &[&str],
        parent_by_id: &HashMap<String, String>,
    ) -> Option<(String, String)> {
        let mut assigned_parent: HashMap<String, String> = HashMap::new();
        for &child in order {
            let Some(parent) = parent_by_id.get(child) else {
                continue;
            };
            let mut current = Some(parent.as_str());
            while let Some(id) = current {
                if id == child {
                    return Some((child.to_string(), parent.clone()));
                }
                current = assigned_parent.get(id).map(String::as_str);
            }
            assigned_parent.insert(child.to_string(), parent.clone());
        }
        None
    }

    let cases = [
        (
            [("a", "root"), ("b", "root"), ("c", "a"), ("d", "a")],
            vec!["a", "b", "c", "d"],
        ),
        (
            [("a", "b"), ("b", "c"), ("c", "a"), ("d", "root")],
            vec!["a", "b", "c", "d"],
        ),
        (
            [("a", "b"), ("b", "c"), ("c", "a"), ("d", "root")],
            vec!["b", "c", "a", "d"],
        ),
        (
            [("a", "b"), ("b", "a"), ("c", "b"), ("d", "root")],
            vec!["c", "a", "b", "d"],
        ),
        (
            [("a", "a"), ("b", "root"), ("c", "b"), ("d", "c")],
            vec!["b", "c", "a", "d"],
        ),
        (
            [("a", "b"), ("b", "c"), ("c", "a"), ("d", "root")],
            vec!["a", "a", "b", "c", "d"],
        ),
    ];

    for (assignments, order) in cases {
        let parent_by_id = assignments
            .into_iter()
            .map(|(child, parent)| (child.to_string(), parent.to_string()))
            .collect::<HashMap<_, _>>();
        let expected = sequential_parent_walk_reference(&order, &parent_by_id);
        let mut graph = compound_graph();
        for id in order
            .iter()
            .copied()
            .chain(parent_by_id.values().map(String::as_str))
        {
            graph.set_node(id, NodeLabel::default());
        }
        let mut assigned = HashSet::new();
        let parent_assignments = order
            .iter()
            .copied()
            .filter_map(|child| {
                let parent = parent_by_id.get(child)?;
                assigned.insert(child).then(|| {
                    (
                        graph.node_ix(child).unwrap(),
                        graph.node_ix(parent).unwrap(),
                    )
                })
            })
            .collect::<Vec<_>>();
        let mut work_control = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )));
        let actual = apply_unparented_parent_assignments(
            &mut graph,
            &parent_assignments,
            UnparentedParentBatchShape::default(),
            &mut work_control,
        );

        match expected {
            None => actual.expect("the reference parent sequence is acyclic"),
            Some((child, parent)) => {
                let Error::InvalidModel { message } = actual.unwrap_err() else {
                    panic!("expected InvalidModel for order {order:?}");
                };
                assert_eq!(
                    message,
                    format!("Setting {parent} as parent of {child} would create a cycle"),
                    "cycle assignment mismatch for order {order:?}"
                );
            }
        }
    }
}

#[test]
fn parent_batch_adapter_charges_the_documented_width_and_depth_bound() {
    for size in (0..=10).map(|shift| 1usize << shift) {
        let mut width_graph = compound_graph();
        width_graph.set_node("root", NodeLabel::default());
        for index in 0..size {
            width_graph.set_node(format!("child-{index}"), NodeLabel::default());
        }
        let root_ix = width_graph.node_ix("root").unwrap();
        let width_assignments = (0..size)
            .map(|index| {
                (
                    width_graph.node_ix(&format!("child-{index}")).unwrap(),
                    root_ix,
                )
            })
            .collect::<Vec<_>>();
        let width_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let mut width_control = OperationLayoutWorkControl::new(Arc::clone(&width_meter));
        let expected_width_work = unparented_parent_batch_work_upper_bound(
            width_graph.node_slot_count(),
            0,
            width_assignments.len(),
            0,
            &width_control,
        )
        .unwrap();

        apply_unparented_parent_assignments(
            &mut width_graph,
            &width_assignments,
            UnparentedParentBatchShape::default(),
            &mut width_control,
        )
        .expect("the unbounded width batch succeeds");

        assert_eq!(width_meter.used(), expected_width_work);
        assert_eq!(width_graph.child_count("root"), size);
        assert_eq!(
            width_graph.children("root").first().copied(),
            Some("child-0")
        );
        let last_child = format!("child-{}", size - 1);
        assert_eq!(
            width_graph.children("root").last().copied(),
            Some(last_child.as_str())
        );

        let mut depth_graph = compound_graph();
        for index in 0..=size {
            depth_graph.set_node(format!("node-{index}"), NodeLabel::default());
        }
        let depth_assignments = (1..=size)
            .rev()
            .map(|index| {
                (
                    depth_graph.node_ix(&format!("node-{index}")).unwrap(),
                    depth_graph.node_ix(&format!("node-{}", index - 1)).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let depth_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let mut depth_control = OperationLayoutWorkControl::new(Arc::clone(&depth_meter));
        let expected_depth_work = unparented_parent_batch_work_upper_bound(
            depth_graph.node_slot_count(),
            0,
            depth_assignments.len(),
            0,
            &depth_control,
        )
        .unwrap();

        apply_unparented_parent_assignments(
            &mut depth_graph,
            &depth_assignments,
            UnparentedParentBatchShape::default(),
            &mut depth_control,
        )
        .expect("the unbounded depth batch succeeds");

        assert_eq!(depth_meter.used(), expected_depth_work);
        assert_eq!(depth_graph.parent("node-0"), None);
        let deepest = format!("node-{size}");
        let expected_parent = format!("node-{}", size - 1);
        assert_eq!(depth_graph.parent(&deepest), Some(expected_parent.as_str()));
    }
}

#[test]
fn parent_batch_adapter_charges_only_real_existing_and_numeric_work() {
    let control = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )));

    assert_eq!(
        unparented_parent_batch_work_upper_bound(4, 0, 3, 0, &control).unwrap(),
        66
    );
    assert_eq!(
        unparented_parent_batch_work_upper_bound(4, 0, 3, 2, &control).unwrap(),
        78
    );
    assert_eq!(
        unparented_parent_batch_work_upper_bound(4, 3, 3, 0, &control).unwrap(),
        102
    );
    assert_eq!(
        unparented_parent_batch_work_upper_bound(4, 3, 0, 0, &control).unwrap(),
        0
    );
}

#[test]
fn flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack() {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("flowchart-deep-subgraph-traversal".to_string())
        .stack_size(512 * 1024)
        .spawn(move || {
            let mut graph = deep_compound_graph(DEEP_SUBGRAPH_DEPTH);
            let mut work_control = OperationLayoutWorkControl::new(Arc::new(
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            ));
            let descendants = extract_descendants("n0", &graph, &mut work_control)
                .expect("unbounded work control accepts descendant extraction");
            let anchor = flowchart_find_non_cluster_child("n0", &graph, "n0", &mut work_control)
                .expect("unbounded work control accepts anchor lookup");
            let deep_subgraphs = deep_subgraphs(DEEP_SUBGRAPH_DEPTH);
            let deep_subgraphs_by_id = deep_subgraphs
                .iter()
                .map(|subgraph| (subgraph.id.as_str(), subgraph))
                .collect();
            let dirs = compute_effective_dir_by_id(
                &deep_subgraphs,
                &deep_subgraphs_by_id,
                &graph,
                "TB",
                false,
                &mut work_control,
            )
            .expect("unbounded work control accepts effective-direction planning");

            let mut copied = compound_graph();
            let descendants_by_id = HashMap::from([("n0".to_string(), descendants.clone())]);
            copy_cluster(
                "n0",
                &mut graph,
                &mut copied,
                "n0",
                &descendants_by_id,
                &mut work_control,
            )
            .expect("unbounded work control accepts cluster copying");

            tx.send(DeepTraversalOutcome {
                descendant_count: descendants.len(),
                anchor,
                root_dir: dirs.get("n0").cloned(),
                child_dir: dirs.get("n1").cloned(),
                copied_node_count: copied.node_count(),
            })
            .unwrap();
        })
        .expect("spawn deep subgraph traversal test");

    let outcome = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("deep subgraph traversal should finish without stack overflow");

    assert_eq!(outcome.descendant_count, DEEP_SUBGRAPH_DEPTH);
    assert_eq!(outcome.anchor, Some(format!("n{DEEP_SUBGRAPH_DEPTH}")));
    assert_eq!(outcome.root_dir.as_deref(), Some("LR"));
    assert_eq!(outcome.child_dir.as_deref(), Some("TB"));
    assert_eq!(outcome.copied_node_count, DEEP_SUBGRAPH_DEPTH);
}

#[test]
fn extract_descendants_handles_deeply_nested_subgraphs() {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(move || {
            let mut g = Graph::new(GraphOptions {
                compound: true,
                ..Default::default()
            });
            for i in (0..DEEP_SUBGRAPH_DEPTH).rev() {
                g.set_parent(format!("n{}", i + 1), format!("n{i}"));
            }
            let mut work_control = OperationLayoutWorkControl::new(Arc::new(
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            ));
            let descendants = extract_descendants("n0", &g, &mut work_control)
                .expect("unbounded work control accepts descendant extraction");
            let _ = tx.send(descendants);
        })
        .unwrap();
    let descendants = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("extract_descendants overflowed the stack on deep nesting");
    assert_eq!(descendants.len(), DEEP_SUBGRAPH_DEPTH);
}
