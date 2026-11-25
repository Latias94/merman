use super::*;
use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
use crate::intermediate::split_long_edges;
use crate::options::{ElkDirection, HierarchyHandling, LayeredOptions};
use crate::p2layers::layer_network_simplex;
use crate::p3order::{process_port_sides, sort_port_lists};

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
        node_label_placement: crate::options::NodeLabelPlacement::Fixed,
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

fn nested_node(id: &str, parent: Option<&str>) -> ElkInputNode {
    ElkInputNode {
        id: id.to_string(),
        width: 54.0,
        height: 68.0,
        parent: parent.map(str::to_string),
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

fn nested_group(id: &str, parent: Option<&str>) -> ElkInputNode {
    ElkInputNode {
        id: id.to_string(),
        width: 0.0,
        height: 0.0,
        parent: parent.map(str::to_string),
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_alignment: None,
        port_constraints: None,
        node_label_placement: crate::options::NodeLabelPlacement::InsideTopCenter,
        node_flexibility: crate::options::NodeFlexibility::None,
        ports_surrounding: None,
        nested_options: Some(Box::new(crate::LayeredOptions {
            spacing: crate::SpacingOptions::layered_base_value(30.0),
            ..Default::default()
        })),
        label: None,
    }
}

fn prepared_graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> LGraph {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes,
        edges,
    })
    .unwrap();
    layer_network_simplex(&mut graph).unwrap();
    split_long_edges(&mut graph);
    process_port_sides(&mut graph);
    sort_port_lists(&mut graph);
    graph
}

fn node_order_by_id(graph: &LGraph, layer: &[usize]) -> Vec<String> {
    layer
        .iter()
        .map(|node| graph.layerless_nodes[*node].id.clone())
        .collect()
}

#[test]
fn complete_dag_randomizes_after_the_shared_initial_order_attempt() {
    // Pinned elkjs 0.9.3, seed 1: both property names have the same ID. Giving
    // SECOND_TRY a distinct ID instead leaves this graph at four crossings.
    for hierarchical in [false, true] {
        let nodes = (0..6).map(|i| node(&format!("n{i}"))).collect();
        let mut edges = Vec::new();
        for source in 0..6 {
            for target in source + 1..6 {
                edges.push(edge(
                    &format!("e{source}{target}"),
                    &format!("n{source}"),
                    &format!("n{target}"),
                ));
            }
        }
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".into(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        })
        .unwrap();
        crate::pipeline::execute_processors_until_processor(
            &mut graph,
            crate::pipeline::ProcessorKind::SortByInputModelProcessor,
        )
        .unwrap();

        if hierarchical {
            let trace = debug_crossings_layer_sweep_hierarchical_with_type(
                &mut graph,
                CrossMinType::Barycenter,
            )
            .unwrap();
            assert_eq!(
                trace
                    .runs
                    .iter()
                    .map(|run| run.crossings)
                    .collect::<Vec<_>>(),
                vec![4.0, 4.0, 4.0, 3.0, 3.0, 3.0, 3.0],
            );
            assert!(trace.runs[0].first_try_with_initial_order);
            assert!(
                trace.runs[1..]
                    .iter()
                    .all(|run| !run.first_try_with_initial_order)
            );
        } else {
            minimize_crossings_layer_sweep(&mut graph);
        }

        assert_eq!(CrossingsCounter::new().count_all_crossings(&graph), 3);
        let layer_order = graph.layers[2]
            .nodes
            .iter()
            .map(|&index| {
                let node = &graph.layerless_nodes[index];
                if node.kind == LNodeKind::Normal {
                    node.id.as_str()
                } else {
                    let edge = node
                        .ports
                        .iter()
                        .flat_map(|port| &port.outgoing_edges)
                        .next()
                        .unwrap();
                    graph.edges[*edge].id.as_str()
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            layer_order,
            ["e03", "e15", "e13", "e14", "n2", "e04", "e05"]
        );
    }
}

#[test]
fn hierarchical_sweep_advances_graph_random_streams_without_order_changes() {
    for cross_min_type in [
        CrossMinType::Barycenter,
        CrossMinType::OneSidedGreedySwitch,
        CrossMinType::TwoSidedGreedySwitch,
    ] {
        let mut root = LGraph::new("root", LayeredOptions::default());
        root.options.consider_model_order_strategy = OrderingStrategy::NodesAndEdges;
        root.random = JavaRandom::new(17);
        let mut child = LGraph::new("child", LayeredOptions::default());
        child.random = JavaRandom::new(31);
        let mut parent = LNode::new("parent", 10.0, 10.0, None);
        parent.nested_graph = Some(Box::new(child));
        root.layerless_nodes.push(parent);
        root.set_node_layer(0, 0);

        // Source initialize() draws nextLong from the root. GraphInfoHolder draws
        // a distributor boolean from each graph except for two-sided greedy switch.
        // Barycenter then resets only the root stream before its first sweep; the
        // zero-crossing early return still consumes the sweep-direction boolean.
        let mut expected_root = JavaRandom::new(17);
        let seed = expected_root.next_long();
        let mut expected_child = JavaRandom::new(31);
        if cross_min_type != CrossMinType::TwoSidedGreedySwitch {
            expected_root.next_bool();
            expected_child.next_bool();
        }
        if cross_min_type == CrossMinType::Barycenter {
            expected_root.set_seed(seed);
        }
        expected_root.next_bool();

        minimize_crossings_layer_sweep_hierarchical_with_type(&mut root, cross_min_type);
        assert_eq!(root.layers[0].nodes, vec![0]);
        assert_eq!(root.random, expected_root, "{cross_min_type:?}");
        assert_eq!(
            root.layerless_nodes[0]
                .nested_graph
                .as_ref()
                .unwrap()
                .random,
            expected_child,
            "{cross_min_type:?}",
        );
    }
}

#[test]
fn hierarchical_sweep_matches_source_node_port_counter_gate() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("A", 10.0, 10.0, None));
    graph.set_node_layer(0, 0);
    graph
        .options
        .consider_model_order_crossing_counter_node_influence = 0.0;
    graph
        .options
        .consider_model_order_crossing_counter_port_influence = 1.0;

    let sweep = HierarchySweep::new(&mut graph, CrossMinType::Barycenter);

    assert!(!sweep.use_node_port_order_crossing_counter(&graph, 0));

    graph
        .options
        .consider_model_order_crossing_counter_node_influence = 1.0;

    assert!(sweep.use_node_port_order_crossing_counter(&graph, 0));
}

