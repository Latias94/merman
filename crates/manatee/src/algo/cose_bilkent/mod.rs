use crate::algo::CoseBilkentOptions;
use crate::error::Result;
use crate::graph::{Graph, LayoutResult, Point};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn layout(graph: &Graph, _opts: &CoseBilkentOptions) -> Result<LayoutResult> {
    graph.validate()?;

    let mut sim = SimGraph::from_graph(graph);

    // COSE-Bilkent port for flat graphs (as used by Mermaid mindmap via Cytoscape).
    // This follows the upstream `cose-base` control flow:
    // - `getFlatForest()` + `positionNodesRadially(...)`
    // - `reduceTrees()` / `growTree()` scaffolding (currently disabled until parity is verified)
    // - spring embedder ticks
    // - `doPostLayout()` -> `transform(0,0)` to move the graph into positive coordinates
    let forest = sim.get_flat_forest();
    if !forest.is_empty() {
        sim.position_nodes_radially(&forest);
    } else {
        // Fallback: keep all nodes at their provided initial positions (typically (0,0)).
        // The full port will use `scatter()` / `positionNodesRandomly()` for non-forest graphs.
    }
    sim.run_spring_embedder();
    sim.transform_to_origin();

    let mut positions: std::collections::BTreeMap<String, Point> =
        std::collections::BTreeMap::new();
    for n in &sim.nodes {
        positions.insert(
            n.id.clone(),
            Point {
                x: n.center_x(),
                y: n.center_y(),
            },
        );
    }

    Ok(LayoutResult { positions })
}

#[derive(Debug, Clone)]
struct SimNode {
    id: String,
    width: f64,
    height: f64,
    // Top-left anchored rectangle, matching upstream `layout-base` `LNode.rect`.
    left: f64,
    top: f64,
    // Incident edge indices in insertion order, matching `LNode.edges` order.
    edges: Vec<usize>,
    active: bool,

    // FR-grid indices (computed by `update_grid`), used by tree growth heuristics.
    start_x: i32,
    finish_x: i32,
    start_y: i32,
    finish_y: i32,

    // Forces (reset each iteration), matching `FDLayoutNode` / `CoSENode`.
    spring_fx: f64,
    spring_fy: f64,
    repulsion_fx: f64,
    repulsion_fy: f64,
    gravitation_fx: f64,
    gravitation_fy: f64,
}

impl SimNode {
    fn set_center(&mut self, cx: f64, cy: f64) {
        self.left = cx - self.width / 2.0;
        self.top = cy - self.height / 2.0;
    }

    fn center_x(&self) -> f64 {
        self.left + self.width / 2.0
    }

    fn center_y(&self) -> f64 {
        self.top + self.height / 2.0
    }

    fn diagonal(&self) -> f64 {
        (self.width * self.width + self.height * self.height).sqrt()
    }

    fn move_by(&mut self, dx: f64, dy: f64) {
        self.left += dx;
        self.top += dy;
    }

    fn half_w(&self) -> f64 {
        self.width / 2.0
    }

    fn half_h(&self) -> f64 {
        self.height / 2.0
    }

    fn right(&self) -> f64 {
        self.left + self.width
    }

    fn bottom(&self) -> f64 {
        self.top + self.height
    }
}

