//! Paint Class's axis-tagged routes through Mermaid's shared Layered SVG path.

use std::collections::HashMap;

use merman_render_core::{
    LayoutEdge, LayoutPoint, Result,
    resources::OperationWorkMeter,
    route_types::{Axis, RoutePoint},
    svg_edge_parity::{
        edge_path::rounded_line_with_marker_offsets_points,
        layered_route::{self, Curve},
        line_hops::{LineHopEdge, LineHopStyle},
    },
};
use rustc_hash::FxHashMap;

use super::{ClassSvgRelation, edge::class_arrow_type_for_relation_end};
use crate::class::layered::{allocation_error, buffer, invalid_projection};

#[derive(Clone, Copy)]
pub(super) struct ClassLayeredSvg<'a> {
    pub(super) axes: &'a [Vec<Axis>],
    pub(super) cluster_order: &'a [usize],
    pub(super) curve: Curve,
    pub(super) label_y_offset: f64,
}

impl ClassLayeredSvg<'_> {
    pub(super) fn validate(&self, edges: &[LayoutEdge], cluster_count: usize) -> Result<()> {
        if self.axes.len() != edges.len() || self.cluster_order.len() != cluster_count {
            return Err(invalid_projection(
                "SVG sidecar does not match the Class layout",
            ));
        }
        Ok(())
    }
}

pub(super) fn arrows(
    relation: Option<ClassSvgRelation<'_>>,
) -> (Option<&'static str>, Option<&'static str>) {
    relation.map_or((None, None), |rel| {
        (
            class_arrow_type_for_relation_end(rel.relation.type1),
            class_arrow_type_for_relation_end(rel.relation.type2),
        )
    })
}

pub(super) fn shaft(
    points: &[LayoutPoint],
    axes: &[Axis],
    curve: Curve,
    (start, end): (Option<&str>, Option<&str>),
    scratch: &mut Vec<RoutePoint>,
    marker_points: &mut Vec<LayoutPoint>,
    work: &OperationWorkMeter,
) -> Result<String> {
    if points.len() != axes.len() {
        return Err(invalid_projection(
            "axis tags do not match their Class relation",
        ));
    }
    work.charge_emit_work(points.len())?;
    work.charge_emit_work(points.len())?;
    scratch.clear();
    scratch
        .try_reserve(points.len())
        .map_err(|_| allocation_error())?;
    scratch.extend(
        points
            .iter()
            .zip(axes)
            .map(|(p, &axis)| RoutePoint::new(p.x, p.y, axis)),
    );
    // RoutePath owns the visible path and marker trim. Reuse the existing marker bounds
    // estimate for the Class viewport, where solver contacts have axis-aligned leads.
    *marker_points = rounded_line_with_marker_offsets_points(points, start, end);
    layered_route::path(
        scratch,
        curve,
        layered_route::marker_trim(start),
        layered_route::marker_trim(end),
        work,
    )
}

pub(super) fn line_hop_paths<'a>(
    edges: &'a [LayoutEdge],
    relations: &FxHashMap<&str, ClassSvgRelation<'_>>,
    curve: Curve,
    style: Option<LineHopStyle>,
    work: &OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    let Some(style) = style else {
        return Ok(HashMap::new());
    };
    work.charge_emit_work(edges.len())?;
    let mut inputs = buffer(edges.len())?;
    for edge in edges.iter().filter(|edge| edge.points.len() >= 2) {
        let (start, end) = arrows(relations.get(edge.id.as_str()).copied());
        inputs.push(LineHopEdge {
            id: &edge.id,
            points: &edge.points,
            curve: Some(curve.as_str()),
            arrow_type_start: start,
            arrow_type_end: end,
        });
    }
    let overrides = layered_route::line_hops(&inputs, style, work)?;
    let mut paths = HashMap::new();
    paths
        .try_reserve(overrides.len())
        .map_err(|_| allocation_error())?;
    for (slot, path) in overrides {
        work.charge_emit_work(path.len())?;
        paths.insert(inputs[slot].id, path);
    }
    Ok(paths)
}
