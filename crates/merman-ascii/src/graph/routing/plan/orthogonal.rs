use super::super::super::charset::GraphCharset;
use super::super::super::layout::CanvasCoord;
use super::super::super::model::{AsciiGraphEdge, GraphEdgeMarker};
use super::super::cell::{DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP};
use super::super::label::RoutedLabelDescriptor;
use super::super::path::StepDirection;
use super::{
    MarkerAnchor, MarkerAnchors, NodeAttachments, PlannedRouteCells, PlannedRouteLabel, RoutePlan,
    edge_line_cell, planned_label, route_cell,
};
use crate::error::Result;
use crate::resource::ResourceContext;

pub(in crate::graph::routing) fn plan_terminal_path(
    path: &[CanvasCoord],
    attachments: NodeAttachments,
    edge: &AsciiGraphEdge,
    label: Option<PlannedRouteLabel>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if path.is_empty() {
        return Ok(None);
    }
    let NodeAttachments { start, end } = attachments;
    if path.len() == 1
        && (start.outward != end.outward.opposite()
            || (edge.start_marker != GraphEdgeMarker::Open
                && edge.end_marker != GraphEdgeMarker::Open))
    {
        return Ok(None);
    }
    let mut cells = PlannedRouteCells::new();
    if let Some(connector) = start.connector {
        cells.try_push(resources, || {
            edge_line_cell(start.contact.x, start.contact.y, connector)
        })?;
    }
    let mut first = None;
    let mut last = None;
    for (index, coord) in path.iter().copied().enumerate() {
        let incoming = if index == 0 {
            start.outward
        } else {
            direction(path[index - 1], coord)
        };
        let outgoing = if index + 1 == path.len() {
            end.outward.opposite()
        } else {
            direction(coord, path[index + 1])
        };
        let mask = direction_mask(incoming.opposite()) | direction_mask(outgoing);
        let cell = cells.try_push(resources, || {
            let mut cell = route_cell(
                coord.x,
                coord.y,
                super::super::cell::stroke_route_char(edge.stroke, mask, charset.unicode),
            );
            cell.directions = mask;
            cell
        })?;
        first.get_or_insert(MarkerAnchor::new(cell, start.outward.opposite()));
        last = Some(MarkerAnchor::new(cell, end.outward.opposite()));
    }
    let (Some(first), Some(last)) = (first, last) else {
        return Ok(None);
    };
    let mut labels = Vec::new();
    if let Some(label) = label {
        labels
            .try_reserve(1)
            .map_err(|_| super::super::layout_allocation_failed())?;
        labels.push(label);
    }
    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(first, last),
    )))
}

pub(in crate::graph::routing) fn terminal_label_hosts(
    path: &[CanvasCoord],
    descriptor: Option<RoutedLabelDescriptor>,
    resources: &ResourceContext,
) -> Result<Vec<Option<PlannedRouteLabel>>> {
    let mut labels = Vec::new();
    labels
        .try_reserve(1)
        .map_err(|_| super::super::layout_allocation_failed())?;
    let Some(descriptor) = descriptor else {
        labels.push(None);
        return Ok(labels);
    };
    let mut segments = Vec::new();
    let mut start_index = 0;
    for index in 1..path.len() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if index + 1 == path.len()
            || direction(path[index - 1], path[index]) != direction(path[index], path[index + 1])
        {
            let start = path[start_index];
            let end = path[index];
            let span =
                resources.checked_work_add(start.x.abs_diff(end.x), start.y.abs_diff(end.y))?;
            let cross_extent = if descriptor.line_count() == 1 {
                0
            } else if start.y == end.y {
                descriptor.line_count()
            } else {
                descriptor.width()
            };
            let along_extent = if start.y == end.y {
                descriptor.width()
            } else {
                descriptor.line_count()
            };
            let deficit = if descriptor.line_count() == 1 {
                0
            } else {
                along_extent.saturating_sub(span.saturating_add(1))
            };
            segments
                .try_reserve(1)
                .map_err(|_| super::super::layout_allocation_failed())?;
            segments.push((
                (deficit, cross_extent),
                std::cmp::Reverse(span),
                start_index,
                start,
                end,
            ));
            start_index = index;
        }
    }
    crate::graph::layout::charge_sort_work(segments.len(), resources)?;
    segments
        .sort_unstable_by_key(|(cross_extent, span, index, _, _)| (*cross_extent, *span, *index));
    labels
        .try_reserve(segments.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    for (_, _, _, start, end) in segments {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if let Some(label) = planned_label(Some(descriptor), start, end) {
            labels.push(Some(label));
        }
    }
    Ok(labels)
}

fn direction(from: CanvasCoord, to: CanvasCoord) -> StepDirection {
    if from.x < to.x {
        StepDirection::Right
    } else if from.x > to.x {
        StepDirection::Left
    } else if from.y < to.y {
        StepDirection::Down
    } else {
        StepDirection::Up
    }
}

fn direction_mask(direction: StepDirection) -> u8 {
    match direction {
        StepDirection::Up => DIR_UP,
        StepDirection::Down => DIR_DOWN,
        StepDirection::Left => DIR_LEFT,
        StepDirection::Right => DIR_RIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AsciiResourcePolicy, TerminalWidthProfile};

    #[test]
    fn label_hosts_minimize_the_measured_perpendicular_footprint() {
        let path: Vec<_> = (4..=20)
            .map(|column| CanvasCoord { x: column, y: 2 })
            .chain((3..=12).map(|row| CanvasCoord { x: 20, y: row }))
            .chain((4..20).rev().map(|column| CanvasCoord { x: column, y: 12 }))
            .collect();
        let descriptor =
            RoutedLabelDescriptor::for_test(0, "b<br>b<br>b", TerminalWidthProfile::Unicode)
                .unwrap();
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let hosts = terminal_label_hosts(&path, Some(descriptor), &resources).unwrap();
        let first = hosts.first().unwrap().as_ref().unwrap();
        assert!(
            matches!(first.anchor, super::super::LabelAnchor::Segment { start, end, .. } if start.x == end.x)
        );
    }
}
