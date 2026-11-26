use super::constraint_projection::{
    apply_constraints_to_displacements, handle_constraints_pre_layout,
};
use super::constraints::ConstraintRuntime;
use super::repulsion_grid::RepulsionGrid;
use super::{
    FcoseRandom, SimGraph, SimNode, SpringEmbedderContext, WorkControl, calc_repulsion_force,
    imath_sign, rect_intersection_points_no_overlap_check, rects_intersect, spectral,
};
use crate::error::WorkFailure;
use crate::work::admit_dynamic_work;

pub(super) fn owner_local_pair_work(
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

pub(super) fn for_each_owner_local_pair(
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

impl SimGraph {
    // cytoscape-fcose default (overrides layout-base default 0.4 via `options.gravity`).
    const DEFAULT_GRAVITY_STRENGTH: f64 = 0.25;
    const DEFAULT_COMPOUND_GRAVITY_STRENGTH: f64 = 1.0; // layout-base `FDLayoutConstants.DEFAULT_COMPOUND_GRAVITY_STRENGTH`
    const DEFAULT_GRAVITY_RANGE_FACTOR: f64 = 3.8; // layout-base `FDLayoutConstants.DEFAULT_GRAVITY_RANGE_FACTOR`
    const DEFAULT_COMPOUND_GRAVITY_RANGE_FACTOR: f64 = 1.5; // layout-base `FDLayoutConstants.DEFAULT_COMPOUND_GRAVITY_RANGE_FACTOR`
    const SIMPLE_NODE_SIZE: f64 = 40.0; // layout-base `LayoutConstants.SIMPLE_NODE_SIZE`
    const PER_LEVEL_IDEAL_EDGE_LENGTH_FACTOR: f64 = 0.1; // layout-base `FDLayoutConstants.PER_LEVEL_IDEAL_EDGE_LENGTH_FACTOR`
    const DEFAULT_COOLING_FACTOR_INCREMENTAL: f64 = 0.3; // layout-base `FDLayoutConstants.DEFAULT_COOLING_FACTOR_INCREMENTAL`
    const FINAL_TEMPERATURE: f64 = 0.04; // cose-base `CoSELayout.initSpringEmbedder()`
    const GRID_CALCULATION_CHECK_PERIOD: usize = 10; // layout-base `FDLayoutConstants.GRID_CALCULATION_CHECK_PERIOD`

    const CONVERGENCE_CHECK_PERIOD: usize = 100;
    const MAX_NODE_DISPLACEMENT_INCREMENTAL: f64 = 100.0; // layout-base `FDLayoutConstants.MAX_NODE_DISPLACEMENT_INCREMENTAL`

    pub(super) fn calculate_displacements(
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

    pub(super) fn run_spring_embedder<W: WorkControl + ?Sized>(
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
