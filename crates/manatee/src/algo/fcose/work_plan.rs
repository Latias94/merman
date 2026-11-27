use super::{CompoundTopology, IndexedFcoseOptions, IndexedGraph};
use crate::error::WorkFailure;

const DEFAULT_FCOSE_ITERATIONS: usize = 2500;

/// Checked execution schedule for one FCoSE invocation.
///
/// The schedule preserves the existing CoSE loop contract: every run has an effective maximum of
/// `max(configured, nodes * 5)`, while the loop body executes at most `maximum - 1` times because
/// the historical termination check happens before the first tranche at `total == maximum`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FcoseIterationSchedule {
    configured_iterations: usize,
    effective_max_iterations: usize,
    run_count: usize,
    setup_work_units: usize,
    iteration_work_units: usize,
    maximum_work_units: usize,
}

impl FcoseIterationSchedule {
    /// Normalize a JavaScript-style numeric `numIter` and construct a checked graph schedule.
    ///
    /// Finite positive values are rounded. Missing, non-finite, and values below one retain the
    /// established 2500-iteration fallback. Values outside `usize` and any derived arithmetic
    /// overflow fail closed.
    pub fn from_configured_number(
        configured: Option<f64>,
        node_count: usize,
        edge_count: usize,
        rerun: bool,
    ) -> std::result::Result<Self, WorkFailure> {
        let configured_iterations = Self::normalize_configured_number(configured)?;
        Self::from_normalized_counts(configured_iterations, node_count, edge_count, rerun)
    }

    pub fn normalize_configured_number(
        configured: Option<f64>,
    ) -> std::result::Result<usize, WorkFailure> {
        let Some(configured) = configured.filter(|value| value.is_finite() && *value >= 1.0) else {
            return Ok(DEFAULT_FCOSE_ITERATIONS);
        };
        let rounded = configured.round();
        if rounded >= usize::MAX as f64 {
            return Err(WorkFailure::ArithmeticOverflow);
        }
        Ok(rounded as usize)
    }

    pub fn from_indexed_graph(
        configured: Option<usize>,
        graph: &IndexedGraph,
        rerun: bool,
    ) -> std::result::Result<Self, WorkFailure> {
        Self::from_options(
            configured,
            graph.nodes.len(),
            graph.compounds.len(),
            graph.edges.len(),
            rerun,
        )
    }

