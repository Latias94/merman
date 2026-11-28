//! Preparation shared by the Grid router and the family geometry projection.

use super::config::{GridPaint, GridSettings};
use super::forest::{GridForest, GridForestGeometry, GridMeasuredItem, layout_measured_forest};
use super::geometry::reserve;
use super::label_spacing::{GridLabelRequirements, GridLabelledEdge};
use super::route_plan::{GridRouteEdge, GridRoutePlan, plan_grid_edges};
use crate::Result;
use crate::graph_layout::{Rect, Size};
use crate::resources::OperationWorkMeter;
use serde_json::Value;
use std::collections::HashMap;

/// The measured label's box and the terminal markers which constrain its placement.
#[derive(Debug, Clone, Copy)]
pub struct GridEdgeLabel {
    pub size: Size,
    pub start_marker: bool,
    pub end_marker: bool,
}

/// An original edge occurrence bound to two measured item slots.
#[derive(Debug, Clone, Copy)]
pub struct GridMeasuredEdge<'a> {
    pub id: &'a str,
    pub source: usize,
    pub target: usize,
    pub label: Option<GridEdgeLabel>,
}

/// Layout geometry for one measured edge, before outline clipping or SVG paint.
#[derive(Debug)]
pub struct GridRoutedEdge {
    pub points: Vec<kurbo::Point>,
    pub label_center: Option<kurbo::Point>,
}

/// One operation's placement, reservations, and planned route order input.
pub struct PreparedGrid<'a> {
    pub(super) forest: GridForest<'a>,
    pub(super) geometry: GridForestGeometry,
    pub(super) plans: Vec<GridRoutePlan<'a>>,
    pub(super) label_requirements: GridLabelRequirements,
    pub(super) edges: &'a [GridMeasuredEdge<'a>],
    paint: GridPaint,
}

impl PreparedGrid<'_> {
    /// Geometry uses the original slots supplied by the family adapter.
    pub fn item_bounds(&self, slot: usize) -> Option<Rect> {
        self.geometry.item(slot).map(|item| item.bounds)
    }

    /// Full-width title exclusion region of a group in drawing coordinates.
    pub fn group_title_bounds(&self, slot: usize) -> Option<Rect> {
        self.geometry.containers.get(slot)?.as_ref()?.title_bounds
    }

    /// The drawing extent before routing and label placement.
    pub fn grid_extent(&self) -> Size {
        self.geometry.root().size
    }

    pub const fn paint(&self) -> GridPaint {
        self.paint
    }
}

