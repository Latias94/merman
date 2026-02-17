use crate::algo::FcoseOptions;
use crate::error::Result;
use crate::graph::{Anchor, Graph, LayoutResult, Point};
use indexmap::{IndexMap, IndexSet};
use nalgebra as na;
use rustc_hash::FxHashMap;

mod spectral;

#[derive(Debug, Default, Clone)]
struct FcoseLayoutTimings {
    total: std::time::Duration,
    from_graph: std::time::Duration,
    constraints: std::time::Duration,
    spring: FcoseSpringTimings,
    translate: std::time::Duration,
    output: std::time::Duration,
}

#[derive(Debug, Default, Clone)]
struct FcoseSpringTimings {
    total: std::time::Duration,
    opts_prep: std::time::Duration,
    spectral: std::time::Duration,
    root_compound: std::time::Duration,
    collapse_start_positions: std::time::Duration,
    pre_constraints: std::time::Duration,
    constraint_rt: std::time::Duration,
    iterations: std::time::Duration,
}

#[derive(Debug, Default, Clone, Copy)]
struct SpringStats {
    iterations: usize,
    spectral_applied: bool,
}

pub fn layout(graph: &Graph, opts: &FcoseOptions) -> Result<LayoutResult> {
    graph.validate()?;

    let timing_enabled = std::env::var("MANATEE_FCOSE_TIMING").ok().as_deref() == Some("1");
    let mut timings = FcoseLayoutTimings::default();
    let total_start = timing_enabled.then(std::time::Instant::now);

    let from_graph_start = timing_enabled.then(std::time::Instant::now);
    let mut sim = SimGraph::from_graph(graph);
    if let Some(s) = from_graph_start {
        timings.from_graph = s.elapsed();
    }

    let constraints_start = timing_enabled.then(std::time::Instant::now);
    let constraints = Constraints::from_opts(&sim, opts);
    if let Some(s) = constraints_start {
        timings.constraints = s.elapsed();
    }

    // Mimic fcose's `aux.relocateComponent(...)`: keep the final component center aligned to the
    // original component center to avoid arbitrary global translations affecting viewBox parity.
    let orig_center = sim.bounding_box_center().unwrap_or((0.0, 0.0));

    let spring_start = timing_enabled.then(std::time::Instant::now);
    let spring_stats = sim.run_spring_embedder(
        &constraints,
        opts,
        if timing_enabled {
            Some(&mut timings.spring)
        } else {
            None
        },
    );
    if let Some(s) = spring_start {
        timings.spring.total = s.elapsed();
    }

    let new_center = sim.bounding_box_center().unwrap_or((0.0, 0.0));
    let translate_start = timing_enabled.then(std::time::Instant::now);
    sim.translate(orig_center.0 - new_center.0, orig_center.1 - new_center.1);
    if let Some(s) = translate_start {
        timings.translate = s.elapsed();
    }

    let output_start = timing_enabled.then(std::time::Instant::now);
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
    if let Some(s) = output_start {
        timings.output = s.elapsed();
    }

    if let Some(s) = total_start {
        timings.total = s.elapsed();
        eprintln!(
            "[manatee-fcose-timing] total={:?} from_graph={:?} constraints={:?} spring_total={:?} spring_opts_prep={:?} spring_spectral={:?} spring_root_compound={:?} spring_collapse_start={:?} spring_pre_constraints={:?} spring_constraint_rt={:?} spring_iterations={:?} translate={:?} output={:?} nodes={} edges={} compounds={} iterations={} spectral_applied={}",
            timings.total,
            timings.from_graph,
            timings.constraints,
            timings.spring.total,
            timings.spring.opts_prep,
            timings.spring.spectral,
            timings.spring.root_compound,
            timings.spring.collapse_start_positions,
            timings.spring.pre_constraints,
            timings.spring.constraint_rt,
            timings.spring.iterations,
            timings.translate,
            timings.output,
            sim.nodes.len(),
            sim.edges.len(),
            sim.compound_parent.len(),
            spring_stats.iterations,
            spring_stats.spectral_applied,
        );
    }

    Ok(LayoutResult { positions })
}

#[derive(Debug, Clone)]
struct SimNode {
    id: String,
    parent: Option<String>,
    width: f64,
    height: f64,
    // Top-left anchored rectangle (layout-base `LNode.rect` style).
    left: f64,
    top: f64,

    spring_fx: f64,
    spring_fy: f64,
    repulsion_fx: f64,
    repulsion_fy: f64,

    // layout-base FR-grid repulsion caches a per-node "surrounding" list, refreshed periodically.
    surrounding: Vec<usize>,
    grid_start_x: i32,
    grid_finish_x: i32,
    grid_start_y: i32,
    grid_finish_y: i32,
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
    #[allow(dead_code)]
    a_anchor: Option<Anchor>,
    #[allow(dead_code)]
    b_anchor: Option<Anchor>,
    ideal_length: f64,
    elasticity: f64,
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

#[derive(Debug, Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
struct ConstraintRuntime {
    horizontal: AxisConstraintRuntime,
    vertical: AxisConstraintRuntime,
}

#[derive(Debug, Clone)]
struct AxisConstraintRuntime {
    node_count: usize,
    dummy_to_nodes: Vec<Vec<usize>>,
    #[allow(dead_code)]
    node_to_dummy: Vec<Option<usize>>,
    fixed_nodes: IndexSet<usize>,
    nodes_in_relative: Vec<usize>,
    rel_map: Vec<Vec<AxisRelAdj>>,
    temp_pos: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
enum AxisRelAdj {
    Right { node: usize, gap: f64 },
    Left { node: usize, gap: f64 },
    Bottom { node: usize, gap: f64 },
    Top { node: usize, gap: f64 },
}

impl ConstraintRuntime {
    fn new(nodes: &[SimNode], c: &Constraints) -> Option<Self> {
        if c.relative.is_empty() {
            return None;
        }
        Some(Self {
            horizontal: AxisConstraintRuntime::new_axis(
                nodes,
                &c.align_vertical,
                &c.relative,
                Axis::Horizontal,
            ),
            vertical: AxisConstraintRuntime::new_axis(
                nodes,
                &c.align_horizontal,
                &c.relative,
                Axis::Vertical,
            ),
        })
    }

    fn update_displacements(
        &mut self,
        nodes: &[SimNode],
        c: &Constraints,
        disps: &mut [(f64, f64)],
        total_iterations: usize,
        max_d: f64,
        rng: &mut XorShift64Star,
    ) {
        // Fixed nodes (not currently exposed by our public API).
        for &idx in &self.horizontal.fixed_nodes {
            if idx < disps.len() {
                disps[idx].0 = 0.0;
            }
        }
        for &idx in &self.vertical.fixed_nodes {
            if idx < disps.len() {
                disps[idx].1 = 0.0;
            }
        }

        // Alignments (match `cose-base` updateDisplacements): average displacements per group.
        for group in &c.align_vertical {
            if group.len() <= 1 {
                continue;
            }
            let mut sum = 0.0;
            for &idx in group {
                sum += disps[idx].0;
            }
            let avg = sum / (group.len() as f64);
            for &idx in group {
                disps[idx].0 = avg;
            }
        }
        for group in &c.align_horizontal {
            if group.len() <= 1 {
                continue;
            }
            let mut sum = 0.0;
            for &idx in group {
                sum += disps[idx].1;
            }
            let avg = sum / (group.len() as f64);
            for &idx in group {
                disps[idx].1 = avg;
            }
        }

        // Relative placements (match `cose-base` relax-movement mode).
        self.horizontal
            .refresh_temp_positions(nodes, Axis::Horizontal);
        self.vertical.refresh_temp_positions(nodes, Axis::Vertical);

        if total_iterations.is_multiple_of(10) {
            self.horizontal.shuffle_tail_third(rng);
            self.vertical.shuffle_tail_third(rng);
        }

        self.horizontal
            .apply_relative_relaxation(disps, max_d, Axis::Horizontal);
        self.vertical
            .apply_relative_relaxation(disps, max_d, Axis::Vertical);

        // Re-apply per-axis displacement caps (matching the upstream `calculateDisplacement` clamp).
        if max_d.is_finite() && max_d > 0.0 {
            for (dx, dy) in disps {
                if dx.abs() > max_d {
                    *dx = max_d * dx.signum();
                }
                if dy.abs() > max_d {
                    *dy = max_d * dy.signum();
                }
            }
        }
    }
}

impl AxisConstraintRuntime {
    fn new_axis(
        nodes: &[SimNode],
        axis_alignment_groups: &[Vec<usize>],
        rel: &[RelConstraint],
        axis: Axis,
    ) -> Self {
        let n = nodes.len();
        let d = axis_alignment_groups.len();

        let mut node_to_dummy: Vec<Option<usize>> = vec![None; n];
        let mut dummy_to_nodes: Vec<Vec<usize>> = Vec::with_capacity(d);
        for (i, group) in axis_alignment_groups.iter().enumerate() {
            let dummy_key = n + i;
            dummy_to_nodes.push(group.clone());
            for &idx in group {
                if idx < n {
                    node_to_dummy[idx] = Some(dummy_key);
                }
            }
        }

        let key_count = n + d;
        let mut rel_map: Vec<Vec<AxisRelAdj>> = vec![Vec::new(); key_count];
        let mut nodes_in_relative_set: IndexSet<usize> = IndexSet::new();

        for r in rel {
            match axis {
                Axis::Horizontal => {
                    let (Some(left), Some(right)) = (r.left, r.right) else {
                        continue;
                    };
                    let lk = node_to_dummy.get(left).copied().flatten().unwrap_or(left);
                    let rk = node_to_dummy.get(right).copied().flatten().unwrap_or(right);
                    nodes_in_relative_set.insert(lk);
                    nodes_in_relative_set.insert(rk);
                    rel_map[lk].push(AxisRelAdj::Right {
                        node: rk,
                        gap: r.gap,
                    });
                    rel_map[rk].push(AxisRelAdj::Left {
                        node: lk,
                        gap: r.gap,
                    });
                }
                Axis::Vertical => {
                    let (Some(top), Some(bottom)) = (r.top, r.bottom) else {
                        continue;
                    };
                    let tk = node_to_dummy.get(top).copied().flatten().unwrap_or(top);
                    let bk = node_to_dummy
                        .get(bottom)
                        .copied()
                        .flatten()
                        .unwrap_or(bottom);
                    nodes_in_relative_set.insert(tk);
                    nodes_in_relative_set.insert(bk);
                    rel_map[tk].push(AxisRelAdj::Bottom {
                        node: bk,
                        gap: r.gap,
                    });
                    rel_map[bk].push(AxisRelAdj::Top {
                        node: tk,
                        gap: r.gap,
                    });
                }
            }
        }

        let mut rt = Self {
            node_count: n,
            dummy_to_nodes,
            node_to_dummy,
            fixed_nodes: IndexSet::new(),
            nodes_in_relative: nodes_in_relative_set.into_iter().collect(),
            rel_map,
            temp_pos: vec![0.0; key_count],
        };
        rt.refresh_temp_positions(nodes, axis);
        rt
    }

