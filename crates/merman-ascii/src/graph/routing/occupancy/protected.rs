use super::{OccupiedRect, ProtectedGeometry, ProtectedKind, ProtectedShape};
use crate::Result;
use crate::graph::layout::{CanvasCoord, GraphLayout, GroupLayout};
use crate::graph::model::GraphGroupKind;
use crate::graph::routing::layout_allocation_failed;
use crate::resource::ResourceContext;

#[cfg(test)]
mod tests;

/// Immutable protection is shared by congestion trials; committed routes and labels stay local.
#[derive(Debug)]
pub(in crate::graph::routing) struct ProtectedScene<'layout> {
    geometry: Vec<ProtectedGeometry<'layout>>,
    index: Vec<SpatialNode>,
    pub(super) extent: CanvasCoord,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

impl Bounds {
    fn point(coord: CanvasCoord) -> Self {
        Self {
            left: coord.x,
            top: coord.y,
            right: coord.x,
            bottom: coord.y,
        }
    }

    fn rectangle(rect: OccupiedRect) -> Self {
        Self {
            left: rect.x,
            top: rect.y,
            right: rect.right - 1,
            bottom: rect.bottom - 1,
        }
    }

    fn protected(geometry: ProtectedGeometry<'_>) -> Self {
        match geometry.shape {
            ProtectedShape::Rect(rect) => {
                let mut bounds = Self::rectangle(rect);
                if geometry.kind == ProtectedKind::Node {
                    // The cell guard also protects one reading cell around an unrelated node.
                    bounds.left = rect.x.saturating_sub(1);
                    bounds.top = rect.y.saturating_sub(1);
                    bounds.right = rect.right;
                    bounds.bottom = rect.bottom;
                }
                bounds
            }
            ProtectedShape::HorizontalSpan { x_start, x_end, y } => Self {
                left: x_start,
                top: y,
                right: x_end,
                bottom: y,
            },
            ProtectedShape::VerticalSpan { x, y_start, y_end } => Self {
                left: x,
                top: y_start,
                right: x,
                bottom: y_end,
            },
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    fn center(self) -> CanvasCoord {
        CanvasCoord {
            x: self.left + (self.right - self.left) / 2,
            y: self.top + (self.bottom - self.top) / 2,
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.left <= other.right
            && other.left <= self.right
            && self.top <= other.bottom
            && other.top <= self.bottom
    }
}

#[derive(Debug)]
struct SpatialNode {
    bounds: Bounds,
    /// The first node after this complete preorder subtree, including its leaves.
    end: usize,
    geometry: Option<usize>,
}

impl<'layout> ProtectedScene<'layout> {
    pub(super) fn try_new(
        layout: &'layout GraphLayout,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut capacity = layout.nodes.len();
        for group in &layout.groups {
            resources.charge_layout_work(1)?;
            let borders = match group.kind {
                GraphGroupKind::Divider => usize::from(group.divider_span.is_some()),
                GraphGroupKind::Container => 4,
            };
            capacity = resources.checked_work_add(capacity, borders)?;
            capacity = resources.checked_work_add(capacity, group.title.lines().len())?;
        }
        resources.charge_layout_work(capacity)?;
        let mut scene = Self {
            geometry: Vec::new(),
            index: Vec::new(),
            extent: CanvasCoord { x: 0, y: 0 },
        };
        scene
            .geometry
            .try_reserve_exact(capacity)
            .map_err(|_| layout_allocation_failed())?;
        for node in &layout.nodes {
            resources.charge_layout_work(1)?;
            scene.extent.x = scene.extent.x.max(node.right());
            scene.extent.y = scene.extent.y.max(node.bottom());
            scene.geometry.push(ProtectedGeometry {
                owner_id: &node.id,
                group_index: None,
                kind: ProtectedKind::Node,
                shape: ProtectedShape::Rect(OccupiedRect::try_new(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    resources,
                )?),
            });
        }
        for (index, group) in layout.groups.iter().enumerate() {
            resources.charge_layout_work(1)?;
            scene.extent.x = scene.extent.x.max(group.right());
            scene.extent.y = scene.extent.y.max(group.bottom());
            scene.register_group(index, group, resources)?;
        }
        scene.build_index(resources)?;
        Ok(scene)
    }

    pub(in crate::graph::routing) fn iter(
        &self,
    ) -> std::slice::Iter<'_, ProtectedGeometry<'layout>> {
        self.geometry.iter()
    }

    pub(super) fn len(&self) -> usize {
        self.geometry.len()
    }

    pub(super) fn all_at(
        &self,
        coord: CanvasCoord,
        resources: &ResourceContext,
        visit: impl FnMut(&ProtectedGeometry<'layout>) -> bool,
    ) -> Result<bool> {
        self.all_intersecting(Bounds::point(coord), resources, visit)
    }

    pub(super) fn all_in(
        &self,
        rect: OccupiedRect,
        resources: &ResourceContext,
        visit: impl FnMut(&ProtectedGeometry<'layout>) -> bool,
    ) -> Result<bool> {
        self.all_intersecting(Bounds::rectangle(rect), resources, visit)
    }

    fn all_intersecting(
        &self,
        query: Bounds,
        resources: &ResourceContext,
        mut visit: impl FnMut(&ProtectedGeometry<'layout>) -> bool,
    ) -> Result<bool> {
        resources.checkpoint()?;
        let mut cursor = 0;
        while let Some(node) = self.index.get(cursor) {
            resources.charge_layout_work(1)?;
            if !node.bounds.intersects(query) {
                cursor = node.end;
                continue;
            }
            if let Some(index) = node.geometry
                && !visit(&self.geometry[index])
            {
                return Ok(false);
            }
            cursor += 1;
        }
        Ok(true)
    }

    fn build_index(&mut self, resources: &ResourceContext) -> Result<()> {
        let count = self.geometry.len();
        if count == 0 {
            return Ok(());
        }
        let nodes = resources.checked_work_mul(count, 2)? - 1;
        resources.charge_layout_work(resources.checked_work_add(count, nodes)?)?;
        self.index
            .try_reserve_exact(nodes)
            .map_err(|_| layout_allocation_failed())?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| layout_allocation_failed())?;
        for (index, geometry) in self.geometry.iter().copied().enumerate() {
            resources.charge_layout_work(1)?;
            entries.push((Bounds::protected(geometry), index));
        }
        Self::build_subtree(&mut self.index, &mut entries, resources)
    }

    // Median partitioning keeps recursion logarithmic and retains only O(P) tree nodes.
    fn build_subtree(
        nodes: &mut Vec<SpatialNode>,
        entries: &mut [(Bounds, usize)],
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.checkpoint()?;
        let mut bounds = entries[0].0;
        let mut centers = Bounds::point(bounds.center());
        for (entry, _) in entries.iter() {
            resources.charge_layout_work(1)?;
            bounds = bounds.union(*entry);
            centers = centers.union(Bounds::point(entry.center()));
        }
        let index = nodes.len();
        nodes.push(SpatialNode {
            bounds,
            end: index + 1,
            geometry: (entries.len() == 1).then_some(entries[0].1),
        });
        if entries.len() > 1 {
            let horizontal = centers.right - centers.left >= centers.bottom - centers.top;
            let middle = entries.len() / 2;
            resources.charge_layout_work(entries.len())?;
            entries.select_nth_unstable_by_key(middle, |(bounds, ordinal)| {
                let center = bounds.center();
                (if horizontal { center.x } else { center.y }, *ordinal)
            });
            let (left, right) = entries.split_at_mut(middle);
            Self::build_subtree(nodes, left, resources)?;
            Self::build_subtree(nodes, right, resources)?;
            nodes[index].end = nodes.len();
        }
        Ok(())
    }

    fn register_group(
        &mut self,
        group_index: usize,
        group: &'layout GroupLayout,
        resources: &ResourceContext,
    ) -> Result<()> {
        let mut push_border = |shape| {
            self.geometry.push(ProtectedGeometry {
                owner_id: &group.id,
                group_index: Some(group_index),
                kind: ProtectedKind::GroupBorder,
                shape,
            });
        };
        match group.kind {
            GraphGroupKind::Divider => {
                if let Some(span) = group.divider_span {
                    resources.charge_layout_work(1)?;
                    push_border(ProtectedShape::HorizontalSpan {
                        x_start: span.x_start,
                        x_end: span.x_end,
                        y: group.y,
                    });
                }
            }
            GraphGroupKind::Container => {
                resources.charge_layout_work(4)?;
                for y in [group.y, group.bottom()] {
                    push_border(ProtectedShape::HorizontalSpan {
                        x_start: group.x,
                        x_end: group.right(),
                        y,
                    });
                }
                for x in [group.x, group.right()] {
                    push_border(ProtectedShape::VerticalSpan {
                        x,
                        y_start: group.y,
                        y_end: group.bottom(),
                    });
                }
            }
        }
        let inner_left = resources.checked_grid_add(group.x, 1)?;
        for (line_index, line) in group.title.lines().iter().enumerate() {
            resources.charge_layout_work(1)?;
            let title_width = group.title.line_width(line);
            if title_width == 0 {
                continue;
            }
            let Some(position) = group.title_line_position(line_index) else {
                continue;
            };
            let protected_left = position.x.saturating_sub(1).max(inner_left);
            let protected_right = resources
                .checked_grid_add(resources.checked_grid_add(position.x, title_width)?, 1)?
                .min(group.right());
            self.geometry.push(ProtectedGeometry {
                owner_id: &group.id,
                group_index: Some(group_index),
                kind: ProtectedKind::GroupTitle,
                shape: ProtectedShape::Rect(OccupiedRect::try_new(
                    protected_left,
                    position.y,
                    protected_right - protected_left,
                    1,
                    resources,
                )?),
            });
        }
        Ok(())
    }
}
