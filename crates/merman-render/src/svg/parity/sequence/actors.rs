use super::super::*;
use super::SequenceEmitCheckpoints;
use super::actor_shapes::{
    ActorLabelContext, LIFELINE_STROKE_WIDTH_PX, is_actor_man_variant, write_actor_man_lifeline,
    write_collection_actor_shape, write_database_bottom_actor_shape,
    write_database_top_actor_shape, write_lifeline_root_open, write_queue_actor_shape,
    write_rect_actor_shape,
};
use super::geometry::node_left_top;
use super::model::SequenceSvgModel;
use rustc_hash::FxHashMap;

pub(super) struct SequenceActorRenderContext<'a> {
    pub(super) model: &'a SequenceSvgModel,
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) edges_by_id: &'a FxHashMap<&'a str, &'a crate::model::LayoutEdge>,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) actor_wrap_width: f64,
    pub(super) actor_height: f64,
    pub(super) label_box_height: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) actor_text_style: &'a TextStyle,
    pub(super) actor_typography: &'a crate::sequence::SequenceResolvedTypography,
    pub(super) typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) fn render_sequence_bottom_actors(
    out: &mut impl SvgOutput,
    ctx: &SequenceActorRenderContext<'_>,
) -> Result<()> {
    let label_ctx = ActorLabelContext::new(
        ctx.actor_wrap_width,
        ctx.measurer,
        ctx.actor_text_style,
        ctx.actor_typography,
        ctx.typography_receipt,
        ctx.math_sidecar,
        ctx.checkpoints,
    );

    // Mermaid draws bottom actors first (reverse DOM order).
    ctx.checkpoints.checkpoint()?;
    for (emission_index, (actor_index, actor_id)) in
        ctx.model.actor_order.iter().enumerate().rev().enumerate()
    {
        ctx.checkpoints.checkpoint_loop(emission_index)?;
        let label_ctx = label_ctx.for_actor(actor_index);
        let Some(actor) = ctx.model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        let node_id = format!("actor-bottom-{actor_id}");
        let Some(n) = ctx.nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        match actor_type {
            // Actor-man variants are drawn later (after `<defs>`), but Mermaid keeps stable
            // indices by emitting empty `<g/>` placeholders here.
            actor_type if is_actor_man_variant(actor_type) => {
                out.push_str("<g/>");
            }
            "collections" => {
                out.push_str("<g>");
                write_collection_actor_shape(out, n, actor_id, actor, "actor-bottom", &label_ctx)?;
                out.push_str("</g>");
            }
            "queue" => {
                out.push_str(r#"<g class="actor actor-bottom">"#);
                write_queue_actor_shape(out, n, actor, "actor-bottom", &label_ctx)?;
                out.push_str("</g>");
            }
            "database" => {
                out.push_str("<g>");
                write_database_bottom_actor_shape(out, n, actor, ctx.label_box_height, &label_ctx)?;
                out.push_str("</g>");
            }
            _ => {
                out.push_str("<g>");
                write_rect_actor_shape(out, n, actor_id, actor, "actor-bottom", &label_ctx)?;
                out.push_str("</g>");
            }
        }
    }
    ctx.checkpoints.checkpoint()
}

pub(super) fn render_sequence_top_actors_and_lifelines(
    out: &mut impl SvgOutput,
    ctx: &SequenceActorRenderContext<'_>,
    theme_receipt: &mut crate::sequence::SequenceLifelineThemeReceipt,
) -> crate::Result<()> {
    let label_ctx = ActorLabelContext::new(
        ctx.actor_wrap_width,
        ctx.measurer,
        ctx.actor_text_style,
        ctx.actor_typography,
        ctx.typography_receipt,
        ctx.math_sidecar,
        ctx.checkpoints,
    );

    ctx.checkpoints.checkpoint()?;
    for (emission_index, (idx, actor_id)) in
        ctx.model.actor_order.iter().enumerate().rev().enumerate()
    {
        ctx.checkpoints.checkpoint_loop(emission_index)?;
        let label_ctx = label_ctx.for_actor(idx);
        theme_receipt.record_line_candidate();
        let Some(actor) = ctx.model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        let node_top_id = format!("actor-top-{actor_id}");
        let node_bottom_id = format!("actor-bottom-{actor_id}");
        let Some(top) = ctx.nodes_by_id.get(node_top_id.as_str()).copied() else {
            continue;
        };
        let Some(bottom) = ctx.nodes_by_id.get(node_bottom_id.as_str()).copied() else {
            continue;
        };
        let (_, top_y) = node_left_top(top);
        let (_, bottom_y) = node_left_top(bottom);

        let (y1, y2) = ctx
            .edges_by_id
            .get(format!("lifeline-{actor_id}").as_str())
            .and_then(|e| Some((e.points.first()?.y, e.points.get(1)?.y)))
            .unwrap_or((top_y + top.height, bottom_y));

        match actor_type {
            actor_type if is_actor_man_variant(actor_type) => {
                write_actor_man_lifeline(out, idx, top.x, y1, y2, actor_id);
                out.checkpoint()?;
                theme_receipt.record_line_emission(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                );
            }
            "collections" => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type);
                out.checkpoint()?;
                theme_receipt.record_line_emission(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                );
                write_collection_actor_shape(out, top, actor_id, actor, "actor-top", &label_ctx)?;
                out.push_str("</g></g>");
            }
            "queue" => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type);
                out.checkpoint()?;
                theme_receipt.record_line_emission(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                );
                write_queue_actor_shape(out, top, actor, "actor-top", &label_ctx)?;
                out.push_str("</g></g>");
            }
            "database" => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type);
                out.checkpoint()?;
                theme_receipt.record_line_emission(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                );
                write_database_top_actor_shape(out, top, actor, ctx.actor_height, &label_ctx)?;
                out.push_str("</g></g>");
            }
            _ => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type);
                out.checkpoint()?;
                theme_receipt.record_line_emission(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                );
                write_rect_actor_shape(out, top, actor_id, actor, "actor-top", &label_ctx)?;
                out.push_str("</g></g>");
            }
        }
    }
    ctx.checkpoints.checkpoint()
}
