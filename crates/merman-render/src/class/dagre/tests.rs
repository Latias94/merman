use super::{ClassLayoutGraph, class_cluster_candidates, project_graph};
use crate::class::measured::{MeasuredGraph, MeasuredNode};
use crate::class::{
    ClassRenderConfig, ClassTextThemePlan, debug_build_class_diagram_dagre_graph,
    layout_class_diagram_typed_with_config,
};
use crate::graph_layout::ir::Direction;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::ClassNodeLabelPlan;
use crate::text::DeterministicTextMeasurer;
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel};
use indexmap::IndexMap;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;

#[test]
fn class_dagre_debug_input_uses_the_production_graph_and_source_identity_order() {
    let source =
        include_str!("../../../../../fixtures/class/stress_class_many_relations_labels_020.mmd");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Class fixture")
        .expect("detect Class fixture")
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class render model");
    };
    let graph = debug_build_class_diagram_dagre_graph(
        model,
        &parsed.metadata().effective_config,
        &DeterministicTextMeasurer::default(),
    )
    .expect("build Class Dagre input");

    assert_eq!(graph.node_ids(), ["A", "B", "C", "D", "E"]);
    assert!(graph.node_ids().into_iter().all(|id| {
        graph
            .node(&id)
            .is_some_and(|node| node.x.is_none() && node.y.is_none())
    }));
    assert_eq!(
        graph
            .edge_keys()
            .into_iter()
            .map(|edge| edge.name.expect("named Class edge"))
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3", "4", "5", "6", "7"]
    );
}

#[test]
fn forward_namespace_parents_do_not_reorder_declared_children() {
    let mut measured = MeasuredGraph {
        direction: Direction::Down,
        nodesep: 50.0,
        ranksep: 50.0,
        nodes: IndexMap::new(),
        parents: IndexMap::new(),
        edges: IndexMap::new(),
    };
    for (child, parent) in [("Child", "Parent"), ("Sibling", "Root"), ("Parent", "Root")] {
        measured.set_node(
            child.into(),
            MeasuredNode {
                is_namespace: true,
                ..Default::default()
            },
        );
        measured.set_parent(child.into(), parent.into());
    }
    measured.set_node(
        "Root".into(),
        MeasuredNode {
            is_namespace: true,
            ..Default::default()
        },
    );

    let graph = project_graph(measured);
    assert_eq!(graph.node_ids(), ["Child", "Parent", "Sibling", "Root"]);
    assert_eq!(graph.children("Root"), ["Sibling", "Parent"]);
    assert_eq!(graph.parent("Child"), Some("Parent"));
    assert_eq!(graph.parent("Parent"), Some("Root"));
}

fn class_candidate_test_graph(edges: &[(&str, &str)]) -> ClassLayoutGraph {
    let mut graph = Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    graph.set_graph(GraphLabel::default());
    for id in [
        "root",
        "a",
        "b",
        "leaf",
        "sibling",
        "outside",
        "outside_child",
    ] {
        graph.set_node(id, NodeLabel::default());
    }
    for (child, parent) in [
        ("a", "root"),
        ("b", "a"),
        ("leaf", "b"),
        ("sibling", "root"),
        ("outside_child", "outside"),
    ] {
        graph.set_parent(child, parent);
    }
    for (index, (from, to)) in edges.iter().copied().enumerate() {
        graph.set_edge_named(
            from,
            to,
            Some(index.to_string()),
            Some(EdgeLabel::default()),
        );
    }
    graph
}

fn reference_class_cluster_candidates(graph: &ClassLayoutGraph) -> Vec<String> {
    let is_strict_descendant = |node: &str, ancestor: &str| {
        let mut parent = graph.parent(node);
        while let Some(id) = parent {
            if id == ancestor {
                return true;
            }
            parent = graph.parent(id);
        }
        false
    };
    graph
        .nodes()
        .filter(|id| graph.children_iter(id).next().is_some())
        .filter(|id| {
            graph.edges().all(|edge| {
                edge.v == *id
                    || edge.w == *id
                    || is_strict_descendant(&edge.v, id) == is_strict_descendant(&edge.w, id)
            })
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn class_cluster_boundary_index_matches_mermaid_descendant_semantics() {
    let ids = [
        "root",
        "a",
        "b",
        "leaf",
        "sibling",
        "outside",
        "outside_child",
    ];
    let mut edge_sets = vec![Vec::new()];
    for from in ids {
        for to in ids {
            edge_sets.push(vec![(from, to)]);
        }
    }
    edge_sets.push(vec![
        ("leaf", "sibling"),
        ("a", "leaf"),
        ("outside_child", "b"),
    ]);

    for edges in edge_sets {
        let graph = class_candidate_test_graph(&edges);
        assert_eq!(
            class_cluster_candidates(&graph),
            reference_class_cluster_candidates(&graph),
            "edges={edges:?}"
        );
    }
}

#[test]
fn class_cluster_boundary_index_handles_max_size_namespace_chain() {
    const NODE_COUNT: usize = 4_000;
    let mut graph = Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    graph.set_graph(GraphLabel::default());
    for index in 0..NODE_COUNT {
        graph.set_node(format!("n{index}"), NodeLabel::default());
    }
    for index in (1..NODE_COUNT).rev() {
        graph.set_parent(format!("n{index}"), format!("n{}", index - 1));
    }

    let candidates = class_cluster_candidates(&graph);

    assert_eq!(candidates.len(), NODE_COUNT - 1);
    assert_eq!(candidates.first().map(String::as_str), Some("n0"));
    let expected_last = format!("n{}", NODE_COUNT - 2);
    assert_eq!(
        candidates.last().map(String::as_str),
        Some(expected_last.as_str())
    );
}

#[test]
fn class_html_prepared_labels_follow_public_layout_and_plan_owners() {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "classDiagram\nclass A {\n<<interface>>\n+value: String\n+call()\n}\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let config = crate::test_support::presentation_config(serde_json::json!({"htmlLabels":true}));
    let typography = ClassTextThemePlan::resolve(None, &config);
    let render_config = ClassRenderConfig::resolve(&config, &typography);
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let mut work = OperationLayoutWorkControl::new(Arc::clone(&meter));
    let (layout, sidecar) = layout_class_diagram_typed_with_config(
        model,
        &config,
        &DeterministicTextMeasurer::default(),
        None,
        &render_config,
        &mut work,
    )
    .unwrap();
    let retained = meter.prepared_text_retained_bytes();
    assert!(retained > 0);
    let plan = Arc::clone(&layout.class_label_plans_by_id["A"]);
    let ClassNodeLabelPlan::PreparedHtml(labels) = plan.as_ref() else {
        panic!("expected prepared HTML labels");
    };
    assert!(labels.annotation.is_some());
    assert_eq!(labels.members.len(), 1);
    assert_eq!(labels.methods.len(), 1);
    assert!(
        serde_json::to_value(&layout)
            .unwrap()
            .get("class_label_plans_by_id")
            .is_none()
    );

    let cloned_layout = layout.clone();
    assert!(Arc::ptr_eq(
        &layout.class_label_plans_by_id["A"],
        &cloned_layout.class_label_plans_by_id["A"]
    ));
    drop(sidecar);
    drop(work);
    drop(layout);
    assert_eq!(meter.prepared_text_retained_bytes(), retained);
    drop(cloned_layout);
    assert_eq!(meter.prepared_text_retained_bytes(), retained);
    drop(plan);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}
