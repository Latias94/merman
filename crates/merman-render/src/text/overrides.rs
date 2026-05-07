//! Text override lookup boundary for generated browser/font compatibility data.

use super::WrapMode;

pub(crate) fn lookup_flowchart_markdown_italic_word_delta_em(
    wrap_mode: WrapMode,
    word: &str,
) -> Option<f64> {
    crate::generated::flowchart_text_overrides_11_12_2::
        lookup_flowchart_markdown_italic_word_delta_em(wrap_mode, word)
}

pub(crate) fn lookup_flowchart_markdown_bold_word_delta_em(
    wrap_mode: WrapMode,
    word: &str,
) -> Option<f64> {
    crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_markdown_bold_word_delta_em(
        wrap_mode, word,
    )
}

pub(crate) fn lookup_flowchart_markdown_bold_word_extra_delta_em(
    wrap_mode: WrapMode,
    word: &str,
) -> f64 {
    crate::generated::flowchart_text_overrides_11_12_2::
        lookup_flowchart_markdown_bold_word_extra_delta_em(wrap_mode, word)
}

pub(crate) fn lookup_flowchart_markdown_bold_char_extra_delta_em(
    wrap_mode: WrapMode,
    word: &str,
    ch: char,
) -> f64 {
    crate::generated::flowchart_text_overrides_11_12_2::
        lookup_flowchart_markdown_bold_char_extra_delta_em(wrap_mode, word, ch)
}

pub(crate) fn lookup_flowchart_html_width_px(
    font_key: &str,
    font_size_px: f64,
    text: &str,
) -> Option<f64> {
    crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_html_width_px(
        font_key,
        font_size_px,
        text,
    )
}

pub(crate) fn lookup_flowchart_svg_bbox_x_px(
    font_key: &str,
    font_size_px: f64,
    text: &str,
) -> Option<(f64, f64)> {
    crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_svg_bbox_x_px(
        font_key,
        font_size_px,
        text,
    )
}

pub(crate) fn lookup_er_html_width_px(font_size_px: f64, text: &str) -> Option<f64> {
    crate::generated::er_text_overrides_11_12_2::lookup_html_width_px(font_size_px, text)
}

pub(crate) fn lookup_mindmap_html_width_px(font_size_px: f64, text: &str) -> Option<f64> {
    crate::generated::mindmap_text_overrides_11_12_2::lookup_html_width_px(font_size_px, text)
}

pub(crate) fn lookup_block_html_width_px(font_size_px: f64, text: &str) -> Option<f64> {
    crate::generated::block_text_overrides_11_12_2::lookup_html_width_px(font_size_px, text)
}

pub(crate) fn lookup_sequence_svg_override_em(font_key: &str, text: &str) -> Option<(f64, f64)> {
    crate::generated::svg_overrides_sequence_11_12_2::lookup_svg_override_em(font_key, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::FLOWCHART_DEFAULT_FONT_KEY;

    #[test]
    fn generated_flowchart_markdown_override_paths_cover_repeat_offenders() {
        assert_eq!(
            lookup_flowchart_markdown_bold_word_delta_em(WrapMode::SvgLike, "Two"),
            Some(9.0 / 128.0)
        );
        assert_eq!(
            lookup_flowchart_markdown_italic_word_delta_em(WrapMode::SvgLike, "Child"),
            Some(172.0 / 2048.0)
        );
        assert_eq!(
            lookup_flowchart_markdown_italic_word_delta_em(WrapMode::HtmlLike, "Markdown"),
            Some(83.0 / 1024.0)
        );
        assert_eq!(
            lookup_flowchart_markdown_bold_word_extra_delta_em(WrapMode::SvgLike, "dog"),
            -7.0 / 16384.0
        );
        assert_eq!(
            lookup_flowchart_markdown_bold_char_extra_delta_em(WrapMode::SvgLike, "a", 'a'),
            1.0 / 1024.0
        );
        assert_eq!(
            lookup_flowchart_markdown_bold_char_extra_delta_em(WrapMode::HtmlLike, "a", 'a'),
            0.0
        );
    }

    #[test]
    fn generated_flowchart_html_override_paths_cover_promoted_leftovers() {
        assert_eq!(
            lookup_flowchart_html_width_px(FLOWCHART_DEFAULT_FONT_KEY, 16.0, "special characters"),
            Some(129.9375)
        );
        assert_eq!(
            lookup_flowchart_html_width_px("courier", 16.0, "special characters"),
            None
        );
        assert_eq!(
            lookup_flowchart_html_width_px(FLOWCHART_DEFAULT_FONT_KEY, 16.0, "Block 1"),
            None
        );
        assert_eq!(
            lookup_flowchart_html_width_px(FLOWCHART_DEFAULT_FONT_KEY, 16.0, "Line 2"),
            Some(43.34375)
        );
    }

    #[test]
    fn generated_flowchart_svg_override_paths_cover_pruned_literals() {
        assert_eq!(
            lookup_flowchart_svg_bbox_x_px(FLOWCHART_DEFAULT_FONT_KEY, 16.0, "End"),
            Some((13.1171875, 13.1171875))
        );
        assert_eq!(lookup_flowchart_svg_bbox_x_px("courier", 16.0, "End"), None);
    }
}
