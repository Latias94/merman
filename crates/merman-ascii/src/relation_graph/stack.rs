use super::{
    RelationGraphBox, RelationGraphLine, assemble_vertical_stack_lines, grid_overflow,
    layout_allocation_failed, vertical_center, vertical_stack_extent,
};
use crate::Result;
use crate::resource::{LogicalExtent, ResourceContext};
use crate::text::StyledLine;

#[derive(Debug)]
pub(crate) struct RelationStackPlan<'a> {
    top: &'a RelationGraphBox,
    bottom: &'a RelationGraphBox,
    center: usize,
    relation_extent: LogicalExtent,
    extent: LogicalExtent,
}

impl<'a> RelationStackPlan<'a> {
    pub(crate) fn try_new(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        extra_half_widths: &[usize],
        resources: &ResourceContext,
        measure_rows: impl FnOnce(usize, &ResourceContext) -> Result<LogicalExtent>,
    ) -> Result<Self> {
        debug_assert_eq!(
            top.width_profile(),
            bottom.width_profile(),
            "stacked relation boxes must share one terminal width profile"
        );
        let center = vertical_center(top, bottom, extra_half_widths);
        let relation_extent = measure_rows(center, resources)?;
        let extent = vertical_stack_extent(top, bottom, center, relation_extent, resources)?;
        Ok(Self {
            top,
            bottom,
            center,
            relation_extent,
            extent,
        })
    }

    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.extent
    }

    pub(crate) fn render_lines(
        self,
        resources: &mut ResourceContext,
        materialize_rows: impl FnOnce(usize, &ResourceContext) -> Result<Vec<RelationGraphLine>>,
    ) -> Result<Vec<RelationGraphLine>> {
        resources.charge_layout_work(self.extent.cells())?;
        let relation_lines = materialize_rows(self.center, resources)?;
        self.validate_relation_lines(&relation_lines, resources)?;
        assemble_vertical_stack_lines(
            self.top,
            self.bottom,
            self.center,
            relation_lines,
            self.extent,
            resources,
        )
    }

    fn validate_relation_lines(
        &self,
        relation_lines: &[RelationGraphLine],
        resources: &ResourceContext,
    ) -> Result<()> {
        let width = relation_lines
            .iter()
            .map(RelationGraphLine::width)
            .max()
            .unwrap_or(0);
        if relation_lines.len() != self.relation_extent.height()
            || width != self.relation_extent.width()
            || relation_lines
                .iter()
                .any(|line| line.width_profile() != self.top.width_profile())
        {
            return Err(grid_overflow(resources));
        }
        Ok(())
    }
}

pub(crate) fn centered_row_blocks_extent(
    center: usize,
    blocks: impl IntoIterator<Item = (usize, usize)>,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let mut width = 0usize;
    let mut height = 0usize;
    for (block_width, row_count) in blocks {
        if row_count == 0 {
            continue;
        }
        let left = center
            .checked_sub(block_width / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        width = width.max(resources.checked_grid_add(left, block_width)?);
        height = resources.checked_grid_add(height, row_count)?;
    }
    resources.grid_extent(width, height)
}

/// A geometry-only parallel plan. Family-owned styled rows are materialized only
/// after port validation and aggregate grid admission succeed.
#[derive(Debug)]
pub(crate) struct RelationParallelPlan<'a> {
    top: &'a RelationGraphBox,
    bottom: &'a RelationGraphBox,
    center: usize,
    lane_left: usize,
    lane_gap: usize,
    lane_extents: Vec<LogicalExtent>,
    extent: LogicalExtent,
}

