//! GitGraph preparation, layout, styling, and complete SVG emission.

use merman_model::{families::git_graph::GitGraphDiagramModel, presentation::PresentationConfig};
use merman_render_core::{Result, text::TextMeasurer};
use merman_theme_contract::v2::FamilyTheme;

mod css_binding;
mod layout;
mod model;
mod native_theme;
mod svg;
mod typography;

use css_binding::GitGraphCssBinding;
use native_theme::GitGraphNativeStyle;

pub use model::{
    GitGraphArrowLayout, GitGraphBranchLayout, GitGraphCommitLayout, GitGraphDiagramLayout,
};
pub use svg::{GitGraphSvgContext, render_gitgraph_diagram_svg_model};
pub use typography::{GitGraphTypographyThemePlan, unsupported_native_font_size};

/// Prepares GitGraph's typed geometry and shared layout/paint typography.
pub fn prepare_gitgraph_diagram(
    model: &GitGraphDiagramModel,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    measurer: &dyn TextMeasurer,
) -> Result<(GitGraphDiagramLayout, GitGraphTypographyThemePlan)> {
    let typography = match native {
        Some(theme) => GitGraphTypographyThemePlan::for_native(config, theme),
        None => GitGraphTypographyThemePlan::resolve(config),
    };
    let layout =
        layout::layout_gitgraph_diagram_typed(model, config.as_value(), &typography, measurer)?;
    Ok((layout, typography))
}

#[cfg(test)]
fn test_config(value: serde_json::Value) -> PresentationConfig {
    use merman_model::{
        MermaidConfig, OperationControl,
        resources::{InputResourcePolicy, ResourceProfile},
    };

    PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(value),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}
