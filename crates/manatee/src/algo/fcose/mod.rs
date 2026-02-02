use crate::algo::FcoseOptions;
use crate::error::Result;
use crate::graph::{Graph, LayoutResult, Point};

pub fn layout(graph: &Graph, opts: &FcoseOptions) -> Result<LayoutResult> {
    graph.validate()?;

    let mut sim = SimGraph::from_graph(graph);
    let constraints = Constraints::from_opts(&sim, opts);

    // Mimic fcose's `aux.relocateComponent(...)`: keep the final component center aligned to the
    // original component center to avoid arbitrary global translations affecting viewBox parity.
    let orig_center = sim.bounding_box_center().unwrap_or((0.0, 0.0));

    sim.run_spring_embedder(&constraints);

    let new_center = sim.bounding_box_center().unwrap_or((0.0, 0.0));
    sim.translate(orig_center.0 - new_center.0, orig_center.1 - new_center.1);

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
    // Top-left anchored rectangle (layout-base `LNode.rect` style).
    left: f64,
    top: f64,

    spring_fx: f64,
    spring_fy: f64,
    repulsion_fx: f64,
    repulsion_fy: f64,
}

impl SimNode {
    fn center_x(&self) -> f64 {
        self.left + self.width / 2.0
    }

    fn center_y(&self) -> f64 {
        self.top + self.height / 2.0
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
    ideal_length: f64,
}

#[derive(Debug, Clone)]
struct Constraints {
    align_horizontal: Vec<Vec<usize>>,
    align_vertical: Vec<Vec<usize>>,
    relative: Vec<RelConstraint>,
}

#[derive(Debug, Clone, Copy)]
struct RelConstraint {
    left: Option<usize>,
    right: Option<usize>,
    top: Option<usize>,
    bottom: Option<usize>,
    gap: f64,
}

impl Constraints {
    fn from_opts(sim: &SimGraph, opts: &FcoseOptions) -> Self {
        let mut align_horizontal: Vec<Vec<usize>> = Vec::new();
        let mut align_vertical: Vec<Vec<usize>> = Vec::new();

        if let Some(a) = opts.alignment_constraint.as_ref() {
            align_horizontal = map_align_lists(sim, &a.horizontal);
            align_vertical = map_align_lists(sim, &a.vertical);
        }

        let mut relative: Vec<RelConstraint> = Vec::new();
        for r in &opts.relative_placement_constraint {
            relative.push(RelConstraint {
                left: r
                    .left
                    .as_deref()
                    .and_then(|id| sim.id_to_idx.get(id).copied()),
                right: r
                    .right
                    .as_deref()
                    .and_then(|id| sim.id_to_idx.get(id).copied()),
                top: r
                    .top
                    .as_deref()
                    .and_then(|id| sim.id_to_idx.get(id).copied()),
                bottom: r
                    .bottom
                    .as_deref()
                    .and_then(|id| sim.id_to_idx.get(id).copied()),
                gap: r.gap.max(0.0),
            });
        }

        Self {
            align_horizontal,
            align_vertical,
            relative,
        }
    }
}

fn map_align_lists(sim: &SimGraph, groups: &[Vec<String>]) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    for g in groups {
        let mut idxs: Vec<usize> = g
            .iter()
            .filter_map(|id| sim.id_to_idx.get(id.as_str()).copied())
            .collect();
        idxs.sort_unstable();
        idxs.dedup();
        if idxs.len() > 1 {
            out.push(idxs);
        }
    }
    out
}

#[derive(Debug)]
struct SimGraph {
    nodes: Vec<SimNode>,
    edges: Vec<SimEdge>,
    id_to_idx: std::collections::BTreeMap<String, usize>,
}

impl SimGraph {
    const DEFAULT_EDGE_LENGTH: f64 = 50.0;
    const DEFAULT_SPRING_STRENGTH: f64 = 0.45;
    const DEFAULT_REPULSION_STRENGTH: f64 = 4500.0;

    const MAX_ITERATIONS: usize = 2500;
    const CONVERGENCE_CHECK_PERIOD: usize = 100;
    const MAX_NODE_DISPLACEMENT: f64 = 300.0;
    const MIN_REPULSION_DIST: f64 = Self::DEFAULT_EDGE_LENGTH / 10.0;

