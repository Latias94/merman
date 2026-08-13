use super::builtin::{
    attr_sanitize::sanitize_element_attributes_cow,
    css_sanitize::sanitize_style_elements,
    foreign_object::{
        drop_native_duplicate_fallbacks, drop_switch_native_fallbacks, foreign_object_fallback_svg,
        strip_foreign_objects,
    },
    presentation_fallback::resolve_resvg_presentation_fallbacks,
};
use super::context::SvgPostprocessMetadata;
use crate::Result;
use crate::environment::{RenderSession, TextMeasurementPhase};
use merman_core::OperationPhase;
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
    /// numeric attributes. Structural resources are limited to same-document fragments, ordinary
    /// image elements require approved, syntactically valid inline raster data URLs, and `feImage`
    /// accepts either form so a default usvg resolver cannot read files.
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
                if !svg.contains("<foreignObject") {
                    return svg;
                }
                let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
                Cow::Owned(foreign_object_fallback_svg(&svg, &measurer))
            }
            Self::StripForeignObject => {
                if !svg.contains("<foreignObject") {
                    return svg;
                }
                Cow::Owned(strip_foreign_objects(&svg))
            }
            Self::DropSwitchNativeFallbacks => {
                if !svg.contains(r#"data-merman-foreignobject-source="switch-native-fallback""#) {
                    return svg;
                }
                Cow::Owned(drop_switch_native_fallbacks(&svg))
            }
            Self::SanitizeCss => {
                if !svg.contains("<style") {
                    return svg;
                }
                Cow::Owned(sanitize_style_elements(&svg))
            }
            Self::ResolvePresentationFallbacks => {
                resolve_resvg_presentation_fallbacks(svg, metadata)
            }
            Self::SanitizeAttributes => sanitize_element_attributes_cow(svg),
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

pub(crate) fn apply_preset_cow<'a>(
    preset: SvgPipelinePreset,
    mut current: Cow<'a, str>,
    metadata: &SvgPostprocessMetadata,
    session: &RenderSession,
    drop_native_duplicates: bool,
) -> Result<Cow<'a, str>> {
    for stage in builtin_stages_for_preset(preset) {
        session.checkpoint(OperationPhase::Postprocess)?;
        current = stage.apply(current, metadata, session);
        session.checkpoint(OperationPhase::Postprocess)?;
        if *stage == BuiltinSvgStage::ForeignObjectFallback && drop_native_duplicates {
            current = Cow::Owned(drop_native_duplicate_fallbacks(&current));
            session.checkpoint(OperationPhase::Postprocess)?;
        }
    }
    Ok(current)
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
    fn resvg_safe_pipeline_uses_preset_stage_runner() {
        let svg = r#"<svg><style>@keyframes a{to{opacity:1}}</style><foreignObject width="10" height="10"><div><p>Hello</p></div></foreignObject><rect width="10px" height="NaN"/></svg>"#;
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();

        let output = super::super::finalize_resvg_svg(svg, &session).unwrap();
        let expected = apply_preset_cow(
            SvgPipelinePreset::ResvgSafe,
            Cow::Borrowed(svg),
            &SvgPostprocessMetadata::from_svg(svg),
            &session,
            false,
        )
        .unwrap();

        assert_eq!(output.as_str(), expected.as_ref());
    }

    #[test]
    fn resvg_safe_finalization_is_idempotent_after_resource_closure() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><a href="https://example.com"><text>docs</text></a><image href="../secret.png"/><use href="#shape"/><defs><path id="shape" d="M0 0H1V1H0z"/></defs><style>.safe{fill:url(#paint)}.external{background:url(/tmp/image.png)}</style></svg>"##;
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();

        let once = super::super::finalize_resvg_svg(svg, &session).unwrap();
        let twice = super::super::finalize_resvg_svg(once.as_str(), &session).unwrap();

        assert_eq!(twice, once);
        assert!(once.as_str().contains(r#"href="https://example.com""#));
        assert!(!once.as_str().contains("secret.png"));
        assert!(!once.as_str().contains("/tmp/image.png"));
    }

    #[test]
    fn resvg_safe_stages_keep_an_unchanged_svg_borrowed() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0L1 1"/></svg>"#;
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let metadata = SvgPostprocessMetadata::from_svg(svg);

        let output = apply_preset_cow(
            SvgPipelinePreset::ResvgSafe,
            Cow::Borrowed(svg),
            &metadata,
            &session,
            false,
        )
        .unwrap();

        assert!(matches!(output, Cow::Borrowed(_)));
        assert_eq!(output, svg);
    }

    #[test]
    fn direct_resvg_safe_helper_does_not_infer_family_from_root_role() {
        let svg = r#"<svg id="quadrant" aria-roledescription="quadrantChart"><g class="data-points"><g class="data-point"><circle fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)"/></g></g></svg>"#;
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();

        let out = super::super::finalize_resvg_svg(svg, &session)
            .unwrap()
            .into_string();

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
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let metadata = SvgPostprocessMetadata::from_svg(svg)
            .with_family_kind(crate::family::RenderFamilyKind::QuadrantChart);

        let out = apply_preset_cow(
            SvgPipelinePreset::ResvgSafe,
            Cow::Borrowed(svg),
            &metadata,
            &session,
            false,
        )
        .unwrap();

        assert!(out.contains(r##"fill="#000000""##), "{out}");
        assert!(out.contains(r#"stroke="none""#), "{out}");
    }

    #[test]
    fn direct_generic_helper_preserves_legal_fragment_paints_with_similar_words() {
        let svg = r##"<svg id="quadrant" aria-roledescription="quadrantChart"><defs><linearGradient id="undefined"/><linearGradient id="nan"/><linearGradient id="undefined-gradient"/><linearGradient id="nan-stroke"/></defs><g class="data-points"><g class="data-point"><circle fill="url(#undefined)" stroke="url(#nan)"/><circle fill="url(#undefined-gradient)" stroke="url(#nan-stroke)"/></g></g></svg>"##;
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();

        let out = super::super::finalize_resvg_svg(svg, &session)
            .unwrap()
            .into_string();

        assert!(out.contains(r##"fill="url(#undefined)""##), "{out}");
        assert!(out.contains(r##"stroke="url(#nan)""##), "{out}");
        assert!(
            out.contains(r##"fill="url(#undefined-gradient)""##),
            "{out}"
        );
        assert!(out.contains(r##"stroke="url(#nan-stroke)""##), "{out}");
    }
}
