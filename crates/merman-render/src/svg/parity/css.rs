#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use merman_render_core::svg_css::PreparedCommonNeoCss;
#[cfg(feature = "diagram-block")]
pub(super) use merman_render_core::svg_css::write_mermaid_base_css_root_rule as write_mermaid_base_css_root_rule_to;

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
