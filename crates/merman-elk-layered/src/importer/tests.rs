use super::*;
use crate::SpacingOptions;
use crate::compound::preprocess_source_ported_compound_graph;
use crate::graph::LNodeKind;
use crate::options::OrderingStrategy;

fn node(id: &str) -> ElkInputNode {
    ElkInputNode {
        id: id.to_string(),
        width: 80.0,
        height: 40.0,
        parent: None,
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_alignment: None,
        port_constraints: None,
        node_label_placement: NodeLabelPlacement::Fixed,
        node_flexibility: crate::options::NodeFlexibility::None,
        ports_surrounding: None,
        nested_options: None,
        label: None,
    }
}

fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
    ElkInputEdge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        labels: Vec::new(),
        minlen: 1,
        inside_self_loops_yo: false,
        model_order: None,
        priority_direction: 0,
        priority_shortness: 0,
        priority_straightness: 0,
    }
}

fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> ElkInputGraph {
    ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes,
        edges,
    }
}

#[test]
fn label_conversion_ignores_empty_text_even_with_nonzero_size() {
    for (width, height) in [(0.0, 0.0), (120.0, 10.0)] {
        assert!(label_to_lgraph(&ElkInputLabel::center("", width, height)).is_none());
    }
}

#[test]
fn label_conversion_preserves_whitespace_and_zero_size_measurement_placeholders() {
    for text in [" ", "\t\n", "\u{fffc}"] {
        let mut input = ElkInputLabel::center(text, 0.0, 0.0);
        input.source_index = Some(7);
        input.placement = EdgeLabelPlacement::Tail;
        input.inline = false;
        let label = label_to_lgraph(&input).unwrap();
        assert_eq!(label.text, text);
        assert_eq!(label.size, LSize::default());
        assert_eq!(label.source_index, Some(7));
        assert_eq!(label.placement, EdgeLabelPlacement::Tail);
        assert!(!label.inline);
    }
}

#[derive(Debug)]
struct RecordingWorkControl {
    remaining: usize,
    checked: usize,
    charged: usize,
    check_calls: usize,
    first_check: Option<usize>,
}

impl RecordingWorkControl {
    fn unlimited() -> Self {
        Self {
            remaining: usize::MAX,
            checked: 0,
            charged: 0,
            check_calls: 0,
            first_check: None,
        }
    }
}

impl WorkControl for RecordingWorkControl {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        self.check_calls = checked_sum([self.check_calls, 1])?;
        self.first_check.get_or_insert(units);
        self.checked = checked_sum([self.checked, units])?;
        if units > self.remaining {
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        if units > self.remaining {
            return Err(WorkError::Interrupted);
        }
        self.remaining -= units;
        self.charged = checked_sum([self.charged, units])?;
        Ok(())
    }
}

fn measured_import_work(input: &ElkInputGraph) -> usize {
    let mut work_control = RecordingWorkControl::unlimited();
    import_graph_with_work_control(input, &mut work_control).unwrap();
    assert_eq!(work_control.check_calls, 2);
    assert_eq!(work_control.checked, work_control.charged);
    work_control.charged
}

fn measured_scoped_import_work(input: &ElkInputGraph, segments: &[ElkInputEdgeSegment]) -> usize {
    let mut work_control = RecordingWorkControl::unlimited();
    import_graph_at_scope_and_segments_with_work_control(
        input,
        &["root"],
        segments,
        &mut work_control,
    )
    .unwrap();
    assert_eq!(work_control.check_calls, 2);
    assert_eq!(work_control.checked, work_control.charged);
    work_control.charged
}

#[test]
fn parent_cycle_reports_the_revisited_cycle_member_after_a_stem() {
    let mut stem = node("stem");
    stem.parent = Some("A".to_string());
    let mut a = node("A");
    a.parent = Some("B".to_string());
    let mut b = node("B");
    b.parent = Some("A".to_string());
    let input = graph(vec![stem, a, b], vec![]);
    let node_order = input
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();

    assert!(matches!(
        detect_parent_cycles(&input, &node_order),
        Err(ImportError::ParentCycle { node_id }) if node_id == "A"
    ));
}

fn measured_hierarchical_import(input: &ElkInputGraph) -> (LGraph, usize, ImportHierarchyProbe) {
    let mut work_control = RecordingWorkControl::unlimited();
    let mut port_probe = ImportPortProbe::default();
    let mut hierarchy_probe = ImportHierarchyProbe::default();
    let graph = import_graph_with_random_seed_authority_and_probe(
        input,
        RandomSeedAuthority::require_explicit(),
        GraphSeedScope::root("root"),
        &[],
        &mut work_control,
        &mut port_probe,
        &mut hierarchy_probe,
    )
    .unwrap();
    assert_eq!(work_control.check_calls, 2);
    assert_eq!(work_control.checked, work_control.charged);
    (graph, work_control.charged, hierarchy_probe)
}

#[test]
fn import_preflight_rejects_before_input_index_validation_or_allocation() {
    let input = graph(vec![node("duplicate"), node("duplicate")], Vec::new());
    let mut work_control = RecordingWorkControl {
        remaining: 1,
        checked: 0,
        charged: 0,
        check_calls: 0,
        first_check: None,
    };

    let error = import_graph_with_work_control(&input, &mut work_control).unwrap_err();

    assert_eq!(error, ImportError::Work(WorkError::Interrupted));
    assert_eq!(work_control.first_check, Some(2));
    assert_eq!(work_control.check_calls, 1);
    assert_eq!(work_control.charged, 0);
}

#[test]
fn import_rejects_duplicate_input_edge_ids_before_endpoint_validation() {
    let input = graph(
        vec![node("A"), node("B")],
        vec![
            edge("duplicate", "A", "B"),
            edge("duplicate", "missing", "B"),
        ],
    );
    let mut work_control = NoopWorkControl;

    let error = import_graph_with_work_control(&input, &mut work_control).unwrap_err();

    assert_eq!(
        error,
        ImportError::DuplicateEdge {
            id: "duplicate".to_string(),
        }
    );
}

