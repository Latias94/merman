use rustc_hash::FxHashMap;

use super::{SimNode, WorkControl};
use crate::algo::grid_cells::reset_dense_cells;
use crate::error::WorkFailure;
use crate::work::admit_dynamic_work;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepulsionGridStorageKind {
    Dense,
    Sparse,
    Implicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridNodeBounds {
    start_x: i64,
    finish_x: i64,
    start_y: i64,
    finish_y: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridScanBounds {
    start_x: i64,
    finish_x: i64,
    start_y: i64,
    finish_y: i64,
}

impl GridScanBounds {
    fn cell_count(self) -> u128 {
        let width = (self.finish_x as i128 - self.start_x as i128 + 1) as u128;
        let height = (self.finish_y as i128 - self.start_y as i128 + 1) as u128;
        width * height
    }
}

impl GridNodeBounds {
    fn from_node(
        node: &SimNode,
        left: f64,
        top: f64,
        repulsion_range: f64,
    ) -> std::result::Result<Self, WorkFailure> {
        Ok(Self {
            start_x: grid_coordinate((node.left - left) / repulsion_range)?,
            finish_x: grid_coordinate((node.right() - left) / repulsion_range)?,
            start_y: grid_coordinate((node.top - top) / repulsion_range)?,
            finish_y: grid_coordinate((node.bottom() - top) / repulsion_range)?,
        })
    }

    fn clipped_axis(start: i64, finish: i64, size: i64) -> Option<(i64, i64)> {
        let first = start.max(0);
        let last = finish.min(size.saturating_sub(1));
        (first <= last).then_some((first, last))
    }

    fn clipped_x(self, size_x: i64) -> Option<(i64, i64)> {
        Self::clipped_axis(self.start_x, self.finish_x, size_x)
    }

    fn clipped_y(self, size_y: i64) -> Option<(i64, i64)> {
        Self::clipped_axis(self.start_y, self.finish_y, size_y)
    }

    fn cell_reference_count(self, size_x: i64, size_y: i64) -> u128 {
        let Some((start_x, finish_x)) = self.clipped_x(size_x) else {
            return 0;
        };
        let Some((start_y, finish_y)) = self.clipped_y(size_y) else {
            return 0;
        };
        let width = (finish_x as i128 - start_x as i128 + 1) as u128;
        let height = (finish_y as i128 - start_y as i128 + 1) as u128;
        width * height
    }
}

fn grid_dimension(span: f64, repulsion_range: f64) -> std::result::Result<i64, WorkFailure> {
    let cells = (span / repulsion_range).ceil().max(1.0);
    if !cells.is_finite() || cells >= i64::MAX as f64 {
        return Err(WorkFailure::ArithmeticOverflow);
    }
    Ok(cells as i64)
}

fn grid_coordinate(value: f64) -> std::result::Result<i64, WorkFailure> {
    let coordinate = value.floor();
    if !coordinate.is_finite() || coordinate < i64::MIN as f64 || coordinate >= i64::MAX as f64 {
        return Err(WorkFailure::ArithmeticOverflow);
    }
    Ok(coordinate as i64)
}

pub(super) fn implicit_grid_work_units(node_entry_count: u128) -> Option<u128> {
    if node_entry_count == 0 {
        return Some(0);
    }
    let comparison_levels = if node_entry_count <= 1 {
        1
    } else {
        128u128.checked_sub((node_entry_count - 1).leading_zeros() as u128)?
    };
    node_entry_count
        .checked_mul(node_entry_count)?
        .checked_mul(comparison_levels)?
        .checked_add(node_entry_count)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepulsionGridPlan {
    size_x: i64,
    size_y: i64,
    total_cell_count: u128,
    cell_reference_count: u128,
    pub(super) storage_kind: RepulsionGridStorageKind,
    work_units: usize,
}

impl RepulsionGridPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_geometry(
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        nodes: &[SimNode],
        repulsion_range: f64,
        node_order: &[usize],
    ) -> std::result::Result<Option<Self>, WorkFailure> {
        if nodes.is_empty() || !repulsion_range.is_finite() || repulsion_range <= 0.0 {
            return Ok(None);
        }
        if !(left.is_finite() && top.is_finite() && right.is_finite() && bottom.is_finite()) {
            return Ok(None);
        }

        let width = right - left;
        let height = bottom - top;
        if !(width.is_finite() && height.is_finite()) {
            return Err(WorkFailure::ArithmeticOverflow);
        }
        let width = width.max(1.0);
        let height = height.max(1.0);

        let size_x = grid_dimension(width, repulsion_range)?;
        let size_y = grid_dimension(height, repulsion_range)?;
        let total_cell_count = (size_x as u128)
            .checked_mul(size_y as u128)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let mut cell_reference_count = 0u128;
        let mut node_entry_count = 0u128;
        for &idx in node_order {
            node_entry_count = node_entry_count
                .checked_add(1)
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            let Some(node) = nodes.get(idx) else {
                continue;
            };
            let bounds = GridNodeBounds::from_node(node, left, top, repulsion_range)?;
            // Saturation marks Dense/Sparse as unavailable while leaving the independent
            // implicit representation eligible. The selected concrete cost is still required to
            // fit `usize` below.
            cell_reference_count =
                cell_reference_count.saturating_add(bounds.cell_reference_count(size_x, size_y));
        }

        // Dense storage preserves the upstream shape for compact graphs. Sparse storage omits
        // empty cells but keeps each cell's insertion order. When a single large rectangle would
        // itself touch more cells than an exact all-node candidate pass, the implicit form derives
        // the same first-hit `(x, y, insertion-order)` sequence without materializing those cells.
        let candidates = [
            (
                RepulsionGridStorageKind::Dense,
                total_cell_count.checked_add(cell_reference_count),
            ),
            (
                RepulsionGridStorageKind::Sparse,
                cell_reference_count.checked_mul(2),
            ),
            (
                RepulsionGridStorageKind::Implicit,
                implicit_grid_work_units(node_entry_count),
            ),
        ];
        let (storage_kind, selected_work) = candidates
            .into_iter()
            .filter_map(|(kind, work)| work.map(|work| (kind, work)))
            .min_by_key(|(_, work)| *work)
            .ok_or(WorkFailure::ArithmeticOverflow)?;

        let work_units =
            usize::try_from(selected_work).map_err(|_| WorkFailure::ArithmeticOverflow)?;
        Ok(Some(Self {
            size_x,
            size_y,
            total_cell_count,
            cell_reference_count,
            storage_kind,
            work_units,
        }))
    }

    #[cfg(test)]
    pub(super) const fn storage_kind(self) -> RepulsionGridStorageKind {
        self.storage_kind
    }

    #[cfg(test)]
    pub(super) const fn total_cell_count(self) -> u128 {
        self.total_cell_count
    }

    #[cfg(test)]
    pub(super) const fn cell_reference_count(self) -> u128 {
        self.cell_reference_count
    }

    pub(super) const fn work_units(self) -> usize {
        self.work_units
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ImplicitGridCandidate {
    first_x: i64,
    first_y: i64,
    insertion_order: usize,
    node_idx: usize,
}

#[derive(Debug, Clone)]
pub(super) enum RepulsionGridCells {
    Dense(Vec<Vec<usize>>),
    Sparse(FxHashMap<(i64, i64), Vec<usize>>),
    Implicit {
        node_order: Vec<usize>,
        candidates: Vec<ImplicitGridCandidate>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct RepulsionGrid {
    size_x: i64,
    size_y: i64,
    pub(super) cells: RepulsionGridCells,
    candidate_visit_counts: Vec<usize>,
    materialized_refresh_prepared: bool,
}

impl RepulsionGrid {
    fn new(storage_kind: RepulsionGridStorageKind) -> Self {
        let cells = match storage_kind {
            RepulsionGridStorageKind::Dense => RepulsionGridCells::Dense(Vec::new()),
            RepulsionGridStorageKind::Sparse => RepulsionGridCells::Sparse(FxHashMap::default()),
            RepulsionGridStorageKind::Implicit => RepulsionGridCells::Implicit {
                node_order: Vec::new(),
                candidates: Vec::new(),
            },
        };
        Self {
            size_x: 1,
            size_y: 1,
            cells,
            candidate_visit_counts: Vec::new(),
            materialized_refresh_prepared: false,
        }
    }

    fn reset(
        &mut self,
        plan: RepulsionGridPlan,
        node_order: &[usize],
    ) -> std::result::Result<(), WorkFailure> {
        self.size_x = plan.size_x;
        self.size_y = plan.size_y;
        self.candidate_visit_counts.clear();
        self.materialized_refresh_prepared = false;
        match plan.storage_kind {
            RepulsionGridStorageKind::Dense => {
                if !matches!(self.cells, RepulsionGridCells::Dense(_)) {
                    self.cells = RepulsionGridCells::Dense(Vec::new());
                }
                let RepulsionGridCells::Dense(cells) = &mut self.cells else {
                    unreachable!("dense storage selected above")
                };
                let target = usize::try_from(plan.total_cell_count)
                    .map_err(|_| WorkFailure::ArithmeticOverflow)?;
                reset_dense_cells(cells, target)?;
            }
            RepulsionGridStorageKind::Sparse => {
                if !matches!(self.cells, RepulsionGridCells::Sparse(_)) {
                    self.cells = RepulsionGridCells::Sparse(FxHashMap::default());
                }
                let RepulsionGridCells::Sparse(cells) = &mut self.cells else {
                    unreachable!("sparse storage selected above")
                };
                cells.clear();
                let reserve = usize::try_from(plan.cell_reference_count.min(plan.total_cell_count))
                    .map_err(|_| WorkFailure::ArithmeticOverflow)?;
                cells
                    .try_reserve(reserve)
                    .map_err(|_| WorkFailure::ArithmeticOverflow)?;
            }
            RepulsionGridStorageKind::Implicit => {
                if !matches!(self.cells, RepulsionGridCells::Implicit { .. }) {
                    self.cells = RepulsionGridCells::Implicit {
                        node_order: Vec::new(),
                        candidates: Vec::new(),
                    };
                }
                let RepulsionGridCells::Implicit {
                    node_order: stored_order,
                    candidates,
                } = &mut self.cells
                else {
                    unreachable!("implicit storage selected above")
                };
                stored_order.clear();
                stored_order
                    .try_reserve_exact(node_order.len())
                    .map_err(|_| WorkFailure::ArithmeticOverflow)?;
                stored_order.extend_from_slice(node_order);
                candidates.clear();
                candidates
                    .try_reserve_exact(node_order.len())
                    .map_err(|_| WorkFailure::ArithmeticOverflow)?;
            }
        }
        Ok(())
    }

    fn cell(&self, x: i64, y: i64) -> &[usize] {
        match &self.cells {
            RepulsionGridCells::Dense(cells) => {
                let Some(index) = self.dense_index(x, y) else {
                    return &[];
                };
                cells.get(index).map(Vec::as_slice).unwrap_or(&[])
            }
            RepulsionGridCells::Sparse(cells) => {
                cells.get(&(x, y)).map(Vec::as_slice).unwrap_or(&[])
            }
            RepulsionGridCells::Implicit { .. } => &[],
        }
    }

    fn dense_index(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || x >= self.size_x || y < 0 || y >= self.size_y {
            return None;
        }
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        let size_y = usize::try_from(self.size_y).ok()?;
        x.checked_mul(size_y)?.checked_add(y)
    }

    fn scan_bounds(&self, node: &SimNode) -> Option<GridScanBounds> {
        let start_x = node.grid_start_x.saturating_sub(1).max(0);
        let finish_x = node
            .grid_finish_x
            .saturating_add(1)
            .min(self.size_x.saturating_sub(1));
        if start_x > finish_x {
            return None;
        }
        let start_y = node.grid_start_y.saturating_sub(1).max(0);
        let finish_y = node
            .grid_finish_y
            .saturating_add(1)
            .min(self.size_y.saturating_sub(1));
        (start_y <= finish_y).then_some(GridScanBounds {
            start_x,
            finish_x,
            start_y,
            finish_y,
        })
    }

    fn scan_work_exceeds(&self, nodes: &[SimNode], node_order: &[usize], work: usize) -> bool {
        // Even a full-grid scan for every entry cannot exceed this bound. If it fits, the
        // materialized path can compute each node's bounds just once, at scan admission.
        let grid_cells = (self.size_x as u128) * (self.size_y as u128);
        if grid_cells.saturating_mul(node_order.len() as u128) <= work as u128 {
            return false;
        }

        // Resolve scan-based promotion before attempting any per-node admission. Once the
        // prefix exceeds the limit, later entries cannot change that decision.
        node_order
            .iter()
            .try_fold(work as u128, |remaining, &idx| {
                let count = nodes
                    .get(idx)
                    .and_then(|node| self.scan_bounds(node))
                    .map(GridScanBounds::cell_count)
                    .unwrap_or_default();
                remaining.checked_sub(count)
            })
            .is_none()
    }

    fn switch_to_implicit(&mut self, node_order: &[usize]) -> std::result::Result<(), WorkFailure> {
        let mut stored_order = Vec::new();
        stored_order
            .try_reserve_exact(node_order.len())
            .map_err(|_| WorkFailure::ArithmeticOverflow)?;
        stored_order.extend_from_slice(node_order);
        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(node_order.len())
            .map_err(|_| WorkFailure::ArithmeticOverflow)?;
        self.cells = RepulsionGridCells::Implicit {
            node_order: stored_order,
            candidates,
        };
        self.candidate_visit_counts.clear();
        self.materialized_refresh_prepared = false;
        Ok(())
    }

    pub(super) fn prepare_refresh<W: WorkControl + ?Sized>(
        &mut self,
        nodes: &[SimNode],
        node_order: &[usize],
        allow_implicit_promotion: bool,
        work_control: &mut W,
    ) -> std::result::Result<(), WorkFailure> {
        if matches!(self.cells, RepulsionGridCells::Implicit { .. }) {
            return Ok(());
        }

        let implicit_work = allow_implicit_promotion
            .then(|| implicit_grid_work_units(node_order.len() as u128))
            .flatten()
            .and_then(|work| usize::try_from(work).ok());
        if let Some(work) =
            implicit_work.filter(|&work| self.scan_work_exceeds(nodes, node_order, work))
        {
            admit_dynamic_work(work_control, work)?;
            return self.switch_to_implicit(node_order);
        }

        // Reuse the empty buffer after reset if it fits the current bound. Leave any
        // previously published counts intact until the replacement succeeds.
        let mut candidate_visit_counts = if !self.materialized_refresh_prepared
            && self.candidate_visit_counts.capacity() <= nodes.len()
        {
            std::mem::take(&mut self.candidate_visit_counts)
        } else {
            Vec::new()
        };
        candidate_visit_counts
            .try_reserve_exact(nodes.len())
            .map_err(|_| WorkFailure::ArithmeticOverflow)?;
        candidate_visit_counts.resize(nodes.len(), 0);
        let mut total_candidate_visits = 0u128;
        let mut visit_count_overflowed = false;
        for &idx in node_order {
            let Some(node) = nodes.get(idx) else {
                continue;
            };
            let Some(bounds) = self.scan_bounds(node) else {
                continue;
            };
            let scan_cell_count = usize::try_from(bounds.cell_count())
                .map_err(|_| WorkFailure::ArithmeticOverflow)?;
            admit_dynamic_work(work_control, scan_cell_count)?;

            let mut visits = 0u128;
            for gx in bounds.start_x..=bounds.finish_x {
                for gy in bounds.start_y..=bounds.finish_y {
                    visits = visits.saturating_add(self.cell(gx, gy).len() as u128);
                }
            }
            match usize::try_from(visits) {
                Ok(count) => candidate_visit_counts[idx] = count,
                Err(_) => visit_count_overflowed = true,
            }
            total_candidate_visits = total_candidate_visits.saturating_add(visits);
        }

        if let Some(work) = implicit_work.filter(|&work| total_candidate_visits > work as u128) {
            admit_dynamic_work(work_control, work)?;
            return self.switch_to_implicit(node_order);
        }

        // Preserve the original error precedence: all scan admissions and the implicit
        // promotion decision come before rejecting an unrepresentable visit count.
        if visit_count_overflowed {
            return Err(WorkFailure::ArithmeticOverflow);
        }
        self.candidate_visit_counts = candidate_visit_counts;
        self.materialized_refresh_prepared = true;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_from_plan(
        grid: Option<Self>,
        plan: RepulsionGridPlan,
        left: f64,
        top: f64,
        nodes: &mut [SimNode],
        repulsion_range: f64,
        node_order: &[usize],
    ) -> std::result::Result<Self, WorkFailure> {
        let mut grid = grid.unwrap_or_else(|| Self::new(plan.storage_kind));
        grid.reset(plan, node_order)?;

        // Mirror layout-base `addNodeToGrid`: visit nodes in `getAllNodes()` order and preserve
        // insertion order within every materialized cell. The implicit representation stores only
        // the bounds; its query path reconstructs the same first-hit cell order.
        for &idx in node_order {
            let Some(node) = nodes.get(idx) else {
                continue;
            };
            let bounds = GridNodeBounds::from_node(node, left, top, repulsion_range)?;
            if let Some(node) = nodes.get_mut(idx) {
                node.grid_start_x = bounds.start_x;
                node.grid_finish_x = bounds.finish_x;
                node.grid_start_y = bounds.start_y;
                node.grid_finish_y = bounds.finish_y;
            }

            let Some((start_x, finish_x)) = bounds.clipped_x(grid.size_x) else {
                continue;
            };
            let Some((start_y, finish_y)) = bounds.clipped_y(grid.size_y) else {
                continue;
            };
            match &mut grid.cells {
                RepulsionGridCells::Dense(cells) => {
                    let size_y = usize::try_from(grid.size_y)
                        .map_err(|_| WorkFailure::ArithmeticOverflow)?;
                    for gx in start_x..=finish_x {
                        let gx =
                            usize::try_from(gx).map_err(|_| WorkFailure::ArithmeticOverflow)?;
                        for gy in start_y..=finish_y {
                            let gy =
                                usize::try_from(gy).map_err(|_| WorkFailure::ArithmeticOverflow)?;
                            let cell_idx = gx
                                .checked_mul(size_y)
                                .and_then(|base| base.checked_add(gy))
                                .ok_or(WorkFailure::ArithmeticOverflow)?;
                            cells
                                .get_mut(cell_idx)
                                .ok_or(WorkFailure::ArithmeticOverflow)?
                                .push(idx);
                        }
                    }
                }
                RepulsionGridCells::Sparse(cells) => {
                    for gx in start_x..=finish_x {
                        for gy in start_y..=finish_y {
                            cells.entry((gx, gy)).or_default().push(idx);
                        }
                    }
                }
                RepulsionGridCells::Implicit { .. } => {}
            }
        }

        Ok(grid)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_or_reuse<W: WorkControl + ?Sized>(
        grid: Option<Self>,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        nodes: &mut [SimNode],
        repulsion_range: f64,
        node_order: &[usize],
        work_control: &mut W,
    ) -> std::result::Result<Option<Self>, WorkFailure> {
        let Some(plan) = RepulsionGridPlan::from_geometry(
            left,
            top,
            right,
            bottom,
            nodes,
            repulsion_range,
            node_order,
        )?
        else {
            return Ok(None);
        };

        // The plan is computed without mutating node grid coordinates. Reject before allocating
        // the selected representation or populating any cell.
        admit_dynamic_work(work_control, plan.work_units())?;
        let mut grid =
            Self::build_from_plan(grid, plan, left, top, nodes, repulsion_range, node_order)?;
        grid.prepare_refresh(nodes, node_order, true, work_control)?;
        Ok(Some(grid))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn refresh_node_surrounding<W: WorkControl + ?Sized>(
        &mut self,
        node_idx: usize,
        nodes: &mut [SimNode],
        processed_generation: &[u32],
        current_processed_generation: u32,
        repulsion_range: f64,
        surrounding_seen: &mut [u32],
        surrounding_seen_generation: &mut u32,
        work_control: &mut W,
    ) -> std::result::Result<(), WorkFailure> {
        if node_idx >= nodes.len() {
            return Ok(());
        }
        let Some(scan_bounds) = self.scan_bounds(&nodes[node_idx]) else {
            nodes[node_idx].surrounding.clear();
            return Ok(());
        };
        let GridScanBounds {
            start_x: scan_start_x,
            finish_x: scan_finish_x,
            start_y: scan_start_y,
            finish_y: scan_finish_y,
        } = scan_bounds;

        if !matches!(self.cells, RepulsionGridCells::Implicit { .. }) {
            if !self.materialized_refresh_prepared {
                return Err(WorkFailure::ArithmeticOverflow);
            }
            let candidate_visit_count = self
                .candidate_visit_counts
                .get(node_idx)
                .copied()
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            admit_dynamic_work(work_control, candidate_visit_count)?;
        }

        if let RepulsionGridCells::Implicit {
            node_order,
            candidates,
        } = &mut self.cells
        {
            candidates.clear();
            for (insertion_order, &other) in node_order.iter().enumerate() {
                let Some(other_node) = nodes.get(other) else {
                    continue;
                };
                let first_x = other_node.grid_start_x.max(scan_start_x);
                let last_x = other_node.grid_finish_x.min(scan_finish_x);
                let first_y = other_node.grid_start_y.max(scan_start_y);
                let last_y = other_node.grid_finish_y.min(scan_finish_y);
                if first_x <= last_x && first_y <= last_y {
                    candidates.push(ImplicitGridCandidate {
                        first_x,
                        first_y,
                        insertion_order,
                        node_idx: other,
                    });
                }
            }
            candidates.sort_unstable_by_key(|candidate| {
                (
                    candidate.first_x,
                    candidate.first_y,
                    candidate.insertion_order,
                )
            });
        }

        let node_owner_idx = nodes[node_idx].owner_idx;
        let node_center_x = nodes[node_idx].center_x();
        let node_center_y = nodes[node_idx].center_y();
        let node_half_w = nodes[node_idx].half_w();
        let node_half_h = nodes[node_idx].half_h();
        let node_count = nodes.len();
        let (left, rest) = nodes.split_at_mut(node_idx);
        let (node, right) = rest
            .split_first_mut()
            .expect("node_idx checked against nodes len");
        let left: &[SimNode] = left;
        let right: &[SimNode] = right;
        let surrounding = &mut node.surrounding;
        surrounding.clear();
        *surrounding_seen_generation = surrounding_seen_generation.wrapping_add(1);
        if *surrounding_seen_generation == 0 {
            surrounding_seen.fill(0);
            *surrounding_seen_generation = 1;
        }
        let generation = *surrounding_seen_generation;
        let mut consider = |other: usize| {
            if other == node_idx
                || other >= node_count
                || other >= processed_generation.len()
                || other >= surrounding_seen.len()
                || processed_generation[other] == current_processed_generation
                || surrounding_seen[other] == generation
            {
                return;
            }
            let other_node = if other < node_idx {
                &left[other]
            } else {
                &right[other - node_idx - 1]
            };
            if node_owner_idx != other_node.owner_idx {
                return;
            }

            let dx =
                (node_center_x - other_node.center_x()).abs() - (node_half_w + other_node.half_w());
            let dy =
                (node_center_y - other_node.center_y()).abs() - (node_half_h + other_node.half_h());
            if dx <= repulsion_range && dy <= repulsion_range {
                surrounding_seen[other] = generation;
                surrounding.push(other);
            }
        };

        match &self.cells {
            RepulsionGridCells::Dense(_) | RepulsionGridCells::Sparse(_) => {
                for gx in scan_start_x..=scan_finish_x {
                    for gy in scan_start_y..=scan_finish_y {
                        for &other in self.cell(gx, gy) {
                            consider(other);
                        }
                    }
                }
            }
            RepulsionGridCells::Implicit { candidates, .. } => {
                for candidate in candidates {
                    consider(candidate.node_idx);
                }
            }
        }
        Ok(())
    }
}
