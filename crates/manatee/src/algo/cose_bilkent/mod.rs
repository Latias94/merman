use crate::algo::CoseBilkentOptions;
use crate::error::Result;
use crate::graph::{Graph, LayoutResult, Point};

pub fn layout(graph: &Graph, _opts: &CoseBilkentOptions) -> Result<LayoutResult> {
    graph.validate()?;

    let mut sim = SimGraph::from_graph(graph);

    // Minimal COSE-Bilkent port for flat graphs that are forests (trees).
    // This follows the upstream `cose-base` control flow:
    // - `getFlatForest()` + `positionNodesRadially(...)`
    // - `doPostLayout()` -> `transform(0,0)` to move the graph into positive coordinates
    // Note: the force-directed spring embedder is not implemented yet.
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

#[derive(Debug)]
struct SimGraph {
    nodes: Vec<SimNode>,
    edges: Vec<SimEdge>,
}

impl SimGraph {
    const DEFAULT_GRAPH_MARGIN: f64 = 15.0;
    const DEFAULT_COMPONENT_SEPERATION: f64 = 60.0; // upstream typo preserved
    const DEFAULT_EDGE_LENGTH: f64 = 50.0;
    const DEFAULT_RADIAL_SEPARATION: f64 = Self::DEFAULT_EDGE_LENGTH;

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
                spring_fx: 0.0,
                spring_fy: 0.0,
                repulsion_fx: 0.0,
                repulsion_fy: 0.0,
                gravitation_fx: 0.0,
                gravitation_fy: 0.0,
            });
        }

        let mut id_to_idx: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for (idx, n) in graph.nodes.iter().enumerate() {
            id_to_idx.insert(n.id.as_str(), idx);
        }

        // Mirror the cytoscape-cose-bilkent behavior: only keep one edge between any two nodes.
        let mut seen_pairs: std::collections::BTreeSet<(usize, usize)> =
            std::collections::BTreeSet::new();
        let mut edges: Vec<SimEdge> = Vec::new();
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
            edges.push(SimEdge { a, b });
            nodes[a].edges.push(ei);
            nodes[b].edges.push(ei);
        }

        Self { nodes, edges }
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
            if self.edge_other_end(ei, a) == b {
                out.push(ei);
            }
        }
        out
    }

    fn neighbors_of(&self, node_idx: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for &ei in &self.nodes[node_idx].edges {
            out.push(self.edge_other_end(ei, node_idx));
        }
        out
    }

    /// Port of `layout-base` `Layout.getFlatForest()` for flat graphs.
    fn get_flat_forest(&self) -> Vec<Vec<usize>> {
        let mut flat_forest: Vec<Vec<usize>> = Vec::new();
        let mut is_forest = true;

        // Root graph nodes in insertion order.
        let all_nodes: Vec<usize> = (0..self.nodes.len()).collect();

        // Graph is always flat in our current model (no compound nodes).

        // BFS for each component; reject if any component is not a tree.
        let mut to_be_visited: std::collections::VecDeque<usize> =
            std::collections::VecDeque::new();
        let mut parents: std::collections::BTreeMap<usize, usize> =
            std::collections::BTreeMap::new();
        let mut unprocessed_nodes: Vec<usize> = all_nodes;

        while !unprocessed_nodes.is_empty() && is_forest {
            to_be_visited.push_back(unprocessed_nodes[0]);

            let mut visited_set: std::collections::BTreeSet<usize> =
                std::collections::BTreeSet::new();
            let mut visited_order: Vec<usize> = Vec::new();

            while let Some(current_node) = to_be_visited.pop_front() {
                if visited_set.insert(current_node) {
                    visited_order.push(current_node);
                }

                // Traverse all neighbors of this node, in edge insertion order.
                for &ei in &self.nodes[current_node].edges {
                    let current_neighbor = self.edge_other_end(ei, current_node);

                    // If BFS is not growing from this neighbor.
                    if parents.get(&current_node).copied() != Some(current_neighbor) {
                        if !visited_set.contains(&current_neighbor) {
                            to_be_visited.push_back(current_neighbor);
                            parents.insert(current_neighbor, current_node);
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

                // Remove all visited nodes from unProcessedNodes using the same splice-by-index logic.
                for v in visited_order {
                    if let Some(pos) = unprocessed_nodes.iter().position(|&n| n == v) {
                        unprocessed_nodes.remove(pos);
                    }
                }

                // Reset per-component state.
                parents.clear();
                to_be_visited.clear();
            }
        }

        flat_forest
    }

    /// Port of `layout-base` `Layout.findCenterOfTree(nodes)`.
    /// Note: this intentionally preserves the upstream loop's in-place removal behavior.
    fn find_center_of_tree(&self, nodes: &[usize]) -> usize {
        let mut list: Vec<usize> = nodes.to_vec();
        let mut removed_nodes: Vec<usize> = Vec::new();
        let mut remaining_degrees: std::collections::BTreeMap<usize, usize> =
            std::collections::BTreeMap::new();
        let mut found_center = false;
        let mut center_node = list[0];

        if list.len() == 1 || list.len() == 2 {
            found_center = true;
            center_node = list[0];
        }

        for &node in &list {
            let degree = self.neighbors_of(node).len();
            remaining_degrees.insert(node, degree);
            if degree == 1 {
                removed_nodes.push(node);
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
                if let Some(pos) = list.iter().position(|&n| n == node) {
                    list.remove(pos);
                }

                for neighbour in self.neighbors_of(node) {
                    if !removed_nodes.contains(&neighbour) {
                        let other_degree = *remaining_degrees.get(&neighbour).unwrap_or(&0);
                        let new_degree = other_degree.saturating_sub(1);
                        if new_degree == 1 {
                            temp_list.push(neighbour);
                        }
                        remaining_degrees.insert(neighbour, new_degree);
                    }
                }

                i += 1;
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
        let start_index: usize;

        let mut edges_to_parent = parent
            .map(|p| self.edges_between(node, p))
            .unwrap_or_default();
        while edges_to_parent.len() > 1 {
            let temp = edges_to_parent.remove(0);
            if let Some(pos) = neighbor_edges.iter().position(|&e| e == temp) {
                neighbor_edges.remove(pos);
            }
            if child_count > 0 {
                child_count -= 1;
            }
        }

        if parent.is_some() && !edges_to_parent.is_empty() && inc_edges_count > 0 {
            start_index = (neighbor_edges
                .iter()
                .position(|&e| e == edges_to_parent[0])
                .unwrap_or(0)
                + 1)
                % inc_edges_count;
        } else {
            start_index = 0;
        }

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

        // Upstream `positionNodesRadially` applies a world-center translation here, but `doPostLayout()`
        // later calls `transform(0,0)` which normalizes the graph to the origin. Relative coordinates
        // are translation-invariant, so we skip the intermediate world-centering step.
        let _ = point;
    }

    fn run_spring_embedder(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        // These are instance fields in upstream `FDLayout`/`CoSELayout`.
        let ideal_edge_length = Self::DEFAULT_EDGE_LENGTH.max(10.0);
        let spring_constant = Self::DEFAULT_SPRING_STRENGTH;
        let repulsion_constant = Self::DEFAULT_REPULSION_STRENGTH;
        let gravity_constant = Self::DEFAULT_GRAVITY_STRENGTH;
        let gravity_range_factor = Self::DEFAULT_GRAVITY_RANGE_FACTOR;

        let n = self.nodes.len() as f64;
        let displacement_threshold_per_node = (3.0 * Self::DEFAULT_EDGE_LENGTH) / 100.0;
        let total_displacement_threshold = displacement_threshold_per_node * n;

        // Non-incremental mode: coolingFactor starts at 1.0 for small graphs.
        let initial_cooling_factor = 1.0;
        let mut cooling_factor = initial_cooling_factor;
        let max_iterations = Self::MAX_ITERATIONS.max((self.nodes.len() * 5) as usize);
        let max_cooling_cycle = (max_iterations as f64) / (Self::CONVERGENCE_CHECK_PERIOD as f64);
        let final_temperature = (Self::CONVERGENCE_CHECK_PERIOD as f64) / (max_iterations as f64);
        let mut cooling_cycle = 0.0f64;
        let cooling_adjuster = 1.0; // layoutQuality=proof leaves this at 1

        let mut total_iterations = 0usize;
        let mut old_total_displacement = 0.0f64;
        let mut last_total_displacement = 0.0f64;

        loop {
            total_iterations += 1;
            if total_iterations == max_iterations {
                break;
            }

            if total_iterations % Self::CONVERGENCE_CHECK_PERIOD == 0 {
                let oscilating = total_iterations > (max_iterations / 3)
                    && (last_total_displacement - old_total_displacement).abs() < 2.0;
                let converged = last_total_displacement < total_displacement_threshold;

                old_total_displacement = last_total_displacement;

                if converged || oscilating {
                    break;
                }

                cooling_cycle += 1.0;

                // cooling schedule 3 (see upstream comment in `CoSELayout.tick`)
                let numerator = (100.0 * (initial_cooling_factor - final_temperature)).ln();
                let denominator = max_cooling_cycle.ln().max(1e-9);
                let power = numerator / denominator;
                let schedule = cooling_cycle.powf(power) / 100.0 * cooling_adjuster;
                cooling_factor = (initial_cooling_factor - schedule).max(final_temperature);
            }

            let mut total_displacement = 0.0f64;

            // Spring forces
            for e in &self.edges {
                let (a, b) = (e.a, e.b);

                // Upstream `FDLayout.calcSpringForce` uses clipping points on the node rectangles
                // (via `IGeometry.getIntersection`) so the "ideal edge length" applies between
                // node borders rather than between node centers.
                if rects_intersect(&self.nodes[a], &self.nodes[b]) {
                    continue;
                }
                let (ax, ay) = rect_clip_point_towards(&self.nodes[a], &self.nodes[b]);
                let (bx, by) = rect_clip_point_towards(&self.nodes[b], &self.nodes[a]);
                let mut lx = bx - ax;
                let mut ly = by - ay;

                if lx.abs() < 1e-9 {
                    lx = 0.0;
                }
                if ly.abs() < 1e-9 {
                    ly = 0.0;
                }
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
                for j in (i + 1)..self.nodes.len() {
                    let (rfx, rfy) = self.calc_repulsion_force(i, j, repulsion_constant);
                    self.nodes[i].repulsion_fx += rfx;
                    self.nodes[i].repulsion_fy += rfy;
                    self.nodes[j].repulsion_fx -= rfx;
                    self.nodes[j].repulsion_fy -= rfy;
                }
            }

            // Gravitation is applied only to disconnected components upstream. Keep it as a no-op for now.
            let _ = (gravity_constant, gravity_range_factor);

            // Move nodes
            for n in &mut self.nodes {
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
            let (ax, ay) = rect_clip_point_towards(na, nb);
            let (bx, by) = rect_clip_point_towards(nb, na);
            let mut dx = bx - ax;
            let mut dy = by - ay;

            if dx.abs() < 1e-9 {
                dx = 0.0;
            }
            if dy.abs() < 1e-9 {
                dy = 0.0;
            }

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
            n.left += dx;
            n.top += dy;
        }
    }
}

fn rects_intersect(a: &SimNode, b: &SimNode) -> bool {
    a.left < b.right() && a.right() > b.left && a.top < b.bottom() && a.bottom() > b.top
}

fn rect_clip_point_towards(a: &SimNode, b: &SimNode) -> (f64, f64) {
    let ax = a.center_x();
    let ay = a.center_y();
    let bx = b.center_x();
    let by = b.center_y();
    let dx = bx - ax;
    let dy = by - ay;

    // If centers coincide, use the center (should be avoided by overlap handling).
    if dx == 0.0 && dy == 0.0 {
        return (ax, ay);
    }

    let mut t_x = f64::INFINITY;
    let mut t_y = f64::INFINITY;
    if dx != 0.0 {
        t_x = (a.half_w() / dx.abs()).max(0.0);
    }
    if dy != 0.0 {
        t_y = (a.half_h() / dy.abs()).max(0.0);
    }
    let t = t_x.min(t_y);
    (ax + t * dx, ay + t * dy)
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

    let dx = -1.0 * (dir_x as f64) * ((move_by_x / 2.0) + separation_buffer);
    let dy = -1.0 * (dir_y as f64) * ((move_by_y / 2.0) + separation_buffer);
    (dx, dy)
}

fn decide_directions_for_overlapping_nodes(a: &SimNode, b: &SimNode) -> (i32, i32) {
    let dir_x = if a.center_x() < b.center_x() { -1 } else { 1 };
    let dir_y = if a.center_y() < b.center_y() { -1 } else { 1 };
    (dir_x, dir_y)
}