    fn refresh_temp_positions(&mut self, nodes: &[SimNode], axis: Axis) {
        let n = self.node_count;
        for key in 0..self.temp_pos.len() {
            let v = if key < n {
                match axis {
                    Axis::Horizontal => nodes[key].center_x(),
                    Axis::Vertical => nodes[key].center_y(),
                }
            } else {
                let dummy_idx = key - n;
                let first = self.dummy_to_nodes[dummy_idx]
                    .first()
                    .copied()
                    .unwrap_or(0)
                    .min(n.saturating_sub(1));
                match axis {
                    Axis::Horizontal => nodes[first].center_x(),
                    Axis::Vertical => nodes[first].center_y(),
                }
            };
            self.temp_pos[key] = v;
        }
    }

    fn shuffle_tail_third(&mut self, rng: &mut XorShift64Star) {
        let len = self.nodes_in_relative.len();
        if len <= 1 {
            return;
        }
        let start = (2 * len) / 3;
        for i in (start..len).rev() {
            let j = rng.next_usize(i + 1);
            self.nodes_in_relative.swap(i, j);
        }
    }

    fn apply_relative_relaxation(&mut self, disps: &mut [(f64, f64)], max_d: f64, axis: Axis) {
        let n = self.node_count;

        for &key in &self.nodes_in_relative {
            if self.fixed_nodes.contains(&key) {
                continue;
            }

            let mut displacement = if key < n {
                match axis {
                    Axis::Horizontal => disps[key].0,
                    Axis::Vertical => disps[key].1,
                }
            } else {
                let dummy_idx = key - n;
                let first = self.dummy_to_nodes[dummy_idx]
                    .first()
                    .copied()
                    .unwrap_or(0)
                    .min(n.saturating_sub(1));
                match axis {
                    Axis::Horizontal => disps[first].0,
                    Axis::Vertical => disps[first].1,
                }
            };

            for adj in &self.rel_map[key] {
                match (*adj, axis) {
                    (AxisRelAdj::Right { node, gap }, Axis::Horizontal) => {
                        let diff = (self.temp_pos[node] - self.temp_pos[key]) - displacement;
                        if diff < gap {
                            displacement -= gap - diff;
                        }
                    }
                    (AxisRelAdj::Left { node, gap }, Axis::Horizontal) => {
                        let diff = (self.temp_pos[key] - self.temp_pos[node]) + displacement;
                        if diff < gap {
                            displacement += gap - diff;
                        }
                    }
                    (AxisRelAdj::Bottom { node, gap }, Axis::Vertical) => {
                        let diff = (self.temp_pos[node] - self.temp_pos[key]) - displacement;
                        if diff < gap {
                            displacement -= gap - diff;
                        }
                    }
                    (AxisRelAdj::Top { node, gap }, Axis::Vertical) => {
                        let diff = (self.temp_pos[key] - self.temp_pos[node]) + displacement;
                        if diff < gap {
                            displacement += gap - diff;
                        }
                    }
                    _ => {}
                }
            }

            if max_d.is_finite() && max_d > 0.0 && displacement.abs() > max_d {
                displacement = max_d * displacement.signum();
            }

            self.temp_pos[key] += displacement;

            if key < n {
                match axis {
                    Axis::Horizontal => disps[key].0 = displacement,
                    Axis::Vertical => disps[key].1 = displacement,
                }
            } else {
                let dummy_idx = key - n;
                for &idx in &self.dummy_to_nodes[dummy_idx] {
                    if idx >= disps.len() {
                        continue;
                    }
                    match axis {
                        Axis::Horizontal => disps[idx].0 = displacement,
                        Axis::Vertical => disps[idx].1 = displacement,
                    }
                }
            }
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
    id_to_idx: FxHashMap<String, usize>,
    compound_parent: FxHashMap<String, Option<String>>,
}

impl SimGraph {
    const DEFAULT_EDGE_LENGTH: f64 = 50.0;
    const DEFAULT_SPRING_STRENGTH: f64 = 0.45;
    const DEFAULT_REPULSION_STRENGTH: f64 = 4500.0;
    const DEFAULT_GRAVITY_STRENGTH: f64 = 0.25; // cytoscape-fcose default `gravity`
    const DEFAULT_GRAVITY_RANGE_FACTOR: f64 = 3.8; // cytoscape-fcose default `gravityRange`
    const DEFAULT_COOLING_FACTOR_INCREMENTAL: f64 = 0.3; // layout-base `FDLayoutConstants.DEFAULT_COOLING_FACTOR_INCREMENTAL`
    const FINAL_TEMPERATURE: f64 = 0.04; // cose-base `CoSELayout.initSpringEmbedder()`
    const GRID_CALCULATION_CHECK_PERIOD: usize = 10; // layout-base `FDLayoutConstants.GRID_CALCULATION_CHECK_PERIOD`

    const MAX_ITERATIONS: usize = 2500;
    const CONVERGENCE_CHECK_PERIOD: usize = 100;
    #[allow(dead_code)]
    const MAX_NODE_DISPLACEMENT: f64 = 300.0;
    const MAX_NODE_DISPLACEMENT_INCREMENTAL: f64 = 100.0; // layout-base `FDLayoutConstants.MAX_NODE_DISPLACEMENT_INCREMENTAL`
    fn from_graph(graph: &Graph) -> Self {
        let mut nodes: Vec<SimNode> = Vec::with_capacity(graph.nodes.len());
        let mut id_to_idx: FxHashMap<String, usize> = FxHashMap::default();
        id_to_idx.reserve(graph.nodes.len().saturating_mul(2));

        for (idx, n) in graph.nodes.iter().enumerate() {
            let w = n.width.max(1.0);
            let h = n.height.max(1.0);
            nodes.push(SimNode {
                id: n.id.clone(),
                parent: n.parent.clone(),
                width: w,
                height: h,
                left: n.x - w / 2.0,
                top: n.y - h / 2.0,
                spring_fx: 0.0,
                spring_fy: 0.0,
                repulsion_fx: 0.0,
                repulsion_fy: 0.0,
                surrounding: Vec::new(),
                grid_start_x: 0,
                grid_finish_x: 0,
                grid_start_y: 0,
                grid_finish_y: 0,
            });
            id_to_idx.insert(n.id.clone(), idx);
        }

        let mut compound_parent: FxHashMap<String, Option<String>> = FxHashMap::default();
        compound_parent.reserve(graph.compounds.len().saturating_mul(2));
        for c in &graph.compounds {
            compound_parent.insert(c.id.clone(), c.parent.clone());
        }

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

            let ideal = if e.ideal_length.is_finite() && e.ideal_length > 0.0 {
                e.ideal_length
            } else {
                Self::DEFAULT_EDGE_LENGTH
            };
            let elasticity = if e.elasticity.is_finite() && e.elasticity > 0.0 {
                e.elasticity
            } else {
                Self::DEFAULT_SPRING_STRENGTH
            };
            edges.push(SimEdge {
                a,
                b,
                a_anchor: e.source_anchor,
                b_anchor: e.target_anchor,
                ideal_length: ideal.max(1.0),
                elasticity,
            });
        }

        Self {
            nodes,
            edges,
            id_to_idx,
            compound_parent,
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

    fn run_spring_embedder(
        &mut self,
        constraints: &Constraints,
        opts: &FcoseOptions,
        mut timings: Option<&mut FcoseSpringTimings>,
    ) -> SpringStats {
        if self.nodes.is_empty() {
            return SpringStats::default();
        }

        let timing_enabled = timings.is_some();

        let random_seed = opts.random_seed;
        let mut rng = XorShift64Star::new(random_seed);

        let opts_prep_start = timing_enabled.then(std::time::Instant::now);
        // layout-base/CoSE uses a *global* `DEFAULT_EDGE_LENGTH` for multiple heuristics (minimum
        // repulsion distance, overlap separation buffer, repulsion grid range, convergence
        // thresholds, etc.). In upstream Cytoscape FCoSE this value is derived from the
        // `idealEdgeLength` option (before per-edge nesting/smart adjustments).
        let default_edge_length = opts
            .default_edge_length
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or_else(|| {
                if self.edges.is_empty() {
                    Self::DEFAULT_EDGE_LENGTH
                } else {
                    let sum: f64 = self.edges.iter().map(|e| e.ideal_length).sum();
                    (sum / (self.edges.len() as f64)).max(1.0)
                }
            });
        let half_default_edge_length = default_edge_length / 2.0;
        // CoSE updates `MIN_REPULSION_DIST` based on the effective `DEFAULT_EDGE_LENGTH` when
        // `idealEdgeLength` is set. For Mermaid Architecture this is always set (as a function),
        // so we scale the minimum repulsion distance with the average ideal length.
        let min_repulsion_dist = (default_edge_length / 10.0).max(0.0005);
        if let (Some(t), Some(s)) = (timings.as_deref_mut(), opts_prep_start) {
            t.opts_prep = s.elapsed();
        }

        // FCoSE performs a spectral initialization when `randomize=true` (Mermaid defaults to
        // `randomize: true`). The upstream JS implementation relies on `Math.random`; in Rust we
        // make this explicit and deterministic via `random_seed`.
        let spectral_start = timing_enabled.then(std::time::Instant::now);
        let spectral_applied = spectral::apply_spectral_start_positions(
            &mut self.nodes,
            &self.edges,
            &self.compound_parent,
            &mut rng,
        );
        if let (Some(t), Some(s)) = (timings.as_deref_mut(), spectral_start) {
            t.spectral = s.elapsed();
        }

        let gravity_constant = Self::DEFAULT_GRAVITY_STRENGTH;

        // Match `cose-base` repulsion cutoff (`CoSELayout.calcRepulsionRange()`):
        //
        // `repulsionRange = 2 * (level + 1) * idealEdgeLength`
        //
        // `cose-base` initializes `level=0`, so this collapses to `2 * DEFAULT_EDGE_LENGTH`.
        // Keeping repulsion unbounded tends to over-spread disconnected or sparse graphs (notably
        // the "no edges" fixtures), which cascades into parity-root `viewBox` / `max-width` drift.
        let repulsion_range = (2.0 * default_edge_length).max(1.0);

        let estimated_size = self.estimated_size();
        let gravity_range = estimated_size * Self::DEFAULT_GRAVITY_RANGE_FACTOR;

        // layout-base uses the FR-grid repulsion variant by default, which caches each node's
        // surrounding set and refreshes it every `GRID_CALCULATION_CHECK_PERIOD` iterations.
        let mut repulsion_grid: Option<RepulsionGrid> = None;

        let root_compound_start = timing_enabled.then(std::time::Instant::now);
        // Precompute root compound membership for each node.
        let node_root_compound: Vec<Option<String>> = self
            .nodes
            .iter()
            .map(|n| {
                let mut cur = n.parent.as_deref()?;
                while let Some(Some(p)) = self.compound_parent.get(cur) {
                    cur = p.as_str();
                }
                Some(cur.to_string())
            })
            .collect();
        let mut root_to_nodes: std::collections::BTreeMap<String, Vec<usize>> =
            std::collections::BTreeMap::new();
        for (idx, root) in node_root_compound.iter().enumerate() {
            if let Some(r) = root {
                root_to_nodes.entry(r.clone()).or_default().push(idx);
            }
        }
        let compound_padding = opts.compound_padding.unwrap_or(0.0).max(0.0);
        if let (Some(t), Some(s)) = (timings.as_deref_mut(), root_compound_start) {
            t.root_compound = s.elapsed();
        }

        // Fallback for degenerate cases where spectral is skipped (e.g. very small graphs).
        if self.edges.is_empty() && !spectral_applied {
            let collapse_start = timing_enabled.then(std::time::Instant::now);
            self.collapse_start_positions(default_edge_length, &mut rng);
            if let (Some(t), Some(s)) = (timings.as_deref_mut(), collapse_start) {
                t.collapse_start_positions = s.elapsed();
            }
        }

        // Upstream `cose-base` runs a dedicated constraint handler before the spring embedder.
        // This can rotate/reflect the draft layout and enforce alignment/relative-placement
        // constraints in position space, which strongly affects overall orientation and the
        // parity-root root viewport.
        if !(constraints.align_horizontal.is_empty()
            && constraints.align_vertical.is_empty()
            && constraints.relative.is_empty())
        {
            let pre_constraints_start = timing_enabled.then(std::time::Instant::now);
            handle_constraints_pre_layout(&mut self.nodes, constraints);
            if let (Some(t), Some(s)) = (timings.as_deref_mut(), pre_constraints_start) {
                t.pre_constraints = s.elapsed();
            }
        }

        let constraint_rt_start = timing_enabled.then(std::time::Instant::now);
        let mut constraint_rt = ConstraintRuntime::new(&self.nodes, constraints);
        if let (Some(t), Some(s)) = (timings.as_deref_mut(), constraint_rt_start) {
            t.constraint_rt = s.elapsed();
        }

        let n = self.nodes.len() as f64;
        let displacement_threshold_per_node = (3.0 * default_edge_length) / 100.0;
        let total_displacement_threshold = displacement_threshold_per_node * n;

        // cytoscape-fcose postprocessing (`cose.js`) forces CoSE incremental mode on by setting
        // `LayoutConstants.DEFAULT_INCREMENTAL = true`. This means we start with the incremental
        // cooling factor and max displacement values, even when `randomize=true`.
        //
        // This is a major contributor to parity-root `viewBox/max-width` stability for sparse
        // graphs (notably the Architecture fixtures).
        let initial_cooling_factor = Self::DEFAULT_COOLING_FACTOR_INCREMENTAL;
        let mut cooling_factor = initial_cooling_factor;
        let max_node_displacement = Self::MAX_NODE_DISPLACEMENT_INCREMENTAL;
        let max_iterations = Self::MAX_ITERATIONS.max(self.nodes.len() * 5);
        let max_cooling_cycle = (max_iterations as f64) / (Self::CONVERGENCE_CHECK_PERIOD as f64);
        let final_temperature = Self::FINAL_TEMPERATURE;
        let mut cooling_cycle = 0.0f64;

        let mut total_iterations = 0usize;
        let mut old_total_displacement = 0.0f64;
        let mut last_total_displacement = 0.0f64;

        let iterations_start = timing_enabled.then(std::time::Instant::now);
        let small_repulsion = self.nodes.len() <= 64;
        let mut processed: Vec<bool> = vec![false; self.nodes.len()];
        let mut disps: Vec<(f64, f64)> = vec![(0.0, 0.0); self.nodes.len()];
        loop {
            total_iterations += 1;
            if total_iterations == max_iterations {
                break;
            }

            if total_iterations.is_multiple_of(Self::CONVERGENCE_CHECK_PERIOD) {
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

                // In Cytoscape CoSE/FCoSE, the spring force is scaled by the effective
                // `edgeElasticity` option. Mermaid Architecture sets this to `0.45` for
                // same-parent edges and `0.001` for edges that cross a group boundary.
                let spring_force = e.elasticity * (len - e.ideal_length.max(1.0));
                let sfx = spring_force * (lx / len);
                let sfy = spring_force * (ly / len);
                self.nodes[a].spring_fx += sfx;
                self.nodes[a].spring_fy += sfy;
                self.nodes[b].spring_fx -= sfx;
                self.nodes[b].spring_fy -= sfy;
            }

            // Repulsion forces (layout-base FR grid variant, with cached surrounding lists).
            //
            // Upstream refreshes the grid + surrounding lists when `totalIterations % 10 == 1`,
            // then reuses those "stale" surrounding lists for the next 9 iterations.
            let refresh_surrounding = (total_iterations % Self::GRID_CALCULATION_CHECK_PERIOD) == 1;
            if refresh_surrounding {
                if small_repulsion {
                    for i in 0..self.nodes.len() {
                        self.nodes[i].surrounding.clear();
                        for j in (i + 1)..self.nodes.len() {
                            let dx = (self.nodes[i].center_x() - self.nodes[j].center_x()).abs()
                                - (self.nodes[i].half_w() + self.nodes[j].half_w());
                            let dy = (self.nodes[i].center_y() - self.nodes[j].center_y()).abs()
                                - (self.nodes[i].half_h() + self.nodes[j].half_h());
                            if dx <= repulsion_range && dy <= repulsion_range {
                                self.nodes[i].surrounding.push(j);
                            }
                        }
                    }
                    repulsion_grid = None;
                } else {
                    repulsion_grid = RepulsionGrid::build(&self.nodes, repulsion_range);
                }
            }

            if repulsion_range.is_finite() && repulsion_range > 0.0 {
                processed.fill(false);
                for i in 0..self.nodes.len() {
                    if refresh_surrounding && !small_repulsion {
                        if let Some(g) = &repulsion_grid {
                            g.refresh_node_surrounding(
                                i,
                                &mut self.nodes,
                                &processed,
                                repulsion_range,
                            );
                        } else {
                            self.nodes[i].surrounding.clear();
                        }
                    }

                    let surrounding = std::mem::take(&mut self.nodes[i].surrounding);
                    for &j in &surrounding {
                        if i == j {
                            continue;
                        }
                        if j >= self.nodes.len() {
                            continue;
                        }
                        if i >= j {
                            continue;
                        }
                        let (rfx, rfy) = calc_repulsion_force(
                            &self.nodes[i],
                            &self.nodes[j],
                            min_repulsion_dist,
                            half_default_edge_length,
                        );
                        let (left, right) = self.nodes.split_at_mut(j);
                        let a = &mut left[i];
                        let b = &mut right[0];
                        a.repulsion_fx += rfx;
                        a.repulsion_fy += rfy;
                        b.repulsion_fx -= rfx;
                        b.repulsion_fy -= rfy;
                    }
                    self.nodes[i].surrounding = surrounding;
                    processed[i] = true;
                }
            } else {
                // Fallback: unbounded repulsion (all pairs).
                for i in 0..self.nodes.len() {
                    for j in (i + 1)..self.nodes.len() {
                        let (rfx, rfy) = calc_repulsion_force(
                            &self.nodes[i],
                            &self.nodes[j],
                            min_repulsion_dist,
                            half_default_edge_length,
                        );
                        self.nodes[i].repulsion_fx += rfx;
                        self.nodes[i].repulsion_fy += rfy;
                        self.nodes[j].repulsion_fx -= rfx;
                        self.nodes[j].repulsion_fy -= rfy;
                    }
                }
            }

            // Gravity forces (approx): apply only when the current distance exceeds the gravity
            // range. In upstream `cose-base` this runs every tick, but usually only affects nodes
            // that drift far from the component center.
            if gravity_range.is_finite() && gravity_range > 0.0 {
                let (cx, cy) = self.bounding_box_center().unwrap_or((0.0, 0.0));
                for n in &mut self.nodes {
                    let dx = n.center_x() - cx;
                    let dy = n.center_y() - cy;
                    let abs_dx = dx.abs() + n.half_w();
                    let abs_dy = dy.abs() + n.half_h();
                    if abs_dx > gravity_range || abs_dy > gravity_range {
                        n.spring_fx += -gravity_constant * dx;
                        n.spring_fy += -gravity_constant * dy;
                    }
                }
            }

            // Move nodes (with constraints applied to displacements).
            //
            // Upstream `cose-base` computes displacements from forces, then applies constraint
            // handling that *updates those displacements* (rather than hard-projecting node
            // positions after the move). Hard projection tends to over-separate constrained nodes
            // and can noticeably inflate root viewBox/max-width in parity-root mode.
            let max_d = cooling_factor * max_node_displacement;
            for (idx, n) in self.nodes.iter().enumerate() {
                let mut mdx = cooling_factor * (n.spring_fx + n.repulsion_fx);
                let mut mdy = cooling_factor * (n.spring_fy + n.repulsion_fy);
                if mdx.abs() > max_d {
                    mdx = max_d * mdx.signum();
                }
                if mdy.abs() > max_d {
                    mdy = max_d * mdy.signum();
                }
                if let Some(slot) = disps.get_mut(idx) {
                    *slot = (mdx, mdy);
                }
            }

            if let Some(rt) = constraint_rt.as_mut() {
                rt.update_displacements(
                    &self.nodes,
                    constraints,
                    &mut disps,
                    total_iterations,
                    max_d,
                    &mut rng,
                );
            } else {
                apply_constraints_to_displacements(&self.nodes, constraints, &mut disps, max_d);
            }
            apply_root_compound_overlap_separation_to_displacements(
                &self.nodes,
                &root_to_nodes,
                compound_padding,
                half_default_edge_length,
                max_d,
                &mut disps,
            );

            for (idx, n) in self.nodes.iter_mut().enumerate() {
                let (mdx, mdy) = disps.get(idx).copied().unwrap_or((0.0, 0.0));
                n.move_by(mdx, mdy);
                total_displacement += mdx.abs() + mdy.abs();

                n.spring_fx = 0.0;
                n.spring_fy = 0.0;
                n.repulsion_fx = 0.0;
                n.repulsion_fy = 0.0;
            }

            last_total_displacement = total_displacement;
        }
        if let (Some(t), Some(s)) = (timings.as_deref_mut(), iterations_start) {
            t.iterations = s.elapsed();
        }

        SpringStats {
            iterations: total_iterations,
            spectral_applied,
        }
    }

    fn estimated_size(&self) -> f64 {
        // layout-base `LGraph.calcEstimatedSize()` for a flat graph:
        // - each node estimated size is (w + h) / 2
        // - graph estimated size is sum / sqrt(n)
        let n = self.nodes.len() as f64;
        if n <= 0.0 {
            return 0.0;
        }
        let sum: f64 = self.nodes.iter().map(|n| (n.width + n.height) / 2.0).sum();
        (sum / n.sqrt()).max(1.0)
    }

    fn collapse_start_positions(&mut self, scale: f64, rng: &mut XorShift64Star) {
        if self.nodes.len() <= 2 {
            return;
        }
        // Keep starts close to the origin (we relocate later).
        let jitter = (0.01 * scale).max(0.01);
        for n in self.nodes.iter_mut() {
            let jx = rng.next_f64_signed() * jitter;
            let jy = rng.next_f64_signed() * jitter;
            n.left = jx;
            n.top = jy;
        }
    }
}

fn handle_constraints_pre_layout(nodes: &mut [SimNode], c: &Constraints) {
    if nodes.is_empty() {
        return;
    }

    let mut x: Vec<f64> = nodes.iter().map(|n| n.center_x()).collect();
    let mut y: Vec<f64> = nodes.iter().map(|n| n.center_y()).collect();

    // Match `cose-base` ConstraintHandler: rotate/reflect the draft layout using an orthogonal
    // Procrustes transform derived from alignment constraints, then vote-based reflection for
    // relative placement directionality.
    if !c.align_vertical.is_empty() || !c.align_horizontal.is_empty() {
        if let Some(t) = procrustes_transform_for_alignments(&x, &y, c) {
            let tt = t.transpose();
            for i in 0..x.len() {
                let v = na::Vector2::new(x[i], y[i]);
                let r = tt * v;
                x[i] = r.x;
                y[i] = r.y;
            }
            if !c.relative.is_empty() {
                apply_reflection_for_relative_placement(&mut x, &mut y, &c.relative);
            }
        }
    } else if !c.relative.is_empty() {
        // `ConstraintHandler` also applies a relative-only transform when there are no alignment
        // constraints: it finds the largest weakly-connected component in the relative-placement
        // DAG and uses it to derive a Procrustes rotation (plus a reflection vote).
        //
        // This has an outsized effect on overall orientation and thus the parity-root viewport.
        handle_relative_only_transform(&mut x, &mut y, &c.relative);
    }

    // Enforce alignment constraints in position space.
    for group in &c.align_vertical {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        for &idx in group {
            sum += x[idx];
        }
        let target = sum / (group.len() as f64);
        for &idx in group {
            x[idx] = target;
        }
    }
    for group in &c.align_horizontal {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        for &idx in group {
            sum += y[idx];
        }
        let target = sum / (group.len() as f64);
        for &idx in group {
            y[idx] = target;
        }
    }

    // Enforce relative placement constraints in position space.
    if !c.relative.is_empty() {
        enforce_relative_placement(&mut x, &mut y, c);
    }

    for (i, n) in nodes.iter_mut().enumerate() {
        n.left = x[i] - n.width / 2.0;
        n.top = y[i] - n.height / 2.0;
    }
}

fn handle_relative_only_transform(x: &mut [f64], y: &mut [f64], rel: &[RelConstraint]) {
    use std::collections::VecDeque;

    #[derive(Debug, Clone, Copy)]
    struct Edge {
        id: usize,
        gap: f64,
    }

    let n_total = x.len().min(y.len());
    if n_total == 0 {
        return;
    }

    let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); n_total];
    let mut present: Vec<bool> = vec![false; n_total];
    for r in rel {
        let (a, b) = if let (Some(left), Some(right)) = (r.left, r.right) {
            (left, right)
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
            (top, bottom)
        } else {
            continue;
        };
        if a >= n_total || b >= n_total {
            continue;
        }
        undirected[a].push(b);
        undirected[b].push(a);
        present[a] = true;
        present[b] = true;
    }

    let present_count = present.iter().filter(|&&v| v).count();
    if present_count == 0 {
        return;
    }

    fn find_components(g: &[Vec<usize>], present: &[bool], node_count: usize) -> Vec<Vec<usize>> {
        let mut visited: Vec<bool> = vec![false; node_count];
        let mut out: Vec<Vec<usize>> = Vec::new();
        for start in 0..node_count {
            if !present[start] || visited[start] {
                continue;
            }

            let mut q: VecDeque<usize> = VecDeque::new();
            let mut comp: Vec<usize> = Vec::new();
            visited[start] = true;
            q.push_back(start);
            while let Some(cur) = q.pop_front() {
                comp.push(cur);
                for &n in &g[cur] {
                    if n >= node_count {
                        continue;
                    }
                    if !visited[n] {
                        visited[n] = true;
                        q.push_back(n);
                    }
                }
            }
            out.push(comp);
        }
        out
    }

    fn find_appropriate_positions(
        nodes_sorted: &[usize],
        in_comp: &[bool],
        graph: &[Vec<Edge>],
        axis: Axis,
        x: &[f64],
        y: &[f64],
    ) -> Vec<f64> {
        let node_count = x.len().min(y.len());
        let mut indeg: Vec<usize> = vec![0; node_count];
        for &src in nodes_sorted {
            if src >= node_count {
                continue;
            }
            for e in &graph[src] {
                if e.id >= node_count || !in_comp[e.id] {
                    continue;
                }
                indeg[e.id] = indeg[e.id].saturating_add(1);
            }
        }

        let mut pos: Vec<f64> = vec![f64::NEG_INFINITY; node_count];
        let mut q: VecDeque<usize> = VecDeque::new();
        for &node in nodes_sorted {
            if node >= node_count {
                continue;
            }
            if indeg[node] == 0 {
                q.push_back(node);
                pos[node] = match axis {
                    Axis::Horizontal => x[node],
                    Axis::Vertical => y[node],
                };
            }
        }

        while let Some(cur) = q.pop_front() {
            let cur_pos = pos.get(cur).copied().unwrap_or(f64::NEG_INFINITY);
            for e in graph.get(cur).into_iter().flatten() {
                if e.id >= node_count || !in_comp[e.id] {
                    continue;
                }
                let next_pos = cur_pos + e.gap;
                if pos[e.id] < next_pos {
                    pos[e.id] = next_pos;
                }
                if let Some(v) = indeg.get_mut(e.id) {
                    *v = v.saturating_sub(1);
                    if *v == 0 {
                        q.push_back(e.id);
                    }
                }
            }
        }

        pos
    }

    let components = find_components(&undirected, &present, n_total);
    if components.is_empty() {
        return;
    }

    let mut largest_idx = 0usize;
    let mut largest_sz = 0usize;
    for (i, c) in components.iter().enumerate() {
        if c.len() > largest_sz {
            largest_sz = c.len();
            largest_idx = i;
        }
    }

    if largest_sz * 2 < present_count {
        apply_reflection_for_relative_placement(x, y, rel);
        return;
    }

    let largest = &components[largest_idx];
    let mut in_comp: Vec<bool> = vec![false; n_total];
    for &idx in largest {
        if idx < n_total {
            in_comp[idx] = true;
        }
    }

    let mut nodes_sorted: Vec<usize> = largest.clone();
    nodes_sorted.sort_unstable();

    // Apply reflection votes based only on edges inside the dominant component (upstream behavior).
    let mut in_comp_constraints: Vec<RelConstraint> = Vec::new();
    let mut dag_h: Vec<Vec<Edge>> = vec![Vec::new(); n_total];
    let mut dag_v: Vec<Vec<Edge>> = vec![Vec::new(); n_total];
    for r in rel {
        if let (Some(left), Some(right)) = (r.left, r.right) {
            if left < n_total && right < n_total && in_comp[left] && in_comp[right] {
                dag_h[left].push(Edge {
                    id: right,
                    gap: r.gap,
                });
                in_comp_constraints.push(*r);
            }
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
            if top < n_total && bottom < n_total && in_comp[top] && in_comp[bottom] {
                dag_v[top].push(Edge {
                    id: bottom,
                    gap: r.gap,
                });
                in_comp_constraints.push(*r);
            }
        }
    }
    apply_reflection_for_relative_placement(x, y, &in_comp_constraints);

    // Build axis DAGs and compute an "appropriate" coordinate per node using a topological
    // relaxation similar to `findAppropriatePositionForRelativePlacement`.
    let pos_h = find_appropriate_positions(&nodes_sorted, &in_comp, &dag_h, Axis::Horizontal, x, y);
    let pos_v = find_appropriate_positions(&nodes_sorted, &in_comp, &dag_v, Axis::Vertical, x, y);

    let mut source: Vec<na::Vector2<f64>> = Vec::with_capacity(largest.len());
    let mut target: Vec<na::Vector2<f64>> = Vec::with_capacity(largest.len());
    for &idx in largest {
        if idx >= n_total {
            continue;
        }
        source.push(na::Vector2::new(x[idx], y[idx]));
        let tx = pos_h.get(idx).copied().unwrap_or(x[idx]);
        let ty = pos_v.get(idx).copied().unwrap_or(y[idx]);
        target.push(na::Vector2::new(tx, ty));
    }

    if let Some(t) = procrustes_transform_from_pairs(&source, &target) {
        let tt = t.transpose();
        for i in 0..x.len().min(y.len()) {
            let v = na::Vector2::new(x[i], y[i]);
            let r = tt * v;
            x[i] = r.x;
            y[i] = r.y;
        }
    }
}

fn procrustes_transform_for_alignments(
    x: &[f64],
    y: &[f64],
    c: &Constraints,
) -> Option<na::Matrix2<f64>> {
    let mut source: Vec<na::Vector2<f64>> = Vec::new();
    let mut target: Vec<na::Vector2<f64>> = Vec::new();

    for group in &c.align_vertical {
        if group.is_empty() {
            continue;
        }
        let mut sum_x = 0.0;
        for &idx in group {
            sum_x += x[idx];
        }
        let x_pos = sum_x / (group.len() as f64);
        for &idx in group {
            source.push(na::Vector2::new(x[idx], y[idx]));
            target.push(na::Vector2::new(x_pos, y[idx]));
        }
    }

    for group in &c.align_horizontal {
        if group.is_empty() {
            continue;
        }
        let mut sum_y = 0.0;
        for &idx in group {
            sum_y += y[idx];
        }
        let y_pos = sum_y / (group.len() as f64);
        for &idx in group {
            source.push(na::Vector2::new(x[idx], y[idx]));
            target.push(na::Vector2::new(x[idx], y_pos));
        }
    }

    if source.len() <= 1 || target.len() != source.len() {
        return None;
    }

    procrustes_transform_from_pairs(&source, &target)
}

fn procrustes_transform_from_pairs(
    source: &[na::Vector2<f64>],
    target: &[na::Vector2<f64>],
) -> Option<na::Matrix2<f64>> {
    if source.len() <= 1 || target.len() != source.len() {
        return None;
    }

    let mut mean_s = na::Vector2::new(0.0, 0.0);
    let mut mean_t = na::Vector2::new(0.0, 0.0);
    for (s, t) in source.iter().zip(target.iter()) {
        mean_s += s;
        mean_t += t;
    }
    let inv_n = 1.0 / (source.len() as f64);
    mean_s *= inv_n;
    mean_t *= inv_n;

    // `ConstraintHandler` forms `tempMatrix = A'B` where A is target, B is source (mean-centered).
    let mut m = na::Matrix2::zeros();
    for (s, t) in source.iter().zip(target.iter()) {
        let sc = s - mean_s;
        let tc = t - mean_t;
        m += tc * sc.transpose();
    }

    if !(m[(0, 0)].is_finite()
        && m[(0, 1)].is_finite()
        && m[(1, 0)].is_finite()
        && m[(1, 1)].is_finite())
    {
        return None;
    }

    let a = m[(0, 0)];
    let b = m[(0, 1)];
    let c = m[(1, 0)];
    let d = m[(1, 1)];

    let x1 = a + d;
    let y1 = c - b;
    let s1 = x1.hypot(y1);

    let x2 = a - d;
    let y2 = b + c;
    let s2 = x2.hypot(y2);

    if !(s1.is_finite() && s2.is_finite()) {
        return None;
    }
    if s1 < 1e-12 && s2 < 1e-12 {
        return None;
    }

    if s1 >= s2 {
        let inv = 1.0 / s1.max(1e-12);
        let cos = x1 * inv;
        let sin = y1 * inv;
        Some(na::Matrix2::new(cos, -sin, sin, cos))
    } else {
        let inv = 1.0 / s2.max(1e-12);
        let cos = x2 * inv;
        let sin = y2 * inv;
        Some(na::Matrix2::new(cos, sin, sin, -cos))
    }
}

fn apply_reflection_for_relative_placement(x: &mut [f64], y: &mut [f64], rel: &[RelConstraint]) {
    let mut reflect_on_y = 0;
    let mut not_reflect_on_y = 0;
    let mut reflect_on_x = 0;
    let mut not_reflect_on_x = 0;

    for r in rel {
        if let (Some(left), Some(right)) = (r.left, r.right) {
            if x[left] - x[right] >= 0.0 {
                reflect_on_y += 1;
            } else {
                not_reflect_on_y += 1;
            }
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
            if y[top] - y[bottom] >= 0.0 {
                reflect_on_x += 1;
            } else {
                not_reflect_on_x += 1;
            }
        }
    }

    if reflect_on_y > not_reflect_on_y && reflect_on_x > not_reflect_on_x {
        for i in 0..x.len() {
            x[i] = -x[i];
            y[i] = -y[i];
        }
    } else if reflect_on_y > not_reflect_on_y {
        for v in x.iter_mut() {
            *v = -*v;
        }
    } else if reflect_on_x > not_reflect_on_x {
        for v in y.iter_mut() {
            *v = -*v;
        }
    }
}

fn enforce_relative_placement(x: &mut [f64], y: &mut [f64], c: &Constraints) {
    #[derive(Debug, Clone, Copy)]
    struct Neighbor {
        id: usize,
        gap: f64,
    }

    let n = x.len().min(y.len());
    if n == 0 {
        return;
    }

    fn enforce_relative_placement_no_align_small(
        x: &mut [f64],
        y: &mut [f64],
        rel: &[RelConstraint],
        n: usize,
    ) {
        use std::collections::VecDeque;

        fn build_axis_dag_keys(
            axis: Axis,
            rel: &[RelConstraint],
            n: usize,
        ) -> (Vec<usize>, Vec<Vec<Neighbor>>) {
            let mut keys: Vec<usize> = Vec::new();
            let mut seen: Vec<bool> = vec![false; n];
            let mut dag: Vec<Vec<Neighbor>> = vec![Vec::new(); n];

            for r in rel {
                match axis {
                    Axis::Horizontal => {
                        let (Some(left), Some(right)) = (r.left, r.right) else {
                            continue;
                        };
                        if left >= n || right >= n {
                            continue;
                        }
                        if !seen[left] {
                            seen[left] = true;
                            keys.push(left);
                        }
                        if !seen[right] {
                            seen[right] = true;
                            keys.push(right);
                        }
                        dag[left].push(Neighbor {
                            id: right,
                            gap: r.gap,
                        });
                    }
                    Axis::Vertical => {
                        let (Some(top), Some(bottom)) = (r.top, r.bottom) else {
                            continue;
                        };
                        if top >= n || bottom >= n {
                            continue;
                        }
                        if !seen[top] {
                            seen[top] = true;
                            keys.push(top);
                        }
                        if !seen[bottom] {
                            seen[bottom] = true;
                            keys.push(bottom);
                        }
                        dag[top].push(Neighbor {
                            id: bottom,
                            gap: r.gap,
                        });
                    }
                }
            }

            (keys, dag)
        }

        fn build_rev(keys: &[usize], dag: &[Vec<Neighbor>], n: usize) -> Vec<Vec<Neighbor>> {
            let mut rev: Vec<Vec<Neighbor>> = vec![Vec::new(); n];
            for &src in keys {
                if src >= n {
                    continue;
                }
                for e in &dag[src] {
                    if e.id >= n {
                        continue;
                    }
                    rev[e.id].push(Neighbor {
                        id: src,
                        gap: e.gap,
                    });
                }
            }
            rev
        }

        fn pos_before(key: usize, axis: Axis, x: &[f64], y: &[f64]) -> f64 {
            match axis {
                Axis::Horizontal => x[key],
                Axis::Vertical => y[key],
            }
        }

        fn component_sources(
            keys: &[usize],
            dag: &[Vec<Neighbor>],
            rev: &[Vec<Neighbor>],
            n: usize,
        ) -> Vec<Vec<usize>> {
            let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); n];
            for &src in keys {
                if src >= n {
                    continue;
                }
                for e in &dag[src] {
                    if e.id >= n {
                        continue;
                    }
                    undirected[src].push(e.id);
                    undirected[e.id].push(src);
                }
            }

            let mut visited: Vec<bool> = vec![false; n];
            let mut out: Vec<Vec<usize>> = Vec::new();
            for &start in keys {
                if start >= n || visited[start] {
                    continue;
                }
                let mut q: VecDeque<usize> = VecDeque::new();
                let mut comp: Vec<usize> = Vec::new();
                visited[start] = true;
                q.push_back(start);
                while let Some(cur) = q.pop_front() {
                    comp.push(cur);
                    for &next in &undirected[cur] {
                        if next < n && !visited[next] {
                            visited[next] = true;
                            q.push_back(next);
                        }
                    }
                }

                let mut sources: Vec<usize> = Vec::new();
                for &node in &comp {
                    if node < n && rev[node].is_empty() {
                        sources.push(node);
                    }
                }
                out.push(sources);
            }
            out
        }

