use super::super::grid::{AxisSizes, effective_axis_size, set_axis_size};
use super::super::{GridCoord, NodeLayout};
use super::bounds::{RawBounds, raw_group_bounds_batch};
use super::layout_work_allocation_failed;
use super::members::group_member_indices;
use crate::error::Result;
use crate::graph::model::{AsciiGraph, GraphDirection};
use crate::graph::topology::GraphGroupTopology;
use crate::operation::AsciiExecution;
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use merman_core::OperationPhase;

/// One visibly empty cell separates unrelated frames and their contents.
const COMPOUND_READING_GUTTER: isize = 1;

#[derive(Clone, Copy)]
struct GridBounds {
    x: usize,
    y: usize,
    right: usize,
    bottom: usize,
}

impl GridBounds {
    fn for_node(coord: GridCoord, resources: &ResourceContext) -> Result<Self> {
        Ok(Self {
            x: coord.x,
            y: coord.y,
            right: resources.checked_grid_add(coord.x, 2)?,
            bottom: resources.checked_grid_add(coord.y, 2)?,
        })
    }

    fn include(&mut self, other: Self) {
        self.x = self.x.min(other.x);
        self.y = self.y.min(other.y);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }
}

struct GroupEnvelope {
    members: Vec<usize>,
    grid: GridBounds,
    canvas: RawBounds,
}

