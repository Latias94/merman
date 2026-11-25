use super::super::*;
use super::SequenceEmitCheckpoints;
use super::actor_shapes::{
    ActorLabelContext, ActorLifelineIdentity, actor_placement_class, write_collection_actor_shape,
    write_database_actor_shape, write_queue_actor_shape, write_rect_actor_shape,
};
use super::geometry::node_left_top;
use super::model::SequenceSvgModel;
use merman_render_sequence::actor::{is_actor_man_variant, write_actor_man_lifeline};
use rustc_hash::FxHashMap;

pub(super) struct SequenceActorRenderContext<'a> {
    pub(super) rect_style: super::actor_shapes::SequenceActorRectStyle,
    pub(super) model: &'a SequenceSvgModel,
    pub(super) diagram_id: SvgDiagramId<'a>,
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) edges_by_id: &'a FxHashMap<&'a str, &'a crate::model::LayoutEdge>,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) actor_label_footprints: &'a [Option<crate::sequence::SequenceActorLabelFootprint>],
    pub(super) actor_wrap_width: f64,
    pub(super) actor_height: f64,
    pub(super) label_box_height: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) actor_text_style: &'a TextStyle,
    pub(super) actor_typography: &'a crate::sequence::SequenceResolvedTypography,
    pub(super) lifeline_effective_stroke_width: f64,
    pub(super) compat: &'a crate::sequence::SequenceCompatBinding,
    pub(super) native: bool,
    pub(super) security_level_loose: bool,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

impl<'a> SequenceActorRenderContext<'a> {
    fn property_icon<'b>(
        &self,
        actor: &'b merman_model::families::sequence::SequenceActor,
    ) -> Option<&'b str> {
        if !self.security_level_loose {
            return None;
        }
        // Strict-like Mermaid output strips property-created use/image elements. The repaired
        // loose contract covers embedded fragments; external image resources have a separate owner.
        actor
            .properties
            .get("icon")?
            .as_str()?
            .trim()
            .strip_prefix('@')
    }

    pub(super) fn label_context(&self) -> ActorLabelContext<'a> {
        ActorLabelContext {
            wrap_width_px: self.actor_wrap_width,
            measurer: self.measurer,
            style: self.actor_text_style,
            typography: self.actor_typography,
            math_sidecar: self.math_sidecar,
            actor_label_footprints: self.actor_label_footprints,
            actor_index: None,
            compat: self.compat,
            diagram_id: self.diagram_id,
            native: self.native,
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
        let label_ctx = label_ctx.for_actor(actor_index);
        let Some(actor) = ctx.model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        let property_icon = ctx.property_icon(actor);
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
                out.push_str("<g");
                label_ctx.write_shadow_attr(out);
                out.push('>');
                write_collection_actor_shape(
                    out,
                    n,
                    actor_id,
                    actor,
                    "actor-bottom",
                    actor_index,
                    &label_ctx,
                    property_icon,
                )?;
                out.push_str("</g>");
            }
            "queue" => {
                let _ = write!(
                    out,
                    r#"<g class="{}">"#,
                    escape_attr(&actor_placement_class(actor, "actor-bottom"))
                );
                write_queue_actor_shape(out, n, actor, actor_index, &label_ctx, property_icon)?;
                out.push_str("</g>");
            }
            "database" => {
                out.push_str("<g>");
                write_database_actor_shape(
                    out,
                    n,
                    actor,
                    actor_index,
                    "actor-bottom",
                    n.width / 3.0 + ctx.label_box_height,
                    &label_ctx,
                )?;
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
                    actor_index,
                    &label_ctx,
                    ctx.rect_style,
                    property_icon,
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
    paint_bounds: Option<&mut Option<Bounds>>,
) -> crate::Result<()> {
    let label_ctx = ctx.label_context();
    let mut paint_bounds = paint_bounds;

    ctx.checkpoints.checkpoint()?;
    for (emission_index, (idx, actor_id)) in
        ctx.model.actor_order.iter().enumerate().rev().enumerate()
    {
        ctx.checkpoints.checkpoint_loop(emission_index)?;
        let label_ctx = label_ctx.for_actor(idx);
        let Some(actor) = ctx.model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        let property_icon = ctx.property_icon(actor);
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

        if let Some(bounds) = paint_bounds.as_deref_mut() {
            super::actor_effect::include_lifeline_paint_bounds(
                bounds,
                top.x,
                y1,
                y2,
                ctx.lifeline_effective_stroke_width,
            );
            out.checkpoint()?;
        }
        match actor_type {
            actor_type if is_actor_man_variant(actor_type) => {
                write_actor_man_lifeline(out, idx, top.x, y1, y2, actor_id);
                out.checkpoint()?;
            }
            "collections" => {
                label_ctx.write_lifeline_root_open(
                    out,
                    idx,
                    top.x,
                    y1,
                    y2,
                    ActorLifelineIdentity { actor_id, actor },
                );
                out.checkpoint()?;
                write_collection_actor_shape(
                    out,
                    top,
                    actor_id,
                    actor,
                    "actor-top",
                    idx,
                    &label_ctx,
                    property_icon,
                )?;
                out.push_str("</g></g>");
            }
            "queue" => {
                label_ctx.write_lifeline_root_open(
                    out,
                    idx,
                    top.x,
                    y1,
                    y2,
                    ActorLifelineIdentity { actor_id, actor },
                );
                out.checkpoint()?;
                write_queue_actor_shape(out, top, actor, idx, &label_ctx, property_icon)?;
                out.push_str("</g></g>");
            }
            "database" => {
                label_ctx.write_lifeline_root_open(
                    out,
                    idx,
                    top.x,
                    y1,
                    y2,
                    ActorLifelineIdentity { actor_id, actor },
                );
                out.checkpoint()?;
                write_database_actor_shape(
                    out,
                    top,
                    actor,
                    idx,
                    "actor-top",
                    ctx.actor_height,
                    &label_ctx,
                )?;
                out.push_str("</g></g>");
            }
            _ => {
                label_ctx.write_lifeline_root_open(
                    out,
                    idx,
                    top.x,
                    y1,
                    y2,
                    ActorLifelineIdentity { actor_id, actor },
                );
                out.checkpoint()?;
                write_rect_actor_shape(
                    out,
                    top,
                    actor_id,
                    actor,
                    "actor-top",
                    idx,
                    &label_ctx,
                    ctx.rect_style,
                    property_icon,
                )?;
                out.push_str("</g></g>");
            }
        }
        out.checkpoint()?;
    }
    ctx.checkpoints.checkpoint()
}
