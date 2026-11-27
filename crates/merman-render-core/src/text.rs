//! Shared text style and measurement values.

#[cfg(all(feature = "markdown-labels", feature = "math-labels"))]
#[doc(hidden)]
pub mod create_text_html;

use serde::{Deserialize, Serialize};

#[cfg(feature = "text-layout")]
mod common_dimensions;
#[cfg(all(feature = "text-layout", feature = "svg-output"))]
pub use common_dimensions::measure_mermaid_text_dimensions_with_checkpoint;
#[cfg(feature = "text-layout")]
pub use common_dimensions::{
    MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, MermaidTextDimensions, measure_mermaid_text_dimensions,
};
#[cfg(feature = "text-layout")]
mod create_text;
mod css_font;
#[cfg(feature = "xhtml-labels")]
#[doc(hidden)]
pub mod html_label_box;
#[cfg(feature = "styled-html-labels")]
#[doc(hidden)]
pub mod html_label_layout;
#[cfg(feature = "text-layout")]
mod icons;
#[cfg(all(feature = "text-layout", feature = "svg-parse"))]
mod label_metrics;
#[cfg(all(feature = "text-layout", feature = "svg-output"))]
#[doc(hidden)]
pub mod literal_label;
#[cfg(feature = "markdown-labels")]
mod markdown;
#[cfg(feature = "markdown-labels")]
mod markdown_label;
mod measure;
#[cfg(feature = "styled-html-labels")]
#[doc(hidden)]
pub mod styled_html;
#[cfg(all(feature = "text-layout", feature = "svg-parse"))]
#[doc(hidden)]
pub mod svg_label;
#[cfg(feature = "xhtml-labels")]
mod xhtml_label;
pub use measure::TextMeasurer;
#[cfg(feature = "text-layout")]
mod deterministic;
#[cfg(feature = "text-layout")]
#[doc(hidden)]
pub mod html_wrap;
#[cfg(all(feature = "text-layout", feature = "svg-parse"))]
#[doc(hidden)]
pub mod inline_html;
#[cfg(feature = "text-layout")]
mod svg_metrics;
#[cfg(all(feature = "text-layout", feature = "svg-output"))]
mod svg_word_wrap;
#[cfg(feature = "text-layout")]
mod wrap;
#[cfg(feature = "text-layout")]
pub use deterministic::{
    DeterministicTextMeasurer, WrappedTextLayout, try_measure_wrapped_text,
    try_measure_wrapped_text_with_line_metrics,
};
#[cfg(feature = "text-layout")]
pub use line_break::split_html_br_lines;
#[cfg(feature = "text-layout")]
pub(crate) use line_break::split_mermaid_common_br_lines;
#[cfg(feature = "text-layout")]
pub use svg_metrics::{
    svg_text_vertical_extents_px, svg_title_bbox_vertical_extents_px,
    svg_wrapped_first_line_bbox_height_px,
};
#[cfg(all(feature = "text-layout", feature = "svg-output"))]
#[doc(hidden)]
pub use svg_word_wrap::flowchart_wrap_svg_source_word_lines;
#[cfg(all(feature = "text-layout", feature = "svg-output"))]
pub use wrap::wrap_label_like_mermaid_lines_with_checkpoint;
#[cfg(feature = "text-layout")]
pub use wrap::{
    ceil_to_1_64_px, round_to_1_64_px, round_to_1_64_px_ties_to_even,
    wrap_label_like_mermaid_lines, wrap_text_lines_measurer,
};
#[cfg(feature = "text-layout")]
mod css_font_stack;
#[cfg(feature = "text-layout")]
mod fonts;
#[cfg(feature = "text-layout")]
mod heuristic;
#[cfg(feature = "text-layout")]
pub mod line_break;
#[cfg(feature = "text-layout")]
#[doc(hidden)]
pub mod measurement;
mod whitespace;
#[cfg(feature = "text-layout")]
pub use create_text::non_markdown_svg_words;
pub use css_font::{FontStyle, GenericFontFamily, is_css_wide_keyword, resolve_css_font_weight};
#[cfg(feature = "text-layout")]
pub use css_font_stack::{ParsedCssFontStack, parse_css_font_stack, xhtml_font_default};
#[cfg(feature = "text-layout")]
pub use fonts::{
    FontCatalog, FontCatalogFingerprint, FontSource, FontSourcePolicy, FontSourcePolicyError,
    FontStack, FontStackError,
};
#[cfg(feature = "text-layout")]
#[doc(hidden)]
pub use heuristic::HeuristicTextWidth;
#[cfg(feature = "text-layout")]
pub use heuristic::{
    append_monospace_text_width_em, append_text_width_em, estimate_monospace_text_width_em,
    estimate_text_width_em, is_monospace_font_family,
};
#[cfg(feature = "text-layout")]
#[doc(hidden)]
pub use icons::replace_fontawesome_icons;
#[cfg(feature = "markdown-labels")]
#[doc(hidden)]
pub use label_metrics::{
    measure_markdown_with_inline_styles, measure_wrapped_markdown_with_inline_styles,
    mermaid_markdown_to_wrapped_word_lines,
};
#[cfg(all(feature = "text-layout", feature = "svg-parse"))]
#[doc(hidden)]
pub use label_metrics::{
    measure_svg_markdown_word_lines, measure_svg_markdown_word_lines_with_origin,
    wrap_markdown_word_lines,
};
#[cfg(feature = "markdown-labels")]
#[doc(hidden)]
pub use markdown::{
    MermaidMarkdownAnalysis, analyze_mermaid_markdown, mermaid_markdown_contains_html_tags,
    mermaid_markdown_is_plain_text, mermaid_markdown_svg_plain_text, mermaid_markdown_to_lines,
};
#[cfg(feature = "markdown-labels")]
#[doc(hidden)]
pub use markdown_label::{
    mermaid_markdown_contains_raw_blocks, mermaid_markdown_to_html_label_fragment,
    mermaid_markdown_to_xhtml_label_fragment, mermaid_markdown_wants_paragraph_wrap,
};
pub use whitespace::*;
#[cfg(feature = "xhtml-labels")]
#[doc(hidden)]
pub use xhtml_label::{
    measure_xhtml_label_fragment_with_terminal_fonts, mermaid_xhtml_label_plain_text,
    mermaid_xhtml_label_text_content,
};

