use merman_render_core::text::{
    inline_html::{MermaidMarkdownWordType, SvgTextEntityMode},
    mermaid_markdown_to_lines,
};
use unicode_segmentation::UnicodeSegmentation;

pub fn zero_width_lane_title_rows(title: &str) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
    let title = title
        .strip_prefix('`')
        .and_then(|title| title.strip_suffix('`'))
        .unwrap_or(title);
    mermaid_markdown_to_lines(title, true)
        .into_iter()
        .flat_map(|line| {
            if line.is_empty() {
                return vec![Vec::new()];
            }
            line.into_iter()
                .flat_map(|(word, style)| {
                    let visible = SvgTextEntityMode::CreateTextSource.visible_word(&word);
                    visible
                        .graphemes(true)
                        .map(|grapheme| vec![(grapheme.to_string(), style)])
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}
