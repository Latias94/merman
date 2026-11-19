//! Integer grid geometry. Work scales with records and child links, never with span values.

use super::validate::{self, Columns, Hierarchy};
use crate::error::Result;
use crate::graph::positioned::{
    PositionedBounds, PositionedElement, PositionedGraph, PositionedSize,
};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;
use merman_model::families::block::BlockDiagramModel;

pub(super) const COLUMN_GAP: usize = 5;
pub(super) const ROW_GAP: usize = 3;

#[derive(Clone, Default)]
pub(super) struct Item {
    pub(super) element: Option<PositionedElement>,
    pub(super) minimum: PositionedSize,
    pub(super) inset: PositionedSize,
    pub(super) span: usize,
    size: PositionedSize,
    local: PositionedBounds,
    row: usize,
    origin: PositionedSize,
}

#[derive(Default)]
struct Row {
    y: usize,
    height: usize,
}

pub(super) fn prepare_items(count: usize) -> Result<Vec<Item>> {
    validate::filled_vec(count, Item::default())
}

pub(super) fn pack(
    model: &BlockDiagramModel,
    hierarchy: &Hierarchy,
    items: &mut [Item],
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for &index in hierarchy.preorder.iter().rev() {
        resources.charge_layout_work(1)?;
        execution.checkpoint(OperationPhase::Layout)?;
        let node = &model.blocks_flat[index];
        if node.block_type != "composite" {
            items[index].size = items[index].minimum;
            continue;
        }
        let columns = validate::columns(node, resources)?;
        let mut unit_width = 1usize;
        // Spans affect width arithmetically. Never allocate a column-sized occupancy matrix.
        for &child in &node.children {
            resources.charge_layout_work(1)?;
            let item = &items[child];
            let gaps = resources.checked_grid_mul(item.span - 1, COLUMN_GAP)?;
            let content = item.size.width.saturating_sub(gaps);
            unit_width = unit_width.max(content.div_ceil(item.span));
        }
        let pitch = resources.checked_grid_add(unit_width, COLUMN_GAP)?;
        let mut rows: Vec<Row> = Vec::new();
        // At most one new row per child. Count was admitted before this allocation.
        resources.charge_layout_work(node.children.len())?;
        validate::reserve(&mut rows, node.children.len())?;
        let mut position = 0usize;
        let mut content_width = 0usize;
        for &child in &node.children {
            resources.charge_layout_work(1)?;
            let (column, row, consumed) = match columns {
                Columns::Auto => (position, 0, items[child].span),
                Columns::Fixed(columns) => {
                    let column = position % columns;
                    (
                        column,
                        position / columns,
                        items[child].span.min(columns - column),
                    )
                }
            };
            if row == rows.len() {
                rows.push(Row::default());
            }
            let item = &mut items[child];
            item.row = row;
            item.local.x = resources.checked_grid_mul(column, pitch)?;
            item.local.width = resources
                .checked_grid_mul(item.span, pitch)?
                .saturating_sub(COLUMN_GAP);
            content_width =
                content_width.max(resources.checked_grid_add(item.local.x, item.local.width)?);
            rows[row].height = rows[row].height.max(item.size.height).max(3);
            position = resources.checked_grid_add(position, consumed)?;
        }
        let mut content_height = 0usize;
        for (row_index, row) in rows.iter_mut().enumerate() {
            resources.charge_layout_work(1)?;
            if row_index > 0 {
                content_height = resources.checked_grid_add(content_height, ROW_GAP)?;
            }
            row.y = content_height;
            content_height = resources.checked_grid_add(content_height, row.height)?;
        }
        for &child in &node.children {
            resources.charge_layout_work(1)?;
            let item = &mut items[child];
            let row = &rows[item.row];
            item.local.y = row.y;
            item.local.height = row.height;
        }
        let item = &mut items[index];
        item.size = PositionedSize {
            width: item.minimum.width.max(resources.checked_grid_add(
                content_width,
                resources.checked_grid_mul(item.inset.width, 2)?,
            )?),
            height: item.minimum.height.max(resources.checked_grid_add(
                content_height,
                if item.element.is_some() {
                    resources.checked_grid_add(item.inset.height, 2)?
                } else {
                    0
                },
            )?),
        };
        // Reject oversized arithmetic geometry before any route or canvas allocation.
        resources.grid_extent(item.size.width, item.size.height)?;
    }
    Ok(())
}

pub(super) fn place(
    model: &BlockDiagramModel,
    hierarchy: &Hierarchy,
    items: &mut [Item],
    graph: &mut PositionedGraph,
    resources: &ResourceContext,
) -> Result<()> {
    for &index in &hierarchy.preorder {
        resources.charge_layout_work(1)?;
        let item = &items[index];
        let origin = item.origin;
        let inset = item.inset;
        if let Some(element) = item.element {
            graph.set_bounds(
                element,
                PositionedBounds {
                    x: origin.width,
                    y: origin.height,
                    width: item.local.width.max(item.size.width),
                    height: item.local.height.max(item.size.height),
                },
                resources,
            )?;
        }
        for &child in &model.blocks_flat[index].children {
            resources.charge_layout_work(1)?;
            let item = &mut items[child];
            item.origin = PositionedSize {
                width: resources.checked_grid_add(
                    resources.checked_grid_add(origin.width, inset.width)?,
                    item.local.x,
                )?,
                height: resources.checked_grid_add(
                    resources.checked_grid_add(origin.height, inset.height)?,
                    item.local.y,
                )?,
            };
        }
    }
    Ok(())
}
