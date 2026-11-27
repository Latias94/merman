use indexmap::IndexMap;
use kurbo::{Point, Rect, RoundedRect, Shape};
use rustc_hash::FxBuildHasher;

use super::*;
use crate::state::{StateNodeKind, StateNodeStylePlan, StateTerminalPaintProperty};
use merman_render_core::theme_svg::{canvas::write_color, edge_label_background::literal_fill};
use merman_theme_contract::v2::Color;

pub(super) type StateCompositePaints<'a> = IndexMap<&'a str, StateCompositePaint, FxBuildHasher>;

#[derive(Clone, Copy)]
struct Fill {
    color: Color,
    opacity: f64,
}

impl Fill {
    fn is_translucent(self) -> bool {
        self.color.channels()[3] < 255 || self.opacity < 1.0
    }
}

pub(super) struct StateCompositePaint {
    outer: RoundedRect,
    inner: RoundedRect,
    header: Option<Fill>,
    body: Option<Fill>,
    header_color: Option<Color>,
}

pub(super) fn composite_body_geometry(
    cluster: &LayoutCluster,
    outer_y: f64,
    origin_y: f64,
    canonical: bool,
) -> (f64, f64) {
    let title_height = cluster.title_label.height.max(0.0);
    if canonical {
        let title_y = cluster.title_label.y - title_height / 2.0 - origin_y;
        let inner_y = title_y + title_height + 1.0;
        (inner_y, (outer_y + cluster.height - 4.0 - inner_y).max(1.0))
    } else {
        (
            outer_y + title_height + 2.0,
            (cluster.height - title_height - 6.0).max(1.0),
        )
    }
}

pub(super) fn prepare_composite_paints<'a>(
    ctx: &StateRenderCtx<'a>,
) -> Result<StateCompositePaints<'a>> {
    if ctx.layout_clusters_by_id.is_empty() {
        return Ok(StateCompositePaints::default());
    }
    ctx.work_meter.charge_emit_work(ctx.node_order.len())?;
    let compatibility = ctx.style_plan.compatibility();
    let mut paints = Vec::with_capacity(ctx.layout_clusters_by_id.len());
    for &id in &ctx.node_order {
        let (Some(&node), Some(&cluster)) =
            (ctx.nodes_by_id.get(id), ctx.layout_clusters_by_id.get(id))
        else {
            continue;
        };
        if !node.is_group
            || matches!(node.shape.as_str(), "noteGroup" | "divider")
            || state_is_hidden(ctx, id)
        {
            continue;
        }

        let mut depth = 0;
        let mut parent = ctx.parent.get(id);
        while let Some(&id) = parent {
            ctx.work_meter.charge_emit_work(1)?;
            depth += 1;
            parent = ctx.parent.get(id);
        }
        let Some(surface) = compatibility.node_shape_surface(node, StateNodeKind::Composite) else {
            continue;
        };
        let body_css = compatibility.terminal_paint_css(
            surface,
            StateTerminalPaintProperty::Fill,
            ctx.diagram_id,
        );
        let header_css = compatibility.terminal_paint_css(
            compatibility.composite_header_surface(),
            StateTerminalPaintProperty::Fill,
            ctx.diagram_id,
        );
        let style = ctx.style_plan.node(id);
        let fill_override = style.and_then(StateNodeStylePlan::fill_override);
        let opacity = style.map_or(Some(1.0), StateNodeStylePlan::composite_fill_opacity);
        let fill = |color| opacity.map(|opacity| Fill { color, opacity });
        let body = literal_fill(fill_override.unwrap_or(&body_css)).and_then(fill);
        // In SVG text mode Mermaid's assigned class also targets the outer rect. In HTML mode
        // the class targets the rect's wrapper, where fill does not override the rect's rule.
        let header_css = if !ctx.html_labels && !compatibility.is_hand_drawn() {
            fill_override.unwrap_or(&header_css)
        } else {
            &header_css
        };
        let header_color = literal_fill(header_css);
        let header = header_color.and_then(fill);

        let x = cluster.x - cluster.width / 2.0;
        let y = cluster.y - cluster.height / 2.0;
        let width = cluster.width.max(1.0);
        let (inner_y, inner_height) =
            composite_body_geometry(cluster, y, 0.0, ctx.uses_canonical_routes());
        let body_radius = style
            .and_then(StateNodeStylePlan::radius_override)
            .unwrap_or(0.0);
        let header_radius = style
            .and_then(StateNodeStylePlan::composite_outer_radius_override)
            .unwrap_or(0.0);
        let radius = |radius| {
            if compatibility.is_hand_drawn() {
                0.0
            } else {
                radius
            }
        };
        paints.push((
            depth,
            id,
            StateCompositePaint {
                outer: Rect::from_origin_size((x, y), (width, cluster.height.max(1.0)))
                    .to_rounded_rect(radius(header_radius)),
                inner: Rect::from_origin_size((x, inner_y), (width, inner_height))
                    .to_rounded_rect(radius(body_radius)),
                header,
                body,
                header_color,
            },
        ));
    }
    // Parent paint is laid down before the nested composite. Preserve source order at each depth.
    paints.sort_by_key(|(depth, _, _)| *depth);
    Ok(paints
        .into_iter()
        .map(|(_, id, paint)| (id, paint))
        .collect())
}

impl StateCompositePaint {
    fn header_stops_at_body(&self) -> bool {
        self.body.is_some_and(Fill::is_translucent) && self.header_color.is_some()
    }

    pub(super) fn composite_at(&self, point: Point, backdrop: Color) -> Color {
        let mut backdrop = backdrop;
        if self.outer.contains(point)
            && (!self.header_stops_at_body() || point.y < self.inner.rect().y0)
            && let Some(header) = self.header
        {
            backdrop = header.color.composite(backdrop, header.opacity);
        }
        if self.inner.contains(point)
            && let Some(body) = self.body
        {
            backdrop = body.color.composite(backdrop, body.opacity);
        }
        backdrop
    }

    pub(super) fn write_header_fill(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        ordinal: usize,
        outer_y: f64,
        inner_y: f64,
    ) -> Option<String> {
        let header = self.header_color.filter(|_| self.header_stops_at_body())?;
        let id = format!("{diagram_id}-merman-state-composite-header-{ordinal}");
        // Draw the original rounded outline, with the header fill stopping where the body
        // begins. Otherwise translucent bodies would pick up their own header fill.
        let _ = write!(
            out,
            r#"<defs><linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="0" x2="0" y1="{}" y2="{}">"#,
            fmt(outer_y),
            fmt(inner_y)
        );
        for (offset, opacity) in [(0, 1.0), (1, 1.0), (1, 0.0)] {
            let _ = write!(out, r#"<stop offset="{offset}" "#);
            let _ = write_color(out, "stop-color", header, opacity);
            out.push_str("/>");
        }
        out.push_str("</linearGradient></defs>");
        Some(format!("url(#{id})"))
    }
}
