#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use merman_render_core::svg_css::PreparedCommonNeoCss;
#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
