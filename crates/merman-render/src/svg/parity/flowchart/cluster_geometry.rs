//! Flowchart cluster frames and final ELK Markdown title geometry.

use super::*;
use crate::text::TextMetrics;

pub(super) use merman_render_flow::cluster_shell::PaintedClusterFrame;

pub(super) fn painted_cluster_frame(
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
) -> PaintedClusterFrame {
    let width = ctx
        .elk_cluster_geometry
        .get(cluster.id.as_str())
        .map_or(cluster.width, |geometry| geometry.width);
    let lane_or_pool = super::swimlane::is_lane_or_pool_cluster(ctx, cluster.id.as_str());
    let css_size = if lane_or_pool {
        [None; 2]
    } else {
        cluster_css_size(ctx, cluster.id.as_str()).unwrap_or_default()
    };
    PaintedClusterFrame::resolve(cluster, width, lane_or_pool, css_size)
}

pub(super) fn cluster_container_radius(
    ctx: &FlowchartRenderCtx<'_>,
    width: f64,
    height: f64,
) -> f64 {
    merman_render_flow::cluster_shell::cluster_container_radius(
        ctx.native.map(|native| native.form.container_radius),
        ctx.diagram_type == "agentflow",
        kurbo::Size::new(width, height),
    )
}

pub(super) fn cluster_css_size(ctx: &FlowchartRenderCtx<'_>, id: &str) -> Option<[Option<f64>; 2]> {
    if ctx.compatibility.look.is_hand_drawn() || super::swimlane::is_lane_or_pool_cluster(ctx, id) {
        return None;
    }
    let terminal = ctx.prepared_nodes.cluster(id).ok()?;
    if ctx.diagram_type == "agentflow" {
        return None;
    }
    let size = terminal.compiled.rectangle_size;
    (size != [None; 2]).then_some(size)
}

#[derive(Clone, Copy)]
pub(super) struct FlowchartElkClusterGeometry {
    pub(super) width: f64,
    pub(super) label: TextMetrics,
}

pub(super) fn prepare_elk_cluster_geometry<'a>(
    ctx: &FlowchartRenderCtx<'a>,
) -> Result<FxHashMap<&'a str, FlowchartElkClusterGeometry>> {
    let mut geometry = FxHashMap::default();
    if !ctx.uses_elk_adapter_dom || !ctx.edge_html_labels {
        return Ok(geometry);
    }
    // ELK emits prepared clusters in this order, which can differ from the
    // id-sorted layout projection. Keep opaque host measurements in writer order.
    for id in ctx.prepared_nodes.cluster_ids() {
        ctx.checkpoint_emit()?;
        ctx.work_meter.charge(1)?;
        let cluster =
            ctx.layout_clusters_by_id
                .get(id)
                .copied()
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("missing prepared Flowchart cluster geometry `{id}`"),
                })?;
        let Some(subgraph) = ctx.subgraphs_by_id.get(cluster.id.as_str()) else {
            continue;
        };
        if subgraph.label_type.as_deref() != Some("markdown") {
            continue;
        }
        let Some(&index) = ctx.subgraph_indices_by_id.get(cluster.id.as_str()) else {
            continue;
        };
        let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
        // clusters.js passes the allocated width to createText before expanding the
        // painted rectangle. Keep that original constraint when min-content grows.
        let label = crate::flowchart::flowchart_label_metrics_for_layout(
            crate::flowchart::FlowchartLabelMetricsRequest {
                measurer: ctx.measurer,
                raw_label: ctx.model.subgraph_title_for_render(index, subgraph),
                label_type: "markdown",
                style: terminal.title_text_style.as_ref(),
                max_width_px: Some(cluster.width.max(1.0)),
                wrap_mode: ctx.edge_wrap_mode,
                config: ctx.config,
                math_renderer: ctx.math_renderer,
            },
        );
        // ELK frame reservations never exceed provider bounds. Mermaid's later
        // cluster painter separately admits the measured title plus node.padding.
        geometry.insert(
            cluster.id.as_str(),
            FlowchartElkClusterGeometry {
                width: cluster.width.max(1.0).max(label.width + cluster.padding),
                label,
            },
        );
    }
    Ok(geometry)
}