        fn find_appropriate_positions(
            keys: &[usize],
            dag: &[Vec<Neighbor>],
            axis: Axis,
            n: usize,
            x: &[f64],
            y: &[f64],
            sources: &[Vec<usize>],
        ) -> Vec<f64> {
            let mut in_deg: Vec<usize> = vec![0; n];
            for &src in keys {
                for e in &dag[src] {
                    in_deg[e.id] = in_deg[e.id].saturating_add(1);
                }
            }

            let mut position: Vec<f64> = vec![0.0; n];
            let mut past_bits: Vec<u64> = vec![0; n];
            let mut past_order: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut q: VecDeque<usize> = VecDeque::new();

            for &k in keys {
                position[k] = f64::NEG_INFINITY;
                if in_deg[k] == 0 {
                    q.push_back(k);
                }
                past_bits[k] = 1u64 << (k as u64);
                past_order[k] = vec![k];
            }

            for component in sources {
                if component.is_empty() {
                    continue;
                }
                let mut sum = 0.0;
                for &node in component {
                    sum += pos_before(node, axis, x, y);
                }
                let avg = sum / (component.len() as f64);
                for &node in component {
                    position[node] = avg;
                }
            }

            while let Some(cur) = q.pop_front() {
                let cur_pos = position[cur];
                for neigh in &dag[cur] {
                    let want = cur_pos + neigh.gap;
                    if position[neigh.id] < want {
                        position[neigh.id] = want;
                    }
                    in_deg[neigh.id] = in_deg[neigh.id].saturating_sub(1);
                    if in_deg[neigh.id] == 0 {
                        q.push_back(neigh.id);
                    }

                    let mut merged_bits = past_bits[cur];
                    let mut merged_order: Vec<usize> = past_order[cur].clone();
                    for &v in &past_order[neigh.id] {
                        let bit = 1u64 << (v as u64);
                        if (merged_bits & bit) == 0 {
                            merged_bits |= bit;
                            merged_order.push(v);
                        }
                    }
                    past_bits[neigh.id] = merged_bits;
                    past_order[neigh.id] = merged_order;
                }
            }

            let mut sink_nodes: Vec<usize> = Vec::new();
            for &k in keys {
                if dag[k].is_empty() {
                    sink_nodes.push(k);
                }
            }

            let mut comp_bits: Vec<u64> = Vec::new();
            let mut comp_order: Vec<Vec<usize>> = Vec::new();
            for &k in keys {
                if !sink_nodes.contains(&k) || past_order[k].is_empty() {
                    continue;
                }
                let first = past_order[k][0];
                let first_bit = 1u64 << (first as u64);
                if let Some(idx) = comp_bits.iter().position(|b| (*b & first_bit) != 0) {
                    let mut bits = comp_bits[idx];
                    let mut order = comp_order[idx].clone();
                    for &v in &past_order[k] {
                        let bit = 1u64 << (v as u64);
                        if (bits & bit) == 0 {
                            bits |= bit;
                            order.push(v);
                        }
                    }
                    comp_bits[idx] = bits;
                    comp_order[idx] = order;
                } else {
                    comp_bits.push(past_bits[k]);
                    comp_order.push(past_order[k].clone());
                }
            }

            for comp in comp_order {
                let mut min_before = f64::INFINITY;
                let mut max_before = f64::NEG_INFINITY;
                let mut min_after = f64::INFINITY;
                let mut max_after = f64::NEG_INFINITY;
                for &node in &comp {
                    let before = pos_before(node, axis, x, y);
                    let after = position[node];
                    min_before = min_before.min(before);
                    max_before = max_before.max(before);
                    min_after = min_after.min(after);
                    max_after = max_after.max(after);
                }
                let diff = ((min_before + max_before) / 2.0) - ((min_after + max_after) / 2.0);
                for &node in &comp {
                    position[node] += diff;
                }
            }

            position
        }

