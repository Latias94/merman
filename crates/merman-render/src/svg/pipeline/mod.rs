mod builtin;
mod context;
mod final_validation;
mod policy;
mod preset;

pub(crate) use builtin::GitGraphBranchLabelBaselinePostprocessor;
pub use builtin::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    RootBackgroundPostprocessor, SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor,
    ScopedCssPostprocessor, StripForeignObjectPostprocessor,
};
pub use context::{SvgPostprocessContext, SvgPostprocessMetadata};
pub use policy::SvgOutputPolicy;
pub use preset::SvgPipelinePreset;

use crate::environment::RenderSession;
use crate::resources::ResourceLimitPhase;
use crate::{Error, Result};
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

pub trait SvgPostprocessor: Send + Sync {
    fn name(&self) -> &'static str;

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>>;
}

/// SVG that has passed the terminal resvg compatibility finalizer.
///
/// The inner string cannot be constructed directly. Custom postprocessors operate on an SVG draft
/// before finalization and therefore cannot claim this type.
///
/// ```compile_fail
/// use merman_render::svg::ResvgCompatibleSvg;
///
/// let forged = ResvgCompatibleSvg { svg: "<svg/>".to_string() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResvgCompatibleSvg {
    svg: String,
}

impl ResvgCompatibleSvg {
    fn finalized(svg: String) -> Self {
        Self { svg }
    }

    pub fn as_str(&self) -> &str {
        &self.svg
    }

    pub fn into_string(self) -> String {
        self.svg
    }
}

impl AsRef<str> for ResvgCompatibleSvg {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone)]
pub struct SvgPipeline {
    preset: SvgPipelinePreset,
    postprocessors: Vec<Arc<dyn SvgPostprocessor>>,
    drop_native_duplicate_fallbacks: bool,
}

impl fmt::Debug for SvgPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self
            .postprocessors
            .iter()
            .map(|pass| pass.name())
            .collect::<Vec<_>>();

        f.debug_struct("SvgPipeline")
            .field("preset", &self.preset)
            .field("postprocessors", &names)
            .field(
                "drop_native_duplicate_fallbacks",
                &self.drop_native_duplicate_fallbacks,
            )
            .finish()
    }
}

impl Default for SvgPipeline {
    fn default() -> Self {
        Self::parity()
    }
}

impl SvgPipeline {
    pub fn parity() -> Self {
        Self::from_preset(SvgPipelinePreset::Parity)
    }

    pub fn readable() -> Self {
        Self::from_preset(SvgPipelinePreset::Readable)
    }

    pub fn resvg_safe() -> Self {
        Self::from_preset(SvgPipelinePreset::ResvgSafe)
    }

    pub fn from_preset(preset: SvgPipelinePreset) -> Self {
        Self {
            preset,
            postprocessors: Vec::new(),
            drop_native_duplicate_fallbacks: false,
        }
    }

    pub fn preset(&self) -> SvgPipelinePreset {
        self.preset
    }

    /// Keeps every configured draft transformation while replacing the terminal output contract.
    pub fn with_preset(mut self, preset: SvgPipelinePreset) -> Self {
        self.preset = preset;
        self
    }

    pub fn into_resvg_safe(self) -> Self {
        self.with_preset(SvgPipelinePreset::ResvgSafe)
    }

    pub fn with_drop_native_duplicate_fallbacks(mut self, drop: bool) -> Self {
        self.drop_native_duplicate_fallbacks = drop;
        self
    }

