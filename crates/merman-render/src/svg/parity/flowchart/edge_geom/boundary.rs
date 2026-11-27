use super::super::*;

pub(in crate::svg::parity::flowchart) use merman_render_flow::edge_geometry::EdgeClipRect as BoundaryNode;

pub(in crate::svg::parity::flowchart) fn boundary_for_node(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
) -> Option<BoundaryNode> {
    if let Some(n) = ctx.layout_nodes_by_id.get(node_id) {
        return Some(BoundaryNode {
            x: n.x + ctx.tx - origin_x,
            y: n.y + ctx.ty - origin_y,
            width: ctx
                .elk_cluster_geometry
                .get(node_id)
                .map_or(n.width, |geometry| geometry.width),
            height: n.height,
        });
    }
    let n = ctx.layout_clusters_by_id.get(node_id)?;
    Some(BoundaryNode {
        x: n.x + ctx.tx - origin_x,
        y: n.y + ctx.ty - origin_y,
        width: ctx
            .elk_cluster_geometry
            .get(node_id)
            .map_or(n.width, |geometry| geometry.width),
        height: n.height,
    })
}

pub(in crate::svg::parity::flowchart) fn boundary_for_cluster(
    ctx: &FlowchartRenderCtx<'_>,
    cluster_id: &str,
    origin_x: f64,
    origin_y: f64,
) -> Option<BoundaryNode> {
    if ctx.recursive_clusters.contains(cluster_id) {
        // Mermaid's clusterDb points at the positioned leaf in the parent graph after it
        // measures the fully rendered subgraph. The frame may have a different center or size.
        return boundary_for_node(ctx, cluster_id, origin_x, origin_y);
    }
    let n = ctx.layout_clusters_by_id.get(cluster_id)?;
    let [width, height] =
        super::super::cluster_geometry::cluster_css_size(ctx, cluster_id).unwrap_or_default();
    Some(BoundaryNode {
        x: n.x + ctx.tx - origin_x,
        y: n.y + ctx.ty - origin_y,
        width: width.unwrap_or(n.width),
        height: height.unwrap_or(n.height),
    })
}