        let (keys_h, dag_h) = build_axis_dag_keys(Axis::Horizontal, rel, n);
        if !keys_h.is_empty() {
            let rev_h = build_rev(&keys_h, &dag_h, n);
            let sources = component_sources(&keys_h, &dag_h, &rev_h, n);
            let pos =
                find_appropriate_positions(&keys_h, &dag_h, Axis::Horizontal, n, x, y, &sources);
            for &k in &keys_h {
                x[k] = pos[k];
            }
        }

        let (keys_v, dag_v) = build_axis_dag_keys(Axis::Vertical, rel, n);
        if !keys_v.is_empty() {
            let rev_v = build_rev(&keys_v, &dag_v, n);
            let sources = component_sources(&keys_v, &dag_v, &rev_v, n);
            let pos =
                find_appropriate_positions(&keys_v, &dag_v, Axis::Vertical, n, x, y, &sources);
            for &k in &keys_v {
                y[k] = pos[k];
            }
        }
    }

    if c.align_vertical.is_empty() && c.align_horizontal.is_empty() && n <= 64 {
        enforce_relative_placement_no_align_small(x, y, &c.relative, n);
        return;
    }

    // Dummy mappings for alignment constraints (per-axis, matching `ConstraintHandler`).
    let mut dummy_to_nodes_for_vertical_alignment: Vec<Vec<usize>> = Vec::new();
    let mut node_to_dummy_for_vertical_alignment: Vec<Option<usize>> = vec![None; n];
    for (i, group) in c.align_vertical.iter().enumerate() {
        let dummy = n + i;
        dummy_to_nodes_for_vertical_alignment.push(group.clone());
        for &idx in group {
            if idx < n {
                node_to_dummy_for_vertical_alignment[idx] = Some(dummy);
            }
        }
    }
    let mut dummy_pos_for_vertical_alignment: Vec<f64> = dummy_to_nodes_for_vertical_alignment
        .iter()
        .map(|g| x[*g.first().unwrap_or(&0)])
        .collect();

