#![forbid(unsafe_code)]

//! Optional ELK layout engine integration for `merman`.
//!
//! This crate owns the ELK-specific dependency surface so higher-level crates can keep the layout
//! engine optional. The current backend is a lightweight deterministic subset of Mermaid's
//! `elk.layered` path, not a complete Eclipse ELK port.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

const DEFAULT_NODE_SPACING: f64 = 50.0;
const DEFAULT_LAYER_SPACING: f64 = 70.0;
const DEFAULT_GROUP_PADDING_X: f64 = 40.0;
const DEFAULT_GROUP_PADDING_Y: f64 = 48.0;
const DEFAULT_GROUP_LABEL_GAP: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Mermaid's default ELK layout, equivalent to upstream `elk.layered`.
    Layered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    Left,
    Right,
    Up,
    #[default]
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeKind {
    #[default]
    Leaf,
    Group,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Graph {
    pub id: String,
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub spacing: Spacing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub node_node: f64,
    pub layer_layer: f64,
    pub group_padding_x: f64,
    pub group_padding_y: f64,
    pub group_label_gap: f64,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            node_node: DEFAULT_NODE_SPACING,
            layer_layer: DEFAULT_LAYER_SPACING,
            group_padding_x: DEFAULT_GROUP_PADDING_X,
            group_padding_y: DEFAULT_GROUP_PADDING_Y,
            group_label_gap: DEFAULT_GROUP_LABEL_GAP,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub width: f64,
    pub height: f64,
    pub parent: Option<String>,
    pub direction: Option<Direction>,
    pub label: Option<Label>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<Label>,
    pub minlen: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Label {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutResult {
    pub nodes: Vec<NodeLayout>,
    pub edges: Vec<EdgeLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLayout {
    pub id: String,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ELK layout algorithm is not implemented yet: {algorithm:?}")]
    UnsupportedAlgorithm { algorithm: Algorithm },
    #[error("ELK graph has duplicate node id: {id}")]
    DuplicateNode { id: String },
    #[error("ELK edge `{edge_id}` references missing node `{node_id}`")]
    MissingEndpoint { edge_id: String, node_id: String },
    #[error("ELK node `{node_id}` references missing parent `{parent_id}`")]
    MissingParent { node_id: String, parent_id: String },
    #[error("ELK parent assignment would create a cycle at node `{node_id}`")]
    ParentCycle { node_id: String },
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    match algorithm {
        Algorithm::Layered => layered_layout(graph),
    }
}

fn layered_layout(graph: &Graph) -> Result<LayoutResult> {
    let index = GraphIndex::new(graph)?;
    let placement = place_leaf_nodes(graph, &index);
    let node_layouts = expand_groups(graph, &index, placement);
    let edges = route_edges(graph, &node_layouts, graph.direction);

    Ok(LayoutResult {
        nodes: node_layouts,
        edges,
    })
}

struct GraphIndex<'a> {
    nodes_by_id: HashMap<&'a str, &'a Node>,
    leaf_ids: Vec<&'a str>,
    child_ids_by_parent: HashMap<&'a str, Vec<&'a str>>,
    topo_order_by_id: HashMap<&'a str, usize>,
}

impl<'a> GraphIndex<'a> {
    fn new(graph: &'a Graph) -> Result<Self> {
        let mut nodes_by_id = HashMap::new();
        let mut leaf_ids = Vec::new();
        let mut child_ids_by_parent: HashMap<&'a str, Vec<&'a str>> = HashMap::new();
        for node in &graph.nodes {
            if nodes_by_id.insert(node.id.as_str(), node).is_some() {
                return Err(Error::DuplicateNode {
                    id: node.id.clone(),
                });
            }
            if node.kind == NodeKind::Leaf {
                leaf_ids.push(node.id.as_str());
            }
        }

        for node in &graph.nodes {
            let Some(parent) = node.parent.as_deref() else {
                continue;
            };
            if !nodes_by_id.contains_key(parent) {
                return Err(Error::MissingParent {
                    node_id: node.id.clone(),
                    parent_id: parent.to_string(),
                });
            }
            child_ids_by_parent
                .entry(parent)
                .or_default()
                .push(node.id.as_str());
        }

        for edge in &graph.edges {
            if !nodes_by_id.contains_key(edge.source.as_str()) {
                return Err(Error::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.source.clone(),
                });
            }
            if !nodes_by_id.contains_key(edge.target.as_str()) {
                return Err(Error::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.target.clone(),
                });
            }
        }

        detect_parent_cycles(graph, &nodes_by_id)?;

        let topo_order_by_id = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (node.id.as_str(), idx))
            .collect();

        Ok(Self {
            nodes_by_id,
            leaf_ids,
            child_ids_by_parent,
            topo_order_by_id,
        })
    }
}

