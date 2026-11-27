use super::*;
use crate::graph::{InLayerConstraint, LLabel, LSize, PortType};
use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, import_graph};
use crate::options::{
    Alignment, ELK_DEFAULT_EDGE_THICKNESS, ElkDirection, LayerConstraint, LayeredOptions,
    PortConstraints,
};
use crate::p2layers::layer_network_simplex;

pub(super) fn node(id: &str) -> ElkInputNode {
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
        node_label_placement: crate::options::NodeLabelPlacement::Fixed,
        node_flexibility: crate::options::NodeFlexibility::None,
        ports_surrounding: None,
        nested_options: None,
        label: None,
    }
}

pub(super) fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
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

pub(super) fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> LGraph {
    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes,
        edges,
    })
    .unwrap()
}

#[test]
fn edge_and_layer_constraint_reverser_makes_first_nodes_outgoing_only() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::First);
    let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);
    let start = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "start")
        .unwrap();

    reverse_edges_for_edge_and_layer_constraints(&mut graph);

    assert!(graph.edges[0].reversed);
    assert_eq!(graph.edges[0].source.node, start);
    assert!(graph.node_incoming_edges(start).is_empty());
    assert_eq!(graph.node_outgoing_edges(start), vec![0]);
}

#[test]
fn edge_and_layer_constraint_reverser_makes_last_nodes_incoming_only() {
    let mut last = node("end");
    last.layer_constraint = Some(LayerConstraint::Last);
    let mut graph = graph(vec![last, node("A")], vec![edge("end-A", "end", "A")]);
    let end = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "end")
        .unwrap();

    reverse_edges_for_edge_and_layer_constraints(&mut graph);

    assert!(graph.edges[0].reversed);
    assert_eq!(graph.edges[0].target.node, end);
    assert_eq!(graph.node_incoming_edges(end), vec![0]);
    assert!(graph.node_outgoing_edges(end).is_empty());
}

#[test]
fn constraint_reversal_candidates_follow_fixed_side_cascades() {
    let mut graph = graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("B-A", "B", "A"), edge("C-B", "C", "B")],
    );
    for node_id in ["A", "B"] {
        let node_index = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == node_id)
            .unwrap();
        graph.layerless_nodes[node_index].port_constraints = PortConstraints::FixedSide;
        for port in &mut graph.layerless_nodes[node_index].ports {
            port.set_side(PortSide::East);
        }
    }

    let mut candidates = Vec::new();
    assert!(!visit_edge_and_layer_constraint_reversal_candidates(
        &graph,
        |edge| {
            candidates.push(edge);
            false
        }
    ));

    let mut executed = graph.clone();
    reverse_edges_for_edge_and_layer_constraints(&mut executed);
    let reversed = executed
        .edges
        .iter()
        .enumerate()
        .filter_map(|(edge, data)| data.reversed.then_some(edge))
        .collect::<Vec<_>>();

    assert_eq!(candidates, reversed);
    assert_eq!(
        candidates
            .iter()
            .map(|&edge| graph.edges[edge].id.as_str())
            .collect::<Vec<_>>(),
        ["B-A", "C-B"]
    );
}

#[test]
fn direct_reversal_does_not_admit_ineligible_fixed_collector_edges() {
    let parallel_edge_count = 128usize;
    let mut edges = (0..parallel_edge_count)
        .map(|index| edge(&format!("F-U-{index}"), "F", "U"))
        .collect::<Vec<_>>();
    edges.push(edge("F-D", "F", "D"));
    let mut first = node("D");
    first.layer_constraint = Some(LayerConstraint::First);
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
        },
        nodes: vec![node("F"), node("U"), first],
        edges,
    })
    .unwrap();
    let fixed = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "F")
        .unwrap();
    graph.layerless_nodes[fixed].port_constraints = PortConstraints::FixedSide;
    for port in &mut graph.layerless_nodes[fixed].ports {
        port.set_side(PortSide::East);
    }

    let mut candidates = Vec::new();
    assert!(!visit_edge_and_layer_constraint_reversal_candidates(
        &graph,
        |edge| {
            candidates.push(edge);
            false
        }
    ));

    assert_eq!(candidates.len(), 1);
    assert_eq!(graph.edges[candidates[0]].id, "F-D");

    reverse_edges_for_edge_and_layer_constraints(&mut graph);
    assert_eq!(graph.edges.iter().filter(|edge| edge.reversed).count(), 1);
    assert!(
        graph
            .edges
            .iter()
            .find(|edge| edge.id == "F-D")
            .unwrap()
            .reversed
    );
}

#[test]
fn long_edge_splitter_makes_layering_proper() {
    let mut graph = graph(
        vec![node("A"), node("B"), node("C"), node("D")],
        vec![
            edge("A-B", "A", "B"),
            edge("B-C", "B", "C"),
            edge("C-D", "C", "D"),
            edge("A-D", "A", "D"),
        ],
    );

    layer_network_simplex(&mut graph).unwrap();
    split_long_edges(&mut graph);

    assert!(
        graph
            .layerless_nodes
            .iter()
            .any(|node| node.kind == LNodeKind::LongEdge)
    );
    for edge in &graph.edges {
        let source_layer = graph.layerless_nodes[edge.source.node].layer_index.unwrap();
        let target_layer = graph.layerless_nodes[edge.target.node].layer_index.unwrap();
        assert_eq!(
            target_layer,
            source_layer + 1,
            "edge {} should connect adjacent layers",
            edge.id
        );
    }
}

#[test]
fn hyperedge_dummy_merger_coalesces_parallel_long_edge_segments() {
    let mut input_graph = ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![node("A"), node("B"), node("C"), node("D"), node("E")],
        edges: vec![
            edge("A-B", "A", "B"),
            edge("B-C", "B", "C"),
            edge("C-D", "C", "D"),
            edge("D-E", "D", "E"),
            edge("A-E-1", "A", "E"),
            edge("A-E-2", "A", "E"),
        ],
    };
    input_graph.options.merge_edges = true;
    let mut input = import_graph(&input_graph).unwrap();

    layer_network_simplex(&mut input).unwrap();
    split_long_edges(&mut input);

    let before = input
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| input.layerless_nodes[*node].kind == LNodeKind::LongEdge)
        .count();
    assert!(before >= 4);

    merge_hyperedge_dummies(&mut input);

    let after = input
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| input.layerless_nodes[*node].kind == LNodeKind::LongEdge)
        .count();
    assert!(after < before);
    assert!(
        input
            .edges
            .iter()
            .enumerate()
            .all(|(edge_index, _)| input.edge_source_attached(edge_index))
    );
    assert!(
        input
            .edges
            .iter()
            .enumerate()
            .all(|(edge_index, _)| input.edge_target_attached(edge_index))
    );
}

#[test]
fn split_edge_moves_head_labels_to_new_segment_and_records_origin_ports() {
    let mut head = ElkInputLabel::center("head", 20.0, 10.0);
    head.placement = EdgeLabelPlacement::Head;
    let mut long = edge("A-C", "A", "C");
    long.labels = vec![head];

    let mut graph = graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), long],
    );

    layer_network_simplex(&mut graph).unwrap();
    let long_edge = graph
        .edges
        .iter()
        .position(|edge| edge.id == "A-C")
        .unwrap();
    let original_source = graph.edges[long_edge].source;
    let original_target = graph.edges[long_edge].target;

    split_long_edges(&mut graph);

    let dummy = graph
        .layerless_nodes
        .iter()
        .position(|node| node.kind == LNodeKind::LongEdge)
        .unwrap();
    assert_eq!(graph.layerless_nodes[dummy].origin_edge, Some(long_edge));
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_source,
        Some(original_source)
    );
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_target,
        Some(original_target)
    );

    assert!(graph.edges[long_edge].labels.is_empty());
    let moved_label_edge = graph
        .edges
        .iter()
        .enumerate()
        .find(|(_, edge)| edge.labels.iter().any(|label| label.text == "head"))
        .map(|(index, _)| index)
        .unwrap();
    assert_ne!(moved_label_edge, long_edge);
    assert_eq!(
        graph.edges[moved_label_edge].labels[0].end_label_edge,
        Some(long_edge)
    );
}