    let mut dummy_to_nodes_for_horizontal_alignment: Vec<Vec<usize>> = Vec::new();
    let mut node_to_dummy_for_horizontal_alignment: Vec<Option<usize>> = vec![None; n];
    for (i, group) in c.align_horizontal.iter().enumerate() {
        let dummy = n + i;
        dummy_to_nodes_for_horizontal_alignment.push(group.clone());
        for &idx in group {
            if idx < n {
                node_to_dummy_for_horizontal_alignment[idx] = Some(dummy);
            }
        }
    }
    let mut dummy_pos_for_horizontal_alignment: Vec<f64> = dummy_to_nodes_for_horizontal_alignment
        .iter()
        .map(|g| y[*g.first().unwrap_or(&0)])
        .collect();

    fn ensure_key(map: &mut IndexMap<usize, Vec<Neighbor>>, key: usize) {
        if !map.contains_key(&key) {
            map.insert(key, Vec::new());
        }
    }

    let mut dag_h: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
    let mut dag_v: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
    for r in &c.relative {
        if let (Some(left), Some(right)) = (r.left, r.right) {
            let src = node_to_dummy_for_vertical_alignment[left].unwrap_or(left);
            let dst = node_to_dummy_for_vertical_alignment[right].unwrap_or(right);
            ensure_key(&mut dag_h, src);
            ensure_key(&mut dag_h, dst);
            dag_h.get_mut(&src).unwrap().push(Neighbor {
                id: dst,
                gap: r.gap,
            });
        } else if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
            let src = node_to_dummy_for_horizontal_alignment[top].unwrap_or(top);
            let dst = node_to_dummy_for_horizontal_alignment[bottom].unwrap_or(bottom);
            ensure_key(&mut dag_v, src);
            ensure_key(&mut dag_v, dst);
            dag_v.get_mut(&src).unwrap().push(Neighbor {
                id: dst,
                gap: r.gap,
            });
        }
    }

