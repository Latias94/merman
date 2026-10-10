use super::builtin::{
    attr_sanitize::sanitize_element_attributes_cow_with_checkpoints,
    css_sanitize::apply_sanitize_style_elements,
    foreign_object::{
        apply_drop_switch_native_fallbacks, apply_foreign_object_fallback,
        apply_strip_foreign_objects, drop_native_duplicate_fallbacks_with_checkpoints,
    },
    id_suffix::apply_lower_id_suffix_selectors,
    prepared_math::project_prepared_math,
    quadrant_resvg_fallback::resolve_quadrant_resvg_fallbacks_with_checkpoints,
};
use super::context::{SvgPostprocessExecution, SvgPostprocessMetadata};
use super::final_validation::SvgStructureMetrics;
use crate::Result;
use crate::environment::TextMeasurementPhase;
use crate::math::{
    BROWSER_ONLY_MATH_NATIVE_UNAVAILABLE_ATTRIBUTE, PREPARED_MATH_CLASS_ATTRIBUTE,
    PreparedMathEvidenceLease,
};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvgPipelinePreset {
    /// Default SVG output for browsers and Webviews, preserving Mermaid HTML labels and styles.
    ///
    /// This does not add text overlays or apply resvg compatibility cleanup. Browser DOM
    /// admission remains the host's responsibility.
    #[default]
    Parity,
    /// Advanced text fallback for consumers that ignore `<foreignObject>` HTML labels.
    ///
    /// Keeps the original HTML and adds a best-effort SVG text overlay without the remaining
    /// resvg compatibility cleanup. Browsers render both representations and may show overlapping
    /// text. Use `Parity` for browser previews and `ResvgSafe` for resvg/usvg consumers.
    Readable,
    /// Convert and validate SVG for resvg/usvg consumers and native PNG/JPEG/PDF export.
    ///
    /// Native binary exports select this contract automatically. It is not a browser DOM-admission
    /// guarantee and is not a lossless substitute for `Readable`.
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
    PreparedMathProjection,
    ForeignObjectFallback,
    StripForeignObject,
    DropSwitchNativeFallbacks,
    SanitizeCss,
    LowerIdSuffixSelectors,
    ResolveQuadrantResvgFallbacks,
    SanitizeAttributes,
}

impl BuiltinSvgStage {
    pub(super) fn apply<'a>(
        self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        execution: SvgPostprocessExecution<'_>,
        structure: SvgStructureMetrics,
        prepared_math_evidence: Option<&PreparedMathEvidenceLease>,
    ) -> Result<Cow<'a, str>> {
        let mut checkpoint = || execution.checkpoint();
        match self {
            Self::PreparedMathProjection => {
                if !svg.contains(PREPARED_MATH_CLASS_ATTRIBUTE)
                    && !svg.contains(BROWSER_ONLY_MATH_NATIVE_UNAVAILABLE_ATTRIBUTE)
                    && prepared_math_evidence.is_none_or(PreparedMathEvidenceLease::is_empty)
                {
                    return Ok(svg);
                }
                Ok(Cow::Owned(project_prepared_math(
                    &svg,
                    metadata.family_id(),
                    prepared_math_evidence,
                )?))
            }
            Self::ForeignObjectFallback => {
                let measurer = execution.controlled_text_measurer(TextMeasurementPhase::Wrap);
                apply_foreign_object_fallback(svg, &measurer, execution, structure)
            }
            Self::StripForeignObject => apply_strip_foreign_objects(svg, checkpoint),
            Self::DropSwitchNativeFallbacks => apply_drop_switch_native_fallbacks(svg, checkpoint),
            Self::SanitizeCss => apply_sanitize_style_elements(svg, checkpoint),
            Self::LowerIdSuffixSelectors => apply_lower_id_suffix_selectors(svg, execution),
            Self::ResolveQuadrantResvgFallbacks => {
                resolve_quadrant_resvg_fallbacks_with_checkpoints(svg, metadata, &mut checkpoint)
            }
            Self::SanitizeAttributes => {
                sanitize_element_attributes_cow_with_checkpoints(svg, &mut checkpoint)
            }
        }
    }
}

pub(crate) fn builtin_stages_for_preset(preset: SvgPipelinePreset) -> &'static [BuiltinSvgStage] {
    match preset {
        SvgPipelinePreset::Parity => &[],
        SvgPipelinePreset::Readable => &[BuiltinSvgStage::ForeignObjectFallback],
        SvgPipelinePreset::ResvgSafe => &[
            BuiltinSvgStage::PreparedMathProjection,
            BuiltinSvgStage::ForeignObjectFallback,
            BuiltinSvgStage::StripForeignObject,
            BuiltinSvgStage::DropSwitchNativeFallbacks,
            BuiltinSvgStage::SanitizeCss,
            BuiltinSvgStage::LowerIdSuffixSelectors,
            BuiltinSvgStage::ResolveQuadrantResvgFallbacks,
            BuiltinSvgStage::SanitizeAttributes,
        ],
    }
}