#[test]
fn flat_import_planning_bound_is_linear_in_independent_node_and_edge_growth() {
    for node_count in [1usize, 8, 32, 128] {
        let nodes = (0..node_count)
            .map(|index| node(format!("node-{index}").as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            measured_import_work(&graph(nodes, Vec::new())),
            node_count + 1
        );
    }

    for edge_count in [0usize, 8, 32, 128] {
        let edges = (0..edge_count)
            .map(|index| edge(format!("edge-{index}").as_str(), "A", "B"))
            .collect::<Vec<_>>();
        assert_eq!(
            measured_import_work(&graph(vec![node("A"), node("B")], edges)),
            3 + 3 * edge_count
        );
    }
}

#[test]
fn hierarchical_import_uses_linear_graph_slots_for_deep_chains() {
    for node_count in [1usize, 4, 16, 64, 256] {
        let mut nodes = Vec::with_capacity(node_count);
        for index in 0..node_count {
            let mut current = node(format!("node-{index}").as_str());
            current.parent = index.checked_sub(1).map(|parent| format!("node-{parent}"));
            current.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
            nodes.push(current);
        }
        let mut input = graph(nodes, Vec::new());
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        let (imported, work, probe) = measured_hierarchical_import(&input);

        assert_eq!(work, 2 * node_count);
        assert_eq!(probe.node_imports, node_count);
        assert_eq!(probe.edge_imports, 0);
        assert_eq!(probe.nested_graphs, node_count.saturating_sub(1));
        assert_eq!(probe.output_path_components, 0);

        let mut graph = &imported;
        for index in 0..node_count {
            assert_eq!(graph.layerless_nodes.len(), 1);
            assert_eq!(graph.layerless_nodes[0].id, format!("node-{index}"));
            if index + 1 < node_count {
                graph = graph.layerless_nodes[0]
                    .nested_graph
                    .as_deref()
                    .expect("every non-leaf chain node owns the next graph slot");
            } else {
                assert!(graph.layerless_nodes[0].nested_graph.is_none());
            }
        }
    }
}

#[test]
fn hierarchical_import_charges_only_retained_cross_edge_paths_by_depth() {
    for node_count in [2usize, 8, 32] {
        for edge_count in [1usize, 4, 16] {
            let mut nodes = Vec::with_capacity(node_count);
            for index in 0..node_count {
                let mut current = node(format!("node-{index}").as_str());
                current.parent = index.checked_sub(1).map(|parent| format!("node-{parent}"));
                current.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
                nodes.push(current);
            }
            let edges = (0..edge_count)
                .map(|index| {
                    edge(
                        format!("edge-{index}").as_str(),
                        "node-0",
                        format!("node-{}", node_count - 1).as_str(),
                    )
                })
                .collect::<Vec<_>>();
            let mut input = graph(nodes, edges);
            input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
            let retained_path_components = edge_count * (node_count - 1);
            // The node descriptors and graph domains are independent of the edge descriptor,
            // owner-query, and retained path work.
            let expected_work = 2 * node_count + 3 * edge_count + retained_path_components;

            let (imported, work, probe) = measured_hierarchical_import(&input);

            assert_eq!(work, expected_work);
            assert_eq!(probe.node_imports, node_count);
            assert_eq!(probe.edge_imports, edge_count);
            assert_eq!(probe.nested_graphs, node_count - 1);
            assert_eq!(probe.output_path_components, retained_path_components);
            assert_eq!(imported.hierarchy_edges.len(), edge_count);
            assert!(imported.hierarchy_edges.iter().all(|edge| {
                edge.source_path.is_empty() && edge.target_path.len() == node_count - 1
            }));
            assert_eq!(
                imported
                    .hierarchy_edges
                    .iter()
                    .map(|edge| edge.id.clone())
                    .collect::<Vec<_>>(),
                (0..edge_count)
                    .map(|index| format!("edge-{index}"))
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn edge_owner_queries_do_not_scale_with_hierarchy_depth() {
    for node_count in [8usize, 64, 512] {
        let mut nodes = Vec::with_capacity(node_count);
        for index in 0..node_count {
            let mut current = node(format!("node-{index}").as_str());
            current.parent = index.checked_sub(1).map(|parent| format!("node-{parent}"));
            nodes.push(current);
        }
        let input = graph(
            nodes,
            vec![edge(
                "root-leaf",
                "node-0",
                format!("node-{}", node_count - 1).as_str(),
            )],
        );

        let index = InputIndex::new(&input).unwrap();

        assert_eq!(index.edge_owner_query_steps, 1);
    }
}

#[test]
fn hierarchy_query_index_resolves_light_branches_and_separate_forests() {
    let mut left = node("left");
    left.parent = Some("root-a".to_string());
    let mut left_leaf = node("left-leaf");
    left_leaf.parent = Some("left".to_string());
    let mut right = node("right");
    right.parent = Some("root-a".to_string());
    let mut right_leaf = node("right-leaf");
    right_leaf.parent = Some("right".to_string());
    let input = graph(
        vec![
            node("root-a"),
            left,
            left_leaf,
            right,
            right_leaf,
            node("root-b"),
        ],
        Vec::new(),
    );
    let index = InputIndex::new(&input).unwrap();
    let root_a = index.node_index("root-a").unwrap();
    let left_leaf = index.node_index("left-leaf").unwrap();
    let right_leaf = index.node_index("right-leaf").unwrap();
    let root_b = index.node_index("root-b").unwrap();

    assert_eq!(
        index.hierarchy_queries.lca(left_leaf, right_leaf).unwrap(),
        (Some(root_a), 2)
    );
    assert_eq!(
        index.hierarchy_queries.lca(left_leaf, root_b).unwrap(),
        (None, 1)
    );
}

#[test]
fn scoped_hierarchy_query_observations_fit_the_precharged_hld_bound() {
    let make_child = |id: &str, parent: &str, has_children: bool| {
        let mut child = node(id);
        child.parent = Some(parent.to_string());
        child.hierarchy_handling = has_children.then_some(HierarchyHandling::IncludeChildren);
        child
    };
    let mut root_a = node("root-a");
    root_a.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut root_b = node("root-b");
    root_b.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut input = graph(
        vec![
            root_a,
            make_child("left", "root-a", true),
            make_child("left-mid", "left", true),
            make_child("left-leaf", "left-mid", false),
            make_child("right", "root-a", true),
            make_child("right-mid", "right", true),
            make_child("right-leaf", "right-mid", false),
            root_b,
            make_child("other-mid", "root-b", true),
            make_child("other-leaf", "other-mid", false),
        ],
        Vec::new(),
    );
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    let scoped = |id: &str, source: &str, target: &str, edge_order: usize| ElkInputEdgeSegment {
        edge: edge(id, source, target),
        source: ElkInputEdgeSegmentEndpoint::Node {
            id: source.to_string(),
        },
        target: ElkInputEdgeSegmentEndpoint::Node {
            id: target.to_string(),
        },
        segment: CompoundEdgeSegment::Output { depth: 0 },
        edge_order,
    };
    let segments = vec![
        scoped("across-light-branches", "left-leaf", "right-leaf", 0),
        scoped("across-forest", "left-leaf", "other-leaf", 1),
    ];

    let index = InputIndex::new(&input).unwrap();
    let observed = plan_scoped_edge_segments(&index, &segments)
        .unwrap()
        .hierarchy_query_steps;
    let bound = ImportPlanningWorkPlan::new(&input, &segments)
        .unwrap()
        .scoped_hierarchy_query_bound;

    assert!(observed > segments.len());
    assert!(observed <= bound, "observed {observed}, bound {bound}");
}

#[test]
fn mixed_separate_subtrees_do_not_add_parent_locator_work() {
    for unreachable_depth in [1usize, 8, 32, 128] {
        let mut materialized = node("materialized");
        materialized.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut separate = node("separate");
        separate.parent = Some("materialized".to_string());
        separate.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
        let mut nodes = vec![materialized, separate];
        for depth in 0..unreachable_depth {
            let mut descendant = node(format!("unreachable-{depth}").as_str());
            descendant.parent = Some(if depth == 0 {
                "separate".to_string()
            } else {
                format!("unreachable-{}", depth - 1)
            });
            nodes.push(descendant);
        }
        let node_count = nodes.len();
        let mut input = graph(nodes, Vec::new());
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let (_, work, probe) = measured_hierarchical_import(&input);
        assert_eq!(work, node_count + 2);
        assert_eq!(probe.node_imports, 2);
        assert_eq!(probe.nested_graphs, 1);
        assert_eq!(probe.output_path_components, 0);
    }
}

#[test]
fn separate_children_self_loop_graph_does_not_materialize_regular_children() {
    let mut group = node("group");
    group.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let loop_edge = ElkInputEdge {
        inside_self_loops_yo: true,
        ..edge("group-loop", "group", "group")
    };
    let mut input = graph(vec![group, child], vec![loop_edge]);
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    input.options.inside_self_loops_activate = true;

    let (imported, work, probe) = measured_hierarchical_import(&input);
    assert_eq!(work, 7);
    assert_eq!(probe.node_imports, 1);
    assert_eq!(probe.edge_imports, 1);
    assert_eq!(probe.nested_graphs, 1);
    assert_eq!(probe.output_path_components, 0);
    let nested = imported.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("inside self-loop creates a nested graph");
    assert!(nested.layerless_nodes.iter().all(|node| node.id != "child"));
}

#[test]
fn scoped_segment_planning_bound_is_linear_in_segments_at_fixed_node_count() {
    for node_count in [8usize, 32] {
        let mut nodes = vec![node("A"), node("B")];
        nodes.extend((2..node_count).map(|index| node(format!("node-{index}").as_str())));
        let input = graph(nodes, Vec::new());
        for segment_count in [8usize, 32] {
            let segments = (0..segment_count)
                .map(|index| ElkInputEdgeSegment {
                    edge: edge(format!("edge-{index}").as_str(), "A", "B"),
                    source: ElkInputEdgeSegmentEndpoint::Node {
                        id: "A".to_string(),
                    },
                    target: ElkInputEdgeSegmentEndpoint::Node {
                        id: "B".to_string(),
                    },
                    segment: CompoundEdgeSegment::Output { depth: 0 },
                    edge_order: index,
                })
                .collect::<Vec<_>>();

            assert_eq!(
                measured_scoped_import_work(&input, &segments),
                node_count + 1 + segment_count * (2 + 4 * (ceil_log2(node_count.max(1)) + 1))
            );
        }
    }
}

#[test]
fn scoped_pieces_may_reference_the_same_logical_edge_id() {
    let input = graph(vec![node("A"), node("B")], Vec::new());
    let segment = |edge_order| ElkInputEdgeSegment {
        edge: edge("logical-edge", "A", "B"),
        source: ElkInputEdgeSegmentEndpoint::Node {
            id: "A".to_string(),
        },
        target: ElkInputEdgeSegmentEndpoint::Node {
            id: "B".to_string(),
        },
        segment: CompoundEdgeSegment::Output { depth: 0 },
        edge_order,
    };
    let mut work_control = NoopWorkControl;

    let imported = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        &[segment(0), segment(1)],
        &mut work_control,
    )
    .unwrap();

    assert_eq!(imported.edges.len(), 2);
    assert_eq!(imported.cross_hierarchy_edges.len(), 2);
    assert!(
        imported
            .cross_hierarchy_edges
            .iter()
            .all(|segment| segment.original_edge_id == "logical-edge")
    );
}

#[test]
fn merged_cross_edge_ports_use_constant_operation_local_probes() {
    let mut group = node("group");
    group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    group.nested_options = Some(Box::new(LayeredOptions {
        merge_edges: true,
        ..Default::default()
    }));
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let edge_count = 32usize;
    let edges = (0..edge_count)
        .map(|index| edge(format!("edge-{index}").as_str(), "child", "outer"))
        .collect::<Vec<_>>();
    let mut input = graph(vec![group, child, node("outer")], edges);
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    input.options.merge_edges = true;
    let mut work_control = NoopWorkControl;
    let mut port_probe = ImportPortProbe::default();
    let mut hierarchy_probe = ImportHierarchyProbe::default();

    import_graph_with_random_seed_authority_and_probe(
        &input,
        RandomSeedAuthority::require_explicit(),
        GraphSeedScope::root("root"),
        &[],
        &mut work_control,
        &mut port_probe,
        &mut hierarchy_probe,
    )
    .unwrap();

    assert_eq!(port_probe.lookups, edge_count * 2);
    assert_eq!(port_probe.creations, 2);
    assert_eq!(port_probe.hits, edge_count * 2 - 2);
}

#[test]
fn dedicated_cross_edge_ports_do_not_populate_the_reuse_index() {
    let mut group = node("group");
    group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let edge_count = 32usize;
    let edges = (0..edge_count)
        .map(|index| edge(format!("edge-{index}").as_str(), "child", "outer"))
        .collect::<Vec<_>>();
    let mut input = graph(vec![group, child, node("outer")], edges);
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    input.options.merge_edges = false;
    let mut work_control = NoopWorkControl;
    let mut port_probe = ImportPortProbe::default();
    let mut hierarchy_probe = ImportHierarchyProbe::default();

    let imported = import_graph_with_random_seed_authority_and_probe(
        &input,
        RandomSeedAuthority::require_explicit(),
        GraphSeedScope::root("root"),
        &[],
        &mut work_control,
        &mut port_probe,
        &mut hierarchy_probe,
    )
    .unwrap();

    assert_eq!(port_probe.lookups, 0);
    assert_eq!(port_probe.creations, 0);
    assert_eq!(port_probe.hits, 0);
    assert_eq!(imported.hierarchy_edges.len(), edge_count);
}

#[test]
fn inside_self_loop_dummies_use_constant_operation_local_probes() {
    let edge_count = 32usize;
    let edges = (0..edge_count)
        .map(|index| ElkInputEdge {
            inside_self_loops_yo: true,
            ..edge(format!("loop-{index}").as_str(), "A", "A")
        })
        .collect::<Vec<_>>();
    let mut input = graph(vec![node("A")], edges);
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    input.options.inside_self_loops_activate = true;
    let mut work_control = NoopWorkControl;
    let mut port_probe = ImportPortProbe::default();
    let mut hierarchy_probe = ImportHierarchyProbe::default();

    import_graph_with_random_seed_authority_and_probe(
        &input,
        RandomSeedAuthority::require_explicit(),
        GraphSeedScope::root("root"),
        &[],
        &mut work_control,
        &mut port_probe,
        &mut hierarchy_probe,
    )
    .unwrap();

    assert_eq!(port_probe.lookups, edge_count * 2);
    assert_eq!(port_probe.creations, 2);
    assert_eq!(port_probe.hits, edge_count * 2 - 2);
}

#[test]
fn edge_label_materialization_is_charged_after_the_initial_cancellation_check() {
    let mut labeled_edge = edge("ab", "A", "B");
    labeled_edge.labels = (0..128)
        .map(|_| ElkInputLabel::center("terminal", 18.0, 16.5))
        .collect();
    let input = graph(vec![node("A"), node("B")], vec![labeled_edge]);
    let planning = ImportPlanningWorkPlan::new(&input, &[])
        .unwrap()
        .total()
        .unwrap();
    assert!(planning > 0);
    for remaining in [0, planning] {
        let mut control = RecordingWorkControl {
            remaining,
            checked: 0,
            charged: 0,
            check_calls: 0,
            first_check: None,
        };
        let error = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            &[],
            &mut control,
        )
        .unwrap_err();
        assert_eq!(error, ImportError::Work(WorkError::Interrupted));
        assert_eq!(control.first_check, Some(planning));
        assert_eq!(control.charged, remaining);
        assert_eq!(control.check_calls, if remaining == 0 { 1 } else { 2 });
    }
    let mut unlimited = RecordingWorkControl::unlimited();
    let imported = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        &[],
        &mut unlimited,
    )
    .unwrap();
    assert_eq!(unlimited.charged, planning + 1 + 128);
    assert_eq!(imported.edges[0].labels.len(), 128);
}

#[test]
fn scoped_segment_work_charges_planning_before_output_materialization() {
    let mut group = node("group");
    group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let mut input = graph(vec![group, child, node("outer")], Vec::new());
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    let segment = ElkInputEdgeSegment {
        edge: edge("child-outer", "child", "outer"),
        source: ElkInputEdgeSegmentEndpoint::Node {
            id: "child".to_string(),
        },
        target: ElkInputEdgeSegmentEndpoint::Node {
            id: "outer".to_string(),
        },
        segment: CompoundEdgeSegment::Output { depth: 0 },
        edge_order: 0,
    };

    let mut unlimited = RecordingWorkControl::unlimited();
    let imported = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        std::slice::from_ref(&segment),
        &mut unlimited,
    )
    .unwrap();
    let required = unlimited.charged;
    let planning = ImportPlanningWorkPlan::new(&input, std::slice::from_ref(&segment))
        .unwrap()
        .total()
        .unwrap();
    let materialization = required - planning;
    assert_eq!(unlimited.first_check, Some(planning));
    assert_eq!(unlimited.checked, required);
    // Planning covers four descriptors plus the heavy-light query ceiling. Output work owns
    // two graph domains, one retained path hop, and two concrete hierarchy-local segments.
    assert_eq!(planning, 16);
    assert_eq!(materialization, 5);
    assert_eq!(imported.cross_hierarchy_edges.len(), 1);
    let nested = imported.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("group graph is materialized");
    assert_eq!(nested.cross_hierarchy_edges.len(), 1);

    let mut below_planning = RecordingWorkControl {
        remaining: planning - 1,
        checked: 0,
        charged: 0,
        check_calls: 0,
        first_check: None,
    };
    let error = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        std::slice::from_ref(&segment),
        &mut below_planning,
    )
    .unwrap_err();
    assert_eq!(error, ImportError::Work(WorkError::Interrupted));
    assert_eq!(below_planning.remaining, planning - 1);
    assert_eq!(below_planning.charged, 0);

    let mut below_materialization = RecordingWorkControl {
        remaining: required - 1,
        checked: 0,
        charged: 0,
        check_calls: 0,
        first_check: None,
    };
    let error = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        std::slice::from_ref(&segment),
        &mut below_materialization,
    )
    .unwrap_err();
    assert_eq!(error, ImportError::Work(WorkError::Interrupted));
    assert_eq!(below_materialization.charged, planning);
    assert_eq!(below_materialization.remaining, materialization - 1);

    for extra in [0usize, 1] {
        let mut accepted = RecordingWorkControl {
            remaining: required + extra,
            checked: 0,
            charged: 0,
            check_calls: 0,
            first_check: None,
        };
        import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            std::slice::from_ref(&segment),
            &mut accepted,
        )
        .unwrap();
        assert_eq!(accepted.checked, required);
        assert_eq!(accepted.charged, required);
        assert_eq!(accepted.remaining, extra);
    }
}

