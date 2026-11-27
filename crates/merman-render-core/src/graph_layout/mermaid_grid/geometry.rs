//! Measured geometry of one Mermaid 12.4 Grid container, in local coordinates.
//!
//! Source: rendering-util/layout-algorithms/grid/layoutCore.ts at 9f93a44a36.

#[cfg(test)]
use super::placement::GridCell;
use super::placement::{
    GridCoordinate, GridHorizontalAlign, GridPlacementResult, GridVerticalAlign,
};
use crate::graph_layout::{Rect, Size};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use merman_model::OperationPhase;
use std::collections::BTreeMap;

const GROUP_ROUTING_GUTTER: f64 = 24.0;
const GROUP_ROUTING_CLEARANCE: f64 = 20.0;
const ROOT_CORRIDOR_MIN_INSET: f64 = 20.0;

#[derive(Debug, Clone, Copy)]
pub struct GridGroupMeasure {
    pub padding: f64,
    pub title: Option<Size>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GridSpacing {
    pub row_gap: f64,
    pub column_gap: f64,
    pub cell_gap: f64,
    pub container_padding: f64,
    pub title_gap: f64,
}

#[derive(Default)]
pub(super) struct GridTrackGaps {
    pub rows: BTreeMap<GridCoordinate, f64>,
    pub columns: BTreeMap<GridCoordinate, f64>,
}

#[derive(Debug)]
pub(super) struct GridMeasuredTracks<'a> {
    pub placement: &'a GridPlacementResult,
    measured: &'a [(&'a str, Size)],
    spacing: GridSpacing,
    pub rows: BTreeMap<GridCoordinate, f64>,
    pub columns: BTreeMap<GridCoordinate, f64>,
    // In ascending row, column order. The index binds back to the placement result.
    pub cells: Vec<(usize, Size)>,
}

#[derive(Debug)]
pub(super) struct GridItemGeometry {
    pub item_index: usize,
    #[cfg(test)]
    pub cell: GridCell,
    pub bounds: Rect,
    pub cell_bounds: Rect,
    pub corridors: kurbo::Rect,
}

#[derive(Debug)]
pub(super) struct GridContainerGeometry {
    pub size: Size,
    pub content: Rect,
    pub title_bounds: Option<Rect>,
    pub items: Vec<GridItemGeometry>,
    pub vertical_corridors: Vec<f64>,
    pub horizontal_corridors: Vec<f64>,
}

pub(super) fn reserve<T>(items: &mut Vec<T>, count: usize) -> Result<()> {
    items
        .try_reserve_exact(count)
        .map_err(|_| Error::LayoutAllocationFailed { backend: "grid" })
}

fn charge_sort(count: usize, work: &OperationWorkMeter) -> Result<()> {
    let comparisons = count
        .checked_mul(count.checked_ilog2().unwrap_or(0) as usize + 1)
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.charge(comparisons)?;
    Ok(())
}

pub(super) fn measure_grid_tracks<'a>(
    placement: &'a GridPlacementResult,
    measured: &'a [(&'a str, Size)],
    spacing: GridSpacing,
    work: &OperationWorkMeter,
) -> Result<GridMeasuredTracks<'a>> {
    // Sort occupied cells by coordinates. Track labels do not size a collection.
    charge_sort(placement.cells.len(), work)?;
    let mut cells = Vec::new();
    reserve(&mut cells, placement.cells.len())?;
    cells.extend((0..placement.cells.len()).map(|index| (index, Size::default())));
    cells.sort_unstable_by_key(|&(index, _)| placement.cells[index].cell);

    let mut rows = BTreeMap::new();
    let mut columns = BTreeMap::new();
    for (cell_index, size) in &mut cells {
        work.charge(1)?;
        let cell = &placement.cells[*cell_index];
        let mut count = 0;
        for index in cell.placement_indices() {
            work.charge(1)?;
            let item = placement.placements[index].item_index;
            let Some(&(id, item_size)) = measured.get(item) else {
                return Err(Error::InvalidModel {
                    message: format!("Grid placement {item} has no measured item"),
                });
            };
            if !item_size.width.is_finite()
                || !item_size.height.is_finite()
                || item_size.width <= 0.0
                || item_size.height <= 0.0
            {
                return Err(Error::InvalidModel {
                    message: format!(
                        "GRID_MISSING_MEASUREMENT: Node \"{id}\" must have width and height greater than 0 (width: {}, height: {})",
                        item_size.width, item_size.height,
                    ),
                });
            }
            size.width = size.width.max(item_size.width);
            size.height += item_size.height;
            count += 1;
        }
        if count > 1 {
            size.height += spacing.cell_gap * (count - 1) as f64;
        }
        let row_height: &mut f64 = rows.entry(cell.cell.row).or_default();
        *row_height = row_height.max(size.height);
        let column_width: &mut f64 = columns.entry(cell.cell.column).or_default();
        *column_width = column_width.max(size.width);
    }
    work.checkpoint(OperationPhase::Layout)?;
    Ok(GridMeasuredTracks {
        placement,
        measured,
        spacing,
        rows,
        columns,
        cells,
    })
}

