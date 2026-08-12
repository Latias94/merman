mod boxes;
mod control;
mod events;
mod layout;
mod lifecycle;
mod model;
mod notes;
mod plan;
mod prepared_body;
mod render;
mod text;
mod validate;

pub(crate) use model::from_sequence_model;
pub(crate) use render::render_sequence_diagram_with_resources;

const BOX_PADDING_LEFT_RIGHT: usize = 2;
const MIN_BOX_WIDTH: usize = 3;
const BOX_BORDER_WIDTH: usize = 2;
const LABEL_LEFT_MARGIN: usize = 2;
const LABEL_BUFFER_SPACE: usize = 10;
const NOTE_SIDE_GAP: usize = 2;
const NOTE_WRAP_TEXT_WIDTH: usize = 24;
const SEQUENCE_ACTOR_WRAP_TEXT_WIDTH: usize = 12;
const SEQUENCE_BOX_WRAP_TEXT_WIDTH: usize = 12;
const SEQUENCE_BOX_CONTENT_OFFSET: usize = BOX_BORDER_WIDTH;
const SEQUENCE_BOX_LABEL_MARGIN: usize = 2;

fn projection_allocation_failed() -> crate::error::AsciiError {
    crate::error::AsciiError::AllocationFailed {
        phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}