#[test]
fn imports_mermaid_flowchart_nodes_edges_labels_and_model_order() {
    let mut a = node("A");
    a.label = Some(ElkInputLabel::center("Alpha", 42.0, 18.0));
    let mut ab = edge("A-B", "A", "B");
    ab.labels = vec![ElkInputLabel::center("go", 20.0, 12.0)];

    let lgraph = import_graph(&graph(vec![a, node("B")], vec![ab])).unwrap();

    assert_eq!(lgraph.layerless_nodes.len(), 2);
    assert_eq!(lgraph.layerless_nodes[0].id, "A");
    assert_eq!(lgraph.layerless_nodes[0].model_order, Some(0));
    assert_eq!(lgraph.layerless_nodes[0].labels[0].text, "Alpha");
    assert_eq!(lgraph.edges.len(), 1);
    assert_eq!(lgraph.edges[0].model_order, Some(0));
    assert_eq!(lgraph.edges[0].thickness, 1.0);
    assert_eq!(
        lgraph.edges[0].labels[0].placement,
        EdgeLabelPlacement::Center
    );
    assert!(lgraph.graph_properties.center_labels);
    assert!(lgraph.options.graph_has_center_labels);
}

#[test]
fn importer_creates_nested_graph_for_inside_self_loop() {
    let mut graph = graph(
        vec![node("A")],
        vec![ElkInputEdge {
            inside_self_loops_yo: true,
            ..edge("A-A", "A", "A")
        }],
    );
    graph.options.inside_self_loops_activate = true;

    let lgraph = import_graph(&graph).unwrap();
    let node = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "A")
        .unwrap();
    assert!(node.nested_graph.is_some());
}