    pub(super) fn from_options(
        configured: Option<usize>,
        leaf_count: usize,
        compound_count: usize,
        edge_count: usize,
        rerun: bool,
    ) -> std::result::Result<Self, WorkFailure> {
        Self::from_normalized_graph_counts(
            configured
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_FCOSE_ITERATIONS),
            leaf_count,
            compound_count,
            edge_count,
            rerun,
        )
    }

    /// Builds a checked schedule from an already-normalized iteration count and graph cardinality.
    ///
    /// Adapters can use this before materializing an [`IndexedGraph`] when their source counts are
    /// a conservative upper bound of the eventual indexed graph.
    pub fn from_normalized_counts(
        configured_iterations: usize,
        node_count: usize,
        edge_count: usize,
        rerun: bool,
    ) -> std::result::Result<Self, WorkFailure> {
        Self::from_normalized_shape(
            configured_iterations,
            node_count,
            node_count,
            edge_count,
            rerun,
        )
    }

    /// Builds a checked schedule when leaf and compound cardinalities are both known.
    ///
    /// Unlike [`Self::from_normalized_counts`], this avoids reserving a deep ancestry table for a
    /// flat graph. The bound remains conservative because an inclusion chain can contain at most
    /// every compound plus one leaf.
    pub fn from_normalized_graph_counts(
        configured_iterations: usize,
        leaf_count: usize,
        compound_count: usize,
        edge_count: usize,
        rerun: bool,
    ) -> std::result::Result<Self, WorkFailure> {
        let node_count = leaf_count
            .checked_add(compound_count)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        Self::from_normalized_shape(
            configured_iterations,
            node_count,
            compound_count,
            edge_count,
            rerun,
        )
    }

    fn from_normalized_shape(
        configured_iterations: usize,
        node_count: usize,
        compound_count: usize,
        edge_count: usize,
        rerun: bool,
    ) -> std::result::Result<Self, WorkFailure> {
        let configured_iterations = if configured_iterations == 0 {
            DEFAULT_FCOSE_ITERATIONS
        } else {
            configured_iterations
        };
        let node_iteration_floor = node_count
            .checked_mul(5)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let effective_max_iterations = configured_iterations.max(node_iteration_floor);
        let run_count = if rerun { 2 } else { 1 };
        let graph_work_units = node_count
            .checked_add(edge_count)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let setup_work_units = graph_work_units
            .checked_add(CompoundTopology::work_units_with_compound_count(
                node_count,
                edge_count,
                compound_count,
            )?)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        if node_count == 0 {
            return Ok(Self {
                configured_iterations,
                effective_max_iterations,
                run_count,
                setup_work_units,
                iteration_work_units: 0,
                maximum_work_units: setup_work_units,
            });
        }
        let iteration_work_units = graph_work_units.max(1);
        let executed_iterations_per_run = effective_max_iterations
            .checked_sub(1)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let iteration_maximum = executed_iterations_per_run
            .checked_mul(run_count)
            .and_then(|iterations| iterations.checked_mul(iteration_work_units))
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let maximum_work_units = setup_work_units
            .checked_add(iteration_maximum)
            .ok_or(WorkFailure::ArithmeticOverflow)?;

        Ok(Self {
            configured_iterations,
            effective_max_iterations,
            run_count,
            setup_work_units,
            iteration_work_units,
            maximum_work_units,
        })
    }

    pub const fn configured_iterations(self) -> usize {
        self.configured_iterations
    }

    pub const fn effective_max_iterations(self) -> usize {
        self.effective_max_iterations
    }

    pub const fn run_count(self) -> usize {
        self.run_count
    }

    pub const fn setup_work_units(self) -> usize {
        self.setup_work_units
    }

    pub const fn iteration_work_units(self) -> usize {
        self.iteration_work_units
    }

    pub const fn maximum_work_units(self) -> usize {
        self.maximum_work_units
    }
}

/// Cardinalities that distinguish raw constraint input from the filtered runtime projection.
///
/// Mermaid/Cytoscape preserves alignment order and duplicates, so both remain part of the work
/// shape even when they refer to the same node repeatedly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct FcoseConstraintWorkShape {
    input_group_count: usize,
    input_member_count: usize,
    input_relative_count: usize,
    retained_group_count: usize,
    retained_member_count: usize,
    retained_relative_count: usize,
    valid_axis_pair_count: usize,
    may_collapse_zero_gap: bool,
    has_compound_zero_gap_self_loop: bool,
}

impl FcoseConstraintWorkShape {
    pub(super) const fn from_counts(
        input_group_count: usize,
        input_member_count: usize,
        input_relative_count: usize,
        retained_group_count: usize,
        retained_member_count: usize,
        valid_axis_pair_count: usize,
    ) -> Self {
        Self {
            input_group_count,
            input_member_count,
            input_relative_count,
            retained_group_count,
            retained_member_count,
            retained_relative_count: input_relative_count,
            valid_axis_pair_count,
            may_collapse_zero_gap: false,
            has_compound_zero_gap_self_loop: false,
        }
    }

    pub(super) fn input_headers(
        opts: &IndexedFcoseOptions,
    ) -> std::result::Result<(usize, usize), WorkFailure> {
        let (horizontal, vertical) = opts
            .alignment_constraint
            .as_ref()
            .map(|alignment| (alignment.horizontal.len(), alignment.vertical.len()))
            .unwrap_or_default();
        let groups = horizontal
            .checked_add(vertical)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        Ok((groups, opts.relative_placement_constraint.len()))
    }