#[derive(Debug, Clone, Copy)]
struct SimEdge {
    a: usize,
    b: usize,
    active: bool,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn from_nodes(nodes: &[SimNode], tree: &[usize]) -> Option<Self> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &idx in tree {
            let n = &nodes[idx];
            min_x = min_x.min(n.left);
            min_y = min_y.min(n.top);
            max_x = max_x.max(n.left + n.width);
            max_y = max_y.max(n.top + n.height);
        }
        if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
            return None;
        }
        Some(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct PrunedNodeData {
    node_idx: usize,
    edge_idx: usize,
    other_idx: usize,
}

#[derive(Debug)]
struct SimGraph {
    nodes: Vec<SimNode>,
    edges: Vec<SimEdge>,
    pruned_nodes_all: Vec<Vec<PrunedNodeData>>,
    grid: Vec<Vec<Vec<usize>>>,
}

impl SimGraph {
    const DEFAULT_GRAPH_MARGIN: f64 = 15.0;
    const DEFAULT_COMPONENT_SEPERATION: f64 = 60.0; // upstream typo preserved
    const DEFAULT_EDGE_LENGTH: f64 = 50.0;
    const DEFAULT_RADIAL_SEPARATION: f64 = Self::DEFAULT_EDGE_LENGTH;

    // `layout-base` `LayoutConstants.WORLD_CENTER_X/Y`.
    const WORLD_CENTER_X: f64 = 1200.0;
    const WORLD_CENTER_Y: f64 = 900.0;

    // `layout-base` `FDLayoutConstants.DEFAULT_COOLING_FACTOR_INCREMENTAL`.
    const DEFAULT_COOLING_FACTOR_INCREMENTAL: f64 = 0.3;

    const MAX_ITERATIONS: usize = 2500;
    const CONVERGENCE_CHECK_PERIOD: usize = 100;
    const MAX_NODE_DISPLACEMENT: f64 = 300.0;
    const MIN_REPULSION_DIST: f64 = Self::DEFAULT_EDGE_LENGTH / 10.0;

    // cytoscape-cose-bilkent default options (Mermaid uses these in `cose-bilkent/cytoscape-setup.ts`).
    const DEFAULT_SPRING_STRENGTH: f64 = 0.45; // edgeElasticity
    const DEFAULT_REPULSION_STRENGTH: f64 = 4500.0; // nodeRepulsion
    const DEFAULT_GRAVITY_STRENGTH: f64 = 0.25; // gravity
    const DEFAULT_GRAVITY_RANGE_FACTOR: f64 = 3.8; // gravityRange

    fn from_graph(graph: &Graph) -> Self {
        let mut nodes: Vec<SimNode> = Vec::with_capacity(graph.nodes.len());
        for n in &graph.nodes {
            nodes.push(SimNode {
                id: n.id.clone(),
                width: n.width.max(1.0),
                height: n.height.max(1.0),
                left: n.x - n.width.max(1.0) / 2.0,
                top: n.y - n.height.max(1.0) / 2.0,
                edges: Vec::new(),
                active: true,
                start_x: 0,
                finish_x: 0,
                start_y: 0,
                finish_y: 0,
                spring_fx: 0.0,
                spring_fy: 0.0,
                repulsion_fx: 0.0,
                repulsion_fy: 0.0,
                gravitation_fx: 0.0,
                gravitation_fy: 0.0,
            });
        }

        let mut id_to_idx: HashMap<&str, usize> = HashMap::with_capacity(graph.nodes.len());
        for (idx, n) in graph.nodes.iter().enumerate() {
            id_to_idx.insert(n.id.as_str(), idx);
        }

        // Mirror the cytoscape-cose-bilkent behavior: only keep one edge between any two nodes.
        let mut seen_pairs: HashSet<(usize, usize)> = HashSet::with_capacity(graph.edges.len());
        let mut edges: Vec<SimEdge> = Vec::with_capacity(graph.edges.len());
        for e in &graph.edges {
            let a = *id_to_idx.get(e.source.as_str()).expect("validated");
            let b = *id_to_idx.get(e.target.as_str()).expect("validated");
            if a == b {
                continue;
            }
            let (u, v) = if a < b { (a, b) } else { (b, a) };
            if seen_pairs.contains(&(u, v)) {
                continue;
            }
            seen_pairs.insert((u, v));
            let ei = edges.len();
            edges.push(SimEdge { a, b, active: true });
            nodes[a].edges.push(ei);
            nodes[b].edges.push(ei);
        }

        Self {
            nodes,
            edges,
            pruned_nodes_all: Vec::new(),
            grid: Vec::new(),
        }
    }

    fn edge_other_end(&self, edge_idx: usize, node_idx: usize) -> usize {
        let e = self.edges[edge_idx];
        if e.a == node_idx {
            e.b
        } else {
            debug_assert_eq!(e.b, node_idx);
            e.a
        }
    }

    fn edges_between(&self, a: usize, b: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for &ei in &self.nodes[a].edges {
            if !self.edges[ei].active {
                continue;
            }
            if self.edge_other_end(ei, a) == b {
                out.push(ei);
            }
        }
        out
    }

    fn neighbors_of(&self, node_idx: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for &ei in &self.nodes[node_idx].edges {
            if !self.edges[ei].active {
                continue;
            }
            let other = self.edge_other_end(ei, node_idx);
            if self.nodes[other].active {
                out.push(other);
            }
        }
        out
    }

    /// Port of `layout-base` `Layout.getFlatForest()` for flat graphs.
    fn get_flat_forest(&self) -> Vec<Vec<usize>> {
        let mut flat_forest: Vec<Vec<usize>> = Vec::new();
        let mut is_forest = true;

        // Root graph nodes in insertion order.
        let all_nodes: Vec<usize> = (0..self.nodes.len())
            .filter(|&idx| self.nodes[idx].active)
            .collect();

        // Graph is always flat in our current model (no compound nodes).

        // BFS for each component; reject if any component is not a tree.
        let mut to_be_visited: VecDeque<usize> = VecDeque::new();
        let mut parents: Vec<Option<usize>> = vec![None; self.nodes.len()];
        let mut parents_touched: Vec<usize> = Vec::new();
        let mut visited: Vec<bool> = vec![false; self.nodes.len()];
        let mut unprocessed_nodes: Vec<usize> = all_nodes;

        while !unprocessed_nodes.is_empty() && is_forest {
            to_be_visited.push_back(unprocessed_nodes[0]);

            let mut visited_order: Vec<usize> = Vec::new();

            while let Some(current_node) = to_be_visited.pop_front() {
                if !visited[current_node] {
                    visited[current_node] = true;
                    visited_order.push(current_node);
                }

                // Traverse all neighbors of this node, in edge insertion order.
                for &ei in &self.nodes[current_node].edges {
                    if !self.edges[ei].active {
                        continue;
                    }
                    let current_neighbor = self.edge_other_end(ei, current_node);
                    if !self.nodes[current_neighbor].active {
                        continue;
                    }

                    // If BFS is not growing from this neighbor.
                    if parents[current_node] != Some(current_neighbor) {
                        if !visited[current_neighbor] {
                            to_be_visited.push_back(current_neighbor);
                            if parents[current_neighbor].is_none() {
                                parents_touched.push(current_neighbor);
                            }
                            parents[current_neighbor] = Some(current_node);
                        } else {
                            is_forest = false;
                            break;
                        }
                    }
                }

                if !is_forest {
                    break;
                }
            }

            if !is_forest {
                flat_forest.clear();
            } else {
                // JS Set preserves insertion order; `visited_order` mimics `[...visited]`.
                flat_forest.push(visited_order.clone());

                // Remove all visited nodes from unProcessedNodes.
                unprocessed_nodes.retain(|&n| !visited[n]);

                // Clear per-component state (only touched indices).
                for &idx in &visited_order {
                    visited[idx] = false;
                }
                for idx in parents_touched.drain(..) {
                    parents[idx] = None;
                }

                to_be_visited.clear();
            }
        }

        flat_forest
    }

    #[allow(dead_code)]
    fn active_degree(&self, node_idx: usize) -> usize {
        if !self.nodes[node_idx].active {
            return 0;
        }
        let mut d = 0usize;
        for &ei in &self.nodes[node_idx].edges {
            if !self.edges[ei].active {
                continue;
            }
            let other = self.edge_other_end(ei, node_idx);
            if self.nodes[other].active {
                d += 1;
            }
        }
        d
    }

    #[allow(dead_code)]
    fn active_leaf_edge(&self, node_idx: usize) -> Option<usize> {
        if self.active_degree(node_idx) != 1 {
            return None;
        }
        for &ei in &self.nodes[node_idx].edges {
            if !self.edges[ei].active {
                continue;
            }
            let other = self.edge_other_end(ei, node_idx);
            if self.nodes[other].active {
                return Some(ei);
            }
        }
        None
    }

    fn update_grid(&mut self, repulsion_range: f64) {
        self.grid.clear();
        if self.nodes.iter().all(|n| !n.active) {
            return;
        }

        let mut left = f64::INFINITY;
        let mut top = f64::INFINITY;
        let mut right = f64::NEG_INFINITY;
        let mut bottom = f64::NEG_INFINITY;
        for n in &self.nodes {
            if !n.active {
                continue;
            }
            left = left.min(n.left);
            top = top.min(n.top);
            right = right.max(n.right());
            bottom = bottom.max(n.bottom());
        }
        if !(left.is_finite() && top.is_finite() && right.is_finite() && bottom.is_finite()) {
            return;
        }

        let size_x = ((right - left) / repulsion_range).ceil().max(1.0) as usize;
        let size_y = ((bottom - top) / repulsion_range).ceil().max(1.0) as usize;
        self.grid = vec![vec![Vec::new(); size_y]; size_x];

        let clamp_x = |v: i32| v.clamp(0, (size_x as i32) - 1);
        let clamp_y = |v: i32| v.clamp(0, (size_y as i32) - 1);

        for (idx, n) in self.nodes.iter_mut().enumerate() {
            if !n.active {
                continue;
            }
            let start_x = ((n.left - left) / repulsion_range).floor() as i32;
            let finish_x = ((n.right() - left) / repulsion_range).floor() as i32;
            let start_y = ((n.top - top) / repulsion_range).floor() as i32;
            let finish_y = ((n.bottom() - top) / repulsion_range).floor() as i32;

            n.start_x = clamp_x(start_x);
            n.finish_x = clamp_x(finish_x);
            n.start_y = clamp_y(start_y);
            n.finish_y = clamp_y(finish_y);

            for gx in (n.start_x as usize)..=(n.finish_x as usize) {
                for gy in (n.start_y as usize)..=(n.finish_y as usize) {
                    self.grid[gx][gy].push(idx);
                }
            }
        }
    }

    #[allow(dead_code)]
    fn reduce_trees(&mut self) {
        self.pruned_nodes_all.clear();

        let mut contains_leaf = true;
        while contains_leaf {
            contains_leaf = false;
            let mut candidates: Vec<PrunedNodeData> = Vec::new();

            for idx in 0..self.nodes.len() {
                if !self.nodes[idx].active {
                    continue;
                }
                let Some(edge_idx) = self.active_leaf_edge(idx) else {
                    continue;
                };
                let other_idx = self.edge_other_end(edge_idx, idx);
                candidates.push(PrunedNodeData {
                    node_idx: idx,
                    edge_idx,
                    other_idx,
                });
                contains_leaf = true;
            }

            if !contains_leaf {
                break;
            }

            // Mirror upstream's "re-check degree before removal" behavior by pruning sequentially.
            candidates.sort_by_key(|d| d.node_idx);
            let mut pruned_in_step: Vec<PrunedNodeData> = Vec::new();
            for cand in candidates {
                if !self.nodes[cand.node_idx].active {
                    continue;
                }
                if self.active_leaf_edge(cand.node_idx) != Some(cand.edge_idx) {
                    continue;
                }
                self.nodes[cand.node_idx].active = false;
                self.edges[cand.edge_idx].active = false;
                pruned_in_step.push(cand);
            }

            if pruned_in_step.is_empty() {
                break;
            }
            self.pruned_nodes_all.push(pruned_in_step);
        }
    }

    fn place_pruned_node(
        &mut self,
        pruned_node: usize,
        node_to_connect: usize,
        repulsion_range: f64,
    ) {
        self.update_grid(repulsion_range);
        if self.grid.is_empty() || self.grid[0].is_empty() {
            return;
        }

        let start_grid_x = self.nodes[node_to_connect].start_x;
        let finish_grid_x = self.nodes[node_to_connect].finish_x;
        let start_grid_y = self.nodes[node_to_connect].start_y;
        let finish_grid_y = self.nodes[node_to_connect].finish_y;

        let mut control_regions = [0i32, 0i32, 0i32, 0i32]; // up, right, down, left

        if start_grid_y > 0 {
            let y0 = (start_grid_y - 1) as usize;
            let y1 = start_grid_y as usize;
            for x in (start_grid_x as usize)..=(finish_grid_x as usize) {
                control_regions[0] +=
                    (self.grid[x][y0].len() + self.grid[x][y1].len()).saturating_sub(1) as i32;
            }
        }
        if (finish_grid_x as usize) + 1 < self.grid.len() {
            let x0 = (finish_grid_x + 1) as usize;
            let x1 = finish_grid_x as usize;
            for y in (start_grid_y as usize)..=(finish_grid_y as usize) {
                control_regions[1] +=
                    (self.grid[x0][y].len() + self.grid[x1][y].len()).saturating_sub(1) as i32;
            }
        }
        if (finish_grid_y as usize) + 1 < self.grid[0].len() {
            let y0 = (finish_grid_y + 1) as usize;
            let y1 = finish_grid_y as usize;
            for x in (start_grid_x as usize)..=(finish_grid_x as usize) {
                control_regions[2] +=
                    (self.grid[x][y0].len() + self.grid[x][y1].len()).saturating_sub(1) as i32;
            }
        }
        if start_grid_x > 0 {
            let x0 = (start_grid_x - 1) as usize;
            let x1 = start_grid_x as usize;
            for y in (start_grid_y as usize)..=(finish_grid_y as usize) {
                control_regions[3] +=
                    (self.grid[x0][y].len() + self.grid[x1][y].len()).saturating_sub(1) as i32;
            }
        }

        let mut min = i32::MAX;
        let mut min_count = 0i32;
        let mut min_index = 0usize;
        for (idx, v) in control_regions.iter().enumerate() {
            if *v < min {
                min = *v;
                min_count = 1;
                min_index = idx;
            } else if *v == min {
                min_count += 1;
            }
        }

        let choose_preferred = |cands: &[usize]| -> usize {
            // Prefer `right`, then `left`, then `up`, then `down`.
            for pref in [1usize, 3, 0, 2] {
                if cands.contains(&pref) {
                    return pref;
                }
            }
            cands[0]
        };

        let grid_for_pruned = if min_count == 3 && min == 0 {
            if control_regions[0] == 0 && control_regions[1] == 0 && control_regions[2] == 0 {
                1
            } else if control_regions[0] == 0 && control_regions[1] == 0 && control_regions[3] == 0
            {
                0
            } else if control_regions[0] == 0 && control_regions[2] == 0 && control_regions[3] == 0
            {
                3
            } else if control_regions[1] == 0 && control_regions[2] == 0 && control_regions[3] == 0
            {
                2
            } else {
                min_index
            }
        } else if min_count == 2 && min == 0 {
            let mut cands: Vec<usize> = Vec::new();
            for (idx, v) in control_regions.iter().enumerate() {
                if *v == 0 {
                    cands.push(idx);
                }
            }
            choose_preferred(&cands)
        } else if min_count == 4 && min == 0 {
            choose_preferred(&[0, 1, 2, 3])
        } else {
            min_index
        };

        let cx = self.nodes[node_to_connect].center_x();
        let cy = self.nodes[node_to_connect].center_y();
        let cw = self.nodes[node_to_connect].half_w();
        let ch = self.nodes[node_to_connect].half_h();
        let pw = self.nodes[pruned_node].half_w();
        let ph = self.nodes[pruned_node].half_h();
        let l = Self::DEFAULT_EDGE_LENGTH;

        match grid_for_pruned {
            0 => self.nodes[pruned_node].set_center(cx, cy - ch - l - ph),
            1 => self.nodes[pruned_node].set_center(cx + cw + l + pw, cy),
            2 => self.nodes[pruned_node].set_center(cx, cy + ch + l + ph),
            _ => self.nodes[pruned_node].set_center(cx - cw - l - pw, cy),
        }
    }

    fn grow_tree_one_step(&mut self, repulsion_range: f64) {
        let Some(step) = self.pruned_nodes_all.pop() else {
            return;
        };

        for node_data in step {
            let node_idx = node_data.node_idx;
            let edge_idx = node_data.edge_idx;
            let node_to_connect = if self.nodes[node_data.other_idx].active {
                node_data.other_idx
            } else {
                let e = self.edges[edge_idx];
                if self.nodes[e.a].active { e.a } else { e.b }
            };

            self.place_pruned_node(node_idx, node_to_connect, repulsion_range);
            self.nodes[node_idx].active = true;
            self.edges[edge_idx].active = true;
        }

        self.update_grid(repulsion_range);
    }

    /// Port of `layout-base` `Layout.findCenterOfTree(nodes)`.
    /// Note: this intentionally preserves the upstream loop's in-place removal behavior.
    fn find_center_of_tree(&self, nodes: &[usize]) -> usize {
        let mut list: Vec<usize> = nodes.to_vec();
        let mut removed_nodes: Vec<usize> = Vec::new();
        let mut removed: Vec<bool> = vec![false; self.nodes.len()];
        let mut remaining_degrees: Vec<usize> = vec![0; self.nodes.len()];
        let mut found_center = false;
        let mut center_node = list[0];

        if list.len() == 1 || list.len() == 2 {
            found_center = true;
            center_node = list[0];
        }

        for &node in &list {
            let degree = self.neighbors_of(node).len();
            remaining_degrees[node] = degree;
            if degree == 1 {
                removed_nodes.push(node);
                removed[node] = true;
            }
        }

        let mut temp_list: Vec<usize> = removed_nodes.clone();

        while !found_center {
            let _temp_list2 = temp_list.clone(); // preserved for parity with upstream logic
            temp_list.clear();

            // The upstream implementation mutates `list` while iterating over it. Replicate that.
            let mut i = 0usize;
            while i < list.len() {
                let node = list[i];
                list.remove(i);

                for neighbour in self.neighbors_of(node) {
                    if !removed[neighbour] {
                        let other_degree = remaining_degrees[neighbour];
                        let new_degree = other_degree.saturating_sub(1);
                        if new_degree == 1 {
                            temp_list.push(neighbour);
                        }
                        remaining_degrees[neighbour] = new_degree;
                    }
                }

                i += 1;
            }

            for &v in &temp_list {
                removed[v] = true;
            }
            removed_nodes.extend(temp_list.iter().copied());

            if list.len() == 1 || list.len() == 2 {
                found_center = true;
                center_node = list[0];
            }
        }

        center_node
    }

    fn max_diagonal_in_tree(&self, tree: &[usize]) -> f64 {
        let mut max_diag = f64::NEG_INFINITY;
        for &idx in tree {
            max_diag = max_diag.max(self.nodes[idx].diagonal());
        }
        if !max_diag.is_finite() { 0.0 } else { max_diag }
    }

    fn branch_radial_layout(
        &mut self,
        node: usize,
        parent: Option<usize>,
        start_angle: f64,
        end_angle: f64,
        distance: f64,
        radial_separation: f64,
    ) {
        // First, position this node by finding its angle.
        let mut half_interval = ((end_angle - start_angle) + 1.0) / 2.0;
        if half_interval < 0.0 {
            half_interval += 180.0;
        }
        let node_angle = (half_interval + start_angle).rem_euclid(360.0);
        let teta = (node_angle * std::f64::consts::TAU) / 360.0;
        let x_ = distance * teta.cos();
        let y_ = distance * teta.sin();
        self.nodes[node].set_center(x_, y_);

        // Traverse all neighbors of this node and recursively call this function.
        let mut neighbor_edges: Vec<usize> = self.nodes[node].edges.clone();
        let mut child_count = neighbor_edges.len();
        if parent.is_some() && child_count > 0 {
            child_count -= 1;
        }
        let mut branch_count = 0usize;
        let inc_edges_count = neighbor_edges.len();

        let mut edges_to_parent = parent
            .map(|p| self.edges_between(node, p))
            .unwrap_or_default();
        while edges_to_parent.len() > 1 {
            let temp = edges_to_parent.remove(0);
            if let Some(pos) = neighbor_edges.iter().position(|&e| e == temp) {
                neighbor_edges.remove(pos);
            }
            child_count = child_count.saturating_sub(1);
        }

        let start_index: usize =
            if parent.is_some() && !edges_to_parent.is_empty() && inc_edges_count > 0 {
                (neighbor_edges
                    .iter()
                    .position(|&e| e == edges_to_parent[0])
                    .unwrap_or(0)
                    + 1)
                    % inc_edges_count
            } else {
                0
            };

        let step_angle = if child_count == 0 {
            0.0
        } else {
            (end_angle - start_angle).abs() / (child_count as f64)
        };

        if child_count == 0 || inc_edges_count == 0 {
            return;
        }

        let mut i = start_index;
        while branch_count != child_count {
            let current_neighbor = self.edge_other_end(neighbor_edges[i], node);
            if Some(current_neighbor) == parent {
                i = (i + 1) % inc_edges_count;
                continue;
            }

            let child_start_angle =
                (start_angle + (branch_count as f64) * step_angle).rem_euclid(360.0);
            let child_end_angle = (child_start_angle + step_angle).rem_euclid(360.0);
            self.branch_radial_layout(
                current_neighbor,
                Some(node),
                child_start_angle,
                child_end_angle,
                distance + radial_separation,
                radial_separation,
            );

            branch_count += 1;
            i = (i + 1) % inc_edges_count;
        }
    }

    fn radial_layout(
        &mut self,
        tree: &[usize],
        center_node: usize,
        starting_x: f64,
        starting_y: f64,
    ) -> (f64, f64) {
        let radial_sep = self
            .max_diagonal_in_tree(tree)
            .max(Self::DEFAULT_RADIAL_SEPARATION);

        self.branch_radial_layout(center_node, None, 0.0, 359.0, 0.0, radial_sep);

        let Some(bounds) = Bounds::from_nodes(&self.nodes, tree) else {
            return (starting_x, starting_y);
        };

        // `Transform` with extents 1.0 is a pure translation: worldOrg + (x - deviceOrg).
        let dx = starting_x - bounds.min_x;
        let dy = starting_y - bounds.min_y;
        for &idx in tree {
            self.nodes[idx].left += dx;
            self.nodes[idx].top += dy;
        }

        (bounds.max_x + dx, bounds.max_y + dy)
    }

    fn position_nodes_radially(&mut self, forest: &[Vec<usize>]) {
        // Tile the trees to a grid row by row; first tree starts at (0,0).
        let number_of_columns = (forest.len() as f64).sqrt().ceil().max(1.0) as usize;
        let mut height = 0.0;
        let mut current_y = 0.0;
        let mut current_x = 0.0;
        let mut point = (0.0, 0.0);

        for (i, tree) in forest.iter().enumerate() {
            if i % number_of_columns == 0 {
                current_x = 0.0;
                current_y = height;
                if i != 0 {
                    current_y += Self::DEFAULT_COMPONENT_SEPERATION;
                }
                height = 0.0;
            }

            let center_node = self.find_center_of_tree(tree);
            point = self.radial_layout(tree, center_node, current_x, current_y);

            if point.1 > height {
                height = point.1.floor();
            }

            current_x = (point.0 + Self::DEFAULT_COMPONENT_SEPERATION).floor();
        }

        // Match upstream `positionNodesRadially` final world-centering pass (layout-base).
        // This can affect floating-point drift and convergence in the subsequent spring embedder.
        let dx = Self::WORLD_CENTER_X - point.0 / 2.0;
        let dy = Self::WORLD_CENTER_Y - point.1 / 2.0;
        for n in &mut self.nodes {
            n.move_by(dx, dy);
        }
    }

    fn run_spring_embedder(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        // Mermaid's Cytoscape COSE-Bilkent applies gravitational forces only when the graph is
        // disconnected (`calculateNodesToApplyGravitationTo()` collects nodes from non-connected
        // graphs). For a connected mindmap tree this list is empty, so gravity is a no-op.
        let nodes_with_gravity = self.nodes_to_apply_gravitation();

        // These are instance fields in upstream `FDLayout`/`CoSELayout`.
        let ideal_edge_length = Self::DEFAULT_EDGE_LENGTH.max(10.0);
        let spring_constant = Self::DEFAULT_SPRING_STRENGTH;
        let repulsion_constant = Self::DEFAULT_REPULSION_STRENGTH;
        let gravity_constant = Self::DEFAULT_GRAVITY_STRENGTH;
        let gravity_range_factor = Self::DEFAULT_GRAVITY_RANGE_FACTOR;
        let repulsion_range = 2.0 * ideal_edge_length;
        self.update_grid(repulsion_range);

        let active_n = self.nodes.iter().filter(|n| n.active).count().max(1) as f64;
        let displacement_threshold_per_node = (3.0 * Self::DEFAULT_EDGE_LENGTH) / 100.0;
        let total_displacement_threshold = displacement_threshold_per_node * active_n;

        // Non-incremental mode: coolingFactor starts at 1.0 for small graphs.
        let initial_cooling_factor = 1.0;
        let mut cooling_factor = initial_cooling_factor;
        let max_iterations = Self::MAX_ITERATIONS.max(active_n as usize * 5);
        let max_cooling_cycle = (max_iterations as f64) / (Self::CONVERGENCE_CHECK_PERIOD as f64);
        let final_temperature = (Self::CONVERGENCE_CHECK_PERIOD as f64) / (max_iterations as f64);
        let mut cooling_cycle = 0.0f64;
        // Mermaid (via `rendering-util/layout-algorithms/cose-bilkent/cytoscape-setup.ts`) uses
        // `quality: 'proof'` for COSE-Bilkent.
        let layout_quality = 2i32;

        let mut total_iterations = 0usize;
        let mut old_total_displacement = 0.0f64;
        let mut last_total_displacement = 0.0f64;

        let mut is_tree_growing = false;
        let mut is_growth_finished = false;
        let mut grow_tree_iterations = 0usize;
        let mut after_growth_iterations = 0usize;

        loop {
            total_iterations += 1;

            if total_iterations == max_iterations && !is_tree_growing && !is_growth_finished {
                if !self.pruned_nodes_all.is_empty() {
                    is_tree_growing = true;
                } else {
                    break;
                }
            }

            if total_iterations.is_multiple_of(Self::CONVERGENCE_CHECK_PERIOD)
                && !is_tree_growing
                && !is_growth_finished
            {
                let oscilating = total_iterations > (max_iterations / 3)
                    && (last_total_displacement - old_total_displacement).abs() < 2.0;
                let converged = last_total_displacement < total_displacement_threshold;

                old_total_displacement = last_total_displacement;

                if converged || oscilating {
                    if !self.pruned_nodes_all.is_empty() {
                        is_tree_growing = true;
                    } else {
                        break;
                    }
                }

                cooling_cycle += 1.0;

                // cooling schedule 3 (see upstream comment in `CoSELayout.tick`)
                let numerator = (100.0 * (initial_cooling_factor - final_temperature)).ln();
                let denominator = max_cooling_cycle.ln().max(1e-9);
                let power = numerator / denominator;
                let cooling_adjuster = match layout_quality {
                    0 => cooling_cycle,
                    1 => cooling_cycle / 3.0,
                    _ => 1.0,
                };
                let schedule = cooling_cycle.powf(power) / 100.0 * cooling_adjuster;
                cooling_factor = (initial_cooling_factor - schedule).max(final_temperature);
            }

            if is_tree_growing {
                if grow_tree_iterations.is_multiple_of(10) {
                    if !self.pruned_nodes_all.is_empty() {
                        self.update_grid(repulsion_range);
                        self.grow_tree_one_step(repulsion_range);
                        self.update_grid(repulsion_range);
                        cooling_factor = Self::DEFAULT_COOLING_FACTOR_INCREMENTAL;
                    } else {
                        is_tree_growing = false;
                        is_growth_finished = true;
                    }
                }
                grow_tree_iterations += 1;
            }

            if is_growth_finished {
                let oscilating = total_iterations > (max_iterations / 3)
                    && (last_total_displacement - old_total_displacement).abs() < 2.0;
                let converged = last_total_displacement < total_displacement_threshold;
                if converged || oscilating {
                    break;
                }

                if after_growth_iterations.is_multiple_of(10) {
                    self.update_grid(repulsion_range);
                }
                cooling_factor = Self::DEFAULT_COOLING_FACTOR_INCREMENTAL
                    * ((100.0 - (after_growth_iterations as f64)) / 100.0).max(0.0);
                after_growth_iterations += 1;
            }

            let mut total_displacement = 0.0f64;

            // Spring forces
            for e in &self.edges {
                if !e.active {
                    continue;
                }
                let (a, b) = (e.a, e.b);
                if !(self.nodes[a].active && self.nodes[b].active) {
                    continue;
                }

                // Upstream `FDLayout.calcSpringForce` uses clipping points on the node rectangles
                // (via `IGeometry.getIntersection`) so the "ideal edge length" applies between
                // node borders rather than between node centers.
                let (target_x, target_y, source_x, source_y, overlapped) =
                    rect_intersection_points(&self.nodes[b], &self.nodes[a]);
                if overlapped {
                    continue;
                }
                let mut lx = target_x - source_x;
                let mut ly = target_y - source_y;

                // Mirror `LEdge.updateLength(...)` from `layout-base`: very small components are
                // snapped to their sign (or 0 if the component is 0).
                if lx.abs() < 1.0 {
                    lx = lx.signum();
                }
                if ly.abs() < 1.0 {
                    ly = ly.signum();
                }

                let len = (lx * lx + ly * ly).sqrt();
                if len == 0.0 {
                    continue;
                }
                let spring_force = spring_constant * (len - ideal_edge_length);
                let sfx = spring_force * (lx / len);
                let sfy = spring_force * (ly / len);
                self.nodes[a].spring_fx += sfx;
                self.nodes[a].spring_fy += sfy;
                self.nodes[b].spring_fx -= sfx;
                self.nodes[b].spring_fy -= sfy;
            }

            // Repulsion forces (O(n^2); sufficient for current fixture sizes).
            for i in 0..self.nodes.len() {
                if !self.nodes[i].active {
                    continue;
                }
                for j in (i + 1)..self.nodes.len() {
                    if !self.nodes[j].active {
                        continue;
                    }
                    // Mirror the FR-grid variant's effective cutoff:
                    // only compute repulsion for pairs that are within `repulsionRange` along both axes.
                    //
                    // `distanceX = abs(cxA-cxB) - (wA/2 + wB/2)` (same for Y).
                    // (See `FDLayout.calculateRepulsionForceOfANode` in `layout-base`.)
                    let a = &self.nodes[i];
                    let b = &self.nodes[j];
                    let dist_x = (a.center_x() - b.center_x()).abs() - (a.half_w() + b.half_w());
                    let dist_y = (a.center_y() - b.center_y()).abs() - (a.half_h() + b.half_h());
                    if dist_x > repulsion_range || dist_y > repulsion_range {
                        continue;
                    }

                    let (rfx, rfy) = self.calc_repulsion_force(i, j, repulsion_constant);
                    self.nodes[i].repulsion_fx += rfx;
                    self.nodes[i].repulsion_fy += rfy;
                    self.nodes[j].repulsion_fx -= rfx;
                    self.nodes[j].repulsion_fy -= rfy;
                }
            }

            // Gravitation (only for disconnected graphs).
            if !nodes_with_gravity.is_empty() {
                if let Some((owner_center_x, owner_center_y, estimated_size)) =
                    self.gravitation_context(gravity_range_factor)
                {
                    for &idx in &nodes_with_gravity {
                        let n = &mut self.nodes[idx];
                        if !n.active {
                            continue;
                        }
                        let distance_x = n.center_x() - owner_center_x;
                        let distance_y = n.center_y() - owner_center_y;
                        let abs_distance_x = distance_x.abs() + n.width / 2.0;
                        let abs_distance_y = distance_y.abs() + n.height / 2.0;
                        if abs_distance_x > estimated_size || abs_distance_y > estimated_size {
                            n.gravitation_fx = -gravity_constant * distance_x;
                            n.gravitation_fy = -gravity_constant * distance_y;
                        }
                    }
                }
            }

            // Move nodes
            for n in &mut self.nodes {
                if !n.active {
                    continue;
                }
                let dx = cooling_factor * (n.spring_fx + n.repulsion_fx + n.gravitation_fx);
                let dy = cooling_factor * (n.spring_fy + n.repulsion_fy + n.gravitation_fy);

                let mut mdx = dx;
                let mut mdy = dy;
                let max_d = cooling_factor * Self::MAX_NODE_DISPLACEMENT;
                if mdx.abs() > max_d {
                    mdx = max_d * mdx.signum();
                }
                if mdy.abs() > max_d {
                    mdy = max_d * mdy.signum();
                }

                n.move_by(mdx, mdy);
                total_displacement += mdx.abs() + mdy.abs();

                // Reset forces
                n.spring_fx = 0.0;
                n.spring_fy = 0.0;
                n.repulsion_fx = 0.0;
                n.repulsion_fy = 0.0;
                n.gravitation_fx = 0.0;
                n.gravitation_fy = 0.0;
            }

            last_total_displacement = total_displacement;
        }
    }

    fn calc_repulsion_force(&self, a: usize, b: usize, repulsion_constant: f64) -> (f64, f64) {
        let na = &self.nodes[a];
        let nb = &self.nodes[b];

        if rects_intersect(na, nb) {
            let (ox, oy) = calc_separation_amount(na, nb, Self::DEFAULT_EDGE_LENGTH / 2.0);
            let repulsion_fx = 2.0 * ox;
            let repulsion_fy = 2.0 * oy;
            // `childrenConstant = 1*1/(1+1) = 0.5` for flat leaf nodes.
            (-0.5 * repulsion_fx, -0.5 * repulsion_fy)
        } else {
            // Use clipping points (approx) to account for node dimensions.
            let (ax, ay, bx, by, _overlapped) = rect_intersection_points(na, nb);
            let mut dx = bx - ax;
            let mut dy = by - ay;

            if dx.abs() < Self::MIN_REPULSION_DIST {
                dx = dx.signum() * Self::MIN_REPULSION_DIST;
            }
            if dy.abs() < Self::MIN_REPULSION_DIST {
                dy = dy.signum() * Self::MIN_REPULSION_DIST;
            }

            let dist_sq = dx * dx + dy * dy;
            let dist = dist_sq.sqrt();
            if dist_sq == 0.0 || dist == 0.0 {
                return (0.0, 0.0);
            }
            let repulsion_force = repulsion_constant / dist_sq;
            let rfx = repulsion_force * dx / dist;
            let rfy = repulsion_force * dy / dist;
            (-rfx, -rfy)
        }
    }

    /// Port of `Layout.transform(newLeftTop)` for the root graph and `newLeftTop = (0,0)`.
    /// This moves the layout into a positive coordinate space with a fixed margin (15px).
    fn transform_to_origin(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        let mut min_left = f64::INFINITY;
        let mut min_top = f64::INFINITY;
        for n in &self.nodes {
            if !n.active {
                continue;
            }
            min_left = min_left.min(n.left);
            min_top = min_top.min(n.top);
        }
        if !(min_left.is_finite() && min_top.is_finite()) {
            return;
        }

        let left_top_x = min_left - Self::DEFAULT_GRAPH_MARGIN;
        let left_top_y = min_top - Self::DEFAULT_GRAPH_MARGIN;

        // Translate so `left_top` becomes (0,0).
        let dx = -left_top_x;
        let dy = -left_top_y;
        for n in &mut self.nodes {
            if !n.active {
                continue;
            }
            n.left += dx;
            n.top += dy;
        }
    }

    fn nodes_to_apply_gravitation(&self) -> Vec<usize> {
        // Port of COSE `calculateNodesToApplyGravitationTo()` for a flat graph: apply gravity to
        // all nodes only if the graph is disconnected.
        let mut first_active: Option<usize> = None;
        for (i, n) in self.nodes.iter().enumerate() {
            if n.active {
                first_active = Some(i);
                break;
            }
        }
        let Some(start) = first_active else {
            return Vec::new();
        };

        let mut stack: Vec<usize> = vec![start];
        let mut seen: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        seen.insert(start);

        while let Some(u) = stack.pop() {
            for &ei in &self.nodes[u].edges {
                if !self.edges[ei].active {
                    continue;
                }
                let v = self.edge_other_end(ei, u);
                if !self.nodes[v].active {
                    continue;
                }
                if seen.insert(v) {
                    stack.push(v);
                }
            }
        }

        let active_count = self.nodes.iter().filter(|n| n.active).count();
        if seen.len() == active_count {
            Vec::new()
        } else {
            (0..self.nodes.len())
                .filter(|&i| self.nodes[i].active)
                .collect()
        }
    }

    fn gravitation_context(&self, gravity_range_factor: f64) -> Option<(f64, f64, f64)> {
        // Port of `FDLayout.calcGravitationalForce` context:
        // - owner center = bbox center of the root graph
        // - estimatedSize = root.estimatedSize * gravityRangeFactor
        let mut min_left = f64::INFINITY;
        let mut max_right = f64::NEG_INFINITY;
        let mut min_top = f64::INFINITY;
        let mut max_bottom = f64::NEG_INFINITY;

        let mut size_sum = 0.0f64;
        let mut active_n = 0usize;

        for n in &self.nodes {
            if !n.active {
                continue;
            }
            active_n += 1;
            min_left = min_left.min(n.left);
            max_right = max_right.max(n.right());
            min_top = min_top.min(n.top);
            max_bottom = max_bottom.max(n.bottom());
            size_sum += (n.width + n.height) / 2.0;
        }

        if active_n == 0
            || !(min_left.is_finite()
                && max_right.is_finite()
                && min_top.is_finite()
                && max_bottom.is_finite())
        {
            return None;
        }

        let owner_center_x = (max_right + min_left) / 2.0;
        let owner_center_y = (max_bottom + min_top) / 2.0;

        let estimated_size_base = if size_sum == 0.0 {
            // `LayoutConstants.EMPTY_COMPOUND_NODE_SIZE`
            40.0
        } else {
            size_sum / (active_n as f64).sqrt()
        };
        let estimated_size = estimated_size_base * gravity_range_factor;
        if !estimated_size.is_finite() {
            return None;
        }

        Some((owner_center_x, owner_center_y, estimated_size))
    }
}

fn rects_intersect(a: &SimNode, b: &SimNode) -> bool {
    a.left < b.right() && a.right() > b.left && a.top < b.bottom() && a.bottom() > b.top
}

/// Port of `layout-base` `IGeometry.getIntersection2(rectA, rectB, result)`.
///
/// Returns `(ax, ay, bx, by, overlapped)` where `(ax,ay)` is rectA's clip point and `(bx,by)` is
/// rectB's clip point on the line segment between their centers.
fn rect_intersection_points(a: &SimNode, b: &SimNode) -> (f64, f64, f64, f64, bool) {
    let p1x = a.center_x();
    let p1y = a.center_y();
    let p2x = b.center_x();
    let p2y = b.center_y();

    if rects_intersect(a, b) {
        return (p1x, p1y, p2x, p2y, true);
    }

    let top_left_ax = a.left;
    let top_left_ay = a.top;
    let top_right_ax = a.right();
    let bottom_left_ax = a.left;
    let bottom_left_ay = a.bottom();
    let bottom_right_ax = a.right();
    let half_width_a = a.half_w();
    let half_height_a = a.half_h();

    let top_left_bx = b.left;
    let top_left_by = b.top;
    let top_right_bx = b.right();
    let bottom_left_bx = b.left;
    let bottom_left_by = b.bottom();
    let bottom_right_bx = b.right();
    let half_width_b = b.half_w();
    let half_height_b = b.half_h();

    // line is vertical
    if p1x == p2x {
        if p1y > p2y {
            return (p1x, top_left_ay, p2x, bottom_left_by, false);
        } else if p1y < p2y {
            return (p1x, bottom_left_ay, p2x, top_left_by, false);
        }
        return (p1x, p1y, p2x, p2y, false);
    }

    // line is horizontal
    if p1y == p2y {
        if p1x > p2x {
            return (top_left_ax, p1y, top_right_bx, p2y, false);
        } else if p1x < p2x {
            return (top_right_ax, p1y, top_left_bx, p2y, false);
        }
        return (p1x, p1y, p2x, p2y, false);
    }

    let slope_a = a.height / a.width;
    let slope_b = b.height / b.width;
    let slope_prime = (p2y - p1y) / (p2x - p1x);

    let mut ax = 0.0;
    let mut ay = 0.0;
    let mut bx = 0.0;
    let mut by = 0.0;
    let mut clip_point_a_found = false;
    let mut clip_point_b_found = false;

    // determine whether clipping point is the corner of nodeA
    if (-slope_a) == slope_prime {
        if p1x > p2x {
            ax = bottom_left_ax;
            ay = bottom_left_ay;
            clip_point_a_found = true;
        } else {
            ax = top_right_ax;
            ay = top_left_ay;
            clip_point_a_found = true;
        }
    } else if slope_a == slope_prime {
        if p1x > p2x {
            ax = top_left_ax;
            ay = top_left_ay;
            clip_point_a_found = true;
        } else {
            ax = bottom_right_ax;
            ay = bottom_left_ay;
            clip_point_a_found = true;
        }
    }

    // determine whether clipping point is the corner of nodeB
    if (-slope_b) == slope_prime {
        if p2x > p1x {
            bx = bottom_left_bx;
            by = bottom_left_by;
            clip_point_b_found = true;
        } else {
            bx = top_right_bx;
            by = top_left_by;
            clip_point_b_found = true;
        }
    } else if slope_b == slope_prime {
        if p2x > p1x {
            bx = top_left_bx;
            by = top_left_by;
            clip_point_b_found = true;
        } else {
            bx = bottom_right_bx;
            by = bottom_left_by;
            clip_point_b_found = true;
        }
    }

    if clip_point_a_found && clip_point_b_found {
        return (ax, ay, bx, by, false);
    }

    let get_cardinal_direction = |slope: f64, slope_prime: f64, line: i32| -> i32 {
        if slope > slope_prime {
            line
        } else {
            1 + (line % 4)
        }
    };

    let cardinal_direction_a: i32;
    let cardinal_direction_b: i32;
    if p1x > p2x {
        if p1y > p2y {
            cardinal_direction_a = get_cardinal_direction(slope_a, slope_prime, 4);
            cardinal_direction_b = get_cardinal_direction(slope_b, slope_prime, 2);
        } else {
            cardinal_direction_a = get_cardinal_direction(-slope_a, slope_prime, 3);
            cardinal_direction_b = get_cardinal_direction(-slope_b, slope_prime, 1);
        }
    } else if p1y > p2y {
        cardinal_direction_a = get_cardinal_direction(-slope_a, slope_prime, 1);
        cardinal_direction_b = get_cardinal_direction(-slope_b, slope_prime, 3);
    } else {
        cardinal_direction_a = get_cardinal_direction(slope_a, slope_prime, 2);
        cardinal_direction_b = get_cardinal_direction(slope_b, slope_prime, 4);
    }

    // calculate clipping Point if it is not found before
    if !clip_point_a_found {
        match cardinal_direction_a {
            1 => {
                ay = top_left_ay;
                ax = p1x + (-half_height_a) / slope_prime;
            }
            2 => {
                ax = bottom_right_ax;
                ay = p1y + half_width_a * slope_prime;
            }
            3 => {
                ay = bottom_left_ay;
                ax = p1x + half_height_a / slope_prime;
            }
            _ => {
                ax = bottom_left_ax;
                ay = p1y + (-half_width_a) * slope_prime;
            }
        }
    }

    if !clip_point_b_found {
        match cardinal_direction_b {
            1 => {
                by = top_left_by;
                bx = p2x + (-half_height_b) / slope_prime;
            }
            2 => {
                bx = bottom_right_bx;
                by = p2y + half_width_b * slope_prime;
            }
            3 => {
                by = bottom_left_by;
                bx = p2x + half_height_b / slope_prime;
            }
            _ => {
                bx = bottom_left_bx;
                by = p2y + (-half_width_b) * slope_prime;
            }
        }
    }

    (ax, ay, bx, by, false)
}

fn calc_separation_amount(a: &SimNode, b: &SimNode, separation_buffer: f64) -> (f64, f64) {
    debug_assert!(rects_intersect(a, b));

    let (dir_x, dir_y) = decide_directions_for_overlapping_nodes(a, b);

    let mut overlap_x = a.right().min(b.right()) - a.left.max(b.left);
    let mut overlap_y = a.bottom().min(b.bottom()) - a.top.max(b.top);

    if (a.left <= b.left) && (a.right() >= b.right()) {
        overlap_x += (b.left - a.left).min(a.right() - b.right());
    } else if (b.left <= a.left) && (b.right() >= a.right()) {
        overlap_x += (a.left - b.left).min(b.right() - a.right());
    }
    if (a.top <= b.top) && (a.bottom() >= b.bottom()) {
        overlap_y += (b.top - a.top).min(a.bottom() - b.bottom());
    } else if (b.top <= a.top) && (b.bottom() >= a.bottom()) {
        overlap_y += (a.top - b.top).min(b.bottom() - a.bottom());
    }

    let mut slope = ((b.center_y() - a.center_y()) / (b.center_x() - a.center_x())).abs();
    if (b.center_y() == a.center_y()) && (b.center_x() == a.center_x()) {
        slope = 1.0;
    }

    let mut move_by_y = slope * overlap_x;
    let mut move_by_x = overlap_y / slope;
    if overlap_x < move_by_x {
        move_by_x = overlap_x;
    } else {
        move_by_y = overlap_y;
    }

    let dx = -(dir_x as f64) * ((move_by_x / 2.0) + separation_buffer);
    let dy = -(dir_y as f64) * ((move_by_y / 2.0) + separation_buffer);
    (dx, dy)
}

fn decide_directions_for_overlapping_nodes(a: &SimNode, b: &SimNode) -> (i32, i32) {
    let dir_x = if a.center_x() < b.center_x() { -1 } else { 1 };
    let dir_y = if a.center_y() < b.center_y() { -1 } else { 1 };
    (dir_x, dir_y)
}
