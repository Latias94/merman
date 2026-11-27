//! Contrast for Shoal connectors on the surfaces actually painted by Swimlane.

use super::super::edge_fill::{
    EDGE_CONTRAST, FlowchartEdgeFillPlan, contains_rect, contrasting_casing, rect_surface_paint,
    segment_enters_surface, spatial_bounds,
};
use super::*;
use crate::flowchart::FlowchartEdgeKey;
use crate::theme::{Color, edge_label_bg::literal_fill};
use kurbo::{BezPath, ParamCurve, Rect, RoundedRect, Shape, Vec2};
use merman_layout_shoal::geom::{Bvh, BvhEntry};
use merman_model::families::flowchart::FlowEdgeVisibility;
use std::ops::ControlFlow::Continue;

const BLACK: Color = Color::rgb(0, 0, 0);
const WHITE: Color = Color::rgb(255, 255, 255);

struct Surface {
    frame: RoundedRect,
    fill: Option<Color>,
}

pub(in crate::svg::parity::flowchart) fn prepare(
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    canvas: Option<Color>,
    default_ink: Color,
) -> Result<FlowchartEdgeFillPlan> {
    let mut plan = FlowchartEdgeFillPlan::default();
    let Some(direction) = ctx.swimlane_direction else {
        return Ok(plan);
    };
    let surfaces = collect_surfaces(ctx, hierarchy, direction, canvas)?;
    if !surfaces
        .iter()
        .filter_map(|surface| surface.fill)
        .chain(canvas)
        .any(|fill| default_ink.contrast(fill) < EDGE_CONTRAST)
    {
        return Ok(plan);
    }
    let levels = surfaces.len().checked_ilog2().unwrap_or(0) as usize + 1;
    ctx.work_meter.charge(
        surfaces
            .len()
            .checked_mul(levels)
            .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?,
    )?;
    let index =
        Bvh::build(surfaces, |surface| spatial_bounds(surface.frame.rect())).map_err(|error| {
            Error::InvalidModel {
                message: format!("could not index Shoal Swimlane paint: {error}"),
            }
        })?;
    let mut backgrounds = Vec::new();
    for &edge in hierarchy.ordered_edges() {
        ctx.checkpoint_emit()?;
        ctx.work_meter.charge(1)?;
        let style = ctx.edge_style_plan.edge_for(edge.key)?;
        if edge.visibility == FlowEdgeVisibility::Invisible
            || !style.has_simple_edge_paint()
            || style.edge_marker_color(false).is_some()
            || ctx.edge_style_plan.animation_for(edge.key)?.is_active()
        {
            continue;
        }
        let Some(cached) = cache.get(&edge.key) else {
            continue;
        };
        let Some(half_width) = ctx
            .edge_style_plan
            .stroke_outset_for(edge.key)?
            .filter(|width| *width > 0.0)
        else {
            continue;
        };
        let d = cached
            .geom
            .emitted_d_for_label
            .as_deref()
            .unwrap_or(&cached.geom.d);
        ctx.work_meter.charge(d.len().div_ceil(16))?;
        let shaft = BezPath::from_svg(d).map_err(|error| Error::InvalidModel {
            message: format!("could not read painted Swimlane edge path: {error}"),
        })?;
        let (Some(first), Some(last)) = (shaft.segments().next(), shaft.segments().last()) else {
            continue;
        };
        let offset = Vec2::new(cached.origin_x - ctx.tx, cached.origin_y - ctx.ty);
        let shaft_bounds = shaft.bounding_box();
        let (start, end) = flowchart_marker_metrics(edge.edge);
        let markers = [(start, first.start()), (end, last.end())].map(|(marker, point)| {
            let reach = marker.depth + marker.trim;
            (reach > 0.0)
                .then(|| Rect::from_points(point + offset, point + offset).inflate(reach, reach))
        });
        let visible_bounds = markers.into_iter().flatten().fold(
            shaft_bounds.inflate(half_width, half_width) + offset,
            |bounds, marker| bounds.union(marker),
        );
        backgrounds.clear();
        let mut unknown = false;
        let mut covered = false;
        let _ = index.visit(|entry| -> Result<_> {
            ctx.work_meter.charge(1)?;
            match entry {
                BvhEntry::Node(bounds) => {
                    let [x0, x1] = bounds.x;
                    let [y0, y1] = bounds.y;
                    Ok(Continue(visible_bounds.overlaps(Rect::new(x0, y0, x1, y1))))
                }
                BvhEntry::Item(surface) => {
                    let frame = surface.frame.rect();
                    if visible_bounds.overlaps(frame)
                        && (markers
                            .iter()
                            .flatten()
                            .any(|marker| marker.overlaps(frame))
                            || shaft_enters_surface(&shaft, surface, offset, half_width, ctx)?)
                    {
                        covered |= contains_rect(surface.frame, visible_bounds);
                        match surface.fill {
                            Some(fill) => backgrounds.push(fill.contrast(BLACK)),
                            None => unknown = true,
                        }
                    }
                    Ok(Continue(true))
                }
            }
        })?;
        if !covered {
            match canvas {
                Some(fill) => backgrounds.push(fill.contrast(BLACK)),
                None => unknown = true,
            }
        }
        let default_luminance = default_ink.contrast(BLACK);
        if backgrounds
            .iter()
            .all(|&bg| contrast(default_luminance, bg) >= EDGE_CONTRAST)
        {
            continue;
        }
        if !unknown
            && let Some(ink) = contrasting_ink(&mut backgrounds, default_ink, ctx.work_meter)?
        {
            plan.set_ink(edge.key, ink.to_string());
        } else if !flowchart_neo_mask_uses_dash_pairs(edge.edge) {
            plan.add_casing(
                edge,
                cached,
                shaft_bounds,
                half_width,
                contrasting_casing(default_ink),
            );
        }
    }
    Ok(plan)
}

