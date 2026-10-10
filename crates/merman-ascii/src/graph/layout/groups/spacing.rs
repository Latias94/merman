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
    resources.transaction(|resources| {
        reserve_compound_axis_spacing_inner(
            graph,
            nodes,
            topology,
            policy,
            column_widths,
            row_heights,
            resources,
            execution,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn reserve_compound_axis_spacing_inner(
    graph: &AsciiGraph,
    nodes: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(OperationPhase::Layout)?;
    resources.charge_layout_work(graph.groups.len())?;
    let mut envelopes = Vec::new();
    envelopes
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    let group_bounds =
        raw_group_bounds_batch(graph, nodes, topology, policy, resources, execution)?;
    let mut membership_resources = resources.clone();
    for group_index in 0..graph.groups.len() {
        execution.checkpoint_loop(OperationPhase::Layout, group_index)?;
        let members = group_member_indices(topology, group_index, &mut membership_resources)?;
        let Some(first) = members.first().and_then(|index| nodes.get(*index)) else {
            // Zero-node subtrees are packed as complete physical envelopes in their immediate
            // scopes by raw_group_bounds_batch. They have no node bands to expand; every anchored
            // parent includes their full overhang in its own axis-separator requirements.
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

    reserve_envelope_spacing(
        &envelopes,
        nodes,
        graph.direction,
        column_widths,
        row_heights,
        resources,
        execution,
    )
}

/// Admit one finite envelope comparison pass before applying either shared-axis result.
#[allow(clippy::too_many_arguments)]
fn reserve_envelope_spacing(
    envelopes: &[GroupEnvelope],
    nodes: &[NodeLayout],
    direction: GraphDirection,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
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
                direction,
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
                direction,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::label::GraphLabel;
    use crate::graph::model::{GraphNodeShape, GraphNodeStyle};
    use crate::resource::{
        AsciiResourceLimitCause, AsciiResourceLimitId, AsciiResourceLimitPhase, AsciiResourcePolicy,
    };
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl};

    fn sibling_nodes_and_envelopes() -> (Vec<NodeLayout>, Vec<GroupEnvelope>) {
        let nodes = vec![
            NodeLayout {
                id: "A".into(),
                label: GraphLabel::new("A"),
                shape: GraphNodeShape::Rect,
                style: GraphNodeStyle::default(),
                grid: GridCoord { x: 0, y: 0 },
                x: 2,
                y: 4,
                width: 3,
                height: 3,
            },
            NodeLayout {
                id: "B".into(),
                label: GraphLabel::new("B"),
                shape: GraphNodeShape::Rect,
                style: GraphNodeStyle::default(),
                grid: GridCoord { x: 4, y: 0 },
                x: 6,
                y: 4,
                width: 3,
                height: 3,
            },
        ];
        let envelopes = vec![
            GroupEnvelope {
                members: vec![0],
                grid: GridBounds {
                    x: 0,
                    y: 0,
                    right: 2,
                    bottom: 2,
                },
                canvas: RawBounds {
                    x: 0,
                    y: 0,
                    right: 6,
                    bottom: 8,
                },
            },
            GroupEnvelope {
                members: vec![1],
                grid: GridBounds {
                    x: 4,
                    y: 0,
                    right: 6,
                    bottom: 2,
                },
                canvas: RawBounds {
                    x: 4,
                    y: 0,
                    right: 10,
                    bottom: 8,
                },
            },
        ];
        (nodes, envelopes)
    }

    #[test]
    fn compound_pair_spacing_admits_exact_work_and_rolls_back_max_minus_one() {
        // Two group/node comparisons each plus one group pair cost five units. One committed
        // separator requirement costs the sixth unit. This fixed ledger is independent of any
        // successful measurement and covers the actual shared pair pass used after bounds.
        const EXPECTED_WORK: usize = 6;
        let (nodes, envelopes) = sibling_nodes_and_envelopes();
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        for max in [EXPECTED_WORK, EXPECTED_WORK - 1] {
            let policy = unbounded
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, max)
                .expect("small layout budget should be valid");
            let resources = ResourceContext::new(policy);
            let mut columns = AxisSizes::default();
            let mut rows = AxisSizes::default();
            columns.insert(3, 2);
            rows.insert(3, 7);
            let original_columns = columns.clone();
            let original_rows = rows.clone();
            let result = resources.transaction(|resources| {
                reserve_envelope_spacing(
                    &envelopes,
                    &nodes,
                    GraphDirection::TopDown,
                    &mut columns,
                    &mut rows,
                    resources,
                    AsciiExecution::for_test(&policy),
                )
            });
            if max == EXPECTED_WORK {
                result.expect("exact pair-work and commit budget should pass");
                assert_eq!(resources.layout_work_used(), EXPECTED_WORK);
                assert_eq!(columns.get(&3), Some(&6));
                assert_eq!(rows, original_rows);
            } else {
                let error = result.expect_err("commit work must reject max-minus-one");
                assert!(
                    matches!(error, crate::AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                        && details.actual == EXPECTED_WORK && details.max == max
                        && details.phase() == AsciiResourceLimitPhase::LayoutWork
                        && details.cause == AsciiResourceLimitCause::Ceiling
                        && details.profile == ResourceProfile::UnboundedForTrustedInput)
                );
                assert_eq!(resources.layout_work_used(), 0);
                assert_eq!(columns, original_columns);
                assert_eq!(rows, original_rows);
            }
        }
    }

    #[test]
    fn compound_pair_spacing_cancels_inside_comparisons_without_partial_axes() {
        let (nodes, envelopes) = sibling_nodes_and_envelopes();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        for transactional in [false, true] {
            let control = OperationControl::new();
            // Three checked pair-count operations and the work charge succeed. The first
            // envelope and its member-node checkpoints succeed. Cancellation interrupts a
            // checked grid addition for the foreign node inside the comparison loop.
            control.cancel_after_checkpoints(6);
            let resources =
                ResourceContext::new(policy).controlled(control.clone(), OperationPhase::Layout);
            let mut columns = AxisSizes::default();
            let mut rows = AxisSizes::default();
            columns.insert(3, 2);
            rows.insert(3, 7);
            let original_columns = columns.clone();
            let original_rows = rows.clone();
            let mut compare = |resources: &ResourceContext| {
                reserve_envelope_spacing(
                    &envelopes,
                    &nodes,
                    GraphDirection::TopDown,
                    &mut columns,
                    &mut rows,
                    resources,
                    AsciiExecution::new(&control, &policy),
                )
            };
            let result = if transactional {
                resources.transaction(compare)
            } else {
                compare(&resources)
            };
            let error =
                result.expect_err("cancellation must interrupt the envelope comparison stage");
            assert!(matches!(error, crate::AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested));
            assert_eq!(
                resources.layout_work_used(),
                if transactional { 0 } else { 5 },
                "the unwrapped pass proves pair work was admitted before cancellation; the wrapped pass proves rollback"
            );
            assert_eq!(columns, original_columns);
            assert_eq!(rows, original_rows);
        }
    }
}
