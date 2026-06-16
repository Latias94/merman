#![forbid(unsafe_code)]

//! Optional ELK layout engine integration for `merman`.
//!
//! Source-port policy:
//! - Mermaid's adapter layer is
//!   https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts.
//! - Mermaid pins `elkjs@0.9.3`; the corresponding source checkout is
//!   https://github.com/kieler/elkjs/tree/a8304cf79fde75bc2ab1a89d28320f53f8637436.
//! - `elkjs` is generated from Eclipse ELK Java sources. The current source baseline is
//!   https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e,
//!   which is the 0.9.x ELK release tag available for the `elkjs@0.9.3` release window.
//!
//! The public API currently delegates to `compat` to keep `flowchart-elk` usable while the
//! source-backed layered implementation is ported. New ELK layout behavior must land in
//! `source_port` with a source file reference; do not tune `compat` from fixture output.

use std::collections::HashMap;

mod compat;
pub use merman_elk_layered as source_port;

use source_port::{
    ElkDirection, ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, GreedySwitchType,
    LGraph, LNodeKind, LPoint, LayeredOptions as SourceLayeredOptions, NodeLabelPlacement, PortRef,
};

pub use compat::{
    Algorithm, CycleBreakingStrategy, Direction, Edge, EdgeLabelLayout, EdgeLayout, EdgeRouting,
    Error, Graph, HierarchyHandling, Label, LayeredOptions, LayoutOptions, LayoutResult,
    ModelOrderStrategy, Node, NodeKind, NodeLayout, NodePlacementStrategy, Point, Result,
    SelfLoopDistributionStrategy, Spacing,
};

pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    compat::layout(graph, algorithm)
}

/// Opt-in source-backed layered layout adapter.
///
/// This follows Mermaid's ELK adapter construction and executes the Rust port of Eclipse ELK's
/// layered pipeline. The default `layout` API intentionally remains on the compatibility backend
/// until the source-backed path covers the full render surface.
pub fn layout_source_ported(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    match algorithm {
        Algorithm::Layered => layout_layered_source_ported(graph),
    }
}

/// Build the source-backed layered input graph used by `layout_source_ported`.
///
/// This is intentionally narrow and primarily exists for parity diagnostics that need to inspect
/// Eclipse ELK processor phases without duplicating Mermaid adapter semantics.
pub fn source_input_from_graph(graph: &Graph) -> source_port::ElkInputGraph {
    graph_to_source_input(graph)
}

fn layout_layered_source_ported(graph: &Graph) -> Result<LayoutResult> {
    let has_parent_nodes = graph.nodes.iter().any(|node| node.parent.is_some());
    if has_parent_nodes
        && graph.options.layered.hierarchy_handling != HierarchyHandling::IncludeChildren
    {
        return Err(Error::UnsupportedSourceGraph {
            reason: "source-backed ELK separate hierarchy handling is not ported yet",
        });
    }

    let input = graph_to_source_input(graph);
    let mut lgraph = source_port::import_graph(&input).map_err(Error::SourceImport)?;
    if has_parent_nodes {
        source_port::execute_ported_compound_processors(&mut lgraph)
            .map_err(Error::SourcePipeline)?;
    } else {
        source_port::execute_ported_processors(&mut lgraph).map_err(Error::SourcePipeline)?;
    }
    Ok(source_graph_to_layout_result(&lgraph))
}

