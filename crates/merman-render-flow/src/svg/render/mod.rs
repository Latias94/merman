//! Flowchart SVG renderer (core drawing routines).

pub(super) mod cluster;
pub(in crate::svg) mod edge_label;
mod edge_path;
pub(in crate::svg) mod node;
mod root;

pub(in crate::svg) use cluster::advance_rough_geometry as advance_flowchart_cluster_rough_geometry;
#[cfg(feature = "layout-shoal")]
pub(in crate::svg) use cluster::{cluster_base_fill, compat_theme_canvas_fallback};
pub(super) use edge_label::{render_flowchart_edge_label, resolve_flowchart_edge_label_position};
pub(super) use edge_path::render_flowchart_edge_path;
pub(super) use merman_render_core::svg_edge_parity::edge_path::neo_marker_mask_offset as flowchart_neo_marker_mask_offset;
pub(super) use node::render_flowchart_node;
pub(crate) use root::FlowchartRootPaint;

fn flowchart_elk_renders_empty_subgraph_as_cluster(ctx: &super::FlowchartRenderCtx<'_>) -> bool {
    ctx.uses_elk_adapter_dom
}