    fn from_graph(graph: &Graph) -> Self {
        let mut nodes: Vec<SimNode> = Vec::with_capacity(graph.nodes.len());
        let mut id_to_idx: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();

        for (idx, n) in graph.nodes.iter().enumerate() {
            let w = n.width.max(1.0);
            let h = n.height.max(1.0);
            nodes.push(SimNode {
                id: n.id.clone(),
                width: w,
                height: h,
                left: n.x - w / 2.0,
                top: n.y - h / 2.0,
                spring_fx: 0.0,
                spring_fy: 0.0,
                repulsion_fx: 0.0,
                repulsion_fy: 0.0,
            });
            id_to_idx.insert(n.id.clone(), idx);
        }

        let mut seen_pairs: std::collections::BTreeSet<(usize, usize)> =
            std::collections::BTreeSet::new();
        let mut edges: Vec<SimEdge> = Vec::new();
        for e in &graph.edges {
            let Some(&a) = id_to_idx.get(e.source.as_str()) else {
                continue;
            };
            let Some(&b) = id_to_idx.get(e.target.as_str()) else {
                continue;
            };
            if a == b {
                continue;
            }
            let (u, v) = if a < b { (a, b) } else { (b, a) };
            if seen_pairs.contains(&(u, v)) {
                continue;
            }
            seen_pairs.insert((u, v));

            let ideal = if e.ideal_length.is_finite() && e.ideal_length > 0.0 {
                e.ideal_length
            } else {
                Self::DEFAULT_EDGE_LENGTH
            };
            edges.push(SimEdge {
                a,
                b,
                ideal_length: ideal.max(1.0),
            });
        }

        Self {
            nodes,
            edges,
            id_to_idx,
        }
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        for n in &mut self.nodes {
            n.left += dx;
            n.top += dy;
        }
    }

