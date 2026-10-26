use std::sync::Arc;

use super::{OperationControl, RenderError, RenderOutput, RenderRequest, RenderTarget, Renderer};
use crate::operation_runner::{Operation, PresentationArtifact};
use merman_core::resources::{InputResourcePolicy, ModelComplexity};
use merman_core::runtime::OperationContext;
use merman_core::{ParseOptions, ParsedDiagramRender, PresentationMetadata};
#[cfg(feature = "svg")]
use merman_render::diagram_theme::DiagramTheme;

/// A successfully parsed source with frozen semantics, configuration and parse-time runtime facts.
///
/// Cloning shares immutable canonical data. Cancellation and output ledgers are never retained.
/// Parse-affecting changes require a new preparation; completed outputs own their public data.
///
/// ```compile_fail
/// let prepared = merman::PreparedSource::from_json(serde_json::json!({"type": "info"}));
/// ```
#[derive(Debug, Clone)]
pub struct PreparedSource {
    state: Arc<PreparedSourceState>,
}

#[derive(Debug)]
struct PreparedSourceState {
    parsed: ParsedDiagramRender,
    parse_context: OperationContext,
    parse_options: ParseOptions,
    source_bytes: usize,
    complexity: ModelComplexity,
    #[cfg(feature = "svg")]
    theme: Option<DiagramTheme>,
}

impl PreparedSource {
    pub(crate) fn new(
        parsed: ParsedDiagramRender,
        parse_context: OperationContext,
        parse_options: ParseOptions,
        source_bytes: usize,
        complexity: ModelComplexity,
        #[cfg(feature = "svg")] theme: Option<DiagramTheme>,
    ) -> Self {
        Self {
            state: Arc::new(PreparedSourceState {
                parsed,
                parse_context,
                parse_options,
                source_bytes,
                complexity,
                #[cfg(feature = "svg")]
                theme,
            }),
        }
    }

    pub fn metadata(&self) -> &PresentationMetadata {
        self.state.parsed.metadata()
    }
    pub fn semantic_kind(&self) -> &'static str {
        self.state.parsed.model().kind()
    }
    pub fn family_id(&self) -> Option<merman_core::DiagramFamilyId> {
        self.state.parsed.family_id()
    }
    /// Returns the real operation context used to produce the frozen semantic facts.
    pub fn parse_context(&self) -> &OperationContext {
        &self.state.parse_context
    }
    pub fn parse_options(&self) -> ParseOptions {
        self.state.parse_options
    }
    pub fn source_bytes(&self) -> usize {
        self.state.source_bytes
    }
    pub(crate) fn model_complexity(&self) -> ModelComplexity {
        self.state.complexity
    }
    #[cfg(feature = "svg")]
    pub(crate) fn theme(&self) -> Option<&DiagramTheme> {
        self.state.theme.as_ref()
    }
    pub(crate) fn parsed(&self) -> &ParsedDiagramRender {
        &self.state.parsed
    }
}

/// Parse-affecting settings for one immutable source preparation.
#[derive(Debug, Clone)]
pub struct PrepareSourceRequest<'a> {
    source: &'a str,
    control: OperationControl,
    parse_options: Option<ParseOptions>,
    resources: Option<InputResourcePolicy>,
    #[cfg(feature = "svg")]
    theme: Option<DiagramTheme>,
}

impl<'a> PrepareSourceRequest<'a> {
    pub fn new(source: &'a str, control: OperationControl) -> Self {
        Self {
            source,
            control,
            parse_options: None,
            resources: None,
            #[cfg(feature = "svg")]
            theme: None,
        }
    }
    pub fn with_parse_options(mut self, options: ParseOptions) -> Self {
        self.parse_options = Some(options);
        self
    }
    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = Some(resources);
        self
    }
    #[cfg(feature = "svg")]
    pub fn with_theme(mut self, theme: DiagramTheme) -> Self {
        self.theme = Some(theme);
        self
    }
}

impl Renderer {
    /// Parses once using current engine defaults and any explicit preparation overrides.
    pub fn prepare_source(
        &self,
        request: PrepareSourceRequest<'_>,
    ) -> Result<Option<PreparedSource>, RenderError> {
        let request = RenderRequest {
            source: request.source,
            target: RenderTarget::Presentation,
            control: request.control,
            parse_options: request.parse_options,
            resources: request.resources,
            #[cfg(feature = "svg")]
            theme: request.theme,
        };
        let (semantic, _) = self.parse_presentation_request(request)?;
        Ok(semantic.map(PresentationArtifact::into_prepared_source))
    }

    /// Renders frozen semantics under a fresh operation, using this renderer's current policy.
    ///
    /// The target's graphical input defaults apply unless this renderer has an explicit policy.
    /// The renderer's parse defaults and configuration cannot mutate an existing snapshot.
    pub fn render_prepared(
        &self,
        prepared: &PreparedSource,
        target: RenderTarget,
        control: OperationControl,
    ) -> Result<RenderOutput, RenderError> {
        let resources = if self.resources_explicit {
            self.resources
        } else {
            target.svg_input_resource_policy().unwrap_or(self.resources)
        };
        self.render_prepared_with_resource_policy(prepared, target, control, resources)
    }

    /// Renders a prepared source to SVG bytes without constructing unrequested output metadata.
    #[cfg(feature = "svg")]
    pub fn render_prepared_svg_data(
        &self,
        prepared: &PreparedSource,
        request: super::SvgRequest,
        control: OperationControl,
    ) -> Result<String, RenderError> {
        let target = RenderTarget::Svg(request);
        let resources = if self.resources_explicit {
            self.resources
        } else {
            target.svg_input_resource_policy().unwrap_or(self.resources)
        };
        let semantic = Operation::resume_prepared(
            self.presentation_engine().runtime_policy(),
            prepared,
            control,
            resources,
        )?;
        let RenderTarget::Svg(request) = semantic.admit_target(target)? else {
            unreachable!("prepared SVG retains its admitted target");
        };
        super::render_svg_data_target(semantic, request)
    }

    /// Overrides source/model admission for this output without changing its frozen parse facts.
    pub fn render_prepared_with_resource_policy(
        &self,
        prepared: &PreparedSource,
        target: RenderTarget,
        control: OperationControl,
        resources: InputResourcePolicy,
    ) -> Result<RenderOutput, RenderError> {
        Operation::resume_prepared(
            self.presentation_engine().runtime_policy(),
            prepared,
            control,
            resources,
        )?
        .render(target)
    }
}
