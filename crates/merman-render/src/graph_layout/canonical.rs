use super::Marker;
use super::contour::ShapePortProfile;
use super::ir::{
    Axis, BorderStitch, Endpoint, KnotKind, MeasuredGraph, Rect, RoutePoint, Side, SolvedGraph,
};
use super::route_paint::{self, Clearance, ObstacleOwner, PaintedRoute, RouteStyle};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{LayoutEdge, LayoutLabel, LayoutPoint};
use crate::{Error, Result};
use kurbo::Vec2;
use rustc_hash::FxHashMap;
use std::ops::Range;

#[derive(Debug)]
struct AxisPath {
    occurrence: usize,
    endpoints: (Endpoint, Endpoint),
    contacts: (RoutePoint, RoutePoint),
    sides: (Side, Side),
    markers: (Marker, Marker),
    knots: Vec<(Axis, KnotKind)>,
    stitches: Range<usize>,
}

#[derive(Debug)]
pub(crate) struct CanonicalRoutes {
    pub(crate) placement: crate::layout_frame::GraphPlacement,
    paths: FxHashMap<String, AxisPath>,
    stitches: Vec<BorderStitch>,
    stitch_contacts: Vec<RoutePoint>,
    clearance: Clearance,
}

pub(crate) struct CanonicalPaintedEdge {
    pub(crate) path: PaintedRoute,
    pub(crate) marker_points: [LayoutPoint; 4],
    pub(crate) data_points: Vec<LayoutPoint>,
    pub(crate) markers: (Marker, Marker),
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
            };
            if paths.insert(id.to_owned(), path).is_some() {
                return Err(invalid("Duplicate canonical route identifier"));
            }
        }
        Ok(Self {
            placement: Default::default(),
            paths,
            stitches: solved.border_stitches.clone(),
            stitch_contacts,
            clearance: obstacles(solved, work)?,
        })
    }

    pub(crate) fn paint(
        &self,
        edge: &LayoutEdge,
        translation: Vec2,
        stroke_outset: f64,
        style: RouteStyle,
    ) -> Result<CanonicalPaintedEdge> {
        let missing = || Error::InvalidModel {
            message: format!("Edge `{}` has no canonical paint geometry", edge.id),
        };
        let route = self
            .paths
            .get(&edge.id)
            .filter(|route| route.knots.len() == edge.points.len())
            .ok_or_else(missing)?;
        let translated = |p: RoutePoint| RoutePoint {
            x: p.x + translation.x,
            y: p.y + translation.y,
            ..p
        };
        let mut points: Vec<_> = edge
            .points
            .iter()
            .zip(&route.knots)
            .map(|(p, &(axis, kind))| {
                translated(RoutePoint {
                    x: p.x,
                    y: p.y,
                    axis,
                    kind,
                })
            })
            .collect();
        let [start, .., end] = points.as_mut_slice() else {
            return Err(missing());
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
        let path = route_paint::build_svg_path(
            &points,
            route.sides,
            style,
            route.markers,
            route_paint::CurveTerminals::Fixed,
            stitches,
            Some(self.clearance.for_edge(
                route.occurrence,
                route.endpoints,
                -translation,
                stroke_outset,
            )),
        )
        .map_err(|error| error.for_edge(&edge.id))?;
        Ok(CanonicalPaintedEdge {
            path,
            marker_points: [start_ref, start_lead, end_lead, end_ref],
            data_points: points
                .iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect(),
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

fn obstacles(solved: &SolvedGraph, work: &mut OperationLayoutWorkControl) -> Result<Clearance> {
    let shapes = work.checked_add(solved.nodes.len(), solved.groups.len())?;
    work.charge_adapter(work.checked_add(shapes, solved.edges.len())?)?;
    let mut entries = Vec::new();
    for (index, &node) in solved.nodes.iter().enumerate() {
        entries.push((bounds(node), ObstacleOwner::Node(index as u32)));
    }
    for (index, group) in solved.groups.iter().enumerate() {
        if let Some(title) = &group.title {
            entries.push((label_bounds(title), ObstacleOwner::GroupTitle(index as u32)));
        }
    }
    for (index, edge) in solved.edges.iter().enumerate() {
        for label in [&edge.label, &edge.start_label, &edge.end_label]
            .into_iter()
            .flatten()
        {
            entries.push((label_bounds(label), ObstacleOwner::EdgeLabel(index)));
        }
    }
    let levels = entries
        .len()
        .checked_ilog2()
        .map_or(0, |levels| levels as usize + 1);
    work.charge_adapter(work.checked_mul(entries.len(), levels)?)?;
    Ok(Clearance::new(entries))
}
