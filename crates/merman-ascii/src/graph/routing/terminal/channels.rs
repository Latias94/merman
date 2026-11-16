use super::{
    FrameSegment, NodeAttachments, Position, SearchGeometry, TerminalPort, axis_mask,
    direction_mask,
};
use crate::error::Result;
use crate::graph::layout::{CanvasCoord, charge_sort_work};
use crate::graph::routing::layout_allocation_failed;
use crate::graph::routing::path::{RouteObstacles, StepDirection};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ChannelPoint {
    column: usize,
    row: usize,
}

/// An implicit orthogonal graph: only the axes are stored, never their Cartesian product.
/// All port pairs in one search share the same scene and terminal escape coordinates.
pub(super) struct ChannelAxes {
    pub(super) x: Vec<usize>,
    pub(super) y: Vec<usize>,
}

impl ChannelAxes {
    pub(super) fn new(
        obstacles: RouteObstacles<'_>,
        starts: &[TerminalPort],
        ends: &[TerminalPort],
        bounds: CanvasCoord,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        execution.checkpoint(OperationPhase::Layout)?;
        let (x, y) = obstacles
            .scene
            .routing_channel_coordinates(obstacles.routes, resources)?;
        let mut axes = Self { x, y };
        axes.push(CanvasCoord { x: 0, y: 0 }, resources, execution)?;
        axes.push(bounds, resources, execution)?;
        for port in starts.iter().chain(ends) {
            for coord in [port.berth, port.attachment.contact] {
                // Include the first possible bend after departure and before arrival,
                // including contacts inset from a nonrectangular node's bounding box.
                for offset in 0..=2 {
                    execution.checkpoint(OperationPhase::Layout)?;
                    resources.charge_layout_work(1)?;
                    for x in [coord.x.checked_sub(offset), coord.x.checked_add(offset)]
                        .into_iter()
                        .flatten()
                    {
                        push_axis(&mut axes.x, x, resources, execution)?;
                    }
                    for y in [coord.y.checked_sub(offset), coord.y.checked_add(offset)]
                        .into_iter()
                        .flatten()
                    {
                        push_axis(&mut axes.y, y, resources, execution)?;
                    }
                }
            }
        }
        finish_axis(&mut axes.x, bounds.x, resources, execution)?;
        finish_axis(&mut axes.y, bounds.y, resources, execution)?;
        Ok(axes)
    }

    fn push(
        &mut self,
        coord: CanvasCoord,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        push_axis(&mut self.x, coord.x, resources, execution)?;
        push_axis(&mut self.y, coord.y, resources, execution)
    }

    pub(super) fn locate(
        &self,
        coord: CanvasCoord,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Option<ChannelPoint>> {
        let Some(column) = locate_axis(&self.x, coord.x, resources, execution)? else {
            return Ok(None);
        };
        let Some(row) = locate_axis(&self.y, coord.y, resources, execution)? else {
            return Ok(None);
        };
        Ok(Some(ChannelPoint { column, row }))
    }

    pub(super) fn coord(&self, point: ChannelPoint) -> Option<CanvasCoord> {
        Some(CanvasCoord {
            x: *self.x.get(point.column)?,
            y: *self.y.get(point.row)?,
        })
    }

    pub(super) fn neighbor(
        &self,
        point: ChannelPoint,
        heading: StepDirection,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Option<ChannelPoint>> {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let mut next = point;
        let adjacent = match heading {
            StepDirection::Left => point.column.checked_sub(1),
            StepDirection::Right => point.column.checked_add(1),
            StepDirection::Up => point.row.checked_sub(1),
            StepDirection::Down => point.row.checked_add(1),
        };
        let Some(adjacent) = adjacent else {
            return Ok(None);
        };
        match heading {
            StepDirection::Left | StepDirection::Right => next.column = adjacent,
            StepDirection::Up | StepDirection::Down => next.row = adjacent,
        }
        Ok(self.coord(next).map(|_| next))
    }
}

fn locate_axis(
    axis: &[usize],
    value: usize,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<usize>> {
    execution.checkpoint(OperationPhase::Layout)?;
    let mut first = 0;
    let mut last = axis.len();
    while first < last {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let middle = first + (last - first) / 2;
        if axis[middle] < value {
            first = middle + 1;
        } else {
            last = middle;
        }
    }
    Ok((axis.get(first) == Some(&value)).then_some(first))
}

fn push_axis(
    axis: &mut Vec<usize>,
    value: usize,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(OperationPhase::Layout)?;
    resources.charge_layout_work(1)?;
    axis.try_reserve(1)
        .map_err(|_| layout_allocation_failed())?;
    axis.push(value);
    Ok(())
}

fn finish_axis(
    axis: &mut Vec<usize>,
    bound: usize,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(OperationPhase::Layout)?;
    charge_sort_work(axis.len(), resources)?;
    axis.sort_unstable();
    let mut retained = 0;
    for index in 0..axis.len() {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let value = axis[index];
        if value <= bound && (retained == 0 || axis[retained - 1] != value) {
            axis[retained] = value;
            retained += 1;
        }
    }
    axis.truncate(retained);
    Ok(())
}

pub(super) struct SegmentCost {
    pub(super) length: usize,
    pub(super) shared: usize,
}

/// A graph edge skips search states, not occupancy checks. The departure cell gets its
/// actual corner mask; every traversed cell and group step retains the unit semantics.
pub(super) fn segment_cost(
    geometry: SearchGeometry<'_>,
    from: Position,
    to: CanvasCoord,
    heading: StepDirection,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<SegmentCost>> {
    execution.checkpoint(OperationPhase::Layout)?;
    resources.charge_layout_work(1)?;
    let attachments = Some(NodeAttachments {
        start: geometry.start.attachment,
        end: geometry.end.attachment,
    });
    let corner = direction_mask(from.heading.opposite()) | direction_mask(heading);
    if !geometry.obstacles.scene.route_cell_is_clear(
        geometry.obstacles.routes,
        geometry.obstacles.owner,
        attachments,
        from.coord,
        corner,
        resources,
    )? {
        return Ok(None);
    }
    let mut cost = SegmentCost {
        length: 0,
        shared: 0,
    };
    let mut previous = from.coord;
    while previous != to {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let Some(next) = super::unit_neighbor(previous, heading, geometry.bounds) else {
            return Ok(None);
        };
        if geometry
            .groups
            .segment(previous, next, resources, execution)?
            == FrameSegment::Blocked
            || !geometry.obstacles.scene.route_cell_is_clear(
                geometry.obstacles.routes,
                geometry.obstacles.owner,
                attachments,
                next,
                axis_mask(heading),
                resources,
            )?
        {
            return Ok(None);
        }
        let congestion = geometry.obstacles.scene.route_cell_congestion(
            geometry.obstacles.routes,
            geometry.obstacles.owner,
            attachments,
            next,
            resources,
        )?;
        cost.shared = resources.checked_work_add(cost.shared, congestion)?;
        cost.length = resources.checked_work_add(cost.length, 1)?;
        previous = next;
    }
    Ok(Some(cost))
}