pub(crate) fn apply_preset_cow<'a>(
    preset: SvgPipelinePreset,
    mut current: Cow<'a, str>,
    metadata: &SvgPostprocessMetadata,
    execution: SvgPostprocessExecution<'_>,
    structure: SvgStructureMetrics,
    drop_native_duplicates: bool,
    prepared_math_evidence: Option<&PreparedMathEvidenceLease>,
) -> Result<Cow<'a, str>> {
    for stage in builtin_stages_for_preset(preset) {
        execution.checkpoint()?;
        current = stage.apply(
            current,
            metadata,
            execution,
            structure,
            prepared_math_evidence,
        )?;
        execution.checkpoint()?;
        execution.preflight_svg_byte_count(current.len())?;
        if *stage == BuiltinSvgStage::ForeignObjectFallback && drop_native_duplicates {
            current = Cow::Owned(drop_native_duplicate_fallbacks_with_checkpoints(
                &current,
                &mut || execution.checkpoint(),
            )?);
            execution.checkpoint()?;
            execution.preflight_svg_byte_count(current.len())?;
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
                BuiltinSvgStage::PreparedMathProjection,
                BuiltinSvgStage::ForeignObjectFallback,
                BuiltinSvgStage::StripForeignObject,
                BuiltinSvgStage::DropSwitchNativeFallbacks,
                BuiltinSvgStage::SanitizeCss,
                BuiltinSvgStage::LowerIdSuffixSelectors,
                BuiltinSvgStage::ResolveQuadrantResvgFallbacks,
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
        let execution = SvgPostprocessExecution::new(&session);

        let output = super::super::finalize_resvg_svg(svg, &session).unwrap();
        let structure =
            super::super::final_validation::validate_well_formed_svg_with_execution(svg, execution)
                .unwrap();
        let expected = apply_preset_cow(
            SvgPipelinePreset::ResvgSafe,
            Cow::Borrowed(svg),
            &SvgPostprocessMetadata::from_svg(svg),
            execution,
            structure,
            false,
            None,
        )
        .unwrap();

        assert_eq!(output.as_str(), expected.as_ref());
    }

    #[test]
    fn resvg_safe_rejects_browser_only_prepared_math_before_foreign_object_fallback() {
        let occurrence_id = crate::math::PreparedMathOccurrenceId::indexed(
            crate::DiagramFamilyId::FLOWCHART,
            "node-label",
            0,
        );
        let evidence = PreparedMathEvidenceLease::new(
            vec![crate::math::PreparedMathExpectation::unavailable(
                occurrence_id.clone(),
                1,
            )],
            Vec::new(),
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<foreignObject width="10" height="10">"#,
                r#"<div xmlns="http://www.w3.org/1999/xhtml">"#,
                r#"<span class="merman-prepared-math" "#,
                r#"data-merman-prepared-math-native="unavailable" "#,
                r#"data-merman-prepared-math-occurrence="{}">"#,
                r#"<svg><path d="M0 0h1v1z"/></svg></span></div>"#,
                r#"</foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
        );
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let execution = SvgPostprocessExecution::new(&session);
        let metadata = SvgPostprocessMetadata::from_svg(&svg)
            .with_family_id(crate::DiagramFamilyId::FLOWCHART);
        let structure = super::super::final_validation::validate_well_formed_svg_with_execution(
            &svg, execution,
        )
        .unwrap();

        let error = apply_preset_cow(
            SvgPipelinePreset::ResvgSafe,
            Cow::Borrowed(&svg),
            &metadata,
            execution,
            structure,
            false,
            Some(&evidence),
        )
        .expect_err("browser-only prepared math must not disappear from ResvgSafe output");

        assert!(matches!(
            error,
            crate::Error::SvgPostprocess { ref pass, ref message }
                if pass == "prepared-math-projection"
                    && message.contains("native projection is unavailable")
        ));

        let error = super::super::finalize_resvg_svg(&svg, &session)
            .expect_err("raw SVG must not mint a renderer-owned math projection");
        assert!(matches!(
            error,
            crate::Error::SvgPostprocess { ref pass, ref message }
                if pass == "prepared-math-projection"
                    && message.contains("renderer-owned family metadata")
        ));
    }

    #[test]
    fn resvg_safe_finalization_is_idempotent_after_resource_closure() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><a href="https://example.com"><text>docs</text></a><image href="../secret.png"/><use href="#shape"/><defs><path id="shape" d="M0 0H1V1H0z"/><linearGradient id="paint"/></defs><style>.safe{fill:url(#paint)}.external{background:url(/tmp/image.png)}</style></svg>"##;
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
        let execution = SvgPostprocessExecution::new(&session);
        let metadata = SvgPostprocessMetadata::from_svg(svg);
        let structure =
            super::super::final_validation::validate_well_formed_svg_with_execution(svg, execution)
                .unwrap();

        let output = apply_preset_cow(
            SvgPipelinePreset::ResvgSafe,
            Cow::Borrowed(svg),
            &metadata,
            execution,
            structure,
            false,
            None,
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
    fn explicit_typed_family_metadata_enables_quadrant_resvg_fallback() {
        let svg = r#"<svg id="quadrant" aria-roledescription="quadrantChart"><g class="data-points"><g class="data-point"><circle fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)"/></g></g></svg>"#;
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let execution = SvgPostprocessExecution::new(&session);
        let metadata = SvgPostprocessMetadata::from_svg(svg)
            .with_family_id(crate::DiagramFamilyId::QUADRANT_CHART);
        let structure =
            super::super::final_validation::validate_well_formed_svg_with_execution(svg, execution)
                .unwrap();

        let out = apply_preset_cow(
            SvgPipelinePreset::ResvgSafe,
            Cow::Borrowed(svg),
            &metadata,
            execution,
            structure,
            false,
            None,
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
