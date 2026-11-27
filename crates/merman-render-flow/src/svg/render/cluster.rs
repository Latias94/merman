//! Flowchart cluster renderer.

use super::super::*;
use crate::svg::cluster_geometry::painted_cluster_frame;

use crate::cluster_shell::{
    ClusterHandDrawn, FlowchartClusterShape, FlowchartClusterSvg, FlowchartClusterTitle,
};

use crate::graph::FlowchartClusterPass;

pub fn advance_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> merman_render_core::Result<()> {
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(merman_render_core::Error::from)?;
    if ctx.compatibility.look.as_str() != "handDrawn"
        || ctx
            .swimlane_lanes_by_id
            .get(cluster.id.as_str())
            .is_some_and(|lane| lane.parent_id.is_none())
        || (!ctx.subgraph_has_children(cluster.id.as_str())
            && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx))
    {
        return Ok(());
    }
    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let frame = painted_cluster_frame(ctx, cluster).in_root(
        kurbo::Vec2::new(ctx.tx, ctx.ty),
        kurbo::Point::new(origin_x, origin_y),
    );
    ClusterHandDrawn::for_compiled_styles(&terminal.compiled, ctx.work_meter, &ctx.hand_drawn_seed)
        .advance_cluster(frame, ctx.diagram_type == "agentflow");
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(merman_render_core::Error::from)
}

#[cfg(feature = "layout-shoal")]
pub(in crate::svg) fn cluster_base_fill<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    terminal: &'a crate::node_inventory::FlowchartPreparedCluster,
    id: &str,
) -> &'a str {
    if ctx.diagram_type == "agentflow" {
        return FlowchartClusterShape::agentflow_fill(
            &terminal.fill,
            &ctx.palette,
            super::super::agentflow::container_color_slot(ctx, id),
        );
    }
    &terminal.fill
}

#[cfg(feature = "layout-shoal")]
pub(in crate::svg) fn compat_theme_canvas_fallback(
    ctx: &FlowchartRenderCtx<'_>,
) -> merman_theme_contract::v2::Color {
    FlowchartClusterSvg::compat_theme_canvas_fallback(ctx.config)
}

#[cfg(feature = "test-support")]
pub fn render_flowchart_cluster(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> merman_render_core::Result<()> {
    render_flowchart_cluster_pass(
        out,
        ctx,
        cluster,
        origin_x,
        origin_y,
        FlowchartClusterPass::Complete,
    )
}

pub(in crate::svg) fn render_flowchart_cluster_pass(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
    pass: FlowchartClusterPass,
) -> merman_render_core::Result<()> {
    if let Some(lane) = ctx.swimlane_lanes_by_id.get(cluster.id.as_str())
        && super::super::swimlane::is_lane_or_pool_cluster(ctx, cluster.id.as_str())
    {
        return super::super::swimlane::render_swimlane_cluster(
            out, ctx, cluster, lane, origin_x, origin_y, pass,
        );
    }

    let Some(sg) = ctx.subgraphs_by_id.get(cluster.id.as_str()) else {
        return Err(merman_render_core::Error::InvalidModel {
            message: format!(
                "Flowchart layout cluster `{}` has no semantic subgraph owner",
                cluster.id
            ),
        });
    };
    let Some(subgraph_index) = ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied() else {
        return Err(merman_render_core::Error::InvalidModel {
            message: format!(
                "missing semantic Flowchart subgraph index for cluster `{}`",
                cluster.id
            ),
        });
    };
    if !ctx.subgraph_has_children(cluster.id.as_str())
        && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx)
    {
        return Ok(());
    }

    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let classes = ctx.model.effective_subgraph_css(subgraph_index, sg).0;

    let elk_geometry = ctx.elk_cluster_geometry.get(cluster.id.as_str());
    let translation = kurbo::Vec2::new(ctx.tx, ctx.ty);
    let group_origin = kurbo::Point::new(origin_x, origin_y);
    let frame = painted_cluster_frame(ctx, cluster).in_root(translation, group_origin);
    let Some(cluster_dom_id) = ctx.document_ids().cluster(cluster.id.as_str()) else {
        return Err(merman_render_core::Error::InvalidModel {
            message: format!(
                "missing prepared Flowchart DOM id for cluster `{}`",
                cluster.id
            ),
        });
    };
    ctx.checkpoint_emit()?;

    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let render_title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
    let title_owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.subgraph_title_owner(cluster.id.as_str()));
    let title_text_style = terminal.title_text_style.as_ref();
    let markdown_wrap_width =
        FlowchartClusterTitle::markdown_wrap_width(cluster, ctx.uses_elk_adapter_dom);
    let painted_markdown = elk_geometry.map(|geometry| geometry.label).or_else(|| {
        (ctx.uses_elk_adapter_dom && label_type == "markdown").then(|| {
            crate::svg::label::flowchart_label_metrics_for_layout(
                merman_render_core::text::create_text_html::FlowchartLabelMetricsRequest {
                    measurer: ctx.measurer,
                    raw_label: render_title,
                    label_type,
                    style: title_text_style,
                    max_width_px: Some(markdown_wrap_width),
                    wrap_mode: ctx.edge_wrap_mode,
                    config: ctx.config,
                    math_renderer: ctx.math_renderer,
                },
            )
        })
    });
    let struck = ctx
        .layout
        .route_contract
        .struck_group_titles()
        .is_some_and(|titles| titles.contains(cluster.id.as_str()));
    let svg = FlowchartClusterSvg {
        semantic_id: &cluster.id,
        document_id: cluster_dom_id,
        classes,
        model: ctx.model,
        diagram_type: ctx.diagram_type,
        compatibility: ctx.compatibility,
        native: ctx.native,
        palette: &ctx.palette,
        canonical_axis: ctx.layout.route_contract.is_canonical_axis(),
        struck,
        frame,
        prepared_nodes: ctx.prepared_nodes,
        parent: &ctx.parent,
    };
    let shape = FlowchartClusterShape {
        terminal,
        work_meter: ctx.work_meter,
        randomness: &ctx.hand_drawn_seed,
    };
    let title = FlowchartClusterTitle {
        layout: cluster,
        translation,
        group_origin,
        canonical_group_title: ctx.layout.route_contract.group_parents().is_some(),
        painted_markdown,
        markdown_wrap_width,
        label_type,
        source: render_title,
        html_labels: ctx.edge_html_labels,
        uses_elk_adapter_dom: ctx.uses_elk_adapter_dom,
        owner: title_owner,
        sidecar: ctx.svg_label_sidecar,
        measurer: ctx.measurer,
        config: ctx.config,
        math_renderer: ctx.math_renderer,
    };
    svg.write(out, pass, shape, title)
}
