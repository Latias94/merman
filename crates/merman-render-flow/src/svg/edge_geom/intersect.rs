//! Bind the facade's route-local node facts to Flow's endpoint geometry.

use crate::edge_geometry::FlowchartEndpointContext;
#[cfg(feature = "layout-grid")]
use crate::edge_geometry::{EndpointQuery, intersect_for_layout_shape_with_query};

use super::{BoundaryNode, FlowchartRenderCtx};

pub(in crate::svg) use crate::edge_geometry::force_intersect_for_layout_shape;

pub(in crate::svg) fn intersect_for_layout_shape(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    node: &BoundaryNode,
    layout_shape: Option<&str>,
    point: &merman_render_core::LayoutPoint,
) -> merman_render_core::LayoutPoint {
    crate::edge_geometry::intersect_for_layout_shape(
        &endpoint_context(ctx),
        node_id,
        node,
        layout_shape,
        point,
    )
}

#[cfg(feature = "layout-grid")]
pub(super) fn intersect_grid_node_outline(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    node: &BoundaryNode,
    query: merman_render_core::graph_layout::mermaid_grid::routing::outline::GridOutlineQuery,
) -> merman_render_core::LayoutPoint {
    let layout_shape = if ctx.is_subgraph_collapsed(node_id) {
        Some("collapsedGroup")
    } else {
        ctx.nodes_by_id
            .get(node_id)
            .and_then(|node| node.layout_shape.as_deref())
    };
    let label_metrics = ctx.layout_nodes_by_id.get(node_id).and_then(|node| {
        Some(merman_render_core::text::TextMetrics {
            width: node.label_width?,
            height: node.label_height?,
            // Shape dimensions use only the measured box.
            line_count: 0,
        })
    });
    let adjacent = merman_render_core::LayoutPoint {
        x: query.adjacent.x,
        y: query.adjacent.y,
    };
    let origin = query
        .line_origin
        .map(|p| merman_render_core::LayoutPoint { x: p.x, y: p.y });
    intersect_for_layout_shape_with_query(
        &endpoint_context(ctx),
        node_id,
        node,
        layout_shape,
        EndpointQuery {
            adjacent: &adjacent,
            line_origin: origin.as_ref(),
            label_metrics,
        },
    )
}

fn endpoint_context<'a>(ctx: &'a FlowchartRenderCtx<'a>) -> FlowchartEndpointContext<'a> {
    FlowchartEndpointContext {
        model: ctx.model,
        nodes_by_id: &ctx.nodes_by_id,
        subgraphs_by_id: &ctx.subgraphs_by_id,
        prepared_nodes: ctx.prepared_nodes,
        class_defs: ctx.class_defs,
        label_base_style: if ctx.node_wrap_mode == merman_render_core::text::WrapMode::HtmlLike {
            ctx.html_label_text_style
        } else {
            ctx.text_style
        },
        svg_label_sidecar: ctx.svg_label_sidecar,
        measurer: ctx.measurer,
        math_renderer: ctx.math_renderer,
        config: ctx.config,
        wrapping_width: ctx.wrapping_width,
        node_wrap_mode: ctx.node_wrap_mode,
        node_padding: ctx.node_padding,
        swimlane_label_owner: ctx.swimlane_direction.is_some(),
        swimlane_image_owner: ctx.swimlane_layout.is_some(),
    }
}
