//! Native node elevation reuses the prepared shape geometry.

use super::*;
use crate::flowchart::FlowchartShape;
use kurbo::Shape;

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
    let width = node.width;
    let height = node.height;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    match FlowchartShape::resolve(info.shape).ok()? {
        FlowchartShape::Process | FlowchartShape::RoundedRectangle => {
            let rect = ctx.rectangle_geometry(node).shape;
            let radius = prepared.selection.source_radii[0]
                .or_else(|| prepared.selection.native_radius.map(f64::from))
                .unwrap_or(0.0);
            Some(rect.to_rounded_rect(radius).to_path(0.05))
        }
        FlowchartShape::Stadium => Some(
            kurbo::Rect::from_center_size((0.0, 0.0), (width, height))
                .to_rounded_rect(height / 2.0)
                .to_path(0.05),
        ),
        FlowchartShape::Circle | FlowchartShape::DoubleCircle => {
            Some(kurbo::Circle::new((0.0, 0.0), width.min(height) / 2.0).to_path(0.05))
        }
        FlowchartShape::Diamond => {
            let (dx, dy) = (width / 2.0, height / 2.0);
            let mut path = kurbo::BezPath::new();
            path.move_to((0.5, -dy));
            path.line_to((0.5 + dx, 0.0));
            path.line_to((0.5, dy));
            path.line_to((0.5 - dx, 0.0));
            path.close_path();
            Some(path)
        }
        _ => None,
    }
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
        ctx.work_meter.charge(silhouette.elements().len())?;
        let _ = crate::theme::elevation_svg::write_depth(
            out,
            &native.elevation,
            &id.to_string(),
            &silhouette,
            (node.x + ctx.tx - origin_x, node.y + ctx.ty - origin_y),
        );
    }
    out.checkpoint()
}
