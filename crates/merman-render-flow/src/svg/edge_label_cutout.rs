//! Reserve a transparent window in each shaft beneath its own edge label.

use super::*;
use crate::projection::FlowchartEdgeKey;
use kurbo::{Rect, Vec2};
use merman_render_core::theme_svg::edge_label_background::literal_fill;
use merman_theme_contract::v2::{EdgeLabelBackground, Paint};

// A clip affects the shaft and its marker paint. The viewport may exclude off-canvas ink.
const CUTOUT_FRAME_OUTSET: f64 = 32.0;

pub(super) fn applies(ctx: &FlowchartRenderCtx<'_>, services: &FlowchartSvgServices<'_>) -> bool {
    if !ctx.layout.route_contract.is_canonical_axis() || ctx.diagram_type == "agentflow" {
        return false;
    }
    if let Some(native) = ctx.native {
        if native.form.edge_label_bg == EdgeLabelBackground::Transparent {
            return true;
        }
    } else if literal_fill(&ctx.compatibility.edge_label_background)
        .is_some_and(|color| color.channels()[3] == 0)
    {
        return true;
    }

    services
        .canvas_background
        .or_else(|| match &services.native_family_theme?.roles.canvas {
            Paint::Solid(color) => Some(*color),
            Paint::LinearGradient(_) => None,
        })
        .is_some_and(|canvas| canvas.channels()[3] == 0)
}

pub(super) fn write_definitions(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    labels: &super::paint::FlowchartEdgeLabels,
    viewport: Rect,
) -> Result<FxHashSet<FlowchartEdgeKey>> {
    let mut cutouts = FxHashSet::default();
    ctx.work_meter.charge(ctx.edge_order.len())?;
    for edge in &ctx.edge_order {
        let key = edge.key;
        let Some((cached, label)) = cache.get(&key).zip(
            ctx.layout_edges_by_key
                .get(&key)
                .and_then(|edge| edge.label()),
        ) else {
            continue;
        };
        if edge.edge.visibility == merman_model::families::flowchart::FlowEdgeVisibility::Invisible
            || label.width <= 0.0
            || label.height <= 0.0
        {
            continue;
        }
        let Some(position) = labels.local_position(key, ctx, cached.origin_x, cached.origin_y)
        else {
            continue;
        };
        let Some(id) = ctx.document_ids().edge(key) else {
            continue;
        };
        ctx.checkpoint_emit()?;
        let gap = Rect::from_center_size((position.x, position.y), (label.width, label.height));
        let path = &cached.geom;
        let ink = if let Some(d) = &path.emitted_d_for_label {
            ctx.work_meter.charge_emit_work(d.len().div_ceil(16))?;
            path_bounds::svg_path_bounds_from_d(d)
        } else {
            path.pb
        };
        let mut outer =
            (viewport - Vec2::new(cached.origin_x, cached.abs_top_transform)).union(gap);
        if let Some(ink) = ink {
            outer = outer.union(Rect::new(ink.min_x, ink.min_y, ink.max_x, ink.max_y));
        }
        #[cfg(any(feature = "canonical-edge-paint", test))]
        let outset = CUTOUT_FRAME_OUTSET
            + ctx
                .edge_style_plan
                .stroke_outset_for(key)?
                .unwrap_or_default();
        #[cfg(not(any(feature = "canonical-edge-paint", test)))]
        let outset = CUTOUT_FRAME_OUTSET;
        let outer = outer.inflate(outset, outset);
        if cutouts.is_empty() {
            out.push_str("<defs>");
        }
        let _ = write!(
            out,
            r#"<clipPath id="{id}-label-cutout"><path clip-rule="evenodd" d="M{} {}H{}V{}H{}ZM{} {}H{}V{}H{}Z"/></clipPath>"#,
            fmt_display(outer.x0),
            fmt_display(outer.y0),
            fmt_display(outer.x1),
            fmt_display(outer.y1),
            fmt_display(outer.x0),
            fmt_display(gap.x0),
            fmt_display(gap.y0),
            fmt_display(gap.x1),
            fmt_display(gap.y1),
            fmt_display(gap.x0),
        );
        out.checkpoint()?;
        cutouts.insert(key);
    }
    if !cutouts.is_empty() {
        out.push_str("</defs>");
        out.checkpoint()?;
    }
    Ok(cutouts)
}

pub(crate) fn write_attribute(
    out: &mut impl std::fmt::Write,
    id: super::document_ids::FlowchartDocumentId<'_>,
) {
    let _ = write!(out, r#" clip-path="url(#{id}-label-cutout)""#);
}