#[test]
fn hierarchical_sweep_discovers_child_graphs_in_current_node_order() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    let mut second = LNode::new("second", 10.0, 10.0, None);
    second.nested_graph = Some(Box::new(LGraph::new(
        "second:nested",
        LayeredOptions::default(),
    )));
    let mut first = LNode::new("first", 10.0, 10.0, None);
    first.nested_graph = Some(Box::new(LGraph::new(
        "first:nested",
        LayeredOptions::default(),
    )));
    graph.layerless_nodes.push(second);
    graph.layerless_nodes.push(first);
    graph.layers = vec![crate::graph::Layer {
        nodes: vec![1, 0],
        size: Default::default(),
    }];

    let sweep = HierarchySweep::new(&mut graph, CrossMinType::Barycenter);

    assert_eq!(
        sweep.infos[0].child_paths,
        vec![GraphPath(vec![1]), GraphPath(vec![0])]
    );
}

#[test]
fn layer_sweep_type_decider_uses_origin_port_side_for_external_dummy() {
    let mut parent_graph = LGraph::new("root", LayeredOptions::default());
    parent_graph
        .layerless_nodes
        .push(LNode::new("parent", 10.0, 10.0, None));
    let origin = parent_graph
        .add_port(0, PortType::Output, PortSide::East, Default::default())
        .unwrap();

    let mut child_graph = LGraph::new("child", LayeredOptions::default());
    let mut external = LNode::new("external:parent", 0.0, 0.0, None);
    external.kind = LNodeKind::ExternalPort;
    external.external_port_side = PortSide::West;
    external.origin_port = Some(crate::graph::GraphPortRef {
        graph_id: parent_graph.id.clone(),
        port: origin,
    });
    child_graph.layerless_nodes.push(external);

    assert_eq!(
        external_dummy_origin_side(&child_graph, Some(&parent_graph), 0),
        PortSide::East
    );
}

