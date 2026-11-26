//! Shared text style and measurement values.

use serde::{Deserialize, Serialize};

#[cfg(feature = "text-layout")]
mod create_text;
mod css_font;
mod measure;
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
use line_break::split_mermaid_common_br_lines;
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
pub use css_font::{is_css_wide_keyword, resolve_css_font_weight};
#[cfg(feature = "text-layout")]
pub use css_font_stack::{ParsedCssFontStack, parse_css_font_stack, xhtml_font_default};
#[cfg(feature = "text-layout")]
pub use fonts::{
    FontCatalog, FontCatalogFingerprint, FontSource, FontSourcePolicy, FontSourcePolicyError,
    FontStack, FontStackError, FontStyle, GenericFontFamily,
};
#[cfg(feature = "text-layout")]
pub use heuristic::{append_text_width_em, estimate_text_width_em};
pub use whitespace::*;

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
