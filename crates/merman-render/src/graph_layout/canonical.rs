use super::Marker;
use super::contour::ShapePortProfile;
use super::ir::{Axis, BorderStitch, Endpoint, MeasuredGraph, Rect, RoutePoint, Side, SolvedGraph};
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
    axes: Vec<Axis>,
    stitches: Range<usize>,
}

#[derive(Debug)]
pub(crate) struct CanonicalRoutes {
    paths: FxHashMap<String, AxisPath>,
    stitches: Vec<BorderStitch>,
    clearance: Clearance,
}

pub(crate) struct CanonicalPaintedEdge {
    pub(crate) path: PaintedRoute,
    pub(crate) marker_points: [LayoutPoint; 4],
    pub(crate) data_points: Vec<LayoutPoint>,
    pub(crate) markers: (Marker, Marker),
}

impl CanonicalRoutes {
    pub(crate) fn from_solved<'id>(
        graph: &MeasuredGraph,
        solved: &SolvedGraph,
        profiles: &[ShapePortProfile],
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
        let mut next_stitch = 0;
        for (occurrence, ((id, edge), measured)) in
            edge_ids.zip(&solved.edges).zip(&graph.edges).enumerate()
        {
            work.charge_adapter(work.checked_add(edge.points.len(), id.len())?)?;
            let [first, .., last] = edge.points.as_slice() else {
                return Err(invalid("Missing canonical path terminals"));
            };
            let (from, start_side) = contact(
                solved,
                profiles,
                edge.source,
                edge.ports.0,
                *first,
                edge.points[1],
            );
            let (to, end_side) = contact(
                solved,
                profiles,
                edge.target,
                edge.ports.1,
                *last,
                edge.points[edge.points.len() - 2],
            );
            let start_stitch = next_stitch;
            next_stitch += solved.border_stitches[start_stitch..]
                .iter()
                .take_while(|stitch| stitch.edge == occurrence)
                .count();
            let path = AxisPath {
                occurrence,
                endpoints: (edge.source, edge.target),
                contacts: (from, to),
                sides: (start_side, end_side),
                markers: (measured.start_marker, measured.end_marker),
                axes: edge.points.iter().map(|p| p.axis).collect(),
                stitches: start_stitch..next_stitch,
            };
            if paths.insert(id.to_owned(), path).is_some() {
                return Err(invalid("Duplicate canonical route identifier"));
            }
        }
        Ok(Self {
            paths,
            stitches: solved.border_stitches.clone(),
            clearance: obstacles(solved, work)?,
        })
    }

    pub(crate) fn paint(
        &self,
        edge: &LayoutEdge,
        translation: Vec2,
        stroke_outset: f64,
        style: RouteStyle,
    ) -> Option<CanonicalPaintedEdge> {
        let route = self.paths.get(&edge.id)?;
        if route.axes.len() != edge.points.len() {
            return None;
        }
        let translated = |p: RoutePoint| RoutePoint {
            x: p.x + translation.x,
            y: p.y + translation.y,
            ..p
        };
        let mut points: Vec<_> = edge
            .points
            .iter()
            .zip(&route.axes)
            .map(|(p, &axis)| {
                translated(RoutePoint {
                    x: p.x,
                    y: p.y,
                    axis,
                })
            })
            .collect();
        let [start, .., end] = points.as_mut_slice() else {
            return None;
        };
        (*start, *end) = (translated(route.contacts.0), translated(route.contacts.1));
        let (start_ref, start_lead) =
            route_paint::marker_lead(*start, route.sides.0, route.markers.0);
        let (end_ref, end_lead) = route_paint::marker_lead(*end, route.sides.1, route.markers.1);
        let path = route_paint::build_svg_path(
            &points,
            route.sides,
            style,
            route.markers,
            &self.stitches[route.stitches.clone()],
            Some(self.clearance.for_edge(
                route.occurrence,
                route.endpoints,
                -translation,
                stroke_outset,
            )),
        )?;
        Some(CanonicalPaintedEdge {
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

fn contact(
    solved: &SolvedGraph,
    profiles: &[ShapePortProfile],
    endpoint: Endpoint,
    side: Side,
    point: RoutePoint,
    neighbor: RoutePoint,
) -> (RoutePoint, Side) {
    match endpoint {
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
        Endpoint::Group(_) => (point, route_paint::group_lead_side(side, point, neighbor)),
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
