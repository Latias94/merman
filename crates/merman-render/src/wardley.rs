mod font_size;
mod theme;

pub(crate) use font_size::WardleyFontSize;
pub(crate) use theme::{WardleyPaintBinding, WardleyStyle, WardleyTextRole};

use crate::Result;
use crate::text::{TextMeasurer, TextStyle};
use merman_model::families::wardley::WardleyDiagramModel;
use merman_render_charts::wardley::{
    WardleyAnnotationMeasurement, layout_wardley_diagram_typed as layout_typed,
};
use serde_json::Value;

pub use merman_render_charts::wardley::{
    WardleyAnnotationLayout, WardleyAnnotationPointLayout, WardleyAnnotationsBoxLayout,
    WardleyArrowDirection, WardleyArrowLayout, WardleyAxesLayout, WardleyCircleLayout,
    WardleyDiagramLayout, WardleyDominantBaseline, WardleyFlowMarkersLayout, WardleyFontWeight,
    WardleyGridLayout, WardleyLineLayout, WardleyLinkLayout, WardleyNodeLayout,
    WardleyNodeShapeLayout, WardleyNoteLayout, WardleyPipelineBoxLayout, WardleyPipelineLinkLayout,
    WardleyPointLayout, WardleyRectLayout, WardleyRotationLayout, WardleySourceOverlayLayout,
    WardleyStageLayout, WardleyTextAnchor, WardleyTextLayout, WardleyTrendLayout,
};

#[cfg(test)]
pub(crate) fn layout_wardley_diagram_typed(
    model: &WardleyDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<WardleyDiagramLayout> {
    let config = crate::test_support::presentation_config(effective_config.clone());
    let style = WardleyStyle::resolve(&config, None);
    layout_wardley_diagram_typed_with_style(
        model,
        diagram_title,
        effective_config,
        &style,
        measurer,
    )
}

pub(crate) fn layout_wardley_diagram_typed_with_style(
    model: &WardleyDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &Value,
    style: &WardleyStyle,
    measurer: &dyn TextMeasurer,
) -> Result<WardleyDiagramLayout> {
    let mut annotation_style = None;
    layout_typed(
        model,
        diagram_title,
        effective_config,
        style.layout_overrides(),
        |text, font_size, measurement| {
            let text_style = annotation_style.get_or_insert_with(|| TextStyle {
                font_family: Some(style.font_family_css().to_owned()),
                font_size,
                font_weight: style
                    .native_font_weight(WardleyTextRole::AnnotationBoxLine)
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
    )
}

#[cfg(all(test, feature = "diagram-wardley"))]
#[path = "wardley_layout_tests.rs"]
mod wardley_layout_tests;
