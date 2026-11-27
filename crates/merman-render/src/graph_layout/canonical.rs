use super::contour::ShapePortProfile;
use super::ir::{
    Axis, BorderStitch, Endpoint, KnotKind, MeasuredGraph, Rect, RoutePoint, Side, SolvedGraph,
};
use super::route_paint::{self, ObstacleOwner, PaintedRoute, RouteStyle};
use super::{Marker, edge_paint_error};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{LayoutLabel, LayoutPoint};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use kurbo::Vec2;
use merman_render_core::graph_layout::axis_clearance::AxisClearance;
pub(crate) use merman_render_core::route_paint::CanonicalPaintScratch;
use rustc_hash::FxHashMap;
use std::ops::Range;

#[cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-requirement",
    feature = "diagram-state"
))]
pub(crate) fn theme_routing_style(
    edge_style: Option<merman_theme_contract::v2::EdgeStyle>,
) -> super::ir::RoutingStyle {
    edge_style
        .map_or(RouteStyle::Curved, RouteStyle::from_theme)
        .routing_style()
}

#[derive(Debug)]
struct AxisPath {
    occurrence: usize,
    endpoints: (Endpoint, Endpoint),
    contacts: (RoutePoint, RoutePoint),
    sides: (Side, Side),
    markers: (Marker, Marker),
    knots: Vec<(Axis, KnotKind)>,
    stitches: Range<usize>,
    corridor_clearance: f64,
}

#[derive(Debug)]
pub(crate) struct CanonicalRoutes {
    #[allow(dead_code, reason = "C4 retains placement on its own family artifact.")]
    pub(crate) placement: crate::layout_frame::GraphPlacement,
    paths: FxHashMap<String, AxisPath>,
    stitches: Vec<BorderStitch>,
    stitch_contacts: Vec<RoutePoint>,
    clearance: AxisClearance,
}

#[derive(Clone, Copy)]
pub(crate) enum CaptionClearance {
    #[allow(dead_code, reason = "C4-only builds use caption cutouts.")]
    Avoid,
    /// The family clips the painted shafts behind captions after route painting.
    #[cfg(feature = "diagram-c4")]
    Cutout,
}

pub(crate) struct CanonicalPaintedEdge<Points = Vec<LayoutPoint>> {
    pub(crate) path: PaintedRoute,
    pub(crate) marker_points: [LayoutPoint; 4],
    pub(crate) data_points: Points,
    pub(crate) markers: (Marker, Marker),
}

impl CanonicalPaintedEdge<()> {
    fn with_data_points<Points>(self, data_points: Points) -> CanonicalPaintedEdge<Points> {
        CanonicalPaintedEdge {
            path: self.path,
            marker_points: self.marker_points,
            data_points,
            markers: self.markers,
        }
    }
}

fn layout_points(points: &[RoutePoint]) -> impl ExactSizeIterator<Item = LayoutPoint> + '_ {
    points.iter().map(|p| LayoutPoint { x: p.x, y: p.y })
}

fn missing_paint_geometry(edge_id: &str) -> Error {
    Error::InvalidModel {
        message: format!("Edge `{edge_id}` has no canonical paint geometry"),
    }
}

#[cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-requirement",
    feature = "diagram-state",
    feature = "diagram-usecase"
))]
impl super::RouteContract {
    pub(crate) fn canonical_graph(routes: CanonicalRoutes) -> Self {
        Self::CanonicalGraph(std::sync::Arc::new(routes))
    }

    pub(crate) fn canonical_routes(&self) -> Option<&CanonicalRoutes> {
        match self {
            Self::CanonicalGraph(routes) => Some(routes),
            _ => None,
        }
    }
}