#[test]
fn importer_reuses_inside_self_loop_dummies_per_node_role() {
    let mut graph = graph(
        vec![node("A")],
        vec![
            ElkInputEdge {
                inside_self_loops_yo: true,
                ..edge("A-A-1", "A", "A")
            },
            ElkInputEdge {
                inside_self_loops_yo: true,
                ..edge("A-A-2", "A", "A")
            },
        ],
    );
    graph.options.inside_self_loops_activate = true;

    let lgraph = import_graph(&graph).unwrap();
    let node = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "A")
        .unwrap();
    let nested = node.nested_graph.as_ref().unwrap();
    let external_dummies = nested
        .layerless_nodes
        .iter()
        .filter(|candidate| candidate.kind == LNodeKind::ExternalPort)
        .collect::<Vec<_>>();

    assert_eq!(external_dummies.len(), 2);
    let input_dummy = external_dummies
        .iter()
        .find(|dummy| dummy.parent_port_type == Some(PortType::Input))
        .expect("inside self-loop input dummy should exist");
    let output_dummy = external_dummies
        .iter()
        .find(|dummy| dummy.parent_port_type == Some(PortType::Output))
        .expect("inside self-loop output dummy should exist");
    assert_eq!(input_dummy.ports[0].incoming_edges.len(), 2);
    assert_eq!(output_dummy.ports[0].outgoing_edges.len(), 2);
}

