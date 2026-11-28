use super::super::*;
use crate::cluster_shell::{
    FlowchartClusterShape,
    swimlane::{SwimlaneClusterSvg, write_swimlane_cluster_with_label_ink},
};
use crate::graph::FlowchartClusterPass;
use crate::svg::render::cluster::{lane_color_slot, swimlane_label_ink};
use merman_render_core::{SwimlaneLaneLayout, theme_svg::edge_label_background::literal_fill};

pub(in crate::svg) fn render_swimlane_cluster(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    lane: &SwimlaneLaneLayout,
    origin_x: f64,
    origin_y: f64,
    pass: FlowchartClusterPass,
) -> merman_render_core::Result<()> {
    if !pass.draws_swimlane() {
        return Ok(());
    }
    ctx.checkpoint_emit()?;
    if !super::is_lane_or_pool_cluster(ctx, lane.id.as_str()) {
        return Err(merman_render_core::Error::InvalidModel {
            message: format!("Swimlane shell `{}` is neither a lane nor a pool", lane.id),
        });
    }
    let cluster_id = cluster.id.as_str();
    let subgraph = ctx.subgraphs_by_id.get(cluster_id).copied();
    let subgraph_index = ctx.subgraph_indices_by_id.get(cluster_id).copied();
    let terminal = ctx.prepared_nodes.cluster(cluster_id)?;
    let direction =
        ctx.swimlane_direction
            .ok_or_else(|| merman_render_core::Error::InvalidModel {
                message: format!(
                    "missing Swimlane direction while rendering lane `{}`",
                    lane.id
                ),
            })?;
    let canonical = ctx.layout.route_contract.group_parents().is_some();
    let is_pool = super::uses_pool_shell(ctx, lane.id.as_str());
    let color_slot = lane_color_slot(ctx, &lane.id);
    let label_ink = if canonical {
        let title_text_style = ctx
            .svg_label_sidecar
            .and_then(|sidecar| {
                sidecar
                    .swimlane_group_title_owner(cluster_id)
                    .and_then(|owner| sidecar.node_source_style(owner))
            })
            .unwrap_or(&terminal.title_text_style)
            .as_ref();
        let default_ink = ctx.native.map(|native| native.on_emphasis.to_string());
        let fill = color_slot
            .and_then(|slot| ctx.palette.background_color(slot))
            .and_then(literal_fill)
            .or_else(|| ctx.native.map(|native| native.emphasis))
            .or_else(|| literal_fill(&terminal.fill));
        swimlane_label_ink(
            ctx,
            &lane.id,
            &terminal.compiled,
            fill,
            default_ink
                .as_deref()
                .unwrap_or(&ctx.compatibility.title_color),
            title_text_style,
        )?
    } else {
        None
    };
    write_swimlane_cluster_with_label_ink(
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
            shape: FlowchartClusterShape {
                terminal,
                work_meter: ctx.work_meter,
                randomness: &ctx.hand_drawn_seed,
            },
            translation: kurbo::Vec2::new(ctx.tx, ctx.ty),
            group_origin: kurbo::Point::new(origin_x, origin_y),
            html_labels: ctx.swimlane_title_html_labels,
            svg_font_fallback: ctx.svg_label_font_fallback(),
            svg_label_sidecar: ctx.svg_label_sidecar,
            config: ctx.config,
            measurer: ctx.measurer,
            math_renderer: ctx.math_renderer,
        },
        color_slot,
        label_ink.as_deref(),
    )
}
