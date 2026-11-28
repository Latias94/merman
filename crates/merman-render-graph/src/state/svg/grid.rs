//! Grid transition paint over the original route and its prepared label anchor.

use super::*;
use crate::state::grid::{allocation_error, buffer};
use kurbo::{Point, Rect};
use merman_model::OperationPhase;
use merman_render_core::{
    Error,
    graph_layout::{
        Size,
        mermaid_grid::{
            GridLineHops, GridPaint,
            routing::{
                outline::{GridOutline, GridOutlineQuery, clip_grid_endpoints},
                paint::{GridPaintedEdge, grid_line_hop_overrides},
            },
        },
    },
    svg_edge_parity::{
        curve::{curve_linear_path_d, curve_rounded_path_d, interpolated_path_d},
        edge_path::maybe_fix_corners,
        line_hops::LineHopEdge,
    },
    theme_svg::edge_label_background::{adaptive_text_ink, literal_fill, write_backplate},
};
use rustc_hash::FxHashSet;
use std::borrow::Cow;

pub(super) type StateGridEdges<'a> = FxHashMap<&'a str, StateGridEdgePaint>;

pub(super) struct StateGridEdgePaint {
    pub(super) points: Vec<LayoutPoint>,
    pub(super) path: String,
    pub(super) hopped: Option<String>,
}

pub(super) fn prepare_edges<'a>(
    ctx: &StateRenderCtx<'a>,
    settings: GridPaint,
) -> Result<StateGridEdges<'a>> {
    let work = ctx.work_meter;
    let mut edges = StateGridEdges::default();
    if !ctx.include_edges {
        return Ok(edges);
    }
    work.charge_emit_work(ctx.edges.len())?;
    edges
        .try_reserve(ctx.layout_edges_by_id.len())
        .map_err(|_| allocation_error())?;
    let mut route = Vec::new();
    for edge in ctx.edges {
        work.checkpoint(OperationPhase::Emit)?;
        let Some(layout) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
            continue;
        };
        if layout.points.len() < 2 {
            continue;
        }
        work.charge_emit_work(edge.id.len().max(1))?;
        work.charge_emit_work(layout.points.len())?;
        route.clear();
        route
            .try_reserve(layout.points.len())
            .map_err(|_| allocation_error())?;
        route.extend(layout.points.iter().map(|p| Point::new(p.x, p.y)));
        let source = endpoint(ctx, &layout.from);
        let target = endpoint(ctx, &layout.to);
        let intersect_source = |query| Ok(source.and_then(|shape| shape.intersect(query)));
        let intersect_target = |query| Ok(target.and_then(|shape| shape.intersect(query)));
        let clipped = clip_grid_endpoints(
            &route,
            source.map(|shape| shape.outline(&intersect_source)),
            target.map(|shape| shape.outline(&intersect_target)),
            work,
        )?;
        work.charge_emit_work(clipped.len())?;
        let mut points = buffer(clipped.len())?;
        points.extend(clipped.iter().map(|p| LayoutPoint { x: p.x, y: p.y }));
        let arrow = Some(edge.arrow_type_end.as_str());
        work.charge_emit_work(points.len())?;
        let path = match settings.curve {
            "rounded" => curve_rounded_path_d(
                &state_rounded_end_marker_offset_points(&points, arrow),
                settings.corner_radius,
            ),
            "linear" => {
                curve_linear_path_d(&state_line_with_end_marker_offset_points(&points, arrow))
            }
            name => {
                let mut adjusted = buffer(points.len())?;
                adjusted.extend_from_slice(&points);
                maybe_fix_corners(&mut adjusted);
                interpolated_path_d(
                    &state_line_with_end_marker_offset_points(&adjusted, arrow),
                    name,
                )
            }
        };
        work.charge_emit_work(path.len())?;
        edges.insert(
            edge.id.as_str(),
            StateGridEdgePaint {
                points,
                path,
                hopped: None,
            },
        );
    }
    if settings.line_hops != GridLineHops::Disabled
        && ctx.style_plan.compatibility().native_edge_style()
            != Some(merman_theme_contract::v2::EdgeStyle::Sketch)
    {
        work.charge_emit_work(ctx.edges.len())?;
        let mut painted = buffer(edges.len())?;
        let mut owners = buffer(edges.len())?;
        for edge in ctx.edges {
            if let Some(paint) = edges.get(edge.id.as_str()) {
                owners.push(edge.id.as_str());
                painted.push(GridPaintedEdge {
                    geometry: LineHopEdge {
                        id: &edge.id,
                        points: &paint.points,
                        curve: Some(settings.curve),
                        arrow_type_start: None,
                        arrow_type_end: Some(edge.arrow_type_end.as_str()),
                    },
                    corner_radius: Some(settings.corner_radius),
                    original_path: &paint.path,
                });
            }
        }
        let overrides = grid_line_hop_overrides(settings.line_hops, &painted, work)?;
        drop(painted);
        for (slot, path) in overrides {
            work.charge_emit_work(path.len())?;
            if let Some(paint) = edges.get_mut(owners[slot]) {
                paint.hopped = Some(path);
            }
        }
    }
    work.checkpoint(OperationPhase::Emit)?;
    Ok(edges)
}

#[derive(Clone, Copy)]
struct StateOutline<'a> {
    node: &'a LayoutNode,
    diamond: bool,
}

