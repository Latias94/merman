use super::*;

#[cfg(all(test, feature = "all-diagrams"))]
use merman_render_core::svg_css::FragmentDiagramId;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
pub(crate) use merman_render_core::svg_css::PreparedCommonNeoCss;
#[cfg(feature = "diagram-block")]
pub(super) use merman_render_core::svg_css::write_mermaid_base_css_root_rule as write_mermaid_base_css_root_rule_to;

pub(super) fn write_mermaid_paragraph_css_to(
    out: &mut impl SvgOutput,
    id: impl std::fmt::Display,
) -> Result<()> {
    let _ = write!(out, "#{id} p{{margin:0;}}");
    out.checkpoint()
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