#[test]
fn inverted_port_processor_splits_east_input_ports() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    let source = push_normal_node(&mut graph, "A");
    let target = push_normal_node(&mut graph, "B");
    graph.layerless_nodes[target].port_constraints = PortConstraints::FixedSide;
    graph.layerless_nodes[target].ports[0].set_side(PortSide::East);
    graph.set_node_layer(source, 0);
    graph.set_node_layer(target, 1);
    let edge = add_test_edge(&mut graph, "A-B", source, 0, target, 0);
    let mut head = LLabel::new("head", 10.0, 8.0);
    head.placement = EdgeLabelPlacement::Head;
    graph.edges[edge].labels.push(head);

    process_inverted_ports(&mut graph);

    let dummy = graph
        .layerless_nodes
        .iter()
        .position(|node| node.kind == LNodeKind::LongEdge)
        .expect("inverted input should create a long-edge dummy");
    assert_eq!(graph.layerless_nodes[dummy].layer_index, Some(1));
    assert_eq!(graph.edges[edge].target.node, dummy);
    assert_eq!(graph.edges[edge].target.port, 0);
    assert!(graph.edges[edge].labels.is_empty());

    let dummy_edge = graph
        .edges
        .iter()
        .position(|candidate| candidate.source.node == dummy && candidate.target.node == target)
        .expect("dummy should reconnect to original target port");
    assert_eq!(graph.edges[dummy_edge].source.port, 1);
    assert_eq!(graph.edges[dummy_edge].target.port, 0);
    assert_eq!(graph.edges[dummy_edge].labels[0].text, "head");
    assert_eq!(graph.edges[dummy_edge].labels[0].end_label_edge, Some(edge));
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_source,
        Some(graph.edges[edge].source)
    );
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_target,
        Some(graph.edges[dummy_edge].target)
    );
}

#[test]
fn inverted_port_processor_splits_west_output_ports() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    let source = push_normal_node(&mut graph, "A");
    let target = push_normal_node(&mut graph, "B");
    graph.layerless_nodes[source].port_constraints = PortConstraints::FixedSide;
    graph.layerless_nodes[source].ports[0].port_type = PortType::Output;
    graph.layerless_nodes[source].ports[0].set_side(PortSide::West);
    graph.set_node_layer(source, 0);
    graph.set_node_layer(target, 1);
    let edge = add_test_edge(&mut graph, "A-B", source, 0, target, 0);
    let mut head = LLabel::new("head", 10.0, 8.0);
    head.placement = EdgeLabelPlacement::Head;
    graph.edges[edge].labels.push(head);

    process_inverted_ports(&mut graph);

    let dummy = graph
        .layerless_nodes
        .iter()
        .position(|node| node.kind == LNodeKind::LongEdge)
        .expect("inverted output should create a long-edge dummy");
    assert_eq!(graph.layerless_nodes[dummy].layer_index, Some(0));
    assert_eq!(graph.edges[edge].source.node, source);
    assert_eq!(graph.edges[edge].target.node, dummy);
    assert_eq!(graph.edges[edge].target.port, 0);
    assert!(graph.edges[edge].labels.is_empty());

    let dummy_edge = graph
        .edges
        .iter()
        .position(|candidate| candidate.source.node == dummy && candidate.target.node == target)
        .expect("dummy should reconnect to original target port");
    assert_eq!(graph.edges[dummy_edge].source.port, 1);
    assert_eq!(graph.edges[dummy_edge].labels[0].text, "head");
    assert_eq!(graph.edges[dummy_edge].labels[0].end_label_edge, Some(edge));
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_source,
        Some(graph.edges[edge].source)
    );
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_target,
        Some(graph.edges[dummy_edge].target)
    );
}

#[test]
fn label_dummy_inserter_splits_center_label_edges_and_moves_labels_to_dummy() {
    let mut center = ElkInputLabel::center("choice", 30.0, 12.0);
    center.placement = EdgeLabelPlacement::Center;
    let mut labelled = edge("A-B", "A", "B");
    labelled.labels = vec![center];
    let mut graph = graph(vec![node("A"), node("B")], vec![labelled]);
    let edge_index = graph
        .edges
        .iter()
        .position(|edge| edge.id == "A-B")
        .unwrap();
    let original_source = graph.edges[edge_index].source;
    let original_target = graph.edges[edge_index].target;

    insert_label_dummies(&mut graph);

    let dummy = graph
        .layerless_nodes
        .iter()
        .position(|node| node.kind == LNodeKind::Label)
        .unwrap();
    assert_eq!(graph.layerless_nodes[dummy].origin_edge, Some(edge_index));
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_source,
        Some(original_source)
    );
    assert_eq!(
        graph.layerless_nodes[dummy].long_edge_target,
        Some(original_target)
    );
    assert!(graph.layerless_nodes[dummy].labels.is_empty());
    assert_eq!(graph.layerless_nodes[dummy].represented_labels.len(), 1);
    assert_eq!(
        graph.layerless_nodes[dummy].represented_labels[0].text,
        "choice"
    );
    assert!(graph.edges[edge_index].labels.is_empty());
    assert_eq!(graph.node_incoming_edges(dummy).len(), 1);
    assert_eq!(graph.node_outgoing_edges(dummy).len(), 1);
}

#[test]
fn label_dummy_remover_restores_center_labels_to_origin_edge() {
    let mut center = ElkInputLabel::center("choice", 30.0, 12.0);
    center.placement = EdgeLabelPlacement::Center;
    let mut labelled = edge("A-C", "A", "C");
    labelled.labels = vec![center];
    let mut graph = graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
    );
    let edge_index = graph
        .edges
        .iter()
        .position(|edge| edge.id == "A-C")
        .unwrap();

    insert_label_dummies(&mut graph);
    layer_network_simplex(&mut graph).unwrap();
    split_long_edges(&mut graph);
    switch_label_dummies(&mut graph);
    select_label_sides(&mut graph);
    let dummy = graph
        .layerless_nodes
        .iter()
        .position(|node| node.kind == LNodeKind::Label)
        .unwrap();
    graph.layerless_nodes[dummy].position = LPoint { x: 20.0, y: 40.0 };

    remove_label_dummies(&mut graph);

    assert!(!graph.layers.iter().any(|layer| {
        layer
            .nodes
            .iter()
            .any(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
    }));
    assert!(graph.layerless_nodes[dummy].represented_labels.is_empty());
    assert_eq!(graph.edges[edge_index].labels.len(), 1);
    assert_eq!(graph.edges[edge_index].labels[0].text, "choice");
    assert!(
        !graph.edge_source_attached(
            graph
                .edges
                .iter()
                .enumerate()
                .find(|(index, edge)| *index != edge_index && edge.id == "A-C")
                .map(|(index, _)| index)
                .unwrap()
        )
    );
}

