#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-requirement",
    feature = "diagram-swimlane"
))]
pub(crate) use merman_render_core::html::{HtmlOutputError, HtmlSanitizeSink};

pub(crate) mod label_box;
