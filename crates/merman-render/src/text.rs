#[cfg(test)]
mod metrics;

pub use line_break::split_html_br_lines;
pub use merman_render_core::text::flowchart_html_line_height_px;
pub use merman_render_core::text::html_label_layout::flowchart_html_has_inline_style_tags;
#[cfg(test)]
use merman_render_core::text::inline_html::MermaidMarkdownWordType;
pub use merman_render_core::text::inline_html::measure_html_with_inline_styles;
use merman_render_core::text::line_break;
#[cfg(feature = "markdown-labels")]
pub use merman_render_core::text::measure_markdown_with_inline_styles;
#[cfg(all(test, feature = "markdown-labels"))]
pub(crate) use merman_render_core::text::measure_wrapped_markdown_with_inline_styles;
pub use merman_render_core::text::replace_fontawesome_icons;
pub use merman_render_core::text::{
    DeterministicTextMeasurer, TextMeasurer, WrappedTextLayout, try_measure_wrapped_text,
    try_measure_wrapped_text_with_line_metrics,
};
#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
pub(crate) use merman_render_core::text::{
    mermaid_markdown_contains_html_tags, mermaid_markdown_to_html_label_fragment,
};
#[doc(hidden)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub use merman_render_flow::typography::unsupported_html_label_style;

#[cfg(all(
    test,
    feature = "layout-elk",
    any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane"
    )
))]
pub(crate) use merman_render_core::text::parse_css_font_stack;

pub use merman_render_core::text::{
    FontCatalog, FontCatalogFingerprint, FontSource, FontSourcePolicy, FontSourcePolicyError,
    FontStack, FontStackError, FontStyle, GenericFontFamily, TextMetrics, TextStyle, WrapMode,
};
pub use merman_render_core::text::{
    ceil_to_1_64_px, round_to_1_64_px, round_to_1_64_px_ties_to_even,
    wrap_label_like_mermaid_lines, wrap_text_lines_measurer,
};
#[cfg(test)]
pub(crate) use merman_render_core::text::{
    is_html_collapsible_ascii_whitespace, resolve_css_font_weight,
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
