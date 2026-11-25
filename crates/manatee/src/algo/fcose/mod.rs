#![allow(clippy::needless_range_loop)]

use crate::algo::FcoseOptions;
use crate::error::{Error, Result, WorkFailure};
use crate::graph::{Anchor, BoundsExtras, Graph, LayoutRect, LayoutResult, Point};
use crate::work::admit_dynamic_work;
use rustc_hash::FxHashMap;

mod constraint_projection;
mod constraints;
mod repulsion_grid;
mod spectral;
mod work_plan;

#[cfg(test)]
use constraint_projection::{
    Mat2, Vec2, apply_reflection_for_relative_placement, procrustes_transform_for_alignments,
};
use constraint_projection::{apply_constraints_to_displacements, handle_constraints_pre_layout};
#[cfg(test)]
use constraints::RelConstraint;
use constraints::{ConstraintRuntime, Constraints};
use repulsion_grid::RepulsionGrid;
#[cfg(test)]
use repulsion_grid::{
    RepulsionGridCells, RepulsionGridPlan, RepulsionGridStorageKind, implicit_grid_work_units,
};

pub use crate::work::{NoopWorkControl, WorkControl};
pub use work_plan::FcoseIterationSchedule;
use work_plan::{FcoseConstraintWorkShape, FcoseWorkPlan};

const GEOMETRY_EPSILON: f64 = 1e-9;