/// Convert measured frame overhang into shared axis dimensions before final projection. A
/// constraint expands the separator band, so both member nodes and all containing frames move
/// together. The existing ranking and group membership remain the placement authority.
#[allow(clippy::too_many_arguments)]
pub(in crate::graph::layout) fn reserve_compound_axis_spacing(
    graph: &AsciiGraph,
    nodes: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let mut envelopes = Vec::new();
    envelopes
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    let group_bounds =
        raw_group_bounds_batch(graph, nodes, topology, policy, resources, execution)?;
    for group_index in 0..graph.groups.len() {
        execution.checkpoint_loop(OperationPhase::Layout, group_index)?;
        let members = group_member_indices(topology, group_index, resources)?;
        let Some(first) = members.first().and_then(|index| nodes.get(*index)) else {
            // Empty groups place their own closed envelope outside measured sibling frames.
            // Their bounds already participate in every containing group's measured envelope;
            // only groups with member bands need an additional shared-axis expansion here.
            continue;
        };
        let mut grid = GridBounds::for_node(first.grid, resources)?;
        resources.charge_layout_work(members.len())?;
        for (member_index, index) in members.iter().copied().enumerate() {
            execution.checkpoint_loop(OperationPhase::Layout, member_index)?;
            if let Some(node) = nodes.get(index) {
                grid.include(GridBounds::for_node(node.grid, resources)?);
            }
        }
        let Some(canvas) = group_bounds.get(group_index).copied().flatten() else {
            continue;
        };
        envelopes.push(GroupEnvelope {
            members,
            grid,
            canvas,
        });
    }

    let mut column_requirements = AxisSizes::default();
    let mut row_requirements = AxisSizes::default();
    let pair_work = resources.checked_work_add(
        resources.checked_work_mul(envelopes.len(), nodes.len())?,
        resources.checked_work_mul(envelopes.len(), envelopes.len().saturating_sub(1))? / 2,
    )?;
    resources.charge_layout_work(pair_work)?;
    for (index, envelope) in envelopes.iter().enumerate() {
        execution.checkpoint_loop(OperationPhase::Layout, index)?;
        for (node_index, node) in nodes.iter().enumerate() {
            execution.checkpoint_loop(OperationPhase::Layout, node_index)?;
            if envelope.members.binary_search(&node_index).is_ok() {
                continue;
            }
            reserve_pair_spacing(
                envelope.grid,
                envelope.canvas,
                GridBounds::for_node(node.grid, resources)?,
                node_canvas_bounds(node, resources)?,
                graph.direction,
                column_widths,
                row_heights,
                &mut column_requirements,
                &mut row_requirements,
                resources,
            )?;
        }
        for (other_index, other) in envelopes.iter().enumerate().skip(index + 1) {
            execution.checkpoint_loop(OperationPhase::Layout, other_index)?;
            // Ancestors and descendants have overlapping member-grid envelopes, so they
            // have no separating axis and do not acquire a sibling gutter.
            reserve_pair_spacing(
                envelope.grid,
                envelope.canvas,
                other.grid,
                other.canvas,
                graph.direction,
                column_widths,
                row_heights,
                &mut column_requirements,
                &mut row_requirements,
                resources,
            )?;
        }
    }

    // Admission and all measurements finish before mutating either axis.
    resources.charge_layout_work(
        resources.checked_work_add(column_requirements.len(), row_requirements.len())?,
    )?;
    column_widths
        .try_reserve(column_requirements.len())
        .map_err(|_| layout_work_allocation_failed())?;
    row_heights
        .try_reserve(row_requirements.len())
        .map_err(|_| layout_work_allocation_failed())?;
    for (index, requirement) in column_requirements {
        set_axis_size(column_widths, index, requirement);
    }
    for (index, requirement) in row_requirements {
        set_axis_size(row_heights, index, requirement);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reserve_pair_spacing(
    left_grid: GridBounds,
    left_canvas: RawBounds,
    right_grid: GridBounds,
    right_canvas: RawBounds,
    direction: GraphDirection,
    columns: &AxisSizes,
    rows: &AxisSizes,
    column_requirements: &mut AxisSizes,
    row_requirements: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    let separated_x = left_grid.right < right_grid.x || right_grid.right < left_grid.x;
    let separated_y = left_grid.bottom < right_grid.y || right_grid.bottom < left_grid.y;
    if !separated_x && !separated_y {
        return Ok(());
    }
    let horizontal =
        separated_x && (!separated_y || direction.canonical() == GraphDirection::LeftRight);
    let (before_end, after_start, before_border, after_border, sizes, requirements) = if horizontal
    {
        if left_grid.right < right_grid.x {
            (
                left_grid.right,
                right_grid.x,
                left_canvas.right,
                right_canvas.x,
                columns,
                column_requirements,
            )
        } else {
            (
                right_grid.right,
                left_grid.x,
                right_canvas.right,
                left_canvas.x,
                columns,
                column_requirements,
            )
        }
    } else if left_grid.bottom < right_grid.y {
        (
            left_grid.bottom,
            right_grid.y,
            left_canvas.bottom,
            right_canvas.y,
            rows,
            row_requirements,
        )
    } else {
        (
            right_grid.bottom,
            left_grid.y,
            right_canvas.bottom,
            left_canvas.y,
            rows,
            row_requirements,
        )
    };
    // A separator band must be outside both member blocks. Placement handles any blocks whose
    // coarse envelopes have no separator; axis sizing cannot change their relative membership.
    let separator = resources.checked_grid_add(before_end, 1)?;
    if separator >= after_start {
        return Ok(());
    }
    let current_gutter = after_border
        .checked_sub(before_border)
        .and_then(|gap| gap.checked_sub(1))
        .ok_or_else(|| resources.grid_overflow())?;
    let extra = COMPOUND_READING_GUTTER
        .checked_sub(current_gutter)
        .ok_or_else(|| resources.grid_overflow())?;
    if extra <= 0 {
        return Ok(());
    }
    let extra = usize::try_from(extra).map_err(|_| resources.grid_overflow())?;
    let required_size = resources.checked_grid_add(effective_axis_size(sizes, separator), extra)?;
    if !requirements.contains_key(&separator) {
        requirements
            .try_reserve(1)
            .map_err(|_| layout_work_allocation_failed())?;
    }
    set_axis_size(requirements, separator, required_size);
    Ok(())
}

fn node_canvas_bounds(node: &NodeLayout, resources: &ResourceContext) -> Result<RawBounds> {
    let right = resources.checked_grid_add(node.x, node.width.saturating_sub(1))?;
    let bottom = resources.checked_grid_add(node.y, node.height.saturating_sub(1))?;
    Ok(RawBounds {
        x: isize::try_from(node.x).map_err(|_| resources.grid_overflow())?,
        y: isize::try_from(node.y).map_err(|_| resources.grid_overflow())?,
        right: isize::try_from(right).map_err(|_| resources.grid_overflow())?,
        bottom: isize::try_from(bottom).map_err(|_| resources.grid_overflow())?,
    })
}