fn collect_surfaces(
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    direction: crate::model::SwimlaneDirection,
    canvas: Option<Color>,
) -> Result<Vec<Surface>> {
    let clusters = hierarchy.root(None)?.clusters();
    ctx.work_meter.charge(clusters.len())?;
    let mut surfaces = Vec::new();
    let mut body_fills = FxHashMap::default();
    for cluster in clusters {
        ctx.checkpoint_emit()?;
        let id = cluster.id.as_str();
        let terminal = ctx.prepared_nodes.cluster(id)?;
        let outside = match ctx.parent.get(id) {
            Some(parent) => body_fills.get(parent).copied().flatten(),
            None => canvas,
        };
        let fill = |base| -> Result<Option<Color>> {
            Ok(
                rect_surface_paint(ctx, base, &terminal.rect_style)?.and_then(
                    |(color, opacity)| {
                        let color = color?;
                        if color.channels()[3] == u8::MAX && opacity == 1.0 {
                            Some(color)
                        } else {
                            outside.map(|outside| color.composite(outside, opacity))
                        }
                    },
                ),
            )
        };
        if let Some(lane) = ctx
            .swimlane_lanes_by_id
            .get(id)
            .filter(|_| super::is_lane_or_pool_cluster(ctx, id))
        {
            let base = literal_fill(&ctx.compatibility.cluster_bkg);
            let palette = super::cluster::lane_color_slot(ctx, id)
                .and_then(|slot| ctx.palette.background_color(slot))
                .and_then(literal_fill);
            let base_body = palette
                .or_else(|| ctx.native.map(|native| native.container))
                .or(base);
            let base_title = palette
                .or_else(|| ctx.native.map(|native| native.emphasis))
                .or(base);
            let (body, title) = super::swimlane_terminal_geometry(lane, direction, true)
                .frame_parts((lane.x, lane.y), direction, true);
            let body_fill = fill(base_body)?;
            body_fills.insert(id, body_fill);
            for (frame, fill) in [(body, body_fill), (title, fill(base_title)?)] {
                if frame.area() > 0.0 {
                    surfaces.push(Surface {
                        frame: frame.to_rounded_rect(0.0),
                        fill,
                    });
                }
            }
        } else if ctx.subgraph_has_children(id) {
            use super::super::{cluster_geometry, render};
            let bounds = cluster_geometry::painted_cluster_frame(ctx, cluster).bounds();
            let radius =
                cluster_geometry::cluster_container_radius(ctx, bounds.width(), bounds.height());
            let fill = fill(literal_fill(render::cluster_base_fill(ctx, terminal, id)))?;
            body_fills.insert(id, fill);
            surfaces.push(Surface {
                frame: bounds.to_rounded_rect(radius),
                fill,
            });
        }
    }
    Ok(surfaces)
}

fn shaft_enters_surface(
    shaft: &BezPath,
    surface: &Surface,
    offset: Vec2,
    half_width: f64,
    ctx: &FlowchartRenderCtx<'_>,
) -> Result<bool> {
    let rect = surface.frame.rect().inflate(half_width, half_width) - offset;
    let radius = surface.frame.radii().top_left;
    let frame = rect.to_rounded_rect(if radius > 0.0 {
        radius + half_width
    } else {
        0.0
    });
    for segment in shaft.segments() {
        if segment_enters_surface(segment, frame, 0, ctx.work_meter)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn contrast(a: f64, b: f64) -> f64 {
    a.max(b) / a.min(b)
}

fn contrasting_ink(
    backgrounds: &mut [f64],
    default: Color,
    work: &crate::resources::OperationWorkMeter,
) -> Result<Option<Color>> {
    let levels = backgrounds.len().checked_ilog2().unwrap_or(0) as usize + 1;
    work.charge(
        backgrounds
            .len()
            .checked_mul(levels + 10)
            .ok_or_else(|| work.arithmetic_overflow())?,
    )?;
    backgrounds.sort_unstable_by(f64::total_cmp);
    let Some((&first, rest)) = backgrounds.split_first() else {
        return Ok(None);
    };
    let mut best = (BLACK, first);
    let white_ratio = WHITE.contrast(BLACK) / rest.last().copied().unwrap_or(first);
    if white_ratio > best.1 {
        best = (WHITE, white_ratio);
    }
    // Each entry is (relative luminance + 0.05) / 0.05. In a gap between two backdrop
    // luminances, the geometric mean maximizes the lower of the two contrast ratios.
    for pair in backgrounds.windows(2) {
        let (dark, light) = (pair[0], pair[1]);
        if (light / dark).sqrt() <= best.1 {
            continue;
        }
        let target = (dark * light).sqrt();
        let (low, high) = if default.contrast(BLACK) >= target {
            (BLACK, default)
        } else {
            (default, WHITE)
        };
        let shade = |step: u16| high.composite(low, f64::from(step) / 255.0);
        let (mut lo, mut hi) = (0, 255);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if shade(mid).contrast(BLACK) < target {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for step in [lo.saturating_sub(1), lo] {
            let ink = shade(step);
            let luminance = ink.contrast(BLACK);
            if (dark..=light).contains(&luminance) {
                let ratio = (luminance / dark).min(light / luminance);
                if ratio > best.1 {
                    best = (ink, ratio);
                }
            }
        }
    }
    Ok((best.1 >= EDGE_CONTRAST).then_some(best.0))
}