#[test]
fn compound_port_sweep_remaps_dummy_origins_in_current_side_order() {
    use crate::graph::{GraphNodeRef, GraphPortRef, LPort};

    let mut parent = LGraph::new("root", LayeredOptions::default());
    let mut owner = LNode::new("owner", 10.0, 10.0, None);
    let mut child = LGraph::new("child", LayeredOptions::default());
    for (index, side) in [
        PortSide::West,
        PortSide::East,
        PortSide::West,
        PortSide::East,
    ]
    .into_iter()
    .enumerate()
    {
        let mut port = LPort::new(format!("port-{index}"), 0, PortType::Output);
        port.side = side;
        port.inside_connections = true;
        if index < 2 {
            port.port_dummy = Some(GraphNodeRef {
                graph_id: child.id.clone(),
                node: index,
            });
        }
        owner.ports.push(port);
        let mut dummy = LNode::new(format!("dummy-{index}"), 0.0, 0.0, None);
        dummy.kind = LNodeKind::ExternalPort;
        dummy.origin_port = Some(GraphPortRef {
            graph_id: parent.id.clone(),
            // The parent backlink wins when it disagrees with the numeric origin.
            port: PortRef {
                node: 0,
                port: if index == 0 { 2 } else { index },
            },
        });
        child.layerless_nodes.push(dummy);
    }
    let mut grandchild = LGraph::new("grandchild", LayeredOptions::default());
    for graph_id in [&parent.id, &child.id] {
        let mut dummy = LNode::new("nested-dummy", 0.0, 0.0, None);
        dummy.origin_port = Some(GraphPortRef {
            graph_id: graph_id.clone(),
            port: PortRef { node: 0, port: 3 },
        });
        grandchild.layerless_nodes.push(dummy);
    }
    child.layerless_nodes[0].nested_graph = Some(Box::new(grandchild));
    owner.nested_graph = Some(Box::new(child));
    parent.layerless_nodes.push(owner);

    for (right, dummy_layer, expected) in [(false, [0, 2], [2, 0]), (true, [3, 1], [3, 1])] {
        let child = parent.layerless_nodes[0].nested_graph.as_ref().unwrap();
        let order = parent_port_order_by_dummy_layer(child, &parent, &dummy_layer, 0, right);
        assert_eq!(order, expected);
        apply_parent_hierarchical_port_order_on_sweep_side(&mut parent, 0, right, &order);
    }
    let owner = &parent.layerless_nodes[0];
    assert_eq!(
        owner
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>(),
        ["port-2", "port-3", "port-0", "port-1"]
    );
    let child = owner.nested_graph.as_ref().unwrap();
    let origin_slots = |graph: &LGraph| {
        graph
            .layerless_nodes
            .iter()
            .map(|node| node.origin_port.as_ref().unwrap().port.port)
            .collect::<Vec<_>>()
    };
    assert_eq!(origin_slots(child), [0, 3, 0, 1]);
    assert_eq!(
        origin_slots(child.layerless_nodes[0].nested_graph.as_ref().unwrap()),
        [1, 3]
    );
}

#[test]
fn hierarchical_sweep_saves_currently_best_snapshot_not_current_order() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(LNode::new("A", 10.0, 10.0, None));
    graph
        .layerless_nodes
        .push(LNode::new("B", 10.0, 10.0, None));
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 0);

    let mut sweep = HierarchySweep::new(&mut graph, CrossMinType::Barycenter);
    sweep.changed.insert(0);
    sweep.set_currently_best_node_orders(&graph);
    sweep.infos[0].current_node_order[0] = vec![1, 0];

    sweep.save_all_node_orders_of_changed_graphs(&graph);

    assert_eq!(
        sweep.infos[0]
            .best_node_and_port_order
            .as_ref()
            .map(|copy| copy.node_order.as_slice()),
        Some([vec![0, 1]].as_slice())
    );
}

#[test]
fn barycenter_orders_free_layer_by_fixed_layer_port_ranks() {
    let mut graph = prepared_graph(
        vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
        vec![
            edge("Top-Right", "Top", "Right"),
            edge("Bottom-Left", "Bottom", "Left"),
        ],
    );
    let top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Top")
        .unwrap();
    let bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Bottom")
        .unwrap();
    let left = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Left")
        .unwrap();
    let right = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Right")
        .unwrap();
    graph.layers[0].nodes = vec![top, bottom];
    graph.layers[1].nodes = vec![left, right];
    initialize_crossing_minimization_port_ids(&mut graph);

    let mut order = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect::<Vec<_>>();
    let port_distributor =
        BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &order);
    let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);

    heuristic.minimize_crossings(&graph, &mut order, 1, true, false);

    assert_eq!(order[1], vec![right, left]);
}

#[test]
fn barycenter_cache_keeps_both_greater_facts_ahead_of_less_facts() {
    use std::cmp::Ordering::{Greater, Less};
    let graph = LGraph::new("relation-priority", LayeredOptions::default());
    let port_distributor = BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &[]);
    let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);
    let expected = [
        None,
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Greater),
    ];
    for (mask, expected) in expected.into_iter().enumerate() {
        let facts = |offset: usize| {
            [(0usize, 11, 23), (1, 23, 11)]
                .into_iter()
                .filter(|(bit, _, _)| mask & (1usize << (bit + offset)) != 0usize)
                .map(|(_, key, member)| (key, HashSet::from([member])))
                .collect()
        };
        heuristic.relations = ModelOrderRelations::from_test_rows(&facts(0), &facts(2));
        assert_eq!(
            heuristic.transitive_ordering(11, 23),
            expected,
            "mask {mask}"
        );
        if [0b1010, 0b1100, 0b1110].contains(&mask) {
            assert_eq!(heuristic.relations.cached_ordering(11, 23), Some(Less));
        }
    }
}

#[test]
fn forced_model_order_clears_relations_after_each_insertion_sort() {
    let mut graph = LGraph::new("relation-lifetime", LayeredOptions::default());
    graph.layerless_nodes = vec![
        LNode::new("first", 10.0, 10.0, Some(0)),
        LNode::new("second", 10.0, 10.0, Some(1)),
    ];
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 0);
    let order = vec![vec![0, 1]];
    let port_distributor =
        BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &order);
    let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);
    for _ in 0..2 {
        let mut layer = vec![0, 1];
        heuristic.insertion_sort_by_model_order(&graph, &mut layer);
        assert_eq!(layer, [0, 1]);
        assert!(heuristic.relations.is_clear());
    }
}