#[test]
fn importer_marks_inside_self_loops_as_graph_self_loops() {
    let mut graph = graph(
        vec![node("A")],
        vec![ElkInputEdge {
            inside_self_loops_yo: true,
            ..edge("A-A", "A", "A")
        }],
    );
    graph.options.inside_self_loops_activate = true;

    let lgraph = import_graph(&graph).unwrap();
    let nested = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "A")
        .and_then(|node| node.nested_graph.as_ref())
        .expect("inside self-loop should create a nested graph");

    assert!(nested.graph_properties.self_loops);
    assert!(nested.options.graph_has_self_loops);
}

#[test]
fn importer_applies_layered_padding_option_to_lgraph_padding() {
    let mut input = graph(vec![node("A")], vec![]);
    input.options.padding = ElkPadding {
        top: 7.0,
        right: 8.0,
        bottom: 9.0,
        left: 10.0,
    };

    let lgraph = import_graph(&input).unwrap();

    assert_eq!(lgraph.padding.top, 7.0);
    assert_eq!(lgraph.padding.right, 8.0);
    assert_eq!(lgraph.padding.bottom, 9.0);
    assert_eq!(lgraph.padding.left, 10.0);
}

#[test]
fn importer_applies_layered_padding_option_to_nested_graphs() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
    let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

    assert_eq!(lgraph.padding.top, 12.0);
    assert_eq!(lgraph.padding.left, 12.0);
    assert_eq!(nested.padding.top, 12.0);
    assert_eq!(nested.padding.left, 12.0);
}

#[test]
fn importer_adds_inside_top_node_label_padding_to_nested_graphs() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    cluster.node_label_placement = NodeLabelPlacement::InsideTopCenter;
    cluster.label = Some(ElkInputLabel::center("Cluster", 64.0, 22.0));
    cluster.nested_options = Some(Box::new(LayeredOptions {
        spacing: SpacingOptions::layered_base_value(30.0),
        ..Default::default()
    }));
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
    let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

    assert_eq!(nested.options.spacing.node_node, 30.0);
    assert_eq!(nested.padding.top, 39.0);
    assert_eq!(nested.padding.right, 12.0);
    assert_eq!(nested.padding.bottom, 12.0);
    assert_eq!(nested.padding.left, 12.0);
}

#[test]
fn importer_applies_nested_hierarchy_merge_without_cloning_root_options() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    cluster.nested_options = Some(Box::new(LayeredOptions {
        spacing: SpacingOptions::layered_base_value(30.0),
        layering_strategy: crate::options::LayeringStrategy::LongestPath,
        random_seed: 17,
        ..Default::default()
    }));
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut input = graph(vec![cluster, child], vec![]);
    input.options.merge_hierarchy_edges = false;
    input.options.consider_model_order_strategy = OrderingStrategy::NodesAndEdges;
    input.options.spacing = SpacingOptions::layered_base_value(40.0);
    input.options.layering_strategy = crate::options::LayeringStrategy::CoffmanGraham;
    input.options.random_seed = 42;

    let lgraph = import_graph(&input).unwrap();
    let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

    assert_eq!(lgraph.layerless_nodes[0].model_order, Some(0));
    assert_eq!(nested.layerless_nodes[0].model_order, None);
    assert!(nested.options.merge_hierarchy_edges);
    assert_eq!(
        nested.options.consider_model_order_strategy,
        OrderingStrategy::None
    );
    assert_eq!(nested.options.spacing.node_node, 30.0);
    assert_eq!(
        nested.options.layering_strategy,
        crate::options::LayeringStrategy::LongestPath
    );
    assert_eq!(nested.options.random_seed, 17);
}

#[test]
fn importer_copies_node_port_constraints_to_nested_graph_options() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    cluster.port_constraints = Some(PortConstraints::FixedSide);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
    let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

    assert_eq!(nested.options.port_constraints, PortConstraints::FixedSide);
}

#[test]
fn importer_forces_nested_direction_to_parent_direction() {
    let mut cluster = node("cluster");
    cluster.direction = Some(ElkDirection::Left);
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut input = graph(vec![cluster, child], vec![]);
    input.options.direction = ElkDirection::Down;

    let lgraph = import_graph(&input).unwrap();
    let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

    assert_eq!(nested.options.direction, ElkDirection::Down);
}

#[test]
fn importer_resolves_root_hierarchy_inherit_to_separate_children() {
    let mut input = graph(vec![node("cluster"), node("A")], vec![]);
    input.options.hierarchy_handling = HierarchyHandling::Inherit;
    input.nodes[1].parent = Some("cluster".to_string());

    let lgraph = import_graph(&input).unwrap();

    assert_eq!(
        lgraph.options.hierarchy_handling,
        HierarchyHandling::SeparateChildren
    );
    assert!(lgraph.layerless_nodes.iter().all(|node| !node.compound));
}

#[test]
fn imports_include_children_hierarchy_into_nested_graphs() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let lgraph = import_graph(&graph(vec![cluster, child, node("B")], vec![])).unwrap();

    let cluster = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let nested = cluster.nested_graph.as_ref().unwrap();
    assert_eq!(nested.parent_node_id.as_deref(), Some("cluster"));
    assert_eq!(nested.layerless_nodes[0].id, "A");
}

#[test]
fn importer_preserves_descendant_edge_for_compound_preprocessor() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let lgraph = import_graph(&graph(
        vec![cluster, child],
        vec![edge("cluster-A", "cluster", "A")],
    ))
    .unwrap();
    let cluster = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let nested = cluster.nested_graph.as_ref().unwrap();
    assert!(nested.edges.is_empty());
    assert_eq!(lgraph.hierarchy_edges.len(), 1);
    assert_eq!(lgraph.hierarchy_edges[0].id, "cluster-A");
    assert_eq!(lgraph.hierarchy_edges[0].source_node_id, "cluster");
    assert_eq!(lgraph.hierarchy_edges[0].target_node_id, "A");
    assert_eq!(lgraph.hierarchy_edges[0].source_path, Vec::<String>::new());
    assert_eq!(lgraph.hierarchy_edges[0].target_path, vec!["cluster"]);
}

#[test]
fn cross_hierarchy_import_creates_endpoint_ports_in_input_order() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut sibling = node("C");
    sibling.parent = Some("cluster".to_string());

    let lgraph = import_graph(&graph(
        vec![cluster, child, sibling, node("B")],
        vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
    ))
    .unwrap();

    let nested = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap()
        .nested_graph
        .as_ref()
        .unwrap();
    let child = nested
        .layerless_nodes
        .iter()
        .find(|node| node.id == "A")
        .unwrap();

    assert_eq!(
        child
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>(),
        vec!["A:0:source", "A:1"]
    );
    assert_eq!(nested.edges[0].source.port, 1);
    assert_eq!(lgraph.hierarchy_edges[0].source_port_key, "A:0:source");
}