#[derive(Debug, Clone, Copy)]
struct Track {
    start: f64,
    length: f64,
    before: f64,
    after: f64,
}

fn gap_after(
    rows_or_columns: &BTreeMap<GridCoordinate, f64>,
    track: GridCoordinate,
    gaps: &BTreeMap<GridCoordinate, f64>,
    minimum_gap: f64,
) -> f64 {
    if rows_or_columns
        .last_key_value()
        .is_some_and(|(&last, _)| last == track)
    {
        0.0
    } else {
        minimum_gap.max(gaps.get(&track).copied().unwrap_or(0.0))
    }
}

fn layout_tracks(
    sizes: &BTreeMap<GridCoordinate, f64>,
    start: f64,
    gaps: &BTreeMap<GridCoordinate, f64>,
    minimum_gap: f64,
    work: &OperationWorkMeter,
) -> Result<(BTreeMap<GridCoordinate, Track>, f64)> {
    let mut cursor = start;
    let mut tracks: BTreeMap<_, Track> = BTreeMap::new();
    for (&key, &length) in sizes {
        work.charge(1)?;
        let mut before = 0.0;
        if let Some(mut previous) = tracks.last_entry() {
            let previous = previous.get_mut();
            before = (previous.start + previous.length + cursor) / 2.0;
            previous.after = before;
        }
        tracks.insert(
            key,
            Track {
                start: cursor,
                length,
                before,
                after: 0.0,
            },
        );
        cursor += length + gap_after(sizes, key, gaps, minimum_gap);
        if !before.is_finite() || !cursor.is_finite() {
            return Err(non_finite_geometry());
        }
    }
    Ok((tracks, cursor))
}

fn outer_corridors(
    tracks: &mut BTreeMap<GridCoordinate, Track>,
    near: f64,
    far: f64,
    additional: usize,
    work: &OperationWorkMeter,
) -> Result<Vec<f64>> {
    let capacity = tracks
        .len()
        .checked_add(2)
        .and_then(|len| len.checked_add(additional))
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.charge(capacity)?;
    let mut corridors = Vec::new();
    reserve(&mut corridors, capacity)?;
    corridors.extend([near, far]);
    if let Some(mut first) = tracks.first_entry() {
        first.get_mut().before = near;
    }
    if let Some(mut last) = tracks.last_entry() {
        last.get_mut().after = far;
    }
    corridors.extend(tracks.values().skip(1).map(|track| track.before));
    Ok(corridors)
}

