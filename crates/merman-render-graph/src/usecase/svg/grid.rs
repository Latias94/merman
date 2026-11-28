//! Grid route paint and the separate labelRect layer in Usecase SVG.

use super::super::{
    UsecaseEdgePlan, UsecasePrepared,
    grid::{UsecaseGridPaint, allocation_error, buffer, invalid_projection},
};
use super::*;
use kurbo::Point;
use merman_model::OperationPhase;
use merman_render_core::{
    LayoutPoint,
    graph_layout::{
        Size,
        mermaid_grid::{
            GridLineHops,
            routing::{
                outline::{GridOutline, GridOutlineQuery, clip_grid_endpoints},
                paint::{GridPaintedEdge, grid_line_hop_overrides},
            },
        },
    },
    svg_bounds::svg_path_bounds_from_d,
    svg_edge_parity::curve,
};
use std::collections::HashSet;

pub(super) struct GridEdgePaint {
    pub(super) points: Vec<LayoutPoint>,
    pub(super) path: String,
    initial_length: Option<f64>,
    hopped: bool,
}

pub(super) fn paint_edges(
    prepared: &UsecasePrepared,
    grid: &UsecaseGridPaint,
    nodes: &HashMap<&str, &LayoutNode>,
    plans: &HashMap<&str, &UsecaseNodePlan>,
    bounds: &mut Bounds,
    work: &OperationWorkMeter,
) -> Result<Vec<GridEdgePaint>> {
    let settings = grid.settings;
    let css = &prepared.measured.css;
    let rough = css.sketches_edges() || css.edge_look == "handDrawn";
    let edges = &prepared.layout().edges;
    work.charge_emit_work(edges.len())?;
    let mut painted = buffer(edges.len())?;
    let mut original = Vec::new();
    for (edge, plan) in edges.iter().zip(&prepared.measured.edges) {
        work.charge_emit_work(edge.points.len())?;
        original.clear();
        original
            .try_reserve(edge.points.len())
            .map_err(|_| allocation_error())?;
        original.extend(edge.points.iter().map(|point| Point::new(point.x, point.y)));
        let source = nodes[edge.from.as_str()];
        let target = nodes[edge.to.as_str()];
        let clip_source = |query| Ok(ellipse_contact(source, query));
        let clip_target = |query| Ok(ellipse_contact(target, query));
        let clipped = clip_grid_endpoints(
            &original,
            plans[edge.from.as_str()]
                .ellipse
                .then(|| outline(source, &clip_source)),
            plans[edge.to.as_str()]
                .ellipse
                .then(|| outline(target, &clip_target)),
            work,
        )?;
        work.charge_emit_work(clipped.len())?;
        let mut points = buffer(clipped.len())?;
        points.extend(clipped.iter().map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        }));
        work.charge_emit_work(points.len())?;
        let start = plan.start_marker.as_deref();
        let end = plan.end_marker.as_deref();
        let shaft = match settings.curve {
            "rounded" => edge_path::rounded_line_with_marker_offsets_points(&points, start, end),
            "linear" => edge_path::line_with_offset_points(&points, start, end),
            _ => {
                let mut adjusted = buffer(points.len())?;
                adjusted.extend_from_slice(&points);
                edge_path::maybe_fix_corners(&mut adjusted);
                edge_path::line_with_offset_points(&adjusted, start, end)
            }
        };
        let geometry = curve::curve_geometry(
            &shaft,
            settings.curve,
            settings.corner_radius,
            css.edge_look == "neo" && !plan.animated && !rough,
        );
        work.charge_emit_work(geometry.d.len())?;
        if let Some(path) = geometry.bounds {
            bounds.min_x = bounds.min_x.min(path.min_x);
            bounds.min_y = bounds.min_y.min(path.min_y);
            bounds.max_x = bounds.max_x.max(path.max_x);
            bounds.max_y = bounds.max_y.max(path.max_y);
        }
        painted.push(GridEdgePaint {
            points,
            path: geometry.d,
            initial_length: geometry.centerline_length,
            hopped: false,
        });
    }

    if settings.line_hops != GridLineHops::Disabled && !rough {
        work.charge_emit_work(edges.len())?;
        let mut rendered = buffer(edges.len())?;
        for (edge, paint) in edges.iter().zip(&painted) {
            rendered.push(GridPaintedEdge {
                geometry: line_hops::LineHopEdge {
                    id: &edge.id,
                    points: &paint.points,
                    curve: Some(settings.curve),
                    arrow_type_start: edge.start_marker.as_deref(),
                    arrow_type_end: edge.end_marker.as_deref(),
                },
                corner_radius: Some(settings.corner_radius),
                original_path: &paint.path,
            });
        }
        let replacements = grid_line_hop_overrides(settings.line_hops, &rendered, work)?;
        for (index, path) in replacements {
            work.charge_emit_work(path.len())?;
            if let Some(path) = svg_path_bounds_from_d(&path) {
                bounds.min_x = bounds.min_x.min(path.min_x);
                bounds.min_y = bounds.min_y.min(path.min_y);
                bounds.max_x = bounds.max_x.max(path.max_x);
                bounds.max_y = bounds.max_y.max(path.max_y);
            }
            painted[index].path = path;
            painted[index].hopped = true;
        }
    }
    Ok(painted)
}

