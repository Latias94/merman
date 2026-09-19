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
    pub(super) text_shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) shadow_plan: &'a super::actor_effect::SequenceActorShadowPlan,
    pub(super) shadow_evidence: &'a crate::diagram_theme::SvgShadowEvidenceRecorder,
    pub(super) rect_style: super::actor_shapes::SequenceActorRectStyle,
    pub(super) geometry_receipt: &'a crate::sequence::SequenceActorThemeReceipt,
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
    pub(super) lifeline_effective_stroke_width: f64,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

impl<'a> SequenceActorRenderContext<'a> {
    pub(super) fn label_context(&self) -> ActorLabelContext<'a> {
        ActorLabelContext {
            shadow: self.text_shadow,
            shadow_evidence: self.shadow_evidence,
            wrap_width_px: self.actor_wrap_width,
            measurer: self.measurer,
            style: self.actor_text_style,
            typography: self.actor_typography,
            typography_receipt: self.typography_receipt,
            math_sidecar: self.math_sidecar,
            actor_index: None,
            checkpoints: self.checkpoints,
        }
    }
}

pub(super) fn render_sequence_bottom_actors(
    out: &mut impl SvgOutput,
    ctx: &SequenceActorRenderContext<'_>,
) -> Result<()> {
    let label_ctx = ctx.label_context();

    // Mermaid draws bottom actors first (reverse DOM order).
    ctx.checkpoints.checkpoint()?;
    for (emission_index, (actor_index, actor_id)) in
        ctx.model.actor_order.iter().enumerate().rev().enumerate()
    {
        ctx.checkpoints.checkpoint_loop(emission_index)?;
        ctx.geometry_receipt.record_geometry_candidate();
        let label_ctx = label_ctx.for_actor(actor_index);
        let Some(actor) = ctx.model.actors.get(actor_id) else {
            continue;
        };
        if !super::actor_shapes::actor_rect_geometry_supported(actor) {
            ctx.geometry_receipt.record_unhandled_geometry();
        }
        let actor_type = actor.actor_type.as_str();
        let node_id = format!("actor-bottom-{actor_id}");
        let Some(n) = ctx.nodes_by_id.get(node_id.as_str()).copied() else {
            if !is_actor_man_variant(actor_type) {
                ctx.typography_receipt.record_missing_text_effect(
                    crate::sequence::SequenceTextSurface::ParticipantLabel,
                );
            }
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
                write_rect_actor_shape(
                    out,
                    n,
                    actor_id,
                    actor,
                    "actor-bottom",
                    &label_ctx,
                    ctx.rect_style,
                    ctx.geometry_receipt,
                    ctx.shadow_plan.terminal(n, ctx.shadow_evidence),
                )?;
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
    paint: &mut super::actor_effect::SequenceLifelinePaint,
    options: &SvgExecution<'_>,
) -> crate::Result<()> {
    let label_ctx = ctx.label_context();

    ctx.checkpoints.checkpoint()?;
    for (emission_index, (idx, actor_id)) in
        ctx.model.actor_order.iter().enumerate().rev().enumerate()
    {
        ctx.checkpoints.checkpoint_loop(emission_index)?;
        ctx.geometry_receipt.record_geometry_candidate();
        let label_ctx = label_ctx.for_actor(idx);
        theme_receipt.record_line_candidate();
        let Some(actor) = ctx.model.actors.get(actor_id) else {
            continue;
        };
        if !super::actor_shapes::actor_rect_geometry_supported(actor) {
            ctx.geometry_receipt.record_unhandled_geometry();
        }
        let actor_type = actor.actor_type.as_str();
        let node_top_id = format!("actor-top-{actor_id}");
        let node_bottom_id = format!("actor-bottom-{actor_id}");
        let Some(top) = ctx.nodes_by_id.get(node_top_id.as_str()).copied() else {
            if !is_actor_man_variant(actor_type) {
                ctx.typography_receipt.record_missing_text_effect(
                    crate::sequence::SequenceTextSurface::ParticipantLabel,
                );
            }
            continue;
        };
        let Some(bottom) = ctx.nodes_by_id.get(node_bottom_id.as_str()).copied() else {
            if !is_actor_man_variant(actor_type) {
                ctx.typography_receipt.record_missing_text_effect(
                    crate::sequence::SequenceTextSurface::ParticipantLabel,
                );
            }
            continue;
        };
        let (_, top_y) = node_left_top(top);
        let (_, bottom_y) = node_left_top(bottom);

        let (y1, y2) = ctx
            .edges_by_id
            .get(format!("lifeline-{actor_id}").as_str())
            .and_then(|e| Some((e.points.first()?.y, e.points.get(1)?.y)))
            .unwrap_or((top_y + top.height, bottom_y));

        let shadow = paint.write_definition(
            out,
            idx,
            top.x,
            y1,
            y2,
            ctx.lifeline_effective_stroke_width,
            options,
            theme_receipt,
        )?;
        let filter = shadow
            .as_ref()
            .map(|(id, _)| format!(" filter=\"url(#{})\"", escape_attr(id)))
            .unwrap_or_default();
        match actor_type {
            actor_type if is_actor_man_variant(actor_type) => {
                write_actor_man_lifeline(out, idx, top.x, y1, y2, actor_id, &filter);
                out.checkpoint()?;
                theme_receipt.record_line_emission_with_effective_width(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                    ctx.lifeline_effective_stroke_width,
                );
            }
            "collections" => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type, &filter);
                out.checkpoint()?;
                theme_receipt.record_line_emission_with_effective_width(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                    ctx.lifeline_effective_stroke_width,
                );
                write_collection_actor_shape(out, top, actor_id, actor, "actor-top", &label_ctx)?;
                out.push_str("</g></g>");
            }
            "queue" => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type, &filter);
                out.checkpoint()?;
                theme_receipt.record_line_emission_with_effective_width(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                    ctx.lifeline_effective_stroke_width,
                );
                write_queue_actor_shape(out, top, actor, "actor-top", &label_ctx)?;
                out.push_str("</g></g>");
            }
            "database" => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type, &filter);
                out.checkpoint()?;
                theme_receipt.record_line_emission_with_effective_width(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                    ctx.lifeline_effective_stroke_width,
                );
                write_database_top_actor_shape(out, top, actor, ctx.actor_height, &label_ctx)?;
                out.push_str("</g></g>");
            }
            _ => {
                write_lifeline_root_open(out, idx, top.x, y1, y2, actor_id, actor_type, &filter);
                out.checkpoint()?;
                theme_receipt.record_line_emission_with_effective_width(
                    idx,
                    top.x,
                    y1,
                    top.x,
                    y2,
                    LIFELINE_STROKE_WIDTH_PX,
                    ctx.lifeline_effective_stroke_width,
                );
                write_rect_actor_shape(
                    out,
                    top,
                    actor_id,
                    actor,
                    "actor-top",
                    &label_ctx,
                    ctx.rect_style,
                    ctx.geometry_receipt,
                    ctx.shadow_plan.terminal(top, ctx.shadow_evidence),
                )?;
                out.push_str("</g></g>");
            }
        }
        out.checkpoint()?;
        paint.record_emission(shadow.as_ref(), ctx.shadow_evidence, theme_receipt);
    }
    ctx.checkpoints.checkpoint()
}
