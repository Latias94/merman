//! Brandes-Koepf node placement.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/bk/BKNodePlacer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/bk/BKAlignedLayout.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/bk/BKAligner.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/bk/BKCompactor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/bk/NeighborhoodInformation.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/options/Spacings.java

use std::collections::{HashMap, HashSet, VecDeque};

use crate::graph::{LGraph, LNodeKind, LayeredEdge, PortRef};
use crate::options::{EdgeStraighteningStrategy, FixedAlignment};

const MIN_LAYERS_FOR_CONFLICTS: usize = 3;

pub fn place_nodes_brandes_koepf(graph: &mut LGraph) {
    if graph.layers.is_empty() {
        return;
    }

    let ni = NeighborhoodInformation::build_for(graph);
    if ni.node_count == 0 {
        return;
    }

    let marked_edges = mark_conflicts(graph, &ni);
    let mut layouts = layouts_for_fixed_alignment(graph, &ni);

    for layout in &mut layouts {
        vertical_alignment(graph, &ni, layout, &marked_edges);
        inside_block_shift(graph, &ni, layout);
    }

    for layout in &mut layouts {
        horizontal_compaction(graph, &ni, layout);
    }

    let chosen = choose_layout(graph, &ni, layouts);
    apply_layout(graph, &ni, &chosen);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VDirection {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HDirection {
    Right,
    Left,
}

#[derive(Debug, Clone, PartialEq)]
struct BKAlignedLayout {
    root: Vec<usize>,
    block_size: Vec<f64>,
    align: Vec<usize>,
    inner_shift: Vec<f64>,
    sink: Vec<usize>,
    shift: Vec<f64>,
    y: Vec<Option<f64>>,
    vdir: VDirection,
    hdir: HDirection,
    straightened: Vec<bool>,
    only_dummies: Vec<bool>,
}

impl BKAlignedLayout {
    fn new(node_count: usize, vdir: VDirection, hdir: HDirection) -> Self {
        Self {
            root: vec![usize::MAX; node_count],
            block_size: vec![0.0; node_count],
            align: vec![usize::MAX; node_count],
            inner_shift: vec![0.0; node_count],
            sink: vec![usize::MAX; node_count],
            shift: vec![0.0; node_count],
            y: vec![None; node_count],
            vdir,
            hdir,
            straightened: vec![false; node_count],
            only_dummies: vec![true; node_count],
        }
    }

    fn layout_size(&self, graph: &LGraph, ni: &NeighborhoodInformation) -> f64 {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for node in ni.iter_nodes() {
            let id = ni.node_id[node];
            let root = self.root[id];
            let Some(y_min) = self.y[id] else {
                continue;
            };
            let y_max = y_min + self.block_size[root];
            min = min.min(y_min);
            max = max.max(y_max);
        }

        if min.is_finite() && max.is_finite() {
            max - min
        } else {
            graph
                .layers
                .iter()
                .flat_map(|layer| layer.nodes.iter().copied())
                .map(|node| graph.layerless_nodes[node].size.height)
                .fold(0.0, f64::max)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Neighbor {
    node: usize,
    edge: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NeighborhoodInformation {
    node_count: usize,
    node_id: Vec<usize>,
    id_to_node: Vec<usize>,
    layer_index: Vec<usize>,
    node_index: Vec<usize>,
    left_neighbors: Vec<Vec<Neighbor>>,
    right_neighbors: Vec<Vec<Neighbor>>,
}

impl NeighborhoodInformation {
    fn build_for(graph: &LGraph) -> Self {
        let node_count = graph.layers.iter().map(|layer| layer.nodes.len()).sum();
        let mut node_id = vec![usize::MAX; graph.layerless_nodes.len()];
        let mut id_to_node = Vec::with_capacity(node_count);
        let mut layer_index = vec![usize::MAX; graph.layers.len()];
        let mut node_index = vec![usize::MAX; node_count];

        let mut next_id = 0usize;
        for (layer_idx, layer) in graph.layers.iter().enumerate() {
            layer_index[layer_idx] = layer_idx;
            for (node_idx, node) in layer.nodes.iter().copied().enumerate() {
                node_id[node] = next_id;
                id_to_node.push(node);
                node_index[next_id] = node_idx;
                next_id += 1;
            }
        }

        let mut ni = Self {
            node_count,
            node_id,
            id_to_node,
            layer_index,
            node_index,
            left_neighbors: vec![Vec::new(); node_count],
            right_neighbors: vec![Vec::new(); node_count],
        };
        ni.determine_all_left_neighbors(graph);
        ni.determine_all_right_neighbors(graph);
        ni
    }

    fn iter_nodes(&self) -> impl Iterator<Item = usize> + '_ {
        self.id_to_node.iter().copied()
    }

    fn determine_all_left_neighbors(&mut self, graph: &LGraph) {
        let nodes = self.id_to_node.clone();
        for node in nodes {
            let id = self.node_id[node];
            let mut result = Vec::new();
            let mut max_priority = 0;

            for edge in graph.node_incoming_edges(node) {
                if is_self_loop_or_in_layer_edge(graph, edge) {
                    continue;
                }
                let edge_priority = graph.edges[edge].priority_straightness;
                if edge_priority > max_priority {
                    max_priority = edge_priority;
                    result.clear();
                }
                if edge_priority == max_priority {
                    result.push(Neighbor {
                        node: graph.edges[edge].source.node,
                        edge,
                    });
                }
            }

            result.sort_by_key(|neighbor| self.node_index[self.node_id[neighbor.node]]);
            self.left_neighbors[id] = result;
        }
    }

    fn determine_all_right_neighbors(&mut self, graph: &LGraph) {
        let nodes = self.id_to_node.clone();
        for node in nodes {
            let id = self.node_id[node];
            let mut result = Vec::new();
            let mut max_priority = 0;

            for edge in graph.node_outgoing_edges(node) {
                if is_self_loop_or_in_layer_edge(graph, edge) {
                    continue;
                }
                let edge_priority = graph.edges[edge].priority_straightness;
                if edge_priority > max_priority {
                    max_priority = edge_priority;
                    result.clear();
                }
                if edge_priority == max_priority {
                    result.push(Neighbor {
                        node: graph.edges[edge].target.node,
                        edge,
                    });
                }
            }

            result.sort_by_key(|neighbor| self.node_index[self.node_id[neighbor.node]]);
            self.right_neighbors[id] = result;
        }
    }
}

fn layouts_for_fixed_alignment(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
) -> Vec<BKAlignedLayout> {
    match graph.options.node_placement_bk_fixed_alignment {
        FixedAlignment::LeftDown => vec![BKAlignedLayout::new(
            ni.node_count,
            VDirection::Down,
            HDirection::Left,
        )],
        FixedAlignment::LeftUp => vec![BKAlignedLayout::new(
            ni.node_count,
            VDirection::Up,
            HDirection::Left,
        )],
        FixedAlignment::RightDown => vec![BKAlignedLayout::new(
            ni.node_count,
            VDirection::Down,
            HDirection::Right,
        )],
        FixedAlignment::RightUp => vec![BKAlignedLayout::new(
            ni.node_count,
            VDirection::Up,
            HDirection::Right,
        )],
        FixedAlignment::None | FixedAlignment::Balanced => vec![
            BKAlignedLayout::new(ni.node_count, VDirection::Down, HDirection::Right),
            BKAlignedLayout::new(ni.node_count, VDirection::Up, HDirection::Right),
            BKAlignedLayout::new(ni.node_count, VDirection::Down, HDirection::Left),
            BKAlignedLayout::new(ni.node_count, VDirection::Up, HDirection::Left),
        ],
    }
}

fn mark_conflicts(graph: &LGraph, ni: &NeighborhoodInformation) -> HashSet<usize> {
    let number_of_layers = graph.layers.len();
    let mut marked_edges = HashSet::new();
    if number_of_layers < MIN_LAYERS_FOR_CONFLICTS {
        return marked_edges;
    }

    let layer_sizes = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.len())
        .collect::<Vec<_>>();

    for i in 1..number_of_layers - 1 {
        let current_layer = &graph.layers[i + 1];
        let mut k_0 = 0usize;
        let mut l = 0usize;

        for l_1 in 0..layer_sizes[i + 1] {
            let v_l_i = current_layer.nodes[l_1];
            if l_1 == layer_sizes[i + 1] - 1
                || incident_to_inner_segment(graph, ni, v_l_i, i + 1, i)
            {
                let mut k_1 = layer_sizes[i].saturating_sub(1);
                if incident_to_inner_segment(graph, ni, v_l_i, i + 1, i)
                    && let Some(left) = ni.left_neighbors[ni.node_id[v_l_i]].first()
                {
                    k_1 = ni.node_index[ni.node_id[left.node]];
                }

                while l <= l_1 {
                    let v_l = current_layer.nodes[l];
                    if !incident_to_inner_segment(graph, ni, v_l, i + 1, i) {
                        for upper_neighbor in &ni.left_neighbors[ni.node_id[v_l]] {
                            let k = ni.node_index[ni.node_id[upper_neighbor.node]];
                            if k < k_0 || k > k_1 {
                                marked_edges.insert(upper_neighbor.edge);
                            }
                        }
                    }
                    l += 1;
                }

                k_0 = k_1;
            }
        }
    }

    marked_edges
}

fn incident_to_inner_segment(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    node: usize,
    layer1: usize,
    layer2: usize,
) -> bool {
    if graph.layerless_nodes[node].kind != LNodeKind::LongEdge {
        return false;
    }

    for edge in graph.node_incoming_edges(node) {
        let source = graph.edges[edge].source.node;
        if graph.layerless_nodes[source].kind == LNodeKind::LongEdge
            && graph.layerless_nodes[source].layer_index == Some(layer2)
            && graph.layerless_nodes[node].layer_index == Some(layer1)
            && ni.layer_index.get(layer2).copied() == Some(layer2)
        {
            return true;
        }
    }

    false
}

fn vertical_alignment(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &mut BKAlignedLayout,
    marked_edges: &HashSet<usize>,
) {
    for node in ni.iter_nodes() {
        let id = ni.node_id[node];
        layout.root[id] = id;
        layout.align[id] = id;
        layout.inner_shift[id] = 0.0;
        layout.only_dummies[id] = true;
    }

    let layer_iter: Box<dyn Iterator<Item = usize>> = match layout.hdir {
        HDirection::Left => Box::new((0..graph.layers.len()).rev()),
        HDirection::Right => Box::new(0..graph.layers.len()),
    };

    for layer_index in layer_iter {
        let layer = &graph.layers[layer_index];
        let mut r = match layout.vdir {
            VDirection::Up => isize::MAX,
            VDirection::Down => -1,
        };

        let node_iter: Box<dyn Iterator<Item = usize>> = match layout.vdir {
            VDirection::Up => Box::new(layer.nodes.iter().copied().rev()),
            VDirection::Down => Box::new(layer.nodes.iter().copied()),
        };

        for v_i_k in node_iter {
            let v_id = ni.node_id[v_i_k];
            let neighbors = match layout.hdir {
                HDirection::Left => &ni.right_neighbors[v_id],
                HDirection::Right => &ni.left_neighbors[v_id],
            };
            if neighbors.is_empty() {
                continue;
            }

            let d = neighbors.len();
            let low = ((d as f64 + 1.0) / 2.0).floor() as usize - 1;
            let high = ((d as f64 + 1.0) / 2.0).ceil() as usize - 1;

            if layout.vdir == VDirection::Up {
                for m in (low..=high).rev() {
                    if layout.align[v_id] == v_id {
                        let u_m = &neighbors[m];
                        let u_id = ni.node_id[u_m.node];
                        if !marked_edges.contains(&u_m.edge) && r > ni.node_index[u_id] as isize {
                            layout.align[u_id] = v_id;
                            layout.root[v_id] = layout.root[u_id];
                            layout.align[v_id] = layout.root[v_id];
                            let root = layout.root[v_id];
                            layout.only_dummies[root] &=
                                graph.layerless_nodes[v_i_k].kind == LNodeKind::LongEdge;
                            r = ni.node_index[u_id] as isize;
                        }
                    }
                }
            } else {
                for u_m in neighbors.iter().take(high + 1).skip(low) {
                    if layout.align[v_id] == v_id {
                        let u_id = ni.node_id[u_m.node];
                        if !marked_edges.contains(&u_m.edge) && r < ni.node_index[u_id] as isize {
                            layout.align[u_id] = v_id;
                            layout.root[v_id] = layout.root[u_id];
                            layout.align[v_id] = layout.root[v_id];
                            let root = layout.root[v_id];
                            layout.only_dummies[root] &=
                                graph.layerless_nodes[v_i_k].kind == LNodeKind::LongEdge;
                            r = ni.node_index[u_id] as isize;
                        }
                    }
                }
            }
        }
    }
}

fn inside_block_shift(graph: &LGraph, ni: &NeighborhoodInformation, layout: &mut BKAlignedLayout) {
    let blocks = blocks(layout);
    for root in blocks.keys().copied() {
        let root_node = ni.id_to_node[root];
        let mut space_above = graph.layerless_nodes[root_node].margin.top;
        let mut space_below = graph.layerless_nodes[root_node].size.height
            + graph.layerless_nodes[root_node].margin.bottom;
        layout.inner_shift[root] = 0.0;

        let mut current = root;
        while layout.align[current] != root {
            let next = layout.align[current];
            let current_node = ni.id_to_node[current];
            let next_node = ni.id_to_node[next];
            let port_pos_diff = get_edge(graph, current_node, next_node)
                .map(|edge| edge_port_position_difference(graph, edge, layout.hdir))
                .unwrap_or(0.0);

            let next_inner_shift = layout.inner_shift[current] + port_pos_diff;
            layout.inner_shift[next] = next_inner_shift;

            let lnode = &graph.layerless_nodes[next_node];
            space_above = space_above.max(lnode.margin.top - next_inner_shift);
            space_below =
                space_below.max(next_inner_shift + lnode.size.height + lnode.margin.bottom);
            current = next;
        }

        let mut current = root;
        loop {
            layout.inner_shift[current] += space_above;
            current = layout.align[current];
            if current == root {
                break;
            }
        }

        layout.block_size[root] = space_above + space_below;
    }
}

fn horizontal_compaction(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &mut BKAlignedLayout,
) {
    for node in ni.iter_nodes() {
        let id = ni.node_id[node];
        layout.sink[id] = id;
        layout.shift[id] = match layout.vdir {
            VDirection::Up => f64::NEG_INFINITY,
            VDirection::Down => f64::INFINITY,
        };
        layout.y[id] = None;
    }

    let mut class_graph = ClassGraph::default();
    let layers = layer_indices(graph, layout.hdir);
    let mut threshold = ThresholdStrategy::for_graph(graph);

    for layer_index in layers {
        let nodes = node_order_for_direction(&graph.layers[layer_index].nodes, layout.vdir);
        for v in nodes {
            let id = ni.node_id[v];
            if layout.root[id] == id {
                place_block(graph, ni, layout, &mut class_graph, &mut threshold, id);
            }
        }
    }

    place_classes(layout, class_graph);

    let layers = layer_indices(graph, layout.hdir);
    for layer_index in layers {
        for v in &graph.layers[layer_index].nodes {
            let id = ni.node_id[*v];
            let root = layout.root[id];
            layout.y[id] = layout.y[root];
            if id == root {
                let sink_shift = layout.shift[layout.sink[id]];
                let apply_shift = match layout.vdir {
                    VDirection::Up => sink_shift > f64::NEG_INFINITY,
                    VDirection::Down => sink_shift < f64::INFINITY,
                };
                if apply_shift {
                    let y = layout.y[id].unwrap_or(0.0) + sink_shift;
                    layout.y[id] = Some(y);
                }
            }
        }
    }

    threshold.post_process(graph, ni, layout);
}

fn place_block(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &mut BKAlignedLayout,
    class_graph: &mut ClassGraph,
    threshold: &mut ThresholdStrategy,
    root: usize,
) {
    if layout.y[root].is_some() {
        return;
    }

    let mut initial_assignment = true;
    layout.y[root] = Some(0.0);
    let mut current = root;
    let mut thresh = match layout.vdir {
        VDirection::Down => f64::NEG_INFINITY,
        VDirection::Up => f64::INFINITY,
    };

    loop {
        let current_node = ni.id_to_node[current];
        let current_index = ni.node_index[current];
        let current_layer_size = graph.layers
            [graph.layerless_nodes[current_node].layer_index.unwrap()]
        .nodes
        .len();

        let needs_neighbor = match layout.vdir {
            VDirection::Down => current_index > 0,
            VDirection::Up => current_index < current_layer_size - 1,
        };

        if needs_neighbor {
            let layer = &graph.layers[graph.layerless_nodes[current_node].layer_index.unwrap()];
            let neighbor_node = match layout.vdir {
                VDirection::Up => layer.nodes[current_index + 1],
                VDirection::Down => layer.nodes[current_index - 1],
            };
            let neighbor_root = layout.root[ni.node_id[neighbor_node]];
            place_block(graph, ni, layout, class_graph, threshold, neighbor_root);

            thresh = threshold.calculate_threshold(graph, ni, layout, thresh, root, current);

            if layout.sink[root] == root {
                layout.sink[root] = layout.sink[neighbor_root];
            }

            if layout.sink[root] == layout.sink[neighbor_root] {
                let spacing = vertical_spacing(graph, current_node, neighbor_node);
                if layout.vdir == VDirection::Up {
                    let current_block_position = layout.y[root].unwrap_or(0.0);
                    let neighbor = &graph.layerless_nodes[neighbor_node];
                    let current_lnode = &graph.layerless_nodes[current_node];
                    let new_position = layout.y[neighbor_root].unwrap_or(0.0)
                        + layout.inner_shift[ni.node_id[neighbor_node]]
                        - neighbor.margin.top
                        - spacing
                        - current_lnode.margin.bottom
                        - current_lnode.size.height
                        - layout.inner_shift[current];
                    let candidate = new_position.min(thresh);
                    layout.y[root] = Some(if initial_assignment {
                        initial_assignment = false;
                        candidate
                    } else {
                        current_block_position.min(candidate)
                    });
                } else {
                    let current_block_position = layout.y[root].unwrap_or(0.0);
                    let neighbor = &graph.layerless_nodes[neighbor_node];
                    let current_lnode = &graph.layerless_nodes[current_node];
                    let new_position = layout.y[neighbor_root].unwrap_or(0.0)
                        + layout.inner_shift[ni.node_id[neighbor_node]]
                        + neighbor.size.height
                        + neighbor.margin.bottom
                        + spacing
                        + current_lnode.margin.top
                        - layout.inner_shift[current];
                    let candidate = new_position.max(thresh);
                    layout.y[root] = Some(if initial_assignment {
                        initial_assignment = false;
                        candidate
                    } else {
                        current_block_position.max(candidate)
                    });
                }
            } else {
                let spacing = graph.options.spacing.node_node;
                let current_lnode = &graph.layerless_nodes[current_node];
                let neighbor = &graph.layerless_nodes[neighbor_node];
                let separation = if layout.vdir == VDirection::Up {
                    layout.y[root].unwrap_or(0.0)
                        + layout.inner_shift[current]
                        + current_lnode.size.height
                        + current_lnode.margin.bottom
                        + spacing
                        - (layout.y[neighbor_root].unwrap_or(0.0)
                            + layout.inner_shift[ni.node_id[neighbor_node]]
                            - neighbor.margin.top)
                } else {
                    layout.y[root].unwrap_or(0.0) + layout.inner_shift[current]
                        - current_lnode.margin.top
                        - layout.y[neighbor_root].unwrap_or(0.0)
                        - layout.inner_shift[ni.node_id[neighbor_node]]
                        - neighbor.size.height
                        - neighbor.margin.bottom
                        - spacing
                };
                class_graph.add_edge(layout.sink[root], layout.sink[neighbor_root], separation);
            }
        } else {
            thresh = threshold.calculate_threshold(graph, ni, layout, thresh, root, current);
        }

        current = layout.align[current];
        if current == root {
            break;
        }
    }

    threshold.finish_block(root);
}

#[derive(Debug)]
enum ThresholdStrategy {
    Null,
    Simple(SimpleThresholdStrategy),
}

impl ThresholdStrategy {
    fn for_graph(graph: &LGraph) -> Self {
        match graph.options.node_placement_bk_edge_straightening {
            EdgeStraighteningStrategy::None => Self::Null,
            EdgeStraighteningStrategy::ImproveStraightness => {
                Self::Simple(SimpleThresholdStrategy::default())
            }
        }
    }

    fn calculate_threshold(
        &mut self,
        graph: &LGraph,
        ni: &NeighborhoodInformation,
        layout: &mut BKAlignedLayout,
        old_thresh: f64,
        block_root: usize,
        current_node: usize,
    ) -> f64 {
        match self {
            Self::Null => match layout.vdir {
                VDirection::Up => f64::INFINITY,
                VDirection::Down => f64::NEG_INFINITY,
            },
            Self::Simple(strategy) => strategy.calculate_threshold(
                graph,
                ni,
                layout,
                old_thresh,
                block_root,
                current_node,
            ),
        }
    }

    fn finish_block(&mut self, root: usize) {
        if let Self::Simple(strategy) = self {
            strategy.block_finished.insert(root);
        }
    }

    fn post_process(
        &mut self,
        graph: &LGraph,
        ni: &NeighborhoodInformation,
        layout: &mut BKAlignedLayout,
    ) {
        if let Self::Simple(strategy) = self {
            strategy.post_process(graph, ni, layout);
        }
    }
}

#[derive(Debug, Default)]
struct SimpleThresholdStrategy {
    block_finished: HashSet<usize>,
    post_processables_queue: VecDeque<Postprocessable>,
    post_processables_stack: Vec<Postprocessable>,
}

impl SimpleThresholdStrategy {
    fn calculate_threshold(
        &mut self,
        graph: &LGraph,
        ni: &NeighborhoodInformation,
        layout: &mut BKAlignedLayout,
        old_thresh: f64,
        block_root: usize,
        current_node: usize,
    ) -> f64 {
        let is_root = block_root == current_node;
        let is_last = layout.align[current_node] == block_root;
        if !(is_root || is_last) {
            return old_thresh;
        }

        let mut threshold = old_thresh;
        if is_root {
            threshold = self.get_bound(graph, ni, layout, block_root, true);
        }
        if threshold.is_infinite() && is_last {
            threshold = self.get_bound(graph, ni, layout, current_node, false);
        }
        threshold
    }

    fn get_bound(
        &mut self,
        graph: &LGraph,
        ni: &NeighborhoodInformation,
        layout: &mut BKAlignedLayout,
        block_node: usize,
        is_root: bool,
    ) -> f64 {
        let invalid = match layout.vdir {
            VDirection::Up => f64::INFINITY,
            VDirection::Down => f64::NEG_INFINITY,
        };
        let pick = self.pick_edge(graph, ni, layout, Postprocessable::new(block_node, is_root));

        match pick.edge {
            None if pick.has_edges => {
                self.post_processables_queue.push_back(pick);
                invalid
            }
            Some(edge) => {
                let left = graph.edges[edge].source;
                let right = graph.edges[edge].target;
                let threshold = if is_root {
                    let root_port = if layout.hdir == HDirection::Right {
                        right
                    } else {
                        left
                    };
                    let other_port = if layout.hdir == HDirection::Right {
                        left
                    } else {
                        right
                    };
                    let other_node = other_port.node;
                    let root_node = root_port.node;
                    let other_root = layout.root[ni.node_id[other_node]];
                    layout.y[other_root].unwrap_or(0.0)
                        + layout.inner_shift[ni.node_id[other_node]]
                        + port_y(graph, other_port)
                        - layout.inner_shift[ni.node_id[root_node]]
                        - port_y(graph, root_port)
                } else {
                    let root_port = if layout.hdir == HDirection::Left {
                        right
                    } else {
                        left
                    };
                    let other_port = if layout.hdir == HDirection::Left {
                        left
                    } else {
                        right
                    };
                    let other_node = other_port.node;
                    let root_node = root_port.node;
                    let other_root = layout.root[ni.node_id[other_node]];
                    layout.y[other_root].unwrap_or(0.0)
                        + layout.inner_shift[ni.node_id[other_node]]
                        + port_y(graph, other_port)
                        - layout.inner_shift[ni.node_id[root_node]]
                        - port_y(graph, root_port)
                };

                layout.straightened[layout.root[ni.node_id[left.node]]] = true;
                layout.straightened[layout.root[ni.node_id[right.node]]] = true;
                threshold
            }
            None => invalid,
        }
    }

    fn pick_edge(
        &self,
        graph: &LGraph,
        ni: &NeighborhoodInformation,
        layout: &BKAlignedLayout,
        mut postprocessable: Postprocessable,
    ) -> Postprocessable {
        let free_node = ni.id_to_node[postprocessable.free];
        let edges = if postprocessable.is_root {
            if layout.hdir == HDirection::Right {
                graph.node_incoming_edges(free_node)
            } else {
                graph.node_outgoing_edges(free_node)
            }
        } else if layout.hdir == HDirection::Left {
            graph.node_incoming_edges(free_node)
        } else {
            graph.node_outgoing_edges(free_node)
        };

        let mut has_edges = false;
        for edge in edges {
            let only_dummies = layout.only_dummies[layout.root[postprocessable.free]];
            if !only_dummies && is_in_layer_edge_index(graph, edge) {
                continue;
            }
            if layout.straightened[layout.root[postprocessable.free]] {
                continue;
            }
            has_edges = true;

            let Some(other) = edge_opposite_node(&graph.edges[edge], free_node) else {
                continue;
            };
            if ni.node_id.get(other).copied().unwrap_or(usize::MAX) == usize::MAX {
                continue;
            }
            let other_root = layout.root[ni.node_id[other]];
            if self.block_finished.contains(&other_root) {
                postprocessable.has_edges = true;
                postprocessable.edge = Some(edge);
                return postprocessable;
            }
        }

        postprocessable.has_edges = has_edges;
        postprocessable.edge = None;
        postprocessable
    }

    fn post_process(
        &mut self,
        graph: &LGraph,
        ni: &NeighborhoodInformation,
        layout: &mut BKAlignedLayout,
    ) {
        while let Some(pp) = self.post_processables_queue.pop_front() {
            let pick = self.pick_edge(graph, ni, layout, pp);
            let Some(edge) = pick.edge else {
                continue;
            };

            let only_dummies = layout.only_dummies[layout.root[pick.free]];
            if !only_dummies && is_in_layer_edge_index(graph, edge) {
                continue;
            }

            if !self.process(graph, ni, layout, pick) {
                self.post_processables_stack.push(Postprocessable {
                    edge: Some(edge),
                    ..pick
                });
            }
        }

        while let Some(pp) = self.post_processables_stack.pop() {
            self.process(graph, ni, layout, pp);
        }
    }

    fn process(
        &self,
        graph: &LGraph,
        ni: &NeighborhoodInformation,
        layout: &mut BKAlignedLayout,
        pp: Postprocessable,
    ) -> bool {
        let Some(edge) = pp.edge else {
            return false;
        };
        let free_node = ni.id_to_node[pp.free];
        let edge_data = &graph.edges[edge];
        let (fix, block) = if edge_data.source.node == free_node {
            (edge_data.target, edge_data.source)
        } else {
            (edge_data.source, edge_data.target)
        };

        let delta = calculate_delta(graph, ni, layout, fix, block);
        if delta > 0.0 && delta < f64::MAX {
            let available_space = check_space_above(graph, ni, layout, block.node, delta);
            let block_root = ni.id_to_node[layout.root[ni.node_id[block.node]]];
            shift_block(ni, layout, block_root, -available_space);
            available_space > 0.0
        } else if delta < 0.0 && -delta < f64::MAX {
            let available_space = check_space_below(graph, ni, layout, block.node, -delta);
            let block_root = ni.id_to_node[layout.root[ni.node_id[block.node]]];
            shift_block(ni, layout, block_root, available_space);
            available_space > 0.0
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Postprocessable {
    free: usize,
    is_root: bool,
    has_edges: bool,
    edge: Option<usize>,
}

impl Postprocessable {
    fn new(free: usize, is_root: bool) -> Self {
        Self {
            free,
            is_root,
            has_edges: false,
            edge: None,
        }
    }
}

#[derive(Debug, Default)]
struct ClassGraph {
    nodes: HashMap<usize, ClassNode>,
}

impl ClassGraph {
    fn add_edge(&mut self, source: usize, target: usize, separation: f64) {
        self.nodes.entry(source).or_insert_with(|| ClassNode {
            node: source,
            ..ClassNode::default()
        });
        self.nodes.entry(target).or_insert_with(|| ClassNode {
            node: target,
            ..ClassNode::default()
        });
        self.nodes.get_mut(&target).unwrap().indegree += 1;
        self.nodes
            .get_mut(&source)
            .unwrap()
            .outgoing
            .push(ClassEdge { target, separation });
    }
}

#[derive(Debug, Default)]
struct ClassNode {
    node: usize,
    class_shift: Option<f64>,
    outgoing: Vec<ClassEdge>,
    indegree: usize,
}

#[derive(Debug)]
struct ClassEdge {
    target: usize,
    separation: f64,
}

fn place_classes(layout: &mut BKAlignedLayout, mut class_graph: ClassGraph) {
    let mut sinks = class_graph
        .nodes
        .values()
        .filter_map(|node| (node.indegree == 0).then_some(node.node))
        .collect::<VecDeque<_>>();

    while let Some(node_id) = sinks.pop_front() {
        if class_graph.nodes[&node_id].class_shift.is_none() {
            class_graph.nodes.get_mut(&node_id).unwrap().class_shift = Some(0.0);
        }
        let class_shift = class_graph.nodes[&node_id].class_shift.unwrap_or(0.0);
        let outgoing = std::mem::take(&mut class_graph.nodes.get_mut(&node_id).unwrap().outgoing);
        for edge in outgoing {
            let candidate = class_shift + edge.separation;
            let target = class_graph.nodes.get_mut(&edge.target).unwrap();
            target.class_shift = match target.class_shift {
                None => Some(candidate),
                Some(current) if layout.vdir == VDirection::Down => Some(current.min(candidate)),
                Some(current) => Some(current.max(candidate)),
            };
            target.indegree = target.indegree.saturating_sub(1);
            if target.indegree == 0 {
                sinks.push_back(edge.target);
            }
        }
    }

    for node in class_graph.nodes.values() {
        layout.shift[node.node] = node.class_shift.unwrap_or(0.0);
    }
}

fn choose_layout(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layouts: Vec<BKAlignedLayout>,
) -> BKAlignedLayout {
    let fallback_layout = layouts.first().cloned();
    let favor_straight_edges = graph
        .options
        .node_placement_favor_straight_edges
        .unwrap_or(graph.options.edge_routing == crate::options::EdgeRouting::Orthogonal);
    let produce_balanced_layout = matches!(
        graph.options.node_placement_bk_fixed_alignment,
        FixedAlignment::Balanced
    ) || (graph.options.node_placement_bk_fixed_alignment
        == FixedAlignment::None
        && !favor_straight_edges);

    if produce_balanced_layout
        && layouts.len() >= 4
        && let Some(balanced) = create_balanced_layout(graph, ni, &layouts)
        && check_order_constraint(graph, ni, &balanced)
    {
        return balanced;
    }

    let mut chosen = None;
    for layout in layouts {
        if !check_order_constraint(graph, ni, &layout) {
            continue;
        }
        if chosen
            .as_ref()
            .map(|current: &BKAlignedLayout| {
                current.layout_size(graph, ni) > layout.layout_size(graph, ni)
            })
            .unwrap_or(true)
        {
            chosen = Some(layout);
        }
    }

    chosen.unwrap_or_else(|| {
        fallback_layout.unwrap_or_else(|| {
            BKAlignedLayout::new(ni.node_count, VDirection::Down, HDirection::Right)
        })
    })
}

fn create_balanced_layout(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layouts: &[BKAlignedLayout],
) -> Option<BKAlignedLayout> {
    if layouts.len() < 4 {
        return None;
    }

    let mut balanced = BKAlignedLayout::new(ni.node_count, VDirection::Down, HDirection::Right);
    let no_of_layouts = layouts.len();
    let mut min = vec![f64::INFINITY; no_of_layouts];
    let mut max = vec![f64::NEG_INFINITY; no_of_layouts];
    let mut width = vec![0.0; no_of_layouts];
    let mut min_width_layout = 0usize;

    for (index, layout) in layouts.iter().enumerate() {
        width[index] = layout.layout_size(graph, ni);
        if width[min_width_layout] > width[index] {
            min_width_layout = index;
        }
        for node in ni.iter_nodes() {
            let id = ni.node_id[node];
            let node_pos_y = layout.y[id]? + layout.inner_shift[id];
            min[index] = min[index].min(node_pos_y);
            max[index] = max[index].max(node_pos_y + graph.layerless_nodes[node].size.height);
        }
    }

    let mut shift = vec![0.0; no_of_layouts];
    for (index, layout) in layouts.iter().enumerate() {
        shift[index] = match layout.vdir {
            VDirection::Down => min[min_width_layout] - min[index],
            VDirection::Up => max[min_width_layout] - max[index],
        };
    }

    for node in ni.iter_nodes() {
        let id = ni.node_id[node];
        let mut calculated_y = Vec::with_capacity(no_of_layouts);
        for (index, layout) in layouts.iter().enumerate() {
            calculated_y.push(layout.y[id]? + layout.inner_shift[id] + shift[index]);
        }
        calculated_y.sort_by(f64::total_cmp);
        balanced.y[id] = Some((calculated_y[1] + calculated_y[2]) / 2.0);
        balanced.inner_shift[id] = 0.0;
        balanced.root[id] = id;
        balanced.block_size[id] = graph.layerless_nodes[node].size.height
            + graph.layerless_nodes[node].margin.top
            + graph.layerless_nodes[node].margin.bottom;
    }

    Some(balanced)
}

fn check_order_constraint(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &BKAlignedLayout,
) -> bool {
    for layer in &graph.layers {
        let mut pos = f64::NEG_INFINITY;
        for node in &layer.nodes {
            let id = ni.node_id[*node];
            let Some(y) = layout.y[id] else {
                return false;
            };
            let top = y + layout.inner_shift[id] - graph.layerless_nodes[*node].margin.top;
            let bottom = y
                + layout.inner_shift[id]
                + graph.layerless_nodes[*node].size.height
                + graph.layerless_nodes[*node].margin.bottom;
            if top > pos && bottom > pos {
                pos = bottom;
            } else {
                return false;
            }
        }
    }
    true
}

fn apply_layout(graph: &mut LGraph, ni: &NeighborhoodInformation, layout: &BKAlignedLayout) {
    for node in ni.iter_nodes() {
        let id = ni.node_id[node];
        if let Some(y) = layout.y[id] {
            graph.layerless_nodes[node].position.y = y + layout.inner_shift[id];
        }
    }
}

fn layer_indices(graph: &LGraph, hdir: HDirection) -> Vec<usize> {
    match hdir {
        HDirection::Left => (0..graph.layers.len()).rev().collect(),
        HDirection::Right => (0..graph.layers.len()).collect(),
    }
}

fn node_order_for_direction(nodes: &[usize], vdir: VDirection) -> Vec<usize> {
    match vdir {
        VDirection::Up => nodes.iter().copied().rev().collect(),
        VDirection::Down => nodes.to_vec(),
    }
}

fn blocks(layout: &BKAlignedLayout) -> HashMap<usize, Vec<usize>> {
    let mut blocks: HashMap<usize, Vec<usize>> = HashMap::new();
    for node in 0..layout.root.len() {
        let root = layout.root[node];
        if root != usize::MAX {
            blocks.entry(root).or_default().push(node);
        }
    }
    blocks
}

fn get_edge(graph: &LGraph, source: usize, target: usize) -> Option<usize> {
    graph
        .node_connected_edges(source)
        .into_iter()
        .find(|edge| edge_opposite_node(&graph.edges[*edge], source) == Some(target))
}

fn edge_opposite_node(edge: &LayeredEdge, node: usize) -> Option<usize> {
    if edge.source.node == node {
        Some(edge.target.node)
    } else if edge.target.node == node {
        Some(edge.source.node)
    } else {
        None
    }
}

fn edge_port_position_difference(graph: &LGraph, edge: usize, hdir: HDirection) -> f64 {
    let source = graph.edges[edge].source;
    let target = graph.edges[edge].target;
    let source_pos = port_y(graph, source);
    let target_pos = port_y(graph, target);
    match hdir {
        HDirection::Left => target_pos - source_pos,
        HDirection::Right => source_pos - target_pos,
    }
}

fn calculate_delta(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &BKAlignedLayout,
    src: PortRef,
    tgt: PortRef,
) -> f64 {
    let src_id = ni.node_id[src.node];
    let tgt_id = ni.node_id[tgt.node];
    let src_root = layout.root[src_id];
    let tgt_root = layout.root[tgt_id];
    let src_pos =
        layout.y[src_root].unwrap_or(0.0) + layout.inner_shift[src_id] + port_y(graph, src);
    let tgt_pos =
        layout.y[tgt_root].unwrap_or(0.0) + layout.inner_shift[tgt_id] + port_y(graph, tgt);
    tgt_pos - src_pos
}

fn shift_block(
    ni: &NeighborhoodInformation,
    layout: &mut BKAlignedLayout,
    root_node: usize,
    delta: f64,
) {
    let root = ni.node_id[root_node];
    let mut current = root;
    loop {
        let new_pos = layout.y[current].unwrap_or(0.0) + delta;
        layout.y[current] = Some(new_pos);
        current = layout.align[current];
        if current == root {
            break;
        }
    }
}

fn check_space_above(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &BKAlignedLayout,
    block_root: usize,
    delta: f64,
) -> f64 {
    let root = ni.node_id[block_root];
    let mut available_space = delta;
    let mut current = root;
    loop {
        current = layout.align[current];
        let current_node = ni.id_to_node[current];
        let min_y_current = min_y(graph, ni, layout, current_node);

        if let Some(neighbor) = upper_neighbor(graph, ni, current_node) {
            let max_y_neighbor = max_y(graph, ni, layout, neighbor);
            available_space = available_space.min(
                min_y_current - (max_y_neighbor + vertical_spacing(graph, current_node, neighbor)),
            );
        }

        if current == root {
            break;
        }
    }
    available_space
}

fn check_space_below(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &BKAlignedLayout,
    block_root: usize,
    delta: f64,
) -> f64 {
    let root = ni.node_id[block_root];
    let mut available_space = delta;
    let mut current = root;
    loop {
        current = layout.align[current];
        let current_node = ni.id_to_node[current];
        let max_y_current = max_y(graph, ni, layout, current_node);

        if let Some(neighbor) = lower_neighbor(graph, ni, current_node) {
            let min_y_neighbor = min_y(graph, ni, layout, neighbor);
            available_space = available_space.min(
                min_y_neighbor - (max_y_current + vertical_spacing(graph, current_node, neighbor)),
            );
        }

        if current == root {
            break;
        }
    }
    available_space
}

fn min_y(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &BKAlignedLayout,
    node: usize,
) -> f64 {
    let node_id = ni.node_id[node];
    let root = layout.root[node_id];
    layout.y[root].unwrap_or(0.0) + layout.inner_shift[node_id]
        - graph.layerless_nodes[node].margin.top
}

fn max_y(
    graph: &LGraph,
    ni: &NeighborhoodInformation,
    layout: &BKAlignedLayout,
    node: usize,
) -> f64 {
    let node_id = ni.node_id[node];
    let root = layout.root[node_id];
    layout.y[root].unwrap_or(0.0)
        + layout.inner_shift[node_id]
        + graph.layerless_nodes[node].size.height
        + graph.layerless_nodes[node].margin.bottom
}

fn lower_neighbor(graph: &LGraph, ni: &NeighborhoodInformation, node: usize) -> Option<usize> {
    let layer = graph.layerless_nodes[node].layer_index?;
    let index = ni.node_index[ni.node_id[node]];
    graph.layers[layer].nodes.get(index + 1).copied()
}

fn upper_neighbor(graph: &LGraph, ni: &NeighborhoodInformation, node: usize) -> Option<usize> {
    let layer = graph.layerless_nodes[node].layer_index?;
    let index = ni.node_index[ni.node_id[node]];
    if index > 0 {
        graph.layers[layer].nodes.get(index - 1).copied()
    } else {
        None
    }
}

fn port_y(graph: &LGraph, port_ref: PortRef) -> f64 {
    let port = &graph.layerless_nodes[port_ref.node].ports[port_ref.port];
    port.position.y + port.anchor.y
}

fn is_self_loop_or_in_layer_edge(graph: &LGraph, edge: usize) -> bool {
    let edge = &graph.edges[edge];
    edge.source.node == edge.target.node || is_in_layer_edge(graph, edge)
}

fn is_in_layer_edge(graph: &LGraph, edge: &LayeredEdge) -> bool {
    graph.layerless_nodes[edge.source.node].layer_index
        == graph.layerless_nodes[edge.target.node].layer_index
}

fn is_in_layer_edge_index(graph: &LGraph, edge: usize) -> bool {
    let edge = &graph.edges[edge];
    is_in_layer_edge(graph, edge)
}

fn vertical_spacing(graph: &LGraph, first: usize, second: usize) -> f64 {
    use LNodeKind::*;

    match (
        graph.layerless_nodes[first].kind,
        graph.layerless_nodes[second].kind,
    ) {
        (Normal, Normal) | (Normal, Label) | (Label, Normal) => graph.options.spacing.node_node,
        (Normal, LongEdge)
        | (LongEdge, Normal)
        | (LongEdge, Label)
        | (Label, LongEdge)
        | (BreakingPoint, Normal)
        | (Normal, BreakingPoint)
        | (BreakingPoint, Label)
        | (Label, BreakingPoint)
        | (BreakingPoint, LongEdge)
        | (LongEdge, BreakingPoint) => graph.options.spacing.edge_node,
        (LongEdge, LongEdge)
        | (LongEdge, NorthSouthPort)
        | (NorthSouthPort, LongEdge)
        | (LongEdge, ExternalPort)
        | (ExternalPort, LongEdge)
        | (Label, Label)
        | (BreakingPoint, BreakingPoint) => graph.options.spacing.edge_edge,
        (Normal, NorthSouthPort)
        | (NorthSouthPort, Normal)
        | (Normal, ExternalPort)
        | (ExternalPort, Normal) => graph.options.spacing.edge_node,
        (NorthSouthPort, NorthSouthPort)
        | (NorthSouthPort, ExternalPort)
        | (ExternalPort, NorthSouthPort) => graph.options.spacing.edge_edge,
        (NorthSouthPort, Label) | (Label, NorthSouthPort) => graph.options.spacing.label_node,
        (ExternalPort, ExternalPort) => graph.options.spacing.port_port,
        (ExternalPort, Label) | (Label, ExternalPort) => graph.options.spacing.label_port_vertical,
        (BreakingPoint, ExternalPort)
        | (ExternalPort, BreakingPoint)
        | (BreakingPoint, NorthSouthPort)
        | (NorthSouthPort, BreakingPoint) => graph.options.spacing.edge_edge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, GreedySwitchType, LayeredOptions};
    use crate::pipeline::{LayeredPhase, execute_processors_until};

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
        ElkInputEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            minlen: 1,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    #[test]
    fn bk_node_placer_assigns_order_preserving_y_coordinates() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::Off,
                node_placement_bk_edge_straightening: EdgeStraighteningStrategy::None,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-C", "A", "C"), edge("B-C", "B", "C")],
        })
        .unwrap();

        execute_processors_until(&mut graph, LayeredPhase::P4NodePlacement).unwrap();

        for layer in &graph.layers {
            let mut bottom = f64::NEG_INFINITY;
            for node in &layer.nodes {
                let lnode = &graph.layerless_nodes[*node];
                assert!(lnode.position.y - lnode.margin.top > bottom);
                bottom = lnode.position.y + lnode.size.height + lnode.margin.bottom;
            }
        }
    }
}