#[test]
fn forced_model_order_keeps_real_nodes_in_input_order() {
    let mut graph = prepared_graph(
        vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
        vec![
            edge("Top-Right", "Top", "Right"),
            edge("Bottom-Left", "Bottom", "Left"),
        ],
    );
    graph.options.force_node_model_order = true;
    let top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Top")
        .unwrap();
    let bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Bottom")
        .unwrap();
    let left = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Left")
        .unwrap();
    let right = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Right")
        .unwrap();
    graph.layers[0].nodes = vec![top, bottom];
    graph.layers[1].nodes = vec![left, right];
    initialize_crossing_minimization_port_ids(&mut graph);

    let mut order = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect::<Vec<_>>();
    let port_distributor =
        BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &order);
    let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);

    heuristic.minimize_crossings(&graph, &mut order, 1, true, false);

    assert_eq!(order[1], vec![left, right]);
}

#[test]
fn forced_model_order_still_places_dummy_nodes_by_barycenter() {
    let mut graph = LGraph::new(
        "root",
        LayeredOptions {
            force_node_model_order: true,
            ..LayeredOptions::default()
        },
    );
    let mut first = LNode::new("first", 10.0, 10.0, None);
    first.model_order = Some(0);
    let mut dummy = LNode::new("dummy", 0.0, 0.0, None);
    dummy.kind = LNodeKind::LongEdge;
    let mut second = LNode::new("second", 10.0, 10.0, None);
    second.model_order = Some(1);
    graph.layerless_nodes = vec![first, dummy, second];
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 0);
    graph.set_node_layer(2, 0);
    let order = vec![vec![0, 1, 2]];
    let port_distributor =
        BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &order);
    let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);
    heuristic.state_mut(0).barycenter = Some(2.0);
    heuristic.state_mut(1).barycenter = Some(0.0);
    heuristic.state_mut(2).barycenter = Some(1.0);
    let mut layer = order[0].clone();

    heuristic.insertion_sort_by_model_order(&graph, &mut layer);

    assert_eq!(layer, vec![1, 0, 2]);
}

#[test]
fn barycenter_port_ranks_keep_same_id_ports_distinct() {
    let mut graph = prepared_graph(
        vec![node("Top"), node("Bottom"), node("Free")],
        vec![
            edge("Top-Free", "Top", "Free"),
            edge("Bottom-Free", "Bottom", "Free"),
        ],
    );
    let top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Top")
        .unwrap();
    let bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Bottom")
        .unwrap();
    let free = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Free")
        .unwrap();
    graph.layers[0].nodes = vec![top, bottom];
    graph.layers[1].nodes = vec![free];
    for port in &mut graph.layerless_nodes[top].ports {
        port.id = "shared".to_string();
    }
    for port in &mut graph.layerless_nodes[bottom].ports {
        port.id = "shared".to_string();
    }
    initialize_crossing_minimization_port_ids(&mut graph);

    let order = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect::<Vec<_>>();
    let mut distributor = BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &order);
    distributor.calculate_port_ranks(&graph, &order[0], PortType::Output);
    distributor.distribute_ports(&mut graph, free, PortSide::West, &order);

    let free_ports = graph.layerless_nodes[free]
        .ports
        .iter()
        .map(|port| {
            port.incoming_edges
                .first()
                .map(|edge| graph.edges[*edge].id.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(free_ports, vec![Some("Bottom-Free"), Some("Top-Free")]);
}

#[test]
fn barycenter_port_ranks_persist_between_rank_calculations() {
    let mut graph = prepared_graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    );
    let a = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    let b = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "B")
        .unwrap();
    graph.layers[0].nodes = vec![a];
    graph.layers[1].nodes = vec![b];
    initialize_crossing_minimization_port_ids(&mut graph);
    let a_out = graph.layerless_nodes[a]
        .ports
        .iter()
        .position(|port| !port.outgoing_edges.is_empty())
        .unwrap();
    let mut distributor =
        BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &[vec![a], vec![b]]);

    distributor.calculate_port_ranks(&graph, &[a], PortType::Output);
    let before = distributor.rank_of(
        &graph,
        PortRef {
            node: a,
            port: a_out,
        },
    );
    distributor.calculate_port_ranks(&graph, &[b], PortType::Input);
    let after = distributor.rank_of(
        &graph,
        PortRef {
            node: a,
            port: a_out,
        },
    );

    assert_eq!(before, after);
    assert_ne!(after, 0.0);
}