#[derive(Debug, Clone)]
struct LeafPlacement {
    by_id: HashMap<String, NodeLayout>,
}

fn place_leaf_nodes(graph: &Graph, index: &GraphIndex<'_>) -> LeafPlacement {
    let ranks = assign_leaf_ranks(graph, index);
    let mut by_rank: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for id in &index.leaf_ids {
        by_rank
            .entry(*ranks.get(id).unwrap_or(&0))
            .or_default()
            .push(id);
    }

    for ids in by_rank.values_mut() {
        ids.sort_by_key(|id| {
            index
                .topo_order_by_id
                .get(*id)
                .copied()
                .unwrap_or(usize::MAX)
        });
    }

    let mut rank_span: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    for (rank, ids) in &by_rank {
        let main = ids
            .iter()
            .filter_map(|id| index.nodes_by_id.get(id).copied())
            .map(|node| main_size(node, graph.direction))
            .fold(0.0, f64::max);
        let cross = ids
            .iter()
            .filter_map(|id| index.nodes_by_id.get(id).copied())
            .map(|node| cross_size(node, graph.direction))
            .sum::<f64>()
            + graph.spacing.node_node.max(0.0) * ids.len().saturating_sub(1) as f64;
        rank_span.insert(*rank, (main, cross));
    }

    let mut rank_main_center: BTreeMap<usize, f64> = BTreeMap::new();
    let mut cursor = 0.0;
    for (rank, (main, _)) in &rank_span {
        let half = main / 2.0;
        rank_main_center.insert(*rank, cursor + half);
        cursor += main + graph.spacing.layer_layer.max(0.0);
    }

    let mut by_id = HashMap::new();
    for (rank, ids) in by_rank {
        let Some((_, total_cross)) = rank_span.get(&rank) else {
            continue;
        };
        let mut cross_cursor = -total_cross / 2.0;
        for id in ids {
            let Some(node) = index.nodes_by_id.get(id).copied() else {
                continue;
            };
            let cross = cross_size(node, graph.direction);
            let main = *rank_main_center.get(&rank).unwrap_or(&0.0);
            let cross_center = cross_cursor + cross / 2.0;
            cross_cursor += cross + graph.spacing.node_node.max(0.0);
            let (x, y) = orient_point(main, cross_center, graph.direction);
            by_id.insert(
                node.id.clone(),
                NodeLayout {
                    id: node.id.clone(),
                    x,
                    y,
                    width: node.width.max(1.0),
                    height: node.height.max(1.0),
                },
            );
        }
    }

    LeafPlacement { by_id }
}

fn assign_leaf_ranks<'a>(graph: &'a Graph, index: &GraphIndex<'a>) -> HashMap<&'a str, usize> {
    let leaf_set: HashSet<&str> = index.leaf_ids.iter().copied().collect();
    let mut incoming: HashMap<&str, usize> = index.leaf_ids.iter().map(|id| (*id, 0)).collect();
    let mut outgoing: HashMap<&str, Vec<(&str, usize)>> =
        index.leaf_ids.iter().map(|id| (*id, Vec::new())).collect();

    for edge in &graph.edges {
        let source = edge.source.as_str();
        let target = edge.target.as_str();
        if source == target || !leaf_set.contains(source) || !leaf_set.contains(target) {
            continue;
        }
        outgoing
            .entry(source)
            .or_default()
            .push((target, edge.minlen.max(1)));
        *incoming.entry(target).or_default() += 1;
    }

    let mut queue: VecDeque<&str> = index
        .leaf_ids
        .iter()
        .copied()
        .filter(|id| incoming.get(id).copied().unwrap_or(0) == 0)
        .collect();
    let mut ranks: HashMap<&str, usize> = index.leaf_ids.iter().map(|id| (*id, 0)).collect();
    let mut visited = 0usize;

    while let Some(id) = queue.pop_front() {
        visited += 1;
        let base = ranks.get(id).copied().unwrap_or(0);
        for (target, minlen) in outgoing.get(id).into_iter().flatten().copied() {
            let next = base.saturating_add(minlen);
            ranks
                .entry(target)
                .and_modify(|rank| *rank = (*rank).max(next))
                .or_insert(next);
            if let Some(deg) = incoming.get_mut(target) {
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    queue.push_back(target);
                }
            }
        }
    }

    if visited != index.leaf_ids.len() {
        let mut pending: Vec<&str> = index
            .leaf_ids
            .iter()
            .copied()
            .filter(|id| incoming.get(id).copied().unwrap_or(0) > 0)
            .collect();
        pending.sort_by_key(|id| {
            index
                .topo_order_by_id
                .get(*id)
                .copied()
                .unwrap_or(usize::MAX)
        });
        let start_rank = ranks.values().copied().max().unwrap_or(0).saturating_add(1);
        for (idx, id) in pending.into_iter().enumerate() {
            ranks.insert(id, start_rank + idx);
        }
    }

    ranks
}

