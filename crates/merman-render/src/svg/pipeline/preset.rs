use super::builtin::{
    attr_sanitize::sanitize_element_attributes,
    css_sanitize::sanitize_style_elements,
    foreign_object::{
        drop_switch_native_fallbacks, foreign_object_fallback_svg, strip_foreign_objects,
    },
    presentation_fallback::resolve_resvg_presentation_fallbacks,
};
use super::context::SvgPostprocessMetadata;
use crate::environment::{RenderSession, TextMeasurementPhase};
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
    ResolvePresentationFallbacks,
    SanitizeAttributes,
}

impl BuiltinSvgStage {
    fn apply<'a>(
        self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Cow<'a, str> {
        match self {
            Self::ForeignObjectFallback => {
                let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
                Cow::Owned(foreign_object_fallback_svg(&svg, &measurer))
            }
            Self::StripForeignObject => Cow::Owned(strip_foreign_objects(&svg)),
            Self::DropSwitchNativeFallbacks => Cow::Owned(drop_switch_native_fallbacks(&svg)),
            Self::SanitizeCss => Cow::Owned(sanitize_style_elements(&svg)),
            Self::ResolvePresentationFallbacks => {
                resolve_resvg_presentation_fallbacks(svg, metadata)
            }
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
            BuiltinSvgStage::ResolvePresentationFallbacks,
            BuiltinSvgStage::SanitizeAttributes,
        ],
    }
}

pub(crate) fn apply_preset<'a>(
    preset: SvgPipelinePreset,
    svg: &'a str,
    metadata: &SvgPostprocessMetadata,
    session: &RenderSession,
) -> Cow<'a, str> {
    apply_preset_cow(preset, Cow::Borrowed(svg), metadata, session)
}

pub(crate) fn apply_preset_cow<'a>(
    preset: SvgPipelinePreset,
    mut current: Cow<'a, str>,
    metadata: &SvgPostprocessMetadata,
    session: &RenderSession,
) -> Cow<'a, str> {
    for stage in builtin_stages_for_preset(preset) {
        current = stage.apply(current, metadata, session);
    }
    current
}

/// Converts Mermaid-like SVG into a best-effort resvg/usvg compatible SVG string.
///
/// This source-string helper is deliberately family-agnostic. Call
/// [`super::SvgPipeline::process_to_string_with_metadata`] with an explicitly typed family context
/// when a family-specific fallback is required.
pub fn resvg_safe_svg(svg: &str, session: &RenderSession) -> String {
    let metadata = SvgPostprocessMetadata::from_svg(svg);
    apply_preset(SvgPipelinePreset::ResvgSafe, svg, &metadata, session).into_owned()
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
                BuiltinSvgStage::ResolvePresentationFallbacks,
                BuiltinSvgStage::SanitizeAttributes
            ]
        );
    }

    #[test]
    fn resvg_safe_function_uses_preset_stage_runner() {
        let svg = r#"<svg><style>@keyframes a{to{opacity:1}}</style><foreignObject width="10" height="10"><div><p>Hello</p></div></foreignObject><rect width="10px" height="NaN"/></svg>"#;
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();

        assert_eq!(
            resvg_safe_svg(svg, &session),
            apply_preset(
                SvgPipelinePreset::ResvgSafe,
                svg,
                &SvgPostprocessMetadata::from_svg(svg),
                &session,
            )
            .into_owned()
        );
    }

    #[test]
    fn direct_resvg_safe_helper_does_not_infer_family_from_root_role() {
        let svg = r#"<svg id="quadrant" aria-roledescription="quadrantChart"><g class="data-points"><g class="data-point"><circle fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)"/></g></g></svg>"#;
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();

        let out = resvg_safe_svg(svg, &session);

        assert!(!out.contains("NaN"), "{out}");
        let document = roxmltree::Document::parse(&out).expect("valid generic resvg-safe SVG");
        let point = document
            .descendants()
            .find(|node| node.has_tag_name("circle"))
            .expect("point circle");
        assert_eq!(point.attribute("fill"), None, "{out}");
        assert_eq!(point.attribute("stroke"), None, "{out}");
    }

    #[test]
    fn explicit_typed_family_metadata_enables_quadrant_presentation_fallback() {
        let svg = r#"<svg id="quadrant" aria-roledescription="quadrantChart"><g class="data-points"><g class="data-point"><circle fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)"/></g></g></svg>"#;
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let metadata = SvgPostprocessMetadata::from_svg(svg)
            .with_family_kind(crate::family::RenderFamilyKind::QuadrantChart);

        let out = apply_preset(SvgPipelinePreset::ResvgSafe, svg, &metadata, &session);

        assert!(out.contains(r##"fill="#000000""##), "{out}");
        assert!(out.contains(r#"stroke="none""#), "{out}");
    }

    #[test]
    fn direct_generic_helper_preserves_legal_fragment_paints_with_similar_words() {
        let svg = r##"<svg id="quadrant" aria-roledescription="quadrantChart"><defs><linearGradient id="undefined"/><linearGradient id="nan"/><linearGradient id="undefined-gradient"/><linearGradient id="nan-stroke"/></defs><g class="data-points"><g class="data-point"><circle fill="url(#undefined)" stroke="url(#nan)"/><circle fill="url(#undefined-gradient)" stroke="url(#nan-stroke)"/></g></g></svg>"##;
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();

        let out = resvg_safe_svg(svg, &session);

        assert!(out.contains(r##"fill="url(#undefined)""##), "{out}");
        assert!(out.contains(r##"stroke="url(#nan)""##), "{out}");
        assert!(
            out.contains(r##"fill="url(#undefined-gradient)""##),
            "{out}"
        );
        assert!(out.contains(r##"stroke="url(#nan-stroke)""##), "{out}");
    }
}