    fn dag_to_undirected(dag: &IndexMap<usize, Vec<Neighbor>>) -> IndexMap<usize, Vec<Neighbor>> {
        let mut u: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
        for (&k, _) in dag.iter() {
            u.insert(k, Vec::new());
        }
        for (&k, neigh) in dag.iter() {
            for n in neigh {
                u.get_mut(&k).unwrap().push(*n);
                u.get_mut(&n.id)
                    .unwrap()
                    .push(Neighbor { id: k, gap: n.gap });
            }
        }
        u
    }

    fn dag_to_reversed(dag: &IndexMap<usize, Vec<Neighbor>>) -> IndexMap<usize, Vec<Neighbor>> {
        let mut r: IndexMap<usize, Vec<Neighbor>> = IndexMap::new();
        for (&k, _) in dag.iter() {
            r.insert(k, Vec::new());
        }
        for (&k, neigh) in dag.iter() {
            for n in neigh {
                r.get_mut(&n.id)
                    .unwrap()
                    .push(Neighbor { id: k, gap: n.gap });
            }
        }
        r
    }

    fn find_components(undirected: &IndexMap<usize, Vec<Neighbor>>) -> Vec<Vec<usize>> {
        use std::collections::{HashSet, VecDeque};
        let mut visited: HashSet<usize> = HashSet::new();
        let mut out: Vec<Vec<usize>> = Vec::new();
        for (&k, _) in undirected.iter() {
            if visited.contains(&k) {
                continue;
            }
            let mut q: VecDeque<usize> = VecDeque::new();
            let mut comp: Vec<usize> = Vec::new();
            q.push_back(k);
            visited.insert(k);
            while let Some(cur) = q.pop_front() {
                comp.push(cur);
                for n in &undirected[&cur] {
                    if visited.insert(n.id) {
                        q.push_back(n.id);
                    }
                }
            }
            out.push(comp);
        }
        out
    }

    fn component_sources(
        dag: &IndexMap<usize, Vec<Neighbor>>,
        rev: &IndexMap<usize, Vec<Neighbor>>,
    ) -> Vec<Vec<usize>> {
        let undirected = dag_to_undirected(dag);
        let comps = find_components(&undirected);
        let mut out: Vec<Vec<usize>> = Vec::new();
        for comp in comps {
            let mut sources: Vec<usize> = Vec::new();
            for node in comp {
                if rev.get(&node).is_none_or(|v| v.is_empty()) {
                    sources.push(node);
                }
            }
            out.push(sources);
        }
        out
    }

    fn pos_before(key: usize, axis: Axis, n: usize, x: &[f64], y: &[f64], dummy: &[f64]) -> f64 {
        if key < n {
            match axis {
                Axis::Horizontal => x[key],
                Axis::Vertical => y[key],
            }
        } else {
            dummy[key - n]
        }
    }

    fn find_appropriate_positions(
        dag: &IndexMap<usize, Vec<Neighbor>>,
        axis: Axis,
        n: usize,
        x: &[f64],
        y: &[f64],
        dummy_pos: &[f64],
        component_sources: &[Vec<usize>],
    ) -> IndexMap<usize, f64> {
        use std::collections::VecDeque;

        let mut in_deg: IndexMap<usize, usize> = IndexMap::new();
        for (&k, _) in dag.iter() {
            in_deg.insert(k, 0);
        }
        for (&_k, neigh) in dag.iter() {
            for n2 in neigh {
                *in_deg.get_mut(&n2.id).unwrap() += 1;
            }
        }

        let mut position: IndexMap<usize, f64> = IndexMap::new();
        let mut past: IndexMap<usize, IndexSet<usize>> = IndexMap::new();
        let mut q: VecDeque<usize> = VecDeque::new();

        for (&k, &deg) in in_deg.iter() {
            position.insert(k, f64::NEG_INFINITY);
            if deg == 0 {
                q.push_back(k);
            }
            past.insert(k, IndexSet::from([k]));
        }

        // Align sources of each component (enforcement path, empty fixed-node set).
        for component in component_sources {
            if component.is_empty() {
                continue;
            }
            let mut sum = 0.0;
            for &node in component {
                sum += pos_before(node, axis, n, x, y, dummy_pos);
            }
            let avg = sum / (component.len() as f64);
            for &node in component {
                position.insert(node, avg);
            }
        }

        while let Some(cur) = q.pop_front() {
            let cur_pos = position[&cur];
            for neigh in &dag[&cur] {
                let want = cur_pos + neigh.gap;
                if position[&neigh.id] < want {
                    position.insert(neigh.id, want);
                }
                let deg = in_deg.get_mut(&neigh.id).unwrap();
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    q.push_back(neigh.id);
                }
                let mut merged: IndexSet<usize> = past[&cur].clone();
                for v in past[&neigh.id].iter().copied() {
                    merged.insert(v);
                }
                past.insert(neigh.id, merged);
            }
        }

        // Readjust position after enforcement.
        let mut sink_nodes: IndexSet<usize> = IndexSet::new();
        for (&k, neigh) in dag.iter() {
            if neigh.is_empty() {
                sink_nodes.insert(k);
            }
        }