fn expand_groups(
    graph: &Graph,
    index: &GraphIndex<'_>,
    placement: LeafPlacement,
) -> Vec<NodeLayout> {
    let mut layouts = placement.by_id;
    for node in graph.nodes.iter().rev() {
        if node.kind != NodeKind::Group {
            continue;
        }
        let Some(children) = index.child_ids_by_parent.get(node.id.as_str()) else {
            let width = node.width.max(label_width(node.label.as_ref())).max(1.0);
            let height = node.height.max(label_height(node.label.as_ref())).max(1.0);
            layouts.insert(
                node.id.clone(),
                NodeLayout {
                    id: node.id.clone(),
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                },
            );
            continue;
        };

        let mut bounds = Bounds::new();
        let mut has_child = false;
        for child_id in children {
            if let Some(child) = layouts.get(*child_id) {
                bounds.include_layout(child);
                has_child = true;
            }
        }

        if !has_child {
            continue;
        }

        let label_w = label_width(node.label.as_ref());
        let label_h = label_height(node.label.as_ref());
        let width = (bounds.max_x - bounds.min_x + graph.spacing.group_padding_x * 2.0)
            .max(label_w + graph.spacing.group_padding_x)
            .max(node.width)
            .max(1.0);
        let height = (bounds.max_y - bounds.min_y
            + graph.spacing.group_padding_y * 2.0
            + if label_h > 0.0 {
                label_h + graph.spacing.group_label_gap
            } else {
                0.0
            })
        .max(label_h + graph.spacing.group_padding_y)
        .max(node.height)
        .max(1.0);

        layouts.insert(
            node.id.clone(),
            NodeLayout {
                id: node.id.clone(),
                x: (bounds.min_x + bounds.max_x) / 2.0,
                y: (bounds.min_y + bounds.max_y) / 2.0,
                width,
                height,
            },
        );
    }

    graph
        .nodes
        .iter()
        .filter_map(|node| layouts.remove(node.id.as_str()))
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn new() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    fn include_layout(&mut self, layout: &NodeLayout) {
        self.min_x = self.min_x.min(layout.x - layout.width / 2.0);
        self.min_y = self.min_y.min(layout.y - layout.height / 2.0);
        self.max_x = self.max_x.max(layout.x + layout.width / 2.0);
        self.max_y = self.max_y.max(layout.y + layout.height / 2.0);
    }
}

fn route_edges(graph: &Graph, nodes: &[NodeLayout], direction: Direction) -> Vec<EdgeLayout> {
    let node_by_id: HashMap<&str, &NodeLayout> =
        nodes.iter().map(|node| (node.id.as_str(), node)).collect();

    graph
        .edges
        .iter()
        .filter_map(|edge| {
            let source = node_by_id.get(edge.source.as_str()).copied()?;
            let target = node_by_id.get(edge.target.as_str()).copied()?;
            let mut points = orthogonal_route(source, target, direction);
            if let Some(label) = edge.label {
                insert_label_clearance(&mut points, label, direction);
            }
            Some(EdgeLayout {
                id: edge.id.clone(),
                points,
            })
        })
        .collect()
}

