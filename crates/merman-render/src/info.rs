use crate::{model::InfoDiagramLayout, text::TextMeasurer, theme::FamilyTheme};
use merman_model::presentation::PresentationConfig;

pub(crate) use merman_render_structured::info::{InfoAppearance, render_info_svg};

pub(crate) fn prepare_info_diagram(
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    measurer: &dyn TextMeasurer,
) -> (InfoDiagramLayout, InfoAppearance) {
    merman_render_structured::info::prepare_info_diagram(config, native, &|text, style| {
        let horizontal = measurer.measure_svg_text_bbox_x(text, style);
        let height = measurer.measure_svg_raw_text_bbox_height_px(text, style);
        (horizontal, height)
    })
}
