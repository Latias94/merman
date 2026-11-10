//! Final ELK Markdown title geometry, after the provider's frame reservation.

use super::*;
use crate::text::TextMetrics;

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