        let mut components: Vec<IndexSet<usize>> = Vec::new();
        for (&k, set) in past.iter() {
            if !sink_nodes.contains(&k) || set.is_empty() {
                continue;
            }
            let first = *set.iter().next().unwrap();
            if let Some(idx) = components.iter().position(|c| c.contains(&first)) {
                let mut merged = components[idx].clone();
                for v in set.iter().copied() {
                    merged.insert(v);
                }
                components[idx] = merged;
            } else {
                components.push(set.clone());
            }
        }

        for comp in components {
            let mut min_before = f64::INFINITY;
            let mut max_before = f64::NEG_INFINITY;
            let mut min_after = f64::INFINITY;
            let mut max_after = f64::NEG_INFINITY;
            for &node in comp.iter() {
                let before = pos_before(node, axis, n, x, y, dummy_pos);
                let after = position[&node];
                min_before = min_before.min(before);
                max_before = max_before.max(before);
                min_after = min_after.min(after);
                max_after = max_after.max(after);
            }
            let diff = ((min_before + max_before) / 2.0) - ((min_after + max_after) / 2.0);
            for &node in comp.iter() {
                position.insert(node, position[&node] + diff);
            }
        }

        position
    }

    if !dag_h.is_empty() {
        let rev = dag_to_reversed(&dag_h);
        let sources = component_sources(&dag_h, &rev);
        let pos = find_appropriate_positions(
            &dag_h,
            Axis::Horizontal,
            n,
            x,
            y,
            &dummy_pos_for_vertical_alignment,
            &sources,
        );
        for (&key, &v) in pos.iter() {
            if key < n {
                x[key] = v;
            } else {
                let di = key - n;
                for &idx in &dummy_to_nodes_for_vertical_alignment[di] {
                    x[idx] = v;
                }
                dummy_pos_for_vertical_alignment[di] = v;
            }
        }
    }

    if !dag_v.is_empty() {
        let rev = dag_to_reversed(&dag_v);
        let sources = component_sources(&dag_v, &rev);
        let pos = find_appropriate_positions(
            &dag_v,
            Axis::Vertical,
            n,
            x,
            y,
            &dummy_pos_for_horizontal_alignment,
            &sources,
        );
        for (&key, &v) in pos.iter() {
            if key < n {
                y[key] = v;
            } else {
                let di = key - n;
                for &idx in &dummy_to_nodes_for_horizontal_alignment[di] {
                    y[idx] = v;
                }
                dummy_pos_for_horizontal_alignment[di] = v;
            }
        }
    }
}

fn apply_root_compound_overlap_separation_to_displacements(
    nodes: &[SimNode],
    root_to_nodes: &std::collections::BTreeMap<String, Vec<usize>>,
    padding: f64,
    separation_buffer: f64,
    max_d: f64,
    disps: &mut [(f64, f64)],
) {
    if root_to_nodes.len() <= 1 {
        return;
    }
    if nodes.is_empty() || disps.is_empty() {
        return;
    }

    #[derive(Debug, Clone, Copy)]
    struct Rect {
        left: f64,
        top: f64,
        width: f64,
        height: f64,
    }

    fn rect_from_node_with_disp(n: &SimNode, dx: f64, dy: f64) -> Rect {
        Rect {
            left: n.left + dx,
            top: n.top + dy,
            width: n.width,
            height: n.height,
        }
    }

    fn rect_union(a: Rect, b: Rect) -> Rect {
        let min_x = a.left.min(b.left);
        let min_y = a.top.min(b.top);
        let max_x = (a.left + a.width).max(b.left + b.width);
        let max_y = (a.top + a.height).max(b.top + b.height);
        Rect {
            left: min_x,
            top: min_y,
            width: (max_x - min_x).max(0.0),
            height: (max_y - min_y).max(0.0),
        }
    }

    fn expand_rect(r: Rect, pad: f64) -> Rect {
        Rect {
            left: r.left - pad,
            top: r.top - pad,
            width: (r.width + 2.0 * pad).max(0.0),
            height: (r.height + 2.0 * pad).max(0.0),
        }
    }

    fn rects_intersect(a: Rect, b: Rect) -> bool {
        a.left < b.left + b.width
            && a.left + a.width > b.left
            && a.top < b.top + b.height
            && a.top + a.height > b.top
    }

    fn calc_separation_amount_rect(a: Rect, b: Rect, buffer: f64) -> (f64, f64) {
        // Equivalent to `IGeometry.calcSeparationAmount(...)` for overlapping rectangles, with the
        // same `DEFAULT_EDGE_LENGTH / 2` buffer used by layout-base.
        //
        // We compute the minimal translation vector to separate the rectangles, preferring the
        // axis with smaller overlap.
        let overlap_x1 = (a.left + a.width + buffer) - b.left;
        let overlap_x2 = (b.left + b.width + buffer) - a.left;
        let overlap_y1 = (a.top + a.height + buffer) - b.top;
        let overlap_y2 = (b.top + b.height + buffer) - a.top;

        let ox = if overlap_x1.abs() < overlap_x2.abs() {
            overlap_x1
        } else {
            -overlap_x2
        };
        let oy = if overlap_y1.abs() < overlap_y2.abs() {
            overlap_y1
        } else {
            -overlap_y2
        };

        if ox.abs() < oy.abs() {
            (ox, 0.0)
        } else {
            (0.0, oy)
        }
    }

    let mut rects: Vec<(String, Rect)> = Vec::with_capacity(root_to_nodes.len());
    for (root, members) in root_to_nodes {
        let mut any = false;
        let mut bb = Rect {
            left: 0.0,
            top: 0.0,
            width: 0.0,
            height: 0.0,
        };
        for &idx in members {
            if idx >= nodes.len() || idx >= disps.len() {
                continue;
            }
            let r = rect_from_node_with_disp(&nodes[idx], disps[idx].0, disps[idx].1);
            bb = if any { rect_union(bb, r) } else { r };
            any = true;
        }
        if any {
            rects.push((root.clone(), expand_rect(bb, padding)));
        }
    }
    if rects.len() <= 1 {
        return;
    }

    // Deterministic, gentle overlap separation: translate all descendants of each root compound.
    // This approximates Cytoscape's compound repulsion without implementing full compound nodes.
    let strength = 0.35;
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let (ref a_id, a_rect) = rects[i];
            let (ref b_id, b_rect) = rects[j];
            if !rects_intersect(a_rect, b_rect) {
                continue;
            }
            let (ox, oy) = calc_separation_amount_rect(a_rect, b_rect, separation_buffer);
            if ox == 0.0 && oy == 0.0 {
                continue;
            }
            let (dx_a, dy_a) = (-0.5 * ox * strength, -0.5 * oy * strength);
            let (dx_b, dy_b) = (0.5 * ox * strength, 0.5 * oy * strength);

            if let Some(members) = root_to_nodes.get(a_id) {
                for &idx in members {
                    disps[idx].0 += dx_a;
                    disps[idx].1 += dy_a;
                }
            }
            if let Some(members) = root_to_nodes.get(b_id) {
                for &idx in members {
                    disps[idx].0 += dx_b;
                    disps[idx].1 += dy_b;
                }
            }
        }
    }

    // Cap displacements after compound separation, matching the upstream displacement clamp.
    if max_d.is_finite() && max_d > 0.0 {
        for (dx, dy) in disps {
            if dx.abs() > max_d {
                *dx = max_d * dx.signum();
            }
            if dy.abs() > max_d {
                *dy = max_d * dy.signum();
            }
        }
    }
}

