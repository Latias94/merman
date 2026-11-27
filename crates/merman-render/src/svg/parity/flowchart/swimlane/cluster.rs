use super::super::*;
use crate::model::SwimlaneLaneLayout;
use merman_render_flow::cluster_shell::{
    ClusterHandDrawn, ClusterShellPaint,
    swimlane::{SwimlaneClusterSvg, write_swimlane_cluster},
};

pub(in crate::svg::parity::flowchart) fn render_swimlane_cluster(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    lane: &SwimlaneLaneLayout,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    if !super::is_lane_or_pool_cluster(ctx, lane.id.as_str()) {
        return Err(crate::Error::InvalidModel {
            message: format!("Swimlane shell `{}` is neither a lane nor a pool", lane.id),
        });
    }
    let cluster_id = cluster.id.as_str();
    let subgraph = ctx.subgraphs_by_id.get(cluster_id).copied();
    let subgraph_index = ctx.subgraph_indices_by_id.get(cluster_id).copied();
    let terminal = ctx.prepared_nodes.cluster(cluster_id)?;
    let direction = ctx
        .swimlane_direction
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!(
                "missing Swimlane direction while rendering lane `{}`",
                lane.id
            ),
        })?;
    let canonical = ctx.layout.route_contract.group_parents().is_some();
    let is_pool = super::uses_pool_shell(ctx, lane.id.as_str());
    write_swimlane_cluster(
        out,
        SwimlaneClusterSvg {
            lane,
            cluster_id,
            subgraph,
            subgraph_index,
            model: ctx.model,
            document_ids: ctx.document_ids(),
            palette: &ctx.palette,
            diagram_type: ctx.diagram_type,
            direction,
            canonical,
            is_pool,
            look: ctx.compatibility.look,
            paint: ClusterShellPaint {
                style: terminal.rect_style.as_str(),
                fill: terminal.fill.as_str(),
                stroke: terminal.stroke.as_str(),
            },
            rough: ctx
                .compatibility
                .look
                .is_hand_drawn()
                .then_some(ClusterHandDrawn {
                    stroke_width: terminal.compiled.stroke_width.as_deref(),
                    stroke_dasharray: terminal.compiled.stroke_dasharray.as_deref(),
                    work_meter: ctx.work_meter,
                    randomness: &ctx.hand_drawn_seed,
                }),
            label_style: &terminal.compiled.label_style,
            title_text_style: terminal.title_text_style.as_ref(),
            translation: kurbo::Vec2::new(ctx.tx, ctx.ty),
            group_origin: kurbo::Point::new(origin_x, origin_y),
            html_labels: ctx.swimlane_title_html_labels,
            svg_label_sidecar: ctx.svg_label_sidecar,
            config: ctx.config,
            measurer: ctx.measurer,
            math_renderer: ctx.math_renderer,
        },
    )
}