fn endpoint<'a>(ctx: &StateRenderCtx<'a>, id: &str) -> Option<StateOutline<'a>> {
    let semantic = ctx.nodes_by_id.get(id)?;
    let node = ctx.layout_nodes_by_id.get(id).copied()?;
    // Rect, rectWithTitle, note, fork/join and every State cluster supply a rectangular
    // intersection. Their solved contacts are already on the attachment face.
    match semantic.shape.as_str() {
        "stateStart" | "stateEnd" => Some(StateOutline {
            node,
            diamond: false,
        }),
        "choice" => Some(StateOutline {
            node,
            diamond: true,
        }),
        _ => None,
    }
}

impl StateOutline<'_> {
    fn outline<'a>(
        self,
        intersect: &'a dyn Fn(GridOutlineQuery) -> Result<Option<Point>>,
    ) -> GridOutline<'a> {
        GridOutline {
            center: Some(Point::new(self.node.x, self.node.y)),
            size: Some(Size {
                width: self.node.width,
                height: self.node.height,
            }),
            intersect,
        }
    }

    fn intersect(self, query: GridOutlineQuery) -> Option<Point> {
        let origin = query.line_origin?;
        let direction = query.adjacent - origin;
        let center = Point::new(self.node.x, self.node.y);
        let vertical = direction.x.abs() <= direction.y.abs();
        let (cross_radius, along_radius, offset, along) = if vertical {
            (
                self.node.width / 2.0,
                self.node.height / 2.0,
                origin.x - center.x,
                direction.y,
            )
        } else {
            (
                self.node.height / 2.0,
                self.node.width / 2.0,
                origin.y - center.y,
                direction.x,
            )
        };
        let fraction = offset.abs() / cross_radius;
        if !fraction.is_finite() || fraction > 1.0 || along == 0.0 {
            return None;
        }
        let factor = if self.diamond {
            1.0 - fraction
        } else {
            (1.0 - fraction * fraction).sqrt()
        };
        let extent = (along_radius * factor).copysign(along);
        Some(if vertical {
            Point::new(origin.x, center.y + extent)
        } else {
            Point::new(center.x + extent, origin.y)
        })
    }
}

pub(super) fn write_labels(out: &mut impl SvgOutput, ctx: &StateRenderCtx<'_>) -> Result<()> {
    let labels = ctx.label_sidecar.grid_labels();
    if labels.is_empty() {
        return Ok(());
    }
    let work = ctx.work_meter;
    work.charge_emit_work(ctx.layout_nodes_by_id.len())?;
    let capacity = ctx
        .layout_nodes_by_id
        .len()
        .checked_add(labels.len())
        .ok_or_else(|| work.arithmetic_overflow())?;
    let mut occupied: FxHashSet<Cow<'_, str>> = FxHashSet::default();
    occupied
        .try_reserve(capacity)
        .map_err(|_| allocation_error())?;
    for &id in ctx.layout_nodes_by_id.keys() {
        work.charge_emit_work(id.len())?;
        occupied.insert(Cow::Borrowed(id));
    }
    let compatibility = ctx.style_plan.compatibility();
    for prepared in labels {
        work.checkpoint(OperationPhase::Emit)?;
        let edge = ctx.layout_edges_by_id.get(prepared.edge_id.as_ref());
        let (Some(edge), Some(label)) = (edge, edge.and_then(|edge| edge.label.as_ref())) else {
            return Err(Error::InvalidModel {
                message: format!("missing State Grid label anchor for `{}`", prepared.edge_id),
            });
        };
        for text in [&edge.from, &edge.to, &edge.id] {
            work.charge_emit_work(text.len())?;
        }
        let base = format!("edge-label-{}-{}-{}", edge.from, edge.to, edge.id);
        let mut id = base.clone();
        let mut suffix = 1usize;
        while occupied.contains(id.as_str()) {
            work.charge_emit_work(id.len().max(1))?;
            id = format!("{base}-{suffix}");
            suffix = suffix
                .checked_add(1)
                .ok_or_else(|| work.arithmetic_overflow())?;
        }
        work.charge_emit_work(id.len().max(1))?;
        if let Some(paint) = compatibility.native_edge_label_paint {
            let center = Point::new(label.x, label.y);
            let fill = state_edge_label_fill(ctx, paint, center)?;
            let ink = literal_fill(&compatibility.transition_label_color)
                .and_then(|ink| adaptive_text_ink(fill, ink))
                .unwrap_or(&compatibility.transition_label_color);
            let _ = write!(
                out,
                r#"<g class="merman-state-grid-transition-label" style="color:{}">"#,
                escape_attr_display(ink),
            );
            let bounds = Rect::from_center_size(center, (label.width, label.height));
            let _ = write_backplate(out, bounds, paint.mode, fill, paint.border);
            out.checkpoint()?;
        }
        prepared
            .terminal
            .write(out, label, &id, (0.0, 0.0), false)?;
        if compatibility.native_edge_label_paint.is_some() {
            out.push_str("</g>");
            out.checkpoint()?;
        }
        occupied.insert(Cow::Owned(id));
    }
    Ok(())
}

pub(super) fn write_native_label_css(
    out: &mut impl std::fmt::Write,
    id: CssSelectorDiagramId<SvgDiagramId<'_>>,
    style: &crate::state::StateStylePlan,
) {
    let compatibility = style.compatibility();
    if compatibility.native_edge_label_paint.is_some() {
        let ink = &compatibility.transition_label_color;
        let _ = write!(
            out,
            "#{id} .merman-state-grid-transition-label .edgeLabel .label text{{fill:var(--merman-state-transition-ink,{ink});}}#{id} .merman-state-grid-transition-label .edgeLabel .nodeLabel{{color:var(--merman-state-transition-ink,{ink});}}",
        );
    }
}