#[test]
fn source_ported_compound_metadata_links_parent_port_and_external_dummy() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let mut lgraph = import_graph(&graph(
        vec![cluster, child, node("B")],
        vec![edge("A-B", "A", "B")],
    ))
    .unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);

    let cluster_index = lgraph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "cluster")
        .unwrap();
    let cluster = &lgraph.layerless_nodes[cluster_index];
    let parent_port = &cluster.ports[0];
    let port_dummy = parent_port
        .port_dummy
        .as_ref()
        .expect("compound port should point to nested external dummy");
    assert!(parent_port.inside_connections);
    assert_eq!(port_dummy.graph_id, "cluster");

    let nested = cluster.nested_graph.as_ref().unwrap();
    let external = &nested.layerless_nodes[port_dummy.node];
    assert_eq!(external.external_port_side, PortSide::South);
    assert_eq!(parent_port.border_offset, external.ports[0].border_offset);
    assert_eq!(
        parent_port.border_offset,
        Some(nested.options.spacing.edge_edge / 2.0)
    );
    let origin = external
        .origin_port
        .as_ref()
        .expect("external dummy should point back to parent port");
    assert_eq!(origin.graph_id, "root");
    assert_eq!(origin.port.node, cluster_index);
    assert_eq!(origin.port.port, 0);
}

#[test]
fn source_ported_compound_does_not_duplicate_existing_parent_dummy() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let mut lgraph = import_graph(&graph(
        vec![node("B"), cluster, child],
        vec![edge("B-A", "B", "A")],
    ))
    .unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);

    let cluster = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let nested = cluster.nested_graph.as_ref().unwrap();
    let external_dummies = nested
        .layerless_nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == LNodeKind::ExternalPort && node.id == "external:cluster")
        .collect::<Vec<_>>();

    assert_eq!(external_dummies.len(), 1);
    let (dummy_node, external_dummy) = external_dummies[0];
    let dummy_port = external_dummy.ports.first().unwrap();
    assert_eq!(
        dummy_port.incoming_edges.len() + dummy_port.outgoing_edges.len(),
        1
    );
    assert_eq!(cluster.ports.len(), 1);
    assert_eq!(
        cluster.ports[0].port_dummy.as_ref().unwrap().node,
        dummy_node
    );
}

#[test]
fn source_ported_compound_metadata_links_parent_to_child_external_dummy() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let mut lgraph = import_graph(&graph(
        vec![cluster, child],
        vec![edge("cluster-A", "cluster", "A")],
    ))
    .unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);

    let cluster_index = lgraph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "cluster")
        .unwrap();
    let cluster = &lgraph.layerless_nodes[cluster_index];
    let parent_port = cluster
        .ports
        .iter()
        .find(|port| port.port_dummy.is_some())
        .expect("parent-to-child edge should create a parent external port");
    assert_eq!(cluster.ports.len(), 1);
    assert_eq!(parent_port.id, "cluster:0:source");
    assert_eq!(parent_port.port_type, PortType::Output);
    let port_dummy = parent_port.port_dummy.as_ref().unwrap();
    assert!(parent_port.inside_connections);
    assert_eq!(port_dummy.graph_id, "cluster");

    let nested = cluster.nested_graph.as_ref().unwrap();
    let external = &nested.layerless_nodes[port_dummy.node];
    let origin = external
        .origin_port
        .as_ref()
        .expect("external dummy should point back to parent port");
    assert_eq!(origin.graph_id, "root");
    assert_eq!(origin.port.node, cluster_index);
    assert_eq!(origin.port.port, 0);
}

#[test]
fn source_ported_compound_parent_boundary_segments_use_external_port_dummies() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());

    let mut lgraph = import_graph(&graph(
        vec![cluster, child],
        vec![edge("cluster-A", "cluster", "A")],
    ))
    .unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);
    let cluster = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let nested = cluster.nested_graph.as_ref().unwrap();
    let segment = nested
        .edges
        .iter()
        .find(|edge| edge.id == "cluster-A")
        .unwrap();

    assert_eq!(
        nested.layerless_nodes[segment.source.node].kind,
        LNodeKind::ExternalPort
    );
    assert_eq!(nested.layerless_nodes[segment.target.node].id, "A");
}

#[test]
fn source_ported_compound_import_records_cross_hierarchy_segments() {
    let mut outer = node("outer");
    outer.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut inner = node("inner");
    inner.parent = Some("outer".to_string());
    inner.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("inner".to_string());

    let mut lgraph = import_graph(&graph(
        vec![outer, inner, child, node("B")],
        vec![edge("A-B", "A", "B")],
    ))
    .unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);

    let outer = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "outer")
        .unwrap();
    let inner_graph = outer
        .nested_graph
        .as_ref()
        .unwrap()
        .layerless_nodes
        .iter()
        .find(|node| node.id == "inner")
        .unwrap()
        .nested_graph
        .as_ref()
        .unwrap();
    let outer_graph = outer.nested_graph.as_ref().unwrap();
    assert_eq!(
        inner_graph.cross_hierarchy_edges[0].segment,
        CompoundEdgeSegment::Output { depth: 2 }
    );
    assert_eq!(
        outer_graph.cross_hierarchy_edges[0].segment,
        CompoundEdgeSegment::Output { depth: 1 }
    );
    assert_eq!(
        lgraph.cross_hierarchy_edges[0].segment,
        CompoundEdgeSegment::Output { depth: 0 }
    );
}

#[test]
fn source_ported_compound_reuses_exported_external_port_when_hierarchy_edges_merge() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut first = edge("A-B", "A", "B");
    let mut first_label = ElkInputLabel::center("first", 12.0, 6.0);
    first_label.placement = EdgeLabelPlacement::Tail;
    first.labels = vec![first_label];
    let mut second = edge("A-C", "A", "C");
    let mut second_label = ElkInputLabel::center("second", 18.0, 6.0);
    second_label.placement = EdgeLabelPlacement::Tail;
    second.labels = vec![second_label];
    let mut input = graph(
        vec![cluster, child, node("B"), node("C")],
        vec![first, second],
    );
    input.options.merge_edges = true;
    input.options.merge_hierarchy_edges = true;

    let mut lgraph = import_graph(&input).unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);

    let nested = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap()
        .nested_graph
        .as_ref()
        .unwrap();
    assert_eq!(nested.edges.len(), 1);
    assert_eq!(nested.cross_hierarchy_edges.len(), 2);
    assert!(
        nested
            .cross_hierarchy_edges
            .iter()
            .all(|segment| segment.edge == 0)
    );
    assert_eq!(nested.edges[0].labels.len(), 2);
    assert_eq!(
        nested.edges[0]
            .labels
            .iter()
            .filter_map(|label| label.original_label_edge.as_deref())
            .collect::<Vec<_>>(),
        vec!["A-B", "A-C"]
    );
    assert!(nested.graph_properties.end_labels);
}

