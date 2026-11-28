//! Class Grid route paint and `labelRect` nodes emitted after the ordinary Class nodes.

use std::collections::HashMap;

use merman_render_core::{
    Error, LayoutEdge, LayoutNode, LayoutPoint,
    graph_layout::mermaid_grid::{
        GridLineHops, GridPaint,
        routing::paint::{GridPaintedEdge, grid_line_hop_overrides},
    },
    resources::OperationWorkMeter,
    svg_edge_parity::{
        curve::{CurveGeometry, curve_geometry},
        edge_path::{
            line_with_offset_points_into, maybe_fix_corners,
            rounded_line_with_marker_offsets_points,
        },
        line_hops::LineHopEdge,
    },
    text::svg_label::write_svg_text_source_word_lines_with_style,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::class::grid::ClassGridLabelBody;

use super::bounds::include_xywh;
use super::edge::{
    ClassEdgeGroupsRenderContext, class_arrow_type_for_relation_end, class_edge_dom_id_into,
};
use super::*;

const LABEL_STYLE: &str = "display: inline-block";

pub(super) fn shaft(
    source_points: &[LayoutPoint],
    (start, end): (Option<&str>, Option<&str>),
    settings: GridPaint,
    adjusted: &mut Vec<LayoutPoint>,
    marker_points: &mut Vec<LayoutPoint>,
    work: &OperationWorkMeter,
) -> Result<CurveGeometry> {
    work.charge_emit_work(source_points.len())?;
    // Class boxes, namespace rectangles, notes, and interface shells use rectangular
    // intersections. Grid's contact points already lie on those measured rectangles.
    match settings.curve {
        "rounded" => {
            *marker_points = rounded_line_with_marker_offsets_points(source_points, start, end);
        }
        "linear" => line_with_offset_points_into(source_points, start, end, marker_points),
        _ => {
            adjusted.clear();
            adjusted
                .try_reserve(source_points.len())
                .map_err(|_| allocation_error())?;
            adjusted.extend_from_slice(source_points);
            maybe_fix_corners(adjusted);
            line_with_offset_points_into(adjusted, start, end, marker_points);
        }
    }
    Ok(curve_geometry(
        marker_points,
        settings.curve,
        settings.corner_radius,
        false,
    ))
}

pub(super) fn line_hop_paths<'a>(
    edges: &'a [LayoutEdge],
    relations: &FxHashMap<&str, ClassSvgRelation<'_>>,
    settings: GridPaint,
    work: &OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    if settings.line_hops == GridLineHops::Disabled {
        return Ok(HashMap::new());
    }
    work.charge_emit_work(edges.len())?;
    let mut painted = Vec::new();
    painted
        .try_reserve_exact(edges.len())
        .map_err(|_| allocation_error())?;
    for edge in edges.iter().filter(|edge| edge.points.len() >= 2) {
        let relation = relations.get(edge.id.as_str());
        painted.push(GridPaintedEdge {
            geometry: LineHopEdge {
                id: &edge.id,
                points: &edge.points,
                curve: Some(settings.curve),
                arrow_type_start: relation
                    .and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1)),
                arrow_type_end: relation
                    .and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2)),
            },
            corner_radius: Some(settings.corner_radius),
            original_path: "",
        });
    }
    let overrides = grid_line_hop_overrides(settings.line_hops, &painted, work)?;
    let mut paths = HashMap::new();
    paths
        .try_reserve(overrides.len())
        .map_err(|_| allocation_error())?;
    for (slot, path) in overrides {
        work.charge_emit_work(path.len())?;
        paths.insert(painted[slot].geometry.id, path);
    }
    Ok(paths)
}

pub(super) fn write_labels(
    out: &mut impl SvgOutput,
    bounds: &mut Option<Bounds>,
    nodes: &[LayoutNode],
    ctx: &ClassEdgeGroupsRenderContext<'_>,
) -> Result<()> {
    let work = ctx.work_meter;
    work.charge_emit_work(nodes.len())?;
    work.charge_emit_work(ctx.edges.len())?;
    let mut occupied = FxHashSet::default();
    occupied
        .try_reserve(nodes.len().saturating_add(1))
        .map_err(|_| allocation_error())?;
    occupied.insert(ctx.diagram_id.semantic_str());
    for node in nodes {
        work.charge_emit_work(node.id.len())?;
        occupied.insert(node.id.as_str());
    }
    let mut dom_id = String::new();
    for edge in ctx.edges {
        ctx.emit.checkpoint()?;
        let Some(label) = &edge.label else { continue };
        let prepared = ctx
            .prepared_labels
            .grid_by_id
            .get(&edge.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Class Grid relationship label for `{}`",
                    edge.id
                ),
            })?;
        work.charge_emit_work(edge.id.len())?;
        work.charge_emit_work(edge.from.len())?;
        work.charge_emit_work(edge.to.len())?;
        class_edge_dom_id_into(&mut dom_id, edge, ctx.relation_index_by_id);
        let base = format!("edge-label-{}-{}-{dom_id}", edge.from, edge.to);
        let mut id = std::borrow::Cow::Borrowed(base.as_str());
        for suffix in 1..=occupied.len() {
            work.charge_emit_work(id.len().max(1))?;
            if !occupied.contains(id.as_ref()) {
                break;
            }
            id = std::borrow::Cow::Owned(format!("{base}-{suffix}"));
        }
        let x = label.x + ctx.content_tx;
        let y = label.y + ctx.content_ty;
        let left = -label.width / 2.0;
        let top = -label.height / 2.0;
        include_xywh(bounds, x + left, y + top, label.width, label.height);
        let _ = write!(
            out,
            r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/>"#,
            escape_attr_display(&id),
            fmt(x),
            fmt(y),
        );
        if let Some(fill) = ctx
            .edge_label_fill((x, y))
            .filter(|fill| fill.channels()[3] != 0)
        {
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="none"/>"#,
                fmt(left),
                fmt(top),
                fmt(label.width),
                fmt(label.height),
                fill,
            );
        }
        let bbox_top = match &prepared.body {
            ClassGridLabelBody::Svg { bbox_top, .. } => *bbox_top,
            ClassGridLabelBody::Html { .. } => 0.0,
        };
        let _ = write!(
            out,
            r#"<g class="label" style="{LABEL_STYLE}" transform="translate({}, {})"><rect/>"#,
            fmt(left),
            fmt(top - bbox_top),
        );
        out.checkpoint()?;
        match &prepared.body {
            ClassGridLabelBody::Svg { rows, .. } => {
                out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
                write_svg_text_source_word_lines_with_style(out, rows, LABEL_STYLE, false, None);
                out.push_str("</g>");
            }
            ClassGridLabelBody::Html {
                fragment,
                wrapping_width,
            } => {
                let wraps = label.width >= *wrapping_width;
                let (display, whitespace) = if wraps {
                    ("table", "break-spaces")
                } else {
                    ("table-cell", "nowrap")
                };
                let _ = write!(
                    out,
                    r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: {display}; white-space: {whitespace}; line-height: 1.5; max-width: {}px; text-align: center;"#,
                    fmt(label.width),
                    fmt(label.height),
                    fmt(*wrapping_width),
                );
                if wraps {
                    let _ = write!(out, " width: {}px;", fmt(*wrapping_width));
                }
                let _ = write!(out, r#""><span style="{LABEL_STYLE}" class="nodeLabel">"#);
                out.push_str(fragment);
                out.push_str("</span></div></foreignObject>");
            }
        }
        out.push_str("</g></g>");
        out.checkpoint()?;
    }
    Ok(())
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}