pub fn flowchart_html_line_height_px(font_size_px: f64) -> f64 {
    font_size_px.max(0.0) * 1.5
}

pub fn sequence_text_line_step_px(font_size_px: f64) -> f64 {
    font_size_px.max(1.0) * 1.1875
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WrapMode {
    #[default]
    SvgLike,
    /// SVG `<text>` behaves as a single shaping run (no whitespace-to-`<tspan>` tokenization).
    ///
    /// Mermaid uses this behavior in some diagrams (e.g. sequence message labels), where the
    /// resulting `getBBox()` width differs measurably from per-word `<tspan>` tokenization.
    SvgLikeSingleRun,
    HtmlLike,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: Option<String>,
    pub font_size: f64,
    pub font_weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_style: Option<String>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextMetrics {
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
}

impl TextMetrics {
    /// Apply Mermaid's `labelHelper` minimum without changing the text's height or wrapping.
    /// The caller selects shapes that use `labelHelper`; explicit shape widths remain authoritative.
    pub fn with_label_min_width(
        mut self,
        label: &str,
        min_width: f64,
        explicit_width: Option<f64>,
    ) -> Self {
        if !label.is_empty() && !explicit_width.is_some_and(|width| width != 0.0) {
            self.width = self.width.max(min_width);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::TextMetrics;

    #[test]
    fn label_minimum_preserves_empty_and_explicit_width_contracts() {
        let metrics = TextMetrics {
            width: 20.0,
            height: 16.0,
            line_count: 2,
        };
        for (label, explicit_width, expected) in [
            ("X", None, 120.0),
            ("X", Some(90.0), 20.0),
            ("X", Some(0.0), 120.0),
            ("", None, 20.0),
            (" ", None, 120.0),
        ] {
            let actual = metrics.with_label_min_width(label, 120.0, explicit_width);
            assert_eq!(actual.width, expected);
            assert_eq!(actual.height, metrics.height);
            assert_eq!(actual.line_count, metrics.line_count);
        }
        assert_eq!(metrics.with_label_min_width("X", 10.0, None).width, 20.0);
    }
}