#[test]
fn source_ported_compound_keeps_nested_hierarchy_merge_default_when_root_disables_merge() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut input = graph(
        vec![cluster, child, node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
    );
    input.options.merge_edges = true;
    input.options.merge_hierarchy_edges = false;

    let mut lgraph = import_graph(&input).unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);

    let nested = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap()
        .nested_graph
        .as_ref()
        .unwrap();
    assert_eq!(nested.edges.len(), 1);
    assert_eq!(
        nested
            .cross_hierarchy_edges
            .iter()
            .map(|segment| segment.edge)
            .collect::<Vec<_>>(),
        vec![0, 0]
    );
}

#[test]
fn source_ported_compound_parent_end_segments_are_not_reused() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut input = graph(
        vec![cluster, child],
        vec![
            edge("cluster-A-1", "cluster", "A"),
            edge("cluster-A-2", "cluster", "A"),
        ],
    );
    input.options.merge_edges = true;
    input.options.merge_hierarchy_edges = true;

    let mut lgraph = import_graph(&input).unwrap();
    preprocess_source_ported_compound_graph(&mut lgraph);

    let nested = lgraph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap()
        .nested_graph
        .as_ref()
        .unwrap();
    assert_eq!(nested.edges.len(), 2);
    assert_eq!(
        nested
            .cross_hierarchy_edges
            .iter()
            .map(|segment| segment.edge)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!(
        nested
            .edges
            .iter()
            .all(|edge| nested.layerless_nodes[edge.source.node].kind == LNodeKind::ExternalPort)
    );
}

#[test]
fn importer_reuses_collector_ports_when_edges_are_merged() {
    let mut input = graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
    );
    input.options.merge_edges = true;

    let lgraph = import_graph(&input).unwrap();

    let a = lgraph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert_eq!(lgraph.layerless_nodes[a].ports.len(), 1);
    assert_eq!(
        lgraph.layerless_nodes[a].ports[0].collector_type,
        Some(PortType::Output)
    );
    assert_eq!(
        lgraph.layerless_nodes[a].ports[0].outgoing_edges,
        vec![0, 1]
    );
    assert!(lgraph.graph_properties.hyperedges);
    assert!(lgraph.options.graph_has_hyperedges);
}

#[test]
fn importer_keeps_dedicated_ports_when_edge_merge_is_disabled() {
    let lgraph = import_graph(&graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
    ))
    .unwrap();

    let a = lgraph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert_eq!(lgraph.layerless_nodes[a].ports.len(), 2);
    assert!(
        lgraph.layerless_nodes[a]
            .ports
            .iter()
            .all(|port| port.collector_type.is_none())
    );
    assert!(!lgraph.graph_properties.hyperedges);
}

#[test]
fn implicit_ports_have_directional_sides_before_preprocessing() {
    for (direction, output_side, input_side) in [
        (ElkDirection::Right, PortSide::East, PortSide::West),
        (ElkDirection::Left, PortSide::West, PortSide::East),
        (ElkDirection::Down, PortSide::South, PortSide::North),
        (ElkDirection::Up, PortSide::North, PortSide::South),
    ] {
        let mut input = graph(
            vec![node("A"), node("B")],
            vec![edge("first", "A", "B"), edge("second", "A", "B")],
        );
        input.options.direction = direction;
        let imported = import_graph(&input).unwrap();
        let source = imported
            .layerless_nodes
            .iter()
            .find(|node| node.id == "A")
            .unwrap();
        let target = imported
            .layerless_nodes
            .iter()
            .find(|node| node.id == "B")
            .unwrap();
        assert_eq!(source.ports.len(), 2);
        assert_eq!(target.ports.len(), 2);
        for index in 0..2 {
            assert_eq!(source.ports[index].side, output_side, "{direction:?}");
            assert_eq!(target.ports[index].side, input_side, "{direction:?}");
            assert_eq!(source.ports[index].outgoing_edges, vec![index]);
            assert_eq!(target.ports[index].incoming_edges, vec![index]);
        }
    }
}

#[test]
fn importer_keeps_dedicated_ports_when_node_port_constraints_are_side_fixed() {
    let mut a = node("A");
    a.port_constraints = Some(PortConstraints::FixedSide);
    let mut input = graph(
        vec![a, node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
    );
    input.options.merge_edges = true;

    let lgraph = import_graph(&input).unwrap();

    let a = lgraph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert_eq!(lgraph.layerless_nodes[a].ports.len(), 2);
    assert!(
        lgraph.layerless_nodes[a]
            .ports
            .iter()
            .all(|port| port.collector_type.is_none())
    );
}

#[test]
fn import_rejects_invalid_parent_and_endpoints() {
    let mut child = node("A");
    child.parent = Some("missing".to_string());
    assert!(matches!(
        import_graph(&graph(vec![child], vec![])),
        Err(ImportError::MissingParent { .. })
    ));

    assert!(matches!(
        import_graph(&graph(vec![node("A")], vec![edge("A-B", "A", "B")])),
        Err(ImportError::MissingEndpoint { .. })
    ));
}

#[test]
fn scoped_segment_import_rejects_an_unmaterialized_owner_scope() {
    let group = node("group");
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let mut input = graph(vec![group, child, node("outer")], vec![]);
    input.options.hierarchy_handling = HierarchyHandling::SeparateChildren;
    let segment = ElkInputEdgeSegment {
        edge: edge("child-outer", "child", "outer"),
        source: ElkInputEdgeSegmentEndpoint::Node {
            id: "child".to_string(),
        },
        target: ElkInputEdgeSegmentEndpoint::Node {
            id: "outer".to_string(),
        },
        segment: CompoundEdgeSegment::Output { depth: 0 },
        edge_order: 0,
    };
    let mut work_control = NoopWorkControl;

    let error = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        &[segment],
        &mut work_control,
    )
    .expect_err("a flat import must reject a segment owned by a nested scope");

    assert_eq!(
        error,
        ImportError::UnavailableSegmentScope {
            edge_id: "child-outer".to_string(),
            graph_parent: "group".to_string(),
        }
    );
}

#[test]
fn scoped_segment_import_rejects_a_scope_below_an_unmaterialized_ancestor() {
    let mut outer = node("outer-group");
    outer.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
    let mut inner = node("inner-group");
    inner.parent = Some("outer-group".to_string());
    inner.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("child");
    child.parent = Some("inner-group".to_string());
    let mut input = graph(vec![outer, inner, child, node("root-node")], vec![]);
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    let segment = ElkInputEdgeSegment {
        edge: edge("child-root", "child", "root-node"),
        source: ElkInputEdgeSegmentEndpoint::Node {
            id: "child".to_string(),
        },
        target: ElkInputEdgeSegmentEndpoint::Node {
            id: "root-node".to_string(),
        },
        segment: CompoundEdgeSegment::Output { depth: 0 },
        edge_order: 0,
    };
    let mut work_control = NoopWorkControl;

    let error = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        &[segment],
        &mut work_control,
    )
    .expect_err("a locally includable scope below a SeparateChildren ancestor is unreachable");

    assert_eq!(
        error,
        ImportError::UnavailableSegmentScope {
            edge_id: "child-root".to_string(),
            graph_parent: "outer-group".to_string(),
        }
    );
}

