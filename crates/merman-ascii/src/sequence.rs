mod boxes;
mod control;
mod events;
mod layout;
mod model;
mod notes;
mod render;
mod text;
mod validate;

pub(crate) use model::from_sequence_model;
pub(crate) use render::render_sequence_diagram;

const BOX_PADDING_LEFT_RIGHT: usize = 2;
const MIN_BOX_WIDTH: usize = 3;
const BOX_BORDER_WIDTH: usize = 2;
const LABEL_LEFT_MARGIN: usize = 2;
const LABEL_BUFFER_SPACE: usize = 10;
const NOTE_SIDE_GAP: usize = 2;
const NOTE_WRAP_TEXT_WIDTH: usize = 24;
const SEQUENCE_BOX_CONTENT_OFFSET: usize = 1;
const SEQUENCE_BOX_LABEL_MARGIN: usize = 2;