#[test]
fn layer_sweep_keeps_best_crossing_order() {
    let mut graph = prepared_graph(
        vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
        vec![
            edge("Top-Right", "Top", "Right"),
            edge("Bottom-Left", "Bottom", "Left"),
        ],
    );
    let top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Top")
        .unwrap();
    let bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Bottom")
        .unwrap();
    let left = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Left")
        .unwrap();
    let right = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Right")
        .unwrap();
    graph.layers[0].nodes = vec![top, bottom];
    graph.layers[1].nodes = vec![left, right];

    let before = CrossingsCounter::new().count_all_crossings(&graph);
    assert!(minimize_crossings_layer_sweep(&mut graph));
    let after = CrossingsCounter::new().count_all_crossings(&graph);

    assert_eq!(before, 1);
    assert_eq!(after, 0);
}

#[test]
fn parent_hierarchical_port_order_only_rewrites_sweep_side() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("parent", 80.0, 40.0, None));
    let west_a = graph
        .add_port(0, PortType::Input, PortSide::West, Default::default())
        .unwrap()
        .port;
    let east_a = graph
        .add_port(0, PortType::Output, PortSide::East, Default::default())
        .unwrap()
        .port;
    let west_b = graph
        .add_port(0, PortType::Input, PortSide::West, Default::default())
        .unwrap()
        .port;
    let east_b = graph
        .add_port(0, PortType::Output, PortSide::East, Default::default())
        .unwrap()
        .port;
    graph.layerless_nodes[0].ports[west_a].id = "west_a".to_string();
    graph.layerless_nodes[0].ports[east_a].id = "east_a".to_string();
    graph.layerless_nodes[0].ports[west_b].id = "west_b".to_string();
    graph.layerless_nodes[0].ports[east_b].id = "east_b".to_string();
    for port in &mut graph.layerless_nodes[0].ports {
        port.inside_connections = true;
    }

    apply_parent_hierarchical_port_order_on_sweep_side(&mut graph, 0, true, &[east_b, east_a]);

    let port_ids = graph.layerless_nodes[0]
        .ports
        .iter()
        .map(|port| port.id.as_str())
        .collect::<Vec<_>>();
    let port_sides = graph.layerless_nodes[0]
        .ports
        .iter()
        .map(|port| port.side)
        .collect::<Vec<_>>();
    assert_eq!(port_ids, vec!["west_a", "east_b", "west_b", "east_a"]);
    assert_eq!(
        port_sides,
        vec![
            PortSide::West,
            PortSide::East,
            PortSide::West,
            PortSide::East
        ]
    );
}