#[test]
fn end_label_preprocessor_moves_head_labels_to_port_cell_and_expands_margin() {
    let mut head = ElkInputLabel::center("head", 24.0, 10.0);
    head.placement = EdgeLabelPlacement::Head;
    head.inline = false;
    let mut labelled = edge("A-B", "A", "B");
    labelled.labels = vec![head];
    let mut graph = graph(vec![node("A"), node("B")], vec![labelled]);

    layer_network_simplex(&mut graph).unwrap();
    crate::p3order::process_port_sides(&mut graph);
    crate::p4nodes::calculate_label_and_node_sizes(&mut graph);
    crate::p4nodes::calculate_innermost_node_margins(&mut graph);
    select_label_sides(&mut graph);
    let target = graph.edges[0].target.node;
    let target_port = graph.edges[0].target.port;
    let old_left_margin = graph.layerless_nodes[target].margin.left;

    preprocess_end_labels(&mut graph);

    assert!(graph.edges[0].labels.is_empty());
    let port = &graph.layerless_nodes[target].ports[target_port];
    assert_eq!(port.labels.len(), 1);
    assert_eq!(port.labels[0].text, "head");
    assert_eq!(port.labels[0].end_label_edge, Some(0));
    assert!(port.end_label_cell.is_some());
    assert!(graph.layerless_nodes[target].margin.left > old_left_margin);
}

#[test]
fn label_dummy_remover_centers_vertical_inline_labels_in_label_space() {
    let mut center = ElkInputLabel::center("choice", 30.0, 12.0);
    center.placement = EdgeLabelPlacement::Center;
    let mut labelled = edge("A-C", "A", "C");
    labelled.labels = vec![center];
    let mut graph = graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
    );
    let edge_index = graph
        .edges
        .iter()
        .position(|edge| edge.id == "A-C")
        .unwrap();

    insert_label_dummies(&mut graph);
    layer_network_simplex(&mut graph).unwrap();
    split_long_edges(&mut graph);
    switch_label_dummies(&mut graph);
    select_label_sides(&mut graph);
    let dummy = graph
        .layerless_nodes
        .iter()
        .position(|node| node.kind == LNodeKind::Label)
        .unwrap();
    graph.layerless_nodes[dummy].position = LPoint { x: 20.0, y: 40.0 };
    graph.layerless_nodes[dummy].size.height = 14.0;
    graph.layerless_nodes[dummy]
        .represented_labels
        .push(LLabel {
            text: "short".to_string(),
            size: LSize {
                width: 12.0,
                height: 4.0,
            },
            position: LPoint::default(),
            placement: EdgeLabelPlacement::Center,
            inline: true,
            label_side: None,
            end_label_edge: None,
            original_label_edge: None,
            source_index: None,
        });

    remove_label_dummies(&mut graph);

    let labels = &graph.edges[edge_index].labels;
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0].position.y, 41.0);
    assert_eq!(labels[1].position.y, 45.0);
}

#[test]
fn long_edge_joiner_merges_split_segments_and_removes_dummy_from_layer() {
    let mut graph = graph(
        vec![node("A"), node("B"), node("C")],
        vec![
            edge("A-B", "A", "B"),
            edge("B-C", "B", "C"),
            edge("A-C", "A", "C"),
        ],
    );
    graph.options.unnecessary_bendpoints = true;
    layer_network_simplex(&mut graph).unwrap();

    let long_edge = graph
        .edges
        .iter()
        .position(|edge| edge.id == "A-C")
        .unwrap();
    split_long_edges(&mut graph);

    let dummy = graph
        .layerless_nodes
        .iter()
        .position(|node| node.kind == LNodeKind::LongEdge)
        .unwrap();
    graph.layerless_nodes[dummy].position = LPoint { x: 50.0, y: 70.0 };
    graph.edges[long_edge]
        .bend_points
        .push(LPoint { x: 10.0, y: 20.0 });
    let dropped = graph
        .edges
        .iter()
        .enumerate()
        .find(|(index, edge)| *index != long_edge && edge.id == "A-C")
        .map(|(index, _)| index)
        .unwrap();
    graph.edges[dropped]
        .bend_points
        .push(LPoint { x: 90.0, y: 100.0 });
    let mut label = LLabel::new("head", 20.0, 10.0);
    label.placement = EdgeLabelPlacement::Head;
    graph.edges[dropped].labels.push(label);
    let original_target = graph.edges[dropped].target;
    let tail_edge = graph
        .edges
        .iter()
        .enumerate()
        .find(|(index, edge)| *index != long_edge && *index != dropped && edge.id == "A-B")
        .map(|(index, _)| index)
        .unwrap();
    graph.set_edge_target(tail_edge, original_target);
    graph.layerless_nodes[original_target.node].ports[original_target.port].incoming_edges =
        vec![dropped, tail_edge];

    join_long_edges(&mut graph);

    assert_eq!(graph.edges[long_edge].target, original_target);
    assert!(graph.edge_source_attached(long_edge));
    assert!(graph.edge_target_attached(long_edge));
    assert!(!graph.edge_source_attached(dropped));
    assert!(!graph.edge_target_attached(dropped));
    assert!(
        !graph
            .layers
            .iter()
            .any(|layer| layer.nodes.contains(&dummy))
    );
    assert_eq!(
        graph.edges[long_edge].bend_points,
        vec![
            LPoint { x: 10.0, y: 20.0 },
            LPoint { x: 50.0, y: 70.0 },
            LPoint { x: 90.0, y: 100.0 }
        ]
    );
    assert_eq!(graph.edges[long_edge].labels.len(), 1);
    assert!(graph.edges[dropped].labels.is_empty());
    assert_eq!(
        graph.layerless_nodes[original_target.node].ports[original_target.port].incoming_edges,
        vec![long_edge, tail_edge]
    );
}

#[test]
fn long_edge_joiner_work_and_order_are_linear_in_fanout() {
    for pair_count in [1usize, 8, 64] {
        let mut fixture = long_edge_join_fanout_fixture(pair_count);
        let mut work = LongEdgeJoinWork::default();
        let retained_layer_nodes = fixture.graph.layers[0].nodes.clone();

        join_long_edges_observed(&mut fixture.graph, &mut work);

        // The fixture has two ordinary source nodes per pair plus the dummy and target.
        assert_eq!(work.preflight_layer_nodes_visited, pair_count * 2 + 1);
        assert_eq!(work.layer_nodes_visited, pair_count * 2 + 2);
        assert_eq!(work.edge_pairs, pair_count);
        assert_eq!(work.indexed_ports_visited, pair_count * 2 + 3);
        assert_eq!(work.indexed_incoming_edges_visited, pair_count * 3);
        assert_eq!(work.fallback_incoming_edges_visited, 0);
        assert_eq!(work.target_slots_replaced, pair_count);
        assert_eq!(work.dummy_input_edges_removed, pair_count);
        assert_eq!(work.dummy_output_edges_removed, pair_count);

        let expected_incoming = fixture
            .target_slot_survivors
            .iter()
            .zip(&fixture.guard_edges)
            .flat_map(|(&surviving, &guard)| [surviving, guard])
            .collect::<Vec<_>>();
        assert_eq!(
            fixture.graph.layerless_nodes[fixture.target.node].ports[fixture.target.port]
                .incoming_edges,
            expected_incoming
        );
        assert!(
            !fixture
                .graph
                .layers
                .iter()
                .any(|layer| layer.nodes.contains(&fixture.dummy))
        );
        assert_eq!(fixture.graph.layers[0].nodes, retained_layer_nodes);

        for pair in &fixture.joined_pairs {
            let surviving = pair.surviving_edge;
            let dropped = pair.dropped_edge;
            assert_eq!(fixture.graph.edges[surviving].target, fixture.target);
            assert!(fixture.graph.edge_source_attached(surviving));
            assert!(fixture.graph.edge_target_attached(surviving));
            assert!(!fixture.graph.edge_source_attached(dropped));
            assert!(!fixture.graph.edge_target_attached(dropped));
            assert_eq!(
                fixture.graph.edges[surviving].bend_points,
                vec![
                    LPoint {
                        x: pair.surviving_index as f64,
                        y: 10.0,
                    },
                    LPoint { x: 50.0, y: 70.0 },
                    LPoint {
                        x: pair.dropped_index as f64,
                        y: 20.0,
                    },
                    LPoint {
                        x: pair.dropped_index as f64,
                        y: 30.0,
                    },
                ]
            );
            assert_eq!(
                fixture.graph.edges[surviving]
                    .labels
                    .iter()
                    .map(|label| label.text.clone())
                    .collect::<Vec<_>>(),
                vec![
                    format!("drop-{}-a", pair.dropped_index),
                    format!("drop-{}-b", pair.dropped_index),
                ]
            );
            assert!(fixture.graph.edges[dropped].labels.is_empty());
        }
    }
}

