#![allow(clippy::needless_range_loop)]

use super::geometry::{self, Rect};
use crate::algo::FcoseOptions;
use crate::error::{Error, Result, WorkFailure};
use crate::graph::{Anchor, BoundsExtras, Graph, LayoutRect, LayoutResult, Point};
use rustc_hash::FxHashMap;

mod compound_topology;
mod constraint_projection;
mod constraints;
mod relocation;
mod repulsion_grid;
mod spectral;
mod spring_embedder;
mod work_plan;

use compound_topology::CompoundTopology;
#[cfg(test)]
use compound_topology::{DisjointSet, EdgeProjection};
#[cfg(test)]
use constraint_projection::{
    Mat2, Vec2, apply_constraints_to_displacements, apply_reflection_for_relative_placement,
    handle_constraints_pre_layout, procrustes_transform_for_alignments,
};
use constraints::Constraints;
#[cfg(test)]
use constraints::RelConstraint;
#[cfg(test)]
use repulsion_grid::{
    RepulsionGridCells, RepulsionGridPlan, RepulsionGridStorageKind, implicit_grid_work_units,
};

pub use crate::work::{NoopWorkControl, WorkControl};
pub use work_plan::FcoseIterationSchedule;
use work_plan::{FcoseConstraintWorkShape, FcoseWorkPlan};

const GEOMETRY_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FcoseRandomSource {
    #[default]
    XorShift64Star,
    Mulberry32,
}

/// Randomness policy for an FCoSE layout invocation.
///
/// This is separate from [`FcoseOptions`] and [`IndexedFcoseOptions`] so callers that construct
/// those public option types with struct literals remain source-compatible as new random modes are
/// added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct FcoseRandomPolicy {
    source: FcoseRandomSource,
    seed: u64,
    seed_offset: Option<usize>,
    reset_seed_each_run: bool,
}

impl FcoseRandomPolicy {
    /// Use a deterministic seed for every layout invocation.
    pub const fn seeded(source: FcoseRandomSource, seed: u64) -> Self {
        Self {
            source,
            seed,
            seed_offset: None,
            reset_seed_each_run: false,
        }
    }

    /// Override the number of random values consumed before the first FCoSE draw.
    pub const fn with_seed_offset(mut self, seed_offset: usize) -> Self {
        self.seed_offset = Some(seed_offset);
        self
    }

    /// Restart a deterministic random stream before each FCoSE rerun.
    ///
    pub const fn with_reset_seed_each_run(mut self, reset: bool) -> Self {
        self.reset_seed_each_run = reset;
        self
    }

    /// Return the configured PRNG implementation.
    pub const fn source(self) -> FcoseRandomSource {
        self.source
    }

    /// Return the required deterministic seed.
    pub const fn seed(self) -> u64 {
        self.seed
    }

    /// Return the explicit pre-layout draw offset, if one was configured.
    pub const fn seed_offset(self) -> Option<usize> {
        self.seed_offset
    }

    /// Return whether deterministic streams restart before each rerun.
    pub const fn resets_seed_each_run(self) -> bool {
        self.reset_seed_each_run
    }

    const fn xorshift(seed: u64) -> Self {
        Self::seeded(FcoseRandomSource::XorShift64Star, seed)
    }
}

#[derive(Debug, Clone)]
pub struct IndexedGraph {
    pub nodes: Vec<IndexedNode>,
    pub edges: Vec<IndexedEdge>,
    /// Optional compound nodes. Parent references in nodes and compounds point into this vector.
    pub compounds: Vec<IndexedCompound>,
}

