use super::actor_shapes::ActorLabelContext;
use super::geometry::node_left_top;
use super::prelude::*;
use crate::actor::{ActorManGlyph, write_actor_man_glyph as write_glyph};
use merman_model::families::sequence::SequenceActor;

pub(super) struct ActorManGlyphPlacement<'a> {
    pub(super) footer: bool,
    pub(super) line_index: usize,
    pub(super) actor_index: usize,
    pub(super) actor_height: f64,
    pub(super) label_box_height: f64,
    pub(super) diagram_id: SvgDiagramId<'a>,
}

pub(super) fn write_actor_man_glyph(
    out: &mut impl SvgOutput,
    actor_id: &str,
    actor: &SequenceActor,
    node: &LayoutNode,
    placement: ActorManGlyphPlacement<'_>,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    let (_, actor_y) = node_left_top(node);
    let actor_type = actor.actor_type.as_str();
    let bands = label_ctx.bands(node, placement.footer);
    let classic_label_center_y = if bands.is_none() {
        label_ctx.classic_glyph_label_center_y(placement.footer)
    } else {
        None
    };
    let label_y = write_glyph(
        out,
        ActorManGlyph {
            actor_id,
            actor_type,
            cx: node.x,
            actor_y,
            width: node.width,
            row_height: node.height,
            actor_height: placement.actor_height,
            label_box_height: placement.label_box_height,
            footer: placement.footer,
            line_index: placement.line_index,
            diagram_id: placement.diagram_id,
            bands,
            classic_label_center_y,
        },
        |out| label_ctx.write_style_attr(out, actor_type, placement.actor_index),
    );
    label_ctx.write_actor_man(out, node.x, label_y, actor)?;
    out.push_str("</g>");
    out.checkpoint()
}