fn graph_to_source_input(graph: &Graph) -> ElkInputGraph {
    ElkInputGraph {
        id: graph.id.clone(),
        options: layered_options_to_source(graph),
        nodes: graph
            .nodes
            .iter()
            .map(|node| ElkInputNode {
                id: node.id.clone(),
                width: node.width,
                height: node.height,
                parent: node.parent.clone(),
                direction: node.direction.map(direction_to_source),
                hierarchy_handling: match (node.kind, node.hierarchy_handling) {
                    (NodeKind::Group, Some(hierarchy_handling)) => {
                        Some(hierarchy_handling_to_source(hierarchy_handling))
                    }
                    (NodeKind::Group, None) => Some(hierarchy_handling_to_source(
                        graph.options.layered.hierarchy_handling,
                    )),
                    (NodeKind::Leaf, _) => None,
                },
                layer_constraint: None,
                port_constraints: None,
                node_label_placement: match node.kind {
                    NodeKind::Group => NodeLabelPlacement::InsideTopCenter,
                    NodeKind::Leaf => NodeLabelPlacement::Fixed,
                },
                nested_spacing_base: match node.kind {
                    NodeKind::Group => Some(30.0),
                    NodeKind::Leaf => None,
                },
                label: node
                    .label
                    .map(|label| ElkInputLabel::center("", label.width, label.height)),
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|edge| ElkInputEdge {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                label: edge
                    .label
                    .map(|label| ElkInputLabel::center("", label.width, label.height)),
                minlen: edge.minlen,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            })
            .collect(),
    }
}

fn layered_options_to_source(graph: &Graph) -> SourceLayeredOptions {
    let mut options =
        SourceLayeredOptions::mermaid_flowchart_defaults(direction_to_source(graph.direction));
    options.hierarchy_handling =
        hierarchy_handling_to_source(graph.options.layered.hierarchy_handling);
    options.edge_routing = edge_routing_to_source(graph.options.layered.edge_routing);
    options.cycle_breaking_strategy =
        cycle_breaking_to_source(graph.options.layered.cycle_breaking);
    options.node_placement_strategy =
        node_placement_to_source(graph.options.layered.node_placement);
    options.consider_model_order_strategy = if graph.options.layered.consider_model_order {
        model_order_to_source(graph.options.layered.model_order)
    } else {
        source_port::OrderingStrategy::None
    };
    options.force_node_model_order = graph.options.layered.force_node_model_order;
    options.merge_edges = graph.options.layered.merge_edges;
    options.merge_hierarchy_edges = graph.options.layered.merge_hierarchy_edges;
    options.unnecessary_bendpoints = graph.options.layered.unnecessary_bendpoints;
    options.self_loop_distribution =
        self_loop_distribution_to_source(graph.options.layered.self_loop_distribution);
    options.greedy_switch_activation_threshold = 0;
    options.greedy_switch_hierarchical_type = GreedySwitchType::TwoSided;
    options
}

fn direction_to_source(direction: Direction) -> ElkDirection {
    match direction {
        Direction::Left => ElkDirection::Left,
        Direction::Right => ElkDirection::Right,
        Direction::Up => ElkDirection::Up,
        Direction::Down => ElkDirection::Down,
    }
}

fn hierarchy_handling_to_source(
    hierarchy_handling: HierarchyHandling,
) -> source_port::HierarchyHandling {
    match hierarchy_handling {
        HierarchyHandling::IncludeChildren => source_port::HierarchyHandling::IncludeChildren,
        HierarchyHandling::SeparateChildren => source_port::HierarchyHandling::SeparateChildren,
    }
}

fn edge_routing_to_source(edge_routing: EdgeRouting) -> source_port::EdgeRouting {
    match edge_routing {
        EdgeRouting::Orthogonal => source_port::EdgeRouting::Orthogonal,
        EdgeRouting::Polyline => source_port::EdgeRouting::Polyline,
    }
}

fn cycle_breaking_to_source(
    cycle_breaking: CycleBreakingStrategy,
) -> source_port::CycleBreakingStrategy {
    match cycle_breaking {
        CycleBreakingStrategy::Greedy => source_port::CycleBreakingStrategy::Greedy,
        CycleBreakingStrategy::DepthFirst => source_port::CycleBreakingStrategy::DepthFirst,
        CycleBreakingStrategy::Interactive => source_port::CycleBreakingStrategy::Interactive,
        CycleBreakingStrategy::ModelOrder => source_port::CycleBreakingStrategy::ModelOrder,
        CycleBreakingStrategy::GreedyModelOrder => {
            source_port::CycleBreakingStrategy::GreedyModelOrder
        }
    }
}

fn node_placement_to_source(
    node_placement: NodePlacementStrategy,
) -> source_port::NodePlacementStrategy {
    match node_placement {
        NodePlacementStrategy::Simple => source_port::NodePlacementStrategy::Simple,
        NodePlacementStrategy::NetworkSimplex => source_port::NodePlacementStrategy::NetworkSimplex,
        NodePlacementStrategy::LinearSegments => source_port::NodePlacementStrategy::LinearSegments,
        NodePlacementStrategy::BrandesKoepf => source_port::NodePlacementStrategy::BrandesKoepf,
    }
}

fn model_order_to_source(model_order: ModelOrderStrategy) -> source_port::OrderingStrategy {
    match model_order {
        ModelOrderStrategy::None => source_port::OrderingStrategy::None,
        ModelOrderStrategy::NodesAndEdges => source_port::OrderingStrategy::NodesAndEdges,
        ModelOrderStrategy::PreferEdges => source_port::OrderingStrategy::PreferEdges,
        ModelOrderStrategy::PreferNodes => source_port::OrderingStrategy::PreferNodes,
    }
}

fn self_loop_distribution_to_source(
    self_loop_distribution: SelfLoopDistributionStrategy,
) -> source_port::SelfLoopDistributionStrategy {
    match self_loop_distribution {
        SelfLoopDistributionStrategy::North => source_port::SelfLoopDistributionStrategy::North,
        SelfLoopDistributionStrategy::Equally => source_port::SelfLoopDistributionStrategy::Equally,
    }
}

fn source_graph_to_layout_result(graph: &LGraph) -> LayoutResult {
    let mut result = SourceLayoutAccumulator {
        add_unnecessary_bendpoints: graph.options.unnecessary_bendpoints,
        ..Default::default()
    };
    append_source_graph_layout(graph, LPoint::default(), 0, &mut result);
    result.into_layout_result()
}

#[derive(Debug, Default)]
struct SourceLayoutAccumulator {
    nodes: Vec<NodeLayout>,
    edges: Vec<OrderedEdgeLayout>,
    compound_edges: HashMap<String, Vec<CompoundEdgeLayoutSegment>>,
    add_unnecessary_bendpoints: bool,
}

impl SourceLayoutAccumulator {
    fn into_layout_result(mut self) -> LayoutResult {
        for segments in self.compound_edges.values_mut() {
            segments.sort_by(compare_compound_layout_segments);
            if let Some(edge) =
                merge_compound_edge_segments(segments, self.add_unnecessary_bendpoints)
            {
                self.edges.push(OrderedEdgeLayout {
                    model_order: segments
                        .iter()
                        .filter_map(|segment| segment.model_order)
                        .min(),
                    edge,
                });
            }
        }
        self.edges.sort_by(|left, right| {
            left.model_order
                .unwrap_or(usize::MAX)
                .cmp(&right.model_order.unwrap_or(usize::MAX))
                .then_with(|| left.edge.id.cmp(&right.edge.id))
        });

        LayoutResult {
            nodes: self.nodes,
            edges: self.edges.into_iter().map(|ordered| ordered.edge).collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct OrderedEdgeLayout {
    model_order: Option<usize>,
    edge: EdgeLayout,
}

fn append_source_graph_layout(
    graph: &LGraph,
    parent_origin: LPoint,
    graph_depth: usize,
    result: &mut SourceLayoutAccumulator,
) {
    let graph_origin = LPoint {
        x: parent_origin.x + graph.offset.x + graph.padding.left,
        y: parent_origin.y + graph.offset.y + graph.padding.top,
    };

    result.nodes.extend(
        graph
            .layerless_nodes
            .iter()
            .filter(|node| node.kind == LNodeKind::Normal)
            .map(|node| NodeLayout {
                id: node.id.clone(),
                x: graph_origin.x + node.position.x + node.size.width / 2.0,
                y: graph_origin.y + node.position.y + node.size.height / 2.0,
                width: node.size.width,
                height: node.size.height,
            }),
    );

    for node in &graph.layerless_nodes {
        let Some(nested_graph) = node.nested_graph.as_deref() else {
            continue;
        };
        append_source_graph_layout(
            nested_graph,
            LPoint {
                x: graph_origin.x + node.position.x,
                y: graph_origin.y + node.position.y,
            },
            graph_depth + 1,
            result,
        );
    }

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !edge_has_layout_endpoints(graph, result, edge_index, edge) {
            continue;
        }

        let edge_layout = EdgeLayout {
            id: edge.id.clone(),
            points: edge_points(graph, edge)
                .into_iter()
                .map(|point| Point {
                    x: graph_origin.x + point.x,
                    y: graph_origin.y + point.y,
                })
                .collect(),
            labels: edge_labels(graph_origin, edge),
        };

        let compound_segments = compound_layout_segments_for_edge(graph, edge_index);
        if !compound_segments.is_empty() {
            for segment in compound_segments {
                let original_edge_id = segment.original_edge_id.clone();
                let edge_layout = edge_layout_for_original_edge(
                    &edge_layout,
                    graph_origin,
                    edge,
                    original_edge_id.as_str(),
                );
                result
                    .compound_edges
                    .entry(original_edge_id)
                    .or_default()
                    .push(CompoundEdgeLayoutSegment {
                        original_edge_id: segment.original_edge_id,
                        segment: segment.segment,
                        graph_depth,
                        model_order: segment.model_order.or(edge.model_order),
                        edge: edge_layout,
                    });
            }
        } else {
            result.edges.push(OrderedEdgeLayout {
                model_order: edge.model_order,
                edge: edge_layout,
            });
        }
    }
}

fn edge_layout_for_original_edge(
    edge: &EdgeLayout,
    graph_origin: LPoint,
    source_edge: &source_port::LayeredEdge,
    original_edge_id: &str,
) -> EdgeLayout {
    let mut edge = edge.clone();
    edge.id = original_edge_id.to_string();
    edge.labels = edge_labels_for_original_edge(graph_origin, source_edge, original_edge_id);
    edge
}

fn edge_labels_for_original_edge(
    graph_origin: LPoint,
    edge: &source_port::LayeredEdge,
    original_edge_id: &str,
) -> Vec<EdgeLabelLayout> {
    edge.labels
        .iter()
        .filter(|label| {
            label
                .original_label_edge
                .as_deref()
                .unwrap_or(original_edge_id)
                == original_edge_id
        })
        .map(|label| EdgeLabelLayout {
            x: graph_origin.x + label.position.x,
            y: graph_origin.y + label.position.y,
            width: label.size.width,
            height: label.size.height,
        })
        .collect()
}

fn compound_layout_segments_for_edge(
    graph: &LGraph,
    edge_index: usize,
) -> Vec<CompoundLayoutSegment> {
    let segments = graph
        .cross_hierarchy_edges
        .iter()
        .filter_map(|segment| {
            (segment.edge == edge_index).then(|| CompoundLayoutSegment {
                original_edge_id: segment.original_edge_id.clone(),
                model_order: segment.original_model_order,
                segment: segment.segment,
            })
        })
        .collect::<Vec<_>>();

    if !segments.is_empty() {
        return segments;
    }

    graph.edges[edge_index]
        .compound_segment
        .map(|segment| {
            vec![CompoundLayoutSegment {
                original_edge_id: graph.edges[edge_index].id.clone(),
                model_order: graph.edges[edge_index].model_order,
                segment,
            }]
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
struct CompoundLayoutSegment {
    original_edge_id: String,
    model_order: Option<usize>,
    segment: source_port::CompoundEdgeSegment,
}

#[derive(Debug, Clone)]
struct CompoundEdgeLayoutSegment {
    original_edge_id: String,
    segment: source_port::CompoundEdgeSegment,
    graph_depth: usize,
    model_order: Option<usize>,
    edge: EdgeLayout,
}

/// Merge hierarchy-local edge segments following ELK's compound postprocessor.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CompoundGraphPostprocessor.java
fn merge_compound_edge_segments(
    segments: &[CompoundEdgeLayoutSegment],
    add_unnecessary_bendpoints: bool,
) -> Option<EdgeLayout> {
    let first = segments.first()?;
    let mut points = Vec::new();
    let mut labels = Vec::new();
    let mut last_point = None;

    if let Some(source) = first.edge.points.first().copied() {
        push_distinct_point(&mut points, source);
    }

    for segment in segments {
        let segment_points = &segment.edge.points;
        if segment_points.is_empty() {
            continue;
        }

        let bend_points = if segment_points.len() > 2 {
            &segment_points[1..segment_points.len() - 1]
        } else {
            &[][..]
        };

        if let (Some(previous), Some(next)) = (
            last_point,
            bend_points.first().or_else(|| segment_points.last()),
        ) && compound_boundary_needs_bendpoint(previous, *next, add_unnecessary_bendpoints)
            && let Some(source) = segment_points.first()
        {
            push_distinct_point(&mut points, *source);
        }

        for point in bend_points {
            push_distinct_point(&mut points, *point);
        }
        labels.extend(segment.edge.labels.iter().cloned());
        last_point = bend_points
            .last()
            .copied()
            .or_else(|| segment_points.first().copied());
    }

    if let Some(target) = segments
        .last()
        .and_then(|segment| segment.edge.points.last())
        .copied()
    {
        push_distinct_point(&mut points, target);
    }

    Some(EdgeLayout {
        id: first.original_edge_id.clone(),
        points,
        labels,
    })
}

fn compound_boundary_needs_bendpoint(
    previous: Point,
    next: Point,
    add_unnecessary_bendpoints: bool,
) -> bool {
    const ORTHOGONAL_TOLERANCE: f64 = 0.001;
    let x_diff_enough = (previous.x - next.x).abs() > ORTHOGONAL_TOLERANCE;
    let y_diff_enough = (previous.y - next.y).abs() > ORTHOGONAL_TOLERANCE;
    if add_unnecessary_bendpoints {
        x_diff_enough || y_diff_enough
    } else {
        x_diff_enough && y_diff_enough
    }
}

fn push_distinct_point(points: &mut Vec<Point>, point: Point) {
    if points.last().is_some_and(|last| *last == point) {
        return;
    }
    points.push(point);
}

fn compare_compound_layout_segments(
    left: &CompoundEdgeLayoutSegment,
    right: &CompoundEdgeLayoutSegment,
) -> std::cmp::Ordering {
    compare_compound_segments(left.segment, right.segment)
        .then_with(|| left.graph_depth.cmp(&right.graph_depth))
}

fn compare_compound_segments(
    left: source_port::CompoundEdgeSegment,
    right: source_port::CompoundEdgeSegment,
) -> std::cmp::Ordering {
    match (left, right) {
        (
            source_port::CompoundEdgeSegment::Output { .. },
            source_port::CompoundEdgeSegment::Input { .. },
        ) => std::cmp::Ordering::Less,
        (
            source_port::CompoundEdgeSegment::Input { .. },
            source_port::CompoundEdgeSegment::Output { .. },
        ) => std::cmp::Ordering::Greater,
        (
            source_port::CompoundEdgeSegment::Output { depth: left },
            source_port::CompoundEdgeSegment::Output { depth: right },
        ) => right.cmp(&left),
        (
            source_port::CompoundEdgeSegment::Input { depth: left },
            source_port::CompoundEdgeSegment::Input { depth: right },
        ) => left.cmp(&right),
    }
}

fn edge_has_layout_endpoints(
    graph: &LGraph,
    result: &SourceLayoutAccumulator,
    edge_index: usize,
    edge: &source_port::LayeredEdge,
) -> bool {
    if !graph.edge_source_attached(edge_index) || !graph.edge_target_attached(edge_index) {
        return false;
    }

    endpoint_has_layout(graph, result, edge.source, edge.source_node_id.as_str())
        && endpoint_has_layout(graph, result, edge.target, edge.target_node_id.as_str())
}

fn endpoint_has_layout(
    graph: &LGraph,
    result: &SourceLayoutAccumulator,
    endpoint: PortRef,
    original_node_id: &str,
) -> bool {
    graph
        .layerless_nodes
        .get(endpoint.node)
        .is_some_and(|node| node.kind == LNodeKind::Normal)
        || result.nodes.iter().any(|node| node.id == original_node_id)
}

fn edge_points(graph: &LGraph, edge: &source_port::LayeredEdge) -> Vec<source_port::LPoint> {
    let mut points = Vec::with_capacity(edge.bend_points.len() + 2);
    points.push(port_anchor(graph, edge.source));
    points.extend(edge.bend_points.iter().copied());
    points.push(port_anchor(graph, edge.target));
    points
}

fn edge_labels(graph_origin: LPoint, edge: &source_port::LayeredEdge) -> Vec<EdgeLabelLayout> {
    edge.labels
        .iter()
        .map(|label| EdgeLabelLayout {
            x: graph_origin.x + label.position.x,
            y: graph_origin.y + label.position.y,
            width: label.size.width,
            height: label.size.height,
        })
        .collect()
}

fn port_anchor(graph: &LGraph, port_ref: PortRef) -> source_port::LPoint {
    let node = &graph.layerless_nodes[port_ref.node];
    let port = &node.ports[port_ref.port];
    source_port::LPoint {
        x: node.position.x + port.position.x + port.anchor.x,
        y: node.position.y + port.position.y + port.anchor.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Leaf,
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            minlen: 1,
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
    fn source_ported_layout_places_connected_nodes_in_direction_order() {
        let graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);

        let result = layout_source_ported(&graph, Algorithm::Layered).unwrap();

        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
        assert!(b.y > a.y);
        assert!(edge.points.len() >= 2);
        assert_eq!(edge.points.first().unwrap().y, a.y + a.height / 2.0);
        assert_eq!(edge.points.last().unwrap().y, b.y - b.height / 2.0);
    }

    #[test]
    fn source_ported_layout_honors_left_right_direction() {
        let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
        graph.direction = Direction::Right;

        let result = layout_source_ported(&graph, Algorithm::Layered).unwrap();

        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        assert!(b.x > a.x);
    }

    #[test]
    fn source_ported_layout_routes_long_edge_after_joiner() {
        let graph = flat_graph(
            vec![leaf("A"), leaf("B"), leaf("C")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("A-C", "A", "C"),
            ],
        );

        let result = layout_source_ported(&graph, Algorithm::Layered).unwrap();

        let long = result.edges.iter().find(|edge| edge.id == "A-C").unwrap();
        assert_eq!(
            result.edges.iter().filter(|edge| edge.id == "A-C").count(),
            1
        );
        assert!(long.points.len() > 4);
    }

    #[test]
    fn source_ported_layout_exports_edge_label_layouts() {
        let mut labelled = edge("A-C", "A", "C");
        labelled.label = Some(Label {
            width: 48.0,
            height: 12.0,
        });
        let graph = flat_graph(
            vec![leaf("A"), leaf("B"), leaf("C")],
            vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
        );

        let result = layout_source_ported(&graph, Algorithm::Layered).unwrap();

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
    fn source_ported_layout_exports_nested_compound_nodes_with_parent_offset() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut second_child = leaf("B");
        second_child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Down),
                    hierarchy_handling: None,
                    label: None,
                },
                child,
                second_child,
            ],
            vec![edge("A-B", "A", "B")],
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let result = layout_source_ported(&graph, Algorithm::Layered).unwrap();

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
    fn source_ported_layout_routes_cross_hierarchy_edge() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Down),
                    hierarchy_handling: None,
                    label: None,
                },
                child,
            ],
            vec![edge("cluster-A", "cluster", "A")],
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let result = layout_source_ported(&graph, Algorithm::Layered).unwrap();

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
    fn source_ported_layout_exports_edge_from_nested_child_to_outer_node() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Down),
                    hierarchy_handling: None,
                    label: None,
                },
                child,
                leaf("B"),
            ],
            vec![edge("A-B", "A", "B")],
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let result = layout_source_ported(&graph, Algorithm::Layered).unwrap();
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
    fn source_ported_layout_rejects_separate_hierarchy_until_ported() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Down),
                    hierarchy_handling: None,
                    label: None,
                },
                child,
            ],
            Vec::new(),
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::SeparateChildren;

        let err = layout_source_ported(&graph, Algorithm::Layered).unwrap_err();

        assert!(matches!(err, Error::UnsupportedSourceGraph { .. }));
    }
}
