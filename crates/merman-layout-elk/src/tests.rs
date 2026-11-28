use super::hierarchy::detect_parent_cycles;
use super::input::graph_to_source_input;
use super::output::{
    CompoundEdgeLayoutSegment, comparison_sort_work_units, flatten_scope_layouts,
    order_compound_layout_segments, source_graph_output_work_units, source_graph_to_layout_result,
};
use super::*;
use source_port::{LGraph, LPoint, LayeredOptions as SourceLayeredOptions};

fn leaf(id: &str) -> Node {
    Node {
        id: id.to_string(),
        kind: NodeKind::Leaf,
        container: Default::default(),
        label_text: None,
        width: 80.0,
        height: 40.0,
        parent: None,
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_alignment: None,
        label: None,
    }
}

fn edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        label: None,
        terminal_labels: Vec::new(),
        minlen: 1,
        inside_self_loops_yo: false,
    }
}

fn flat_graph(nodes: Vec<Node>, edges: Vec<Edge>) -> Graph {
    Graph {
        id: "root".to_string(),
        direction: Direction::Down,
        nodes,
        edges,
        ..Default::default()
    }
}

#[test]
fn node_port_alignment_matches_elkjs_in_flat_and_separate_scopes() {
    // elkjs 0.9.3, three implicit ports on a 40px side with Mermaid's 12px
    // surrounding margins and spacing.baseValue=40. CENTER expands 5px gaps to 10px.
    for nested in [false, true] {
        let expected = if nested {
            // buildSubgraphLayoutOptions uses spacing.baseValue=24.
            [[15.4, 20.0, 24.6], [10.8, 20.0, 29.2]]
        } else {
            [[15.0, 20.0, 25.0], [10.0, 20.0, 30.0]]
        };
        for (alignment, expected) in [None, Some(PortAlignment::Center)]
            .into_iter()
            .zip(expected)
        {
            let mut nodes = vec![leaf("A"), leaf("B"), leaf("C"), leaf("T")];
            nodes[3].port_alignment = alignment;
            if nested {
                for node in &mut nodes {
                    node.parent = Some("frame".into());
                }
                nodes.push(group("frame", None, HierarchyHandling::SeparateChildren));
            }
            let mut graph = flat_graph(
                nodes,
                vec![
                    edge("AT", "A", "T"),
                    edge("BT", "B", "T"),
                    edge("CT", "C", "T"),
                ],
            );
            graph.direction = Direction::Right;
            let result = layout(&graph).unwrap();
            let target = result.nodes.iter().find(|node| node.id == "T").unwrap();
            let mut offsets: Vec<_> = result
                .edges
                .iter()
                .map(|edge| edge.points.last().unwrap().y - (target.y - target.height / 2.0))
                .collect();
            offsets.sort_by(f64::total_cmp);
            assert_eq!(offsets.len(), expected.len());
            for (actual, expected) in offsets.iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 1e-9,
                    "nested={nested}, alignment={alignment:?}, offsets={offsets:?}"
                );
            }
        }
    }
}

#[test]
fn container_flexibility_belongs_to_the_container_node() {
    let mut container = leaf("frame");
    container.kind = NodeKind::Group;
    let mut child = leaf("child");
    child.parent = Some("frame".into());
    let mut graph = flat_graph(vec![container, child], vec![]);
    graph.options.layered.node_placement = NodePlacementStrategy::NetworkSimplex;
    let source = graph_to_source_input(&graph).unwrap();
    let imported = source_port::import_graph(&source).unwrap();
    let container = &imported.layerless_nodes[0];
    assert_eq!(
        container.node_flexibility,
        source_port::NodeFlexibility::PortPosition
    );
    let nested = container.nested_graph.as_ref().unwrap();
    assert_eq!(
        nested.options.node_placement_strategy,
        source_port::NodePlacementStrategy::BrandesKoepf
    );
    assert_eq!(
        nested.layerless_nodes[0].node_flexibility,
        source_port::NodeFlexibility::None
    );
}

#[test]
fn coffman_graham_bound_changes_the_published_layout() {
    let mut graph = flat_graph(
        vec![leaf("A"), leaf("B"), leaf("C"), leaf("D")],
        vec![
            edge("ab", "A", "B"),
            edge("ac", "A", "C"),
            edge("bd", "B", "D"),
            edge("cd", "C", "D"),
        ],
    );
    graph.options.layered.layering = LayeringStrategy::CoffmanGraham;
    graph.options.layered.layering_layer_bound = 1;
    let narrow = layout(&graph).unwrap();
    graph.options.layered.layering_layer_bound = 2;
    let wide = layout(&graph).unwrap();
    let y =
        |result: &LayoutResult, id: &str| result.nodes.iter().find(|node| node.id == id).unwrap().y;
    assert_ne!(y(&narrow, "B"), y(&narrow, "C"));
    assert_eq!(y(&wide, "B"), y(&wide, "C"));
    assert!(y(&narrow, "D") - y(&narrow, "A") > y(&wide, "D") - y(&wide, "A"));
}

#[test]
fn parent_cycle_reports_the_revisited_cycle_member_after_a_stem() {
    let graph = flat_graph(vec![leaf("stem"), leaf("A"), leaf("B")], vec![]);
    let parent = [Some(1), Some(2), Some(1)];

    assert!(matches!(
        detect_parent_cycles(&graph, &parent),
        Err(Error::SourceImport(source_port::ImportError::ParentCycle { node_id }))
            if node_id == "A"
    ));
}

fn group(id: &str, parent: Option<&str>, handling: HierarchyHandling) -> Node {
    Node {
        id: id.to_string(),
        kind: NodeKind::Group,
        container: Default::default(),
        label_text: None,
        width: 0.0,
        height: 0.0,
        parent: parent.map(str::to_string),
        direction: None,
        hierarchy_handling: Some(handling),
        layer_constraint: None,
        port_alignment: None,
        label: Some(Label {
            width: 24.0,
            height: 18.0,
        }),
    }
}

fn deep_separate_graph(depth: usize) -> Graph {
    let mut nodes = Vec::with_capacity(depth + 1);
    for index in 0..depth {
        nodes.push(group(
            format!("group-{index}").as_str(),
            index
                .checked_sub(1)
                .map(|parent| format!("group-{parent}"))
                .as_deref(),
            HierarchyHandling::SeparateChildren,
        ));
    }
    let mut terminal = leaf("terminal");
    terminal.parent = depth.checked_sub(1).map(|parent| format!("group-{parent}"));
    nodes.push(terminal);

    flat_graph(nodes, Vec::new())
}

fn deep_include_graph(depth: usize) -> Graph {
    let mut graph = deep_separate_graph(depth);
    graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;
    for node in &mut graph.nodes {
        if node.kind == NodeKind::Group {
            node.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        }
    }
    graph
}

fn repeated_cross_scope_graph(edge_count: usize) -> Graph {
    let mut child = leaf("child");
    child.parent = Some("group".to_string());
    let edges = (0..edge_count)
        .map(|index| {
            let mut edge = edge(format!("cross-{index}").as_str(), "child", "outer");
            edge.label = Some(Label {
                width: 20.0,
                height: 10.0,
            });
            edge
        })
        .collect();
    flat_graph(
        vec![
            group("group", None, HierarchyHandling::SeparateChildren),
            child,
            leaf("outer"),
        ],
        edges,
    )
}

