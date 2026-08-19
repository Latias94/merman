use super::{
    CssOverridePolicy, CssOverridePostprocessor, GitGraphBranchLabelBaselinePostprocessor,
    RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline, SvgPipelinePreset,
};

/// Canonical consumer-facing SVG output policy shared by presentation, bindings, and CLI exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutputPolicy {
    pub preset: SvgPipelinePreset,
    pub css_override_policy: CssOverridePolicy,
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
            pipeline.push_postprocessor_preserving_prepared_math(
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

        assert!(pipeline.preserves_prepared_math_evidence());
    }

    #[test]
    fn caller_supplied_css_still_invalidates_renderer_owned_math_evidence() {
        let pipeline = SvgOutputPolicy {
            preset: SvgPipelinePreset::ResvgSafe,
            scoped_css: Some(".merman-prepared-math-native { opacity: 0; }".to_owned()),
            ..SvgOutputPolicy::default()
        }
        .pipeline();

        assert!(!pipeline.preserves_prepared_math_evidence());
    }
}