#[test]
fn parent_hierarchical_port_order_updates_edge_references() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("parent", 80.0, 40.0, None));
    let _west_a = graph
        .add_port(0, PortType::Input, PortSide::West, Default::default())
        .unwrap()
        .port;
    let east_a = graph
        .add_port(0, PortType::Output, PortSide::East, Default::default())
        .unwrap()
        .port;
    let _west_b = graph
        .add_port(0, PortType::Input, PortSide::West, Default::default())
        .unwrap()
        .port;
    let east_b = graph
        .add_port(0, PortType::Output, PortSide::East, Default::default())
        .unwrap()
        .port;
    for port in &mut graph.layerless_nodes[0].ports {
        port.inside_connections = true;
    }
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("target", 10.0, 10.0, None));
    let target_port = graph
        .add_port(1, PortType::Input, PortSide::West, Default::default())
        .unwrap()
        .port;

    let edge_a = graph
        .add_edge(crate::graph::LayeredEdge {
            id: "edge-a".to_string(),
            source: PortRef {
                node: 0,
                port: east_a,
            },
            target: PortRef {
                node: 1,
                port: target_port,
            },
            source_node_id: "parent".to_string(),
            target_node_id: "target".to_string(),
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
    let edge_b = graph
        .add_edge(crate::graph::LayeredEdge {
            id: "edge-b".to_string(),
            source: PortRef {
                node: 0,
                port: east_b,
            },
            target: PortRef {
                node: 1,
                port: target_port,
            },
            source_node_id: "parent".to_string(),
            target_node_id: "target".to_string(),
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

    apply_parent_hierarchical_port_order_on_sweep_side(&mut graph, 0, true, &[east_b, east_a]);

    assert_eq!(graph.edges[edge_a].source.port, east_b);
    assert_eq!(graph.edges[edge_b].source.port, east_a);
    assert_eq!(
        graph.layerless_nodes[0]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>(),
        vec!["parent:0", "parent:3", "parent:2", "parent:1"]
    );
    assert_eq!(
        graph.layerless_nodes[0]
            .ports
            .iter()
            .enumerate()
            .map(|(index, port)| (index, port.id.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (0, "parent:0"),
            (1, "parent:3"),
            (2, "parent:2"),
            (3, "parent:1"),
        ]
    );
}

#[test]
fn north_south_hierarchical_ports_use_source_barycenter_keys() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("parent", 80.0, 40.0, None));
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("input_dummy", 0.0, 0.0, None));
    graph.layerless_nodes[1].kind = LNodeKind::ExternalPort;
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("output_dummy", 0.0, 0.0, None));
    graph.layerless_nodes[2].kind = LNodeKind::ExternalPort;
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("both_dummy", 0.0, 0.0, None));
    graph.layerless_nodes[3].kind = LNodeKind::ExternalPort;
    graph.set_node_layer(1, 0);
    graph.set_node_layer(2, 0);
    graph.set_node_layer(3, 0);
    graph.layers[0].nodes = vec![1, 2, 3];

    let north_input = graph
        .add_port(0, PortType::Input, PortSide::North, Default::default())
        .unwrap();
    let south_input = graph
        .add_port(0, PortType::Input, PortSide::South, Default::default())
        .unwrap();
    let north_output = graph
        .add_port(0, PortType::Output, PortSide::North, Default::default())
        .unwrap();
    let south_both = graph
        .add_port(0, PortType::Output, PortSide::South, Default::default())
        .unwrap();
    for (port, dummy) in [
        (north_input, 1usize),
        (south_input, 1usize),
        (north_output, 2usize),
        (south_both, 3usize),
    ] {
        graph.layerless_nodes[0].ports[port.port].port_dummy = Some(crate::graph::GraphNodeRef {
            graph_id: "root".to_string(),
            node: dummy,
        });
    }

    let in_dummy_port = graph
        .add_port(1, PortType::Input, PortSide::South, Default::default())
        .unwrap();
    let out_dummy_port = graph
        .add_port(2, PortType::Output, PortSide::South, Default::default())
        .unwrap();
    let both_in_dummy_port = graph
        .add_port(3, PortType::Input, PortSide::South, Default::default())
        .unwrap();
    let both_out_dummy_port = graph
        .add_port(3, PortType::Output, PortSide::South, Default::default())
        .unwrap();

    graph
        .add_edge(crate::graph::LayeredEdge {
            id: "north-in".to_string(),
            source: north_input,
            target: in_dummy_port,
            source_node_id: "parent".to_string(),
            target_node_id: "input_dummy".to_string(),
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
    graph
        .add_edge(crate::graph::LayeredEdge {
            id: "out".to_string(),
            source: out_dummy_port,
            target: north_output,
            source_node_id: "output_dummy".to_string(),
            target_node_id: "parent".to_string(),
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
    graph
        .add_edge(crate::graph::LayeredEdge {
            id: "both-in".to_string(),
            source: south_both,
            target: both_in_dummy_port,
            source_node_id: "parent".to_string(),
            target_node_id: "both_dummy".to_string(),
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
    graph
        .add_edge(crate::graph::LayeredEdge {
            id: "both-out".to_string(),
            source: both_out_dummy_port,
            target: south_both,
            source_node_id: "both_dummy".to_string(),
            target_node_id: "parent".to_string(),
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

    let node_positions = graph.layers[0]
        .nodes
        .iter()
        .copied()
        .enumerate()
        .map(|(position, node)| (node, position))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        north_south_port_barycenter_sum(&graph, &node_positions, north_input, 1, 7.0),
        Some(-0.0)
    );
    assert_eq!(
        north_south_port_barycenter_sum(&graph, &node_positions, south_input, 1, 7.0),
        Some(7.0)
    );
    assert_eq!(
        north_south_port_barycenter_sum(&graph, &node_positions, north_output, 2, 7.0),
        Some(2.0)
    );
    assert_eq!(
        north_south_port_barycenter_sum(&graph, &node_positions, south_both, 3, 7.0),
        Some(3.5)
    );

    let current_node_positions = [(3usize, 0usize), (2, 1), (1, 2)]
        .into_iter()
        .collect::<HashMap<_, _>>();
    assert_eq!(
        north_south_port_barycenter_sum(&graph, &current_node_positions, south_input, 1, 7.0),
        Some(5.0)
    );
}

#[test]
fn barycenter_sweep_distributes_free_and_fixed_layer_ports() {
    let mut graph = prepared_graph(
        vec![
            node("Top"),
            node("Bottom"),
            node("Left"),
            node("Right"),
            node("Free"),
            node("Fixed"),
        ],
        vec![
            edge("Top-Free", "Top", "Free"),
            edge("Bottom-Free", "Bottom", "Free"),
            edge("Free-Right", "Free", "Right"),
            edge("Free-Left", "Free", "Left"),
            edge("Fixed-Right", "Fixed", "Right"),
            edge("Fixed-Left", "Fixed", "Left"),
        ],
    );

    let top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Top")
        .unwrap();
    let bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Bottom")
        .unwrap();
    let left = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Left")
        .unwrap();
    let right = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Right")
        .unwrap();
    let free = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Free")
        .unwrap();
    let fixed = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Fixed")
        .unwrap();

    graph.layers[0].nodes = vec![top, bottom, fixed];
    graph.layers[1].nodes = vec![free, right, left];
    for (layer_index, layer) in graph.layers.iter().enumerate() {
        for node in &layer.nodes {
            graph.layerless_nodes[*node].layer_index = Some(layer_index);
        }
    }
    initialize_crossing_minimization_port_ids(&mut graph);

    let mut graph_info = GraphInfoHolder::new(&graph);
    let port_distributor = BarycenterPortDistributor::new(
        PortDistributorKind::NodeRelative,
        &graph_info.current_node_order,
    );
    let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);

    let reorder_work = ReorderNodePortsWorkContext::new(&graph).unwrap();
    sweep_reducing_crossings(
        &mut graph,
        &mut graph_info,
        &mut heuristic,
        true,
        false,
        &mut NoopWorkControl,
        &reorder_work,
    )
    .unwrap();

    let free_ports = graph.layerless_nodes[free]
        .ports
        .iter()
        .map(|port| port.id.as_str())
        .collect::<Vec<_>>();
    let fixed_ports = graph.layerless_nodes[fixed]
        .ports
        .iter()
        .map(|port| port.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(free_ports, vec!["Free:2", "Free:3", "Free:1", "Free:0"]);
    assert_eq!(fixed_ports, vec!["Fixed:0", "Fixed:1"]);
}

#[test]
fn hierarchical_sweep_orders_nested_sources_by_child_boundary_ports() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![
            nested_group("O", None),
            nested_group("A", Some("O")),
            nested_group("B", Some("A")),
            nested_node("b", Some("A")),
            nested_node("a", Some("A")),
            nested_node("c", Some("B")),
        ],
        edges: vec![edge("L_b_B_0", "b", "B"), edge("L_a_c_0", "a", "c")],
    })
    .unwrap();
    graph.options.hierarchy_handling = HierarchyHandling::IncludeChildren;

    crate::compound::preprocess_source_ported_compound_graph(&mut graph);
    crate::configurator::configure_graph_properties(&mut graph).unwrap();

    crate::pipeline::execute_ported_compound_processors_until(
        &mut graph,
        crate::pipeline::LayeredPhase::P3NodeOrdering,
    )
    .unwrap();

    let o = graph.layerless_nodes[0].nested_graph.as_ref().unwrap();
    let a = o.layerless_nodes[0].nested_graph.as_ref().unwrap();
    assert_eq!(node_order_by_id(a, &a.layers[0].nodes), vec!["a", "b"]);
}

#[test]
fn hierarchical_barycenter_keeps_current_order_between_randomized_tries() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![
            nested_group("bar", None),
            nested_group("foo", None),
            nested_node("A", None),
            nested_node("B", None),
            nested_node("G", None),
            nested_node("E", Some("bar")),
            nested_node("F", Some("bar")),
            nested_node("C", Some("foo")),
            nested_node("D", Some("foo")),
        ],
        edges: vec![
            edge("L_A_B_0", "A", "B"),
            edge("L_B_C_0", "B", "C"),
            edge("L_C_D_0", "C", "D"),
            edge("L_B_D_0", "B", "D"),
            edge("L_D_E_0", "D", "E"),
            edge("L_E_A_0", "E", "A"),
            edge("L_E_F_0", "E", "F"),
            edge("L_F_D_0", "F", "D"),
            edge("L_F_G_0", "F", "G"),
            edge("L_B_G_0", "B", "G"),
            edge("L_G_D_0", "G", "D"),
        ],
    })
    .unwrap();
    graph.options.hierarchy_handling = HierarchyHandling::IncludeChildren;

    crate::compound::preprocess_source_ported_compound_graph(&mut graph);
    crate::configurator::configure_graph_properties(&mut graph).unwrap();
    crate::pipeline::execute_ported_compound_processors_until_processor(
        &mut graph,
        crate::pipeline::ProcessorKind::SortByInputModelProcessor,
    )
    .unwrap();
    let trace =
        debug_crossings_layer_sweep_hierarchical_with_type(&mut graph, CrossMinType::Barycenter)
            .unwrap();
    let runs = trace
        .runs
        .iter()
        .filter(|run| run.graph_id == "root")
        .collect::<Vec<_>>();
    assert_eq!(
        runs.iter().map(|run| run.crossings).collect::<Vec<_>>(),
        [2.0, 2.0, 2.0, 1.0, 2.0, 1.0, 2.0]
    );
    assert!(runs[0].first_try_with_initial_order);
    assert!(
        runs[1..]
            .iter()
            .all(|run| !run.first_try_with_initial_order)
    );

    let first_layer_order = node_order_by_id(&graph, &graph.layers[0].nodes);
    let last_layer_order = node_order_by_id(&graph, &graph.layers[2].nodes);
    // Pinned elkjs 0.9.3 puts the inverted L_D_E port dummy before both groups.
    // The 0.12.0 oracle's hierarchical-randomized-tries case observes [B, bar, dummy]
    // and [foo, dummy] after the two-pass input sort and seven randomized tries.
    // FIRST/SECOND_TRY still share their Property ID; the first strictly best run wins.
    assert_eq!(first_layer_order.len(), 3);
    assert_eq!(first_layer_order[0], "B", "{first_layer_order:?}");
    assert_eq!(first_layer_order[1], "bar");
    assert!(first_layer_order[2].starts_with("invertedPort:"));
    assert_eq!(last_layer_order.len(), 2);
    assert_eq!(last_layer_order[0], "foo", "{last_layer_order:?}");
    assert!(last_layer_order[1].starts_with("invertedPort:"));
}