#[test]
fn long_edge_joiner_skips_global_index_when_no_dummy_exists() {
    let mut graph = graph(vec![node("A"), node("B")], vec![edge("A-B", "A", "B")]);
    layer_network_simplex(&mut graph).unwrap();
    let expected_layers = graph.layers.clone();
    let mut work = LongEdgeJoinWork::default();

    join_long_edges_observed(&mut graph, &mut work);

    assert_eq!(work.preflight_layer_nodes_visited, 2);
    assert_eq!(work.layer_nodes_visited, 0);
    assert_eq!(work.indexed_ports_visited, 0);
    assert_eq!(work.indexed_incoming_edges_visited, 0);
    assert_eq!(work.fallback_incoming_edges_visited, 0);
    assert_eq!(graph.layers, expected_layers);
}

#[test]
fn long_edge_joiner_recovers_exact_target_slots_from_a_stale_index() {
    let mut fixture = long_edge_join_fanout_fixture(8);
    let mut stale_positions = HashMap::new();
    let mut work = LongEdgeJoinWork::default();

    join_long_edge_at_observed(
        &mut fixture.graph,
        fixture.dummy,
        true,
        Some(&mut stale_positions),
        &mut work,
    );

    let target_incoming = &fixture.graph.layerless_nodes[fixture.target.node].ports
        [fixture.target.port]
        .incoming_edges;
    let expected_incoming = fixture
        .target_slot_survivors
        .iter()
        .zip(&fixture.guard_edges)
        .flat_map(|(&surviving, &guard)| [surviving, guard])
        .collect::<Vec<_>>();
    assert_eq!(target_incoming, &expected_incoming);
    assert!(work.fallback_incoming_edges_visited > 0);
}

#[test]
fn layer_constraint_preprocessor_hides_separate_nodes_before_layering() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::FirstSeparate);
    let mut graph = graph(
        vec![first, node("A"), node("B")],
        vec![edge("start-A", "start", "A"), edge("A-B", "A", "B")],
    );
    let edge_index = graph
        .edges
        .iter()
        .position(|edge| edge.id == "start-A")
        .unwrap();

    preprocess_layer_constraints(&mut graph).unwrap();

    let hidden = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "start")
        .unwrap();
    let a = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert!(graph.layerless_nodes[hidden].hidden);
    assert_eq!(graph.hidden_nodes, vec![hidden]);
    assert_eq!(
        graph.layerless_nodes[a].layer_constraint,
        LayerConstraint::None
    );
    assert!(
        !graph.edge_target_attached(edge_index),
        "preprocessor should detach the opposite target endpoint"
    );
    assert!(graph.edge_source_attached(edge_index));
}

#[test]
fn layer_constraint_preprocessor_constrains_isolated_opposite_node() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::FirstSeparate);
    let mut graph = graph(vec![first, node("A")], vec![edge("start-A", "start", "A")]);

    preprocess_layer_constraints(&mut graph).unwrap();

    let a = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert_eq!(
        graph.layerless_nodes[a].layer_constraint,
        LayerConstraint::First
    );
}

#[test]
fn layer_constraint_preprocessor_keeps_explicit_opposite_constraint() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::FirstSeparate);
    let mut explicit = node("A");
    explicit.layer_constraint = Some(LayerConstraint::None);
    let mut graph = graph(vec![first, explicit], vec![edge("start-A", "start", "A")]);

    preprocess_layer_constraints(&mut graph).unwrap();

    let a = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert!(graph.layerless_nodes[a].layer_constraint_explicit);
    assert_eq!(
        graph.layerless_nodes[a].layer_constraint,
        LayerConstraint::None
    );
}

#[test]
fn layer_constraint_preprocessor_keeps_node_connected_to_both_separate_sides_unconstrained() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::FirstSeparate);
    let mut last = node("end");
    last.layer_constraint = Some(LayerConstraint::LastSeparate);
    let mut graph = graph(
        vec![first, node("A"), last],
        vec![edge("start-A", "start", "A"), edge("A-end", "A", "end")],
    );

    preprocess_layer_constraints(&mut graph).unwrap();

    let a = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert_eq!(
        graph.layerless_nodes[a].layer_constraint,
        LayerConstraint::None
    );
}

#[test]
fn layer_constraint_postprocessor_restores_hidden_nodes_in_separate_layer() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::FirstSeparate);
    let mut graph = graph(
        vec![first, node("A"), node("B")],
        vec![edge("start-A", "start", "A"), edge("A-B", "A", "B")],
    );
    let edge_index = graph
        .edges
        .iter()
        .position(|edge| edge.id == "start-A")
        .unwrap();

    preprocess_layer_constraints(&mut graph).unwrap();
    layer_network_simplex(&mut graph).unwrap();
    postprocess_layer_constraints(&mut graph).unwrap();

    let start = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "start")
        .unwrap();
    let a = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    assert!(!graph.layerless_nodes[start].hidden);
    assert!(graph.hidden_nodes.is_empty());
    assert_eq!(graph.layerless_nodes[start].layer_index, Some(0));
    assert!(
        graph.layerless_nodes[a].layer_index.unwrap()
            > graph.layerless_nodes[start].layer_index.unwrap()
    );
    assert!(graph.edge_source_attached(edge_index));
    assert!(graph.edge_target_attached(edge_index));
}

#[test]
fn layer_constraint_postprocessor_restores_hidden_incoming_edge_source() {
    let mut last = node("end");
    last.layer_constraint = Some(LayerConstraint::LastSeparate);
    let mut graph = graph(vec![node("A"), last], vec![edge("A-end", "A", "end")]);
    let edge_index = graph
        .edges
        .iter()
        .position(|edge| edge.id == "A-end")
        .unwrap();

    preprocess_layer_constraints(&mut graph).unwrap();
    assert!(!graph.edge_source_attached(edge_index));
    assert!(graph.edge_target_attached(edge_index));

    layer_network_simplex(&mut graph).unwrap();
    postprocess_layer_constraints(&mut graph).unwrap();

    let end = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "end")
        .unwrap();
    assert_eq!(
        graph.layerless_nodes[end].layer_index,
        Some(graph.layers.len() - 1)
    );
    assert!(graph.edge_source_attached(edge_index));
    assert!(graph.edge_target_attached(edge_index));
}

#[test]
fn layer_constraint_preprocessor_rejects_invalid_separate_edges() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::FirstSeparate);
    let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);

    let err = preprocess_layer_constraints(&mut graph).unwrap_err();
    assert!(matches!(
        err,
        IntermediateError::FirstSeparateIncomingEdge { .. }
    ));
}