#[test]
fn scoped_segment_import_accepts_a_materialized_owner_scope() {
    let mut group = node("group");
    group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let mut input = graph(vec![group, child, node("outer")], vec![]);
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    let mut segment_edge = edge("child-outer", "child", "outer");
    segment_edge.model_order = Some(7);
    segment_edge.labels = vec![ElkInputLabel::center("identity", 40.0, 12.0)];
    let segment = ElkInputEdgeSegment {
        edge: segment_edge,
        source: ElkInputEdgeSegmentEndpoint::Node {
            id: "child".to_string(),
        },
        target: ElkInputEdgeSegmentEndpoint::Node {
            id: "outer".to_string(),
        },
        segment: CompoundEdgeSegment::Output { depth: 2 },
        edge_order: 0,
    };
    let mut work_control = NoopWorkControl;

    let imported = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        &[segment],
        &mut work_control,
    )
    .expect("the nested owner scope is materialized");

    assert_eq!(imported.cross_hierarchy_edges.len(), 1);
    let nested = imported
        .layerless_nodes
        .iter()
        .find(|node| node.id == "group")
        .and_then(|node| node.nested_graph.as_deref())
        .expect("group scope should be materialized");
    assert_eq!(nested.cross_hierarchy_edges.len(), 1);
    assert_eq!(
        imported.cross_hierarchy_edges[0].original_model_order,
        Some(7)
    );
    assert_eq!(
        nested.cross_hierarchy_edges[0].original_model_order,
        Some(7)
    );
    assert_eq!(
        imported.cross_hierarchy_edges[0].segment,
        CompoundEdgeSegment::Output { depth: 2 }
    );
    assert_eq!(
        nested.cross_hierarchy_edges[0].segment,
        CompoundEdgeSegment::Output { depth: 3 }
    );
    let labels = imported
        .edges
        .iter()
        .chain(nested.edges.iter())
        .flat_map(|edge| edge.labels.iter())
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "identity");
    assert_eq!(
        labels[0].original_label_edge.as_deref(),
        Some("child-outer")
    );
}

#[test]
fn scoped_input_segment_rebases_every_materialized_hierarchy_level() {
    let mut group = node("group");
    group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let mut input = graph(vec![group, child, node("outer")], vec![]);
    input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
    let segment = ElkInputEdgeSegment {
        edge: edge("outer-child", "outer", "child"),
        source: ElkInputEdgeSegmentEndpoint::Node {
            id: "outer".to_string(),
        },
        target: ElkInputEdgeSegmentEndpoint::Node {
            id: "child".to_string(),
        },
        segment: CompoundEdgeSegment::Input { depth: 4 },
        edge_order: 0,
    };
    let mut work_control = NoopWorkControl;

    let imported = import_graph_at_scope_and_segments_with_work_control(
        &input,
        &["root"],
        &[segment],
        &mut work_control,
    )
    .expect("the nested owner scope is materialized");
    let nested = imported
        .layerless_nodes
        .iter()
        .find(|node| node.id == "group")
        .and_then(|node| node.nested_graph.as_deref())
        .expect("group scope should be materialized");

    assert_eq!(
        imported.cross_hierarchy_edges[0].segment,
        CompoundEdgeSegment::Input { depth: 4 }
    );
    assert_eq!(
        nested.cross_hierarchy_edges[0].segment,
        CompoundEdgeSegment::Input { depth: 5 }
    );
}

#[test]
fn import_preserves_model_order_strategy_without_enabling_wrapping() {
    let mut input = graph(vec![node("A"), node("B")], vec![edge("A-B", "A", "B")]);
    input.options.consider_model_order_strategy = OrderingStrategy::NodesAndEdges;

    let lgraph = import_graph(&input).unwrap();

    assert_eq!(
        lgraph.options.consider_model_order_strategy,
        OrderingStrategy::NodesAndEdges
    );
    assert!(!lgraph.options.graph_has_hyperedges);
}

#[test]
fn import_preserves_elk_unseeded_zero_until_the_pipeline_resolves_it() {
    let mut input = graph(vec![node("A")], vec![]);
    input.options.random_seed = 0;
    let mut imported = import_graph(&input).expect("zero is a valid upstream input value");

    assert!(matches!(
        crate::pipeline::execute_ported_processors(&mut imported),
        Err(crate::PipelineError::RandomSeed(crate::RandomSeedError::Unresolved {
            graph_path
        })) if graph_path == "root"
    ));
    assert_eq!(imported.options.random_seed, 0);
}

#[test]
fn operation_seed_scopes_nested_graphs_and_configuration_invocations() {
    use std::num::NonZeroU64;

    let mut group = node("group");
    group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    group.nested_options = Some(Box::new(LayeredOptions {
        random_seed: 0,
        ..Default::default()
    }));
    let mut child = node("child");
    child.parent = Some("group".to_string());
    let mut input = graph(vec![group, child], vec![]);
    input.options.random_seed = 0;
    let operation_seed = OperationSeed::from_operation_seed(
        NonZeroU64::new(0x0ddc_0ffe_e15e_cafe).expect("nonzero operation seed"),
    );

    let mut first = import_graph_with_operation_seed(&input, operation_seed).expect("import");
    let mut second = import_graph_with_operation_seed(&input, operation_seed).expect("import");
    crate::configurator::configure_graph_properties(&mut first).expect("resolve first");
    let first_root_first = first.random.clone().next_long();
    let first_nested_first = first.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("nested graph")
        .random
        .clone()
        .next_long();
    crate::configurator::configure_graph_properties(&mut first)
        .expect("resolve repeated configuration");
    let first_root_second = first.random.clone().next_long();
    let first_nested_second = first.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("nested graph")
        .random
        .clone()
        .next_long();

    crate::configurator::configure_graph_properties(&mut second).expect("resolve replay");
    let second_root_first = second.random.clone().next_long();
    let second_nested_first = second.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("nested graph")
        .random
        .clone()
        .next_long();
    crate::configurator::configure_graph_properties(&mut second)
        .expect("resolve replayed configuration");
    let second_root_second = second.random.clone().next_long();
    let second_nested_second = second.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("nested graph")
        .random
        .clone()
        .next_long();

    assert_eq!(first.options.random_seed, 0);
    assert_eq!(
        first.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("nested graph")
            .options
            .random_seed,
        0
    );
    assert_ne!(first_root_first, first_nested_first);
    assert_ne!(first_root_second, first_nested_second);
    assert_ne!(first_root_first, first_root_second);
    assert_ne!(first_nested_first, first_nested_second);
    assert_eq!(first.options.random_seed, second.options.random_seed);
    assert_eq!(first_root_first, second_root_first);
    assert_eq!(first_nested_first, second_nested_first);
    assert_eq!(first_root_second, second_root_second);
    assert_eq!(first_nested_second, second_nested_second);
}