pub fn prepare_grid<'a>(
    items: &'a [GridMeasuredItem<'a>],
    edges: &'a [GridMeasuredEdge<'a>],
    config_section: Option<&Value>,
    work: &OperationWorkMeter,
) -> Result<PreparedGrid<'a>> {
    let forest = GridForest::new(items, work)?;
    let settings = GridSettings::read(config_section, work)?;
    let mut occurrences = HashMap::new();
    work.charge(settings.occurrences.len())?;
    occurrences
        .try_reserve(settings.occurrences.len())
        .map_err(|_| crate::Error::LayoutAllocationFailed { backend: "grid" })?;
    occurrences.extend(
        settings
            .occurrences
            .iter()
            .map(|(&id, placements)| (id, placements.as_slice())),
    );

    let mut route_edges = Vec::new();
    let mut labelled_edges = Vec::new();
    work.charge(edges.len())?;
    reserve(&mut route_edges, edges.len())?;
    for (slot, edge) in edges.iter().enumerate() {
        work.charge(1)?;
        route_edges.push(GridRouteEdge {
            id: edge.id,
            source: edge.source,
            target: edge.target,
        });
        if let Some(label) = edge.label {
            labelled_edges
                .try_reserve(1)
                .map_err(|_| crate::Error::LayoutAllocationFailed { backend: "grid" })?;
            labelled_edges.push(GridLabelledEdge {
                edge: slot,
                source: edge.source,
                target: edge.target,
                size: label.size,
            });
        }
    }
    let label_requirements = GridLabelRequirements::new(&forest, &labelled_edges, work)?;
    let geometry = layout_measured_forest(
        &forest,
        &occurrences,
        settings.placement,
        settings.spacing,
        |group, children, tracks| {
            label_requirements.gaps(group, children, tracks, settings.spacing, work)
        },
        work,
    )?;
    let plans = plan_grid_edges(&forest, &geometry, &route_edges, work)?;
    Ok(PreparedGrid {
        forest,
        geometry,
        plans,
        label_requirements,
        edges,
        paint: settings.paint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::mermaid_grid::GridLineHops;
    use crate::graph_layout::mermaid_grid::forest::GridItemKind;
    use crate::graph_layout::mermaid_grid::placement::{
        GridCoordinate, GridItem, GridPlacementSpec,
    };
    use crate::resources::RenderResourcePolicy;
    use serde_json::json;

    fn leaf(id: &str, width: f64, column: i64) -> GridMeasuredItem<'_> {
        GridMeasuredItem {
            item: GridItem {
                id,
                authored_id: None,
                source_ordinal: None,
                inline: GridPlacementSpec {
                    row: GridCoordinate::new(0),
                    column: GridCoordinate::new(column),
                    ..GridPlacementSpec::default()
                },
            },
            parent_id: None,
            kind: GridItemKind::Leaf(Size {
                width,
                height: 40.0,
            }),
        }
    }

    #[test]
    fn keeps_layout_slots_and_curve_after_label_gap_reservation() {
        let items = [leaf("a", 80.0, -1), leaf("b", 70.0, 100)];
        let edges = [GridMeasuredEdge {
            id: "labelled",
            source: 0,
            target: 1,
            label: Some(GridEdgeLabel {
                size: Size {
                    width: 90.0,
                    height: 18.0,
                },
                start_marker: false,
                end_marker: true,
            }),
        }];
        let config = json!({"curve":"linear", "lineHops":"gap", "edgeCornerRadius":0.5});
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let prepared = prepare_grid(&items, &edges, Some(&config), &work).unwrap();
        assert_eq!(prepared.item_bounds(0).unwrap().x, 0.0);
        assert_eq!(prepared.item_bounds(1).unwrap().x, 196.0);
        assert_eq!(
            prepared.grid_extent(),
            Size {
                width: 266.0,
                height: 40.0
            }
        );
        assert_eq!(prepared.group_title_bounds(0), None);
        assert_eq!(prepared.paint().curve, "linear");
        assert_eq!(prepared.paint().corner_radius, 0.5);
        assert_eq!(prepared.paint().line_hops, GridLineHops::Gap);
        assert_eq!(prepared.plans[0].edge.id, edges[0].id);
        assert_eq!(prepared.forest.by_id["b"], edges[0].target);
    }

    #[test]
    fn containment_is_admitted_before_reading_grid_options() {
        let items = [leaf("a", 80.0, 0), leaf("a", 70.0, 1)];
        let invalid = json!({"placements":{"a":{"row":false}}});
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let result = prepare_grid(&items, &[], Some(&invalid), &work);
        assert!(
            matches!(result, Err(crate::Error::InvalidModel { message }) if message.starts_with("GRID_INVALID_CONTAINMENT"))
        );
    }

    #[test]
    fn sibling_session_routes_around_a_blocker_and_keeps_original_bundle_slots() {
        use crate::resources::ResourceLimitId;
        use kurbo::Point;

        let blocked = [
            leaf("s", 40.0, 1),
            leaf("blocker", 40.0, 2),
            leaf("t", 40.0, 3),
        ];
        let edge = |id, source, target| GridMeasuredEdge {
            id,
            source,
            target,
            label: None,
        };
        let blocked_edges = [edge("blocked", 0, 2)];
        let run = |work: &OperationWorkMeter| -> Result<_> {
            let prepared = prepare_grid(&blocked, &blocked_edges, None, work)?;
            prepared.route_sibling_edges(work)
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let work = OperationWorkMeter::new(policy);
        let routed = run(&work).unwrap();
        let points =
            |points: &[(f64, f64)]| points.iter().copied().map(Point::from).collect::<Vec<_>>();
        assert_eq!(
            routed,
            [points(&[
                (40.0, 20.0),
                (84.0, 20.0),
                (84.0, -6.0),
                (136.0, -6.0),
                (136.0, 20.0),
                (180.0, 20.0)
            ])]
        );

        let used = work.used();
        let meter = |limit| {
            OperationWorkMeter::new(
                policy
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            )
        };
        assert_eq!(run(&meter(used)).unwrap(), routed);
        assert!(matches!(
            run(&meter(used - 1)),
            Err(crate::Error::ResourceLimitExceeded(_))
        ));

        let siblings = [leaf("a", 40.0, 1), leaf("a0", 40.0, 2)];
        let edges = [edge("z", 0, 1), edge("other", 1, 0), edge("a", 0, 1)];
        let work = OperationWorkMeter::new(policy);
        let prepared = prepare_grid(&siblings, &edges, None, &work).unwrap();
        let routed = prepared.route_sibling_edges(&work).unwrap();
        assert_eq!(
            routed,
            [
                points(&[(40.0, 28.0), (90.0, 28.0)]),
                points(&[(90.0, 12.0), (40.0, 12.0)]),
                points(&[(40.0, 20.0), (90.0, 20.0)])
            ]
        );
    }
}