fn deep_cross_scope_graph(depth: usize, edge_count: usize) -> Graph {
    let mut graph = deep_separate_graph(depth);
    graph.options.layered.hierarchy_handling = HierarchyHandling::SeparateChildren;
    graph.nodes.push(leaf("outer"));
    graph.edges = (0..edge_count)
        .map(|index| edge(format!("deep-cross-{index}").as_str(), "terminal", "outer"))
        .collect();
    graph
}

fn synthetic_scope_arena(
    index: &HierarchyIndex<'_>,
    points_per_segment: usize,
) -> Vec<Option<ScopeLayout>> {
    index
        .scopes
        .iter()
        .map(|scope| {
            let nodes = scope
                .nodes
                .iter()
                .map(|node| {
                    let node = &index.graph.nodes[*node];
                    NodeLayout {
                        id: node.id.clone(),
                        x: 50.0,
                        y: 50.0,
                        width: node.width.max(1.0),
                        height: node.height.max(1.0),
                    }
                })
                .collect();
            let mut edge_metadata = HashMap::with_capacity(scope.edges.len());
            let edges = scope
                .edges
                .iter()
                .map(|scoped| {
                    let source = &index.graph.edges[scoped.original];
                    let segment = scoped.segment.map(|segment| SegmentedEdgeMetadata {
                        segment,
                        model_order: Some(index.edge_model_order[scoped.original]),
                        order: scoped
                            .segment_order
                            .expect("synthetic segmented edge keeps path order"),
                        count: scoped.segment_count,
                    });
                    edge_metadata.insert(
                        source.id.clone(),
                        ScopeEdgeMetadata {
                            original: scoped.original,
                            segment,
                        },
                    );
                    EdgeLayout {
                        id: source.id.clone(),
                        points: (0..points_per_segment)
                            .map(|point| Point {
                                x: point as f64,
                                y: point as f64,
                            })
                            .collect(),
                        labels: scoped
                            .carries_label
                            .then_some(EdgeLabelLayout {
                                terminal: None,
                                x: 1.0,
                                y: 1.0,
                                width: 20.0,
                                height: 10.0,
                            })
                            .into_iter()
                            .collect(),
                    }
                })
                .collect();
            Some(ScopeLayout {
                size_constraints_active: false,
                label_margin: source_port::LMargin::default(),
                content_shifts: HashMap::new(),
                layout: LayoutResult { nodes, edges },
                size: source_port::LSize {
                    width: 100.0,
                    height: 100.0,
                },
                edge_metadata,
            })
        })
        .collect()
}

#[derive(Debug)]
struct RecordingWorkControl {
    remaining: usize,
    checked: usize,
    charged: usize,
}

impl RecordingWorkControl {
    fn unlimited() -> Self {
        Self {
            remaining: usize::MAX,
            checked: 0,
            charged: 0,
        }
    }
}

impl WorkControl for RecordingWorkControl {
    fn check(&mut self, units: usize) -> std::result::Result<(), WorkError> {
        self.checked = self
            .checked
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        if units > self.remaining {
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), WorkError> {
        if units > self.remaining {
            return Err(WorkError::Interrupted);
        }
        self.remaining -= units;
        self.charged = self
            .charged
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        Ok(())
    }
}

#[test]
fn flat_scope_omits_unused_terminal_payload_at_a_fixed_budget() {
    for count in [0, 1, 8192] {
        let mut source_edge = edge("ab", "A", "B");
        source_edge.terminal_labels = vec![
            TerminalLabel {
                key: TerminalLabelKey::StartLeft,
                label: Label {
                    width: 20.0,
                    height: 12.0
                },
            };
            count
        ];
        let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![source_edge]);
        graph.options.algorithm = Algorithm::MrTree;
        let index = HierarchyIndex::build(&graph, &mut RecordingWorkControl::unlimited()).unwrap();
        let mut work = RecordingWorkControl {
            remaining: 3,
            checked: 0,
            charged: 0,
        };
        let (flat, _) = index.materialize_flat_scope(0, &[], &mut work).unwrap();
        assert_eq!(work.charged, 3);
        assert!(flat.edges[0].terminal_labels.is_empty());
        assert_eq!(flat.edges[0].terminal_labels.capacity(), 0);
        assert_eq!(graph.edges[0].terminal_labels.len(), count);
    }
}

#[test]
fn separate_children_hierarchy_index_has_unique_linear_ownership() {
    for depth in [8, 16, 32, 64, 128, 256] {
        let graph = deep_separate_graph(depth);
        let mut work_control = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();

        assert_eq!(index.scopes.len(), depth + 1);
        assert_eq!(index.postorder.len(), index.scopes.len());
        assert_eq!(index.postorder.last(), Some(&0));
        assert_eq!(
            index
                .scopes
                .iter()
                .map(|scope| scope.nodes.len())
                .sum::<usize>(),
            graph.nodes.len()
        );
        let mut node_owners = vec![0usize; graph.nodes.len()];
        for scope in &index.scopes {
            for node in &scope.nodes {
                node_owners[*node] += 1;
            }
        }
        assert!(node_owners.into_iter().all(|owners| owners == 1));
        assert_eq!(work_control.checked, work_control.charged);
    }
}

#[test]
fn separate_children_wrapper_work_is_linear_in_unique_items() {
    for depth in [8, 16, 32, 64, 128, 256] {
        let graph = deep_separate_graph(depth);
        let mut work_control = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
        let arena = std::iter::repeat_with(|| None)
            .take(index.scopes.len())
            .collect::<Vec<Option<ScopeLayout>>>();
        for scope in index.postorder.iter().copied() {
            index
                .materialize_scope(scope, &arena, &mut work_control)
                .unwrap();
        }

        assert_eq!(work_control.charged, graph.nodes.len() * 3);
        assert_eq!(work_control.checked, work_control.charged);
    }
}

#[test]
fn hierarchy_index_work_is_linear_in_emitted_boundary_segments() {
    for depth in [4usize, 8, 16] {
        let mut baseline_work = RecordingWorkControl::unlimited();
        HierarchyIndex::build(&deep_cross_scope_graph(depth, 0), &mut baseline_work).unwrap();

        for edge_count in [1usize, 4, 16] {
            let graph = deep_cross_scope_graph(depth, edge_count);
            let mut work_control = RecordingWorkControl::unlimited();
            let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
            let segment_count = index
                .scopes
                .iter()
                .map(|scope| scope.edges.len())
                .sum::<usize>();

            assert_eq!(segment_count, edge_count * (depth + 1));
            // Each edge contributes two indexed descriptor visits. The combined owner/section
            // walk then charges exactly once for every hierarchy-local section it emits.
            assert_eq!(
                work_control.charged - baseline_work.charged,
                2 * edge_count + segment_count
            );
        }
    }
}

