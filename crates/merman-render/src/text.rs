mod flowchart_parity;
mod metrics;

pub use flowchart_parity::flowchart_html_has_inline_style_tags;
pub use line_break::split_html_br_lines;
pub(crate) use line_break::{mermaid_html_breaks_to_newlines, split_mermaid_common_br_lines};
pub use merman_render_core::text::flowchart_html_line_height_px;
pub(crate) use merman_render_core::text::html_wrap::{self, HtmlWhiteSpace};
pub use merman_render_core::text::inline_html::measure_html_with_inline_styles;
pub(crate) use merman_render_core::text::inline_html::{
    MermaidMarkdownWordType, SvgTextEntityMode, measure_html_with_authored_width,
    measure_html_with_document_root, svg_markdown_word_style,
};
use merman_render_core::text::line_break;
#[cfg(feature = "markdown-labels")]
pub use merman_render_core::text::measure_markdown_with_inline_styles;
pub use merman_render_core::text::replace_fontawesome_icons;
pub use merman_render_core::text::{
    DeterministicTextMeasurer, TextMeasurer, WrappedTextLayout, try_measure_wrapped_text,
    try_measure_wrapped_text_with_line_metrics,
};
pub(crate) use merman_render_core::text::{
    MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, measure_mermaid_text_dimensions,
    measure_mermaid_text_dimensions_with_checkpoint,
};
#[cfg(feature = "markdown-labels")]
pub(crate) use merman_render_core::text::{
    MermaidMarkdownAnalysis, analyze_mermaid_markdown, mermaid_markdown_contains_html_tags,
    mermaid_markdown_contains_raw_blocks, mermaid_markdown_is_plain_text,
    mermaid_markdown_to_html_label_fragment, mermaid_markdown_to_lines,
    mermaid_markdown_to_xhtml_label_fragment, mermaid_markdown_wants_paragraph_wrap,
};
pub(crate) use merman_render_core::text::{
    measure_svg_markdown_word_lines, measure_svg_markdown_word_lines_with_origin,
    measure_xhtml_label_fragment_with_terminal_fonts, wrap_markdown_word_lines,
};
#[cfg(feature = "markdown-labels")]
pub(crate) use merman_render_core::text::{
    measure_wrapped_markdown_with_inline_styles, mermaid_markdown_to_wrapped_word_lines,
};
#[cfg(feature = "diagram-flowchart")]
pub(crate) use metrics::flowchart_label_xhtml_for_typography_admission;
#[cfg(any(feature = "diagram-block", feature = "diagram-class"))]
pub(crate) use metrics::measure_unwrapped_xhtml_label_fragment;
#[doc(hidden)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub use metrics::unsupported_html_label_style;

pub(crate) use merman_render_core::text::{
    ParsedCssFontStack, non_markdown_svg_words, parse_css_font_stack, xhtml_font_default,
};

#[cfg(feature = "diagram-c4")]
pub(crate) use merman_render_core::text::is_css_wide_keyword;
pub(crate) use merman_render_core::text::wrap_label_like_mermaid_lines_with_checkpoint;
pub use merman_render_core::text::{
    FontCatalog, FontCatalogFingerprint, FontSource, FontSourcePolicy, FontSourcePolicyError,
    FontStack, FontStackError, FontStyle, GenericFontFamily, TextMetrics, TextStyle, WrapMode,
};
pub use merman_render_core::text::{
    ceil_to_1_64_px, round_to_1_64_px, round_to_1_64_px_ties_to_even,
    wrap_label_like_mermaid_lines, wrap_text_lines_measurer,
};
pub(crate) use merman_render_core::text::{
    is_ecmascript_whitespace, is_html_collapsible_ascii_whitespace, resolve_css_font_weight,
    trim_html_collapsible_ascii_whitespace,
};
pub(crate) use merman_render_core::text::{
    mermaid_xhtml_label_plain_text, mermaid_xhtml_label_text_content,
};
pub(crate) use merman_render_core::text::{
    svg_text_vertical_extents_px, svg_title_bbox_vertical_extents_px,
};

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
mod tests;
