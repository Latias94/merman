use super::builtin::{
    attr_sanitize::sanitize_element_attributes,
    css_sanitize::sanitize_style_elements,
    foreign_object::{foreign_object_fallback_svg, strip_foreign_objects},
};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvgPipelinePreset {
    #[default]
    Parity,
    Readable,
    ResvgSafe,
}

pub(crate) fn apply_preset(preset: SvgPipelinePreset, svg: &str) -> Cow<'_, str> {
    match preset {
        SvgPipelinePreset::Parity => Cow::Borrowed(svg),
        SvgPipelinePreset::Readable => Cow::Owned(foreign_object_fallback_svg(svg)),
        SvgPipelinePreset::ResvgSafe => Cow::Owned(resvg_safe_svg(svg)),
    }
}

pub fn resvg_safe_svg(svg: &str) -> String {
    let svg = foreign_object_fallback_svg(svg);
    let svg = strip_foreign_objects(&svg);
    let svg = sanitize_style_elements(&svg);
    sanitize_element_attributes(&svg)
}