impl CanonicalRoutes {
    pub(crate) fn from_solved<'id>(
        graph: &MeasuredGraph,
        solved: &SolvedGraph,
        profiles: &[ShapePortProfile],
        group_corner_radius: impl Fn(u32) -> f64,
        edge_ids: impl ExactSizeIterator<Item = &'id str>,
        captions: CaptionClearance,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        let invalid = |message: &str| Error::InvalidModel {
            message: message.into(),
        };
        if edge_ids.len() != solved.edges.len()
            || graph.edges.len() != solved.edges.len()
            || profiles.len() != solved.nodes.len()
        {
            return Err(invalid(
                "Canonical geometry does not match the family bindings",
            ));
        }
        work.charge_adapter(work.checked_add(edge_ids.len(), solved.border_stitches.len())?)?;
        let mut paths = FxHashMap::with_capacity_and_hasher(edge_ids.len(), Default::default());
        let mut stitch_contacts = Vec::with_capacity(solved.border_stitches.len());
        let mut obstacles = obstacle_inventory(solved, captions, work)?;
        let group_contact = |id, side, port| {
            let radius = group_corner_radius(id);
            if radius == 0.0 {
                return port;
            }
            let rect = solved.groups[id as usize].rect;
            let center = rect.center();
            ShapePortProfile::rounded_rectangle(rect.width, rect.height, radius, radius).contact(
                &LayoutPoint {
                    x: center.x,
                    y: center.y,
                },
                side,
                port,
            )
        };
        let contact = |endpoint, side, point, neighbor| match endpoint {
            Endpoint::Node(id) => {
                let index = id as usize;
                let center = solved.nodes[index].center();
                (
                    profiles[index].contact(
                        &LayoutPoint {
                            x: center.x,
                            y: center.y,
                        },
                        side,
                        point,
                    ),
                    side,
                )
            }
            Endpoint::Group(id) => (
                group_contact(id, side, point),
                route_paint::group_lead_side(side, point, neighbor),
            ),
        };
        let mut next_stitch = 0;
        for (occurrence, ((id, edge), measured)) in
            edge_ids.zip(&solved.edges).zip(&graph.edges).enumerate()
        {
            work.charge_adapter(work.checked_add(edge.points.len(), id.len())?)?;
            let [first, .., last] = edge.points.as_slice() else {
                return Err(invalid("Missing canonical path terminals"));
            };
            let (from, start_side) = contact(edge.source, edge.ports.0, *first, edge.points[1]);
            let (to, end_side) = contact(
                edge.target,
                edge.ports.1,
                *last,
                edge.points[edge.points.len() - 2],
            );
            let start_stitch = next_stitch;
            for stitch in &solved.border_stitches[start_stitch..] {
                if stitch.edge != occurrence {
                    break;
                }
                let &port = edge
                    .points
                    .get(stitch.point)
                    .ok_or_else(|| invalid("Missing canonical group stitch"))?;
                stitch_contacts.push(group_contact(stitch.group, stitch.side, port));
                next_stitch += 1;
            }
            let path = AxisPath {
                occurrence,
                endpoints: (edge.source, edge.target),
                contacts: (from, to),
                sides: (start_side, end_side),
                markers: (measured.start_marker, measured.end_marker),
                knots: edge.points.iter().map(|p| (p.axis, p.kind)).collect(),
                stitches: start_stitch..next_stitch,
                corridor_clearance: if (from, to) == (*first, *last) {
                    edge.corridor_clearance
                } else {
                    0.0
                },
            };
            if matches!(captions, CaptionClearance::Avoid) {
                work.charge_adapter(2)?;
                obstacles.extend(
                    [
                        (edge.source, from, start_side, measured.start_marker),
                        (edge.target, to, end_side, measured.end_marker),
                    ]
                    .into_iter()
                    .filter_map(|(endpoint, port, side, marker)| {
                        route_paint::marker_obstacle(occurrence, endpoint, port, side, marker)
                    }),
                );
            }
            if paths.insert(id.to_owned(), path).is_some() {
                return Err(invalid("Duplicate canonical route identifier"));
            }
        }
        let levels = obstacles
            .len()
            .checked_ilog2()
            .map_or(0, |levels| levels as usize + 1);
        work.charge_adapter(work.checked_mul(obstacles.len(), levels)?)?;
        Ok(Self {
            placement: Default::default(),
            paths,
            stitches: solved.border_stitches.clone(),
            stitch_contacts,
            clearance: AxisClearance::new(obstacles),
        })
    }

    #[cfg(any(feature = "diagram-requirement", feature = "diagram-usecase"))]
    pub(crate) fn paint(
        &self,
        edge: &crate::model::LayoutEdge,
        translation: Vec2,
        stroke_outset: f64,
        style: RouteStyle,
        work: &OperationWorkMeter,
    ) -> Result<CanonicalPaintedEdge> {
        self.paint_with_scratch(
            edge,
            translation,
            stroke_outset,
            style,
            work,
            &mut CanonicalPaintScratch::default(),
        )
    }

    #[cfg(any(
        feature = "diagram-class",
        feature = "diagram-requirement",
        feature = "diagram-usecase"
    ))]
    pub(crate) fn paint_with_scratch(
        &self,
        edge: &crate::model::LayoutEdge,
        translation: Vec2,
        stroke_outset: f64,
        style: RouteStyle,
        work: &OperationWorkMeter,
        scratch: &mut CanonicalPaintScratch,
    ) -> Result<CanonicalPaintedEdge> {
        self.paint_points_with_scratch(
            &edge.id,
            edge.points.iter().map(|p| (p.x, p.y)),
            translation,
            stroke_outset,
            style,
            work,
            scratch,
        )
    }

    #[cfg(any(
        feature = "diagram-c4",
        feature = "diagram-class",
        feature = "diagram-requirement",
        feature = "diagram-usecase"
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "Route identity and coordinates can come from different layout representations."
    )]
    pub(crate) fn paint_points_with_scratch(
        &self,
        edge_id: &str,
        coordinates: impl ExactSizeIterator<Item = (f64, f64)>,
        translation: Vec2,
        stroke_outset: f64,
        style: RouteStyle,
        work: &OperationWorkMeter,
        scratch: &mut CanonicalPaintScratch,
    ) -> Result<CanonicalPaintedEdge> {
        let route =
            self.fill_paint_points(edge_id, coordinates, translation, &mut scratch.points)?;
        let painted = self.paint_geometry(
            edge_id,
            route,
            translation,
            stroke_outset,
            style,
            work,
            scratch,
        )?;
        Ok(painted.with_data_points(layout_points(&scratch.points).collect()))
    }

    /// Paints an edge whose data points are consumed before the next edge is painted.
    #[cfg(any(
        feature = "diagram-architecture",
        feature = "diagram-er",
        feature = "diagram-state"
    ))]
    pub(crate) fn paint_with_borrowed_points<'points>(
        &self,
        edge: &crate::model::LayoutEdge,
        translation: Vec2,
        stroke_outset: f64,
        style: RouteStyle,
        work: &OperationWorkMeter,
        scratch: &'points mut CanonicalPaintScratch,
    ) -> Result<CanonicalPaintedEdge<&'points [LayoutPoint]>> {
        let route = self.fill_paint_points(
            &edge.id,
            edge.points.iter().map(|p| (p.x, p.y)),
            translation,
            &mut scratch.points,
        )?;
        let painted = self.paint_geometry(
            &edge.id,
            route,
            translation,
            stroke_outset,
            style,
            work,
            scratch,
        )?;
        scratch.data_points.clear();
        scratch.data_points.extend(layout_points(&scratch.points));
        Ok(painted.with_data_points(scratch.data_points.as_slice()))
    }

    // Specialize only point preparation for each source iterator.
    fn fill_paint_points(
        &self,
        edge_id: &str,
        coordinates: impl ExactSizeIterator<Item = (f64, f64)>,
        translation: Vec2,
        points: &mut Vec<RoutePoint>,
    ) -> Result<&AxisPath> {
        let route = self
            .paths
            .get(edge_id)
            .filter(|route| route.knots.len() == coordinates.len())
            .ok_or_else(|| missing_paint_geometry(edge_id))?;
        points.clear();
        points.extend(
            coordinates
                .zip(&route.knots)
                .map(|((x, y), &(axis, kind))| RoutePoint {
                    x: x + translation.x,
                    y: y + translation.y,
                    axis,
                    kind,
                }),
        );
        Ok(route)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "The shared painter takes route identity and prepared metadata separately."
    )]
    fn paint_geometry(
        &self,
        edge_id: &str,
        route: &AxisPath,
        translation: Vec2,
        stroke_outset: f64,
        style: RouteStyle,
        work: &OperationWorkMeter,
        scratch: &mut CanonicalPaintScratch,
    ) -> Result<CanonicalPaintedEdge<()>> {
        let translated = |p: RoutePoint| RoutePoint {
            x: p.x + translation.x,
            y: p.y + translation.y,
            ..p
        };
        let points = &mut scratch.points;
        let [start, .., end] = points.as_mut_slice() else {
            return Err(missing_paint_geometry(edge_id));
        };
        let contacts = (translated(route.contacts.0), translated(route.contacts.1));
        (*start, *end) = contacts;
        let stitches = &self.stitches[route.stitches.clone()];
        for (stitch, &contact) in stitches
            .iter()
            .zip(&self.stitch_contacts[route.stitches.clone()])
        {
            points[stitch.point] = translated(contact);
        }
        let (start_ref, start_lead) =
            route_paint::marker_lead(contacts.0, route.sides.0, route.markers.0);
        let (end_ref, end_lead) =
            route_paint::marker_lead(contacts.1, route.sides.1, route.markers.1);
        let clearance = if style == RouteStyle::Curved
            && route.corridor_clearance > stroke_outset
            && stitches.is_empty()
        {
            None
        } else {
            Some(
                self.clearance
                    .resolve(work)?
                    .for_edge(
                        route.occurrence,
                        route.endpoints,
                        -translation,
                        stroke_outset,
                    )
                    .with_terminal_contacts(route.contacts),
            )
        };
        let path = route_paint::build_svg_path_with_scratch(
            points,
            route.sides,
            style,
            route.markers,
            route_paint::CurveTerminals::Smooth,
            stitches,
            clearance,
            &mut scratch.route,
        )
        .map_err(|error| edge_paint_error(edge_id, error))?;
        Ok(CanonicalPaintedEdge {
            path,
            marker_points: [start_ref, start_lead, end_lead, end_ref],
            data_points: (),
            markers: route.markers,
        })
    }
}

