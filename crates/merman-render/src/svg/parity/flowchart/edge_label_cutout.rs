//! Reserve a transparent window in each shaft beneath its own edge label.

use super::*;
use crate::flowchart::FlowchartEdgeKey;
use kurbo::{Rect, Vec2};

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
        let outer = (viewport - Vec2::new(cached.origin_x, cached.abs_top_transform))
            .union(gap)
            .inflate(1.0, 1.0);
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

pub(super) fn write_attribute(
    out: &mut impl std::fmt::Write,
    id: super::document_ids::FlowchartDocumentId<'_>,
) {
    let _ = write!(out, r#" clip-path="url(#{id}-label-cutout)""#);
}
