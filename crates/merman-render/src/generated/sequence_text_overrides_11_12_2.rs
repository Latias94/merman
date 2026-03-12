// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Sequence text-layout constants in one place, so
// layout and SVG parity code do not duplicate the same note/line-height measurements inline.

pub fn sequence_note_wrap_slack_px() -> f64 {
    12.0
}

pub fn sequence_text_dimensions_height_px(font_size_px: f64) -> f64 {
    (font_size_px.max(1.0) * (17.0 / 16.0)).max(1.0)
}

pub fn sequence_text_line_step_px(font_size_px: f64) -> f64 {
    font_size_px.max(1.0) * 1.1875
}

pub fn sequence_note_text_pad_total_px() -> f64 {
    20.0
}

pub fn sequence_self_message_frame_extra_y_px() -> f64 {
    60.0
}

pub fn sequence_self_message_separator_extra_y_px() -> f64 {
    30.0
}

pub fn sequence_frame_side_pad_px() -> f64 {
    11.0
}

pub fn sequence_frame_geom_pad_px() -> f64 {
    10.0
}

pub fn sequence_self_only_frame_min_pad_left_px() -> f64 {
    5.0
}

pub fn sequence_self_only_frame_min_pad_right_px() -> f64 {
    15.0
}