fn bounds(rect: Rect) -> kurbo::Rect {
    kurbo::Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height)
}

fn label_bounds(label: &LayoutLabel) -> kurbo::Rect {
    kurbo::Rect::from_center_size((label.x, label.y), (label.width, label.height))
}

fn obstacle_inventory(
    solved: &SolvedGraph,
    captions: CaptionClearance,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<(kurbo::Rect, ObstacleOwner)>> {
    if solved.edges.is_empty() {
        return Ok(Vec::new());
    }
    let shapes = work.checked_add(solved.nodes.len(), solved.groups.len())?;
    work.charge_adapter(work.checked_add(shapes, solved.edges.len())?)?;
    let mut entries = Vec::new();
    let edge_slots = if matches!(captions, CaptionClearance::Avoid) {
        work.checked_mul(solved.edges.len(), 5)?
    } else {
        0
    };
    entries
        .try_reserve_exact(work.checked_add(shapes, edge_slots)?)
        .map_err(|_| Error::LayoutAllocationFailed { backend: "shoal" })?;
    for (index, &node) in solved.nodes.iter().enumerate() {
        entries.push((bounds(node), ObstacleOwner::Node(index as u32)));
    }
    for (index, group) in solved.groups.iter().enumerate() {
        if let Some(title) = &group.title {
            entries.push((label_bounds(title), ObstacleOwner::GroupTitle(index as u32)));
        }
    }
    if matches!(captions, CaptionClearance::Avoid) {
        for (index, edge) in solved.edges.iter().enumerate() {
            for label in [&edge.label, &edge.start_label, &edge.end_label]
                .into_iter()
                .flatten()
            {
                entries.push((label_bounds(label), ObstacleOwner::EdgeLabel(index)));
            }
        }
    }
    Ok(entries)
}