#[test]
fn layer_constraint_preprocessor_allows_external_port_incident_exception() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::FirstSeparate);
    let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);
    for node in &mut graph.layerless_nodes {
        node.kind = LNodeKind::ExternalPort;
    }

    preprocess_layer_constraints(&mut graph).unwrap();
}

#[test]
fn layer_constraint_postprocessor_rejects_invalid_first_incoming_edge() {
    let mut first = node("start");
    first.layer_constraint = Some(LayerConstraint::First);
    let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);
    layer_network_simplex(&mut graph).unwrap();

    let err = postprocess_layer_constraints(&mut graph).unwrap_err();
    assert!(matches!(err, IntermediateError::FirstIncomingEdge { .. }));
}

#[test]
fn layer_size_calculator_sets_layer_sizes_graph_height_and_vertical_offset() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes: vec![node("A"), node("B")],
        edges: Vec::new(),
    })
    .unwrap();
    graph.layerless_nodes[0].size.width = 30.0;
    graph.layerless_nodes[0].size.height = 10.0;
    graph.layerless_nodes[0].position.y = -5.0;
    graph.layerless_nodes[0].margin.top = 3.0;
    graph.layerless_nodes[0].margin.right = 7.0;
    graph.layerless_nodes[0].margin.bottom = 11.0;
    graph.layerless_nodes[0].margin.left = 5.0;
    graph.layerless_nodes[1].size.width = 40.0;
    graph.layerless_nodes[1].size.height = 20.0;
    graph.layerless_nodes[1].position.y = 25.0;
    graph.layerless_nodes[1].margin.top = 2.0;
    graph.layerless_nodes[1].margin.right = 3.0;
    graph.layerless_nodes[1].margin.bottom = 4.0;
    graph.layerless_nodes[1].margin.left = 1.0;
    graph.layers.push(crate::graph::Layer {
        nodes: vec![0, 1],
        size: Default::default(),
    });

    calculate_layer_sizes_and_graph_height(&mut graph);

    assert_eq!(graph.layers[0].size.width, 44.0);
    assert_eq!(graph.layers[0].size.height, 57.0);
    assert_eq!(graph.size.height, 57.0);
    assert_eq!(graph.offset.y, 8.0);
}

#[test]
fn layer_size_calculator_includes_surrounding_spacing_for_external_port_bounds() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes: vec![node("port")],
        edges: Vec::new(),
    })
    .unwrap();
    graph.options.spacing.ports_surrounding.top = 6.0;
    graph.options.spacing.ports_surrounding.bottom = 8.0;
    graph.layerless_nodes[0].kind = LNodeKind::ExternalPort;
    graph.layerless_nodes[0].size.height = 12.0;
    graph.layerless_nodes[0].position.y = 10.0;
    graph.layerless_nodes[0].margin.top = 1.0;
    graph.layerless_nodes[0].margin.bottom = 2.0;
    graph.layers.push(crate::graph::Layer {
        nodes: vec![0],
        size: Default::default(),
    });

    calculate_layer_sizes_and_graph_height(&mut graph);

    assert_eq!(graph.layers[0].size.height, 29.0);
    assert_eq!(graph.size.height, 29.0);
    assert_eq!(graph.offset.y, -3.0);
}

#[test]
fn hierarchical_port_constraint_processor_orders_east_west_dummies() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedOrder;
    let west_late = push_external_dummy(&mut graph, "west-late", PortSide::West, 0.8);
    let west_early = push_external_dummy(&mut graph, "west-early", PortSide::West, 0.2);
    let normal = push_normal_node(&mut graph, "A");

    graph.set_node_layer(west_late, 0);
    graph.set_node_layer(west_early, 0);
    graph.set_node_layer(normal, 0);

    process_hierarchical_port_constraints(&mut graph);

    assert_eq!(
        graph.layerless_nodes[west_early].in_layer_successor_constraints,
        vec![west_late]
    );
    assert!(
        graph.layerless_nodes[west_late]
            .in_layer_successor_constraints
            .is_empty()
    );
}

#[test]
fn hierarchical_port_constraint_processor_replaces_north_south_dummies() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedPos;
    let north = push_external_dummy(&mut graph, "north", PortSide::North, 5.0);
    let a = push_normal_node(&mut graph, "A");
    let b = push_normal_node(&mut graph, "B");

    graph.set_node_layer(a, 0);
    graph.set_node_layer(north, 0);
    graph.set_node_layer(b, 1);

    let in_edge = add_test_edge(&mut graph, "north-A", north, 0, a, 0);
    let out_edge = add_test_edge(&mut graph, "B-north", b, 0, north, 0);

    process_hierarchical_port_constraints(&mut graph);

    assert_eq!(graph.layerless_nodes[north].layer_index, None);
    assert_eq!(graph.replaced_external_port_dummies, vec![north]);
    let replacements = graph
        .layerless_nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (node.replaced_external_port_dummy == Some(north)).then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(replacements.len(), 2);
    assert_eq!(graph.edges[in_edge].source.node, replacements[0]);
    assert_eq!(graph.edges[in_edge].source.port, 1);
    assert_eq!(graph.edges[out_edge].target.node, replacements[1]);
    assert_eq!(graph.edges[out_edge].target.port, 0);
    assert!(graph.edge_source_attached(in_edge));
    assert!(graph.edge_target_attached(out_edge));
    assert_eq!(
        graph.layerless_nodes[replacements[0]].port_constraints,
        PortConstraints::FixedPos
    );
    assert_eq!(
        graph.layerless_nodes[replacements[0]].node_alignment,
        Alignment::Center
    );
}

#[test]
fn hierarchical_port_constraint_processor_reuses_north_south_replacements_by_origin_port() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedSide;
    let first = push_external_dummy(&mut graph, "north-a", PortSide::North, 0.0);
    let second = push_external_dummy(&mut graph, "north-b", PortSide::North, 0.0);
    let a = push_normal_node(&mut graph, "A");
    let b = push_normal_node(&mut graph, "B");

    let origin_port = PortRef { node: 42, port: 7 };
    let parent = LGraph::new("parent", LayeredOptions::default());
    graph.layerless_nodes[first].origin_port = Some(parent.scoped_port(origin_port));
    graph.layerless_nodes[second].origin_port = Some(parent.scoped_port(origin_port));

    graph.set_node_layer(first, 0);
    graph.set_node_layer(second, 0);
    graph.set_node_layer(a, 0);
    graph.set_node_layer(b, 0);

    let a_edge = add_test_edge(&mut graph, "north-a-A", first, 0, a, 0);
    let b_edge = add_test_edge(&mut graph, "north-b-B", second, 0, b, 0);

    process_hierarchical_port_constraints(&mut graph);

    let replacements = graph
        .layerless_nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (node.replaced_external_port_dummy == Some(first)
                || node.replaced_external_port_dummy == Some(second))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(replacements.len(), 1);
    assert_eq!(graph.edges[a_edge].source.node, replacements[0]);
    assert_eq!(graph.edges[b_edge].source.node, replacements[0]);
}

