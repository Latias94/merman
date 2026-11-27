//! Native node elevation reuses the prepared shape geometry.

use super::*;
use merman_render_flow::node_body::{native_node_silhouette, write_native_node_depth};

pub(in crate::svg::parity::flowchart) fn native_silhouette(
    ctx: &FlowchartRenderCtx<'_>,
    node: &LayoutNode,
) -> Option<kurbo::BezPath> {
    let native = ctx.native?;
    if matches!(native.elevation, merman_theme_contract::v2::Elevation::Flat)
        || ctx.compatibility.look.is_hand_drawn()
    {
        return None;
    }
    let info = helpers::resolve_node_render_info(ctx, &node.id)?;
    let prepared = ctx.prepared_nodes.node(&node.id)?;
    if prepared.source.has_source_filter() {
        return None;
    }
    native_node_silhouette(
        node,
        info.shape,
        &prepared.selection,
        ctx.svg_label_sidecar,
        ctx.swimlane_layout.is_some(),
    )
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_node_depth(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    let Some(native) = ctx.native else {
        return Ok(());
    };
    if let Some(node) = ctx.layout_nodes_by_id.get(node_id)
        && let Some(silhouette) = native_silhouette(ctx, node)
        && let Some(id) = ctx.document_ids().node(node_id)
    {
        write_native_node_depth(
            out,
            &native.elevation,
            id,
            &silhouette,
            (node.x + ctx.tx - origin_x, node.y + ctx.ty - origin_y),
            ctx.work_meter,
        )?;
    }
    out.checkpoint()
}