#[test]
fn forster_constraints_restore_successor_order_after_barycenter_sort() {
    let mut graph = prepared_graph(
        vec![node("A"), node("B"), node("C")],
        vec![edge("A-C", "A", "C"), edge("B-C", "B", "C")],
    );
    let a = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "A")
        .unwrap();
    let b = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "B")
        .unwrap();
    let c = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "C")
        .unwrap();
    graph.layers[0].nodes = vec![c];
    graph.layers[1].nodes = vec![b, a];
    graph.layerless_nodes[a]
        .in_layer_successor_constraints
        .push(b);

    let mut order = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect::<Vec<_>>();
    let port_distributor =
        BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &order);
    let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);

    heuristic.minimize_crossings(&graph, &mut order, 1, true, false);

    assert_eq!(order[1], vec![a, b]);
}

#[test]
fn forster_single_node_groups_merge_by_current_barycenters() {
    let mut states = HashMap::new();
    states.insert(
        0,
        BarycenterState {
            summed_weight: 9.0,
            degree: 3,
            barycenter: Some(3.0),
            visited: false,
        },
    );
    states.insert(
        1,
        BarycenterState {
            summed_weight: 1.0,
            degree: 1,
            barycenter: Some(1.0),
            visited: false,
        },
    );
    let first = ConstraintGroup::new(0);
    let second = ConstraintGroup::new(1);

    let merged = merge_constraint_groups(&mut states, &first, &second);

    assert_eq!(merged.barycenter(&states), Some(2.0));
    assert_eq!(states[&0].barycenter, Some(2.0));
    assert_eq!(states[&1].barycenter, Some(2.0));
}

