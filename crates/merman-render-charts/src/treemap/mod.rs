//! Treemap layout, presentation binding, and SVG emission.

use merman_model::{
    families::treemap::TreemapDiagramRenderModel, presentation::PresentationConfig,
};
use merman_render_core::{Result, resources::OperationWorkMeter};
use merman_theme_contract::v2::{Color, FamilyTheme};
use std::sync::Arc;

mod config;
mod css;
mod css_binding;
mod layout;
mod model;
mod svg;
mod theme;
mod value_format;
mod viewport;

pub use layout::{
    TREEMAP_SECTION_HEADER_HEIGHT_PX, TREEMAP_SECTION_INNER_PADDING_PX,
    layout_treemap_diagram_typed_with_work_meter,
};
pub use model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
pub use svg::render_treemap_diagram_svg;
pub use theme::TreemapStyle;
pub use value_format::format_treemap_value;
pub use viewport::treemap_view_box;

const TREEMAP_TITLE_CLASS: &str = "treemapTitle";

/// Prepares the Treemap layout and its SVG presentation with the same operation meter.
///
/// `fallback_c_scale` is the renderer's shared, static Mermaid palette.
pub fn prepare_treemap_diagram(
    model: &TreemapDiagramRenderModel,
    title: Option<&str>,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    background: Option<Color>,
    fallback_c_scale: &[&'static str; 12],
    work: &Arc<OperationWorkMeter>,
) -> Result<(TreemapDiagramLayout, TreemapStyle)> {
    let layout = layout_treemap_diagram_typed_with_work_meter(
        model,
        title,
        config.as_value(),
        work.as_ref(),
    )?;
    let style = TreemapStyle::resolve(config, &layout, native, background, fallback_c_scale, work)?;
    Ok((layout, style))
}

#[cfg(test)]
mod test_support {
    use merman_model::{
        MermaidConfig, OperationControl,
        presentation::PresentationConfig,
        resources::{InputResourcePolicy, ResourceProfile},
    };

    pub(super) const FALLBACK_C_SCALE: [&str; 12] = [
        "red", "blue", "gray", "black", "orange", "green", "yellow", "purple", "brown", "pink",
        "white", "navy",
    ];

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
