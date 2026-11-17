//! Coordinate events for the compressed orthogonal search graph.

use super::{ProtectedShape, SceneOccupancy};
use crate::error::Result;
use crate::graph::layout::charge_sort_work;
use crate::graph::routing::cell::{DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP, route_char_directions};
use crate::graph::routing::{PreparedRoute, layout_allocation_failed};
use crate::resource::ResourceContext;
use rustc_hash::FxHashSet as HashSet;

#[derive(Default)]
struct AxisEvents {
    x: HashSet<usize>,
    y: HashSet<usize>,
}

impl AxisEvents {
    fn insert(
        axis: &mut HashSet<usize>,
        coordinate: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        // Include the boundary, its reading margin, and the first free transit lane.
        for value in coordinate.saturating_sub(2)..=resources.checked_grid_add(coordinate, 2)? {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if !axis.contains(&value) {
                axis.try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
                axis.insert(value);
            }
        }
        Ok(())
    }

    fn point(&mut self, x: usize, y: usize, resources: &ResourceContext) -> Result<()> {
        Self::insert(&mut self.x, x, resources)?;
        Self::insert(&mut self.y, y, resources)
    }

    fn finish(self, resources: &ResourceContext) -> Result<(Vec<usize>, Vec<usize>)> {
        fn ordered(events: HashSet<usize>, resources: &ResourceContext) -> Result<Vec<usize>> {
            resources.charge_layout_work(events.len())?;
            charge_sort_work(events.len(), resources)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(events.len())
                .map_err(|_| layout_allocation_failed())?;
            for event in events {
                resources.checkpoint()?;
                values.push(event);
            }
            values.sort_unstable();
            Ok(values)
        }
        Ok((ordered(self.x, resources)?, ordered(self.y, resources)?))
    }
}

impl SceneOccupancy<'_> {
    /// Events change collision or turn opportunities. Empty spans do not allocate
    /// search vertices; the winning channel is expanded only when materialized.
    pub(in crate::graph::routing) fn routing_channel_coordinates(
        &self,
        routes: &[PreparedRoute],
        resources: &ResourceContext,
    ) -> Result<(Vec<usize>, Vec<usize>)> {
        let mut events = AxisEvents::default();
        for protected in self.protected.iter() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            match protected.shape {
                ProtectedShape::Rect(rect) => {
                    events.point(rect.x, rect.y, resources)?;
                    events.point(rect.right - 1, rect.bottom - 1, resources)?;
                }
                ProtectedShape::HorizontalSpan { x_start, x_end, y } => {
                    events.point(x_start, y, resources)?;
                    events.point(x_end, y, resources)?;
                }
                ProtectedShape::VerticalSpan { x, y_start, y_end } => {
                    events.point(x, y_start, resources)?;
                    events.point(x, y_end, resources)?;
                }
            }
        }
        for route in routes {
            for (_, cell) in route.plan.active_cells() {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let directions = if cell.directions == 0 {
                    route_char_directions(cell.ch)
                } else {
                    cell.directions
                };
                if directions != (DIR_LEFT | DIR_RIGHT) && directions != (DIR_UP | DIR_DOWN) {
                    events.point(cell.coord.x, cell.coord.y, resources)?;
                }
            }
        }
        for coord in self.terminal_claims.keys().chain(self.markers.keys()) {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            events.point(coord.x, coord.y, resources)?;
        }
        // Labels store their complete host-gap reservations as cells. Extract only
        // the union's boundary events, retaining no model-sized per-cell vector.
        for coord in self.labels.keys() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(5)?;
            for x in [coord.x.checked_sub(1), coord.x.checked_add(1)] {
                if x.is_none_or(|x| {
                    !self
                        .labels
                        .contains_key(&super::CanvasCoord { x, ..*coord })
                }) {
                    AxisEvents::insert(&mut events.x, coord.x, resources)?;
                    break;
                }
            }
            for y in [coord.y.checked_sub(1), coord.y.checked_add(1)] {
                if y.is_none_or(|y| {
                    !self
                        .labels
                        .contains_key(&super::CanvasCoord { y, ..*coord })
                }) {
                    AxisEvents::insert(&mut events.y, coord.y, resources)?;
                    break;
                }
            }
        }
        events.finish(resources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::layout::{CanvasCoord, layout_graph};
    use crate::graph::model::{AsciiGraph, GraphDirection};
    use crate::operation::AsciiExecution;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy};

    #[test]
    fn long_label_reservations_keep_only_boundary_coordinates_and_exact_work() {
        let graph = AsciiGraph::new(GraphDirection::LeftRight);
        let layout = layout_graph(&graph, &AsciiRenderOptions::ascii());
        let policy = AsciiResourcePolicy::unbounded();
        let mut setup = ResourceContext::new(policy);
        let mut scene = SceneOccupancy::try_new_for_routes(
            &layout,
            0,
            4,
            &mut setup,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        scene.labels.extend(
            (10..1010).map(|x| (CanvasCoord { x, y: 10 }, super::super::LabelCellKind::Text)),
        );
        let measured = ResourceContext::new(policy);
        let axes = scene.routing_channel_coordinates(&[], &measured).unwrap();
        assert_eq!(axes.0, [8, 9, 10, 11, 12, 1007, 1008, 1009, 1010, 1011]);
        assert_eq!(axes.1, [8, 9, 10, 11, 12]);
        let exact = measured.layout_work_used();
        for (limit, passes) in [(exact, true), (exact - 1, false)] {
            let resources = ResourceContext::new(
                policy
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            );
            let result = scene.routing_channel_coordinates(&[], &resources);
            if passes {
                assert_eq!(result.unwrap(), axes);
            } else {
                assert!(matches!(
                    result,
                    Err(crate::AsciiError::ResourceLimitExceeded(_))
                ));
            }
        }
    }
}