pub(super) fn materialize_grid_container(
    tracks: &GridMeasuredTracks<'_>,
    gaps: &GridTrackGaps,
    group: Option<GridGroupMeasure>,
    work: &OperationWorkMeter,
) -> Result<GridContainerGeometry> {
    let GridMeasuredTracks {
        placement,
        measured,
        spacing,
        ..
    } = tracks;
    let group_padding = group.map_or(0.0, |group| group.padding.max(spacing.container_padding));
    let title = group.and_then(|group| group.title).unwrap_or_default();
    let title_band = if title.height > 0.0 {
        title.height + spacing.title_gap
    } else {
        0.0
    };
    let gutter = if group.is_some() {
        GROUP_ROUTING_GUTTER
    } else {
        0.0
    };
    let inset = group_padding + gutter;
    // Keep the reduction's addition order from layoutCore.ts.
    work.charge(tracks.columns.len())?;
    let content_width = tracks.columns.iter().fold(0.0, |total, (&key, &width)| {
        total + width + gap_after(&tracks.columns, key, &gaps.columns, spacing.column_gap)
    });
    let width = if group.is_some() {
        (content_width + inset * 2.0).max(title.width + group_padding * 2.0)
    } else {
        content_width
    };
    let left = (width - content_width) / 2.0;
    let top = group_padding + title_band + gutter;
    let (mut columns, _) = layout_tracks(
        &tracks.columns,
        left,
        &gaps.columns,
        spacing.column_gap,
        work,
    )?;
    let (mut rows, bottom) = layout_tracks(&tracks.rows, top, &gaps.rows, spacing.row_gap, work)?;
    let right = left + content_width;
    let content_height = bottom - top;
    let height = content_height + title_band + inset * 2.0;
    if !width.is_finite() || !height.is_finite() {
        return Err(non_finite_geometry());
    }

    let (left_corridor, right_corridor, top_corridor, bottom_corridor) = if group.is_some() {
        (
            left - GROUP_ROUTING_CLEARANCE,
            right + GROUP_ROUTING_CLEARANCE,
            top - GROUP_ROUTING_CLEARANCE,
            bottom + GROUP_ROUTING_CLEARANCE,
        )
    } else {
        let x_margin = (spacing.column_gap / 2.0).max(ROOT_CORRIDOR_MIN_INSET);
        let y_margin = (spacing.row_gap / 2.0).max(ROOT_CORRIDOR_MIN_INSET);
        (-x_margin, right + x_margin, -y_margin, bottom + y_margin)
    };
    if !right_corridor.is_finite() || !bottom_corridor.is_finite() {
        return Err(non_finite_geometry());
    }

    let mut vertical_corridors =
        outer_corridors(&mut columns, left_corridor, right_corridor, 0, work)?;
    let stack_gaps = placement
        .placements
        .len()
        .saturating_sub(placement.cells.len());
    let additional_corridors = stack_gaps
        .checked_mul(2)
        .ok_or_else(|| work.arithmetic_overflow())?;
    let mut horizontal_corridors = outer_corridors(
        &mut rows,
        top_corridor,
        bottom_corridor,
        additional_corridors,
        work,
    )?;

    work.charge(placement.placements.len())?;
    let mut items = Vec::new();
    reserve(&mut items, placement.placements.len())?;
    for &(cell_index, cell_size) in &tracks.cells {
        work.charge(1)?;
        let stack = &placement.cells[cell_index];
        let column = columns[&stack.cell.column];
        let row = rows[&stack.cell.row];
        let vertical_factor = match stack.vertical_align {
            GridVerticalAlign::Top => 0.0,
            GridVerticalAlign::Center => 0.5,
            GridVerticalAlign::Bottom => 1.0,
        };
        let mut item_top = row.start + (row.length - cell_size.height) * vertical_factor;
        let mut members = stack.placement_indices().enumerate().peekable();
        while let Some((position, member)) = members.next() {
            work.charge(1)?;
            let resolved = placement.placements[member];
            let (_, size) = measured[resolved.item_index];
            let horizontal_factor = match resolved.horizontal_align {
                GridHorizontalAlign::Left => 0.0,
                GridHorizontalAlign::Center => 0.5,
                GridHorizontalAlign::Right => 1.0,
            };
            let item_left = column.start + (column.length - size.width) * horizontal_factor;
            let near = if position == 0 {
                row.before
            } else {
                let near = item_top - spacing.cell_gap / 2.0;
                horizontal_corridors.push(near);
                near
            };
            let far = if members.peek().is_none() {
                row.after
            } else {
                let far = item_top + size.height + spacing.cell_gap / 2.0;
                horizontal_corridors.push(far);
                far
            };
            items.push(GridItemGeometry {
                item_index: resolved.item_index,
                #[cfg(test)]
                cell: stack.cell,
                bounds: Rect {
                    x: item_left,
                    y: item_top,
                    width: size.width,
                    height: size.height,
                },
                cell_bounds: Rect {
                    x: column.start,
                    y: row.start,
                    width: column.length,
                    height: row.length,
                },
                corridors: kurbo::Rect::new(column.before, near, column.after, far),
            });
            item_top += size.height + spacing.cell_gap;
        }
    }
    for corridors in [&mut vertical_corridors, &mut horizontal_corridors] {
        charge_sort(corridors.len(), work)?;
        corridors.sort_unstable_by(f64::total_cmp);
        corridors.dedup_by(|a, b| *a == *b);
    }
    work.checkpoint(OperationPhase::Layout)?;
    Ok(GridContainerGeometry {
        size: Size { width, height },
        content: Rect {
            x: left,
            y: top,
            width: content_width,
            height: content_height,
        },
        title_bounds: (title_band > 0.0).then_some(Rect {
            x: 0.0,
            y: 0.0,
            width,
            height: group_padding + title_band,
        }),
        items,
        vertical_corridors,
        horizontal_corridors,
    })
}