#[test]
fn elk_wrapper_export_and_flatten_work_is_linear_in_cross_scope_geometry() {
    const POINTS_PER_SEGMENT: usize = 5;

    for edge_count in [1usize, 8, 32, 128] {
        let graph = repeated_cross_scope_graph(edge_count);
        let mut index_work = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut index_work).unwrap();
        assert_eq!(index.scopes.len(), 2);
        assert_eq!(
            index
                .scopes
                .iter()
                .map(|scope| scope.edges.len())
                .sum::<usize>(),
            edge_count * 2
        );

        let mut flatten_work = RecordingWorkControl::unlimited();
        let result = flatten_scope_layouts(
            &index,
            synthetic_scope_arena(&index, POINTS_PER_SEGMENT),
            &mut flatten_work,
        )
        .unwrap();

        // 2 scope-scan units + 2E geometry-scan units + (9 + 5E) structural units
        // + 2 * (10E points + E labels + 2E edge records) geometry units.
        assert_eq!(flatten_work.charged, 11 + 33 * edge_count);
        assert_eq!(flatten_work.checked, flatten_work.charged);
        assert_eq!(result.nodes.len(), graph.nodes.len());
        assert_eq!(result.edges.len(), edge_count);

        let flat = flat_graph(
            vec![leaf("source"), leaf("target")],
            (0..edge_count)
                .map(|index| {
                    let mut edge = edge(format!("flat-{index}").as_str(), "source", "target");
                    edge.label = Some(Label {
                        width: 20.0,
                        height: 10.0,
                    });
                    edge
                })
                .collect(),
        );
        let imported = source_port::import_graph(&graph_to_source_input(&flat).unwrap()).unwrap();
        let sort_work = match edge_count {
            1 => 0,
            8 => 24,
            32 => 160,
            128 => 896,
            _ => unreachable!("test matrix is fixed"),
        };
        // Two nodes, five linear export units per labelled straight edge, and the independent
        // E*ceil(log2(E)) comparison-sort bound used by the final Mermaid model-order pass.
        assert_eq!(
            source_graph_output_work_units(&imported),
            Ok(2 + 5 * edge_count + sort_work)
        );
    }
}

#[test]
fn output_sort_work_is_checked_and_superlinear() {
    assert_eq!(comparison_sort_work_units(0), Ok(0));
    assert_eq!(comparison_sort_work_units(1), Ok(0));
    assert_eq!(comparison_sort_work_units(2), Ok(2));
    assert_eq!(comparison_sort_work_units(3), Ok(6));
    assert_eq!(comparison_sort_work_units(8), Ok(24));
    assert_eq!(
        comparison_sort_work_units(usize::MAX),
        Err(WorkError::ArithmeticOverflow)
    );
}

#[test]
fn flatten_work_is_linear_in_points_at_fixed_cross_scope_cardinality() {
    const EDGE_COUNT: usize = 8;
    let graph = repeated_cross_scope_graph(EDGE_COUNT);
    let mut index_work = RecordingWorkControl::unlimited();
    let index = HierarchyIndex::build(&graph, &mut index_work).unwrap();
    let segment_count = index
        .scopes
        .iter()
        .map(|scope| scope.edges.len())
        .sum::<usize>();
    assert_eq!(segment_count, EDGE_COUNT * 2);

    let measured = [1usize, 8, 64].map(|points_per_segment| {
        let mut work_control = RecordingWorkControl::unlimited();
        flatten_scope_layouts(
            &index,
            synthetic_scope_arena(&index, points_per_segment),
            &mut work_control,
        )
        .unwrap();
        (points_per_segment, work_control.charged)
    });

    for pair in measured.windows(2) {
        let (before_points, before_work) = pair[0];
        let (after_points, after_work) = pair[1];
        assert_eq!(
            after_work - before_work,
            2 * segment_count * (after_points - before_points)
        );
    }
}

#[test]
fn flatten_geometry_execution_budget_is_atomic() {
    const EDGE_COUNT: usize = 4;
    const POINTS_PER_SEGMENT: usize = 5;
    // 2 scope-scan + 8 geometry-scan planning units, then one 29 structural + 104 geometry
    // execution tranche. A failed execution check must retain only the completed planning work.
    const PLANNING_WORK: usize = 10;
    const EXECUTION_WORK: usize = 133;
    const REQUIRED: usize = PLANNING_WORK + EXECUTION_WORK;

    let graph = repeated_cross_scope_graph(EDGE_COUNT);
    let mut index_work = RecordingWorkControl::unlimited();
    let index = HierarchyIndex::build(&graph, &mut index_work).unwrap();

    let mut below = RecordingWorkControl {
        remaining: REQUIRED - 1,
        checked: 0,
        charged: 0,
    };
    let error = flatten_scope_layouts(
        &index,
        synthetic_scope_arena(&index, POINTS_PER_SEGMENT),
        &mut below,
    )
    .unwrap_err();
    assert_eq!(error.work_error(), Some(WorkError::Interrupted));
    assert_eq!(below.charged, PLANNING_WORK);
    assert_eq!(below.remaining, EXECUTION_WORK - 1);

    for extra in [0usize, 1] {
        let mut accepted = RecordingWorkControl {
            remaining: REQUIRED + extra,
            checked: 0,
            charged: 0,
        };
        let result = flatten_scope_layouts(
            &index,
            synthetic_scope_arena(&index, POINTS_PER_SEGMENT),
            &mut accepted,
        )
        .unwrap();
        assert_eq!(result.edges.len(), EDGE_COUNT);
        assert_eq!(accepted.charged, REQUIRED);
        assert_eq!(accepted.remaining, extra);
    }
}

#[test]
fn separate_children_edges_have_one_owner_and_output_sensitive_segments() {
    let mut a = leaf("a");
    a.parent = Some("left".to_string());
    let mut a2 = leaf("a2");
    a2.parent = Some("left".to_string());
    let mut b = leaf("b");
    b.parent = Some("right".to_string());
    let graph = flat_graph(
        vec![
            group("left", None, HierarchyHandling::SeparateChildren),
            a,
            a2,
            group("right", None, HierarchyHandling::SeparateChildren),
            b,
            leaf("outer"),
        ],
        vec![
            edge("a-a2", "a", "a2"),
            edge("a-outer", "a", "outer"),
            edge("a-b", "a", "b"),
            edge("left-a", "left", "a"),
        ],
    );
    let mut work_control = RecordingWorkControl::unlimited();
    let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();

    let mut owners = vec![0usize; graph.edges.len()];
    let mut segments = vec![0usize; graph.edges.len()];
    for scope in &index.scopes {
        for edge in &scope.owned_edges {
            owners[*edge] += 1;
        }
        for edge in &scope.edges {
            segments[edge.original] += 1;
        }
    }
    assert_eq!(owners, vec![1, 1, 1, 1]);
    assert_eq!(segments, vec![1, 2, 3, 1]);
}

#[test]
fn hierarchy_index_rejects_duplicate_edge_ids_before_scope_metadata_can_alias() {
    let graph = flat_graph(
        vec![leaf("a"), leaf("b"), leaf("c")],
        vec![edge("duplicate", "a", "b"), edge("duplicate", "a", "c")],
    );
    let mut work_control = RecordingWorkControl::unlimited();

    let error = HierarchyIndex::build(&graph, &mut work_control).unwrap_err();

    assert!(matches!(
        error,
        Error::SourceImport(source_port::ImportError::DuplicateEdge { id })
            if id == "duplicate"
    ));
}