    pub(super) fn input_member_count(
        opts: &IndexedFcoseOptions,
    ) -> std::result::Result<usize, WorkFailure> {
        opts.alignment_constraint
            .as_ref()
            .map(|alignment| {
                alignment
                    .horizontal
                    .iter()
                    .chain(&alignment.vertical)
                    .try_fold(0usize, |members, group| members.checked_add(group.len()))
                    .ok_or(WorkFailure::ArithmeticOverflow)
            })
            .transpose()
            .map(|members| members.unwrap_or_default())
    }

    pub(super) fn from_indexed_options(
        opts: &IndexedFcoseOptions,
        leaf_count: usize,
        node_count: usize,
        input_group_count: usize,
        input_member_count: usize,
    ) -> std::result::Result<Self, WorkFailure> {
        let mut retained_group_count = 0usize;
        let mut retained_member_count = 0usize;
        if let Some(alignment) = opts.alignment_constraint.as_ref() {
            for group in alignment.horizontal.iter().chain(&alignment.vertical) {
                let valid_members = group.iter().filter(|idx| **idx < node_count).count();
                if valid_members > 1 {
                    retained_group_count = retained_group_count
                        .checked_add(1)
                        .ok_or(WorkFailure::ArithmeticOverflow)?;
                    retained_member_count = retained_member_count
                        .checked_add(valid_members)
                        .ok_or(WorkFailure::ArithmeticOverflow)?;
                }
            }
        }

        let mut valid_axis_pair_count = 0usize;
        let mut may_collapse_zero_gap = false;
        let mut has_compound_zero_gap_self_loop = false;
        for relative in &opts.relative_placement_constraint {
            let horizontal = relative
                .left
                .zip(relative.right)
                .is_some_and(|(left, right)| left < node_count && right < node_count);
            let vertical = relative
                .top
                .zip(relative.bottom)
                .is_some_and(|(top, bottom)| top < node_count && bottom < node_count);
            valid_axis_pair_count = valid_axis_pair_count
                .checked_add(usize::from(horizontal) + usize::from(vertical))
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            if (horizontal || vertical) && relative.gap <= 0.0 {
                may_collapse_zero_gap = true;
                if leaf_count < node_count && !has_compound_zero_gap_self_loop {
                    has_compound_zero_gap_self_loop = relative
                        .zero_gap_compound_self_loops(leaf_count..node_count)
                        .next()
                        .is_some();
                }
            }
        }

        Ok(Self {
            may_collapse_zero_gap,
            has_compound_zero_gap_self_loop,
            ..Self::from_counts(
                input_group_count,
                input_member_count,
                opts.relative_placement_constraint.len(),
                retained_group_count,
                retained_member_count,
                valid_axis_pair_count,
            )
        })
    }

    pub(super) const fn may_collapse_zero_gap(self) -> bool {
        self.may_collapse_zero_gap
    }

    pub(super) const fn has_compound_zero_gap_self_loop(self) -> bool {
        self.has_compound_zero_gap_self_loop
    }

    pub(super) fn with_normalized_constraints(
        mut self,
        constraints: &super::Constraints,
    ) -> std::result::Result<Self, WorkFailure> {
        self.retained_group_count = constraints
            .align_horizontal
            .len()
            .checked_add(constraints.align_vertical.len())
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        self.retained_member_count = constraints
            .align_horizontal
            .iter()
            .chain(&constraints.align_vertical)
            .try_fold(0usize, |members, group| members.checked_add(group.len()))
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        self.retained_relative_count = constraints.relative.len();
        self.valid_axis_pair_count = constraints
            .relative
            .iter()
            .try_fold(0usize, |pairs, constraint| {
                pairs
                    .checked_add(usize::from(constraint.left.zip(constraint.right).is_some()))
                    .and_then(|pairs| {
                        pairs.checked_add(usize::from(
                            constraint.top.zip(constraint.bottom).is_some(),
                        ))
                    })
            })
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        Ok(self)
    }

    pub(super) fn setup_work_units(
        self,
        schedule: FcoseIterationSchedule,
    ) -> std::result::Result<usize, WorkFailure> {
        // The raw rows must be available once more if normalization can erase a compound target.
        let compound_self_loop_scan = if self.has_compound_zero_gap_self_loop {
            self.input_relative_count
        } else {
            0
        };
        schedule
            .setup_work_units()
            .checked_add(self.input_group_count)
            .and_then(|units| units.checked_add(self.input_member_count))
            .and_then(|units| units.checked_add(self.input_relative_count))
            .and_then(|units| units.checked_add(compound_self_loop_scan))
            .ok_or(WorkFailure::ArithmeticOverflow)
    }