impl IndexedGraph {
    fn validate(&self) -> Result<()> {
        for (idx, n) in self.nodes.iter().enumerate() {
            if n.parent.is_some_and(|p| p >= self.compounds.len()) {
                return Err(crate::error::Error::MissingEndpoint {
                    edge_id: format!("node-parent:#{idx}"),
                });
            }
        }

        for (idx, c) in self.compounds.iter().enumerate() {
            if c.parent.is_some_and(|p| p >= self.compounds.len()) {
                return Err(crate::error::Error::MissingEndpoint {
                    edge_id: format!("compound-parent:#{idx}"),
                });
            }
        }

        // Reject cyclic compound ownership before SimGraph or spectral hierarchy materialization.
        // Public IndexedGraph callers can otherwise construct a self/indirect parent cycle that
        // makes every parent-chain walk non-terminating despite a finite work budget.
        let mut state = vec![0u8; self.compounds.len()];
        let mut path: Vec<usize> = Vec::new();
        for start in 0..self.compounds.len() {
            if state[start] != 0 {
                continue;
            }

            path.clear();
            let mut current = Some(start);
            while let Some(compound) = current {
                match state[compound] {
                    0 => {
                        state[compound] = 1;
                        path.push(compound);
                        current = self.compounds[compound].parent;
                    }
                    1 => {
                        return Err(crate::error::Error::MissingEndpoint {
                            edge_id: format!("compound-parent-cycle:#{compound}"),
                        });
                    }
                    _ => break,
                }
            }
            for compound in path.drain(..) {
                state[compound] = 2;
            }
        }

        for (idx, e) in self.edges.iter().enumerate() {
            if e.source >= self.nodes.len() || e.target >= self.nodes.len() {
                return Err(crate::error::Error::MissingEndpoint {
                    edge_id: format!("#{idx}"),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IndexedNode {
    /// Parent compound index, if any.
    pub parent: Option<usize>,
    pub width: f64,
    pub height: f64,
    /// Initial center position.
    pub x: f64,
    pub y: f64,
    pub bounds_extras: BoundsExtras,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexedCompound {
    /// Parent compound index, if any.
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexedEdge {
    pub source: usize,
    pub target: usize,
    pub label_width: Option<f64>,
    pub label_height: Option<f64>,
    pub source_anchor: Option<Anchor>,
    pub target_anchor: Option<Anchor>,
    pub curve_style_segments: bool,
    pub ideal_length: f64,
    pub elasticity: f64,
}

#[derive(Debug, Clone)]
pub struct IndexedFcoseOptions {
    pub random_seed: u64,
    pub random_seed_offset: Option<usize>,
    pub rerun: bool,
    pub randomize: bool,
    pub node_separation: Option<f64>,
    pub num_iter: Option<usize>,
    pub default_edge_length: Option<f64>,
    /// Alignment groups use FCoSE element indices: leaves first, then compounds. Only leaves
    /// and empty compounds can be direct constraint targets.
    pub alignment_constraint: Option<IndexedAlignmentConstraint>,
    /// Relative constraints use FCoSE element indices: leaves first, then compounds. Only leaves
    /// and empty compounds can be direct constraint targets.
    pub relative_placement_constraint: Vec<IndexedRelativePlacementConstraint>,
    pub compound_padding: Option<f64>,
    pub relocate_center: Option<(f64, f64)>,
}

impl Default for IndexedFcoseOptions {
    fn default() -> Self {
        Self {
            random_seed: 0,
            random_seed_offset: None,
            rerun: false,
            randomize: true,
            node_separation: None,
            num_iter: None,
            default_edge_length: None,
            alignment_constraint: None,
            relative_placement_constraint: Vec::new(),
            compound_padding: None,
            relocate_center: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IndexedAlignmentConstraint {
    /// Elements in each inner vec share the same y coordinate.
    pub horizontal: Vec<Vec<usize>>,
    /// Elements in each inner vec share the same x coordinate.
    pub vertical: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexedRelativePlacementConstraint {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub top: Option<usize>,
    pub bottom: Option<usize>,
    /// Minimum separation for each complete, in-range pair. Negative values are clamped to zero.
    /// NaN and positive infinity make an active axis infeasible.
    pub gap: f64,
}

impl IndexedRelativePlacementConstraint {
    fn zero_gap_compound_self_loops(
        self,
        compounds: std::ops::Range<usize>,
    ) -> impl Iterator<Item = usize> {
        [self.left.zip(self.right), self.top.zip(self.bottom)]
            .into_iter()
            .flatten()
            .filter_map(move |(from, to)| {
                (from == to && self.gap <= 0.0 && compounds.contains(&from)).then_some(from)
            })
    }
}

#[derive(Debug, Clone)]
pub struct IndexedLayoutResult {
    pub node_positions: Vec<Point>,
    pub compound_positions: Vec<Point>,
    /// Final layout-base compound rectangles after the last bounds update and relocation.
    ///
    /// These are the internal compound node rects used by the FCoSE port, not Cytoscape
    /// `node.boundingBox()` values.
    pub compound_bounds: Vec<LayoutRect>,
}

pub fn layout(graph: &Graph, opts: &FcoseOptions) -> Result<LayoutResult> {
    layout_with_random_policy(graph, opts, FcoseRandomPolicy::xorshift(opts.random_seed))
}

/// Lay out a graph with an explicit random policy while preserving [`FcoseOptions`]' stable
/// struct-literal API.
pub fn layout_with_random_policy(
    graph: &Graph,
    opts: &FcoseOptions,
    random_policy: FcoseRandomPolicy,
) -> Result<LayoutResult> {
    graph.validate()?;

    let (indexed_graph, indexed_opts) = graph_to_indexed(graph, opts);
    let indexed = layout_indexed_with_random_policy(&indexed_graph, &indexed_opts, random_policy)?;

    let mut positions: std::collections::BTreeMap<String, Point> =
        std::collections::BTreeMap::new();
    for (idx, n) in graph.nodes.iter().enumerate() {
        if let Some(p) = indexed.node_positions.get(idx).copied() {
            positions.insert(n.id.clone(), p);
        }
    }
    for (idx, c) in graph.compounds.iter().enumerate() {
        if let Some(p) = indexed.compound_positions.get(idx).copied() {
            positions.insert(c.id.clone(), p);
        }
    }

    Ok(LayoutResult { positions })
}

pub fn layout_indexed(
    graph: &IndexedGraph,
    opts: &IndexedFcoseOptions,
) -> Result<IndexedLayoutResult> {
    layout_indexed_with_random_policy(graph, opts, FcoseRandomPolicy::xorshift(opts.random_seed))
}

/// Lay out an indexed graph with an explicit random policy.
pub fn layout_indexed_with_random_policy(
    graph: &IndexedGraph,
    opts: &IndexedFcoseOptions,
    random_policy: FcoseRandomPolicy,
) -> Result<IndexedLayoutResult> {
    let mut work_control = NoopWorkControl;
    layout_indexed_with_random_policy_and_work_control(
        graph,
        opts,
        random_policy,
        &mut work_control,
    )
}

/// Lay out an indexed graph with explicit randomness and caller-owned work accounting.
pub fn layout_indexed_with_random_policy_and_work_control<W: WorkControl + ?Sized>(
    graph: &IndexedGraph,
    opts: &IndexedFcoseOptions,
    random_policy: FcoseRandomPolicy,
    work_control: &mut W,
) -> Result<IndexedLayoutResult> {
    let node_count = graph
        .nodes
        .len()
        .checked_add(graph.compounds.len())
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let edge_count = graph.edges.len();
    let (input_group_count, input_relative_count) = FcoseConstraintWorkShape::input_headers(opts)?;

    // Check 1 protects graph validation and the group-header scan without inspecting any node,
    // edge, alignment member, or relative endpoint. Checks are non-consuming by contract.
    let header_scan_bound = node_count
        .checked_add(edge_count)
        .and_then(|units| units.checked_add(input_group_count))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    work_control.check(header_scan_bound)?;

    graph.validate()?;

    let input_member_count = FcoseConstraintWorkShape::input_member_count(opts)?;
    // Check 2 protects the member/endpoint projection used to derive retained runtime shape.
    let input_scan_bound = header_scan_bound
        .checked_add(input_member_count)
        .and_then(|units| units.checked_add(input_relative_count))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    work_control.check(input_scan_bound)?;

    let mut shape = FcoseConstraintWorkShape::from_indexed_options(
        opts,
        graph.nodes.len(),
        node_count,
        input_group_count,
        input_member_count,
    )?;
    let schedule = FcoseIterationSchedule::from_options(
        opts.num_iter,
        graph.nodes.len(),
        graph.compounds.len(),
        edge_count,
        opts.rerun,
    )?;
    let random_seed_offset = random_policy
        .seed_offset
        .or(opts.random_seed_offset)
        .unwrap_or(usize::from(opts.randomize));
    let prepared_constraints = if shape.may_collapse_zero_gap() {
        let setup_work = shape.setup_work_units(schedule)?;
        work_control.check(setup_work)?;
        let mut constraints = Constraints::from_indexed_opts(node_count, opts);
        if constraints.normalize(node_count, work_control)? {
            shape = shape.with_normalized_constraints(&constraints)?;
        }
        Some(constraints)
    } else {
        None
    };
    let work_plan = FcoseWorkPlan::from_schedule(
        schedule,
        node_count,
        shape,
        random_seed_offset,
        random_policy.reset_seed_each_run,
    )?;
    // Admit the predictable schedule before building the simulation. For rows that could
    // collapse, a non-consuming setup check protected classification and normalization.
    work_control.check(work_plan.maximum_work_units())?;
    work_control.charge(work_plan.setup_work_units())?;
    let constraints = if let Some(constraints) = prepared_constraints {
        constraints
    } else {
        let mut constraints = Constraints::from_indexed_opts(node_count, opts);
        constraints.normalize(node_count, work_control)?;
        constraints
    };
    let mut sim = SimGraph::from_indexed(graph);
    let projection_node_count = constraints.projection_node_count(&sim)?;
    if shape.has_compound_zero_gap_self_loop() {
        // Normalization erases redundant self-loops. Validate those authored targets after
        // the other constraint checks, using the same compound children as the projection.
        work_control.check(0)?;
        let compounds = sim.leaf_count..node_count;
        if let Some(index) = opts
            .relative_placement_constraint
            .iter()
            .flat_map(|relative| relative.zero_gap_compound_self_loops(compounds.clone()))
            .find(|&index| !sim.children_by_owner[index].is_empty())
        {
            return Err(Error::UnsupportedCompoundConstraint { index });
        }
    }

    let spectral_topology = if opts.randomize && sim.leaf_count > 0 {
        Some(spectral::SpectralTopology::build(
            &sim.nodes[..sim.leaf_count],
            &sim.edges,
            &sim.compound_parent,
            work_control,
        )?)
    } else {
        None
    };

    let mut rng = new_fcose_random(random_policy);
    advance_random_with_work_control(
        &mut rng,
        work_plan.random_seed_offset_work_units(),
        work_control,
    )?;
    let mut owner_bounds = OwnerBounds::new(sim.nodes.len() + 1);

    for run_idx in 0..schedule.run_count() {
        reset_fcose_random_for_run(
            &mut rng,
            random_policy,
            run_idx,
            work_plan.random_seed_offset_work_units(),
            work_control,
        )?;
        // Mirror upstream component center bookkeeping (`eles.boundingBox()` before layout) by
        // ensuring compound rects wrap their current children before we compute `orig_center`.
        let compound_padding = opts.compound_padding.unwrap_or(0.0).max(0.0);
        if compound_padding > 0.0 {
            for n in &mut sim.nodes {
                if n.is_compound {
                    n.padding = compound_padding;
                }
            }
        }
        sim.update_bounds(&mut owner_bounds);

        // Mimic fcose's `aux.relocateComponent(...)`: keep the final component center aligned to
        // the original component center to avoid arbitrary global translations affecting viewBox
        // parity.
        //
        // Upstream uses Cytoscape `eles.boundingBox()` to capture the pre-layout component center,
        // and then relocates the final node rects to that center. Importantly, compounds are part
        // of that bbox, and compound sizes include padding (and may include label sizing depending
        // on style). Using leaves-only centers creates deterministic root drift for group-heavy
        // diagrams (e.g. architecture groups-within-groups).
        //
        // Mermaid Architecture runs Cytoscape FCoSE twice (`layout.run()` in `layoutstop`). The
        // selected random policy determines whether a deterministic stream restarts per run or
        // remains continuous.
        // Upstream Cytoscape FCoSE keeps the final component aligned to the *pre-layout*
        // `options.eles.boundingBox()` center (nodes + edges + labels).
        //
        // Important: In proof/default quality, `aux.relocateComponent(...)` computes the
        // "current" bbox from layout-base node rects (`node.getRect()`), which excludes labels
        // when `nodeDimensionsIncludeLabels: false`.
        let orig_center = opts
            .relocate_center
            .or_else(|| sim.bounding_box_center_eles(run_idx))
            .unwrap_or((0.0, 0.0));
        sim.run_spring_embedder(SpringEmbedderContext {
            constraints: &constraints,
            projection_node_count,
            options: opts,
            work_plan,
            rng: &mut rng,
            owner_bounds: &mut owner_bounds,
            spectral_topology: spectral_topology.as_ref(),
            work_control,
        })?;

        // Ensure compound node rectangles reflect the final child placements before we compute the
        // "current" component bounding box for relocation (`aux.relocateComponent(...)` parity).
        sim.update_bounds(&mut owner_bounds);

        let new_center = sim.bounding_box_center_rects().unwrap_or((0.0, 0.0));
        let dx = orig_center.0 - new_center.0;
        let dy = orig_center.1 - new_center.1;
        sim.translate(dx, dy);

        if run_idx + 1 < schedule.run_count() {
            sim.update_bounds(&mut owner_bounds);
        }
    }

    let leaf_count = sim.leaf_count;
    let compound_count = sim.compound_parent.len();

    let mut node_positions: Vec<Point> = Vec::with_capacity(leaf_count);
    let mut compound_positions: Vec<Point> = Vec::with_capacity(compound_count);
    let mut compound_bounds: Vec<LayoutRect> = Vec::with_capacity(compound_count);
    let nodes = std::mem::take(&mut sim.nodes);
    for (idx, n) in nodes.into_iter().enumerate() {
        let x = n.center_x();
        let y = n.center_y();
        if idx < leaf_count {
            node_positions.push(Point { x, y });
        } else {
            compound_positions.push(Point { x, y });
            compound_bounds.push(LayoutRect {
                left: n.left,
                top: n.top,
                width: n.width,
                height: n.height,
            });
        }
    }
    let result = IndexedLayoutResult {
        node_positions,
        compound_positions,
        compound_bounds,
    };
    validate_indexed_layout_result(&result)?;
    Ok(result)
}

fn validate_indexed_layout_result(result: &IndexedLayoutResult) -> Result<()> {
    fn point_is_finite(point: Point) -> bool {
        point.x.is_finite() && point.y.is_finite()
    }

    fn rect_is_finite(rect: LayoutRect) -> bool {
        rect.left.is_finite()
            && rect.top.is_finite()
            && rect.width.is_finite()
            && rect.height.is_finite()
    }

    if !result.node_positions.iter().copied().all(point_is_finite) {
        return Err(Error::NonFiniteLayout {
            field: "node_positions",
        });
    }
    if !result
        .compound_positions
        .iter()
        .copied()
        .all(point_is_finite)
    {
        return Err(Error::NonFiniteLayout {
            field: "compound_positions",
        });
    }
    if !result.compound_bounds.iter().copied().all(rect_is_finite) {
        return Err(Error::NonFiniteLayout {
            field: "compound_bounds",
        });
    }
    Ok(())
}

fn graph_to_indexed(graph: &Graph, opts: &FcoseOptions) -> (IndexedGraph, IndexedFcoseOptions) {
    let mut node_id_to_idx: FxHashMap<&str, usize> = FxHashMap::default();
    node_id_to_idx.reserve(graph.nodes.len().saturating_mul(2));
    let mut compound_id_to_idx: FxHashMap<&str, usize> = FxHashMap::default();
    compound_id_to_idx.reserve(graph.compounds.len().saturating_mul(2));
    let mut element_id_to_idx: FxHashMap<&str, usize> = FxHashMap::default();
    element_id_to_idx.reserve((graph.nodes.len() + graph.compounds.len()).saturating_mul(2));

    for (idx, n) in graph.nodes.iter().enumerate() {
        node_id_to_idx.insert(n.id.as_str(), idx);
        element_id_to_idx.insert(n.id.as_str(), idx);
    }
    for (idx, c) in graph.compounds.iter().enumerate() {
        compound_id_to_idx.insert(c.id.as_str(), idx);
        element_id_to_idx.insert(c.id.as_str(), graph.nodes.len() + idx);
    }

    let indexed_graph = IndexedGraph {
        nodes: graph
            .nodes
            .iter()
            .map(|n| IndexedNode {
                parent: n
                    .parent
                    .as_deref()
                    .and_then(|p| compound_id_to_idx.get(p).copied()),
                width: n.width,
                height: n.height,
                x: n.x,
                y: n.y,
                bounds_extras: n.bounds_extras,
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .filter_map(|e| {
                let source = node_id_to_idx.get(e.source.as_str()).copied()?;
                let target = node_id_to_idx.get(e.target.as_str()).copied()?;
                Some(IndexedEdge {
                    source,
                    target,
                    label_width: e.label_width,
                    label_height: e.label_height,
                    source_anchor: e.source_anchor,
                    target_anchor: e.target_anchor,
                    curve_style_segments: false,
                    ideal_length: e.ideal_length,
                    elasticity: e.elasticity,
                })
            })
            .collect(),
        compounds: graph
            .compounds
            .iter()
            .map(|c| IndexedCompound {
                parent: c
                    .parent
                    .as_deref()
                    .and_then(|p| compound_id_to_idx.get(p).copied()),
            })
            .collect(),
    };

    let indexed_opts = IndexedFcoseOptions {
        random_seed: opts.random_seed,
        random_seed_offset: opts.random_seed_offset,
        rerun: opts.rerun,
        randomize: opts.randomize,
        node_separation: opts.node_separation,
        num_iter: opts.num_iter,
        default_edge_length: opts.default_edge_length,
        alignment_constraint: opts.alignment_constraint.as_ref().map(|a| {
            IndexedAlignmentConstraint {
                horizontal: map_string_align_lists(&a.horizontal, &element_id_to_idx),
                vertical: map_string_align_lists(&a.vertical, &element_id_to_idx),
            }
        }),
        relative_placement_constraint: opts
            .relative_placement_constraint
            .iter()
            .map(|r| IndexedRelativePlacementConstraint {
                left: r
                    .left
                    .as_deref()
                    .and_then(|id| element_id_to_idx.get(id).copied()),
                right: r
                    .right
                    .as_deref()
                    .and_then(|id| element_id_to_idx.get(id).copied()),
                top: r
                    .top
                    .as_deref()
                    .and_then(|id| element_id_to_idx.get(id).copied()),
                bottom: r
                    .bottom
                    .as_deref()
                    .and_then(|id| element_id_to_idx.get(id).copied()),
                gap: r.gap,
            })
            .collect(),
        compound_padding: opts.compound_padding,
        relocate_center: opts.relocate_center,
    };

    (indexed_graph, indexed_opts)
}

fn map_string_align_lists(
    groups: &[Vec<String>],
    element_id_to_idx: &FxHashMap<&str, usize>,
) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    for g in groups {
        let idxs: Vec<usize> = g
            .iter()
            .filter_map(|id| element_id_to_idx.get(id.as_str()).copied())
            .collect();
        if idxs.len() > 1 {
            out.push(idxs);
        }
    }
    out
}

#[derive(Debug, Clone)]
struct SimNode {
    parent: Option<usize>,
    owner_idx: usize,
    is_compound: bool,
    width: f64,
    height: f64,
    bounds_extras: BoundsExtras,
    // layout-base `LNode.estimatedSize` (stable across updateBounds mutations).
    estimated_size: f64,
    // Top-left anchored rectangle (layout-base `LNode.rect` style).
    left: f64,
    top: f64,

    spring_fx: f64,
    spring_fy: f64,
    repulsion_fx: f64,
    repulsion_fy: f64,
    gravitation_fx: f64,
    gravitation_fy: f64,

    // layout-base `LNode.noOfChildren` weight (leaf descendants count).
    no_of_children: f64,

    // Compound padding (Cytoscape style `padding`, mapped onto layout-base `paddingLeft/...`).
    // Used as a margin when computing child graph bounds.
    padding: f64,

    // layout-base FR-grid repulsion caches a per-node "surrounding" list, refreshed periodically.
    surrounding: Vec<usize>,
    grid_start_x: i64,
    grid_finish_x: i64,
    grid_start_y: i64,
    grid_finish_y: i64,
}

impl SimNode {
    fn rect(&self) -> Rect {
        Rect::new(self.left, self.top, self.width, self.height)
    }

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

fn imath_sign(value: f64) -> f64 {
    // layout-base `IMath.sign`: returns 1, -1, or 0 (and yields 0 for NaN).
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
struct SimEdge {
    a: usize,
    b: usize,
    a_anchor: Option<Anchor>,
    b_anchor: Option<Anchor>,
    curve_style_segments: bool,
    base_ideal_length: f64,
    ideal_length: f64,
    elasticity: f64,
    label_width: Option<f64>,
    label_height: Option<f64>,
}

#[derive(Debug)]
struct SimGraph {
    nodes: Vec<SimNode>,
    edges: Vec<SimEdge>,
    compound_topology: CompoundTopology,
    compound_parent: Vec<Option<usize>>,
    leaf_count: usize,
    // Owner graph identity for repulsion/gravity: each node belongs to the child graph of its
    // parent compound, or the root graph.
    root_owner_idx: usize,
    // Immediate children list for each owner graph (owner idx is a compound node idx, or
    // `root_owner_idx` for the root graph).
    children_by_owner: Vec<Vec<usize>>,
    // Reusable mark array for repulsion grid neighborhood deduplication.
    surrounding_seen: Vec<u32>,
    // Compound node indices in descending inclusion depth (deepest first), for updateBounds.
    compounds_deep_first: Vec<usize>,
    // Estimated size for each owner graph (static; computed from node sizes).
    owner_estimated_size: Vec<f64>,
    // layout-base `LNode.inclusionTreeDepth` (root-level nodes depth=1).
    inclusion_depth: Vec<usize>,
}

#[derive(Debug, Clone)]
struct OwnerBounds {
    left: Vec<f64>,
    right: Vec<f64>,
    top: Vec<f64>,
    bottom: Vec<f64>,
}

struct SpringEmbedderContext<'a, W: WorkControl + ?Sized> {
    constraints: &'a Constraints,
    projection_node_count: usize,
    options: &'a IndexedFcoseOptions,
    work_plan: FcoseWorkPlan,
    rng: &'a mut FcoseRandom,
    owner_bounds: &'a mut OwnerBounds,
    spectral_topology: Option<&'a spectral::SpectralTopology>,
    work_control: &'a mut W,
}

impl OwnerBounds {
    fn new(owner_count: usize) -> Self {
        let mut bounds = Self {
            left: Vec::new(),
            right: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
        };
        bounds.reset(owner_count);
        bounds
    }

    fn reset(&mut self, owner_count: usize) {
        self.left.resize(owner_count, f64::INFINITY);
        self.right.resize(owner_count, f64::NEG_INFINITY);
        self.top.resize(owner_count, f64::INFINITY);
        self.bottom.resize(owner_count, f64::NEG_INFINITY);

        self.left.fill(f64::INFINITY);
        self.right.fill(f64::NEG_INFINITY);
        self.top.fill(f64::INFINITY);
        self.bottom.fill(f64::NEG_INFINITY);
    }
}

impl SimGraph {
    const DEFAULT_EDGE_LENGTH: f64 = 50.0;
    const DEFAULT_SPRING_STRENGTH: f64 = 0.45;
    const DEFAULT_REPULSION_STRENGTH: f64 = 4500.0;
    const DEFAULT_GRAPH_MARGIN: f64 = 15.0; // layout-base `LayoutConstants.DEFAULT_GRAPH_MARGIN`
    const EMPTY_COMPOUND_NODE_SIZE: f64 = 40.0; // layout-base `LayoutConstants.EMPTY_COMPOUND_NODE_SIZE`

    fn from_indexed(graph: &IndexedGraph) -> Self {
        let leaf_count = graph.nodes.len();
        let compound_count = graph.compounds.len();
        let mut nodes: Vec<SimNode> = Vec::with_capacity(leaf_count + compound_count);

        for n in &graph.nodes {
            let w = n.width.max(1.0);
            let h = n.height.max(1.0);
            nodes.push(SimNode {
                parent: n.parent,
                owner_idx: usize::MAX,
                is_compound: false,
                width: w,
                height: h,
                bounds_extras: n.bounds_extras,
                estimated_size: 0.0,
                left: n.x - w / 2.0,
                top: n.y - h / 2.0,
                spring_fx: 0.0,
                spring_fy: 0.0,
                repulsion_fx: 0.0,
                repulsion_fy: 0.0,
                gravitation_fx: 0.0,
                gravitation_fy: 0.0,
                no_of_children: 1.0,
                padding: 0.0,
                surrounding: Vec::new(),
                grid_start_x: 0,
                grid_finish_x: 0,
                grid_start_y: 0,
                grid_finish_y: 0,
            });
        }

        let compound_parent: Vec<Option<usize>> =
            graph.compounds.iter().map(|c| c.parent).collect();

        // Materialize compound nodes as layout nodes (Cytoscape parent nodes).
        for c in &graph.compounds {
            nodes.push(SimNode {
                parent: c.parent,
                owner_idx: usize::MAX,
                is_compound: true,
                width: Self::EMPTY_COMPOUND_NODE_SIZE,
                height: Self::EMPTY_COMPOUND_NODE_SIZE,
                bounds_extras: BoundsExtras::default(),
                estimated_size: Self::EMPTY_COMPOUND_NODE_SIZE,
                left: 0.0,
                top: 0.0,
                spring_fx: 0.0,
                spring_fy: 0.0,
                repulsion_fx: 0.0,
                repulsion_fy: 0.0,
                gravitation_fx: 0.0,
                gravitation_fy: 0.0,
                no_of_children: 1.0,
                padding: 0.0,
                surrounding: Vec::new(),
                grid_start_x: 0,
                grid_finish_x: 0,
                grid_start_y: 0,
                grid_finish_y: 0,
            });
        }

        let mut edges: Vec<SimEdge> = Vec::new();
        for e in &graph.edges {
            if e.source >= leaf_count || e.target >= leaf_count || e.source == e.target {
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
                a: e.source,
                b: e.target,
                a_anchor: e.source_anchor,
                b_anchor: e.target_anchor,
                curve_style_segments: e.curve_style_segments,
                base_ideal_length: ideal.max(1.0),
                ideal_length: ideal.max(1.0),
                elasticity,
                label_width: e.label_width.filter(|v| v.is_finite() && *v > 0.0),
                label_height: e.label_height.filter(|v| v.is_finite() && *v > 0.0),
            });
        }

        let root_owner_idx = nodes.len();

        // Resolve owner graph identities (`node.getOwner()` in layout-base): nodes repel only
        // within the same owner graph (i.e. same parent compound).
        for n in &mut nodes {
            let owner_idx = n
                .parent
                .map(|p| leaf_count + p)
                .filter(|idx| *idx < root_owner_idx)
                .unwrap_or(root_owner_idx);
            n.owner_idx = owner_idx;
        }

        let mut children_by_owner: Vec<Vec<usize>> = vec![Vec::new(); nodes.len() + 1];
        // Preserve Cytoscape insertion order within each owner graph:
        // - parent (compound) nodes are created before non-parent nodes
        // - within each category, relative order follows Mermaid's `addGroups(...)` and
        //   `addServices/addJunctions(...)` array iteration order
        //
        // This ordering is observable in `graphManager.getGraphs()/getAllNodes()` iteration and
        // affects deterministic parity for FR-grid repulsion (processed set ordering).
        for compound_idx in 0..compound_count {
            let idx = leaf_count + compound_idx;
            let owner = nodes
                .get(idx)
                .map(|n| n.owner_idx)
                .unwrap_or(root_owner_idx);
            if owner < children_by_owner.len() {
                children_by_owner[owner].push(idx);
            }
        }
        for idx in 0..leaf_count {
            let owner = nodes
                .get(idx)
                .map(|n| n.owner_idx)
                .unwrap_or(root_owner_idx);
            if owner < children_by_owner.len() {
                children_by_owner[owner].push(idx);
            }
        }

        // Compute compound inclusion depths (root-level nodes depth=1), and build a stable
        // deepest-first compound node order for updateBounds.
        let mut inclusion_depth: Vec<usize> = vec![1; nodes.len()];
        fn depth_of(idx: usize, nodes: &[SimNode], memo: &mut [Option<usize>]) -> usize {
            if idx >= nodes.len() {
                return 1;
            }
            if let Some(v) = memo[idx] {
                return v;
            }

            let mut path: Vec<usize> = Vec::new();
            let mut cur = idx;
            let mut base_depth = 0usize;
            while cur < nodes.len() {
                if let Some(depth) = memo[cur] {
                    base_depth = depth;
                    break;
                }
                path.push(cur);
                if path.len() > nodes.len() {
                    base_depth = 0;
                    break;
                }
                let owner = nodes[cur].owner_idx;
                if owner >= nodes.len() {
                    base_depth = 0;
                    break;
                }
                cur = owner;
            }

            let mut depth = base_depth;
            while let Some(node_idx) = path.pop() {
                depth = depth.saturating_add(1);
                memo[node_idx] = Some(depth);
            }

            memo[idx].unwrap_or(1)
        }
        let mut memo: Vec<Option<usize>> = vec![None; nodes.len()];
        for i in 0..nodes.len() {
            inclusion_depth[i] = depth_of(i, &nodes, &mut memo);
        }
        let mut compounds_deep_first: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, n)| n.is_compound.then_some(idx))
            .collect();
        compounds_deep_first.sort_by_key(|&idx| std::cmp::Reverse(inclusion_depth[idx]));

        // `LNode.getNoOfChildren()` is a scalar subtree weight: leaves and empty compounds count
        // as one, while non-empty compounds sum the weights of their immediate children. Keep the
        // postorder scalar only; materializing every compound-to-leaf relation turns a deep
        // compound chain into quadratic setup memory and per-iteration displacement work.
        let mut no_of_children: Vec<usize> = vec![1; nodes.len()];
        for &cidx in &compounds_deep_first {
            let children = &children_by_owner[cidx];
            no_of_children[cidx] = children
                .iter()
                .map(|&child| no_of_children[child])
                .sum::<usize>()
                .max(1);
        }

        // Compute estimated sizes (used for gravity ranges, and to match layout-base defaults).
        let mut est_size: Vec<f64> = vec![0.0; nodes.len()];
        for idx in 0..nodes.len() {
            if !nodes[idx].is_compound {
                est_size[idx] = (nodes[idx].width + nodes[idx].height) / 2.0;
            }
        }
        // Deepest-first postorder (children first).
        for &cidx in &compounds_deep_first {
            let children = &children_by_owner[cidx];
            let sum: f64 = children.iter().map(|&ch| est_size[ch]).sum();
            let size = if children.is_empty() {
                Self::EMPTY_COMPOUND_NODE_SIZE
            } else {
                (sum / (children.len() as f64).sqrt()).max(1.0)
            };
            est_size[cidx] = size;
        }
        // layout-base `LNode.calcEstimatedSize()` also sets compound node `rect.width/height` to
        // the estimated size. This is later overwritten by `updateBounds()`, but it affects
        // early spring-embedder iterations (repulsion ranges, smart ideal edge length, etc.).
        for &cidx in &compounds_deep_first {
            let s = est_size[cidx].max(1.0);
            nodes[cidx].width = s;
            nodes[cidx].height = s;
        }

        for idx in 0..nodes.len() {
            nodes[idx].estimated_size = est_size[idx].max(1.0);
        }

        let mut owner_estimated_size: Vec<f64> =
            vec![Self::EMPTY_COMPOUND_NODE_SIZE; nodes.len() + 1];
        // For compound owners, estimated size is the compound node's estimated size.
        for &cidx in &compounds_deep_first {
            owner_estimated_size[cidx] = est_size[cidx].max(1.0);
        }
        // Root owner estimated size is computed from its immediate children.
        {
            let children = &children_by_owner[root_owner_idx];
            let sum: f64 = children.iter().map(|&ch| est_size[ch]).sum();
            owner_estimated_size[root_owner_idx] = if children.is_empty() {
                Self::EMPTY_COMPOUND_NODE_SIZE
            } else {
                (sum / (children.len() as f64).sqrt()).max(1.0)
            };
        }

        for (idx, n) in nodes.iter_mut().enumerate() {
            n.no_of_children = no_of_children[idx] as f64;
        }

        let compound_topology = CompoundTopology::build(
            &nodes,
            &edges,
            root_owner_idx,
            &children_by_owner,
            &inclusion_depth,
        );

        Self {
            nodes,
            edges,
            compound_topology,
            compound_parent,
            leaf_count,
            root_owner_idx,
            children_by_owner,
            surrounding_seen: vec![0; leaf_count + compound_count],
            compounds_deep_first,
            owner_estimated_size,
            inclusion_depth,
        }
    }

    fn update_bounds(&mut self, bounds: &mut OwnerBounds) {
        debug_assert_eq!(self.root_owner_idx, self.nodes.len());

        let owner_count = self.nodes.len() + 1;
        bounds.reset(owner_count);
        let left = &mut bounds.left;
        let right = &mut bounds.right;
        let top = &mut bounds.top;
        let bottom = &mut bounds.bottom;

        // Mirror layout-base `graphManager.updateBounds()`:
        // - update child compound bounds first
        // - then compute each graph bounds with a margin derived from parent compound padding
        for &cidx in &self.compounds_deep_first {
            let children = &self.children_by_owner[cidx];
            if children.is_empty() {
                // Empty compound: keep its current rect as-is.
                left[cidx] = self.nodes[cidx].left - self.nodes[cidx].padding;
                right[cidx] = self.nodes[cidx].right() + self.nodes[cidx].padding;
                top[cidx] = self.nodes[cidx].top - self.nodes[cidx].padding;
                bottom[cidx] = self.nodes[cidx].bottom() + self.nodes[cidx].padding;
                continue;
            }

            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for &ch in children {
                min_x = min_x.min(self.nodes[ch].left);
                min_y = min_y.min(self.nodes[ch].top);
                max_x = max_x.max(self.nodes[ch].right());
                max_y = max_y.max(self.nodes[ch].bottom());
            }
            let margin = self.nodes[cidx].padding.max(0.0);
            min_x -= margin;
            min_y -= margin;
            max_x += margin;
            max_y += margin;

            left[cidx] = min_x;
            right[cidx] = max_x;
            top[cidx] = min_y;
            bottom[cidx] = max_y;

            // Update compound node rect to wrap its child graph.
            self.nodes[cidx].left = min_x;
            self.nodes[cidx].top = min_y;
            self.nodes[cidx].width = (max_x - min_x).max(1.0);
            self.nodes[cidx].height = (max_y - min_y).max(1.0);
        }

        // Root graph bounds (margin defaults to LayoutConstants.DEFAULT_GRAPH_MARGIN).
        {
            let children = &self.children_by_owner[self.root_owner_idx];
            if children.is_empty() {
                left[self.root_owner_idx] = 0.0;
                right[self.root_owner_idx] = 0.0;
                top[self.root_owner_idx] = 0.0;
                bottom[self.root_owner_idx] = 0.0;
            } else {
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for &ch in children {
                    min_x = min_x.min(self.nodes[ch].left);
                    min_y = min_y.min(self.nodes[ch].top);
                    max_x = max_x.max(self.nodes[ch].right());
                    max_y = max_y.max(self.nodes[ch].bottom());
                }
                let margin = Self::DEFAULT_GRAPH_MARGIN;
                left[self.root_owner_idx] = min_x - margin;
                right[self.root_owner_idx] = max_x + margin;
                top[self.root_owner_idx] = min_y - margin;
                bottom[self.root_owner_idx] = max_y + margin;
            }
        }
    }

    fn all_nodes_layout_order(&self) -> Vec<usize> {
        // layout-base `graphManager.getAllNodes()` returns a flat list created by concatenating
        // `graph.getNodes()` over `graphManager.getGraphs()` in graph creation order. Graphs are
        // created recursively: root graph first, then each compound's child graph when that
        // compound node is encountered.
        //
        // Reconstruct that order by visiting owner graphs in pre-order, following the
        // `children_by_owner` inclusion tree.
        let mut out: Vec<usize> = Vec::with_capacity(self.nodes.len());
        let mut visited_graph: Vec<bool> = vec![false; self.nodes.len() + 1];
        let mut stack = vec![self.root_owner_idx];
        while let Some(owner) = stack.pop() {
            if owner >= visited_graph.len() {
                continue;
            }
            if std::mem::replace(&mut visited_graph[owner], true) {
                continue;
            }

            let nodes = self
                .children_by_owner
                .get(owner)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            for &idx in nodes {
                out.push(idx);
            }
            for &idx in nodes.iter().rev() {
                let is_compound = self.nodes.get(idx).is_some_and(|n| n.is_compound);
                let has_children = self
                    .children_by_owner
                    .get(idx)
                    .is_some_and(|v| !v.is_empty());
                if is_compound && has_children {
                    stack.push(idx);
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
enum FcoseRandom {
    XorShift64Star(XorShift64Star),
    Mulberry32(Mulberry32),
}

impl FcoseRandom {
    fn next_f64_signed(&mut self) -> f64 {
        (self.next_f64_unit() * 2.0) - 1.0
    }

    fn next_f64_unit(&mut self) -> f64 {
        match self {
            Self::XorShift64Star(rng) => rng.next_f64_unit(),
            Self::Mulberry32(rng) => rng.next_f64_unit(),
        }
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        let idx = (self.next_f64_unit() * (upper as f64)).floor() as usize;
        idx.min(upper - 1)
    }
}

fn new_fcose_random(policy: FcoseRandomPolicy) -> FcoseRandom {
    match policy.source {
        FcoseRandomSource::XorShift64Star => {
            FcoseRandom::XorShift64Star(XorShift64Star::new(policy.seed))
        }
        FcoseRandomSource::Mulberry32 => {
            FcoseRandom::Mulberry32(Mulberry32::new(policy.seed as u32))
        }
    }
}

fn advance_random(rng: &mut FcoseRandom, count: usize) {
    for _ in 0..count {
        let _ = rng.next_f64_unit();
    }
}

fn advance_random_with_work_control<W: WorkControl + ?Sized>(
    rng: &mut FcoseRandom,
    count: usize,
    work_control: &mut W,
) -> std::result::Result<(), WorkFailure> {
    if count > 0 {
        work_control.charge(count)?;
    }
    advance_random(rng, count);
    Ok(())
}

fn reset_fcose_random_for_run<W: WorkControl + ?Sized>(
    rng: &mut FcoseRandom,
    policy: FcoseRandomPolicy,
    run_idx: usize,
    seed_offset: usize,
    work_control: &mut W,
) -> std::result::Result<bool, WorkFailure> {
    if run_idx == 0 || !policy.reset_seed_each_run {
        return Ok(false);
    }
    // Admission precedes both resetting the stream and consuming the configured offset, so a
    // rejected rerun cannot partially mutate caller-observable deterministic state.
    if seed_offset > 0 {
        work_control.charge(seed_offset)?;
    }
    *rng = new_fcose_random(policy);
    advance_random(rng, seed_offset);
    Ok(true)
}

#[derive(Debug, Clone)]
struct XorShift64Star {
    state: u64,
}

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D_u64)
    }

    fn next_f64_unit(&mut self) -> f64 {
        // Map to [0, 1) with 53 bits of precision.
        let u = self.next_u64() >> 11;
        (u as f64) / ((1u64 << 53) as f64)
    }
}

#[derive(Debug, Clone)]
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_f64_unit(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        let mut value = self.state;
        value = (value ^ (value >> 15)).wrapping_mul(value | 1);
        value ^= value.wrapping_add((value ^ (value >> 7)).wrapping_mul(value | 61));
        let output = value ^ (value >> 14);
        f64::from(output) / 4_294_967_296.0
    }
}

#[cfg(test)]
mod tests;

#[inline]
fn rects_intersect(a: &SimNode, b: &SimNode) -> bool {
    a.rect().intersects(b.rect())
}

#[inline]
fn definitely_less(a: f64, b: f64) -> bool {
    a + GEOMETRY_EPSILON < b
}

#[inline]
fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= GEOMETRY_EPSILON
}

#[cfg(test)]
fn rect_clip_points(a: &SimNode, b: &SimNode) -> (f64, f64, f64, f64) {
    let (ax, ay, bx, by, overlapped) = rect_intersection_points(a, b);
    if overlapped {
        return (a.center_x(), a.center_y(), b.center_x(), b.center_y());
    }
    (ax, ay, bx, by)
}

#[cfg(test)]
#[inline]
fn rect_intersection_points(a: &SimNode, b: &SimNode) -> (f64, f64, f64, f64, bool) {
    geometry::clip_points(
        a.rect(),
        b.rect(),
        (a.center_x(), a.center_y()),
        (b.center_x(), b.center_y()),
    )
}

#[inline]
fn rect_intersection_points_no_overlap_check(a: &SimNode, b: &SimNode) -> (f64, f64, f64, f64) {
    geometry::clip_points_disjoint(
        a.rect(),
        b.rect(),
        (a.center_x(), a.center_y()),
        (b.center_x(), b.center_y()),
    )
}

#[inline]
fn calc_repulsion_force_overlapping(
    a: &SimNode,
    b: &SimNode,
    separation_buffer: f64,
    a_center_x: f64,
    a_center_y: f64,
    b_center_x: f64,
    b_center_y: f64,
) -> (f64, f64) {
    let (ox, oy) = calc_separation_amount_with_centers(
        a,
        b,
        separation_buffer,
        a_center_x,
        a_center_y,
        b_center_x,
        b_center_y,
    );
    let repulsion_fx = 2.0 * ox;
    let repulsion_fy = 2.0 * oy;

    // layout-base: scale overlap separation by a children constant so large compounds move
    // more slowly than leaves (and to reduce oscillation).
    let denom = (a.no_of_children + b.no_of_children).max(1.0);
    let children_constant = (a.no_of_children * b.no_of_children) / denom;

    // Return a force delta to be applied as:
    // - nodeA += rfx/rfy
    // - nodeB -= rfx/rfy
    (
        -children_constant * repulsion_fx,
        -children_constant * repulsion_fy,
    )
}

#[inline]
fn calc_repulsion_force_non_overlapping_from_points(
    min_repulsion_dist: f64,
    children_product: f64,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
) -> (f64, f64) {
    let mut dx = bx - ax;
    let mut dy = by - ay;

    if dx.abs() < min_repulsion_dist {
        dx = imath_sign(dx) * min_repulsion_dist;
    }
    if dy.abs() < min_repulsion_dist {
        dy = imath_sign(dy) * min_repulsion_dist;
    }

    let dist_sq = dx * dx + dy * dy;
    let dist = dist_sq.sqrt();
    if dist_sq == 0.0 {
        return (0.0, 0.0);
    }

    // layout-base:
    // `(nodeA.nodeRepulsion/2 + nodeB.nodeRepulsion/2) * noOfChildrenA * noOfChildrenB / dist^2`.
    // Default node repulsion is 4500 for both nodes.
    let repulsion_force = SimGraph::DEFAULT_REPULSION_STRENGTH * children_product / dist_sq;
    let repulsion_fx = repulsion_force * dx / dist;
    let repulsion_fy = repulsion_force * dy / dist;
    (-repulsion_fx, -repulsion_fy)
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn calc_repulsion_force(
    a: &SimNode,
    b: &SimNode,
    min_repulsion_dist: f64,
    separation_buffer: f64,
    a_center_x: f64,
    a_center_y: f64,
    b_center_x: f64,
    b_center_y: f64,
) -> (f64, f64) {
    calc_repulsion_force_with_centers(
        a,
        b,
        min_repulsion_dist,
        separation_buffer,
        a_center_x,
        a_center_y,
        b_center_x,
        b_center_y,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn calc_repulsion_force_with_centers(
    a: &SimNode,
    b: &SimNode,
    min_repulsion_dist: f64,
    separation_buffer: f64,
    a_center_x: f64,
    a_center_y: f64,
    b_center_x: f64,
    b_center_y: f64,
) -> (f64, f64) {
    if rects_intersect(a, b) {
        calc_repulsion_force_overlapping(
            a,
            b,
            separation_buffer,
            a_center_x,
            a_center_y,
            b_center_x,
            b_center_y,
        )
    } else {
        let children_product = a.no_of_children * b.no_of_children;
        let (ax, ay, bx, by) = rect_intersection_points_no_overlap_check(a, b);
        calc_repulsion_force_non_overlapping_from_points(
            min_repulsion_dist,
            children_product,
            ax,
            ay,
            bx,
            by,
        )
    }
}

#[cfg(test)]
fn calc_separation_amount(a: &SimNode, b: &SimNode, separation_buffer: f64) -> (f64, f64) {
    calc_separation_amount_with_centers(
        a,
        b,
        separation_buffer,
        a.center_x(),
        a.center_y(),
        b.center_x(),
        b.center_y(),
    )
}

#[inline]
fn calc_separation_amount_with_centers(
    a: &SimNode,
    b: &SimNode,
    separation_buffer: f64,
    a_center_x: f64,
    a_center_y: f64,
    b_center_x: f64,
    b_center_y: f64,
) -> (f64, f64) {
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

    let center_dx = b_center_x - a_center_x;
    let center_dy = b_center_y - a_center_y;
    let mut slope = (center_dy / center_dx).abs();
    if nearly_equal(center_dy, 0.0) && nearly_equal(center_dx, 0.0) {
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
    let dir_x = if definitely_less(a.center_x(), b.center_x()) {
        -1.0
    } else {
        1.0
    };
    let dir_y = if definitely_less(a.center_y(), b.center_y()) {
        -1.0
    } else {
        1.0
    };
    (dir_x, dir_y)
}
