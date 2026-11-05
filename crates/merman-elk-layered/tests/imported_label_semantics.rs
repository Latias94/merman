//! Source-backed import-order and default-value regressions.
//!
//! The selected elk-worker.js imports only truthy label text (32135, 32245), copies original
//! edge properties into compound segments (29347), and defaults edge thickness to 1 (68677).
//! LabelDummyInserter (36706) and LongEdgeSplitter therefore reserve 1 + 10 + 2 + 1 = 14.
//! Eclipse ELK v0.12.0's importer, compound preprocessor, and label inserter agree.
//! The public input represents absent/null node labels with None; text itself is non-nullable.

use merman_elk_layered::{
    CompoundEdgeSegment, EdgeLabelPlacement, ElkDirection, ElkInputEdge, ElkInputEdgeSegment,
    ElkInputEdgeSegmentEndpoint, ElkInputGraph, ElkInputLabel, ElkInputNode, HierarchyHandling,
    LGraph, LNodeKind, LSize, LayeredOptions, LayeredPhase, NodeFlexibility, NodeLabelPlacement,
    NoopWorkControl, ProcessorKind, execute_ported_compound_processors,
    execute_ported_compound_processors_until, import_graph,
    import_graph_at_scope_and_segments_with_work_control,
};

fn node(id: &str, parent: Option<&str>) -> ElkInputNode {
    ElkInputNode {
        id: id.to_owned(),
        width: 40.0,
        height: 20.0,
        parent: parent.map(str::to_owned),
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

fn label(text: &str) -> ElkInputLabel {
    ElkInputLabel {
        source_index: Some(7),
        inline: false,
        ..ElkInputLabel::center(text, 120.0, 10.0)
    }
}

fn input(text: Option<&str>, compound: bool) -> ElkInputGraph {
    let mut nodes = vec![node("A", compound.then_some("G")), node("B", None)];
    if compound {
        let mut group = node("G", None);
        group.width = 0.0;
        group.height = 0.0;
        nodes.insert(0, group);
    }
    ElkInputGraph {
        id: "root".to_owned(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            hierarchy_handling: HierarchyHandling::IncludeChildren,
            ..Default::default()
        },
        nodes,
        edges: vec![ElkInputEdge {
            id: "A-B".to_owned(),
            source: "A".to_owned(),
            target: "B".to_owned(),
            labels: text.map(label).into_iter().collect(),
            minlen: 1,
            inside_self_loops_yo: false,
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }],
    }
}

fn scopes(graph: &LGraph) -> Vec<&LGraph> {
    let mut graphs = vec![graph];
    let mut next = 0;
    while next < graphs.len() {
        let current = graphs[next];
        graphs.extend(
            current
                .layerless_nodes
                .iter()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
        next += 1;
    }
    graphs
}

#[test]
fn node_labels_keep_whitespace_and_ignore_empty_or_absent_text() {
    for text in [None, Some(""), Some(" "), Some("\t\n"), Some("L")] {
        let mut input = input(None, false);
        input.edges.clear();
        input.nodes[0].label = text.map(label);
        let graph = import_graph(&input).unwrap();
        let labels = &graph.layerless_nodes[0].labels;
        let present = text.is_some_and(|text| !text.is_empty());
        assert_eq!(labels.len(), usize::from(present), "{text:?}");
        if present {
            assert_eq!(labels[0].text, text.unwrap());
            assert_eq!(labels[0].source_index, Some(7));
        }
        assert!(!graph.graph_properties.center_labels);
        assert!(!graph.graph_properties.end_labels);
    }
}

#[test]
fn edge_label_properties_follow_only_imported_labels() {
    for placement in [
        EdgeLabelPlacement::Center,
        EdgeLabelPlacement::Head,
        EdgeLabelPlacement::Tail,
    ] {
        for text in [None, Some(""), Some(" "), Some("\t\n"), Some("L")] {
            let mut input = input(text, false);
            for label in &mut input.edges[0].labels {
                label.placement = placement;
            }
            let graph = import_graph(&input).unwrap();
            let present = text.is_some_and(|text| !text.is_empty());
            assert_eq!(graph.edges[0].labels.len(), usize::from(present));
            if present {
                assert_eq!(graph.edges[0].labels[0].text, text.unwrap());
                assert_eq!(graph.edges[0].labels[0].source_index, Some(7));
            }
            let center = present && placement == EdgeLabelPlacement::Center;
            let end = present && placement != EdgeLabelPlacement::Center;
            assert_eq!(graph.graph_properties.center_labels, center);
            assert_eq!(graph.graph_properties.end_labels, end);
            assert_eq!(graph.options.graph_has_center_labels, center);
            assert_eq!(graph.options.graph_has_end_labels, end);
        }
    }
}

#[test]
fn empty_center_label_matches_no_label_through_flat_and_compound_layering() {
    for compound in [false, true] {
        let mut without_label = import_graph(&input(None, compound)).unwrap();
        execute_ported_compound_processors_until(&mut without_label, LayeredPhase::P2Layering)
            .unwrap();

        for text in ["", " ", "\t\n", "L"] {
            let mut graph = import_graph(&input(Some(text), compound)).unwrap();
            if compound {
                assert_eq!(
                    graph.hierarchy_edges[0].labels.len(),
                    usize::from(!text.is_empty())
                );
            }
            let executed =
                execute_ported_compound_processors_until(&mut graph, LayeredPhase::P2Layering)
                    .unwrap();
            let graphs = scopes(&graph);
            let dummies = graphs
                .iter()
                .flat_map(|graph| &graph.layerless_nodes)
                .filter(|node| node.kind == LNodeKind::Label)
                .collect::<Vec<_>>();
            let present = !text.is_empty();
            assert_eq!(dummies.len(), usize::from(present), "{compound}: {text:?}");
            assert_eq!(
                executed
                    .iter()
                    .filter(|execution| execution
                        .processors
                        .contains(&ProcessorKind::LabelDummyInserter))
                    .count(),
                usize::from(present),
            );
            for scope in &graphs {
                assert_eq!(
                    scope.graph_properties.center_labels,
                    scope.id == "root" && present
                );
                assert_eq!(
                    scope.options.graph_has_center_labels,
                    scope.graph_properties.center_labels
                );
                if !compound || !present {
                    assert!(!scope.graph_properties.end_labels);
                    assert!(!scope.options.graph_has_end_labels);
                }
                assert!(scope.edges.iter().all(|edge| edge.thickness == 1.0));
            }
            if present {
                assert_eq!(
                    dummies[0].size,
                    LSize {
                        width: 120.0,
                        height: 14.0
                    }
                );
                assert_eq!(dummies[0].represented_labels[0].text, text);
                assert_eq!(dummies[0].represented_labels[0].source_index, Some(7));
            } else {
                assert_eq!(graph, without_label);
            }
        }
    }
}

#[test]
fn compound_center_label_inherits_default_edge_thickness() {
    let mut input = input(Some("L"), true);
    input.edges[0].labels[0].width = 30.0;
    let mut graph = import_graph(&input).unwrap();
    execute_ported_compound_processors_until(&mut graph, LayeredPhase::P2Layering).unwrap();

    let dummy = graph
        .layerless_nodes
        .iter()
        .find(|node| node.kind == LNodeKind::Label)
        .unwrap();
    assert_eq!(
        dummy.size,
        LSize {
            width: 30.0,
            height: 14.0
        }
    );
    assert_eq!(
        dummy.represented_labels[0].original_label_edge.as_deref(),
        Some("A-B")
    );
    assert_eq!(graph.edges[dummy.origin_edge.unwrap()].thickness, 1.0);
    assert_eq!(graph.options.spacing.edge_label, 2.0);
}

#[test]
fn compound_labels_keep_placement_and_mark_the_scope_that_receives_them() {
    for placement in [
        EdgeLabelPlacement::Center,
        EdgeLabelPlacement::Head,
        EdgeLabelPlacement::Tail,
    ] {
        for text in [None, Some(""), Some(" ")] {
            let mut input = input(text, true);
            for label in &mut input.edges[0].labels {
                label.placement = placement;
            }
            let mut graph = import_graph(&input).unwrap();
            execute_ported_compound_processors_until(&mut graph, LayeredPhase::P1CycleBreaking)
                .unwrap();
            let present = text.is_some_and(|text| !text.is_empty());
            let graphs = scopes(&graph);
            let labels = graphs
                .iter()
                .flat_map(|graph| &graph.edges)
                .flat_map(|edge| &edge.labels)
                .collect::<Vec<_>>();
            assert_eq!(labels.len(), usize::from(present));
            for scope in &graphs {
                assert_eq!(
                    scope.options.graph_has_center_labels,
                    scope.graph_properties.center_labels
                );
                assert_eq!(
                    scope.options.graph_has_end_labels,
                    scope.graph_properties.end_labels
                );
                if !present {
                    assert!(!scope.graph_properties.center_labels);
                    assert!(!scope.graph_properties.end_labels);
                }
                if scope.edges.iter().any(|edge| !edge.labels.is_empty()) {
                    assert_eq!(
                        scope.id,
                        if placement == EdgeLabelPlacement::Tail {
                            "G"
                        } else {
                            "root"
                        }
                    );
                    if placement == EdgeLabelPlacement::Center {
                        assert!(scope.graph_properties.center_labels);
                    } else {
                        assert!(scope.graph_properties.end_labels);
                    }
                }
            }
            if present {
                assert_eq!(labels[0].placement, placement);
                assert_eq!(labels[0].source_index, Some(7));
                assert_eq!(labels[0].original_label_edge.as_deref(), Some("A-B"));
            }
        }
    }
}

#[test]
fn scoped_cross_hierarchy_segments_apply_the_same_empty_text_filter() {
    for text in [None, Some(""), Some(" ")] {
        let mut input = input(text, true);
        let segment = ElkInputEdgeSegment {
            edge: input.edges.remove(0),
            source: ElkInputEdgeSegmentEndpoint::Node { id: "A".to_owned() },
            target: ElkInputEdgeSegmentEndpoint::Node { id: "B".to_owned() },
            segment: CompoundEdgeSegment::Output { depth: 2 },
            edge_order: 0,
        };
        let mut graph = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            &[segment],
            &mut NoopWorkControl,
        )
        .unwrap();
        let present = text.is_some_and(|text| !text.is_empty());
        let graphs = scopes(&graph);
        let labels = graphs
            .iter()
            .flat_map(|graph| &graph.edges)
            .flat_map(|edge| &edge.labels)
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), usize::from(present));
        if present {
            assert_eq!(labels[0].original_label_edge.as_deref(), Some("A-B"));
            assert_eq!(labels[0].source_index, Some(7));
        }
        assert!(
            graphs
                .iter()
                .flat_map(|graph| &graph.edges)
                .all(|edge| edge.thickness == 1.0)
        );
        assert_eq!(graph.graph_properties.center_labels, present);
        assert_eq!(graph.options.graph_has_center_labels, present);
        execute_ported_compound_processors_until(&mut graph, LayeredPhase::P2Layering).unwrap();
        assert_eq!(
            scopes(&graph)
                .iter()
                .flat_map(|graph| &graph.layerless_nodes)
                .filter(|node| node.kind == LNodeKind::Label)
                .count(),
            usize::from(present),
        );
    }
}

