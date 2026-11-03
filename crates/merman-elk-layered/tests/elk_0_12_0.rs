//! Public-boundary regressions captured by `elkjs_0_12_0_oracle.cjs` from elkjs 0.12.0.
//! Worker SHA-256: e4f5c51c6f2fd564bb64e52e81c4a85a316ef2f24f8b90e1db5eb886c765b030.
//! Source comparison: Eclipse ELK 323312916048544e4e594eab3303cdb0d01e2929.

use merman_elk_layered::algorithms::{box_layout, mrtree};
use merman_elk_layered::*;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual={actual}, expected={expected}"
    );
}

fn node(index: usize, width: f64, height: f64) -> ElkInputNode {
    ElkInputNode {
        id: format!("n{index}"),
        width,
        height,
        parent: None,
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_constraints: None,
        port_alignment: None,
        node_label_placement: NodeLabelPlacement::Fixed,
        node_flexibility: NodeFlexibility::None,
        ports_surrounding: None,
        nested_options: None,
        label: None,
    }
}

fn input(sizes: &[[f64; 2]], edges: &[(usize, usize)], options: LayeredOptions) -> ElkInputGraph {
    ElkInputGraph {
        id: "root".into(),
        nodes: sizes
            .iter()
            .enumerate()
            .map(|(index, &[width, height])| node(index, width, height))
            .collect(),
        edges: edges
            .iter()
            .enumerate()
            .map(|(index, &(source, target))| ElkInputEdge {
                id: format!("e{index}"),
                source: format!("n{source}"),
                target: format!("n{target}"),
                labels: Vec::new(),
                minlen: 1,
                inside_self_loops_yo: false,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            })
            .collect(),
        options,
    }
}

#[test]
fn box_padding_changes_row_width_without_reordering_equal_area_nodes() {
    let rectangles = [box_layout::Rectangle {
        width: 40.0,
        height: 20.0,
        ..Default::default()
    }; 6];
    let result =
        box_layout::layout(&rectangles, &Default::default(), &mut NoopWorkControl).unwrap();
    assert_eq!((result.width, result.height), (125.0, 120.0));
    let positions: Vec<_> = result
        .rectangles
        .iter()
        .map(|node| (node.x, node.y))
        .collect();
    assert_eq!(
        positions,
        [
            (15.0, 15.0),
            (70.0, 15.0),
            (15.0, 50.0),
            (70.0, 50.0),
            (15.0, 85.0),
            (70.0, 85.0)
        ]
    );
}

#[test]
fn mrtree_uses_graph_padding_after_recomputing_component_bounds() {
    let nodes: Vec<_> = [
        [40.0, 20.0],
        [30.0, 50.0],
        [70.0, 30.0],
        [20.0, 20.0],
        [45.0, 25.0],
    ]
    .into_iter()
    .map(|[width, height]| mrtree::Node {
        width,
        height,
        ..Default::default()
    })
    .collect();
    let edges = [
        mrtree::Edge {
            source: 0,
            target: 1,
        },
        mrtree::Edge {
            source: 2,
            target: 3,
        },
    ];
    let output = mrtree::layout(&nodes, &edges, &Default::default(), &mut NoopWorkControl).unwrap();
    assert_eq!((output.width, output.height), (157.5, 177.5));
    for (actual, [x, y]) in output.nodes.iter().zip([
        [20.0, 20.0],
        [25.0, 60.0],
        [80.0, 20.0],
        [105.0, 70.0],
        [20.0, 130.0],
    ]) {
        close(actual.x, x);
        close(actual.y, y);
    }
    for (actual, expected) in output.edges.iter().zip([
        [[40.0, 40.0], [40.0, 43.0], [40.0, 60.0]],
        [[115.0, 50.0], [115.0, 53.0], [115.0, 70.0]],
    ]) {
        let route = actual.as_ref().unwrap();
        assert_eq!(route.len(), expected.len());
        for (actual, [x, y]) in route.iter().zip(expected) {
            close(actual.x, x);
            close(actual.y, y);
        }
    }
}

#[test]
fn model_order_groups_parallel_edges_using_each_edges_own_order() {
    let input = input(
        &[[40.0, 20.0]; 3],
        &[(0, 2), (0, 1), (0, 2)],
        LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            crossing_minimization_strategy: CrossingMinimizationStrategy::None,
            separate_connected_components: false,
            ..Default::default()
        },
    );
    let mut graph = import_graph(&input).unwrap();
    execute_processors_until_processor(&mut graph, ProcessorKind::SortByInputModelProcessor)
        .unwrap();
    let order: Vec<_> = graph.layerless_nodes[0]
        .ports
        .iter()
        .flat_map(|port| &port.outgoing_edges)
        .map(|&edge| graph.edges[edge].id.as_str())
        .collect();
    assert_eq!(order, ["e0", "e2", "e1"]);
}