// Mermaid 11.16 leaves these Cytoscape 3.33.3 bbox phases at their defaults. Keep body, label,
// parent, and final antialiasing expansion separate because they do not share the same outset.
const CYTOSCAPE_EDGE_BODY_WIDTH_PX: f64 = 3.0;
const CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX: f64 = 2.0;
const CYTOSCAPE_PARENT_BODY_BORDER_WIDTH_PX: f64 = 1.0;
const CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX: f64 = 1.0;
const CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX: f64 = CYTOSCAPE_EDGE_BODY_WIDTH_PX / 2.0;
const CYTOSCAPE_PARENT_BODY_NON_PADDING_BBOX_OUTSET_PX: f64 =
    CYTOSCAPE_PARENT_BODY_BORDER_WIDTH_PX / 2.0 + CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX;

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
    /// Alignment groups use FCoSE element indices: leaves first, then compounds.
    pub alignment_constraint: Option<IndexedAlignmentConstraint>,
    /// Relative constraints use FCoSE element indices: leaves first, then compounds.
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
    pub gap: f64,
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

    let shape = FcoseConstraintWorkShape::from_indexed_options(
        opts,
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
    let work_plan = FcoseWorkPlan::from_schedule(
        schedule,
        node_count,
        shape,
        random_seed_offset,
        random_policy.reset_seed_each_run,
    )?;
    // Check 3 admits the complete predictable schedule, including compound-topology construction,
    // before SimGraph/Constraints allocation. Spectral convergence and relative-projection clone
    // work remain exact dynamic charges.
    work_control.check(work_plan.maximum_work_units())?;
    work_control.charge(work_plan.setup_work_units())?;
    let mut sim = SimGraph::from_indexed(graph);

    let constraints = Constraints::from_indexed_opts(&sim, opts);
    constraints.validate(sim.nodes.len())?;

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

    fn bound_left(&self) -> f64 {
        self.left - self.bounds_extras.left.max(0.0)
    }

    fn bound_right(&self) -> f64 {
        self.right() + self.bounds_extras.right.max(0.0)
    }

    fn bound_top(&self) -> f64 {
        self.top - self.bounds_extras.top.max(0.0)
    }

    fn bound_bottom(&self) -> f64 {
        self.bottom() + self.bounds_extras.bottom.max(0.0)
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

/// The static compound graph projection used by the CoSE kernel.
///
/// Cytoscape/layout-base computes each edge's lowest common ancestor and the two immediate
/// children below that ancestor before the spring embedder starts.  The previous Rust path
/// rebuilt a temporary mark array for every edge on every rerun and independently rescanned all
/// owner graphs to decide where gravity applies.  Keep those source-backed facts together so the
/// hierarchy is built once and reused by both runs and every iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EdgeProjection {
    lca_owner_idx: usize,
    source_in_lca: usize,
    target_in_lca: usize,
}

#[derive(Debug, Clone)]
struct CompoundTopology {
    edge_projections: Vec<EdgeProjection>,
    owner_connected: Vec<bool>,
}

impl CompoundTopology {
    /// Return a conservative setup tranche for the temporary binary-lifting table, edge
    /// projections, and one global connectivity union-find.
    ///
    /// This count-only entry point assumes the deepest possible compound chain. Callers that know
    /// the compound cardinality should use [`Self::work_units_with_compound_count`] to avoid
    /// charging a flat graph for a `log V` ancestry table.
    #[cfg(test)]
    fn work_units(node_count: usize, edge_count: usize) -> std::result::Result<usize, WorkFailure> {
        Self::work_units_with_compound_count(node_count, edge_count, node_count)
    }

    /// Return a conservative topology tranche for a known compound cardinality.
    ///
    /// Ancestry projection is O((V + E) log D), where `D <= compounds + 1`, while union-find
    /// parent scans retain an independent O((V + E) log V) worst-case bound. Peak temporary space
    /// is O(V log D + E), and retained topology is O(V + E) after the ancestry table is dropped.
    fn work_units_with_compound_count(
        node_count: usize,
        edge_count: usize,
        compound_count: usize,
    ) -> std::result::Result<usize, WorkFailure> {
        if node_count == 0 {
            return Ok(0);
        }

        let owner_count = node_count
            .checked_add(1)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let max_inclusion_depth = if compound_count >= node_count {
            node_count
        } else {
            compound_count + 1
        };
        let ancestry_levels = Self::lifting_level_count(max_inclusion_depth);
        let lifting_slots = owner_count
            .checked_mul(ancestry_levels)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let edge_projection_units = edge_count
            .checked_mul(
                ancestry_levels
                    .checked_mul(4)
                    .and_then(|units| units.checked_add(4))
                    .ok_or(WorkFailure::ArithmeticOverflow)?,
            )
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let find_work_units = Self::dsu_find_work_units(node_count, edge_count)?;
        let connectivity_units = owner_count
            .checked_add(node_count)
            .and_then(|units| units.checked_add(find_work_units))
            .ok_or(WorkFailure::ArithmeticOverflow)?;

        // Parent/depth initialization and the lifting table itself are both charged.  The
        // connectivity term covers DSU initialization, owner scans, and `levels(V)` parent-slot
        // visits for each of at most `2E + V` find calls. Union-by-size bounds the uncompressed
        // parent height; path halving can only reduce the executed work.
        owner_count
            .checked_mul(2)
            .and_then(|units| units.checked_add(lifting_slots))
            .and_then(|units| units.checked_add(edge_projection_units))
            .and_then(|units| units.checked_add(connectivity_units))
            .ok_or(WorkFailure::ArithmeticOverflow)
    }

    fn dsu_find_work_units(
        node_count: usize,
        edge_count: usize,
    ) -> std::result::Result<usize, WorkFailure> {
        let find_call_count = edge_count
            .checked_mul(2)
            .and_then(|calls| calls.checked_add(node_count))
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        find_call_count
            .checked_mul(Self::lifting_level_count(node_count))
            .ok_or(WorkFailure::ArithmeticOverflow)
    }

    fn lifting_level_count(cardinality: usize) -> usize {
        if cardinality <= 1 {
            1
        } else {
            (usize::BITS - cardinality.leading_zeros()) as usize
        }
    }

    fn build(
        nodes: &[SimNode],
        edges: &[SimEdge],
        root_owner_idx: usize,
        children_by_owner: &[Vec<usize>],
        inclusion_depth: &[usize],
    ) -> Self {
        let node_count = nodes.len();
        if node_count == 0 {
            return Self {
                edge_projections: Vec::new(),
                owner_connected: vec![true; children_by_owner.len()],
            };
        }

        let max_inclusion_depth = inclusion_depth.iter().copied().max().unwrap_or(1);
        let levels = Self::lifting_level_count(max_inclusion_depth);
        let owner_count = node_count + 1;
        let mut depth = vec![0usize; owner_count];
        for (idx, value) in inclusion_depth.iter().copied().enumerate().take(node_count) {
            depth[idx] = value;
        }

        let mut first_ancestor = vec![root_owner_idx; owner_count];
        for (idx, node) in nodes.iter().enumerate() {
            first_ancestor[idx] = node.owner_idx.min(root_owner_idx);
        }
        first_ancestor[root_owner_idx] = root_owner_idx;
        let mut ancestors = Vec::with_capacity(levels);
        ancestors.push(first_ancestor);
        for level in 1..levels {
            let previous = &ancestors[level - 1];
            let mut current = vec![root_owner_idx; owner_count];
            for idx in 0..owner_count {
                let parent = previous[idx];
                current[idx] = previous.get(parent).copied().unwrap_or(root_owner_idx);
            }
            ancestors.push(current);
        }

        let ancestry = CompoundAncestry {
            root_owner_idx,
            depth,
            ancestors,
        };
        let edge_projections = edges
            .iter()
            .map(|edge| ancestry.project_edge(edge.a, edge.b))
            .collect::<Vec<_>>();
        let owner_connected = Self::connected_owners(nodes, &edge_projections, children_by_owner);

        Self {
            edge_projections,
            owner_connected,
        }
    }

    fn connected_owners(
        nodes: &[SimNode],
        edge_projections: &[EdgeProjection],
        children_by_owner: &[Vec<usize>],
    ) -> Vec<bool> {
        let mut dsu = DisjointSet::new(nodes.len());
        for projection in edge_projections {
            let source = projection.source_in_lca;
            let target = projection.target_in_lca;
            if source >= nodes.len() || target >= nodes.len() || source == target {
                continue;
            }
            // An LCA projection is always made of immediate children of that owner graph.  The
            // guard keeps malformed private test fixtures fail-closed without changing public
            // graph validation behavior.
            if nodes[source].owner_idx == projection.lca_owner_idx
                && nodes[target].owner_idx == projection.lca_owner_idx
            {
                dsu.union(source, target);
            }
        }

        let mut connected = vec![true; children_by_owner.len()];
        for (owner, children) in children_by_owner.iter().enumerate() {
            let Some((&first, rest)) = children.split_first() else {
                continue;
            };
            let representative = dsu.find(first);
            for &child in rest {
                if dsu.find(child) != representative {
                    connected[owner] = false;
                    break;
                }
            }
        }
        connected
    }
}

#[derive(Debug, Clone)]
struct CompoundAncestry {
    root_owner_idx: usize,
    depth: Vec<usize>,
    ancestors: Vec<Vec<usize>>,
}

impl CompoundAncestry {
    fn parent_of(&self, node_idx: usize) -> usize {
        self.ancestors
            .first()
            .and_then(|level| level.get(node_idx).copied())
            .unwrap_or(self.root_owner_idx)
    }

    fn lift(&self, mut node_idx: usize, mut distance: usize) -> usize {
        let mut level = 0usize;
        while distance != 0 {
            if distance & 1 == 1 {
                node_idx = self
                    .ancestors
                    .get(level)
                    .and_then(|table| table.get(node_idx).copied())
                    .unwrap_or(self.root_owner_idx);
            }
            distance >>= 1;
            level = level.saturating_add(1);
        }
        node_idx
    }

    fn lca(&self, mut first: usize, mut second: usize) -> usize {
        first = first.min(self.root_owner_idx);
        second = second.min(self.root_owner_idx);
        if self.depth[first] < self.depth[second] {
            std::mem::swap(&mut first, &mut second);
        }
        first = self.lift(first, self.depth[first].saturating_sub(self.depth[second]));
        if first == second {
            return first;
        }
        for table in self.ancestors.iter().rev() {
            let first_parent = table.get(first).copied().unwrap_or(self.root_owner_idx);
            let second_parent = table.get(second).copied().unwrap_or(self.root_owner_idx);
            if first_parent != second_parent {
                first = first_parent;
                second = second_parent;
            }
        }
        self.parent_of(first)
    }

    fn child_below(&self, node_idx: usize, ancestor: usize) -> usize {
        let node_idx = node_idx.min(self.root_owner_idx);
        if self.parent_of(node_idx) == ancestor {
            return node_idx;
        }
        let desired_depth = self.depth[ancestor].saturating_add(1);
        let distance = self.depth[node_idx].saturating_sub(desired_depth);
        self.lift(node_idx, distance)
    }

    fn project_edge(&self, source: usize, target: usize) -> EdgeProjection {
        let source = source.min(self.root_owner_idx);
        let target = target.min(self.root_owner_idx);
        let source_owner = self.parent_of(source);
        let target_owner = self.parent_of(target);
        let lca_owner_idx = self.lca(source_owner, target_owner);
        let source_in_lca = if source_owner == lca_owner_idx {
            source
        } else {
            self.child_below(source, lca_owner_idx)
        };
        let target_in_lca = if target_owner == lca_owner_idx {
            target
        } else {
            self.child_below(target, lca_owner_idx)
        };
        EdgeProjection {
            lca_owner_idx,
            source_in_lca,
            target_in_lca,
        }
    }
}

#[derive(Debug, Clone)]
struct DisjointSet {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl DisjointSet {
    fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
            size: vec![1; len],
        }
    }

    fn find(&mut self, mut node: usize) -> usize {
        while self.parent.get(node).copied().unwrap_or(node) != node {
            let parent = self.parent[node];
            let grandparent = self.parent.get(parent).copied().unwrap_or(parent);
            self.parent[node] = grandparent;
            node = grandparent;
        }
        node
    }

    fn union(&mut self, first: usize, second: usize) {
        if first >= self.parent.len() || second >= self.parent.len() {
            return;
        }
        let mut first_root = self.find(first);
        let mut second_root = self.find(second);
        if first_root == second_root {
            return;
        }
        if self.size[first_root] < self.size[second_root] {
            std::mem::swap(&mut first_root, &mut second_root);
        }
        self.parent[second_root] = first_root;
        self.size[first_root] = self.size[first_root].saturating_add(self.size[second_root]);
    }

    #[cfg(test)]
    fn parent_depth(&self, mut node: usize) -> usize {
        let mut depth = 0usize;
        while self
            .parent
            .get(node)
            .copied()
            .is_some_and(|parent| parent != node)
        {
            node = self.parent[node];
            depth += 1;
            assert!(depth <= self.parent.len(), "disjoint-set parent cycle");
        }
        depth
    }
}