#[test]
fn inside_self_loop_labels_filter_empty_text_before_setting_properties() {
    for text in [None, Some(""), Some(" ")] {
        let mut input = input(text, false);
        input.options.inside_self_loops_activate = true;
        input.edges[0].target = "A".to_owned();
        input.edges[0].inside_self_loops_yo = true;
        let graph = import_graph(&input).unwrap();
        let nested = graph.layerless_nodes[0].nested_graph.as_deref().unwrap();
        let present = text.is_some_and(|text| !text.is_empty());
        assert_eq!(nested.edges[0].labels.len(), usize::from(present));
        assert_eq!(nested.graph_properties.center_labels, present);
        assert_eq!(nested.options.graph_has_center_labels, present);
        assert!(!nested.graph_properties.end_labels);
        assert_eq!(nested.edges[0].thickness, 1.0);
    }
}

#[test]
fn raw_title_geometry_precedes_empty_text_filtering() {
    for width in [0.0, 120.0] {
        for text in [None, Some(""), Some(" "), Some("\t\n")] {
            let mut input = input(None, true);
            input.edges.clear();
            let group = &mut input.nodes[0];
            group.node_label_placement = NodeLabelPlacement::InsideTopCenter;
            group.label = text.map(|text| ElkInputLabel::center(text, width, 10.0));
            group.nested_options = Some(Box::new(LayeredOptions {
                node_size_include_labels: true,
                ..input.options.clone()
            }));
            let mut graph = import_graph(&input).unwrap();
            let group = &graph.layerless_nodes[0];
            let nested = group.nested_graph.as_deref().unwrap();
            let present = text.is_some_and(|text| !text.is_empty());
            assert_eq!(group.labels.len(), usize::from(present));
            assert_eq!(nested.padding.top, if text.is_some() { 27.0 } else { 12.0 });
            assert_eq!(nested.padding.left, 12.0);
            assert_eq!(nested.padding.right, 12.0);
            assert_eq!(nested.padding.bottom, 12.0);
            assert_eq!(
                nested.options.node_size_minimum,
                text.map(|_| LSize {
                    width: if width > 0.0 { 130.0 } else { 0.0 },
                    height: 30.0,
                }),
            );
            execute_ported_compound_processors(&mut graph).unwrap();
            let group = &graph.layerless_nodes[0];
            assert_eq!(
                group.size,
                LSize {
                    width: if text.is_some() && width > 0.0 {
                        130.0
                    } else {
                        64.0
                    },
                    height: if text.is_some() { 59.0 } else { 44.0 },
                }
            );
            let nested = group.nested_graph.as_deref().unwrap();
            let child = nested
                .layerless_nodes
                .iter()
                .find(|node| node.id == "A")
                .unwrap();
            assert_eq!(
                child.position.x + nested.offset.x + nested.padding.left,
                12.0
            );
            assert_eq!(
                child.position.y + nested.offset.y + nested.padding.top,
                if text.is_some() { 27.0 } else { 12.0 }
            );
        }
    }
}