fn non_finite_geometry() -> Error {
    Error::InvalidModel {
        message: "Grid container geometry exceeds the finite coordinate range".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use std::collections::HashMap;

    use super::super::placement::{
        GridItem, GridPlacementOptions, GridPlacementSpec, resolve_grid_placements,
    };

    const SPACING: GridSpacing = GridSpacing {
        row_gap: 34.0,
        column_gap: 24.0,
        cell_gap: 10.0,
        container_padding: 20.0,
        title_gap: 8.0,
    };

    fn size(width: f64, height: f64) -> Size {
        Size { width, height }
    }

    #[test]
    fn measured_geometry_matches_upstream_sparse_stack_and_group() {
        const LIMIT: i64 = 9_007_199_254_740_991;
        let measured = [
            ("a", size(36.0, 18.0)),
            ("b", size(44.0, 12.0)),
            ("c", size(50.0, 75.0)),
            ("d", size(29.0, 11.0)),
        ];
        let items: Vec<_> = [(-LIMIT, -7), (-LIMIT, -7), (-LIMIT, LIMIT), (LIMIT, -7)]
            .into_iter()
            .enumerate()
            .map(|(index, (row, column))| GridItem {
                id: measured[index].0,
                authored_id: None,
                source_ordinal: Some(index),
                inline: GridPlacementSpec {
                    row: GridCoordinate::new(row),
                    column: GridCoordinate::new(column),
                    ..GridPlacementSpec::default()
                },
            })
            .collect();
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let placement = resolve_grid_placements(
            None,
            &items,
            &HashMap::new(),
            GridPlacementOptions::default(),
            &work,
        )
        .unwrap();
        let tracks = measure_grid_tracks(&placement, &measured, SPACING, &work).unwrap();
        let gaps = GridTrackGaps::default();
        let root = materialize_grid_container(&tracks, &gaps, None, &work).unwrap();
        let group = materialize_grid_container(
            &tracks,
            &gaps,
            Some(GridGroupMeasure {
                padding: 12.0,
                title: Some(size(220.0, 20.0)),
            }),
            &work,
        )
        .unwrap();
        assert_eq!(root.size, size(118.0, 120.0));
        assert_eq!(root.vertical_corridors, [-20.0, 56.0, 138.0]);
        assert_eq!(root.horizontal_corridors, [-20.0, 40.5, 92.0, 140.0]);
        assert_eq!(group.size, size(260.0, 236.0));
        assert_eq!(group.vertical_corridors, [51.0, 127.0, 209.0]);
        assert_eq!(group.horizontal_corridors, [52.0, 112.5, 164.0, 212.0]);
        assert_eq!(group.title_bounds.unwrap().height, 48.0);
        assert_eq!((group.content.x, group.content.y), (71.0, 72.0));

        let centers = |geometry: &GridContainerGeometry| {
            geometry
                .items
                .iter()
                .map(|item| {
                    let center = item.bounds.center();
                    (measured[item.item_index].0, center.x, center.y)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            centers(&root),
            [
                ("a", 22.0, 26.5),
                ("b", 22.0, 51.5),
                ("c", 93.0, 37.5),
                ("d", 22.0, 114.5)
            ]
        );
        assert_eq!(
            centers(&group),
            [
                ("a", 93.0, 98.5),
                ("b", 93.0, 123.5),
                ("c", 164.0, 109.5),
                ("d", 93.0, 186.5)
            ]
        );
        assert_eq!(root.items[0].cell.row.get(), -LIMIT);
        assert_eq!(root.items[0].cell_bounds.height, 75.0);
        assert_eq!(root.items[0].corridors.y1, 40.5);
        assert_eq!(root.items[1].corridors.y0, 40.5);
    }
}