#[test]
fn two_sided_greedy_switch_orders_layer_by_both_neighbors() {
    let mut graph = prepared_graph(
        vec![
            node("LeftTop"),
            node("LeftBottom"),
            node("MiddleTop"),
            node("MiddleBottom"),
            node("RightTop"),
            node("RightBottom"),
        ],
        vec![
            edge("LeftTop-MiddleBottom", "LeftTop", "MiddleBottom"),
            edge("LeftBottom-MiddleTop", "LeftBottom", "MiddleTop"),
            edge("MiddleTop-RightBottom", "MiddleTop", "RightBottom"),
            edge("MiddleBottom-RightTop", "MiddleBottom", "RightTop"),
        ],
    );
    let left_top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "LeftTop")
        .unwrap();
    let left_bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "LeftBottom")
        .unwrap();
    let middle_top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "MiddleTop")
        .unwrap();
    let middle_bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "MiddleBottom")
        .unwrap();
    let right_top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "RightTop")
        .unwrap();
    let right_bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "RightBottom")
        .unwrap();
    graph.layers[0].nodes = vec![left_top, left_bottom];
    graph.layers[1].nodes = vec![middle_top, middle_bottom];
    graph.layers[2].nodes = vec![right_top, right_bottom];
    for (layer_index, layer) in graph.layers.iter().enumerate() {
        for node in &layer.nodes {
            graph.layerless_nodes[*node].layer_index = Some(layer_index);
        }
    }

    let before = CrossingsCounter::new().count_all_crossings(&graph);
    let mut order = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect::<Vec<_>>();
    let mut heuristic = GreedySwitchHeuristic::new(CrossMinType::TwoSidedGreedySwitch);

    assert!(heuristic.minimize_crossings(&graph, &mut order, 1, true));
    assert_eq!(order[1], vec![middle_bottom, middle_top]);

    graph.layers[1].nodes = order[1].clone();
    let after = CrossingsCounter::new().count_all_crossings(&graph);

    assert_eq!(before, 2);
    assert_eq!(after, 0);
}

#[test]
fn one_sided_greedy_switch_reduces_between_layer_crossings() {
    let mut graph = prepared_graph(
        vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
        vec![
            edge("Top-Right", "Top", "Right"),
            edge("Bottom-Left", "Bottom", "Left"),
        ],
    );
    let top = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Top")
        .unwrap();
    let bottom = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Bottom")
        .unwrap();
    let left = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Left")
        .unwrap();
    let right = graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Right")
        .unwrap();
    graph.layers[0].nodes = vec![top, bottom];
    graph.layers[1].nodes = vec![left, right];
    for (layer_index, layer) in graph.layers.iter().enumerate() {
        for node in &layer.nodes {
            graph.layerless_nodes[*node].layer_index = Some(layer_index);
        }
    }

    let before = CrossingsCounter::new().count_all_crossings(&graph);
    minimize_crossings_layer_sweep_with_type(&mut graph, CrossMinType::OneSidedGreedySwitch);
    let after = CrossingsCounter::new().count_all_crossings(&graph);

    assert_eq!(before, 1);
    assert_eq!(after, 0);
}
