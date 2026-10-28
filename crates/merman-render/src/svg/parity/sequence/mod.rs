mod activation;
mod actor_effect;
mod actor_man;
mod actor_man_glyphs;
mod actor_popup;
mod actor_shapes;
mod actors;
mod block_text;
mod blocks;
mod control_paint;
mod css;
mod frames;
mod geometry;
mod interactions;
mod math_label;
mod messages;
mod model;
mod notes;
mod render;
mod root;
mod settings;
mod text_effect;

pub(super) use crate::sequence::SequenceOperationCheckpoints as SequenceEmitCheckpoints;

pub(super) use render::render_sequence_diagram_svg_model_with_config;

/// Escape a prepared text node without interpreting another entity layer.
pub(super) fn escape_prepared_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    super::util::escape_xml_raw_into(&mut escaped, text);
    escaped
}
