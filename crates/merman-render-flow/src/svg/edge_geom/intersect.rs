//! Bind the facade's route-local node facts to Flow's endpoint geometry.

use crate::edge_geometry::FlowchartEndpointContext;

use super::{BoundaryNode, FlowchartRenderCtx};

pub(in crate::svg) use crate::edge_geometry::force_intersect_for_layout_shape;

pub(in crate::svg) fn intersect_for_layout_shape(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    node: &BoundaryNode,
    layout_shape: Option<&str>,
    point: &merman_render_core::LayoutPoint,
) -> merman_render_core::LayoutPoint {
    let endpoint = FlowchartEndpointContext {
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
    };
    crate::edge_geometry::intersect_for_layout_shape(&endpoint, node_id, node, layout_shape, point)
}