#[test]
fn hierarchical_port_constraint_processor_reuses_north_south_replacements_per_virtual_layer() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedSide;
    let north_source = push_external_dummy(&mut graph, "north-source", PortSide::North, 0.0);
    let north_target = push_external_dummy(&mut graph, "north-target", PortSide::North, 0.0);
    let middle = push_normal_node(&mut graph, "middle");
    let next = push_normal_node(&mut graph, "next");

    let origin_port = PortRef { node: 13, port: 5 };
    let parent = LGraph::new("parent", LayeredOptions::default());
    for dummy in [north_source, north_target] {
        graph.layerless_nodes[dummy].origin_port = Some(parent.scoped_port(origin_port));
    }

    graph.set_node_layer(next, 0);
    graph.set_node_layer(north_target, 0);
    graph.layers.push(crate::graph::Layer {
        nodes: vec![],
        size: Default::default(),
    });
    graph.set_node_layer(north_source, 2);
    graph.set_node_layer(middle, 2);

    let incoming_from_north = add_test_edge(
        &mut graph,
        "north-source-middle",
        north_source,
        0,
        middle,
        0,
    );
    let outgoing_to_north =
        add_test_edge(&mut graph, "next-north-target", next, 0, north_target, 0);

    process_hierarchical_port_constraints(&mut graph);

    let replacements = graph
        .layerless_nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (node.replaced_external_port_dummy == Some(north_source)
                || node.replaced_external_port_dummy == Some(north_target))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(replacements.len(), 1);
    assert_eq!(
        graph.edges[incoming_from_north].source.node,
        replacements[0]
    );
    assert_eq!(graph.edges[incoming_from_north].source.port, 1);
    assert_eq!(graph.edges[outgoing_to_north].target.node, replacements[0]);
    assert_eq!(graph.edges[outgoing_to_north].target.port, 0);
}

#[test]
fn hierarchical_port_constraint_processor_copies_replacement_dummy_metadata() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedSide;
    let north = push_external_dummy(&mut graph, "north", PortSide::North, 0.42);
    let target = push_normal_node(&mut graph, "target");
    let parent = LGraph::new("parent", LayeredOptions::default());
    let origin = parent.scoped_port(PortRef { node: 9, port: 3 });
    graph.layerless_nodes[north].origin_port = Some(origin.clone());
    graph.layerless_nodes[north].model_order = Some(17);
    graph.layerless_nodes[north].in_layer_constraint = InLayerConstraint::Top;

    graph.set_node_layer(north, 0);
    graph.set_node_layer(target, 0);
    add_test_edge(&mut graph, "north-target", north, 0, target, 0);

    process_hierarchical_port_constraints(&mut graph);

    let replacement = graph
        .layerless_nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            (node.replaced_external_port_dummy == Some(north)).then_some(index)
        })
        .unwrap();
    let node = &graph.layerless_nodes[replacement];
    assert_eq!(node.port_ratio_or_position, 0.42);
    assert_eq!(node.origin_port, Some(origin));
    assert_eq!(node.model_order, Some(17));
    assert_eq!(node.in_layer_constraint, InLayerConstraint::Top);
    assert_eq!(node.port_constraints, PortConstraints::FixedPos);
    assert_eq!(node.node_alignment, Alignment::Center);
    assert_eq!(node.ports.len(), 2);
}

#[test]
fn hierarchical_port_dummy_size_processor_sizes_north_south_dummies_per_layer() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.spacing.edge_edge_between_layers = 7.0;

    let north_a = push_replaced_north_south_dummy(&mut graph, "north-a", PortSide::North);
    let north_b = push_replaced_north_south_dummy(&mut graph, "north-b", PortSide::North);
    let south_a = push_replaced_north_south_dummy(&mut graph, "south-a", PortSide::South);
    let south_b = push_replaced_north_south_dummy(&mut graph, "south-b", PortSide::South);

    graph.set_node_layer(north_a, 0);
    graph.set_node_layer(north_b, 0);
    graph.set_node_layer(south_a, 0);
    graph.set_node_layer(south_b, 0);

    process_hierarchical_port_dummy_sizes(&mut graph);

    assert_eq!(
        graph.layerless_nodes[north_a].node_alignment,
        Alignment::Center
    );
    assert_eq!(
        graph.layerless_nodes[north_b].node_alignment,
        Alignment::Center
    );
    assert_eq!(
        graph.layerless_nodes[south_a].node_alignment,
        Alignment::Center
    );
    assert_eq!(
        graph.layerless_nodes[south_b].node_alignment,
        Alignment::Center
    );
    assert_eq!(graph.layerless_nodes[north_a].size.width, 0.0);
    assert_eq!(graph.layerless_nodes[north_b].size.width, 14.0);
    assert_eq!(graph.layerless_nodes[south_a].size.width, 14.0);
    assert_eq!(graph.layerless_nodes[south_b].size.width, 0.0);
    assert_eq!(graph.layerless_nodes[north_a].ports[0].position.x, 0.0);
    assert_eq!(graph.layerless_nodes[north_b].ports[0].position.x, 0.0);
    assert_eq!(graph.layerless_nodes[north_b].ports[1].position.x, 14.0);
    assert_eq!(graph.layerless_nodes[south_a].ports[1].position.x, 14.0);
}

#[test]
fn hierarchical_port_position_processor_sets_fixed_ratio_east_west_y_coordinates() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedRatio;
    graph.size.height = 100.0;
    graph.padding.top = 10.0;
    graph.padding.bottom = 20.0;
    graph.offset.y = 7.0;

    let west = push_external_dummy(&mut graph, "west", PortSide::West, 0.25);
    let east = push_external_dummy(&mut graph, "east", PortSide::East, 0.5);
    let north = push_external_dummy(&mut graph, "north", PortSide::North, 0.75);
    graph.layerless_nodes[west].ports[0].position.y = 3.0;
    graph.layerless_nodes[east].ports[0].position.y = 4.0;
    graph.layerless_nodes[north].ports[0].position.y = 5.0;

    graph.set_node_layer(west, 0);
    graph.set_node_layer(north, 0);
    graph.set_node_layer(east, 1);

    process_hierarchical_port_positions(&mut graph);

    assert_eq!(graph.layerless_nodes[west].position.y, 12.5);
    assert_eq!(graph.layerless_nodes[east].position.y, 44.0);
    assert_eq!(graph.layerless_nodes[north].position.y, 0.0);
}

#[test]
fn hierarchical_port_position_processor_uses_port_position_for_east_west_dummies() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedRatio;
    graph.size.height = 100.0;

    let west = push_external_dummy(&mut graph, "west", PortSide::West, 0.25);
    graph.layerless_nodes[west].ports[0].position.y = 7.0;
    graph.set_node_layer(west, 0);

    process_hierarchical_port_positions(&mut graph);

    assert_eq!(graph.layerless_nodes[west].position.y, 18.0);
}

#[test]
fn hierarchical_port_position_processor_sets_fixed_pos_only_on_border_layers() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedPos;
    graph.padding.top = 6.0;
    graph.offset.y = -2.0;

    let first = push_external_dummy(&mut graph, "first", PortSide::West, 30.0);
    let middle = push_external_dummy(&mut graph, "middle", PortSide::West, 90.0);
    let last = push_external_dummy(&mut graph, "last", PortSide::East, 50.0);
    graph.layerless_nodes[first].ports[0].position.y = 1.0;
    graph.layerless_nodes[middle].ports[0].position.y = 2.0;
    graph.layerless_nodes[last].ports[0].position.y = 3.0;

    graph.set_node_layer(first, 0);
    graph.set_node_layer(middle, 1);
    graph.set_node_layer(last, 2);

    process_hierarchical_port_positions(&mut graph);

    assert_eq!(graph.layerless_nodes[first].position.y, 25.0);
    assert_eq!(graph.layerless_nodes[middle].position.y, 0.0);
    assert_eq!(graph.layerless_nodes[last].position.y, 43.0);
}