fn owner_local_pair_work(
    children_by_owner: &[Vec<usize>],
) -> std::result::Result<usize, WorkFailure> {
    children_by_owner
        .iter()
        .try_fold(0usize, |work, children| {
            let pairs = children
                .len()
                .checked_mul(children.len().saturating_sub(1))?
                / 2;
            work.checked_add(pairs)
        })
        .ok_or(WorkFailure::ArithmeticOverflow)
}

fn for_each_owner_local_pair(
    children_by_owner: &[Vec<usize>],
    mut visit: impl FnMut(usize, usize),
) {
    for children in children_by_owner {
        for (offset, &first) in children.iter().enumerate() {
            for &second in &children[offset + 1..] {
                visit(first, second);
            }
        }
    }
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
    // cytoscape-fcose default (overrides layout-base default 0.4 via `options.gravity`).
    const DEFAULT_GRAVITY_STRENGTH: f64 = 0.25;
    const DEFAULT_COMPOUND_GRAVITY_STRENGTH: f64 = 1.0; // layout-base `FDLayoutConstants.DEFAULT_COMPOUND_GRAVITY_STRENGTH`
    const DEFAULT_GRAVITY_RANGE_FACTOR: f64 = 3.8; // layout-base `FDLayoutConstants.DEFAULT_GRAVITY_RANGE_FACTOR`
    const DEFAULT_COMPOUND_GRAVITY_RANGE_FACTOR: f64 = 1.5; // layout-base `FDLayoutConstants.DEFAULT_COMPOUND_GRAVITY_RANGE_FACTOR`
    const DEFAULT_GRAPH_MARGIN: f64 = 15.0; // layout-base `LayoutConstants.DEFAULT_GRAPH_MARGIN`
    const EMPTY_COMPOUND_NODE_SIZE: f64 = 40.0; // layout-base `LayoutConstants.EMPTY_COMPOUND_NODE_SIZE`
    const SIMPLE_NODE_SIZE: f64 = 40.0; // layout-base `LayoutConstants.SIMPLE_NODE_SIZE`
    const PER_LEVEL_IDEAL_EDGE_LENGTH_FACTOR: f64 = 0.1; // layout-base `FDLayoutConstants.PER_LEVEL_IDEAL_EDGE_LENGTH_FACTOR`
    const DEFAULT_COOLING_FACTOR_INCREMENTAL: f64 = 0.3; // layout-base `FDLayoutConstants.DEFAULT_COOLING_FACTOR_INCREMENTAL`
    const FINAL_TEMPERATURE: f64 = 0.04; // cose-base `CoSELayout.initSpringEmbedder()`
    const GRID_CALCULATION_CHECK_PERIOD: usize = 10; // layout-base `FDLayoutConstants.GRID_CALCULATION_CHECK_PERIOD`

    const CONVERGENCE_CHECK_PERIOD: usize = 100;
    const MAX_NODE_DISPLACEMENT_INCREMENTAL: f64 = 100.0; // layout-base `FDLayoutConstants.MAX_NODE_DISPLACEMENT_INCREMENTAL`

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

    fn translate(&mut self, dx: f64, dy: f64) {
        for n in &mut self.nodes {
            n.left += dx;
            n.top += dy;
        }
    }

    fn bounding_box_center_rects(&self) -> Option<(f64, f64)> {
        self.layout_rect_bbox()
            .map(|r| (r.left + (r.width / 2.0), r.top + (r.height / 2.0)))
    }

    fn layout_rect_bbox(&self) -> Option<LayoutRect> {
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
        Some(LayoutRect {
            left: min_x,
            top: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    fn bounding_box_center_eles(&self, run_idx: usize) -> Option<(f64, f64)> {
        if self.nodes.is_empty() {
            return None;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        // Nodes (incl. labels). Cytoscape `eles.boundingBox()` treats compound nodes as wrappers
        // around their child graphs, and `compound-sizing-wrt-labels: include` causes descendant
        // label extents to affect compound bounds.
        //
        // Model this by:
        // - using leaf `bound_*` as the base primitive; Architecture supplies compound-child
        //   extras for grouped leaves and final-element extras for top-level leaves
        // - computing compound bboxes bottom-up from immediate children (so compound padding
        //   stacks across deep nesting, as observed in Mermaid/Cytoscape)
        // - applying layout-base/cose-base child-graph padding, then Cytoscape's centered parent
        //   half-border and final whole-bbox expansion
        //
        // This keeps layout rects (used by the spring embedder) unchanged while making the
        // relocation origin (`eles.boundingBox()` center) match upstream.
        #[derive(Debug, Clone, Copy)]
        struct Bb {
            x1: f64,
            y1: f64,
            x2: f64,
            y2: f64,
        }

        impl Bb {
            fn union(self, other: Bb) -> Bb {
                Bb {
                    x1: self.x1.min(other.x1),
                    y1: self.y1.min(other.y1),
                    x2: self.x2.max(other.x2),
                    y2: self.y2.max(other.y2),
                }
            }

            fn inflate(self, pad: f64) -> Bb {
                Bb {
                    x1: self.x1 - pad,
                    y1: self.y1 - pad,
                    x2: self.x2 + pad,
                    y2: self.y2 + pad,
                }
            }

            fn centered(x: f64, y: f64, outset: f64) -> Bb {
                Bb {
                    x1: x - outset,
                    y1: y - outset,
                    x2: x + outset,
                    y2: y + outset,
                }
            }
        }

        fn leaf_bbox(n: &SimNode) -> Bb {
            let x1 = n.bound_left();
            let y1 = n.bound_top();
            let x2 = n.bound_right();
            let y2 = n.bound_bottom();
            Bb { x1, y1, x2, y2 }
        }

        let mut bbs: Vec<Option<Bb>> = vec![None; self.nodes.len()];
        for (idx, n) in self.nodes.iter().enumerate() {
            if !n.is_compound {
                bbs[idx] = Some(leaf_bbox(n));
            }
        }

        for &cidx in &self.compounds_deep_first {
            let Some(n) = self.nodes.get(cidx) else {
                continue;
            };
            if !n.is_compound {
                continue;
            }
            let children = self
                .children_by_owner
                .get(cidx)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            if children.is_empty() {
                // Empty compound: fall back to its rect (no label extras tracked for compounds).
                bbs[cidx] = Some(Bb {
                    x1: n.left,
                    y1: n.top,
                    x2: n.right(),
                    y2: n.bottom(),
                });
                continue;
            }
            let mut bb: Option<Bb> = None;
            for &ch in children {
                let ch_bb = bbs.get(ch).and_then(|v| *v).unwrap_or_else(|| {
                    let Some(cn) = self.nodes.get(ch) else {
                        return Bb {
                            x1: 0.0,
                            y1: 0.0,
                            x2: 0.0,
                            y2: 0.0,
                        };
                    };
                    Bb {
                        x1: cn.left,
                        y1: cn.top,
                        x2: cn.right(),
                        y2: cn.bottom(),
                    }
                });
                bb = Some(bb.map(|b| b.union(ch_bb)).unwrap_or(ch_bb));
            }
            let compound_bbox_outset =
                n.padding.max(0.0) + CYTOSCAPE_PARENT_BODY_NON_PADDING_BBOX_OUTSET_PX;
            bbs[cidx] = bb.map(|b| b.inflate(compound_bbox_outset));
        }

        let top_level = self
            .children_by_owner
            .get(self.root_owner_idx)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        for &idx in top_level {
            let Some(bb) = bbs.get(idx).and_then(|v| *v).or_else(|| {
                self.nodes.get(idx).map(|n| Bb {
                    x1: n.left,
                    y1: n.top,
                    x2: n.right(),
                    y2: n.bottom(),
                })
            }) else {
                continue;
            };
            min_x = min_x.min(bb.x1);
            min_y = min_y.min(bb.y1);
            max_x = max_x.max(bb.x2);
            max_y = max_y.max(bb.y2);
        }

        // Edges: Cytoscape `eles.boundingBox()` includes edge geometry. For Mermaid Architecture,
        // edge endpoints are manually specified as `{ 0/50%/100% }` offsets (see
        // `source-endpoint`/`target-endpoint` in Mermaid's Cytoscape stylesheet).
        //
        // Cytoscape resolves those endpoints by adding the pixel offset to `node.position()`
        // (even though `position()` represents the node center for normal geometry). Mermaid then
        // reuses that same `position()` value as the SVG top-left `translate(x,y)` for the icon.
        //
        // In our port we mirror upstream by treating `SimNode.center_{x,y}` as that top-left
        // anchor, and compute endpoint points as offsets from it (not as shape intersections).
        fn endpoint(n: &SimNode, anchor: Option<Anchor>) -> (f64, f64) {
            let ox = n.center_x();
            let oy = n.center_y();
            let w = n.width;
            let h = n.height;
            match anchor {
                Some(Anchor::Left) => (ox, oy + (h / 2.0)),
                Some(Anchor::Right) => (ox + w, oy + (h / 2.0)),
                Some(Anchor::Top) => (ox + (w / 2.0), oy),
                Some(Anchor::Bottom) => (ox + (w / 2.0), oy + h),
                None => (ox + (w / 2.0), oy + (h / 2.0)),
            }
        }

        fn polyline_midpoint(points: &[(f64, f64)]) -> Option<(f64, f64)> {
            if points.len() < 2 {
                return None;
            }
            let mut total = 0.0f64;
            let mut seg_lens: Vec<f64> = Vec::with_capacity(points.len().saturating_sub(1));
            for w in points.windows(2) {
                let dx = w[1].0 - w[0].0;
                let dy = w[1].1 - w[0].1;
                let len = (dx * dx + dy * dy).sqrt();
                seg_lens.push(len);
                total += len;
            }
            if !total.is_finite() || total <= 0.0 {
                return Some(points[0]);
            }
            let target = total / 2.0;
            let mut acc = 0.0f64;
            for (i, &len) in seg_lens.iter().enumerate() {
                if !len.is_finite() || len <= 0.0 {
                    continue;
                }
                if acc + len >= target {
                    let t = ((target - acc) / len).clamp(0.0, 1.0);
                    let (x0, y0) = points[i];
                    let (x1, y1) = points[i + 1];
                    return Some((x0 + (x1 - x0) * t, y0 + (y1 - y0) * t));
                }
                acc += len;
            }
            points.last().copied()
        }

        for e in &self.edges {
            let Some(a) = self.nodes.get(e.a) else {
                continue;
            };
            let Some(b) = self.nodes.get(e.b) else {
                continue;
            };
            let (sx, sy) = endpoint(a, e.a_anchor);
            let (tx, ty) = endpoint(b, e.b_anchor);

            let mut path_points: Vec<(f64, f64)> = vec![(sx, sy), (tx, ty)];
            let mut label_point_override: Option<(f64, f64)> = None;
            let mut edge_bounds = Bb::centered(sx, sy, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX)
                .union(Bb::centered(tx, ty, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX));

            // Mermaid styles XY edges as Cytoscape `curve-style: segments` with
            // `segment-weights: 0` and `segment-distances: 0.5px` in the pre-layout state. Other
            // diagonal edges remain `curve-style: straight`; their labels stay at the straight
            // midpoint even after Mermaid writes segment weights/distances during run chaining.
            if e.curve_style_segments && run_idx == 0 && sx != tx && sy != ty {
                const SEG_DIST: f64 = 0.5;
                let dx = tx - sx;
                let dy = ty - sy;
                let len = (dx * dx + dy * dy).sqrt();
                if len.is_finite() && len > 0.0 {
                    // Left-hand perpendicular, normalized.
                    let off_x = (-dy / len) * SEG_DIST;
                    let off_y = (dx / len) * SEG_DIST;
                    // `segment-weights: 0` => base point at source endpoint.
                    let px = sx + off_x;
                    let py = sy + off_y;
                    path_points.insert(1, (px, py));
                    // Cytoscape's pre-layout `segments` curve places the edge label near the
                    // segment control point. Using that point for bbox purposes matches the
                    // upstream `edge.boundingBox()` extents for diagonal Architecture edges.
                    label_point_override = Some((px, py));
                    edge_bounds =
                        edge_bounds.union(Bb::centered(px, py, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX));
                }
            }

            // After the first run, Mermaid updates segment weights/distances for `edge.segments`
            // so XY edges become orthogonal with a single bend at either `(sx, ty)` or `(tx, sy)`.
            if e.curve_style_segments && run_idx > 0 && sx != tx && sy != ty {
                let (bx, by) = match e.a_anchor {
                    Some(Anchor::Top) | Some(Anchor::Bottom) => (sx, ty),
                    _ => (tx, sy),
                };
                path_points.insert(1, (bx, by));
                // After Mermaid updates segment weights/distances, the SVG renderer treats this
                // bend point as the "midpoint" of the orthogonal polyline. Cytoscape's edge label
                // placement for the same style is closest to this bend as well, so retain it as
                // this relocation bbox phase's label point.
                label_point_override = Some((bx, by));
                edge_bounds =
                    edge_bounds.union(Bb::centered(bx, by, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX));
            }

            // Edge labels: Cytoscape includes label geometry inside `edge.boundingBox()`, and
            // `eles.boundingBox()` unions it into the overall component bbox.
            if let (Some(lw), Some(lh)) = (e.label_width, e.label_height) {
                let lw = lw.max(0.0);
                let lh = lh.max(0.0);
                if lw.is_finite() && lw > 0.0 && lh.is_finite() && lh > 0.0 {
                    let mp = label_point_override.or_else(|| polyline_midpoint(&path_points));
                    if let Some((mx, my)) = mp {
                        let hw = lw / 2.0;
                        let hh = lh / 2.0;
                        edge_bounds = edge_bounds.union(Bb {
                            x1: mx - hw - CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                            y1: my - hh - CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                            x2: mx + hw + CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                            y2: my + hh + CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                        });
                    }
                }
            }

            // Cytoscape expands the completed edge element bbox after body and label union.
            let edge_bounds = edge_bounds.inflate(CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX);
            min_x = min_x.min(edge_bounds.x1);
            min_y = min_y.min(edge_bounds.y1);
            max_x = max_x.max(edge_bounds.x2);
            max_y = max_y.max(edge_bounds.y2);
        }

        if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
            return None;
        }
        Some(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
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

    fn calculate_displacements(
        &self,
        all_nodes_in_layout_order: &[usize],
        cooling_factor: f64,
        max_displacement: f64,
        displacements: &mut [(f64, f64)],
        propagated_by_owner: &mut [(f64, f64)],
    ) {
        displacements.fill((0.0, 0.0));
        propagated_by_owner.fill((0.0, 0.0));

        // Port `CoSELayout.moveNodes()` and `CoSENode.calculateDisplacement()` without
        // materializing every compound-to-leaf relationship. Upstream processes owner graphs in
        // pre-order: a compound propagates its clamped displacement recursively to leaf
        // descendants, while nested compounds do not inherit that displacement themselves.
        // `propagated_by_owner` stores the ordered sum of ancestor compound displacements for the
        // leaves in each child graph, reducing propagation from O(sum(descendant leaves)) to O(N).
        for &idx in all_nodes_in_layout_order {
            let Some(node) = self.nodes.get(idx) else {
                continue;
            };

            let is_non_empty_compound = node.is_compound
                && self
                    .children_by_owner
                    .get(idx)
                    .is_some_and(|children| !children.is_empty());
            let inherited = if is_non_empty_compound {
                (0.0, 0.0)
            } else {
                propagated_by_owner
                    .get(node.owner_idx)
                    .copied()
                    .unwrap_or((0.0, 0.0))
            };
            let denominator = node.no_of_children.max(1.0);
            let mut dx = inherited.0
                + cooling_factor * (node.spring_fx + node.repulsion_fx + node.gravitation_fx)
                    / denominator;
            let mut dy = inherited.1
                + cooling_factor * (node.spring_fy + node.repulsion_fy + node.gravitation_fy)
                    / denominator;

            if dx.abs() > max_displacement {
                dx = max_displacement * imath_sign(dx);
            }
            if dy.abs() > max_displacement {
                dy = max_displacement * imath_sign(dy);
            }
            if let Some(slot) = displacements.get_mut(idx) {
                *slot = (dx, dy);
            }

            if !is_non_empty_compound {
                continue;
            }

            let ancestor_displacement = propagated_by_owner
                .get(node.owner_idx)
                .copied()
                .unwrap_or((0.0, 0.0));
            if let Some(slot) = propagated_by_owner.get_mut(idx) {
                slot.0 = ancestor_displacement.0 + dx;
                slot.1 = ancestor_displacement.1 + dy;
            }
        }
    }

    fn run_spring_embedder<W: WorkControl + ?Sized>(
        &mut self,
        context: SpringEmbedderContext<'_, W>,
    ) -> std::result::Result<(), WorkFailure> {
        let SpringEmbedderContext {
            constraints,
            options: opts,
            work_plan,
            rng,
            owner_bounds,
            spectral_topology,
            work_control,
        } = context;
        let schedule = work_plan.schedule();

        // `cytoscape-fcose` constructs a fresh CoSELayout for every `layout.run()`. Keep the
        // generation-based FR-grid deduplication scratch run-local as well; reusing markers while
        // restarting the generation counter can silently drop repulsion pairs on the second run.
        self.surrounding_seen.fill(0);

        if self.nodes.is_empty() {
            return Ok(());
        }

        // Recompute per-edge ideal lengths (layout-base `FDLayout.calcIdealEdgeLengths`).
        // This must be re-applied on each run because Mermaid runs FCoSE twice.
        //
        // Important: the "global" default edge length constant (used for several heuristics) is
        // derived from the *base* `idealEdgeLength` option before the smart inter-graph
        // adjustments are applied. Reset first so the second run starts from the same baseline.
        self.reset_edge_ideal_lengths();

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

        self.adjust_intergraph_ideal_edge_lengths();
        // CoSE updates `MIN_REPULSION_DIST` based on the effective `DEFAULT_EDGE_LENGTH` when
        // `idealEdgeLength` is set. For Mermaid Architecture this is always set (as a function),
        // so we scale the minimum repulsion distance with the average ideal length.
        let min_repulsion_dist = (default_edge_length / 10.0).max(0.0005);

        // Apply uniform compound padding (Cytoscape style `padding`).
        let compound_padding = opts.compound_padding.unwrap_or(0.0).max(0.0);
        for n in &mut self.nodes {
            if n.is_compound {
                n.padding = compound_padding;
            }
        }

        // FCoSE performs a spectral initialization when `randomize=true`. Mermaid 11.15 sets
        // Architecture's default to `false`, while cytoscape-fcose's library default is `true`.
        let mut spectral_applied = false;
        if let Some(spectral_topology) = spectral_topology {
            let node_separation = opts
                .node_separation
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(75.0);
            spectral_applied = spectral::apply_spectral_start_positions(
                &mut self.nodes[..self.leaf_count],
                &self.edges,
                spectral_topology,
                node_separation,
                rng,
                work_control,
            )?;
        }

        let gravity_constant = Self::DEFAULT_GRAVITY_STRENGTH;

        // Upstream CoSE applies gravitational forces only to nodes that belong to a *disconnected*
        // owner graph (see `CoSELayout.calculateNodesToApplyGravitationTo()`), where "connected"
        // is evaluated in the compound graph using `LEdge.getOtherEndInGraph(...)` (i.e. edges
        // incident to descendants can connect immediate children via ancestor lifting).
        //
        // Applying gravity to all nodes (a common simplification) makes sparse cross-compound
        // Architecture graphs significantly more compact than Mermaid/Cytoscape, which in turn
        // changes the root `viewBox/max-width` in parity-root comparisons.
        // Match `cose-base` repulsion cutoff (`CoSELayout.calcRepulsionRange()`):
        //
        // `repulsionRange = 2 * (level + 1) * idealEdgeLength`
        //
        // Cytoscape FCoSE runs the compound graph in a single CoSE pass with `level=0`, so this
        // reduces to `2 * idealEdgeLength`.
        let repulsion_range = (2.0 * default_edge_length).max(1.0);

        // layout-base uses the FR-grid repulsion variant by default, which caches each node's
        // surrounding set and refreshes it every `GRID_CALCULATION_CHECK_PERIOD` iterations.
        let mut repulsion_grid: Option<RepulsionGrid> = None;

        // Fallback for degenerate cases where spectral is skipped (e.g. very small graphs).
        if opts.randomize && self.edges.is_empty() && !spectral_applied {
            self.collapse_start_positions(default_edge_length, rng);
        }

        // Upstream `cose-base` runs a dedicated constraint handler before the spring embedder.
        // This can rotate/reflect the draft layout and enforce alignment/relative-placement
        // constraints in position space, which strongly affects overall orientation and the
        // parity-root root viewport.
        let has_constraints = !constraints.align_horizontal.is_empty()
            || !constraints.align_vertical.is_empty()
            || !constraints.relative.is_empty();
        if has_constraints {
            work_control.charge(work_plan.run_setup_work_units())?;
            handle_constraints_pre_layout(
                &mut self.nodes[..self.leaf_count],
                constraints,
                work_control,
            )?;
        }

        let mut constraint_rt = ConstraintRuntime::new(&self.nodes, constraints);

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
        let max_iterations = schedule.effective_max_iterations();
        let max_cooling_cycle = (max_iterations as f64) / (Self::CONVERGENCE_CHECK_PERIOD as f64);
        let final_temperature = Self::FINAL_TEMPERATURE;
        let mut cooling_cycle = 0.0f64;

        let mut total_iterations = 0usize;
        let mut old_total_displacement = 0.0f64;
        let mut last_total_displacement = 0.0f64;

        let mut processed_generation: Vec<u32> = vec![0; self.nodes.len()];
        let mut disps: Vec<(f64, f64)> = vec![(0.0, 0.0); self.nodes.len()];
        let mut propagated_displacements: Vec<(f64, f64)> = vec![(0.0, 0.0); self.nodes.len() + 1];
        let all_nodes_in_layout_order = self.all_nodes_layout_order();
        let mut current_processed_generation: u32 = 0;
        let mut surrounding_seen_generation: u32 = 1;
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
                let cooling_adjustment = cooling_cycle.powf(power) / 100.0;
                cooling_factor =
                    (initial_cooling_factor - cooling_adjustment).max(final_temperature);
            }

            work_control.charge(work_plan.iteration_work_units())?;

            let mut total_displacement = 0.0f64;

            // Match `cose-base` tick order: update compound bounds (with padding) before forces.
            self.update_bounds(owner_bounds);

            // Spring forces (per-edge ideal lengths).
            for e in &self.edges {
                // layout-base spring forces act between the edge's actual endpoints
                // (`edge.getSource()/getTarget()`), even for inter-graph edges. LCA-lifted
                // endpoints are used for *ideal edge length* adjustments, not for force
                // application.
                let (a, b) = (e.a, e.b);
                if a == b {
                    continue;
                }
                if a >= self.nodes.len() || b >= self.nodes.len() {
                    continue;
                }
                if rects_intersect(&self.nodes[a], &self.nodes[b]) {
                    continue;
                }
                let (ax, ay, bx, by) =
                    rect_intersection_points_no_overlap_check(&self.nodes[a], &self.nodes[b]);
                let mut lx = bx - ax;
                let mut ly = by - ay;
                // layout-base `LEdge.updateLength()` clamps very small deltas to {-1, 0, 1}
                // using `IMath.sign`, to avoid divide-by-zero instability.
                if lx.abs() < 1.0 {
                    lx = imath_sign(lx);
                }
                if ly.abs() < 1.0 {
                    ly = imath_sign(ly);
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
            current_processed_generation = current_processed_generation.wrapping_add(1);
            if current_processed_generation == 0 {
                processed_generation.fill(0);
                current_processed_generation = 1;
            }
            if refresh_surrounding {
                let (l, r, t, b) = (
                    owner_bounds.left[self.root_owner_idx],
                    owner_bounds.right[self.root_owner_idx],
                    owner_bounds.top[self.root_owner_idx],
                    owner_bounds.bottom[self.root_owner_idx],
                );
                repulsion_grid = RepulsionGrid::build_or_reuse(
                    repulsion_grid,
                    l,
                    t,
                    r,
                    b,
                    &mut self.nodes,
                    repulsion_range,
                    &all_nodes_in_layout_order,
                    work_control,
                )?;
            }

            if repulsion_range.is_finite() && repulsion_range > 0.0 {
                if refresh_surrounding {
                    for &i in &all_nodes_in_layout_order {
                        if let Some(g) = &mut repulsion_grid {
                            g.refresh_node_surrounding(
                                i,
                                &mut self.nodes,
                                &processed_generation,
                                current_processed_generation,
                                repulsion_range,
                                &mut self.surrounding_seen,
                                &mut surrounding_seen_generation,
                                work_control,
                            )?;
                        } else {
                            self.nodes[i].surrounding.clear();
                        }
                        processed_generation[i] = current_processed_generation;
                    }
                }

                let surrounding_pair_work = all_nodes_in_layout_order
                    .iter()
                    .try_fold(0usize, |work, &idx| {
                        work.checked_add(
                            self.nodes
                                .get(idx)
                                .map(|node| node.surrounding.len())
                                .unwrap_or_default(),
                        )
                    })
                    .ok_or(WorkFailure::ArithmeticOverflow)?;
                admit_dynamic_work(work_control, surrounding_pair_work)?;

                for &i in &all_nodes_in_layout_order {
                    let surrounding = std::mem::take(&mut self.nodes[i].surrounding);
                    let node_i_center_x = self.nodes[i].center_x();
                    let node_i_center_y = self.nodes[i].center_y();
                    for &j in &surrounding {
                        let node_j_center_x = self.nodes[j].center_x();
                        let node_j_center_y = self.nodes[j].center_y();
                        let (rfx, rfy) = calc_repulsion_force(
                            &self.nodes[i],
                            &self.nodes[j],
                            min_repulsion_dist,
                            half_default_edge_length,
                            node_i_center_x,
                            node_i_center_y,
                            node_j_center_x,
                            node_j_center_y,
                        );
                        // Apply a symmetric pairwise force.
                        //
                        // Unlike `i < j` index-based deduping, upstream CoSE/FCoSE dedupes via a
                        // processed set in `getAllNodes()` order. Here `surrounding` is already
                        // filtered by `processed`, so we must not skip pairs where `j < i`.
                        self.nodes[i].repulsion_fx += rfx;
                        self.nodes[i].repulsion_fy += rfy;
                        self.nodes[j].repulsion_fx -= rfx;
                        self.nodes[j].repulsion_fy -= rfy;
                    }
                    self.nodes[i].surrounding = surrounding;
                }
            } else {
                // Fallback: unbounded repulsion for every same-owner pair. layout-base scans the
                // flat `getAllNodes()` list and discards cross-owner pairs; grouping by owner keeps
                // the same within-owner insertion order while avoiding those discarded scans.
                let pair_work = owner_local_pair_work(&self.children_by_owner)?;
                admit_dynamic_work(work_control, pair_work)?;
                let (nodes, children_by_owner) = (&mut self.nodes, &self.children_by_owner);
                for_each_owner_local_pair(children_by_owner, |i, j| {
                    let node_i_center_x = nodes[i].center_x();
                    let node_i_center_y = nodes[i].center_y();
                    let node_j_center_x = nodes[j].center_x();
                    let node_j_center_y = nodes[j].center_y();
                    let (rfx, rfy) = calc_repulsion_force(
                        &nodes[i],
                        &nodes[j],
                        min_repulsion_dist,
                        half_default_edge_length,
                        node_i_center_x,
                        node_i_center_y,
                        node_j_center_x,
                        node_j_center_y,
                    );
                    nodes[i].repulsion_fx += rfx;
                    nodes[i].repulsion_fy += rfy;
                    nodes[j].repulsion_fx -= rfx;
                    nodes[j].repulsion_fy -= rfy;
                });
            }

            // Gravity forces (layout-base `FDLayout.calcGravitationalForce`), per owner graph.
            let owner_connected = &self.compound_topology.owner_connected;
            for n in &mut self.nodes {
                n.gravitation_fx = 0.0;
                n.gravitation_fy = 0.0;
                if owner_connected.get(n.owner_idx).copied().unwrap_or(true) {
                    continue;
                }

                let owner = n.owner_idx;
                let (l, r, t, b) = (
                    owner_bounds.left.get(owner).copied().unwrap_or(0.0),
                    owner_bounds.right.get(owner).copied().unwrap_or(0.0),
                    owner_bounds.top.get(owner).copied().unwrap_or(0.0),
                    owner_bounds.bottom.get(owner).copied().unwrap_or(0.0),
                );
                if !(l.is_finite() && r.is_finite() && t.is_finite() && b.is_finite()) {
                    continue;
                }
                let cx = (l + r) / 2.0;
                let cy = (t + b) / 2.0;

                let dx = n.center_x() - cx;
                let dy = n.center_y() - cy;
                let abs_dx = dx.abs() + n.half_w();
                let abs_dy = dy.abs() + n.half_h();

                let (range_factor, compound_mul) = if owner == self.root_owner_idx {
                    (Self::DEFAULT_GRAVITY_RANGE_FACTOR, 1.0)
                } else {
                    (
                        Self::DEFAULT_COMPOUND_GRAVITY_RANGE_FACTOR,
                        Self::DEFAULT_COMPOUND_GRAVITY_STRENGTH,
                    )
                };
                let estimated =
                    self.owner_estimated_size.get(owner).copied().unwrap_or(0.0) * range_factor;
                if estimated.is_finite()
                    && estimated > 0.0
                    && (abs_dx > estimated || abs_dy > estimated)
                {
                    n.gravitation_fx = -gravity_constant * dx * compound_mul;
                    n.gravitation_fy = -gravity_constant * dy * compound_mul;
                }
            }

            // Move nodes (with constraints applied to displacements).
            //
            // Upstream `cose-base` computes displacements from forces, then applies constraint
            // handling that *updates those displacements* (rather than hard-projecting node
            // positions after the move). Hard projection tends to over-separate constrained nodes
            // and can noticeably inflate root viewBox/max-width in parity-root mode.
            let max_d = cooling_factor * max_node_displacement;
            self.calculate_displacements(
                &all_nodes_in_layout_order,
                cooling_factor,
                max_d,
                &mut disps,
                &mut propagated_displacements,
            );

            if let Some(rt) = constraint_rt.as_mut() {
                rt.update_displacements(constraints, &mut disps, total_iterations, rng);
            } else {
                apply_constraints_to_displacements(&self.nodes, constraints, &mut disps, max_d);
            }

            for (idx, n) in self.nodes.iter_mut().enumerate() {
                let (mdx, mdy) = disps.get(idx).copied().unwrap_or((0.0, 0.0));
                let is_non_empty_compound = n.is_compound
                    && !self
                        .children_by_owner
                        .get(idx)
                        .is_some_and(|v| v.is_empty());
                if !is_non_empty_compound {
                    n.move_by(mdx, mdy);
                    total_displacement += mdx.abs() + mdy.abs();
                }

                n.spring_fx = 0.0;
                n.spring_fy = 0.0;
                n.repulsion_fx = 0.0;
                n.repulsion_fy = 0.0;
                n.gravitation_fx = 0.0;
                n.gravitation_fy = 0.0;
            }

            last_total_displacement = total_displacement;
        }

        // Ensure compound rectangles reflect the final leaf positions before callers compute
        // component bbox/centering (e.g. `aux.relocateComponent(...)` parity).
        self.update_bounds(owner_bounds);
        Ok(())
    }

    fn reset_edge_ideal_lengths(&mut self) {
        for e in &mut self.edges {
            e.ideal_length = e.base_ideal_length;
        }
    }

    fn adjust_intergraph_ideal_edge_lengths(&mut self) {
        if self.edges.is_empty() || self.nodes.is_empty() {
            return;
        }

        let nodes: &[SimNode] = &self.nodes;
        let inclusion_depth: &[usize] = &self.inclusion_depth;
        let root_owner_idx = self.root_owner_idx;

        for (e, projection) in self
            .edges
            .iter_mut()
            .zip(&self.compound_topology.edge_projections)
        {
            let lca_owner = projection.lca_owner_idx;
            let src_in_lca = projection.source_in_lca;
            let tgt_in_lca = projection.target_in_lca;

            if nodes[e.a].owner_idx == nodes[e.b].owner_idx {
                continue;
            }

            let original = e.base_ideal_length.max(1.0);

            let lca_depth = if lca_owner == root_owner_idx {
                1usize
            } else {
                inclusion_depth.get(lca_owner).copied().unwrap_or(1).max(1)
            };

            // layout-base `DEFAULT_USE_SMART_IDEAL_EDGE_LENGTH_CALCULATION = true`.
            let size_src = nodes
                .get(src_in_lca)
                .map(|n| n.estimated_size)
                .unwrap_or(Self::SIMPLE_NODE_SIZE);
            let size_tgt = nodes
                .get(tgt_in_lca)
                .map(|n| n.estimated_size)
                .unwrap_or(Self::SIMPLE_NODE_SIZE);
            e.ideal_length += size_src + size_tgt - 2.0 * Self::SIMPLE_NODE_SIZE;

            let src_depth = inclusion_depth.get(e.a).copied().unwrap_or(1).max(1);
            let tgt_depth = inclusion_depth.get(e.b).copied().unwrap_or(1).max(1);
            let hops = (src_depth + tgt_depth).saturating_sub(2 * lca_depth);
            e.ideal_length += original * Self::PER_LEVEL_IDEAL_EDGE_LENGTH_FACTOR * (hops as f64);

            if !e.ideal_length.is_finite() || e.ideal_length <= 0.0 {
                e.ideal_length = 1.0;
            }
        }
    }

    fn collapse_start_positions(&mut self, scale: f64, rng: &mut FcoseRandom) {
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

fn rects_intersect(a: &SimNode, b: &SimNode) -> bool {
    // Mirror layout-base `RectangleD.intersects`: touching edges count as intersection.
    !(a.right() < b.left || a.bottom() < b.top || b.right() < a.left || b.bottom() < a.top)
}

#[inline]
fn definitely_less(a: f64, b: f64) -> bool {
    a + GEOMETRY_EPSILON < b
}

#[inline]
fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= GEOMETRY_EPSILON
}

fn get_cardinal_direction(slope: f64, slope_prime: f64, line: i32) -> i32 {
    if slope > slope_prime {
        line
    } else {
        1 + (line % 4)
    }
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
    let p1x = a.center_x();
    let p1y = a.center_y();
    let p2x = b.center_x();
    let p2y = b.center_y();

    if rects_intersect(a, b) {
        return (p1x, p1y, p2x, p2y, true);
    }

    let (ax, ay, bx, by) = rect_intersection_points_no_overlap_check(a, b);
    (ax, ay, bx, by, false)
}

#[inline]
fn rect_intersection_points_no_overlap_check(a: &SimNode, b: &SimNode) -> (f64, f64, f64, f64) {
    // Port of layout-base `IGeometry.getIntersection2(rectA, rectB, result)`.
    //
    // result[0-1] contains clip point on rectA; result[2-3] contains clip point on rectB.
    let p1x = a.center_x();
    let p1y = a.center_y();
    let p2x = b.center_x();
    let p2y = b.center_y();

    let left_a = a.left;
    let right_a = a.right();
    let top_a = a.top;
    let bottom_a = a.bottom();
    let half_width_a = a.half_w();
    let half_height_a = a.half_h();

    let left_b = b.left;
    let right_b = b.right();
    let top_b = b.top;
    let bottom_b = b.bottom();
    let half_width_b = b.half_w();
    let half_height_b = b.half_h();

    let mut clip_ax = p1x;
    let mut clip_ay = p1y;
    let mut clip_bx = p2x;
    let mut clip_by = p2y;

    if p1x == p2x {
        if p1y > p2y {
            return (p1x, top_a, p2x, bottom_b);
        } else if p1y < p2y {
            return (p1x, bottom_a, p2x, top_b);
        }
    } else if p1y == p2y {
        if p1x > p2x {
            return (left_a, p1y, right_b, p2y);
        } else if p1x < p2x {
            return (right_a, p1y, left_b, p2y);
        }
    } else {
        let slope_a = a.height / a.width;
        let slope_b = b.height / b.width;
        let slope_prime = (p2y - p1y) / (p2x - p1x);

        let mut clip_a_found = false;
        let mut clip_b_found = false;

        if -slope_a == slope_prime {
            if p1x > p2x {
                clip_ax = left_a;
                clip_ay = bottom_a;
                clip_a_found = true;
            } else {
                clip_ax = right_a;
                clip_ay = top_a;
                clip_a_found = true;
            }
        } else if slope_a == slope_prime {
            if p1x > p2x {
                clip_ax = left_a;
                clip_ay = top_a;
                clip_a_found = true;
            } else {
                clip_ax = right_a;
                clip_ay = bottom_a;
                clip_a_found = true;
            }
        }

        if -slope_b == slope_prime {
            if p2x > p1x {
                clip_bx = left_b;
                clip_by = bottom_b;
                clip_b_found = true;
            } else {
                clip_bx = right_b;
                clip_by = top_b;
                clip_b_found = true;
            }
        } else if slope_b == slope_prime {
            if p2x > p1x {
                clip_bx = left_b;
                clip_by = top_b;
                clip_b_found = true;
            } else {
                clip_bx = right_b;
                clip_by = bottom_b;
                clip_b_found = true;
            }
        }

        if !clip_a_found || !clip_b_found {
            let (card_a, card_b) = if p1x > p2x {
                if p1y > p2y {
                    (
                        get_cardinal_direction(slope_a, slope_prime, 4),
                        get_cardinal_direction(slope_b, slope_prime, 2),
                    )
                } else {
                    (
                        get_cardinal_direction(-slope_a, slope_prime, 3),
                        get_cardinal_direction(-slope_b, slope_prime, 1),
                    )
                }
            } else if p1y > p2y {
                (
                    get_cardinal_direction(-slope_a, slope_prime, 1),
                    get_cardinal_direction(-slope_b, slope_prime, 3),
                )
            } else {
                (
                    get_cardinal_direction(slope_a, slope_prime, 2),
                    get_cardinal_direction(slope_b, slope_prime, 4),
                )
            };

            if !clip_a_found {
                match card_a {
                    1 => {
                        clip_ay = top_a;
                        clip_ax = p1x + -half_height_a / slope_prime;
                    }
                    2 => {
                        clip_ax = right_a;
                        clip_ay = p1y + half_width_a * slope_prime;
                    }
                    3 => {
                        clip_ay = bottom_a;
                        clip_ax = p1x + half_height_a / slope_prime;
                    }
                    4 => {
                        clip_ax = left_a;
                        clip_ay = p1y + -half_width_a * slope_prime;
                    }
                    _ => {}
                }
            }

            if !clip_b_found {
                match card_b {
                    1 => {
                        clip_by = top_b;
                        clip_bx = p2x + -half_height_b / slope_prime;
                    }
                    2 => {
                        clip_bx = right_b;
                        clip_by = p2y + half_width_b * slope_prime;
                    }
                    3 => {
                        clip_by = bottom_b;
                        clip_bx = p2x + half_height_b / slope_prime;
                    }
                    4 => {
                        clip_bx = left_b;
                        clip_by = p2y + -half_width_b * slope_prime;
                    }
                    _ => {}
                }
            }
        }
    }

    (clip_ax, clip_ay, clip_bx, clip_by)
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
