#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-requirement",
    feature = "diagram-swimlane"
))]
pub(crate) use merman_render_core::html::{HtmlOutputError, HtmlSanitizeSink};
pub(crate) use merman_render_core::text::html_label_box as label_box;