fn orthogonal_route(source: &NodeLayout, target: &NodeLayout, direction: Direction) -> Vec<Point> {
    let (start, end) = endpoint_pair(source, target, direction);
    if (start.x - end.x).abs() <= 1e-9 || (start.y - end.y).abs() <= 1e-9 {
        return vec![start, end];
    }

    match direction {
        Direction::Down | Direction::Up => {
            let mid_y = (start.y + end.y) / 2.0;
            vec![
                start,
                Point {
                    x: start.x,
                    y: mid_y,
                },
                Point { x: end.x, y: mid_y },
                end,
            ]
        }
        Direction::Right | Direction::Left => {
            let mid_x = (start.x + end.x) / 2.0;
            vec![
                start,
                Point {
                    x: mid_x,
                    y: start.y,
                },
                Point { x: mid_x, y: end.y },
                end,
            ]
        }
    }
}

fn endpoint_pair(source: &NodeLayout, target: &NodeLayout, direction: Direction) -> (Point, Point) {
    match direction {
        Direction::Down => (
            Point {
                x: source.x,
                y: source.y + source.height / 2.0,
            },
            Point {
                x: target.x,
                y: target.y - target.height / 2.0,
            },
        ),
        Direction::Up => (
            Point {
                x: source.x,
                y: source.y - source.height / 2.0,
            },
            Point {
                x: target.x,
                y: target.y + target.height / 2.0,
            },
        ),
        Direction::Right => (
            Point {
                x: source.x + source.width / 2.0,
                y: source.y,
            },
            Point {
                x: target.x - target.width / 2.0,
                y: target.y,
            },
        ),
        Direction::Left => (
            Point {
                x: source.x - source.width / 2.0,
                y: source.y,
            },
            Point {
                x: target.x + target.width / 2.0,
                y: target.y,
            },
        ),
    }
}

fn insert_label_clearance(points: &mut Vec<Point>, label: Label, direction: Direction) {
    if points.len() < 2 {
        return;
    }
    let mid = points.len() / 2;
    let a = points[mid - 1];
    let b = points[mid];
    if (a.x - b.x).abs() <= 1e-9 && (a.y - b.y).abs() <= 1e-9 {
        return;
    }

    let gap = match direction {
        Direction::Down | Direction::Up => label.height.max(0.0) / 2.0,
        Direction::Right | Direction::Left => label.width.max(0.0) / 2.0,
    };
    if gap <= 1.0 {
        return;
    }

    if (a.x - b.x).abs() <= 1e-9 {
        let sign = if b.y >= a.y { 1.0 } else { -1.0 };
        let y = (a.y + b.y) / 2.0;
        points.insert(
            mid,
            Point {
                x: a.x,
                y: y - sign * gap,
            },
        );
        points.insert(
            mid + 1,
            Point {
                x: b.x,
                y: y + sign * gap,
            },
        );
    } else if (a.y - b.y).abs() <= 1e-9 {
        let sign = if b.x >= a.x { 1.0 } else { -1.0 };
        let x = (a.x + b.x) / 2.0;
        points.insert(
            mid,
            Point {
                x: x - sign * gap,
                y: a.y,
            },
        );
        points.insert(
            mid + 1,
            Point {
                x: x + sign * gap,
                y: b.y,
            },
        );
    }
}

fn main_size(node: &Node, direction: Direction) -> f64 {
    match direction {
        Direction::Down | Direction::Up => node.height.max(1.0),
        Direction::Right | Direction::Left => node.width.max(1.0),
    }
}

fn cross_size(node: &Node, direction: Direction) -> f64 {
    match direction {
        Direction::Down | Direction::Up => node.width.max(1.0),
        Direction::Right | Direction::Left => node.height.max(1.0),
    }
}

fn orient_point(main: f64, cross: f64, direction: Direction) -> (f64, f64) {
    match direction {
        Direction::Down => (cross, main),
        Direction::Up => (cross, -main),
        Direction::Right => (main, cross),
        Direction::Left => (-main, cross),
    }
}

fn label_width(label: Option<&Label>) -> f64 {
    label.map(|label| label.width.max(0.0)).unwrap_or(0.0)
}

fn label_height(label: Option<&Label>) -> f64 {
    label.map(|label| label.height.max(0.0)).unwrap_or(0.0)
}

