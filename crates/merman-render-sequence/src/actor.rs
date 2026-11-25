mod geometry;
mod glyph;

pub use geometry::{
    SEQUENCE_GLYPH_BAND_HEIGHT, SequenceActorBands, SequenceActorLabelFootprint,
    SequenceActorLabelKind, sequence_actor_lifeline_start_y, sequence_actor_popup_rect_height,
    sequence_actor_stack_height, sequence_actor_visual_height,
};
pub use glyph::{
    ActorManGlyph, LIFELINE_STROKE_WIDTH_PX, is_actor_man_variant, write_actor_man_glyph,
    write_actor_man_lifeline, write_neo_actor_shadow_attr,
};
