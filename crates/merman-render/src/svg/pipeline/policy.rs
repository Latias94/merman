use super::{
    CssOverridePolicy, CssOverridePostprocessor, GitGraphBranchLabelBaselinePostprocessor,
    RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline, SvgPipelinePreset,
    SvgPostprocessor,
};

/// Canonical consumer-facing SVG output policy shared by presentation, bindings, and CLI exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutputPolicy {
    pub preset: SvgPipelinePreset,
    pub css_override_policy: CssOverridePolicy,
    /// Explicit host canvas color. `None` preserves Mermaid's unpainted root.
    pub root_background_color: Option<String>,
    pub drop_native_duplicate_fallbacks: bool,
    /// Caller styles. Native output rejects these when the SVG contains formula geometry.
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
            pipeline.push_postprocessor(GitGraphBranchLabelBaselinePostprocessor);
        }

        if let Some(color) = self
            .root_background_color
            .as_deref()
            .filter(|color| !color.trim().is_empty())
        {
            pipeline.push_postprocessor(RootBackgroundPostprocessor::new(color.trim()));
        }

        if let Some(css) = self
            .scoped_css
            .as_deref()
            .filter(|css| !css.trim().is_empty())
        {
            pipeline.push_postprocessor(NativeMathCssPostprocessor(
                ScopedCssPostprocessor::new(css.to_string())
                    .with_override_policy(self.css_override_policy),
            ));
        }

        pipeline
    }
}

/// Caller CSS cannot preserve formula paint in the native CSS engine, even through root defs.
struct NativeMathCssPostprocessor(ScopedCssPostprocessor);

impl SvgPostprocessor for NativeMathCssPostprocessor {
    fn name(&self) -> &'static str {
        "scoped-css"
    }

    fn scoped_css(&self) -> Option<&str> {
        Some(self.0.css())
    }

    fn process<'a>(
        &self,
        svg: std::borrow::Cow<'a, str>,
        ctx: &super::SvgPostprocessContext<'_>,
    ) -> crate::Result<std::borrow::Cow<'a, str>> {
        // Use the execution preset: native consumers can upgrade a parity pipeline later.
        if ctx.preset() == SvgPipelinePreset::ResvgSafe {
            use quick_xml::{Reader, XmlVersion, events::Event};

            let mut reader = Reader::from_str(svg.as_ref());
            loop {
                ctx.checkpoint()?;
                match reader.read_event() {
                    Ok(Event::Start(tag) | Event::Empty(tag))
                        if tag.local_name().as_ref() == "svg" =>
                    {
                        for attribute in tag.attributes() {
                            ctx.checkpoint()?;
                            let attribute = attribute.map_err(|error| {
                                crate::Error::svg_postprocess("scoped-css", error.to_string())
                            })?;
                            if attribute.key.as_ref() != "class" {
                                continue;
                            }
                            let value = attribute
                                .normalized_value(XmlVersion::Implicit1_0)
                                .map_err(|error| {
                                    crate::Error::svg_postprocess("scoped-css", error.to_string())
                                })?;
                            if value
                                .split_ascii_whitespace()
                                .any(|class| class == "merman-native-math-svg")
                            {
                                return Err(crate::Error::svg_postprocess(
                                    "native-math-css",
                                    "native math with scoped CSS is unsupported because formula paint cannot be preserved; omit caller CSS or export parity SVG for browser rendering",
                                ));
                            }
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(error) => {
                        return Err(crate::Error::svg_postprocess(
                            "scoped-css",
                            error.to_string(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        self.0.process(svg, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" id="diagram" width="40" height="20"><svg class="formula merman-native-math-svg" viewBox="0 0 2 1" width="20" height="10"><path d="M0 0H2V1H0Z"/></svg></svg>"#;
    const CSS: &str = "path { fill: #ff0000 !important; stroke: #00ff00 !important; stroke-width: 3px !important; }";

    #[test]
    fn native_math_with_scoped_css_is_rejected_after_preset_upgrade() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for preset in [SvgPipelinePreset::Parity, SvgPipelinePreset::ResvgSafe] {
            for override_policy in [
                CssOverridePolicy::Preserve,
                CssOverridePolicy::StripExistingImportant,
            ] {
                let pipeline = SvgOutputPolicy {
                    preset,
                    css_override_policy: override_policy,
                    scoped_css: Some(CSS.to_owned()),
                    ..Default::default()
                }
                .pipeline()
                .into_resvg_safe();
                assert!(pipeline.has_scoped_css());
                let error = pipeline
                    .process_resvg_compatible(MATH, &session)
                    .unwrap_err();
                assert!(
                    error.to_string().contains("native math with scoped CSS"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn native_math_css_guard_uses_xml_classes_and_ignores_label_text() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let pipeline = SvgOutputPolicy {
            preset: SvgPipelinePreset::ResvgSafe,
            scoped_css: Some(CSS.to_owned()),
            ..Default::default()
        }
        .pipeline();
        let encoded = MATH.replace("merman-native-math-svg", "merman-native-math-sv&#103;");
        assert!(
            pipeline
                .process_resvg_compatible(&encoded, &session)
                .is_err()
        );
        for ordinary in [
            r#"<svg id="diagram"><text>merman-native-math-svg</text></svg>"#,
            r#"<svg id="diagram"><!-- <svg class="merman-native-math-svg"/> --><path d="M0 0H2V1H0Z"/></svg>"#,
            r#"<svg id="diagram"><svg class="other-merman-native-math-svg"/></svg>"#,
        ] {
            pipeline
                .process_resvg_compatible(ordinary, &session)
                .unwrap();
        }
    }

    #[test]
    fn math_without_caller_css_and_browser_math_with_css_remain_available() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for css in [None, Some(" \n\t".to_owned())] {
            SvgOutputPolicy {
                preset: SvgPipelinePreset::ResvgSafe,
                scoped_css: css,
                ..Default::default()
            }
            .pipeline()
            .process_resvg_compatible(MATH, &session)
            .unwrap();
        }
        let browser = SvgOutputPolicy {
            scoped_css: Some(CSS.to_owned()),
            ..Default::default()
        }
        .pipeline()
        .process_to_string(MATH, &session)
        .unwrap();
        assert!(browser.contains("data-merman-postprocess=\"scoped-css\""));
    }
}
