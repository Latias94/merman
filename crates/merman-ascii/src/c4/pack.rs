//! Source-order row packing. Only declared elements, not numeric column counts, allocate storage.

use super::validate::{Hierarchy, filled};
use crate::Result;
use crate::graph::positioned::{
    PositionedBounds, PositionedElement, PositionedGraph, PositionedSize,
};
use crate::resource::ResourceContext;

pub(super) const COLUMN_GAP: usize = 7;
pub(super) const ROW_GAP: usize = 5;

#[derive(Clone, Default)]
pub(super) struct Item {
    pub(super) element: Option<PositionedElement>,
    pub(super) minimum: PositionedSize,
    pub(super) inset: PositionedSize,
    size: PositionedSize,
    local: PositionedBounds,
    origin: PositionedSize,
}

pub(super) fn items(count: usize) -> Result<Vec<Item>> {
    filled(count, Item::default())
}

pub(super) fn pack(
    hierarchy: &Hierarchy,
    shape_count: usize,
    items: &mut [Item],
    resources: &ResourceContext,
) -> Result<()> {
    for &index in hierarchy.preorder.iter().rev() {
        resources.charge_layout_work(1)?;
        let children = &hierarchy.children[index];
        let split = children.partition_point(|&child| child < shape_count);
        let mut width = 0usize;
        let mut height = 0usize;
        for (batch, columns) in [
            (&children[..split], hierarchy.shape_columns),
            (&children[split..], hierarchy.boundary_columns),
        ] {
            for row in batch.chunks(columns) {
                resources.checkpoint()?;
                if height > 0 {
                    height = resources.checked_grid_add(height, ROW_GAP)?;
                }
                let mut x = 0usize;
                let mut row_height = 0usize;
                for (offset, &child) in row.iter().enumerate() {
                    resources.charge_layout_work(1)?;
                    if offset > 0 {
                        x = resources.checked_grid_add(x, COLUMN_GAP)?;
                    }
                    let item = &mut items[child];
                    item.local.x = x;
                    item.local.y = height;
                    x = resources.checked_grid_add(x, item.size.width)?;
                    row_height = row_height.max(item.size.height);
                }
                width = width.max(x);
                height = resources.checked_grid_add(height, row_height)?;
            }
        }
        let item = &mut items[index];
        item.size = PositionedSize {
            width: item.minimum.width.max(
                resources
                    .checked_grid_add(width, resources.checked_grid_mul(item.inset.width, 2)?)?,
            ),
            height: item.minimum.height.max(resources.checked_grid_add(
                height,
                if item.element.is_some() {
                    resources.checked_grid_add(item.inset.height, 2)?
                } else {
                    0
                },
            )?),
        };
        resources.grid_extent(item.size.width, item.size.height)?;
    }
    Ok(())
}

pub(super) fn place(
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
                    width: item.size.width,
                    height: item.size.height,
                },
                resources,
            )?;
        }
        for &child in &hierarchy.children[index] {
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