    fn bounding_box_center(&self) -> Option<(f64, f64)> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for n in &self.nodes {
            min_x = min_x.min(n.left);
            min_y = min_y.min(n.top);
            max_x = max_x.max(n.right());
            max_y = max_y.max(n.bottom());
        }
        if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
            return None;
        }
        Some(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
    }

    fn run_spring_embedder(&mut self, constraints: &Constraints) {
        if self.nodes.is_empty() {
            return;
        }

        let ideal_edge_length_avg = if self.edges.is_empty() {
            Self::DEFAULT_EDGE_LENGTH
        } else {
            let sum: f64 = self.edges.iter().map(|e| e.ideal_length).sum();
            (sum / (self.edges.len() as f64)).max(1.0)
        };

        let spring_constant = Self::DEFAULT_SPRING_STRENGTH;
        let repulsion_constant = Self::DEFAULT_REPULSION_STRENGTH;

        let n = self.nodes.len() as f64;
        let displacement_threshold_per_node = (3.0 * ideal_edge_length_avg) / 100.0;
        let total_displacement_threshold = displacement_threshold_per_node * n;

        let initial_cooling_factor = 1.0;
        let mut cooling_factor = initial_cooling_factor;
        let max_iterations = Self::MAX_ITERATIONS.max((self.nodes.len() * 5) as usize);
        let max_cooling_cycle = (max_iterations as f64) / (Self::CONVERGENCE_CHECK_PERIOD as f64);
        let final_temperature = (Self::CONVERGENCE_CHECK_PERIOD as f64) / (max_iterations as f64);
        let mut cooling_cycle = 0.0f64;

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

                let numerator = (100.0 * (initial_cooling_factor - final_temperature)).ln();
                let denominator = max_cooling_cycle.ln().max(1e-9);
                let power = numerator / denominator;
                let schedule = cooling_cycle.powf(power) / 100.0;
                cooling_factor = (initial_cooling_factor - schedule).max(final_temperature);
            }

            let mut total_displacement = 0.0f64;

            // Spring forces (per-edge ideal lengths).
            for e in &self.edges {
                let (a, b) = (e.a, e.b);
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

                let spring_force = spring_constant * (len - e.ideal_length.max(1.0));
                let sfx = spring_force * (lx / len);
                let sfy = spring_force * (ly / len);
                self.nodes[a].spring_fx += sfx;
                self.nodes[a].spring_fy += sfy;
                self.nodes[b].spring_fx -= sfx;
                self.nodes[b].spring_fy -= sfy;
            }

            // Repulsion forces (O(n^2)).
            for i in 0..self.nodes.len() {
                for j in (i + 1)..self.nodes.len() {
                    let (rfx, rfy) = calc_repulsion_force(
                        &self.nodes[i],
                        &self.nodes[j],
                        repulsion_constant,
                        ideal_edge_length_avg / 2.0,
                    );
                    self.nodes[i].repulsion_fx += rfx;
                    self.nodes[i].repulsion_fy += rfy;
                    self.nodes[j].repulsion_fx -= rfx;
                    self.nodes[j].repulsion_fy -= rfy;
                }
            }

            // Move nodes.
            for n in &mut self.nodes {
                let dx = cooling_factor * (n.spring_fx + n.repulsion_fx);
                let dy = cooling_factor * (n.spring_fy + n.repulsion_fy);

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

                n.spring_fx = 0.0;
                n.spring_fy = 0.0;
                n.repulsion_fx = 0.0;
                n.repulsion_fy = 0.0;
            }

            // Constraint projection (approximation): keep alignments exact and maintain required gaps.
            self.enforce_constraints(constraints);

            last_total_displacement = total_displacement;
        }
    }

    fn enforce_constraints(&mut self, c: &Constraints) {
        // Alignments.
        for group in &c.align_horizontal {
            let mut sum = 0.0;
            let mut cnt = 0.0;
            for &idx in group {
                sum += self.nodes[idx].center_y();
                cnt += 1.0;
            }
            if cnt > 0.0 {
                let y = sum / cnt;
                for &idx in group {
                    self.nodes[idx].top = y - self.nodes[idx].half_h();
                }
            }
        }
        for group in &c.align_vertical {
            let mut sum = 0.0;
            let mut cnt = 0.0;
            for &idx in group {
                sum += self.nodes[idx].center_x();
                cnt += 1.0;
            }
            if cnt > 0.0 {
                let x = sum / cnt;
                for &idx in group {
                    self.nodes[idx].left = x - self.nodes[idx].half_w();
                }
            }
        }

        // Relative placements.
        // Note: Constraints are expressed in terms of node order + a `gap` between borders.
        for r in &c.relative {
            if let (Some(left), Some(right)) = (r.left, r.right) {
                let required = self.nodes[left].center_x()
                    + self.nodes[left].half_w()
                    + r.gap
                    + self.nodes[right].half_w();
                let actual = self.nodes[right].center_x();
                if actual < required {
                    let delta = required - actual;
                    self.nodes[right].move_by(delta, 0.0);
                }
            }
            if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
                let required = self.nodes[top].center_y()
                    + self.nodes[top].half_h()
                    + r.gap
                    + self.nodes[bottom].half_h();
                let actual = self.nodes[bottom].center_y();
                if actual < required {
                    let delta = required - actual;
                    self.nodes[bottom].move_by(0.0, delta);
                }
            }
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

fn calc_repulsion_force(
    a: &SimNode,
    b: &SimNode,
    repulsion_constant: f64,
    separation_buffer: f64,
) -> (f64, f64) {
    if rects_intersect(a, b) {
        let (ox, oy) = calc_separation_amount(a, b, separation_buffer);
        let repulsion_fx = 2.0 * ox;
        let repulsion_fy = 2.0 * oy;
        (-0.5 * repulsion_fx, -0.5 * repulsion_fy)
    } else {
        let (ax, ay) = rect_clip_point_towards(a, b);
        let (bx, by) = rect_clip_point_towards(b, a);
        let mut dx = bx - ax;
        let mut dy = by - ay;

        if dx.abs() < 1e-9 {
            dx = 0.0;
        }
        if dy.abs() < 1e-9 {
            dy = 0.0;
        }

        if dx.abs() < SimGraph::MIN_REPULSION_DIST {
            dx = dx.signum() * SimGraph::MIN_REPULSION_DIST;
        }
        if dy.abs() < SimGraph::MIN_REPULSION_DIST {
            dy = dy.signum() * SimGraph::MIN_REPULSION_DIST;
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

fn calc_separation_amount(a: &SimNode, b: &SimNode, separation_buffer: f64) -> (f64, f64) {
    debug_assert!(rects_intersect(a, b));

    let (dir_x, dir_y) = decide_directions_for_overlapping_nodes(a, b);

    // Port of layout-base `IGeometry.calcSeparationAmount` overlap logic used by FDLayout.
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

    let dx = -1.0 * dir_x * ((move_by_x / 2.0) + separation_buffer);
    let dy = -1.0 * dir_y * ((move_by_y / 2.0) + separation_buffer);
    (dx, dy)
}

fn decide_directions_for_overlapping_nodes(a: &SimNode, b: &SimNode) -> (f64, f64) {
    let dir_x = if a.center_x() < b.center_x() {
        -1.0
    } else {
        1.0
    };
    let dir_y = if a.center_y() < b.center_y() {
        -1.0
    } else {
        1.0
    };
    (dir_x, dir_y)
}
