//! Tests for the renderer-level label measurement exports.

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
pub(crate) use merman_render_core::text::inline_html::style_requests_bold_font_weight;

#[cfg(test)]
mod inline_frontend_tests;
