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

mod compat;
pub use merman_elk_layered as source_port;

use source_port::{
    ElkDirection, ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, GreedySwitchType,
    LGraph, LNodeKind, LayeredOptions as SourceLayeredOptions, PortRef,
};

pub use compat::{
    Algorithm, CycleBreakingStrategy, Direction, Edge, EdgeLayout, EdgeRouting, Error, Graph,
    HierarchyHandling, Label, LayeredOptions, LayoutOptions, LayoutResult, Node, NodeKind,
    NodeLayout, Point, Result, Spacing,
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

fn layout_layered_source_ported(graph: &Graph) -> Result<LayoutResult> {
    if graph.nodes.iter().any(|node| node.parent.is_some()) {
        return Err(Error::UnsupportedSourceGraph {
            reason: "compound hierarchy requires the ported compound layout runner",
        });
    }
    let input = graph_to_source_input(graph);
    let mut lgraph = source_port::import_graph(&input).map_err(Error::SourceImport)?;
    source_port::execute_ported_processors(&mut lgraph).map_err(Error::SourcePipeline)?;
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
                hierarchy_handling: match node.kind {
                    NodeKind::Group => Some(hierarchy_handling_to_source(
                        graph.options.layered.hierarchy_handling,
                    )),
                    NodeKind::Leaf => None,
                },
                layer_constraint: None,
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
    options.consider_model_order_strategy = if graph.options.layered.consider_model_order {
        source_port::OrderingStrategy::NodesAndEdges
    } else {
        source_port::OrderingStrategy::None
    };
    options.force_node_model_order = graph.options.layered.force_node_model_order;
    options.merge_edges = graph.options.layered.merge_edges;
    options.spacing.node_node = graph.spacing.node_node;
    options.spacing.node_node_between_layers = graph.spacing.layer_layer;
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
        CycleBreakingStrategy::ModelOrder => source_port::CycleBreakingStrategy::ModelOrder,
    }
}

fn source_graph_to_layout_result(graph: &LGraph) -> LayoutResult {
    LayoutResult {
        nodes: graph
            .layerless_nodes
            .iter()
            .filter(|node| node.kind == LNodeKind::Normal)
            .map(|node| NodeLayout {
                id: node.id.clone(),
                x: node.position.x + node.size.width / 2.0 + graph.offset.x,
                y: node.position.y + node.size.height / 2.0 + graph.offset.y,
                width: node.size.width,
                height: node.size.height,
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .filter(|edge| edge_has_layout_endpoints(graph, edge.source, edge.target))
            .map(|edge| EdgeLayout {
                id: edge.id.clone(),
                points: edge_points(graph, edge)
                    .into_iter()
                    .map(|point| Point {
                        x: point.x + graph.offset.x,
                        y: point.y + graph.offset.y,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn edge_has_layout_endpoints(graph: &LGraph, source: PortRef, target: PortRef) -> bool {
    graph
        .layerless_nodes
        .get(source.node)
        .is_some_and(|node| node.kind == LNodeKind::Normal)
        && graph
            .layerless_nodes
            .get(target.node)
            .is_some_and(|node| node.kind == LNodeKind::Normal)
}

fn edge_points(graph: &LGraph, edge: &source_port::LayeredEdge) -> Vec<source_port::LPoint> {
    let mut points = Vec::with_capacity(edge.bend_points.len() + 2);
    points.push(port_anchor(graph, edge.source));
    points.extend(edge.bend_points.iter().copied());
    points.push(port_anchor(graph, edge.target));
    points
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
    fn source_ported_layout_rejects_compound_graph_until_compound_runner_is_ported() {
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
                    label: None,
                },
                child,
            ],
            Vec::new(),
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let err = layout_source_ported(&graph, Algorithm::Layered).unwrap_err();

        assert!(matches!(err, Error::UnsupportedSourceGraph { .. }));
    }
}
