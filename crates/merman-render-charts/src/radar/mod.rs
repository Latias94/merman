//! Radar layout, presentation bindings, and SVG emission.

use merman_model::{families::radar::RadarDiagramModel, presentation::PresentationConfig};
use merman_render_core::{Result, resources::OperationWorkMeter};
use merman_theme_contract::v2::FamilyTheme;

mod config;
mod css_binding;
mod layout;
mod model;
mod native_theme;
mod point_list;
mod svg;
mod theme;

pub use layout::layout_radar_diagram_typed_with_work_meter;
pub use model::{
    RadarAxisLayout, RadarCurveLayout, RadarDiagramLayout, RadarGraticuleShapeLayout,
    RadarLegendItemLayout,
};
pub use svg::{RadarSvgContext, render_radar_svg};
pub use theme::RadarStyle;

/// Prepares the chart layout and SVG appearance with the same operation meter.
///
/// `fallback_c_scale` is the renderer's shared Mermaid palette.
pub fn prepare_radar_diagram(
    model: &RadarDiagramModel,
    config: &PresentationConfig,
    native_theme: Option<&FamilyTheme>,
    fallback_c_scale: &[&str; 12],
    work_meter: &OperationWorkMeter,
) -> Result<(RadarDiagramLayout, RadarStyle)> {
    let style = RadarStyle::resolve(native_theme, config, model, fallback_c_scale);
    let layout = layout_radar_diagram_typed_with_work_meter(model, config.as_value(), work_meter)?;
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
        "hsl(240, 100%, 76.2745098039%)",
        "blue",
        "gray",
        "black",
        "orange",
        "green",
        "yellow",
        "purple",
        "brown",
        "pink",
        "white",
        "hsl(210, 100%, 76.2745098039%)",
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