#[test]
fn hierarchical_port_orthogonal_router_tail_fixes_east_west_coordinates_and_slanted_segments() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedPos;
    graph.size.width = 100.0;
    graph.size.height = 40.0;
    graph.padding.left = 5.0;
    graph.padding.right = 7.0;
    graph.padding.top = 3.0;
    graph.padding.bottom = 11.0;
    graph.offset.x = 2.0;
    graph.offset.y = 4.0;

    let west = push_external_dummy(&mut graph, "west", PortSide::West, 22.0);
    let east = push_external_dummy(&mut graph, "east", PortSide::East, 70.0);
    let middle = push_normal_node(&mut graph, "A");
    graph.layerless_nodes[west].external_port_size.height = 6.0;
    graph.layerless_nodes[east].external_port_size.height = 8.0;
    graph.layerless_nodes[west].ports[0].position.y = 2.0;
    graph.layerless_nodes[east].ports[0].position.y = 5.0;

    graph.set_node_layer(west, 0);
    graph.set_node_layer(middle, 1);
    graph.set_node_layer(east, 2);

    let west_edge = add_test_edge(&mut graph, "west-A", west, 0, middle, 0);
    graph.edges[west_edge].bend_points = vec![LPoint { x: 10.0, y: 999.0 }];
    let east_edge = add_test_edge(&mut graph, "A-east", middle, 0, east, 0);
    graph.edges[east_edge].bend_points = vec![LPoint { x: 90.0, y: 999.0 }];

    process_hierarchical_port_orthogonal_edges(&mut graph);

    assert_eq!(graph.layerless_nodes[west].position.x, -7.0);
    assert_eq!(graph.layerless_nodes[east].position.x, 105.0);
    assert_eq!(graph.layerless_nodes[west].position.y, 13.0);
    assert_eq!(graph.layerless_nodes[east].position.y, 58.0);
    assert_eq!(graph.size.height, 59.0);
    assert_eq!(graph.edges[west_edge].bend_points[0].y, 15.0);
    assert_eq!(graph.edges[east_edge].bend_points[0].y, 63.0);
}

#[test]
fn hierarchical_port_orthogonal_router_tail_sets_north_south_border_y_coordinates() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.size.height = 80.0;
    graph.padding.top = 6.0;
    graph.padding.bottom = 9.0;
    graph.offset.y = 4.0;

    let north = push_replaced_north_south_dummy(&mut graph, "north", PortSide::North);
    let south = push_replaced_north_south_dummy(&mut graph, "south", PortSide::South);
    graph.set_node_layer(north, 0);
    graph.set_node_layer(south, 1);

    process_hierarchical_port_orthogonal_edges(&mut graph);

    assert_eq!(graph.layerless_nodes[north].position.y, -10.0);
    assert_eq!(graph.layerless_nodes[south].position.y, 85.0);
}

#[test]
fn hierarchical_port_orthogonal_router_restores_and_removes_north_dummy_replacements() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedSide;
    graph.options.spacing.node_node = 20.0;
    graph.options.spacing.edge_edge = 10.0;
    graph.size.height = 50.0;

    let north = push_external_dummy(&mut graph, "north", PortSide::North, 0.0);
    graph.layerless_nodes[north].size.width = 4.0;
    graph.layerless_nodes[north].ports[0].position = LPoint { x: 2.0, y: 0.0 };
    let source = push_normal_node(&mut graph, "A");
    graph.layerless_nodes[source].position = LPoint { x: 50.0, y: 30.0 };
    let source_port = graph
        .add_port(
            source,
            PortType::Output,
            PortSide::North,
            LPoint { x: 30.0, y: 0.0 },
        )
        .unwrap();

    graph.set_node_layer(source, 0);
    graph.set_node_layer(north, 0);
    let edge = graph
        .add_edge(LayeredEdge {
            id: "A-north".to_string(),
            source: source_port,
            target: PortRef {
                node: north,
                port: 0,
            },
            source_node_id: "A".to_string(),
            target_node_id: "north".to_string(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        })
        .unwrap();

    process_hierarchical_port_constraints(&mut graph);
    let replacement = graph
        .layerless_nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            (node.replaced_external_port_dummy == Some(north)).then_some(index)
        })
        .unwrap();
    process_hierarchical_port_orthogonal_edges(&mut graph);

    assert_eq!(graph.layerless_nodes[north].layer_index, Some(1));
    assert_eq!(graph.layerless_nodes[replacement].layer_index, None);
    assert_eq!(graph.layerless_nodes[north].ports[0].side, PortSide::South);
    assert_eq!(graph.edges[edge].target.node, north);
    assert_eq!(graph.edges[edge].target.port, 0);
    assert!(graph.edge_source_attached(edge));
    assert!(graph.edge_target_attached(edge));
    let detached_edge = graph
        .edges
        .iter()
        .position(|edge| edge.id.ends_with(":hierarchical-origin"))
        .unwrap();
    assert_eq!(
        graph.edges[detached_edge].thickness,
        ELK_DEFAULT_EDGE_THICKNESS
    );
    assert!(!graph.edge_source_attached(detached_edge));
    assert!(!graph.edge_target_attached(detached_edge));
}

#[test]
fn hierarchical_port_orthogonal_router_places_fixed_ratio_north_south_dummies() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedRatio;
    graph.size.width = 100.0;
    graph.padding.left = 10.0;
    graph.padding.right = 30.0;
    graph.offset.x = 4.0;

    let north = push_external_dummy(&mut graph, "north", PortSide::North, 0.25);
    let south = push_external_dummy(&mut graph, "south", PortSide::South, 0.5);
    graph.layerless_nodes[north].ports[0].position.x = 2.0;
    graph.layerless_nodes[south].ports[0].position.x = 3.0;
    graph.replaced_external_port_dummies = vec![north, south];
    graph.layers.push(crate::graph::Layer {
        nodes: vec![],
        size: Default::default(),
    });

    process_hierarchical_port_orthogonal_edges(&mut graph);

    assert_eq!(graph.layerless_nodes[north].position.x, 19.0);
    assert_eq!(graph.layerless_nodes[south].position.x, 53.0);
}

#[test]
fn hierarchical_port_orthogonal_router_routes_fixed_pos_north_edges_above_graph() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedPos;
    graph.options.spacing.node_node = 20.0;
    graph.options.spacing.edge_edge = 10.0;
    graph.size.height = 50.0;

    let north = push_external_dummy(&mut graph, "north", PortSide::North, 5.0);
    graph.layerless_nodes[north].size.width = 4.0;
    graph.layerless_nodes[north].ports[0].position = LPoint { x: 2.0, y: 0.0 };
    let source = push_normal_node(&mut graph, "A");
    let source_port = graph
        .add_port(
            source,
            PortType::Output,
            PortSide::North,
            LPoint { x: 30.0, y: 0.0 },
        )
        .unwrap();

    graph.set_node_layer(source, 0);
    graph.set_node_layer(north, 0);
    graph
        .add_edge(LayeredEdge {
            id: "A-north".to_string(),
            source: source_port,
            target: PortRef {
                node: north,
                port: 0,
            },
            source_node_id: "A".to_string(),
            target_node_id: "north".to_string(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        })
        .unwrap();

    process_hierarchical_port_constraints(&mut graph);
    process_hierarchical_port_orthogonal_edges(&mut graph);

    assert_eq!(graph.offset.y, 20.0);
    assert!(graph.size.height > 50.0);
    let origin_edge = graph
        .edges
        .iter()
        .find(|edge| edge.id.ends_with(":hierarchical-origin"))
        .unwrap();
    assert!(!origin_edge.bend_points.is_empty());
}

