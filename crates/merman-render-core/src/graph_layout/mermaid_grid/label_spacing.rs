//! Label reservations from Mermaid 12.4.0 `labelSpacing.ts` and `labelGeometry.ts`.

use super::forest::GridForest;
use super::geometry::{GridMeasuredTracks, GridSpacing, GridTrackGaps};
use super::placement::GridCoordinate;
use crate::graph_layout::{Axis, Size};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use std::collections::{BTreeMap, HashMap};
use std::ops::Bound::{Excluded, Unbounded};

pub(super) const LABEL_CLEARANCE: f64 = 6.0;
const MARKER_CLEARANCE: f64 = 12.0;
const SPAN_MARGIN: f64 = 2.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct GridLabelSpan {
    pub cross: f64,
    pub required_along: f64,
}

pub(super) fn grid_label_span(size: Size, axis: Axis) -> Option<GridLabelSpan> {
    if !size.width.is_finite()
        || size.width <= 0.0
        || !size.height.is_finite()
        || size.height <= 0.0
    {
        return None;
    }
    let (along, cross) = match axis {
        Axis::Horizontal => (size.width, size.height),
        Axis::Vertical => (size.height, size.width),
    };
    let inset = LABEL_CLEARANCE.max(MARKER_CLEARANCE);
    Some(GridLabelSpan {
        cross,
        required_along: along + inset + inset + SPAN_MARGIN,
    })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GridLabelledEdge {
    pub edge: usize,
    pub source: usize,
    pub target: usize,
    pub size: Size,
}

#[derive(Debug, Default)]
pub(super) struct GridLabelRequirements {
    by_container: HashMap<usize, Vec<GridLabelledEdge>>,
    pub self_loops: HashMap<usize, GridLabelSpan>,
}

impl GridLabelRequirements {
    pub(super) fn new(
        forest: &GridForest<'_>,
        edges: &[GridLabelledEdge],
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        let mut result = Self::default();
        for &edge in edges {
            work.charge(1)?;
            let Some(parent) = direct_parent(forest, edge.source) else {
                continue;
            };
            if edge.source == edge.target {
                if let Some(span) = grid_label_span(edge.size, Axis::Horizontal) {
                    result
                        .self_loops
                        .try_reserve(1)
                        .map_err(|_| allocation_error())?;
                    result.self_loops.insert(edge.edge, span);
                }
            } else if direct_parent(forest, edge.target) == Some(parent) {
                result
                    .by_container
                    .try_reserve(1)
                    .map_err(|_| allocation_error())?;
                let children = result.by_container.entry(parent).or_default();
                children.try_reserve(1).map_err(|_| allocation_error())?;
                children.push(edge);
            }
        }
        Ok(result)
    }

    pub(super) fn gaps(
        &self,
        container: usize,
        child_slots: &[usize],
        tracks: &GridMeasuredTracks<'_>,
        spacing: GridSpacing,
        work: &OperationWorkMeter,
    ) -> Result<GridTrackGaps> {
        let mut gaps = GridTrackGaps::default();
        let Some(edges) = self.by_container.get(&container) else {
            return Ok(gaps);
        };
        let mut cells = HashMap::new();
        work.charge(tracks.placement.placements.len())?;
        cells
            .try_reserve(tracks.placement.placements.len())
            .map_err(|_| allocation_error())?;
        for placement in &tracks.placement.placements {
            work.charge(1)?;
            cells.insert(child_slots[placement.item_index], placement.cell);
        }
        let rows = track_origins(&tracks.rows, spacing.row_gap, work)?;
        let columns = track_origins(&tracks.columns, spacing.column_gap, work)?;
        for edge in edges {
            work.charge(1)?;
            let (Some(&source), Some(&target)) = (cells.get(&edge.source), cells.get(&edge.target))
            else {
                continue;
            };
            if source.row == target.row
                && are_consecutive(source.column, target.column, &tracks.columns)
            {
                add_gap(
                    edge,
                    Axis::Horizontal,
                    source.row,
                    source.column.min(target.column),
                    &rows,
                    &mut gaps.columns,
                    work,
                )?;
            } else if source.column == target.column
                && are_consecutive(source.row, target.row, &tracks.rows)
            {
                add_gap(
                    edge,
                    Axis::Vertical,
                    source.column,
                    source.row.min(target.row),
                    &columns,
                    &mut gaps.rows,
                    work,
                )?;
            }
        }
        Ok(gaps)
    }
}

fn direct_parent(forest: &GridForest<'_>, item: usize) -> Option<usize> {
    forest.parents.get(item).copied().flatten()
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

fn are_consecutive(
    first: GridCoordinate,
    second: GridCoordinate,
    tracks: &BTreeMap<GridCoordinate, f64>,
) -> bool {
    let low = first.min(second);
    let high = first.max(second);
    tracks
        .range((Excluded(low), Unbounded))
        .next()
        .is_some_and(|(&track, _)| track == high)
}

fn track_origins(
    tracks: &BTreeMap<GridCoordinate, f64>,
    gap: f64,
    work: &OperationWorkMeter,
) -> Result<BTreeMap<GridCoordinate, (f64, f64)>> {
    let mut cursor = 0.0;
    let mut origins = BTreeMap::new();
    for (&track, &size) in tracks {
        work.charge(1)?;
        origins.insert(track, (cursor, size));
        cursor += size + gap;
    }
    Ok(origins)
}

fn add_gap(
    edge: &GridLabelledEdge,
    axis: Axis,
    cross_track: GridCoordinate,
    along_track: GridCoordinate,
    cross_tracks: &BTreeMap<GridCoordinate, (f64, f64)>,
    gaps: &mut BTreeMap<GridCoordinate, f64>,
    work: &OperationWorkMeter,
) -> Result<()> {
    let Some(span) = grid_label_span(edge.size, axis) else {
        return Ok(());
    };
    let Some(&(origin, size)) = cross_tracks.get(&cross_track) else {
        return Ok(());
    };
    let center = origin + size * 0.5;
    let low = center - span.cross * 0.5;
    let high = center + span.cross * 0.5;
    for (&other, &(start, length)) in cross_tracks {
        work.charge(1)?;
        if other != cross_track && !(high <= start || low >= start + length) {
            return Ok(());
        }
    }
    let gap = gaps.entry(along_track).or_default();
    *gap = gap.max(span.required_along);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::mermaid_grid::forest::{
        GridItemKind, GridMeasuredItem, layout_measured_forest,
    };
    use crate::graph_layout::mermaid_grid::geometry::GridGroupMeasure;
    use crate::graph_layout::mermaid_grid::placement::{
        GridItem, GridPlacementOptions, GridPlacementSpec,
    };
    use crate::resources::RenderResourcePolicy;

    const SPACING: GridSpacing = GridSpacing {
        row_gap: 10.0,
        column_gap: 50.0,
        cell_gap: 20.0,
        container_padding: 20.0,
        title_gap: 8.0,
    };

    fn leaf(id: &'static str, row: i64, column: i64) -> GridMeasuredItem<'static> {
        GridMeasuredItem {
            item: GridItem {
                id,
                authored_id: None,
                source_ordinal: None,
                inline: GridPlacementSpec {
                    row: GridCoordinate::new(row),
                    column: GridCoordinate::new(column),
                    ..GridPlacementSpec::default()
                },
            },
            parent_id: None,
            kind: GridItemKind::Leaf(Size {
                width: 80.0,
                height: 40.0,
            }),
        }
    }

    fn edge(
        edge: usize,
        source: usize,
        target: usize,
        width: f64,
        height: f64,
    ) -> GridLabelledEdge {
        GridLabelledEdge {
            edge,
            source,
            target,
            size: Size { width, height },
        }
    }

    #[test]
    fn adjacent_tracks_use_the_largest_label_span() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let items = [leaf("a", 1, -10), leaf("b", 1, 77)];
        let forest = GridForest::new(&items, &work).unwrap();
        let requirements = GridLabelRequirements::new(
            &forest,
            &[edge(0, 0, 1, 60.0, 20.0), edge(1, 1, 0, 90.0, 20.0)],
            &work,
        )
        .unwrap();
        let result = layout_measured_forest(
            &forest,
            &HashMap::new(),
            GridPlacementOptions::default(),
            SPACING,
            |container, children, tracks| {
                requirements.gaps(container, children, tracks, SPACING, &work)
            },
            &work,
        )
        .unwrap();
        assert_eq!(result.item(1).unwrap().bounds.x, 80.0 + 116.0);
        assert_eq!(result.root().size.width, 80.0 + 116.0 + 80.0);
    }

    #[test]
    fn a_cross_axis_collision_leaves_the_authored_gap() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let items = [leaf("a", 1, 1), leaf("b", 1, 2), leaf("c", 2, 2)];
        let forest = GridForest::new(&items, &work).unwrap();
        let requirements =
            GridLabelRequirements::new(&forest, &[edge(0, 0, 1, 90.0, 120.0)], &work).unwrap();
        let result = layout_measured_forest(
            &forest,
            &HashMap::new(),
            GridPlacementOptions::default(),
            SPACING,
            |container, children, tracks| {
                requirements.gaps(container, children, tracks, SPACING, &work)
            },
            &work,
        )
        .unwrap();
        assert_eq!(result.item(1).unwrap().bounds.x, 130.0);
    }

    #[test]
    fn labels_and_self_loops_are_indexed_under_their_measured_owners() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut group = leaf("g", 1, 2);
        group.kind = GridItemKind::Group(GridGroupMeasure {
            padding: 0.0,
            title: None,
        });
        let mut inner = leaf("inner", 1, 1);
        inner.parent_id = Some("g");
        let items = [leaf("root", 1, 1), group, inner];
        let forest = GridForest::new(&items, &work).unwrap();
        let requirements = GridLabelRequirements::new(
            &forest,
            &[
                edge(0, 0, 1, 60.0, 20.0),
                edge(1, 0, 2, 90.0, 20.0),
                edge(2, 2, 2, 34.0, 11.0),
            ],
            &work,
        )
        .unwrap();
        assert_eq!(requirements.by_container.len(), 1);
        assert_eq!(requirements.by_container[&items.len()][0].edge, 0);
        assert_eq!(requirements.self_loops[&2].required_along, 60.0);
        assert_eq!(requirements.self_loops[&2].cross, 11.0);
    }
}
