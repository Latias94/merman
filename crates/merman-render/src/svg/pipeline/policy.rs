use super::{
    CssOverridePolicy, CssOverridePostprocessor, GitGraphBranchLabelBaselinePostprocessor,
    RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline, SvgPipelinePreset,
};

/// Canonical consumer-facing SVG output policy shared by presentation, bindings, and CLI exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutputPolicy {
    pub preset: SvgPipelinePreset,
    pub css_override_policy: CssOverridePolicy,
    /// Explicit host canvas color. `None` preserves Mermaid's unpainted root.
    pub root_background_color: Option<String>,
    pub drop_native_duplicate_fallbacks: bool,
    pub scoped_css: Option<String>,
}

impl Default for SvgOutputPolicy {
    fn default() -> Self {
        Self {
            preset: SvgPipelinePreset::Parity,
            css_override_policy: CssOverridePolicy::Preserve,
            root_background_color: None,
            drop_native_duplicate_fallbacks: false,
            scoped_css: None,
        }
    }
}

impl SvgOutputPolicy {
    pub fn pipeline(&self) -> SvgPipeline {
        let mut pipeline = SvgPipeline::from_preset(self.preset);

        if matches!(
            self.css_override_policy,
            CssOverridePolicy::StripExistingImportant
        ) {
            pipeline.push_postprocessor(CssOverridePostprocessor::strip_existing_important());
        }

        pipeline =
            pipeline.with_drop_native_duplicate_fallbacks(self.drop_native_duplicate_fallbacks);

        if matches!(self.preset, SvgPipelinePreset::ResvgSafe) {
            pipeline.push_family_postprocessor_preserving_prepared_math(
                crate::DiagramFamilyId::GIT_GRAPH,
                GitGraphBranchLabelBaselinePostprocessor,
            );
        }

        if let Some(color) = self
            .root_background_color
            .as_deref()
            .filter(|color| !color.trim().is_empty())
        {
            pipeline.push_postprocessor_preserving_prepared_math(RootBackgroundPostprocessor::new(
                color.trim(),
            ));
        }

        if let Some(css) = self
            .scoped_css
            .as_deref()
            .filter(|css| !css.trim().is_empty())
        {
            pipeline.push_postprocessor(
                ScopedCssPostprocessor::new(css.to_string())
                    .with_override_policy(self.css_override_policy),
            );
        }

        pipeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_resvg_passes_preserve_renderer_owned_math_evidence() {
        let pipeline = SvgOutputPolicy {
            preset: SvgPipelinePreset::ResvgSafe,
            root_background_color: Some("#111827".to_owned()),
            ..SvgOutputPolicy::default()
        }
        .pipeline();

        assert!(pipeline.preserves_prepared_math_evidence(None));
    }

    #[test]
    fn caller_supplied_css_still_invalidates_renderer_owned_math_evidence() {
        let pipeline = SvgOutputPolicy {
            preset: SvgPipelinePreset::ResvgSafe,
            scoped_css: Some(".merman-prepared-math-native { opacity: 0; }".to_owned()),
            ..SvgOutputPolicy::default()
        }
        .pipeline();

        assert!(!pipeline.preserves_prepared_math_evidence(None));
    }

    #[test]
    fn canonical_gitgraph_pass_only_invalidates_its_own_family() {
        use crate::DiagramFamilyId;
        let pipeline = SvgOutputPolicy {
            preset: SvgPipelinePreset::ResvgSafe,
            ..SvgOutputPolicy::default()
        }
        .pipeline();
        for family in DiagramFamilyId::all()
            .iter()
            .copied()
            .map(Some)
            .chain([None])
        {
            let preserves = family.is_some_and(|family| family != DiagramFamilyId::GIT_GRAPH);
            assert_eq!(pipeline.preserves_typed_theme_evidence(family), preserves);
            assert_eq!(pipeline.preserves_prepared_text_evidence(family), preserves);
            assert!(pipeline.preserves_prepared_math_evidence(family));
        }
    }

    #[test]
    fn family_scoping_does_not_exempt_global_or_public_postprocessors() {
        let background = SvgOutputPolicy {
            preset: SvgPipelinePreset::ResvgSafe,
            root_background_color: Some("#111827".to_owned()),
            ..SvgOutputPolicy::default()
        }
        .pipeline();
        let family = Some(crate::DiagramFamilyId::SEQUENCE);
        assert!(!background.preserves_typed_theme_evidence(family));
        assert!(!background.preserves_prepared_text_evidence(family));
        let custom =
            SvgPipeline::resvg_safe().with_postprocessor(GitGraphBranchLabelBaselinePostprocessor);
        assert!(!custom.preserves_typed_theme_evidence(family));
        assert!(!custom.preserves_prepared_text_evidence(family));
        assert!(!custom.preserves_prepared_math_evidence(family));
    }
}