#[test]
fn hierarchical_port_orthogonal_router_restores_fixed_order_north_south_spacing() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.port_constraints = PortConstraints::FixedOrder;
    graph.options.spacing.port_port = 7.0;

    let late = push_external_dummy(&mut graph, "late", PortSide::North, 0.8);
    let early = push_external_dummy(&mut graph, "early", PortSide::North, 0.2);
    graph.layerless_nodes[late].position.x = 0.0;
    graph.layerless_nodes[early].position.x = 0.0;
    graph.layerless_nodes[late].size.width = 5.0;
    graph.layerless_nodes[early].size.width = 5.0;
    graph.replaced_external_port_dummies = vec![late, early];
    graph.layers.push(crate::graph::Layer {
        nodes: vec![],
        size: Default::default(),
    });

    process_hierarchical_port_orthogonal_edges(&mut graph);

    assert_eq!(graph.layerless_nodes[early].position.x, 0.0);
    assert_eq!(graph.layerless_nodes[late].position.x, 12.0);
}

fn push_replaced_north_south_dummy(graph: &mut LGraph, id: &str, external_side: PortSide) -> usize {
    let node = graph.layerless_nodes.len();
    let mut dummy = LNode::new(id, 0.0, 0.0, None);
    dummy.kind = LNodeKind::ExternalPort;
    dummy.external_port_side = external_side;
    graph.layerless_nodes.push(dummy);
    graph
        .add_port(node, PortType::Input, PortSide::West, Default::default())
        .unwrap();
    graph
        .add_port(node, PortType::Output, PortSide::East, Default::default())
        .unwrap();
    node
}

fn push_external_dummy(
    graph: &mut LGraph,
    id: &str,
    external_side: PortSide,
    ratio_or_position: f64,
) -> usize {
    let node = graph.layerless_nodes.len();
    let mut dummy = LNode::new(id, 0.0, 0.0, None);
    dummy.kind = LNodeKind::ExternalPort;
    dummy.external_port_side = external_side;
    dummy.port_ratio_or_position = ratio_or_position;
    graph.layerless_nodes.push(dummy);
    graph
        .add_port(node, PortType::Input, PortSide::West, Default::default())
        .unwrap();
    node
}

fn push_normal_node(graph: &mut LGraph, id: &str) -> usize {
    let node = graph.layerless_nodes.len();
    graph.layerless_nodes.push(LNode::new(id, 80.0, 40.0, None));
    graph
        .add_port(node, PortType::Input, PortSide::West, Default::default())
        .unwrap();
    node
}

fn add_test_edge(
    graph: &mut LGraph,
    id: &str,
    source_node: usize,
    source_port: usize,
    target_node: usize,
    target_port: usize,
) -> usize {
    graph
        .add_edge(LayeredEdge {
            id: id.to_string(),
            source: PortRef {
                node: source_node,
                port: source_port,
            },
            target: PortRef {
                node: target_node,
                port: target_port,
            },
            source_node_id: graph.layerless_nodes[source_node].id.clone(),
            target_node_id: graph.layerless_nodes[target_node].id.clone(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        })
        .unwrap()
}

struct LongEdgeJoinFanoutFixture {
    graph: LGraph,
    dummy: usize,
    target: PortRef,
    joined_pairs: Vec<ExpectedLongEdgeJoinPair>,
    target_slot_survivors: Vec<usize>,
    guard_edges: Vec<usize>,
}

struct ExpectedLongEdgeJoinPair {
    surviving_edge: usize,
    dropped_edge: usize,
    surviving_index: usize,
    dropped_index: usize,
}

fn long_edge_join_fanout_fixture(pair_count: usize) -> LongEdgeJoinFanoutFixture {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.options.unnecessary_bendpoints = true;

    let dummy = graph.layerless_nodes.len();
    let mut dummy_node = LNode::new("long-edge", 0.0, 0.0, None);
    dummy_node.kind = LNodeKind::LongEdge;
    dummy_node.position = LPoint { x: 50.0, y: 70.0 };
    graph.layerless_nodes.push(dummy_node);
    let dummy_input = graph
        .add_port(dummy, PortType::Input, PortSide::West, LPoint::default())
        .unwrap();
    let dummy_output = graph
        .add_port(dummy, PortType::Output, PortSide::East, LPoint::default())
        .unwrap();

    let target_node = push_normal_node(&mut graph, "target");
    let target = PortRef {
        node: target_node,
        port: 0,
    };

    let mut surviving_edges = Vec::with_capacity(pair_count);
    let mut dropped_edges = Vec::with_capacity(pair_count);
    let mut guard_edges = Vec::with_capacity(pair_count);
    for pair_index in 0..pair_count {
        let source = push_output_node(&mut graph, &format!("source-{pair_index}"));
        let guard_source = push_output_node(&mut graph, &format!("guard-source-{pair_index}"));
        graph.set_node_layer(source, 0);
        graph.set_node_layer(guard_source, 0);

        let surviving = add_test_edge(
            &mut graph,
            &format!("surviving-{pair_index}"),
            source,
            0,
            dummy_input.node,
            dummy_input.port,
        );
        graph.edges[surviving].bend_points.push(LPoint {
            x: pair_index as f64,
            y: 10.0,
        });
        surviving_edges.push(surviving);

        let dropped = add_test_edge(
            &mut graph,
            &format!("dropped-{pair_index}"),
            dummy_output.node,
            dummy_output.port,
            target.node,
            target.port,
        );
        graph.edges[dropped].bend_points.extend([
            LPoint {
                x: pair_index as f64,
                y: 20.0,
            },
            LPoint {
                x: pair_index as f64,
                y: 30.0,
            },
        ]);
        graph.edges[dropped]
            .labels
            .push(LLabel::new(format!("drop-{pair_index}-a"), 10.0, 5.0));
        graph.edges[dropped]
            .labels
            .push(LLabel::new(format!("drop-{pair_index}-b"), 10.0, 5.0));
        dropped_edges.push(dropped);

        let guard = add_test_edge(
            &mut graph,
            &format!("guard-{pair_index}"),
            guard_source,
            0,
            target.node,
            target.port,
        );
        guard_edges.push(guard);
    }

    let mut input_order = surviving_edges
        .iter()
        .copied()
        .enumerate()
        .collect::<Vec<_>>();
    input_order.reverse();
    let mut output_order = dropped_edges
        .iter()
        .copied()
        .enumerate()
        .collect::<Vec<_>>();
    output_order.rotate_left(pair_count / 3);
    graph.layerless_nodes[dummy_input.node].ports[dummy_input.port].incoming_edges =
        input_order.iter().map(|(_, edge)| *edge).collect();
    graph.layerless_nodes[dummy_output.node].ports[dummy_output.port].outgoing_edges =
        output_order.iter().map(|(_, edge)| *edge).collect();

    let mut target_slot_survivors = vec![usize::MAX; pair_count];
    let joined_pairs = input_order
        .into_iter()
        .zip(output_order)
        .map(
            |((surviving_index, surviving_edge), (dropped_index, dropped_edge))| {
                target_slot_survivors[dropped_index] = surviving_edge;
                ExpectedLongEdgeJoinPair {
                    surviving_edge,
                    dropped_edge,
                    surviving_index,
                    dropped_index,
                }
            },
        )
        .collect();

    graph.set_node_layer(dummy, 1);
    graph.set_node_layer(target.node, 2);

    LongEdgeJoinFanoutFixture {
        graph,
        dummy,
        target,
        joined_pairs,
        target_slot_survivors,
        guard_edges,
    }
}

fn push_output_node(graph: &mut LGraph, id: &str) -> usize {
    let node = graph.layerless_nodes.len();
    graph.layerless_nodes.push(LNode::new(id, 80.0, 40.0, None));
    graph
        .add_port(node, PortType::Output, PortSide::East, LPoint::default())
        .unwrap();
    node
}