    pub fn with_postprocessor<P>(mut self, postprocessor: P) -> Self
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(Arc::new(postprocessor));
        self
    }

    pub fn with_shared_postprocessor(mut self, postprocessor: Arc<dyn SvgPostprocessor>) -> Self {
        self.postprocessors.push(postprocessor);
        self
    }

    pub fn push_postprocessor<P>(&mut self, postprocessor: P)
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(Arc::new(postprocessor));
    }

    pub fn process<'a>(&self, svg: &'a str, session: &RenderSession) -> Result<Cow<'a, str>> {
        let metadata = SvgPostprocessMetadata::from_svg(svg);
        self.process_with_metadata(svg, &metadata, session)
    }

    pub fn process_with_metadata<'a>(
        &self,
        svg: &'a str,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<Cow<'a, str>> {
        self.process_cow_with_metadata(Cow::Borrowed(svg), metadata, session)
    }

    fn process_cow_with_metadata<'a>(
        &self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<Cow<'a, str>> {
        let mut current = svg;
        session
            .resource_limits()
            .check_svg_bytes(current.as_ref(), ResourceLimitPhase::SvgPostprocess)?;

        for (index, postprocessor) in self.postprocessors.iter().enumerate() {
            let ctx = SvgPostprocessContext::new(
                self.preset,
                index,
                postprocessor.name(),
                metadata,
                session,
            );
            current = postprocessor
                .process(current, &ctx)
                .map_err(|err| Error::svg_postprocess(postprocessor.name(), err.to_string()))?;
            session
                .resource_limits()
                .check_svg_bytes(current.as_ref(), ResourceLimitPhase::SvgPostprocess)?;
        }

        let finalized = preset::apply_preset_cow(
            self.preset,
            current,
            metadata,
            session,
            self.drop_native_duplicate_fallbacks,
        );
        session
            .resource_limits()
            .check_svg_bytes(finalized.as_ref(), ResourceLimitPhase::SvgPostprocess)?;
        if self.preset == SvgPipelinePreset::ResvgSafe {
            final_validation::validate_resvg_compatible_svg(
                finalized.as_ref(),
                session.resource_limits(),
            )?;
        }
        Ok(finalized)
    }

    pub fn process_to_string(&self, svg: &str, session: &RenderSession) -> Result<String> {
        Ok(self.process(svg, session)?.into_owned())
    }

    pub fn process_owned_to_string(&self, svg: String, session: &RenderSession) -> Result<String> {
        let metadata = SvgPostprocessMetadata::from_svg(&svg);
        self.process_owned_to_string_with_metadata(svg, &metadata, session)
    }

    pub fn process_to_string_with_metadata(
        &self,
        svg: &str,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<String> {
        Ok(self
            .process_with_metadata(svg, metadata, session)?
            .into_owned())
    }

    pub fn process_owned_to_string_with_metadata(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<String> {
        Ok(self
            .process_cow_with_metadata(Cow::Owned(svg), metadata, session)?
            .into_owned())
    }

    pub fn process_resvg_compatible(
        &self,
        svg: &str,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        let metadata = SvgPostprocessMetadata::from_svg(svg);
        self.process_resvg_compatible_with_metadata(svg, &metadata, session)
    }

    pub fn process_resvg_compatible_with_metadata(
        &self,
        svg: &str,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        Ok(ResvgCompatibleSvg::finalized(
            self.process_to_string_with_metadata(svg, metadata, session)?,
        ))
    }

    pub fn process_owned_resvg_compatible_with_metadata(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        Ok(ResvgCompatibleSvg::finalized(
            self.process_owned_to_string_with_metadata(svg, metadata, session)?,
        ))
    }

    fn ensure_resvg_safe_contract(&self) -> Result<()> {
        if self.preset != SvgPipelinePreset::ResvgSafe {
            return Err(Error::svg_postprocess(
                "resvg-finalize",
                "ResvgCompatibleSvg requires the resvg-safe terminal preset",
            ));
        }
        Ok(())
    }
}

/// Finalizes arbitrary SVG for resvg/raster consumption without a family capability.
pub fn finalize_resvg_svg(svg: &str, session: &RenderSession) -> Result<ResvgCompatibleSvg> {
    SvgPipeline::resvg_safe().process_resvg_compatible(svg, session)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_session() -> RenderSession {
        crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn parity_pipeline_preserves_svg_exactly() {
        let svg = r#"<svg><style>@keyframes a{to{opacity:1}}</style><rect width="10"/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity().process(svg, &session).unwrap();
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, svg);
    }

    #[test]
    fn parity_pipeline_returns_owned_svg_without_reallocating() {
        let svg = String::from(r#"<svg><rect width="10"/></svg>"#);
        let allocation = svg.as_ptr();
        let session = render_session();

        let out = SvgPipeline::parity()
            .process_owned_to_string(svg, &session)
            .unwrap();

        assert_eq!(out.as_ptr(), allocation);
    }

    #[test]
    fn readable_pipeline_matches_foreign_object_fallback() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(10,20)"><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml"><p>Layer 7\nHTTP</p></div></foreignObject></g></svg>"#;
        let session = render_session();
        let measurer = session.text_measurer(crate::environment::TextMeasurementPhase::Wrap);

        let expected = super::builtin::foreign_object::foreign_object_fallback_svg(svg, &measurer);
        let out = SvgPipeline::readable()
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(out, expected);
        assert!(out.contains(">Layer 7</text>"));
        assert!(out.contains(">HTTP</text>"));
    }

    #[test]
    fn resvg_safe_pipeline_strips_generic_raster_hazards() {
        let svg = r#"<svg id="test" xmlns="http://www.w3.org/2000/svg"><style type="text/css">@keyframes bounce { 0% { transform: scale(1); } 100% { transform: scale(1.1); } } #test :root { --bg: white; } .node rect { animation: dash 1s linear; transform: rotate(45deg); fill: red; }</style><g transform="translate(undefined,NaN)"><foreignObject width="10" height="10"><div xmlns="http://www.w3.org/1999/xhtml"><p>Hello</p></div></foreignObject><rect width="10px" height="12px" stroke="" style="fill: ; stroke: #333; transform: rotate(45deg); animation: dash 1s;"/><rect width="10px" height="" fill="hsl(240, 100%, NaN%)"/></g></svg>"#;
        let session = render_session();

        let out = SvgPipeline::resvg_safe()
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes"));
        assert!(!out.contains(":root"));
        assert!(!out.contains("animation"));
        assert!(!out.contains("deg"));
        assert!(!out.contains("NaN"));
        assert!(!out.contains("undefined"));
        assert!(!out.contains(r#"height="""#));
        assert!(!out.contains(r#"fill="hsl"#));
        assert!(!out.contains(r#"stroke="""#));
        assert!(out.contains(r#"width="10""#));
        assert!(out.contains(r#"height="12""#));
        assert!(out.contains("stroke:#333"));
        assert!(out.contains(">Hello</text>"));
    }

    struct AppendPass(&'static str);

    impl SvgPostprocessor for AppendPass {
        fn name(&self) -> &'static str {
            self.0
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Ok(Cow::Owned(format!(
                "{}<!--{}:{}:{:?}:{}:{}:{}-->",
                svg,
                ctx.pass_index(),
                ctx.pass_name(),
                ctx.preset(),
                ctx.diagram_type().unwrap_or("none"),
                ctx.diagram_title().unwrap_or("none"),
                ctx.svg_id().unwrap_or("none")
            )))
        }
    }

    #[test]
    fn custom_postprocessors_run_before_builtin_finalizer_in_order() {
        let svg = r#"<svg><foreignObject width="10" height="10"><div><p>Hello</p></div></foreignObject></svg>"#;
        let pipeline = SvgPipeline::readable()
            .with_postprocessor(AppendPass("first"))
            .with_postprocessor(AppendPass("second"));
        let session = render_session();

        let out = pipeline.process_to_string(svg, &session).unwrap();

        let first = out.find("<!--0:first:Readable").unwrap();
        let second = out.find("<!--1:second:Readable").unwrap();
        assert!(first < second);
        assert!(out.contains("data-merman-foreignobject"));
    }

    #[test]
    fn custom_postprocessor_output_is_cleaned_by_resvg_finalizer() {
        struct InjectActiveContent;

        impl SvgPostprocessor for InjectActiveContent {
            fn name(&self) -> &'static str {
                "inject-active-content"
            }

            fn process<'a>(
                &self,
                svg: Cow<'a, str>,
                _ctx: &SvgPostprocessContext<'_>,
            ) -> Result<Cow<'a, str>> {
                Ok(Cow::Owned(svg.replace(
                    "</svg>",
                    r#"<script>alert(1)</script><rect animation="spin 1s"/></svg>"#,
                )))
            }
        }

        let session = render_session();
        let output = SvgPipeline::resvg_safe()
            .with_postprocessor(InjectActiveContent)
            .process_resvg_compatible("<svg></svg>", &session)
            .unwrap();

        assert!(!output.as_str().contains("script"));
        assert!(!output.as_str().contains("animation"));
    }

    #[test]
    fn expanded_draft_is_budgeted_before_terminal_xml_validation() {
        struct ExpandDraft;

        impl SvgPostprocessor for ExpandDraft {
            fn name(&self) -> &'static str {
                "expand-draft"
            }

            fn process<'a>(
                &self,
                _svg: Cow<'a, str>,
                _ctx: &SvgPostprocessContext<'_>,
            ) -> Result<Cow<'a, str>> {
                Ok(Cow::Owned(format!("<svg>{}</svg>", "x".repeat(128))))
            }
        }

        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_limits(crate::resources::RenderResourceLimits {
                max_svg_bytes: Some(64),
                ..crate::resources::RenderResourceLimits::unbounded_for_trusted_input()
            })
            .begin_session()
            .unwrap();
        let error = SvgPipeline::resvg_safe()
            .with_postprocessor(ExpandDraft)
            .process_resvg_compatible("<svg/>", &session)
            .unwrap_err();

        assert!(error.to_string().contains("max_svg_bytes"), "{error}");
    }

    #[test]
    fn non_resvg_pipeline_cannot_construct_resvg_compatible_svg() {
        let session = render_session();
        let error = SvgPipeline::parity()
            .process_resvg_compatible("<svg/>", &session)
            .unwrap_err();

        assert!(error.to_string().contains("resvg-safe terminal preset"));
    }

    #[test]
    fn custom_postprocessor_context_exposes_metadata() {
        let svg = r#"<svg id="host-diagram"><rect width="10"/></svg>"#;
        let metadata = SvgPostprocessMetadata::from_svg(svg)
            .with_diagram_type("flowchart-v2")
            .with_diagram_title("Host Diagram");
        let pipeline = SvgPipeline::parity().with_postprocessor(AppendPass("meta"));
        let session = render_session();

        let out = pipeline
            .process_to_string_with_metadata(svg, &metadata, &session)
            .unwrap();

        assert!(out.contains("<!--0:meta:Parity:flowchart-v2:Host Diagram:host-diagram-->"));
    }

    struct ErrorPass;

    impl SvgPostprocessor for ErrorPass {
        fn name(&self) -> &'static str {
            "error-pass"
        }

        fn process<'a>(
            &self,
            _svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Err(Error::InvalidModel {
                message: "boom".to_string(),
            })
        }
    }

    #[test]
    fn custom_postprocessor_errors_surface_with_pass_name() {
        let session = render_session();
        let err = SvgPipeline::parity()
            .with_postprocessor(ErrorPass)
            .process_to_string("<svg/>", &session)
            .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("error-pass"));
        assert!(message.contains("boom"));
    }
}
