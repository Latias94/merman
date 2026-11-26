//! Sankey layout, presentation binding, and SVG emission.

use merman_model::{families::sankey::SankeyDiagramModel, presentation::PresentationConfig};
use merman_render_core::{Result, resources::OperationWorkMeter};
use merman_theme_contract::v2::{Color, FamilyTheme};
use std::sync::Arc;

mod config;
mod css;
mod layout;
mod model;
mod svg;
mod theme;

pub use model::{SankeyDiagramLayout, SankeyLinkLayout, SankeyNodeLayout};
pub use svg::render_sankey_diagram_svg;
pub use theme::SankeyStyle;

/// Prepares the Sankey layout and presentation with the same operation meter.
pub fn prepare_sankey_diagram(
    model: &SankeyDiagramModel,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    canvas: Option<Color>,
    work: &Arc<OperationWorkMeter>,
) -> Result<(SankeyDiagramLayout, SankeyStyle)> {
    let layout = layout::layout_sankey_diagram_typed(model, config.as_value(), work.as_ref())?;
    let style = SankeyStyle::resolve(config, &layout, native, canvas, work)?;
    Ok((layout, style))
}

#[cfg(test)]
mod test_support {
    use merman_model::{
        MermaidConfig, OperationControl,
        presentation::PresentationConfig,
        resources::{InputResourcePolicy, ResourceProfile},
    };

    pub(super) fn presentation_config(value: serde_json::Value) -> PresentationConfig {
        PresentationConfig::from_config_controlled(
            MermaidConfig::from_value(value),
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }
}
