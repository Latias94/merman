//! Remove redundant straight channel space from an already validated scene.
//!
//! Every box/text cell and every bend/terminal coordinate remains distinct. Between those
//! features, a run containing only parallel straight strokes needs at most one row/column.
//! Keeping that cell also preserves gaps between disconnected collinear strokes. The same
//! monotone maps move the whole scene, so this never chooses or reroutes an individual edge.

use super::route::LayeredRelationRoutePlan;
use super::scene::LayeredRelationScene;
use crate::Result;
use crate::relation_graph::layout_allocation_failed;
use crate::resource::ResourceContext;

pub(super) struct CompactAxis {
    coordinates: Vec<usize>,
    extent: usize,
}

impl CompactAxis {
    fn new(length: usize) -> Result<Self> {
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(length)
            .map_err(|_| layout_allocation_failed())?;
        coordinates.resize(length, usize::MAX);
        Ok(Self {
            coordinates,
            extent: length,
        })
    }

    pub(super) fn protect(
        &mut self,
        start: usize,
        end: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.charge_layout_work(end - start)?;
        self.coordinates[start..end].fill(0);
        Ok(())
    }

    fn finish(&mut self, resources: &ResourceContext) -> Result<()> {
        resources.charge_layout_work(self.coordinates.len())?;
        let mut next = 0;
        let mut in_run = false;
        for coordinate in &mut self.coordinates {
            resources.checkpoint()?;
            if *coordinate != usize::MAX {
                *coordinate = next;
                next += 1;
                in_run = false;
            } else {
                if !in_run {
                    next += 1;
                }
                *coordinate = next - 1;
                in_run = true;
            }
        }
        self.extent = next;
        Ok(())
    }

    pub(super) fn map(&self, coordinate: usize) -> usize {
        self.coordinates[coordinate]
    }

    pub(super) fn map_unused_anchor(&self, coordinate: usize) -> usize {
        // Unused private label anchors can lie outside cropped content. Actual painted
        // features are protected before finishing and always lie within this axis.
        self.map(coordinate.min(self.coordinates.len() - 1))
    }

    pub(super) fn extent(&self) -> usize {
        self.extent
    }
}

pub(in crate::relation_graph) fn compact_private_scene(
    scene: &mut LayeredRelationScene<'_>,
    routes: &mut Vec<LayeredRelationRoutePlan>,
    resources: &ResourceContext,
) -> Result<bool> {
    if scene.width() == 0 || scene.height() == 0 {
        return Ok(false);
    }
    let axes = resources.checked_grid_add(scene.width(), scene.height())?;
    let retained = routes.iter().try_fold(routes.capacity(), |total, route| {
        resources.checked_grid_add(total, route.retained_compaction_cells(resources)?)
    })?;
    resources.grid_extent(resources.checked_grid_add(axes, retained)?, 1)?;
    resources.charge_layout_work(axes)?;
    let mut x = CompactAxis::new(scene.width())?;
    let mut y = CompactAxis::new(scene.height())?;
    for node in scene.placed_boxes() {
        resources.checkpoint()?;
        x.protect(node.x(), node.right() + 1, resources)?;
        y.protect(node.y(), node.bottom() + 1, resources)?;
    }
    for route in routes.iter() {
        resources.checkpoint()?;
        route.protect_compaction_features(&mut x, &mut y, resources)?;
    }
    x.finish(resources)?;
    y.finish(resources)?;
    let changed = x.extent() != scene.width() || y.extent() != scene.height();
    if changed {
        resources.charge_layout_work(scene.placed_box_count())?;
        scene.compact_axes(&x, &y);
        for route in routes {
            resources.checkpoint()?;
            route.compact_axes(&x, &y, resources)?;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AsciiError, AsciiResourceLimitId, AsciiResourcePolicy};

    #[test]
    fn compaction_retains_footprints_and_one_cell_between_separate_features() {
        let resources = ResourceContext::new(AsciiResourcePolicy::default());
        let mut axis = CompactAxis::new(16).unwrap();
        for (start, end) in [(0, 3), (7, 8), (12, 16)] {
            axis.protect(start, end, &resources).unwrap();
        }
        axis.finish(&resources).unwrap();
        assert_eq!(axis.extent(), 10);
        assert_eq!((axis.map(0), axis.map(2)), (0, 2));
        assert_eq!((axis.map(7), axis.map(12), axis.map(15)), (4, 6, 9));
        assert_eq!(axis.map(7) - axis.map(2), 2);
        assert_eq!(axis.map(12) - axis.map(7), 2);
    }

    #[test]
    fn compaction_observes_exact_and_n_minus_one_work_admission() {
        for (limit, succeeds) in [(16, true), (15, false)] {
            let policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap();
            let resources = ResourceContext::new(policy);
            let mut axis = CompactAxis::new(12).unwrap();
            axis.protect(2, 6, &resources).unwrap();
            let result = axis.finish(&resources);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
            }
        }
    }
}
