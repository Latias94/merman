//! Wardley layout, presentation binding, and SVG emission.

use merman_model::{families::wardley::WardleyDiagramModel, presentation::PresentationConfig};
use merman_render_core::{
    Result,
    text::{TextMeasurer, TextStyle},
};
use merman_theme_contract::v2::FamilyTheme;

mod font_size;
mod layout;
mod model;
mod svg;
mod theme;

#[cfg(test)]
mod layout_tests;

pub use layout::{
    WardleyAnnotationMeasurement, WardleyLayoutFontSizes, WardleyLayoutOverrides,
    layout_wardley_diagram_typed,
};
pub use model::{
    WardleyAnnotationLayout, WardleyAnnotationPointLayout, WardleyAnnotationsBoxLayout,
    WardleyArrowDirection, WardleyArrowLayout, WardleyAxesLayout, WardleyCircleLayout,
    WardleyDiagramLayout, WardleyDominantBaseline, WardleyFlowMarkersLayout, WardleyFontWeight,
    WardleyGridLayout, WardleyLineLayout, WardleyLinkLayout, WardleyNodeLayout,
    WardleyNodeShapeLayout, WardleyNoteLayout, WardleyPipelineBoxLayout, WardleyPipelineLinkLayout,
    WardleyPointLayout, WardleyRectLayout, WardleyRotationLayout, WardleySourceOverlayLayout,
    WardleyStageLayout, WardleyTextAnchor, WardleyTextLayout, WardleyTrendLayout,
};
pub use svg::{WardleySvgContext, render_wardley_svg};
pub use theme::WardleyStyle;

/// Binds the effective Wardley styles and prepares the matching layout for this operation.
pub fn prepare_wardley_diagram(
    model: &WardleyDiagramModel,
    diagram_title: Option<&str>,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    measurer: &dyn TextMeasurer,
) -> Result<(WardleyDiagramLayout, WardleyStyle)> {
    let style = WardleyStyle::resolve(config, native);
    let mut annotation_style = None;
    let layout = layout_wardley_diagram_typed(
        model,
        diagram_title,
        config.as_value(),
        style.layout_overrides(),
        |text, font_size, measurement| {
            let text_style = annotation_style.get_or_insert_with(|| TextStyle {
                font_family: Some(style.font_family_css().to_owned()),
                font_size,
                font_weight: style
                    .native_font_weight(theme::WardleyTextRole::AnnotationBoxLine)
                    .map(|weight| weight.to_string()),
                font_style: None,
            });
            match measurement {
                WardleyAnnotationMeasurement::ComputedLength => {
                    measurer.measure_svg_text_computed_length_px(text, text_style)
                }
                WardleyAnnotationMeasurement::RawBboxHeight => {
                    measurer.measure_svg_raw_text_bbox_height_px(text, text_style)
                }
            }
        },
    )?;
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