pub(super) fn path_style(
    plan: &UsecaseEdgePlan,
    paint: &GridEdgePaint,
    work: &OperationWorkMeter,
) -> Result<String> {
    work.charge_emit_work(plan.source_style.len())?;
    let mut style = String::new();
    if let Some(length) = paint.initial_length {
        // The Neo dotted mask can expand linearly with physical path length.
        let units = if plan.dotted {
            (length / 4.0).max(0.0) as usize
        } else {
            1
        };
        work.charge_emit_work(
            units
                .checked_mul(4)
                .ok_or_else(|| work.arithmetic_overflow())?,
        )?;
        edge_path::write_neo_edge_mask(
            &mut style,
            length,
            plan.start_marker.as_deref(),
            plan.end_marker.as_deref(),
            plan.dotted,
            false,
        );
    }
    style.push_str(&plan.source_style);
    style.push(';');
    if paint.hopped {
        return Ok(line_hops::rewrite_style_after_line_hop_with_phase(
            &style,
            &paint.path,
            OperationPhase::Emit,
            work,
        )?
        .into_owned());
    }
    Ok(style)
}

fn outline<'a>(
    node: &LayoutNode,
    intersect: &'a dyn Fn(GridOutlineQuery) -> Result<Option<Point>>,
) -> GridOutline<'a> {
    GridOutline {
        center: Some(Point::new(node.x, node.y)),
        size: Some(Size {
            width: node.width,
            height: node.height,
        }),
        intersect,
    }
}

fn ellipse_contact(node: &LayoutNode, query: GridOutlineQuery) -> Option<Point> {
    let origin = query.line_origin?;
    let offset = origin - Point::new(node.x, node.y);
    let delta = query.adjacent - origin;
    let rx2 = (node.width / 2.0).powi(2);
    let ry2 = (node.height / 2.0).powi(2);
    let a = delta.x * delta.x / rx2 + delta.y * delta.y / ry2;
    let b = (2.0 * offset.x * delta.x) / rx2 + (2.0 * offset.y * delta.y) / ry2;
    let c = offset.x * offset.x / rx2 + offset.y * offset.y / ry2 - 1.0;
    let discriminant = b * b - 4.0 * a * c;
    if a <= 0.0 || discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    // Mermaid takes the intersection closest to the exterior end of the segment.
    [(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)]
        .into_iter()
        .find(|value| (0.0..=1.0).contains(value))
        .map(|value| origin + delta * value)
}

pub(super) fn write_labels(
    out: &mut impl SvgOutput,
    prepared: &UsecasePrepared,
    grid: &UsecaseGridPaint,
    diagram_id: SvgId<'_>,
    overflow: bool,
    work: &OperationWorkMeter,
) -> Result<()> {
    if grid.labels.is_empty() {
        return Ok(());
    }
    let layout = prepared.layout();
    let count = layout
        .nodes
        .len()
        .checked_add(grid.labels.len())
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.charge_emit_work(count)?;
    let mut used = HashSet::new();
    used.try_reserve(count).map_err(|_| allocation_error())?;
    used.insert(Cow::Borrowed(diagram_id.semantic_str()));
    for node in &layout.nodes {
        work.charge_emit_work(node.id.len())?;
        used.insert(Cow::Borrowed(&node.id));
    }
    for label in &grid.labels {
        let edge = &layout.edges[label.edge_slot];
        let placement = edge
            .label
            .as_ref()
            .ok_or_else(|| invalid_projection("missing label box"))?;
        work.charge_emit_work(edge.from.len())?;
        work.charge_emit_work(edge.to.len())?;
        work.charge_emit_work(edge.id.len())?;
        let stem = format!("edge-label-{}-{}-{}", edge.from, edge.to, edge.id);
        let mut id = Cow::Borrowed(stem.as_str());
        for suffix in 1..=used.len() {
            work.charge_emit_work(id.len())?;
            if !used.contains(id.as_ref()) {
                break;
            }
            id = Cow::Owned(format!("{stem}-{suffix}"));
        }
        label.write(out, &id, placement, overflow)?;
        used.insert(Cow::Owned(id.into_owned()));
    }
    Ok(())
}
