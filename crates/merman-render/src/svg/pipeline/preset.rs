use super::builtin::{
    attr_sanitize::sanitize_element_attributes,
    css_sanitize::sanitize_style_elements,
    foreign_object::{
        drop_switch_native_fallbacks, foreign_object_fallback_svg, strip_foreign_objects,
    },
};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvgPipelinePreset {
    /// Preserve Mermaid-like SVG output without consumer-oriented cleanup.
    #[default]
    Parity,
    /// Add best-effort SVG text fallbacks for labels rendered as `<foreignObject>`.
    ///
    /// This keeps the original `<foreignObject>` labels for browser parity, so consumers that
    /// render both native HTML labels and fallback text may display duplicate text.
    Readable,
    /// Produce output for resvg/usvg-like consumers.
    ///
    /// This starts from the readable fallback path, strips native `<foreignObject>` labels, and
    /// removes known rasterization hazards such as unsupported CSS animation constructs and invalid
    /// numeric attributes.
    ResvgSafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinSvgStage {
    ForeignObjectFallback,
    StripForeignObject,
    DropSwitchNativeFallbacks,
    SanitizeCss,
    SanitizeAttributes,
}

impl BuiltinSvgStage {
    fn apply<'a>(self, svg: Cow<'a, str>) -> Cow<'a, str> {
        match self {
            Self::ForeignObjectFallback => Cow::Owned(foreign_object_fallback_svg(&svg)),
            Self::StripForeignObject => Cow::Owned(strip_foreign_objects(&svg)),
            Self::DropSwitchNativeFallbacks => Cow::Owned(drop_switch_native_fallbacks(&svg)),
            Self::SanitizeCss => Cow::Owned(sanitize_style_elements(&svg)),
            Self::SanitizeAttributes => Cow::Owned(sanitize_element_attributes(&svg)),
        }
    }
}

pub(crate) fn builtin_stages_for_preset(preset: SvgPipelinePreset) -> &'static [BuiltinSvgStage] {
    match preset {
        SvgPipelinePreset::Parity => &[],
        SvgPipelinePreset::Readable => &[BuiltinSvgStage::ForeignObjectFallback],
        SvgPipelinePreset::ResvgSafe => &[
            BuiltinSvgStage::ForeignObjectFallback,
            BuiltinSvgStage::StripForeignObject,
            BuiltinSvgStage::DropSwitchNativeFallbacks,
            BuiltinSvgStage::SanitizeCss,
            BuiltinSvgStage::SanitizeAttributes,
        ],
    }
}

pub(crate) fn apply_preset(preset: SvgPipelinePreset, svg: &str) -> Cow<'_, str> {
    let mut current = Cow::Borrowed(svg);
    for stage in builtin_stages_for_preset(preset) {
        current = stage.apply(current);
    }
    current
}

/// Converts Mermaid-like SVG into a best-effort resvg/usvg compatible SVG string.
pub fn resvg_safe_svg(svg: &str) -> String {
    apply_preset(SvgPipelinePreset::ResvgSafe, svg).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_stage_order_is_explicit_for_presets() {
        assert_eq!(builtin_stages_for_preset(SvgPipelinePreset::Parity), &[]);
        assert_eq!(
            builtin_stages_for_preset(SvgPipelinePreset::Readable),
            &[BuiltinSvgStage::ForeignObjectFallback]
        );
        assert_eq!(
            builtin_stages_for_preset(SvgPipelinePreset::ResvgSafe),
            &[
                BuiltinSvgStage::ForeignObjectFallback,
                BuiltinSvgStage::StripForeignObject,
                BuiltinSvgStage::DropSwitchNativeFallbacks,
                BuiltinSvgStage::SanitizeCss,
                BuiltinSvgStage::SanitizeAttributes
            ]
        );
    }

    #[test]
    fn resvg_safe_function_uses_preset_stage_runner() {
        let svg = r#"<svg><style>@keyframes a{to{opacity:1}}</style><foreignObject width="10" height="10"><div><p>Hello</p></div></foreignObject><rect width="10px" height="NaN"/></svg>"#;

        assert_eq!(
            resvg_safe_svg(svg),
            apply_preset(SvgPipelinePreset::ResvgSafe, svg).into_owned()
        );
    }
}