#[test]
fn separate_children_segment_edge_order_is_unique_within_materialized_scope() {
    let mut nested_group = group("nested", Some("group"), HierarchyHandling::SeparateChildren);
    nested_group.label = None;
    let mut source = leaf("source");
    source.parent = Some("nested".to_string());
    let mut sibling = leaf("sibling");
    sibling.parent = Some("group".to_string());
    let graph = flat_graph(
        vec![
            group("group", None, HierarchyHandling::SeparateChildren),
            nested_group,
            source,
            sibling,
            leaf("root-target"),
        ],
        vec![
            edge("to-sibling", "source", "sibling"),
            edge("to-root", "source", "root-target"),
        ],
    );
    let mut work_control = RecordingWorkControl::unlimited();
    let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
    let nested_scope = index.child_scope_by_anchor[1].unwrap();
    let arena = std::iter::repeat_with(|| None)
        .take(index.scopes.len())
        .collect::<Vec<Option<ScopeLayout>>>();

    let (_, segments, _) = index
        .materialize_scope(nested_scope, &arena, &mut work_control)
        .unwrap();

    assert_eq!(
        segments
            .iter()
            .map(|segment| segment.edge.model_order)
            .collect::<Vec<_>>(),
        vec![Some(0), Some(0)]
    );
    assert_eq!(
        segments
            .iter()
            .map(|segment| segment.edge_order)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn separate_children_layout_is_small_stack_safe_and_deterministic() {
    for graph in [deep_separate_graph(256), deep_include_graph(256)] {
        let expected = layout(&graph).unwrap();
        let replayed = std::thread::Builder::new()
            .name("elk-compound-small-stack".to_string())
            .stack_size(128 * 1024)
            .spawn(move || layout(&graph))
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(replayed.nodes.len(), 257);
        assert_eq!(replayed, expected);
    }
}

#[test]
fn source_graph_dump_is_small_stack_safe_for_deep_hierarchy() {
    let depth = 256;
    let graph = deep_include_graph(depth);
    let dump = std::thread::Builder::new()
        .name("elk-source-dump-small-stack".to_string())
        .stack_size(128 * 1024)
        .spawn(move || {
            SourcePhaseDiagnostics::from_graph(&graph)
                .unwrap()
                .graph_dump()
        })
        .unwrap()
        .join()
        .expect("source graph dump must not overflow the worker stack");

    assert_eq!(
        dump.lines()
            .filter(|line| line.trim_start().starts_with("graph "))
            .count(),
        depth + 1
    );
}

#[test]
fn source_graph_dump_with_cross_boundary_origins_is_small_stack_safe() {
    let depth = 256;
    let mut graph = deep_include_graph(depth);
    graph.nodes.push(leaf("outside"));
    graph.edges.push(edge("cross", "terminal", "outside"));
    let mut diagnostics = SourcePhaseDiagnostics::from_graph(&graph).unwrap();
    diagnostics
        .execute_compound_until_processor(ProcessorKind::LongEdgeSplitter)
        .unwrap();
    let dump = std::thread::Builder::new()
        .name("elk-source-cross-boundary-dump-small-stack".to_string())
        .stack_size(128 * 1024)
        .spawn(move || diagnostics.graph_dump())
        .unwrap()
        .join()
        .expect("source graph dump must not overflow the worker stack");

    assert_eq!(
        dump.lines()
            .filter(|line| line.trim_start().starts_with("graph "))
            .count(),
        depth + 1
    );
    assert!(dump.contains(r#"dummy=Some(GraphNodeRef { graph_id: "group-255", node: "#));
    assert!(dump.contains(r#"origin=Some(GraphPortRef { graph_id: "group-254", port: PortRef { "#));
    assert!(!dump.contains("domain:"));
    assert!(!dump.contains("GraphSeedScope"));
}

#[test]
fn separate_children_cross_boundary_edges_merge_once_with_one_label() {
    let mut a = leaf("a");
    a.parent = Some("left".to_string());
    let mut b = leaf("b");
    b.parent = Some("right".to_string());
    let mut descendant_to_outer = edge("a-outer", "a", "outer");
    descendant_to_outer.label = Some(Label {
        width: 36.0,
        height: 14.0,
    });
    let graph = flat_graph(
        vec![
            group("left", None, HierarchyHandling::SeparateChildren),
            a,
            group("right", None, HierarchyHandling::SeparateChildren),
            b,
            leaf("outer"),
        ],
        vec![descendant_to_outer, edge("a-b", "a", "b")],
    );

    let result = layout(&graph).unwrap();
    for edge_id in ["a-outer", "a-b"] {
        let matches = result
            .edges
            .iter()
            .filter(|edge| edge.id == edge_id)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "edge {edge_id} must be flattened once");
        assert!(matches[0].points.len() >= 2);
        assert!(
            matches[0]
                .points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
    }
    assert_eq!(
        result
            .edges
            .iter()
            .find(|edge| edge.id == "a-outer")
            .unwrap()
            .labels
            .len(),
        1
    );
}

#[test]
fn separate_children_cross_boundary_edges_keep_owner_model_order() {
    let mut left = leaf("left");
    left.parent = Some("group".to_string());
    let mut right = leaf("right");
    right.parent = Some("group".to_string());
    let mut graph = flat_graph(
        vec![
            group("group", None, HierarchyHandling::SeparateChildren),
            left,
            right,
            leaf("outer-a"),
            leaf("outer-b"),
        ],
        vec![
            edge("child-local", "left", "right"),
            edge("cross-boundary", "left", "outer-a"),
            edge("root-local", "outer-a", "outer-b"),
        ],
    );
    graph.options.layered.consider_model_order = true;

    let mut work_control = RecordingWorkControl::unlimited();
    let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
    let arena = std::iter::repeat_with(|| None)
        .take(index.scopes.len())
        .collect::<Vec<Option<ScopeLayout>>>();
    let (input, segments, _) = index
        .materialize_scope(0, &arena, &mut work_control)
        .unwrap();
    let imported = source_port::import_graph_at_seed_scope_and_segments_with_work_control(
        &input,
        &index.scopes[0].seed_scope,
        &segments,
        &mut work_control,
    )
    .unwrap();

    let model_order = |edge_id: &str| {
        imported
            .edges
            .iter()
            .find(|edge| edge.id == edge_id)
            .unwrap_or_else(|| panic!("missing imported edge {edge_id}"))
            .model_order
    };
    assert_eq!(model_order("cross-boundary"), Some(0));
    assert_eq!(model_order("root-local"), Some(1));

    let result = layout(&graph).unwrap();
    assert_eq!(
        result
            .edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<Vec<_>>(),
        vec!["cross-boundary", "root-local", "child-local"]
    );
}

fn assert_direct_parent_child_edge_precedes_later_root_edge(source: &str, target: &str) {
    let mut child = leaf("child");
    child.parent = Some("group".to_string());
    let graph = flat_graph(
        vec![
            group("group", None, HierarchyHandling::SeparateChildren),
            child,
            leaf("root-a"),
            leaf("root-b"),
        ],
        vec![
            edge("parent-child", source, target),
            edge("root-local", "root-a", "root-b"),
        ],
    );
    let mut work_control = RecordingWorkControl::unlimited();
    let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();

    let result =
        flatten_scope_layouts(&index, synthetic_scope_arena(&index, 2), &mut work_control).unwrap();

    assert_eq!(
        result
            .edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<Vec<_>>(),
        ["parent-child", "root-local"]
    );
}

#[test]
fn parent_to_direct_child_edge_keeps_owner_scope_order_when_flattened() {
    assert_direct_parent_child_edge_precedes_later_root_edge("group", "child");
}

#[test]
fn direct_child_to_parent_edge_keeps_owner_scope_order_when_flattened() {
    assert_direct_parent_child_edge_precedes_later_root_edge("child", "group");
}

#[test]
fn elk_work_control_interrupts_before_hierarchy_index_allocation() {
    let graph = deep_separate_graph(8);
    let initial_budget = graph.nodes.len() - 1;
    let mut work_control = RecordingWorkControl {
        remaining: initial_budget,
        checked: 0,
        charged: 0,
    };

    let error = layout_with_work_control(&graph, &mut work_control).unwrap_err();

    assert_eq!(error.work_error(), Some(WorkError::Interrupted));
    assert_eq!(work_control.remaining, initial_budget);
    assert_eq!(work_control.charged, 0);
}

#[test]
fn source_backed_layout_uses_default_median_layer_for_cross_hierarchy_center_label() {
    let group = |id: &str, parent: Option<&str>, label: Label| Node {
        id: id.to_string(),
        kind: NodeKind::Group,
        container: Default::default(),
        label_text: None,
        width: 0.0,
        height: 0.0,
        parent: parent.map(str::to_string),
        direction: None,
        hierarchy_handling: Some(HierarchyHandling::IncludeChildren),
        layer_constraint: None,
        port_alignment: None,
        label: Some(label),
    };
    let child = |id: &str, parent: &str, width: f64| Node {
        parent: Some(parent.to_string()),
        width,
        height: 54.0,
        ..leaf(id)
    };
    let labelled_edge = |id: &str, source: &str, target: &str, width: f64| Edge {
        label: Some(Label {
            width,
            height: 24.0,
        }),
        ..edge(id, source, target)
    };
    let graph = flat_graph(
        vec![
            group(
                "container_Alpha",
                None,
                Label {
                    width: 116.859375,
                    height: 22.0,
                },
            ),
            group(
                "container_Beta",
                Some("container_Alpha"),
                Label {
                    width: 109.171875,
                    height: 22.0,
                },
            ),
            child("process_C", "container_Beta", 131.28125),
            child("Process_D", "container_Beta", 130.78125),
            child("process_A", "container_Alpha", 131.15625),
            child("process_B", "container_Alpha", 130.765625),
        ],
        vec![
            edge("C-D", "process_C", "Process_D"),
            edge("A-B", "process_A", "process_B"),
            labelled_edge("B-Beta", "process_B", "container_Beta", 99.03125),
            labelled_edge("A-C", "process_A", "process_C", 66.609375),
        ],
    );

    let result = layout(&graph).unwrap();

    let alpha = result
        .nodes
        .iter()
        .find(|node| node.id == "container_Alpha")
        .expect("outer compound node");
    let cross_hierarchy_label = result
        .edges
        .iter()
        .find(|edge| edge.id == "A-C")
        .and_then(|edge| edge.labels.first())
        .expect("cross-hierarchy center label");
    let sibling_label = result
        .edges
        .iter()
        .find(|edge| edge.id == "B-Beta")
        .and_then(|edge| edge.labels.first())
        .expect("sibling center label");

    // elkjs 0.9.3 with Mermaid 12 root and container options.
    assert_eq!(alpha.width, 295.375);
    assert_eq!(cross_hierarchy_label.x, 36.0);
    assert_eq!(cross_hierarchy_label.y, 192.0);
    assert_eq!(cross_hierarchy_label.width, 66.609375);
    assert_eq!(sibling_label.x, 167.9921875);
    assert_eq!(sibling_label.y, 255.0);
}

#[test]
fn source_backed_layout_accounts_for_inline_label_edge_thickness() {
    let mut a1 = leaf("a1");
    a1.parent = Some("one".to_string());
    a1.width = 76.796875;
    a1.height = 54.0;
    let mut a2 = leaf("a2");
    a2.parent = Some("one".to_string());
    a2.width = 76.796875;
    a2.height = 54.0;
    let mut first = edge("e1", "a1", "a2");
    first.label = Some(Label {
        width: 13.109375,
        height: 24.0,
    });
    let mut second = edge("e2", "a1", "a2");
    second.label = first.label;
    let mut graph = flat_graph(
        vec![
            Node {
                id: "one".to_string(),
                kind: NodeKind::Group,
                container: Default::default(),
                label_text: None,
                width: 0.0,
                height: 0.0,
                parent: None,
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                port_alignment: None,
                label: Some(Label {
                    width: 26.046875,
                    height: 22.0,
                }),
            },
            a1,
            a2,
        ],
        vec![first, second],
    );
    graph.direction = Direction::Left;

    let result = layout(&graph).unwrap();
    let group = result.nodes.iter().find(|node| node.id == "one").unwrap();
    let first = result.edges.iter().find(|edge| edge.id == "e1").unwrap();
    let second = result.edges.iter().find(|edge| edge.id == "e2").unwrap();

    // elkjs 0.9.3 with Mermaid 12 root and container options.
    assert_eq!(group.width, 334.703125);
    assert_eq!(group.height, 145.0);
    assert_eq!(first.labels[0].x, 172.796875);
    assert_eq!(first.labels[0].y, 108.0);
    assert!((first.points[0].y - 103.4).abs() < 1e-9);
    assert_eq!(first.points[2].y, 120.5);
    assert_eq!(second.labels[0].x, 172.796875);
    assert_eq!(second.labels[0].y, 63.0);
    assert!((second.points[0].y - 92.6).abs() < 1e-9);
    assert_eq!(second.points[2].y, 75.5);
}

#[test]
fn source_backed_layout_places_connected_nodes_in_direction_order() {
    let graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);

    let result = layout(&graph).unwrap();

    let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
    let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
    let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
    assert!(b.y > a.y);
    assert!(edge.points.len() >= 2);
    assert_eq!(edge.points.first().unwrap().y, a.y + a.height / 2.0);
    assert_eq!(edge.points.last().unwrap().y, b.y - b.height / 2.0);
}

#[test]
fn source_backed_layout_honors_left_right_direction() {
    let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
    graph.direction = Direction::Right;

    let result = layout(&graph).unwrap();

    let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
    let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
    assert!(b.x > a.x);
}

#[test]
fn source_ported_layout_rejects_unported_routing_before_layout() {
    let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
    graph.options.layered.edge_routing = EdgeRouting::Polyline;

    assert!(matches!(
        layout(&graph),
        Err(Error::SourcePipeline(
            source_port::PipelineError::UnsupportedProcessor {
                kind: source_port::ProcessorKind::PolylineEdgeRouter,
            }
        ))
    ));
}

#[test]
fn leaf_label_measurements_do_not_create_layered_margins() {
    // Pinned elkjs 0.9.3 replay of newshapesset2_lr_allpairs_059 with the local
    // measured shapes. Treating the hidden 120px bolt/circle labels as fixed ELK
    // labels previously added 85 + 106 = 191px to the final node's x position.
    let sizes = [
        (169.4, 87.0),
        (172.0, 127.23614471649148),
        (35.0, 70.0),
        (14.0, 14.0),
        (162.0, 76.0),
    ];
    let nodes = sizes
        .into_iter()
        .enumerate()
        .map(|(i, (width, height))| Node {
            width,
            height,
            label: Some(Label {
                width: 120.0,
                height: if i == 0 { 63.0 } else { 42.0 },
            }),
            ..leaf(&format!("n{i}{i}"))
        })
        .collect();
    let mut edges = Vec::new();
    for from in 0..5 {
        for to in from + 1..5 {
            edges.push(edge(
                &format!("L_n{from}{from}_n{to}{to}_0"),
                &format!("n{from}{from}"),
                &format!("n{to}{to}"),
            ));
        }
    }
    let mut graph = flat_graph(nodes, edges);
    graph.direction = Direction::Right;
    let input = graph_to_source_input(&graph).unwrap();
    assert!(input.nodes.iter().all(|node| node.label.is_none()));
    assert!(graph.nodes.iter().all(|node| node.label.is_some()));
    let actual = layout(&graph).unwrap();
    for (id, expected_x) in [
        ("n00", 96.7),
        ("n11", 327.4),
        ("n22", 470.9),
        ("n33", 575.4),
        ("n44", 723.4),
    ] {
        let node = actual.nodes.iter().find(|node| node.id == id).unwrap();
        assert!((node.x - expected_x).abs() < 1e-8, "{node:?}");
    }
    for node in &mut graph.nodes {
        node.label = None;
    }
    assert_eq!(actual, layout(&graph).unwrap());
}

#[test]
fn layered_scopes_preserve_group_labels_but_ignore_leaf_measurements() {
    for handling in [
        HierarchyHandling::IncludeChildren,
        HierarchyHandling::SeparateChildren,
    ] {
        let mut child = leaf("child");
        child.width = 14.0;
        child.height = 14.0;
        child.parent = Some("frame".into());
        child.label = Some(Label {
            width: 120.0,
            height: 42.0,
        });
        let mut frame = group("frame", None, handling);
        frame.label = Some(Label {
            width: 160.0,
            height: 24.0,
        });
        let mut graph = flat_graph(vec![frame, child], vec![]);
        if handling == HierarchyHandling::IncludeChildren {
            let input = graph_to_source_input(&graph).unwrap();
            assert!(
                input
                    .nodes
                    .iter()
                    .find(|node| node.id == "frame")
                    .unwrap()
                    .label
                    .is_some()
            );
            assert!(
                input
                    .nodes
                    .iter()
                    .find(|node| node.id == "child")
                    .unwrap()
                    .label
                    .is_none()
            );
        }
        let actual = layout(&graph).unwrap();
        graph.nodes[1].label = None;
        assert_eq!(actual, layout(&graph).unwrap(), "{handling:?}");
        graph.nodes[0].label = None;
        let without_title = layout(&graph).unwrap();
        let frame_size = |result: &LayoutResult| {
            let node = result.nodes.iter().find(|node| node.id == "frame").unwrap();
            (node.width, node.height)
        };
        assert_ne!(
            frame_size(&actual),
            frame_size(&without_title),
            "{handling:?}"
        );
    }
}

#[test]
fn source_backed_layout_excludes_empty_group_labels_from_elk_geometry() {
    let empty_group = Node {
        id: "B".to_string(),
        kind: NodeKind::Group,
        container: Default::default(),
        label_text: None,
        width: 0.0,
        height: 0.0,
        parent: None,
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_alignment: None,
        label: Some(Label {
            width: 9.0625,
            height: 22.0,
        }),
    };
    let mut laid_out_group = leaf("A");
    laid_out_group.width = 191.328125;
    laid_out_group.height = 105.0;
    let mut graph = flat_graph(vec![empty_group, laid_out_group], vec![]);
    graph.direction = Direction::Right;

    let result = layout(&graph).unwrap();
    let empty_group = result.nodes.iter().find(|node| node.id == "B").unwrap();
    let laid_out_group = result.nodes.iter().find(|node| node.id == "A").unwrap();

    assert_eq!(empty_group.width, 0.0);
    assert_eq!(empty_group.height, 0.0);
    assert_eq!(laid_out_group.y - laid_out_group.height / 2.0, 52.0);
}

#[test]
fn source_backed_layout_routes_long_edge_after_joiner() {
    let graph = flat_graph(
        vec![leaf("A"), leaf("B"), leaf("C")],
        vec![
            edge("A-B", "A", "B"),
            edge("B-C", "B", "C"),
            edge("A-C", "A", "C"),
        ],
    );

    let result = layout(&graph).unwrap();

    let long = result.edges.iter().find(|edge| edge.id == "A-C").unwrap();
    assert_eq!(
        result.edges.iter().filter(|edge| edge.id == "A-C").count(),
        1
    );
    assert!(long.points.len() > 4);
}

#[test]
fn source_backed_layout_exports_edge_label_layouts() {
    let mut labelled = edge("A-C", "A", "C");
    labelled.label = Some(Label {
        width: 48.0,
        height: 12.0,
    });
    let graph = flat_graph(
        vec![leaf("A"), leaf("B"), leaf("C")],
        vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
    );

    let result = layout(&graph).unwrap();

    let edge = result.edges.iter().find(|edge| edge.id == "A-C").unwrap();
    let label = edge
        .labels
        .first()
        .expect("source-backed ELK should export placed edge label bounds");
    assert_eq!(label.width, 48.0);
    assert_eq!(label.height, 12.0);
    assert!(label.x.is_finite());
    assert!(label.y.is_finite());
}

#[test]
fn layered_options_to_source_propagates_inside_self_loops_activate() {
    let mut graph = flat_graph(vec![leaf("A")], vec![]);
    graph.options.layered.inside_self_loops_activate = true;

    let input = graph_to_source_input(&graph).unwrap();

    assert!(input.options.inside_self_loops_activate);
}

#[test]
fn layered_options_to_source_preserves_upstream_random_seed_sentinel() {
    let mut graph = flat_graph(vec![leaf("A")], vec![]);
    graph.options.layered.random_seed = 0;

    let input = graph_to_source_input(&graph).unwrap();

    assert_eq!(input.options.random_seed, 0);
}

#[test]
fn raw_layout_rejects_unseeded_elk_zero_but_operation_seed_is_replayable() {
    use std::num::NonZeroU64;

    let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
    graph.options.layered.random_seed = 0;

    assert!(matches!(
        layout(&graph),
        Err(Error::SourcePipeline(source_port::PipelineError::RandomSeed(
            source_port::RandomSeedError::Unresolved { graph_path }
        ))) if graph_path == "root"
    ));

    let operation_seed = ElkOperationSeed::from_operation_seed(
        NonZeroU64::new(0x6d65_726d_616e).expect("nonzero operation seed"),
    );
    let first = layout_with_operation_seed(&graph, operation_seed).expect("deterministic layout");
    let replayed = layout_with_operation_seed(&graph, operation_seed).expect("replayed layout");

    assert_eq!(first, replayed);
}

#[test]
fn source_phase_diagnostics_uses_the_same_zero_seed_boundary() {
    use std::num::NonZeroU64;

    let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
    graph.options.layered.random_seed = 0;

    let mut raw = SourcePhaseDiagnostics::from_graph(&graph).expect("diagnostic session");
    assert!(matches!(
        raw.execute_all(),
        Err(Error::SourcePipeline(source_port::PipelineError::RandomSeed(
            source_port::RandomSeedError::Unresolved { graph_path }
        ))) if graph_path == "root"
    ));

    let operation_seed = ElkOperationSeed::from_operation_seed(
        NonZeroU64::new(0x6469_6167_6e6f_7374).expect("nonzero operation seed"),
    );
    let mut first = SourcePhaseDiagnostics::from_graph_with_operation_seed(&graph, operation_seed)
        .expect("seeded diagnostic session");
    let mut replayed =
        SourcePhaseDiagnostics::from_graph_with_operation_seed(&graph, operation_seed)
            .expect("replayed seeded diagnostic session");

    assert_eq!(
        first.execute_all().unwrap(),
        replayed.execute_all().unwrap()
    );
}

#[test]
fn graph_to_source_input_propagates_inside_self_loop_edge_flag() {
    let graph = flat_graph(
        vec![leaf("A")],
        vec![Edge {
            inside_self_loops_yo: true,
            ..edge("A-A", "A", "A")
        }],
    );

    let input = graph_to_source_input(&graph).unwrap();

    assert!(input.edges[0].inside_self_loops_yo);
}

#[test]
fn source_graph_export_applies_graph_offset_and_padding_to_layout() {
    let mut graph = LGraph::new("root", SourceLayeredOptions::default());
    graph.offset = LPoint { x: 1.0, y: 2.0 };
    graph.padding = source_port::LPadding {
        top: 7.0,
        right: 0.0,
        bottom: 0.0,
        left: 12.0,
    };

    let mut a = source_port::LNode::new("A", 10.0, 20.0, None);
    a.position = LPoint { x: 3.0, y: 5.0 };
    let mut b = source_port::LNode::new("B", 10.0, 20.0, None);
    b.position = LPoint { x: 50.0, y: 60.0 };
    graph.layerless_nodes.push(a);
    graph.layerless_nodes.push(b);

    let source = graph
        .add_port(
            0,
            source_port::PortType::Output,
            source_port::PortSide::South,
            LPoint { x: 5.0, y: 20.0 },
        )
        .unwrap();
    let target = graph
        .add_port(
            1,
            source_port::PortType::Input,
            source_port::PortSide::North,
            LPoint { x: 5.0, y: 0.0 },
        )
        .unwrap();

    let mut label = source_port::LLabel::new("label", 6.0, 7.0);
    label.position = LPoint { x: 30.0, y: 40.0 };
    graph
        .add_edge(source_port::LayeredEdge {
            id: "A-B".to_string(),
            source,
            target,
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
            labels: vec![label],
            minlen: 1,
            reversed: false,
            bend_points: vec![LPoint { x: 20.0, y: 30.0 }],
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        })
        .unwrap();

    let result = source_graph_to_layout_result(&graph);

    let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
    let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
    let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
    assert_eq!(a.x, 21.0);
    assert_eq!(a.y, 24.0);
    assert_eq!(b.x, 68.0);
    assert_eq!(b.y, 79.0);
    assert_eq!(edge.points[0], Point { x: 21.0, y: 34.0 });
    assert_eq!(edge.points[1], Point { x: 33.0, y: 39.0 });
    assert_eq!(edge.points[2], Point { x: 68.0, y: 69.0 });
    assert_eq!(edge.labels[0].x, 43.0);
    assert_eq!(edge.labels[0].y, 49.0);
}

#[test]
fn source_graph_export_groups_compound_segments_by_original_edge_id() {
    let mut graph = LGraph::new("root", SourceLayeredOptions::default());
    graph
        .layerless_nodes
        .push(source_port::LNode::new("A", 10.0, 20.0, None));
    graph
        .layerless_nodes
        .push(source_port::LNode::new("B", 10.0, 20.0, None));

    let source = graph
        .add_port(
            0,
            source_port::PortType::Output,
            source_port::PortSide::South,
            LPoint { x: 5.0, y: 20.0 },
        )
        .unwrap();
    let target = graph
        .add_port(
            1,
            source_port::PortType::Input,
            source_port::PortSide::North,
            LPoint { x: 5.0, y: 0.0 },
        )
        .unwrap();

    let segment_edge = graph
        .add_edge(source_port::LayeredEdge {
            id: "merged-segment".to_string(),
            source,
            target,
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
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
        .cross_hierarchy_edges
        .push(source_port::CrossHierarchyEdge {
            original_edge_id: "A-B".to_string(),
            original_model_order: None,
            graph_id: "root".to_string(),
            edge: segment_edge,
            segment: source_port::CompoundEdgeSegment::Output { depth: 0 },
        });

    let result = source_graph_to_layout_result(&graph);

    assert!(result.edges.iter().any(|edge| edge.id == "A-B"));
    assert!(!result.edges.iter().any(|edge| edge.id == "merged-segment"));
}

#[test]
fn compound_segment_counting_order_matches_elk_cross_hierarchy_order() {
    let segment = |segment| CompoundEdgeLayoutSegment {
        original_edge_id: "edge".to_string(),
        segment,
        model_order: None,
        edge: EdgeLayout {
            id: "edge".to_string(),
            points: Vec::new(),
            labels: Vec::new(),
        },
    };
    let mut segments = vec![
        segment(source_port::CompoundEdgeSegment::Input { depth: 4 }),
        segment(source_port::CompoundEdgeSegment::Output { depth: 5 }),
        segment(source_port::CompoundEdgeSegment::Input { depth: 2 }),
        segment(source_port::CompoundEdgeSegment::Output { depth: 3 }),
        segment(source_port::CompoundEdgeSegment::Output { depth: 4 }),
        segment(source_port::CompoundEdgeSegment::Input { depth: 3 }),
    ];

    order_compound_layout_segments(&mut segments);

    assert_eq!(
        segments
            .into_iter()
            .map(|segment| segment.segment)
            .collect::<Vec<_>>(),
        vec![
            source_port::CompoundEdgeSegment::Output { depth: 5 },
            source_port::CompoundEdgeSegment::Output { depth: 4 },
            source_port::CompoundEdgeSegment::Output { depth: 3 },
            source_port::CompoundEdgeSegment::Input { depth: 2 },
            source_port::CompoundEdgeSegment::Input { depth: 3 },
            source_port::CompoundEdgeSegment::Input { depth: 4 },
        ]
    );
}

#[test]
fn source_graph_export_all_original_edges_for_shared_compound_segment() {
    let mut graph = LGraph::new("root", SourceLayeredOptions::default());
    graph
        .layerless_nodes
        .push(source_port::LNode::new("A", 10.0, 20.0, None));
    graph
        .layerless_nodes
        .push(source_port::LNode::new("B", 10.0, 20.0, None));

    let source = graph
        .add_port(
            0,
            source_port::PortType::Output,
            source_port::PortSide::South,
            LPoint { x: 5.0, y: 20.0 },
        )
        .unwrap();
    let target = graph
        .add_port(
            1,
            source_port::PortType::Input,
            source_port::PortSide::North,
            LPoint { x: 5.0, y: 0.0 },
        )
        .unwrap();

    let segment_edge = graph
        .add_edge(source_port::LayeredEdge {
            id: "merged-segment".to_string(),
            source,
            target,
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
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

    for original_edge_id in ["A-B-1", "A-B-2"] {
        graph
            .cross_hierarchy_edges
            .push(source_port::CrossHierarchyEdge {
                original_edge_id: original_edge_id.to_string(),
                original_model_order: None,
                graph_id: "root".to_string(),
                edge: segment_edge,
                segment: source_port::CompoundEdgeSegment::Output { depth: 0 },
            });
    }

    let result = source_graph_to_layout_result(&graph);

    assert!(result.edges.iter().any(|edge| edge.id == "A-B-1"));
    assert!(result.edges.iter().any(|edge| edge.id == "A-B-2"));
    assert!(!result.edges.iter().any(|edge| edge.id == "merged-segment"));
}

#[test]
fn source_graph_export_filters_shared_compound_segment_labels_by_original_edge() {
    let mut graph = LGraph::new("root", SourceLayeredOptions::default());
    graph
        .layerless_nodes
        .push(source_port::LNode::new("A", 10.0, 20.0, None));
    graph
        .layerless_nodes
        .push(source_port::LNode::new("B", 10.0, 20.0, None));

    let source = graph
        .add_port(
            0,
            source_port::PortType::Output,
            source_port::PortSide::South,
            LPoint { x: 5.0, y: 20.0 },
        )
        .unwrap();
    let target = graph
        .add_port(
            1,
            source_port::PortType::Input,
            source_port::PortSide::North,
            LPoint { x: 5.0, y: 0.0 },
        )
        .unwrap();
    let mut first_label = source_port::LLabel::new("first", 10.0, 4.0);
    first_label.original_label_edge = Some("A-B-1".to_string());
    let mut second_label = source_port::LLabel::new("second", 20.0, 4.0);
    second_label.original_label_edge = Some("A-B-2".to_string());

    let segment_edge = graph
        .add_edge(source_port::LayeredEdge {
            id: "merged-segment".to_string(),
            source,
            target,
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
            labels: vec![first_label, second_label],
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

    for original_edge_id in ["A-B-1", "A-B-2"] {
        graph
            .cross_hierarchy_edges
            .push(source_port::CrossHierarchyEdge {
                original_edge_id: original_edge_id.to_string(),
                original_model_order: None,
                graph_id: "root".to_string(),
                edge: segment_edge,
                segment: source_port::CompoundEdgeSegment::Output { depth: 0 },
            });
    }

    let result = source_graph_to_layout_result(&graph);
    let first = result.edges.iter().find(|edge| edge.id == "A-B-1").unwrap();
    let second = result.edges.iter().find(|edge| edge.id == "A-B-2").unwrap();

    assert_eq!(first.labels.len(), 1);
    assert_eq!(first.labels[0].width, 10.0);
    assert_eq!(second.labels.len(), 1);
    assert_eq!(second.labels[0].width, 20.0);
}

#[test]
fn source_backed_layout_exports_nested_compound_nodes_with_parent_offset() {
    let mut child = leaf("A");
    child.parent = Some("cluster".to_string());
    let mut second_child = leaf("B");
    second_child.parent = Some("cluster".to_string());
    let mut graph = flat_graph(
        vec![
            Node {
                id: "cluster".to_string(),
                kind: NodeKind::Group,
                container: Default::default(),
                label_text: None,
                width: 0.0,
                height: 0.0,
                parent: None,
                direction: Some(Direction::Down),
                hierarchy_handling: None,
                layer_constraint: None,
                port_alignment: None,
                label: None,
            },
            child,
            second_child,
        ],
        vec![edge("A-B", "A", "B")],
    );
    graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

    let result = layout(&graph).unwrap();

    let cluster = result
        .nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
    let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
    let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
    assert_eq!(result.nodes.len(), 3);
    assert!(cluster.width >= a.width);
    assert!(cluster.height >= b.y - a.y);
    assert!(a.y > cluster.y - cluster.height / 2.0);
    assert!(b.y < cluster.y + cluster.height / 2.0);
    assert!(b.y > a.y);
    assert_eq!(edge.points.first().unwrap().y, a.y + a.height / 2.0);
    assert_eq!(edge.points.last().unwrap().y, b.y - b.height / 2.0);
}

#[test]
fn source_backed_layout_routes_cross_hierarchy_edge() {
    let mut child = leaf("A");
    child.parent = Some("cluster".to_string());
    let mut graph = flat_graph(
        vec![
            Node {
                id: "cluster".to_string(),
                kind: NodeKind::Group,
                container: Default::default(),
                label_text: None,
                width: 0.0,
                height: 0.0,
                parent: None,
                direction: Some(Direction::Down),
                hierarchy_handling: None,
                layer_constraint: None,
                port_alignment: None,
                label: None,
            },
            child,
        ],
        vec![edge("cluster-A", "cluster", "A")],
    );
    graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

    let result = layout(&graph).unwrap();

    let cluster = result
        .nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let child = result.nodes.iter().find(|node| node.id == "A").unwrap();
    let edge = result
        .edges
        .iter()
        .find(|edge| edge.id == "cluster-A")
        .unwrap();
    assert_eq!(result.nodes.len(), 2);
    assert!(edge.points.len() >= 2);
    assert!(
        edge.points.first().unwrap().x >= cluster.x - cluster.width / 2.0
            && edge.points.first().unwrap().x <= cluster.x + cluster.width / 2.0
    );
    assert_eq!(edge.points.last().unwrap().x, child.x);
}

#[test]
fn source_backed_layout_exports_edge_from_nested_child_to_outer_node() {
    let mut child = leaf("A");
    child.parent = Some("cluster".to_string());
    let mut graph = flat_graph(
        vec![
            Node {
                id: "cluster".to_string(),
                kind: NodeKind::Group,
                container: Default::default(),
                label_text: None,
                width: 0.0,
                height: 0.0,
                parent: None,
                direction: Some(Direction::Down),
                hierarchy_handling: None,
                layer_constraint: None,
                port_alignment: None,
                label: None,
            },
            child,
            leaf("B"),
        ],
        vec![edge("A-B", "A", "B")],
    );
    graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

    let result = layout(&graph).unwrap();
    let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
    let edge = result
        .edges
        .iter()
        .find(|edge| edge.id == "A-B")
        .expect("cross-hierarchy child edge should be exported");
    assert!(edge.points.len() >= 2);
    assert!(
        edge.points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
    );
    assert_eq!(edge.points.last().unwrap().x, b.x);
}

#[test]
fn source_backed_layout_recursively_lays_out_separate_children() {
    let mut child = leaf("A");
    child.parent = Some("cluster".to_string());
    let mut second_child = leaf("B");
    second_child.parent = Some("cluster".to_string());
    let mut graph = flat_graph(
        vec![
            Node {
                id: "cluster".to_string(),
                kind: NodeKind::Group,
                container: Default::default(),
                label_text: None,
                width: 0.0,
                height: 0.0,
                parent: None,
                direction: Some(Direction::Right),
                hierarchy_handling: Some(HierarchyHandling::SeparateChildren),
                layer_constraint: None,
                port_alignment: None,
                label: Some(Label {
                    width: 42.0,
                    height: 18.0,
                }),
            },
            child,
            second_child,
            leaf("outer"),
        ],
        vec![
            edge("A-B", "A", "B"),
            edge("cluster-outer", "cluster", "outer"),
        ],
    );
    graph.direction = Direction::Down;
    graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

    let result = layout(&graph).unwrap();

    let cluster = result
        .nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
    let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
    let outer = result.nodes.iter().find(|node| node.id == "outer").unwrap();
    let inner_edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
    let outer_edge = result
        .edges
        .iter()
        .find(|edge| edge.id == "cluster-outer")
        .unwrap();

    assert!(cluster.width >= b.x + b.width / 2.0 - (a.x - a.width / 2.0));
    assert!(cluster.height >= 18.0);
    assert!(a.x < b.x);
    assert!(outer.y > cluster.y);
    assert_eq!(inner_edge.points.first().unwrap().y, a.y);
    assert_eq!(inner_edge.points.last().unwrap().y, b.y);
    assert_eq!(outer_edge.points.first().unwrap().x, cluster.x);
}