fn detect_parent_cycles<'a>(
    graph: &'a Graph,
    nodes_by_id: &HashMap<&'a str, &'a Node>,
) -> Result<()> {
    for node in &graph.nodes {
        let mut seen = HashSet::new();
        let mut current = node.parent.as_deref();
        while let Some(parent) = current {
            if !seen.insert(parent) {
                return Err(Error::ParentCycle {
                    node_id: node.id.clone(),
                });
            }
            current = nodes_by_id
                .get(parent)
                .and_then(|node| node.parent.as_deref());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layered_layout_places_connected_nodes_in_direction_order() {
        let graph = Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![
                Node {
                    id: "A".to_string(),
                    kind: NodeKind::Leaf,
                    width: 80.0,
                    height: 40.0,
                    parent: None,
                    direction: None,
                    label: None,
                },
                Node {
                    id: "B".to_string(),
                    kind: NodeKind::Leaf,
                    width: 80.0,
                    height: 40.0,
                    parent: None,
                    direction: None,
                    label: None,
                },
            ],
            edges: vec![Edge {
                id: "A-B".to_string(),
                source: "A".to_string(),
                target: "B".to_string(),
                label: None,
                minlen: 1,
            }],
            ..Default::default()
        };

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();

        assert!(b.y > a.y);
        assert_eq!(result.edges[0].points.len(), 2);
        assert_eq!(result.edges[0].points[0].y, a.y + a.height / 2.0);
        assert_eq!(result.edges[0].points[1].y, b.y - b.height / 2.0);
    }

    #[test]
    fn layered_layout_honors_left_right_direction() {
        let mut graph = graph_with_two_nodes();
        graph.direction = Direction::Right;

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();

        assert!(b.x > a.x);
        assert_eq!(result.edges[0].points[0].x, a.x + a.width / 2.0);
        assert_eq!(result.edges[0].points[1].x, b.x - b.width / 2.0);
    }

    #[test]
    fn layered_layout_expands_group_around_children_and_label() {
        let mut graph = graph_with_two_nodes();
        graph.nodes.insert(
            0,
            Node {
                id: "cluster".to_string(),
                kind: NodeKind::Group,
                width: 0.0,
                height: 0.0,
                parent: None,
                direction: Some(Direction::Down),
                label: Some(Label {
                    width: 300.0,
                    height: 24.0,
                }),
            },
        );
        for node in &mut graph.nodes {
            if node.kind == NodeKind::Leaf {
                node.parent = Some("cluster".to_string());
            }
        }

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let cluster = result
            .nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();

        assert!(cluster.width >= 300.0 + DEFAULT_GROUP_PADDING_X);
        assert!(cluster.y < b.y);
        assert!(cluster.x - cluster.width / 2.0 <= a.x - a.width / 2.0);
        assert!(cluster.x + cluster.width / 2.0 >= b.x + b.width / 2.0);
    }

    #[test]
    fn layered_layout_rejects_missing_edge_endpoint() {
        let mut graph = graph_with_two_nodes();
        graph.edges[0].target = "missing".to_string();

        let err = layout(&graph, Algorithm::Layered).unwrap_err();
        assert!(matches!(
            err,
            Error::MissingEndpoint {
                edge_id,
                node_id
            } if edge_id == "A-B" && node_id == "missing"
        ));
    }

    #[test]
    fn layered_layout_gives_cyclic_nodes_stable_separate_ranks() {
        let mut graph = graph_with_two_nodes();
        graph.edges.push(Edge {
            id: "B-A".to_string(),
            source: "B".to_string(),
            target: "A".to_string(),
            label: None,
            minlen: 1,
        });

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();

        assert_ne!(a.y, b.y);
    }

    fn graph_with_two_nodes() -> Graph {
        Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![
                Node {
                    id: "A".to_string(),
                    kind: NodeKind::Leaf,
                    width: 80.0,
                    height: 40.0,
                    parent: None,
                    direction: None,
                    label: None,
                },
                Node {
                    id: "B".to_string(),
                    kind: NodeKind::Leaf,
                    width: 80.0,
                    height: 40.0,
                    parent: None,
                    direction: None,
                    label: None,
                },
            ],
            edges: vec![Edge {
                id: "A-B".to_string(),
                source: "A".to_string(),
                target: "B".to_string(),
                label: None,
                minlen: 1,
            }],
            ..Default::default()
        }
    }
}