    fn run_work_units(self) -> std::result::Result<usize, WorkFailure> {
        self.retained_group_count
            .checked_add(self.retained_member_count)
            .and_then(|units| units.checked_add(self.retained_relative_count))
            .ok_or(WorkFailure::ArithmeticOverflow)
    }

    fn iteration_work_units(self) -> std::result::Result<usize, WorkFailure> {
        self.retained_group_count
            .checked_add(self.retained_member_count)
            .and_then(|units| units.checked_add(self.valid_axis_pair_count))
            .ok_or(WorkFailure::ArithmeticOverflow)
    }
}

/// Complete predictable FCoSE work admitted before simulation allocation.
///
/// Relative-placement ancestry clones are convergence/data-shape dependent and are charged
/// exactly at their allocation sites, like spectral convergence tranches; they are deliberately
/// excluded from this predictable maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FcoseWorkPlan {
    schedule: FcoseIterationSchedule,
    setup_work_units: usize,
    run_setup_work_units: usize,
    iteration_work_units: usize,
    random_seed_offset_work_units: usize,
    maximum_work_units: usize,
}

impl FcoseWorkPlan {
    pub(super) fn from_schedule(
        schedule: FcoseIterationSchedule,
        node_count: usize,
        shape: FcoseConstraintWorkShape,
        random_seed_offset: usize,
        reset_seed_each_run: bool,
    ) -> std::result::Result<Self, WorkFailure> {
        let setup_work_units = shape.setup_work_units(schedule)?;
        let constraint_run_work_units = shape.run_work_units()?;
        let run_setup_work_units = if node_count > 0 && constraint_run_work_units > 0 {
            constraint_run_work_units
        } else {
            0
        };
        let iteration_work_units = if node_count > 0 {
            schedule
                .iteration_work_units()
                .checked_add(shape.iteration_work_units()?)
                .ok_or(WorkFailure::ArithmeticOverflow)?
        } else {
            0
        };
        let executed_iterations_per_run = if node_count > 0 {
            schedule
                .effective_max_iterations()
                .checked_sub(1)
                .ok_or(WorkFailure::ArithmeticOverflow)?
        } else {
            0
        };
        let iteration_maximum = executed_iterations_per_run
            .checked_mul(schedule.run_count())
            .and_then(|iterations| iterations.checked_mul(iteration_work_units))
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let run_setup_maximum = run_setup_work_units
            .checked_mul(schedule.run_count())
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let random_reset_count = if reset_seed_each_run {
            schedule.run_count()
        } else {
            1
        };
        let random_maximum = random_seed_offset
            .checked_mul(random_reset_count)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let maximum_work_units = setup_work_units
            .checked_add(run_setup_maximum)
            .and_then(|units| units.checked_add(iteration_maximum))
            .and_then(|units| units.checked_add(random_maximum))
            .ok_or(WorkFailure::ArithmeticOverflow)?;

        Ok(Self {
            schedule,
            setup_work_units,
            run_setup_work_units,
            iteration_work_units,
            random_seed_offset_work_units: random_seed_offset,
            maximum_work_units,
        })
    }

    pub(super) const fn schedule(self) -> FcoseIterationSchedule {
        self.schedule
    }

    pub(super) const fn setup_work_units(self) -> usize {
        self.setup_work_units
    }

    pub(super) const fn run_setup_work_units(self) -> usize {
        self.run_setup_work_units
    }

    pub(super) const fn iteration_work_units(self) -> usize {
        self.iteration_work_units
    }

    pub(super) const fn random_seed_offset_work_units(self) -> usize {
        self.random_seed_offset_work_units
    }

    pub(super) const fn maximum_work_units(self) -> usize {
        self.maximum_work_units
    }
}