#[test]
fn ordinary_layered_geometry_matches_target_runtime_in_all_directions() {
    let sizes = [
        [40.0, 20.0],
        [30.0, 50.0],
        [70.0, 30.0],
        [20.0, 20.0],
        [45.0, 25.0],
    ];
    let edges = [(0, 1), (0, 2), (1, 3), (2, 4)];
    for (direction, extent, positions) in [
        (
            ElkDirection::Down,
            [144.0, 159.0],
            [
                [13.666666666666666, 12.0],
                [12.0, 52.0],
                [62.0, 62.0],
                [17.0, 122.0],
                [74.5, 122.0],
            ],
        ),
        (
            ElkDirection::Up,
            [144.0, 159.0],
            [
                [13.666666666666666, 127.0],
                [12.0, 57.0],
                [62.0, 67.0],
                [17.0, 17.0],
                [74.5, 12.0],
            ],
        ),
        (
            ElkDirection::Right,
            [219.0, 124.0],
            [
                [12.0, 30.333333333333336],
                [92.0, 12.0],
                [72.0, 82.0],
                [162.0, 27.0],
                [162.0, 84.5],
            ],
        ),
        (
            ElkDirection::Left,
            [219.0, 124.0],
            [
                [167.0, 30.333333333333336],
                [97.0, 12.0],
                [77.0, 82.0],
                [37.0, 27.0],
                [12.0, 84.5],
            ],
        ),
    ] {
        let input = input(
            &sizes,
            &edges,
            LayeredOptions {
                direction,
                consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
                ..Default::default()
            },
        );
        let mut graph = import_graph(&input).unwrap();
        execute_ported_processors(&mut graph).unwrap();
        let size = graph.exported_root_size();
        close(size.width, extent[0]);
        close(size.height, extent[1]);
        for (node, [x, y]) in graph
            .layerless_nodes
            .iter()
            .take(sizes.len())
            .zip(positions)
        {
            close(node.position.x + graph.padding.left + graph.offset.x, x);
            close(node.position.y + graph.padding.top + graph.offset.y, y);
        }
    }
}

#[test]
fn one_sided_self_loop_clears_inline_mode_and_preserves_target_label_geometry() {
    let mut input = input(
        &[[80.0, 40.0]],
        &[(0, 0)],
        LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            ..Default::default()
        },
    );
    input.edges[0]
        .labels
        .push(ElkInputLabel::center("loop", 60.0, 18.0));
    let mut graph = import_graph(&input).unwrap();
    execute_ported_processors(&mut graph).unwrap();
    let size = graph.exported_root_size();
    assert_eq!((size.width, size.height), (104.0, 94.0));
    let label = &graph.edges[0].labels[0];
    assert!(!label.inline);
    close(label.position.x + graph.padding.left + graph.offset.x, 22.0);
    close(label.position.y + graph.padding.top + graph.offset.y, 12.0);
}

#[test]
fn crossing_influence_alone_retains_imported_model_order() {
    for (node_influence, port_influence) in [(1.0, 0.0), (0.0, 1.0)] {
        let input = input(
            &[[40.0, 20.0]; 2],
            &[(0, 1)],
            LayeredOptions {
                consider_model_order_crossing_counter_node_influence: node_influence,
                consider_model_order_crossing_counter_port_influence: port_influence,
                ..Default::default()
            },
        );
        let graph = import_graph(&input).unwrap();
        assert_eq!(graph.max_model_order_nodes, 2);
        assert_eq!(graph.layerless_nodes[0].model_order, Some(0));
        assert_eq!(graph.layerless_nodes[1].model_order, Some(1));
        assert_eq!(graph.edges[0].model_order, Some(0));
    }
}

#[test]
fn opposing_self_loop_centers_inline_label_on_the_outer_segment() {
    let mut input = input(
        &[[80.0, 40.0]],
        &[(0, 0)],
        LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            ..Default::default()
        },
    );
    input.nodes[0].port_constraints = Some(PortConstraints::FixedSide);
    input.edges[0]
        .labels
        .push(ElkInputLabel::center("loop", 60.0, 18.0));
    let mut graph = import_graph(&input).unwrap();
    let source = graph.edges[0].source;
    let target = graph.edges[0].target;
    graph.layerless_nodes[0].ports[source.port].side = PortSide::East;
    graph.layerless_nodes[0].ports[target.port].side = PortSide::West;
    graph.sync_graph_properties_to_options();
    execute_ported_processors(&mut graph).unwrap();
    let size = graph.exported_root_size();
    assert_eq!((size.width, size.height), (124.0, 92.0));
    let label = &graph.edges[0].labels[0];
    assert!(label.inline);
    close(label.position.x + graph.padding.left + graph.offset.x, 32.0);
    close(label.position.y + graph.padding.top + graph.offset.y, 62.0);
    for (point, [x, y]) in graph.edges[0].bend_points.iter().zip([
        [112.0, 32.0],
        [112.0, 71.0],
        [12.0, 71.0],
        [12.0, 32.0],
    ]) {
        close(point.x + graph.padding.left + graph.offset.x, x);
        close(point.y + graph.padding.top + graph.offset.y, y);
    }
    assert_eq!(graph.edges[0].bend_points.len(), 4);
}
