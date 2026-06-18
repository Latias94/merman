//! Temporary compatibility backend.
//!
//! This module preserves the pre-port `flowchart-elk` behavior while the source-backed ELK
//! layered implementation lands under `source_port`. Do not extend this module for Mermaid / ELK
//! parity work; new layout behavior must be ported from the pinned Eclipse ELK source.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

const DEFAULT_NODE_SPACING: f64 = 50.0;
const DEFAULT_LAYER_SPACING: f64 = 70.0;
const DEFAULT_GROUP_PADDING_X: f64 = 40.0;
const DEFAULT_GROUP_PADDING_Y: f64 = 48.0;
const DEFAULT_GROUP_LABEL_GAP: f64 = 10.0;
const PARALLEL_EDGE_SPACING: f64 = 12.0;

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
    pub options: LayoutOptions,
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
pub struct LayoutOptions {
    pub layered: LayeredOptions,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            layered: LayeredOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredOptions {
    pub hierarchy_handling: HierarchyHandling,
    pub edge_routing: EdgeRouting,
    pub cycle_breaking: CycleBreakingStrategy,
    pub node_placement: NodePlacementStrategy,
    pub model_order: ModelOrderStrategy,
    pub consider_model_order: bool,
    pub force_node_model_order: bool,
    pub merge_edges: bool,
    pub merge_hierarchy_edges: bool,
    pub unnecessary_bendpoints: bool,
    pub inside_self_loops_activate: bool,
    pub self_loop_distribution: SelfLoopDistributionStrategy,
    pub self_loop_ordering: SelfLoopOrderingStrategy,
}

impl Default for LayeredOptions {
    fn default() -> Self {
        Self {
            hierarchy_handling: HierarchyHandling::IncludeChildren,
            edge_routing: EdgeRouting::Orthogonal,
            cycle_breaking: CycleBreakingStrategy::Greedy,
            node_placement: NodePlacementStrategy::BrandesKoepf,
            model_order: ModelOrderStrategy::NodesAndEdges,
            consider_model_order: true,
            force_node_model_order: false,
            merge_edges: false,
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: true,
            inside_self_loops_activate: false,
            self_loop_distribution: SelfLoopDistributionStrategy::Equally,
            self_loop_ordering: SelfLoopOrderingStrategy::Stacked,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HierarchyHandling {
    #[default]
    IncludeChildren,
    SeparateChildren,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeRouting {
    #[default]
    Orthogonal,
    Polyline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CycleBreakingStrategy {
    ModelOrder,
    #[default]
    Greedy,
    DepthFirst,
    Interactive,
    GreedyModelOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodePlacementStrategy {
    Simple,
    NetworkSimplex,
    LinearSegments,
    #[default]
    BrandesKoepf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelOrderStrategy {
    None,
    #[default]
    NodesAndEdges,
    PreferEdges,
    PreferNodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelfLoopDistributionStrategy {
    North,
    #[default]
    Equally,
    NorthSouth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelfLoopOrderingStrategy {
    #[default]
    Stacked,
    ReverseStacked,
    Sequenced,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub width: f64,
    pub height: f64,
    pub parent: Option<String>,
    pub direction: Option<Direction>,
    pub hierarchy_handling: Option<HierarchyHandling>,
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
    pub labels: Vec<EdgeLabelLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabelLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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
    #[error("source-backed ELK layout does not support this graph yet: {reason}")]
    UnsupportedSourceGraph { reason: &'static str },
    #[error(transparent)]
    SourceImport(#[from] merman_elk_layered::ImportError),
    #[error(transparent)]
    SourcePipeline(#[from] merman_elk_layered::PipelineError),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    match algorithm {
        Algorithm::Layered => layered_layout(graph),
    }
}

fn layered_layout(graph: &Graph) -> Result<LayoutResult> {
    let index = GraphIndex::new(graph)?;
    let engine = LayoutEngine { graph, index };
    Ok(engine.run())
}

struct GraphIndex<'a> {
    nodes_by_id: HashMap<&'a str, &'a Node>,
    root_child_ids: Vec<&'a str>,
    child_ids_by_parent: HashMap<&'a str, Vec<&'a str>>,
    topo_order_by_id: HashMap<&'a str, usize>,
}

impl<'a> GraphIndex<'a> {
    fn new(graph: &'a Graph) -> Result<Self> {
        let mut nodes_by_id = HashMap::new();
        for node in &graph.nodes {
            if nodes_by_id.insert(node.id.as_str(), node).is_some() {
                return Err(Error::DuplicateNode {
                    id: node.id.clone(),
                });
            }
        }

        let mut root_child_ids = Vec::new();
        let mut child_ids_by_parent: HashMap<&'a str, Vec<&'a str>> = HashMap::new();
        for node in &graph.nodes {
            let Some(parent) = node.parent.as_deref() else {
                root_child_ids.push(node.id.as_str());
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
            root_child_ids,
            child_ids_by_parent,
            topo_order_by_id,
        })
    }

    fn direct_children(&self, parent_id: Option<&'a str>) -> Vec<&'a str> {
        match parent_id {
            Some(parent_id) => self
                .child_ids_by_parent
                .get(parent_id)
                .cloned()
                .unwrap_or_default(),
            None => self.root_child_ids.clone(),
        }
    }

    fn direct_child_under(
        &self,
        container_parent: Option<&'a str>,
        node_id: &'a str,
    ) -> Option<&'a str> {
        let mut current = node_id;
        loop {
            let node = self.nodes_by_id.get(current).copied()?;
            if node.parent.as_deref() == container_parent {
                return Some(current);
            }
            current = node.parent.as_deref()?;
        }
    }
}

struct LayoutEngine<'a> {
    graph: &'a Graph,
    index: GraphIndex<'a>,
}

impl<'a> LayoutEngine<'a> {
    fn run(&self) -> LayoutResult {
        let container = self.layout_container(None, self.graph.direction);
        let mut nodes_by_id: HashMap<String, NodeLayout> = container
            .nodes
            .into_iter()
            .map(|node| (node.id.clone(), node))
            .collect();
        let nodes = self
            .graph
            .nodes
            .iter()
            .filter_map(|node| nodes_by_id.remove(node.id.as_str()))
            .collect::<Vec<_>>();
        let edges = self.route_edges(&nodes);

        LayoutResult { nodes, edges }
    }

    fn layout_container(
        &self,
        parent_id: Option<&'a str>,
        direction: Direction,
    ) -> ContainerLayout {
        let child_ids = self.index.direct_children(parent_id);
        if child_ids.is_empty() {
            return ContainerLayout { nodes: Vec::new() };
        }

        let subtrees = child_ids
            .iter()
            .filter_map(|child_id| self.layout_node(child_id, direction))
            .collect::<Vec<_>>();
        if subtrees.is_empty() {
            return ContainerLayout { nodes: Vec::new() };
        }

        let size_by_id = subtrees
            .iter()
            .map(|subtree| {
                (
                    subtree.id,
                    NodeSize {
                        width: subtree.node.width,
                        height: subtree.node.height,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let ranks = self.assign_child_ranks(parent_id, &child_ids);
        let mut by_rank: BTreeMap<usize, Vec<&'a str>> = BTreeMap::new();
        for subtree in &subtrees {
            by_rank
                .entry(ranks.get(subtree.id).copied().unwrap_or(0))
                .or_default()
                .push(subtree.id);
        }

        for ids in by_rank.values_mut() {
            self.sort_rank_ids(ids);
        }
        let rank_edges =
            self.cycle_broken_rank_edges(&self.rank_edges_for_container(parent_id, &child_ids));
        self.minimize_rank_crossings(&mut by_rank, &rank_edges);

        let mut rank_span: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
        for (rank, ids) in &by_rank {
            let main = ids
                .iter()
                .filter_map(|id| size_by_id.get(*id).copied())
                .map(|size| main_size(size, direction))
                .fold(0.0, f64::max);
            let cross = ids
                .iter()
                .filter_map(|id| size_by_id.get(*id).copied())
                .map(|size| cross_size(size, direction))
                .sum::<f64>()
                + self.graph.spacing.node_node.max(0.0) * ids.len().saturating_sub(1) as f64;
            rank_span.insert(*rank, (main, cross));
        }

        let mut rank_main_center: BTreeMap<usize, f64> = BTreeMap::new();
        let mut cursor = 0.0;
        for (rank, (main, _)) in &rank_span {
            let half = main / 2.0;
            rank_main_center.insert(*rank, cursor + half);
            cursor += main + self.graph.spacing.layer_layer.max(0.0);
        }

        let mut cross_positions: HashMap<&'a str, f64> = HashMap::new();
        for (rank, ids) in &by_rank {
            let Some((_, total_cross)) = rank_span.get(&rank) else {
                continue;
            };
            let mut cross_cursor = -total_cross / 2.0;
            for id in ids {
                let Some(size) = size_by_id.get(*id).copied() else {
                    continue;
                };
                let cross = cross_size(size, direction);
                let cross_center = cross_cursor + cross / 2.0;
                cross_cursor += cross + self.graph.spacing.node_node.max(0.0);
                cross_positions.insert(*id, cross_center);
            }
        }
        align_rank_cross_positions(
            &by_rank,
            &rank_edges,
            &size_by_id,
            direction,
            self.graph.spacing.node_node.max(0.0),
            &mut cross_positions,
        );

        let mut positions: HashMap<&'a str, Point> = HashMap::new();
        for (rank, ids) in by_rank {
            for id in ids {
                let main = *rank_main_center.get(&rank).unwrap_or(&0.0);
                let cross_center = cross_positions.get(id).copied().unwrap_or(0.0);
                let (x, y) = orient_point(main, cross_center, direction);
                positions.insert(id, Point { x, y });
            }
        }

        let mut nodes = Vec::new();
        for mut subtree in subtrees {
            let Some(position) = positions.get(subtree.id).copied() else {
                continue;
            };
            translate_layout(&mut subtree.node, position.x, position.y);
            nodes.push(subtree.node);
            for mut descendant in subtree.descendants {
                translate_layout(&mut descendant, position.x, position.y);
                nodes.push(descendant);
            }
        }

        ContainerLayout { nodes }
    }

    fn layout_node(
        &self,
        node_id: &'a str,
        inherited_direction: Direction,
    ) -> Option<NodeSubtree<'a>> {
        let node = self.index.nodes_by_id.get(node_id).copied()?;
        if node.kind == NodeKind::Leaf {
            return Some(NodeSubtree {
                id: node.id.as_str(),
                node: NodeLayout {
                    id: node.id.clone(),
                    x: 0.0,
                    y: 0.0,
                    width: leaf_width(node),
                    height: leaf_height(node),
                },
                descendants: Vec::new(),
            });
        }

        let direction = node.direction.unwrap_or(inherited_direction);
        let mut child_container = self.layout_container(Some(node.id.as_str()), direction);
        let (width, height) = self.group_size(node, &child_container.nodes);
        if let Some(bounds) = Bounds::from_layouts(&child_container.nodes) {
            let label_block = label_block_height(
                node.label.as_ref(),
                self.graph.spacing.group_label_gap.max(0.0),
            );
            let content_center_y = if label_block > 0.0 {
                -height / 2.0
                    + self.graph.spacing.group_padding_y.max(0.0)
                    + label_block
                    + bounds.height() / 2.0
            } else {
                0.0
            };
            let dx = -bounds.center_x();
            let dy = content_center_y - bounds.center_y();
            for child in &mut child_container.nodes {
                translate_layout(child, dx, dy);
            }
        }

        Some(NodeSubtree {
            id: node.id.as_str(),
            node: NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            descendants: child_container.nodes,
        })
    }

    fn group_size(&self, node: &Node, children: &[NodeLayout]) -> (f64, f64) {
        let pad_x = self.graph.spacing.group_padding_x.max(0.0);
        let pad_y = self.graph.spacing.group_padding_y.max(0.0);
        let label_w = label_width(node.label.as_ref());
        let label_h = label_height(node.label.as_ref());
        let label_block =
            label_block_height(node.label.as_ref(), self.graph.spacing.group_label_gap);

        if let Some(bounds) = Bounds::from_layouts(children) {
            let width = (bounds.width() + pad_x * 2.0)
                .max(label_w + pad_x * 2.0)
                .max(node.width)
                .max(1.0);
            let height = (bounds.height() + pad_y * 2.0 + label_block.max(0.0))
                .max(label_h + pad_y * 2.0)
                .max(node.height)
                .max(1.0);
            return (width, height);
        }

        (
            node.width.max(label_w + pad_x * 2.0).max(1.0),
            node.height.max(label_h + pad_y * 2.0).max(1.0),
        )
    }

    fn assign_child_ranks(
        &self,
        parent_id: Option<&'a str>,
        child_ids: &[&'a str],
    ) -> HashMap<&'a str, usize> {
        let mut incoming: HashMap<&str, usize> = child_ids.iter().map(|id| (*id, 0)).collect();
        let mut outgoing: HashMap<&str, Vec<RankEdge<'a>>> =
            child_ids.iter().map(|id| (*id, Vec::new())).collect();
        let rank_edges =
            self.cycle_broken_rank_edges(&self.rank_edges_for_container(parent_id, child_ids));

        for edge in &rank_edges {
            outgoing.entry(edge.source).or_default().push(*edge);
            *incoming.entry(edge.target).or_default() += 1;
        }

        let mut queue: VecDeque<&str> = child_ids
            .iter()
            .copied()
            .filter(|id| incoming.get(id).copied().unwrap_or(0) == 0)
            .collect();
        let mut ranks: HashMap<&str, usize> = child_ids.iter().map(|id| (*id, 0)).collect();
        let mut visited = 0usize;

        while let Some(id) = queue.pop_front() {
            visited += 1;
            let base = ranks.get(id).copied().unwrap_or(0);
            for edge in outgoing.get(id).into_iter().flatten().copied() {
                let next = base.saturating_add(edge.minlen);
                ranks
                    .entry(edge.target)
                    .and_modify(|rank| *rank = (*rank).max(next))
                    .or_insert(next);
                if let Some(deg) = incoming.get_mut(edge.target) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push_back(edge.target);
                    }
                }
            }
        }

        if visited != child_ids.len() {
            let mut pending = child_ids
                .iter()
                .copied()
                .filter(|id| incoming.get(id).copied().unwrap_or(0) > 0)
                .collect::<Vec<_>>();
            self.sort_rank_ids(&mut pending);
            let start_rank = ranks.values().copied().max().unwrap_or(0).saturating_add(1);
            for (idx, id) in pending.into_iter().enumerate() {
                ranks.insert(id, start_rank + idx);
            }
        }

        self.tighten_ranks_toward_successors(child_ids, &rank_edges, &mut ranks);
        ranks
    }

    fn rank_edges_for_container(
        &self,
        parent_id: Option<&'a str>,
        child_ids: &[&'a str],
    ) -> Vec<RankEdge<'a>> {
        let child_set: HashSet<&str> = child_ids.iter().copied().collect();
        let mut rank_edges = Vec::new();
        for edge in &self.graph.edges {
            let Some(source) = self
                .index
                .direct_child_under(parent_id, edge.source.as_str())
            else {
                continue;
            };
            let Some(target) = self
                .index
                .direct_child_under(parent_id, edge.target.as_str())
            else {
                continue;
            };
            if source == target || !child_set.contains(source) || !child_set.contains(target) {
                continue;
            }
            rank_edges.push(RankEdge {
                source,
                target,
                minlen: edge.minlen.max(1),
                order: rank_edges.len(),
                model_order_backward: self.model_order_backward(source, target),
            });
        }
        rank_edges
    }

    fn model_order_backward(&self, source: &'a str, target: &'a str) -> bool {
        let source_order = self
            .index
            .topo_order_by_id
            .get(source)
            .copied()
            .unwrap_or(usize::MAX);
        let target_order = self
            .index
            .topo_order_by_id
            .get(target)
            .copied()
            .unwrap_or(usize::MAX);
        source_order > target_order
    }

    fn cycle_broken_rank_edges(&self, rank_edges: &[RankEdge<'a>]) -> Vec<RankEdge<'a>> {
        match self.graph.options.layered.cycle_breaking {
            CycleBreakingStrategy::ModelOrder | CycleBreakingStrategy::Interactive => rank_edges
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(edge_index, edge)| {
                    (!self.model_order_rank_edge_closes_cycle(
                        rank_edges,
                        edge_index,
                        edge.source,
                        edge.target,
                    ))
                    .then_some(edge)
                })
                .collect(),
            CycleBreakingStrategy::Greedy
            | CycleBreakingStrategy::DepthFirst
            | CycleBreakingStrategy::GreedyModelOrder => {
                let mut accepted_edges = Vec::new();
                let mut accepted_indices = Vec::new();
                for (edge_index, edge) in rank_edges.iter().copied().enumerate().rev() {
                    if rank_path_exists(&accepted_edges, edge.target, edge.source) {
                        continue;
                    }
                    accepted_edges.push(edge);
                    accepted_indices.push(edge_index);
                }
                accepted_indices.sort_unstable();
                accepted_indices
                    .into_iter()
                    .filter_map(|idx| rank_edges.get(idx).copied())
                    .collect()
            }
        }
    }

    fn model_order_rank_edge_closes_cycle(
        &self,
        rank_edges: &[RankEdge<'a>],
        edge_index: usize,
        source: &'a str,
        target: &'a str,
    ) -> bool {
        let source_order = self
            .index
            .topo_order_by_id
            .get(source)
            .copied()
            .unwrap_or(usize::MAX);
        let target_order = self
            .index
            .topo_order_by_id
            .get(target)
            .copied()
            .unwrap_or(usize::MAX);
        if source_order <= target_order {
            return false;
        }

        let rank_edges = rank_edges
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(idx, edge)| (idx != edge_index).then_some(edge))
            .collect::<Vec<_>>();
        rank_path_exists(&rank_edges, target, source)
    }

    fn tighten_ranks_toward_successors(
        &self,
        child_ids: &[&'a str],
        rank_edges: &[RankEdge<'a>],
        ranks: &mut HashMap<&'a str, usize>,
    ) {
        let mut changed = true;
        while changed {
            changed = false;
            let mut ids = child_ids.to_vec();
            ids.sort_by_key(|id| std::cmp::Reverse(ranks.get(*id).copied().unwrap_or(0)));

            for id in ids {
                let current = ranks.get(id).copied().unwrap_or(0);
                let lower_bound = best_incoming_rank(rank_edges, ranks, id).unwrap_or(0);
                let Some(upper_bound) = best_outgoing_rank(rank_edges, ranks, id) else {
                    continue;
                };

                if upper_bound > current && upper_bound >= lower_bound {
                    ranks.insert(id, upper_bound);
                    changed = true;
                }
            }
        }
    }

    fn sort_rank_ids(&self, ids: &mut Vec<&'a str>) {
        if self.graph.options.layered.force_node_model_order
            || self.graph.options.layered.consider_model_order
        {
            ids.sort_by_key(|id| {
                self.index
                    .topo_order_by_id
                    .get(*id)
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        } else {
            ids.sort_unstable();
        }
    }

    fn minimize_rank_crossings(
        &self,
        by_rank: &mut BTreeMap<usize, Vec<&'a str>>,
        rank_edges: &[RankEdge<'a>],
    ) {
        if by_rank.len() < 2 || self.graph.options.layered.force_node_model_order {
            return;
        }

        for _ in 0..4 {
            let ranks = by_rank.keys().copied().collect::<Vec<_>>();
            for rank in ranks.iter().copied().skip(1) {
                let order = node_order_by_id(by_rank);
                self.sort_rank_by_neighbors(by_rank, rank, rank_edges, &order, false);
            }

            let ranks = by_rank.keys().copied().rev().collect::<Vec<_>>();
            for rank in ranks.into_iter().skip(1) {
                let order = node_order_by_id(by_rank);
                self.sort_rank_by_neighbors(by_rank, rank, rank_edges, &order, true);
            }
        }
    }

    fn sort_rank_by_neighbors(
        &self,
        by_rank: &mut BTreeMap<usize, Vec<&'a str>>,
        rank: usize,
        rank_edges: &[RankEdge<'a>],
        order: &HashMap<&'a str, usize>,
        reverse: bool,
    ) {
        let Some(ids) = by_rank.get_mut(&rank) else {
            return;
        };
        if ids.len() < 2 {
            return;
        }

        let mut ordered = ids
            .iter()
            .copied()
            .map(|id| {
                (
                    id,
                    neighbor_order(id, rank_edges, order, reverse),
                    self.index
                        .topo_order_by_id
                        .get(id)
                        .copied()
                        .unwrap_or(usize::MAX),
                )
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|a, b| match (a.1, b.1) {
            (Some(a_order), Some(b_order)) => a_order
                .partial_cmp(&b_order)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| a.0.cmp(b.0)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.2.cmp(&b.2).then_with(|| a.0.cmp(b.0)),
        });
        ids.clear();
        ids.extend(ordered.into_iter().map(|(id, _, _)| id));
    }

    fn route_edges(&self, nodes: &[NodeLayout]) -> Vec<EdgeLayout> {
        let node_by_id: HashMap<&str, &NodeLayout> =
            nodes.iter().map(|node| (node.id.as_str(), node)).collect();
        let parallel_edges = self.parallel_edge_positions();

        self.graph
            .edges
            .iter()
            .filter_map(|edge| {
                let source = node_by_id.get(edge.source.as_str()).copied()?;
                let target = node_by_id.get(edge.target.as_str()).copied()?;
                let direction = self.edge_direction(edge);
                let mut points = match self.graph.options.layered.edge_routing {
                    EdgeRouting::Orthogonal => orthogonal_route(source, target, direction),
                    EdgeRouting::Polyline => polyline_route(source, target, direction),
                };
                if !self.graph.options.layered.merge_edges {
                    if let Some((index, total)) = parallel_edges.get(edge.id.as_str()).copied() {
                        offset_parallel_route(&mut points, direction, index, total);
                    }
                }
                if let Some(label) = edge.label {
                    insert_label_clearance(&mut points, label, direction);
                }
                Some(EdgeLayout {
                    id: edge.id.clone(),
                    points,
                    labels: Vec::new(),
                })
            })
            .collect()
    }

    fn parallel_edge_positions(&self) -> HashMap<&'a str, (usize, usize)> {
        let mut totals: HashMap<(&str, &str), usize> = HashMap::new();
        for edge in &self.graph.edges {
            *totals
                .entry((edge.source.as_str(), edge.target.as_str()))
                .or_default() += 1;
        }

        let mut seen: HashMap<(&str, &str), usize> = HashMap::new();
        let mut positions = HashMap::new();
        for edge in &self.graph.edges {
            let key = (edge.source.as_str(), edge.target.as_str());
            let total = totals.get(&key).copied().unwrap_or(1);
            let index = seen.entry(key).or_default();
            positions.insert(edge.id.as_str(), (*index, total));
            *index += 1;
        }
        positions
    }

    fn edge_direction(&self, edge: &'a Edge) -> Direction {
        let container = self.lowest_common_container(edge.source.as_str(), edge.target.as_str());
        self.effective_direction(container)
    }

    fn lowest_common_container(&self, source_id: &'a str, target_id: &'a str) -> Option<&'a str> {
        let source_chain = self.container_chain_for_endpoint(source_id);
        let target_chain = self.container_chain_for_endpoint(target_id);
        source_chain
            .into_iter()
            .find(|container| target_chain.contains(container))
            .flatten()
    }

    fn container_chain_for_endpoint(&self, node_id: &'a str) -> Vec<Option<&'a str>> {
        let Some(node) = self.index.nodes_by_id.get(node_id).copied() else {
            return vec![None];
        };

        let mut chain = Vec::new();
        if node.kind == NodeKind::Group {
            chain.push(Some(node.id.as_str()));
        }

        let mut parent = node.parent.as_deref();
        while let Some(parent_id) = parent {
            chain.push(Some(parent_id));
            parent = self
                .index
                .nodes_by_id
                .get(parent_id)
                .and_then(|node| node.parent.as_deref());
        }
        chain.push(None);
        chain
    }

    fn effective_direction(&self, container_id: Option<&'a str>) -> Direction {
        let Some(container_id) = container_id else {
            return self.graph.direction;
        };
        let Some(node) = self.index.nodes_by_id.get(container_id).copied() else {
            return self.graph.direction;
        };
        let inherited = self.effective_direction(node.parent.as_deref());
        node.direction.unwrap_or(inherited)
    }
}

fn best_incoming_rank<'a>(
    rank_edges: &[RankEdge<'a>],
    ranks: &HashMap<&'a str, usize>,
    node_id: &'a str,
) -> Option<usize> {
    rank_edges
        .iter()
        .filter(|edge| edge.target == node_id)
        .filter_map(|edge| {
            ranks
                .get(edge.source)
                .map(|rank| rank.saturating_add(edge.minlen))
        })
        .max()
}

fn best_outgoing_rank<'a>(
    rank_edges: &[RankEdge<'a>],
    ranks: &HashMap<&'a str, usize>,
    node_id: &'a str,
) -> Option<usize> {
    rank_edges
        .iter()
        .filter(|edge| edge.source == node_id)
        .filter_map(|edge| {
            ranks
                .get(edge.target)
                .map(|rank| rank.saturating_sub(edge.minlen))
        })
        .min()
}

fn node_order_by_id<'a>(by_rank: &BTreeMap<usize, Vec<&'a str>>) -> HashMap<&'a str, usize> {
    by_rank
        .values()
        .flat_map(|ids| ids.iter().copied().enumerate().map(|(idx, id)| (id, idx)))
        .collect()
}

fn neighbor_order<'a>(
    node_id: &'a str,
    rank_edges: &[RankEdge<'a>],
    order: &HashMap<&'a str, usize>,
    reverse: bool,
) -> Option<f64> {
    let mut sum = 0.0;
    let mut weight_sum = 0.0;
    for edge in rank_edges {
        let neighbor = if reverse {
            (edge.source == node_id).then_some(edge.target)
        } else {
            (edge.target == node_id).then_some(edge.source)
        };
        let Some(neighbor) = neighbor else {
            continue;
        };
        let Some(neighbor_order) = order.get(neighbor).copied() else {
            continue;
        };
        sum += neighbor_order as f64;
        weight_sum += 1.0;
    }

    (weight_sum > 0.0).then_some(sum / weight_sum)
}

fn align_rank_cross_positions<'a>(
    by_rank: &BTreeMap<usize, Vec<&'a str>>,
    rank_edges: &[RankEdge<'a>],
    size_by_id: &HashMap<&'a str, NodeSize>,
    direction: Direction,
    spacing: f64,
    positions: &mut HashMap<&'a str, f64>,
) {
    if by_rank.len() < 2 {
        return;
    }

    for _ in 0..4 {
        for rank in by_rank.keys().copied().collect::<Vec<_>>() {
            align_single_rank_cross_positions(
                by_rank, rank, rank_edges, size_by_id, direction, spacing, positions, false,
            );
        }
        for rank in by_rank.keys().copied().rev().collect::<Vec<_>>() {
            align_single_rank_cross_positions(
                by_rank, rank, rank_edges, size_by_id, direction, spacing, positions, true,
            );
        }
    }
}

fn align_single_rank_cross_positions<'a>(
    by_rank: &BTreeMap<usize, Vec<&'a str>>,
    rank: usize,
    rank_edges: &[RankEdge<'a>],
    size_by_id: &HashMap<&'a str, NodeSize>,
    direction: Direction,
    spacing: f64,
    positions: &mut HashMap<&'a str, f64>,
    reverse: bool,
) {
    let Some(ids) = by_rank.get(&rank) else {
        return;
    };
    if ids.len() != 1 {
        return;
    }

    let desired = ids
        .iter()
        .copied()
        .map(|id| {
            (
                id,
                preferred_neighbor_cross_position(id, rank_edges, positions, reverse)
                    .unwrap_or_else(|| positions.get(id).copied().unwrap_or(0.0)),
            )
        })
        .collect::<HashMap<_, _>>();

    let packed = pack_rank_cross_positions(ids, &desired, size_by_id, direction, spacing);
    positions.extend(packed);
}

fn preferred_neighbor_cross_position<'a>(
    node_id: &'a str,
    rank_edges: &[RankEdge<'a>],
    positions: &HashMap<&'a str, f64>,
    reverse: bool,
) -> Option<f64> {
    let mut candidates = rank_edges
        .iter()
        .filter_map(|edge| {
            let neighbor = if reverse {
                (edge.source == node_id).then_some(edge.target)
            } else {
                (edge.target == node_id).then_some(edge.source)
            };
            let position = positions.get(neighbor?).copied()?;
            Some((*edge, position))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|(a, _), (b, _)| {
        a.model_order_backward
            .cmp(&b.model_order_backward)
            .then_with(|| b.order.cmp(&a.order))
    });
    candidates.first().map(|(_, position)| *position)
}

fn pack_rank_cross_positions<'a>(
    ids: &[&'a str],
    desired: &HashMap<&'a str, f64>,
    size_by_id: &HashMap<&'a str, NodeSize>,
    direction: Direction,
    spacing: f64,
) -> HashMap<&'a str, f64> {
    let mut out = HashMap::new();
    let mut cursor: Option<f64> = None;

    for id in ids {
        let Some(size) = size_by_id.get(*id).copied() else {
            continue;
        };
        let cross = cross_size(size, direction);
        let wanted = desired.get(*id).copied().unwrap_or(0.0);
        let min_center = cursor.map(|right| right + spacing + cross / 2.0);
        let center = min_center.map_or(wanted, |min_center| wanted.max(min_center));
        cursor = Some(center + cross / 2.0);
        out.insert(*id, center);
    }

    if out.is_empty() {
        return out;
    }

    let mut shift = 0.0;
    let mut weight = 0.0;
    for id in ids {
        let Some(center) = out.get(*id).copied() else {
            continue;
        };
        let wanted = desired.get(*id).copied().unwrap_or(center);
        shift += wanted - center;
        weight += 1.0;
    }
    if weight > 0.0 {
        let shift = shift / weight;
        for center in out.values_mut() {
            *center += shift;
        }
    }

    out
}

fn rank_path_exists<'a>(rank_edges: &[RankEdge<'a>], start: &'a str, target: &'a str) -> bool {
    let mut seen = HashSet::new();
    let mut stack = vec![start];

    while let Some(id) = stack.pop() {
        if id == target {
            return true;
        }
        if !seen.insert(id) {
            continue;
        }
        for edge in rank_edges.iter().copied() {
            if edge.source == id {
                stack.push(edge.target);
            }
        }
    }

    false
}

#[derive(Debug, Clone)]
struct ContainerLayout {
    nodes: Vec<NodeLayout>,
}

#[derive(Debug, Clone)]
struct NodeSubtree<'a> {
    id: &'a str,
    node: NodeLayout,
    descendants: Vec<NodeLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RankEdge<'a> {
    source: &'a str,
    target: &'a str,
    minlen: usize,
    order: usize,
    model_order_backward: bool,
}

#[derive(Debug, Clone, Copy)]
struct NodeSize {
    width: f64,
    height: f64,
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

    fn from_layouts(layouts: &[NodeLayout]) -> Option<Self> {
        let mut bounds = Self::new();
        for layout in layouts {
            bounds.include_layout(layout);
        }
        bounds.is_valid().then_some(bounds)
    }

    fn include_layout(&mut self, layout: &NodeLayout) {
        self.min_x = self.min_x.min(layout.x - layout.width / 2.0);
        self.min_y = self.min_y.min(layout.y - layout.height / 2.0);
        self.max_x = self.max_x.max(layout.x + layout.width / 2.0);
        self.max_y = self.max_y.max(layout.y + layout.height / 2.0);
    }

    fn is_valid(&self) -> bool {
        self.min_x.is_finite()
            && self.min_y.is_finite()
            && self.max_x.is_finite()
            && self.max_y.is_finite()
            && self.max_x >= self.min_x
            && self.max_y >= self.min_y
    }

    fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f64 {
        self.max_y - self.min_y
    }

    fn center_x(&self) -> f64 {
        (self.min_x + self.max_x) / 2.0
    }

    fn center_y(&self) -> f64 {
        (self.min_y + self.max_y) / 2.0
    }
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

fn polyline_route(source: &NodeLayout, target: &NodeLayout, direction: Direction) -> Vec<Point> {
    let (start, end) = endpoint_pair(source, target, direction);
    vec![start, end]
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

fn offset_parallel_route(
    points: &mut Vec<Point>,
    direction: Direction,
    index: usize,
    total: usize,
) {
    if total <= 1 || points.len() < 2 {
        return;
    }

    let center = (total.saturating_sub(1)) as f64 / 2.0;
    let offset = (index as f64 - center) * PARALLEL_EDGE_SPACING;
    if offset.abs() <= 1e-9 {
        return;
    }

    if points.len() == 2 {
        let start = points[0];
        let end = points[1];
        *points = match direction {
            Direction::Down | Direction::Up => {
                let mid_y = (start.y + end.y) / 2.0;
                vec![
                    start,
                    Point {
                        x: start.x + offset,
                        y: mid_y,
                    },
                    Point {
                        x: end.x + offset,
                        y: mid_y,
                    },
                    end,
                ]
            }
            Direction::Right | Direction::Left => {
                let mid_x = (start.x + end.x) / 2.0;
                vec![
                    start,
                    Point {
                        x: mid_x,
                        y: start.y + offset,
                    },
                    Point {
                        x: mid_x,
                        y: end.y + offset,
                    },
                    end,
                ]
            }
        };
        return;
    }

    let last = points.len().saturating_sub(1);
    for point in points.iter_mut().take(last).skip(1) {
        match direction {
            Direction::Down | Direction::Up => point.x += offset,
            Direction::Right | Direction::Left => point.y += offset,
        }
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

fn main_size(size: NodeSize, direction: Direction) -> f64 {
    match direction {
        Direction::Down | Direction::Up => size.height.max(1.0),
        Direction::Right | Direction::Left => size.width.max(1.0),
    }
}

fn cross_size(size: NodeSize, direction: Direction) -> f64 {
    match direction {
        Direction::Down | Direction::Up => size.width.max(1.0),
        Direction::Right | Direction::Left => size.height.max(1.0),
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

fn leaf_width(node: &Node) -> f64 {
    node.width.max(label_width(node.label.as_ref())).max(1.0)
}

fn leaf_height(node: &Node) -> f64 {
    node.height.max(label_height(node.label.as_ref())).max(1.0)
}

fn label_width(label: Option<&Label>) -> f64 {
    label.map(|label| label.width.max(0.0)).unwrap_or(0.0)
}

fn label_height(label: Option<&Label>) -> f64 {
    label.map(|label| label.height.max(0.0)).unwrap_or(0.0)
}

fn label_block_height(label: Option<&Label>, gap: f64) -> f64 {
    let height = label_height(label);
    if height > 0.0 {
        height + gap.max(0.0)
    } else {
        0.0
    }
}

fn translate_layout(layout: &mut NodeLayout, dx: f64, dy: f64) {
    layout.x += dx;
    layout.y += dy;
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
                    hierarchy_handling: None,
                    label: None,
                },
                Node {
                    id: "B".to_string(),
                    kind: NodeKind::Leaf,
                    width: 80.0,
                    height: 40.0,
                    parent: None,
                    direction: None,
                    hierarchy_handling: None,
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
                hierarchy_handling: None,
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
    fn layered_layout_uses_local_group_direction() {
        let mut graph = graph_with_two_nodes();
        graph
            .nodes
            .insert(0, group("cluster", None, Some(Direction::Right)));
        for node in &mut graph.nodes {
            if node.kind == NodeKind::Leaf {
                node.parent = Some("cluster".to_string());
            }
        }

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();

        assert!(b.x > a.x);
        assert_eq!(edge.points[0].x, a.x + a.width / 2.0);
        assert_eq!(edge.points[1].x, b.x - b.width / 2.0);
    }

    #[test]
    fn layered_layout_composes_nested_group_directions() {
        let graph = Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![
                group("outer", None, Some(Direction::Right)),
                group("inner", Some("outer"), Some(Direction::Down)),
                leaf("A", Some("inner")),
                leaf("B", Some("inner")),
                leaf("C", Some("outer")),
            ],
            edges: vec![edge("A-B", "A", "B"), edge("inner-C", "inner", "C")],
            ..Default::default()
        };

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let inner = result.nodes.iter().find(|node| node.id == "inner").unwrap();
        let c = result.nodes.iter().find(|node| node.id == "C").unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();

        assert!(b.y > a.y);
        assert!(c.x > inner.x);
    }

    #[test]
    fn layered_layout_routes_cross_group_edge_with_common_container_direction() {
        let graph = Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![
                group("cluster", None, Some(Direction::Right)),
                leaf("A", Some("cluster")),
                leaf("B", Some("cluster")),
                leaf("C", None),
            ],
            edges: vec![edge("B-C", "B", "C")],
            ..Default::default()
        };

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let c = result.nodes.iter().find(|node| node.id == "C").unwrap();
        let route = result.edges.iter().find(|edge| edge.id == "B-C").unwrap();

        assert!(c.y > b.y);
        assert_eq!(route.points.first().unwrap().y, b.y + b.height / 2.0);
        assert_eq!(route.points.last().unwrap().y, c.y - c.height / 2.0);
        assert!(route.points.len() >= 2);
    }

    #[test]
    fn layered_layout_keeps_sibling_groups_non_overlapping() {
        let graph = Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![
                group("left", None, Some(Direction::Down)),
                leaf("A", Some("left")),
                group("right", None, Some(Direction::Down)),
                leaf("B", Some("right")),
            ],
            edges: Vec::new(),
            ..Default::default()
        };

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let left = result.nodes.iter().find(|node| node.id == "left").unwrap();
        let right = result.nodes.iter().find(|node| node.id == "right").unwrap();

        assert!(left.x + left.width / 2.0 <= right.x - right.width / 2.0);
    }

    #[test]
    fn layered_layout_separates_parallel_edges() {
        let mut graph = graph_with_two_nodes();
        graph.edges.push(Edge {
            id: "A-B-2".to_string(),
            source: "A".to_string(),
            target: "B".to_string(),
            label: None,
            minlen: 1,
        });

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let first = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
        let second = result.edges.iter().find(|edge| edge.id == "A-B-2").unwrap();

        assert_ne!(first.points, second.points);
        assert!(first.points.len() > 2);
        assert!(second.points.len() > 2);
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

    #[test]
    fn layered_layout_breaks_feedback_edges_before_ranking_branches() {
        let graph = Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![
                leaf("A", None),
                leaf("B", None),
                leaf("C", None),
                leaf("D", None),
                leaf("I", None),
                leaf("E", None),
                leaf("F", None),
                leaf("H", None),
                leaf("G", None),
            ],
            edges: vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-D", "C", "D"),
                edge("C-I", "C", "I"),
                edge("C-E", "C", "E"),
                edge("D-F", "D", "F"),
                edge("E-F", "E", "F"),
                edge("F-H", "F", "H"),
                edge("H-B", "H", "B"),
                edge("F-G", "F", "G"),
            ],
            ..Default::default()
        };

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let c = result.nodes.iter().find(|node| node.id == "C").unwrap();
        let d = result.nodes.iter().find(|node| node.id == "D").unwrap();
        let i = result.nodes.iter().find(|node| node.id == "I").unwrap();
        let e = result.nodes.iter().find(|node| node.id == "E").unwrap();
        let f = result.nodes.iter().find(|node| node.id == "F").unwrap();
        let h = result.nodes.iter().find(|node| node.id == "H").unwrap();
        let g = result.nodes.iter().find(|node| node.id == "G").unwrap();

        assert!(c.y < b.y);
        assert_eq!(d.y, i.y);
        assert_eq!(i.y, e.y);
        assert_ne!(d.x, i.x);
        assert_ne!(i.x, e.x);
        assert!(f.y > e.y);
        assert_eq!(h.y, g.y);
    }

    #[test]
    fn layered_layout_prefers_forward_edges_when_aligning_feedback_targets() {
        let graph = Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![
                leaf("A", None),
                leaf("B", None),
                leaf("C", None),
                leaf("D", None),
            ],
            edges: vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-D", "C", "D"),
                edge("D-B", "D", "B"),
            ],
            ..Default::default()
        };

        let result = layout(&graph, Algorithm::Layered).unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let d = result.nodes.iter().find(|node| node.id == "D").unwrap();

        assert_eq!(a.x, b.x);
        assert_ne!(d.x, b.x);
    }

    fn graph_with_two_nodes() -> Graph {
        Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes: vec![leaf("A", None), leaf("B", None)],
            edges: vec![edge("A-B", "A", "B")],
            ..Default::default()
        }
    }

    fn leaf(id: &str, parent: Option<&str>) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Leaf,
            width: 80.0,
            height: 40.0,
            parent: parent.map(str::to_string),
            direction: None,
            hierarchy_handling: None,
            label: None,
        }
    }

    fn group(id: &str, parent: Option<&str>, direction: Option<Direction>) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Group,
            width: 0.0,
            height: 0.0,
            parent: parent.map(str::to_string),
            direction,
            hierarchy_handling: None,
            label: Some(Label {
                width: 80.0,
                height: 20.0,
            }),
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
}