impl<'a> RelationParallelPlan<'a> {
    pub(crate) fn new(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        lane_extents: Vec<LogicalExtent>,
        lane_gap: usize,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        debug_assert_eq!(
            top.width_profile(),
            bottom.width_profile(),
            "parallel relation boxes must share one terminal width profile"
        );
        resources.charge_layout_work(lane_extents.len().max(1))?;
        for lane_extent in &lane_extents {
            resources.grid_extent(lane_extent.width().max(1), lane_extent.height())?;
        }
        let lanes_content_width = lane_extents.iter().try_fold(0usize, |total, extent| {
            resources.checked_grid_add(total, extent.width().max(1))
        })?;
        let gap_count = lane_extents.len().saturating_sub(1);
        let gaps_width = resources.checked_grid_mul(lane_gap, gap_count)?;
        let lanes_width = resources.checked_grid_add(lanes_content_width, gaps_width)?;
        let lane_center = lanes_width / 2;
        let center = (top.width / 2).max(bottom.width / 2).max(lane_center);
        let lane_left = center - lane_center;
        let extent = parallel_relation_extent(
            top,
            bottom,
            center,
            lane_left,
            lane_gap,
            &lane_extents,
            resources,
        )?;

        Ok(Self {
            top,
            bottom,
            center,
            lane_left,
            lane_gap,
            lane_extents,
            extent,
        })
    }

    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.extent
    }

    /// Check that every lane's stem column lands on both endpoint box faces.
    ///
    /// Lane cells are centered with the same left-padding rule used by
    /// `render_lines`; keeping this calculation here prevents family renderers
    /// from accepting a visually plausible but disconnected parallel layout.
    pub(crate) fn ports_fit(&self, resources: &ResourceContext) -> Result<bool> {
        let top_left = self
            .center
            .checked_sub(self.top.width / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        let bottom_left = self
            .center
            .checked_sub(self.bottom.width / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        let top_right = resources.checked_grid_add(top_left, self.top.width.saturating_sub(1))?;
        let bottom_right =
            resources.checked_grid_add(bottom_left, self.bottom.width.saturating_sub(1))?;

        let mut lane_left = self.lane_left;
        for (lane_index, lane_extent) in self.lane_extents.iter().copied().enumerate() {
            let lane_width = lane_extent.width().max(1);
            let lane_anchor =
                resources.checked_grid_add(lane_left, lane_width.saturating_sub(1) / 2)?;
            if !(top_left..=top_right).contains(&lane_anchor)
                || !(bottom_left..=bottom_right).contains(&lane_anchor)
            {
                return Ok(false);
            }
            lane_left = resources.checked_grid_add(lane_left, lane_width)?;
            if lane_index + 1 < self.lane_extents.len() {
                lane_left = resources.checked_grid_add(lane_left, self.lane_gap)?;
            }
        }
        Ok(true)
    }

    pub(crate) fn render_lines(
        &self,
        resources: &mut ResourceContext,
        materialize_lanes: impl FnOnce(&mut ResourceContext) -> Result<Vec<Vec<RelationGraphLine>>>,
    ) -> Result<Vec<RelationGraphLine>> {
        resources.charge_layout_work(self.extent.cells())?;
        let lanes = materialize_lanes(resources)?;
        if lanes.len() != self.lane_extents.len() {
            return Err(grid_overflow(resources));
        }
        for (lane, planned) in lanes.iter().zip(&self.lane_extents) {
            let actual_width = lane.iter().map(RelationGraphLine::width).max().unwrap_or(1);
            if lane.len() != planned.height()
                || actual_width != planned.width().max(1)
                || lane
                    .iter()
                    .any(|line| line.width_profile() != self.top.width_profile())
            {
                return Err(grid_overflow(resources));
            }
        }

        let mut relation_lines = Vec::new();
        let row_count = self.row_count();
        relation_lines
            .try_reserve_exact(row_count)
            .map_err(|_| layout_allocation_failed())?;
        for row_index in 0..row_count {
            let mut line = StyledLine::with_resources(self.top.width_profile(), resources);
            line.try_push_spaces(self.lane_left)?;
            for (lane_index, lane) in lanes.iter().enumerate() {
                if lane_index > 0 {
                    line.try_push_spaces(self.lane_gap)?;
                }
                let lane_width = self.lane_extents[lane_index].width().max(1);
                let Some(cell) = lane.get(row_index) else {
                    line.try_push_spaces(lane_width)?;
                    continue;
                };
                let remaining = lane_width
                    .checked_sub(cell.width())
                    .ok_or_else(|| grid_overflow(resources))?;
                let left_padding = remaining / 2;
                let right_padding = remaining
                    .checked_sub(left_padding)
                    .ok_or_else(|| grid_overflow(resources))?;
                line.try_push_spaces(left_padding)?;
                line.try_push_line(&cell.line)?;
                line.try_push_spaces(right_padding)?;
            }
            relation_lines.push(RelationGraphLine::from_styled(line));
        }

        assemble_vertical_stack_lines(
            self.top,
            self.bottom,
            self.center,
            relation_lines,
            self.extent,
            resources,
        )
    }

    fn row_count(&self) -> usize {
        self.lane_extents
            .iter()
            .map(|extent| extent.height())
            .max()
            .unwrap_or(0)
    }
}

fn parallel_relation_extent(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    lane_left: usize,
    lane_gap: usize,
    lane_extents: &[LogicalExtent],
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let row_count = lane_extents
        .iter()
        .map(|extent| extent.height())
        .max()
        .unwrap_or(0);
    let height = resources.checked_grid_add(
        resources.checked_grid_add(top.height(), row_count)?,
        bottom.height(),
    )?;
    let lanes_width = lane_extents.iter().try_fold(0usize, |total, extent| {
        resources.checked_grid_add(total, extent.width().max(1))
    })?;
    let gaps_width = resources.checked_grid_mul(lane_gap, lane_extents.len().saturating_sub(1))?;
    let relation_width = resources.checked_grid_add(
        lane_left,
        resources.checked_grid_add(lanes_width, gaps_width)?,
    )?;
    let top_left = center
        .checked_sub(top.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let bottom_left = center
        .checked_sub(bottom.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let top_width = resources.checked_grid_add(top_left, top.width())?;
    let bottom_width = resources.checked_grid_add(bottom_left, bottom.width())?;
    resources.grid_extent(relation_width.max(top_width).max(bottom_width), height)
}
