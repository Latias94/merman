#[cfg(test)]
use merman_render_sequence::frame::SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX;
pub(crate) use merman_render_sequence::frame::{
    SEQUENCE_FRAME_GEOM_PAD_PX, SEQUENCE_FRAME_SIDE_PAD_PX,
};

pub(crate) const SEQUENCE_MESSAGE_WRAP_PADDING_SIDES: f64 = 2.0;
pub(crate) const SEQUENCE_ACTOR_POPUP_PANEL_BASE_HEIGHT: f64 = 20.0;
pub(crate) const SEQUENCE_ACTOR_POPUP_ROW_HEIGHT: f64 = 30.0;

pub(crate) fn sequence_text_dimensions_height_px(font_size_px: f64) -> f64 {
    (font_size_px.max(1.0) * (17.0 / 16.0)).round().max(1.0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sequence_text_and_frame_constants_match_mermaid() {
        assert_eq!(super::SEQUENCE_MESSAGE_WRAP_PADDING_SIDES, 2.0);
        assert_eq!(super::SEQUENCE_ACTOR_POPUP_PANEL_BASE_HEIGHT, 20.0);
        assert_eq!(super::SEQUENCE_ACTOR_POPUP_ROW_HEIGHT, 30.0);
        assert_eq!(super::sequence_text_dimensions_height_px(16.0), 17.0);
        assert_eq!(super::sequence_text_dimensions_height_px(10.0), 11.0);
        assert_eq!(crate::text::sequence_text_line_step_px(16.0), 19.0);
        assert_eq!(super::SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX, 60.0);
        assert_eq!(super::SEQUENCE_FRAME_SIDE_PAD_PX, 11.0);
        assert_eq!(super::SEQUENCE_FRAME_GEOM_PAD_PX, 10.0);
    }
}