fn apply_constraints_to_displacements(
    nodes: &[SimNode],
    c: &Constraints,
    disps: &mut [(f64, f64)],
    max_d: f64,
) {
    // Alignments: enforce exact alignment by adjusting displacements to a shared target line.
    for group in &c.align_horizontal {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        let mut cnt = 0.0;
        for &idx in group {
            sum += nodes[idx].center_y() + disps[idx].1;
            cnt += 1.0;
        }
        if cnt > 0.0 {
            let target = sum / cnt;
            for &idx in group {
                disps[idx].1 += target - (nodes[idx].center_y() + disps[idx].1);
            }
        }
    }
    for group in &c.align_vertical {
        if group.len() <= 1 {
            continue;
        }
        let mut sum = 0.0;
        let mut cnt = 0.0;
        for &idx in group {
            sum += nodes[idx].center_x() + disps[idx].0;
            cnt += 1.0;
        }
        if cnt > 0.0 {
            let target = sum / cnt;
            for &idx in group {
                disps[idx].0 += target - (nodes[idx].center_x() + disps[idx].0);
            }
        }
    }

    // Relative placements: iteratively relax displacements to satisfy minimum center gaps.
    // This is a small, deterministic approximation of `cose-base` constraint handling.
    for _ in 0..4 {
        let mut changed = false;
        for r in &c.relative {
            if let (Some(left), Some(right)) = (r.left, r.right) {
                let new_gap = (nodes[right].center_x() + disps[right].0)
                    - (nodes[left].center_x() + disps[left].0);
                if new_gap < r.gap {
                    let delta = r.gap - new_gap;
                    disps[left].0 -= delta / 2.0;
                    disps[right].0 += delta / 2.0;
                    changed = true;
                }
            }
            if let (Some(top), Some(bottom)) = (r.top, r.bottom) {
                let new_gap = (nodes[bottom].center_y() + disps[bottom].1)
                    - (nodes[top].center_y() + disps[top].1);
                if new_gap < r.gap {
                    let delta = r.gap - new_gap;
                    disps[top].1 -= delta / 2.0;
                    disps[bottom].1 += delta / 2.0;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Re-apply per-axis displacement caps (matching the upstream `calculateDisplacement` clamp).
    if max_d.is_finite() && max_d > 0.0 {
        for (dx, dy) in disps {
            if dx.abs() > max_d {
                *dx = max_d * dx.signum();
            }
            if dy.abs() > max_d {
                *dy = max_d * dy.signum();
            }
        }
    }
}

#[derive(Debug, Clone)]
struct XorShift64Star {
    state: u64,
}

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    #[allow(dead_code)]
    fn mix_u64(&mut self, v: u64) {
        // One-way mix to decorrelate node indices.
        self.state ^= v.wrapping_mul(0x9E3779B97F4A7C15_u64);
        let _ = self.next_u64();
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D_u64)
    }

    fn next_f64_signed(&mut self) -> f64 {
        // Map to [-1, 1] (exclusive).
        let u = self.next_u64() >> 11;
        let v = (u as f64) / ((1u64 << 53) as f64);
        (v * 2.0) - 1.0
    }

    fn next_f64_unit(&mut self) -> f64 {
        // Map to [0, 1) with 53 bits of precision.
        let u = self.next_u64() >> 11;
        (u as f64) / ((1u64 << 53) as f64)
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        // Match the seeded upstream baselines which override `Math.random()` with a 53-bit float
        // derived from `nextU64() >> 11`, then select indices via
        // `Math.floor(Math.random() * upper)`.
        //
        // Using `% upper` introduces modulo bias and (more importantly for parity) can yield a
        // different first sample pivot for small graphs (e.g. upper=3), which cascades into a
        // different spectral embedding orientation.
        let v = self.next_f64_unit();
        let idx = (v * (upper as f64)).floor() as usize;
        idx.min(upper - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{RepulsionGrid, SimNode, XorShift64Star};

    fn node_at(left: f64, top: f64, w: f64, h: f64) -> SimNode {
        SimNode {
            id: "n".to_string(),
            parent: None,
            width: w,
            height: h,
            left,
            top,
            spring_fx: 0.0,
            spring_fy: 0.0,
            repulsion_fx: 0.0,
            repulsion_fy: 0.0,
            surrounding: Vec::new(),
            grid_start_x: 0,
            grid_finish_x: 0,
            grid_start_y: 0,
            grid_finish_y: 0,
        }
    }

    #[test]
    fn xorshift64star_next_f64_unit_matches_seeded_upstream_baseline() {
        // Mirrors the JS prelude in `xtask` used to generate deterministic upstream SVGs:
        //
        // - xorshift64* (same shift/multiply constants)
        // - `Math.random = () => Number(nextU64() >> 11n) / 2^53`
        let mut rng = XorShift64Star::new(1);
        let expected = [
            0.28083505005035947,
            0.6711372530266764,
            0.7258461452833668,
            0.303529299965799,
            0.056176763098259475,
        ];
        for (i, &e) in expected.iter().enumerate() {
            let v = rng.next_f64_unit();
            assert!(
                (v - e).abs() < 1e-15,
                "unexpected rng value at {i}: got {v}, expected {e}"
            );
        }
    }

    #[test]
    fn xorshift64star_next_usize_matches_js_floor_random_times_upper() {
        // For seed=1, the first `Math.random()` value is ~0.2808 so `floor(r * 3) == 0`.
        // Using `% 3` on the underlying u64 yields `1`, which would diverge from the upstream
        // spectral sampling path for small graphs.
        let mut rng = XorShift64Star::new(1);
        assert_eq!(rng.next_usize(3), 0);
    }

    #[test]
    fn repulsion_grid_surrounding_excludes_processed_nodes() {
        // Build a tiny 1D-ish layout:
        //
        // - node0 and node1 are exactly within range
        // - node2 is far outside range
        let repulsion_range = 10.0;
        let mut nodes = vec![
            node_at(0.0, 0.0, 10.0, 10.0),
            node_at(20.0, 0.0, 10.0, 10.0),
            node_at(200.0, 0.0, 10.0, 10.0),
        ];
        let grid = RepulsionGrid::build(&nodes, repulsion_range).expect("grid");

        let mut processed = vec![false; nodes.len()];
        grid.refresh_node_surrounding(0, &mut nodes, &processed, repulsion_range);
        assert_eq!(nodes[0].surrounding, vec![1]);

        processed[0] = true;
        grid.refresh_node_surrounding(1, &mut nodes, &processed, repulsion_range);
        assert!(
            !nodes[1].surrounding.contains(&0),
            "node1 should not include already-processed node0"
        );
    }

    #[test]
    fn relative_placement_gap_is_center_to_center() {
        use super::{Constraints, RelConstraint, apply_constraints_to_displacements};

        let nodes = vec![
            node_at(0.0, 0.0, 10.0, 10.0),  // center_x = 5
            node_at(20.0, 0.0, 10.0, 10.0), // center_x = 25
        ];
        let mut disps = vec![(0.0, 0.0); nodes.len()];

        let c = Constraints {
            align_horizontal: Vec::new(),
            align_vertical: Vec::new(),
            relative: vec![RelConstraint {
                left: Some(0),
                right: Some(1),
                top: None,
                bottom: None,
                gap: 50.0,
            }],
        };

        apply_constraints_to_displacements(&nodes, &c, &mut disps, 1e9);
        let gap = (nodes[1].center_x() + disps[1].0) - (nodes[0].center_x() + disps[0].0);
        assert!((gap - 50.0).abs() < 1e-9, "gap: got {gap}");
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
    min_repulsion_dist: f64,
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

        if dx.abs() < min_repulsion_dist {
            dx = dx.signum() * min_repulsion_dist;
        }
        if dy.abs() < min_repulsion_dist {
            dy = dy.signum() * min_repulsion_dist;
        }

        let dist_sq = dx * dx + dy * dy;
        let dist = dist_sq.sqrt();
        if dist_sq == 0.0 || dist == 0.0 {
            return (0.0, 0.0);
        }
        // layout-base: `(nodeA.nodeRepulsion/2 + nodeB.nodeRepulsion/2) / dist^2`.
        // FCoSE default `nodeRepulsion` is a constant 4500, so this collapses to 4500/dist^2.
        let repulsion_force = SimGraph::DEFAULT_REPULSION_STRENGTH / dist_sq;
        let rfx = repulsion_force * dx / dist;
        let rfy = repulsion_force * dy / dist;
        (-rfx, -rfy)
    }
}

#[derive(Debug, Clone)]
struct RepulsionGrid {
    left: f64,
    top: f64,
    size_x: i32,
    size_y: i32,
    // Flat grid: cells[x * size_y + y] contains node indices.
    cells: Vec<Vec<usize>>,
}

impl RepulsionGrid {
    fn idx(&self, x: i32, y: i32) -> usize {
        (x as usize) * (self.size_y as usize) + (y as usize)
    }

    fn cell(&self, x: i32, y: i32) -> &[usize] {
        &self.cells[self.idx(x, y)]
    }

    fn build(nodes: &[SimNode], repulsion_range: f64) -> Option<Self> {
        if nodes.is_empty() {
            return None;
        }
        if !repulsion_range.is_finite() || repulsion_range <= 0.0 {
            return None;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for n in nodes {
            min_x = min_x.min(n.left);
            min_y = min_y.min(n.top);
            max_x = max_x.max(n.right());
            max_y = max_y.max(n.bottom());
        }
        if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
            return None;
        }

        let w = (max_x - min_x).max(1.0);
        let h = (max_y - min_y).max(1.0);
        let size_x = ((w / repulsion_range).floor() as i32 + 1).max(1);
        let size_y = ((h / repulsion_range).floor() as i32 + 1).max(1);
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); (size_x as usize) * (size_y as usize)];

        // Mirror layout-base `addNodeToGrid`: push the node into every cell that intersects the
        // node's rect, using top-left anchored coordinates.
        for (idx, n) in nodes.iter().enumerate() {
            let mut start_x = ((n.left - min_x) / repulsion_range).floor() as i32;
            let mut finish_x = ((n.right() - min_x) / repulsion_range).floor() as i32;
            let mut start_y = ((n.top - min_y) / repulsion_range).floor() as i32;
            let mut finish_y = ((n.bottom() - min_y) / repulsion_range).floor() as i32;

            start_x = start_x.clamp(0, size_x - 1);
            finish_x = finish_x.clamp(0, size_x - 1);
            start_y = start_y.clamp(0, size_y - 1);
            finish_y = finish_y.clamp(0, size_y - 1);

            for gx in start_x..=finish_x {
                for gy in start_y..=finish_y {
                    let cell_idx = (gx as usize) * (size_y as usize) + (gy as usize);
                    cells[cell_idx].push(idx);
                }
            }
        }

        Some(Self {
            left: min_x,
            top: min_y,
            size_x,
            size_y,
            cells,
        })
    }

    fn refresh_node_surrounding(
        &self,
        node_idx: usize,
        nodes: &mut [SimNode],
        processed: &[bool],
        repulsion_range: f64,
    ) {
        let (start_x, finish_x, start_y, finish_y) =
            self.node_grid_coords(node_idx, nodes, repulsion_range);
        nodes[node_idx].grid_start_x = start_x;
        nodes[node_idx].grid_finish_x = finish_x;
        nodes[node_idx].grid_start_y = start_y;
        nodes[node_idx].grid_finish_y = finish_y;

        let mut seen: Vec<bool> = vec![false; nodes.len()];
        let mut surrounding: Vec<usize> = Vec::new();

        for gx in (start_x - 1)..=(finish_x + 1) {
            if gx < 0 || gx >= self.size_x {
                continue;
            }
            for gy in (start_y - 1)..=(finish_y + 1) {
                if gy < 0 || gy >= self.size_y {
                    continue;
                }
                for &other in self.cell(gx, gy) {
                    if other == node_idx {
                        continue;
                    }
                    if processed.get(other).copied().unwrap_or(false) {
                        continue;
                    }
                    if seen[other] {
                        continue;
                    }

                    let dx = (nodes[node_idx].center_x() - nodes[other].center_x()).abs()
                        - (nodes[node_idx].half_w() + nodes[other].half_w());
                    let dy = (nodes[node_idx].center_y() - nodes[other].center_y()).abs()
                        - (nodes[node_idx].half_h() + nodes[other].half_h());
                    if dx <= repulsion_range && dy <= repulsion_range {
                        seen[other] = true;
                        surrounding.push(other);
                    }
                }
            }
        }

        nodes[node_idx].surrounding = surrounding;
    }

    fn node_grid_coords(
        &self,
        node_idx: usize,
        nodes: &[SimNode],
        repulsion_range: f64,
    ) -> (i32, i32, i32, i32) {
        let n = &nodes[node_idx];
        let mut start_x = ((n.left - self.left) / repulsion_range).floor() as i32;
        let mut finish_x = ((n.right() - self.left) / repulsion_range).floor() as i32;
        let mut start_y = ((n.top - self.top) / repulsion_range).floor() as i32;
        let mut finish_y = ((n.bottom() - self.top) / repulsion_range).floor() as i32;

        start_x = start_x.clamp(0, self.size_x - 1);
        finish_x = finish_x.clamp(0, self.size_x - 1);
        start_y = start_y.clamp(0, self.size_y - 1);
        finish_y = finish_y.clamp(0, self.size_y - 1);

        (start_x, finish_x, start_y, finish_y)
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

    let dx = -dir_x * ((move_by_x / 2.0) + separation_buffer);
    let dy = -dir_y * ((move_by_y / 2.0) + separation_buffer);
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
