//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

use merman_core::{
    Engine, OperationCancelled, OperationControl, ParseOptions, resources::InputResourcePolicy,
    runtime::RuntimePolicyError,
};

use crate::operation_runner::Operation;

#[cfg(feature = "ascii")]
use merman_ascii::{AsciiError, AsciiRenderOptions, AsciiResourcePolicy};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_export::ExportError;
#[cfg(feature = "svg")]
use merman_render::{
    LayoutOptions, ResourceLimitExceeded as SvgResourceLimitExceeded,
    environment::RenderEnvironment,
    presentation::PresentationRenderPolicy,
    svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions},
};

pub use crate::operation_runner::SemanticArtifact;

/// Structured error returned by the canonical rendering facade.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    #[error("operation cancelled during {0}")]
    Cancelled(#[from] OperationCancelled),
    #[error(transparent)]
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    RuntimePolicy(#[from] RuntimePolicyError),
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[cfg(feature = "svg")]
    #[error(transparent)]
    Svg(#[from] merman_render::Error),
    #[cfg(feature = "ascii")]
    #[error(transparent)]
    Ascii(#[from] AsciiError),
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error("render target is not available in this feature configuration: {0}")]
    UnsupportedTarget(&'static str),
}

/// Transport-neutral resource rejection projected by the common facade.
///
/// Target adapters retain their richer policy types internally. Hosts can classify every
/// source, layout, output, ASCII-grid, and export quota through this stable descriptor without
/// matching backend-specific errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "resource limit `{id}` exceeded during {phase}: actual={actual} maximum={maximum} cause={cause}"
)]
#[non_exhaustive]
pub struct ResourceLimitExceeded {
    pub id: &'static str,
    pub phase: &'static str,
    pub actual: u64,
    pub maximum: u64,
    pub cause: ResourceLimitCause,
}

/// Stable facade-level reason for a resource rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimitCause {
    Ceiling,
    ArithmeticOverflow,
}

impl ResourceLimitCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ceiling => "ceiling",
            Self::ArithmeticOverflow => "arithmetic_overflow",
        }
    }
}

impl std::fmt::Display for ResourceLimitCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ResourceLimitExceeded {
    pub(crate) fn from_input(error: merman_core::resources::InputResourceLimitExceeded) -> Self {
        Self {
            id: error.limit,
            phase: error.phase.as_str(),
            actual: error.actual as u64,
            maximum: error.max as u64,
            cause: ResourceLimitCause::Ceiling,
        }
    }

    fn from_operation(error: merman_core::OperationResourceLimitExceeded) -> Self {
        Self {
            id: error.id,
            phase: error.phase.as_str(),
            actual: error.consumed.saturating_add(error.requested),
            maximum: error.limit,
            cause: ResourceLimitCause::Ceiling,
        }
    }

    #[cfg(feature = "svg")]
    fn from_svg(error: SvgResourceLimitExceeded) -> Self {
        Self {
            id: error.limit,
            phase: error.phase.as_str(),
            actual: error.actual as u64,
            maximum: error.max as u64,
            cause: match error.cause {
                merman_render::ResourceLimitCause::Ceiling => ResourceLimitCause::Ceiling,
                merman_render::ResourceLimitCause::ArithmeticOverflow => {
                    ResourceLimitCause::ArithmeticOverflow
                }
                _ => ResourceLimitCause::Ceiling,
            },
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn from_export(details: merman_export::ExportResourceLimitDetails) -> Self {
        Self {
            id: details.limit_id,
            phase: details.phase,
            actual: details.actual,
            maximum: details.max,
            cause: ResourceLimitCause::Ceiling,
        }
    }
}

#[cfg(feature = "svg")]
fn map_svg_error(error: merman_render::Error) -> RenderError {
    match error {
        merman_render::Error::Cancelled(cancelled) => RenderError::Cancelled(cancelled),
        merman_render::Error::ResourceLimitExceeded(resource) => {
            RenderError::from(ResourceLimitExceeded::from_svg(resource))
        }
        other => RenderError::Svg(other),
    }
}

#[cfg(feature = "ascii")]
fn map_ascii_error(error: AsciiError) -> RenderError {
    match error {
        AsciiError::Cancelled(cancelled) => RenderError::Cancelled(cancelled),
        AsciiError::ResourceLimitExceeded(resource) => {
            RenderError::from(ResourceLimitExceeded::from_operation(resource))
        }
        other => RenderError::Ascii(other),
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn map_export_error(error: ExportError) -> RenderError {
    if let Some(details) = error.resource_limit_details() {
        return RenderError::from(ResourceLimitExceeded::from_export(details));
    }
    match error {
        ExportError::Cancelled(cancelled) => RenderError::Cancelled(cancelled),
        other => RenderError::Export(other),
    }
}

/// Target request for the canonical facade.
///
/// The semantic target is available in every feature configuration. SVG, ASCII, and terminal
/// export variants are feature-gated leaves of the same dispatch seam.
#[derive(Debug, Clone)]
pub enum RenderTarget {
    Semantic,
    #[cfg(feature = "svg")]
    Svg(SvgRequest),
    #[cfg(feature = "ascii")]
    Ascii(AsciiRequest),
    #[cfg(feature = "png")]
    Png(PngRequest),
    #[cfg(feature = "jpeg")]
    Jpeg(JpegRequest),
    #[cfg(feature = "pdf")]
    Pdf(PdfRequest),
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub struct SvgRequest {
    pub environment: RenderEnvironment,
    pub layout: LayoutOptions,
    pub options: SvgRenderOptions,
    pub debug: SvgDebugOptions,
    pub pipeline: Option<SvgPipeline>,
    pub presentation: PresentationRenderPolicy,
}

#[cfg(feature = "svg")]
impl Default for SvgRequest {
    fn default() -> Self {
        Self {
            environment: RenderEnvironment::deterministic(),
            layout: LayoutOptions::headless_svg_defaults(),
            options: SvgRenderOptions::default(),
            debug: SvgDebugOptions::default(),
            pipeline: None,
            presentation: PresentationRenderPolicy::default(),
        }
    }
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone)]
pub struct AsciiRequest {
    pub options: AsciiRenderOptions,
    pub resources: AsciiResourcePolicy,
}

#[cfg(feature = "ascii")]
impl Default for AsciiRequest {
    fn default() -> Self {
        Self {
            options: AsciiRenderOptions::default(),
            resources: AsciiResourcePolicy::default(),
        }
    }
}

#[cfg(feature = "png")]
#[derive(Debug, Clone)]
pub struct PngRequest {
    pub svg: SvgRequest,
    pub options: merman_export::RasterOptions,
}

#[cfg(feature = "jpeg")]
#[derive(Debug, Clone)]
pub struct JpegRequest {
    pub svg: SvgRequest,
    pub options: merman_export::RasterOptions,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone)]
pub struct PdfRequest {
    pub svg: SvgRequest,
    pub options: merman_export::PdfOptions,
}

/// One source-to-target request. The control is cloneable so a host can retain a handle and cancel
/// the synchronous worker from another task or thread.
#[derive(Debug, Clone)]
pub struct RenderRequest<'a> {
    pub source: &'a str,
    pub target: RenderTarget,
    pub control: OperationControl,
    pub parse_options: ParseOptions,
    pub resources: InputResourcePolicy,
}

impl<'a> RenderRequest<'a> {
    pub fn semantic(source: &'a str, control: OperationControl) -> Self {
        Self {
            source,
            target: RenderTarget::Semantic,
            control,
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }

    #[cfg(feature = "svg")]
    pub fn svg(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self {
            source,
            target: RenderTarget::Svg(request),
            control,
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }

    #[cfg(feature = "ascii")]
    pub fn ascii(source: &'a str, control: OperationControl, request: AsciiRequest) -> Self {
        Self {
            source,
            target: RenderTarget::Ascii(request),
            control,
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }

    #[cfg(feature = "png")]
    pub fn png(source: &'a str, control: OperationControl, request: PngRequest) -> Self {
        Self {
            source,
            target: RenderTarget::Png(request),
            control,
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }

    #[cfg(feature = "jpeg")]
    pub fn jpeg(source: &'a str, control: OperationControl, request: JpegRequest) -> Self {
        Self {
            source,
            target: RenderTarget::Jpeg(request),
            control,
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }

    #[cfg(feature = "pdf")]
    pub fn pdf(source: &'a str, control: OperationControl, request: PdfRequest) -> Self {
        Self {
            source,
            target: RenderTarget::Pdf(request),
            control,
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }

    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = parse_options;
        self
    }

    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = resources;
        self
    }
}

/// Successful output from a canonical request.
#[derive(Debug)]
pub enum RenderOutput {
    Semantic(Option<SemanticArtifact>),
    #[cfg(feature = "svg")]
    Svg(Option<String>),
    #[cfg(feature = "ascii")]
    Ascii(Option<String>),
    #[cfg(feature = "png")]
    Png(Option<Vec<u8>>),
    #[cfg(feature = "jpeg")]
    Jpeg(Option<Vec<u8>>),
    #[cfg(feature = "pdf")]
    Pdf(Option<Vec<u8>>),
}

/// Long-lived renderer defaults and host-independent engine configuration.
#[derive(Debug, Clone)]
pub struct Renderer {
    engine: Engine,
    parse_options: ParseOptions,
    resources: InputResourcePolicy,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            engine: Engine::new(),
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_engine(mut self, engine: Engine) -> Self {
        self.engine = engine;
        self
    }

    pub fn with_runtime_policy(mut self, policy: merman_core::runtime::RuntimePolicy) -> Self {
        self.engine = self.engine.with_runtime_policy(policy);
        self
    }

    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = parse_options;
        self
    }

    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = resources;
        self
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn parse_options(&self) -> ParseOptions {
        self.parse_options
    }

    pub fn resource_policy(&self) -> &InputResourcePolicy {
        &self.resources
    }

    /// Executes one typed target request through the canonical operation runner.
    pub fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput, RenderError> {
        let operation = Operation::begin(
            &self.engine,
            request.source,
            request.control,
            request.resources,
        )?;
        let semantic = operation.parse_render_model(request.source, request.parse_options)?;
        let Some(semantic) = semantic else {
            return Ok(RenderOutput::empty(request.target));
        };
        semantic.render(request.target)
    }

    /// Prepares a format-neutral semantic artifact through the same runner used by `render`.
    pub fn prepare_semantic(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        self.prepare_semantic_with(source, control, self.parse_options, self.resources)
    }

    pub fn prepare_semantic_with(
        &self,
        source: &str,
        control: OperationControl,
        parse_options: ParseOptions,
        resources: InputResourcePolicy,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        let operation = Operation::begin(&self.engine, source, control, resources)?;
        operation.parse_render_model(source, parse_options)
    }
}

impl SemanticArtifact {
    /// Consumes this operation-owned semantic artifact into one typed output target.
    pub fn render(self, target: RenderTarget) -> Result<RenderOutput, RenderError> {
        match target {
            RenderTarget::Semantic => Ok(RenderOutput::Semantic(Some(self))),
            #[cfg(feature = "svg")]
            RenderTarget::Svg(request) => render_svg_target(self, request).map(RenderOutput::Svg),
            #[cfg(feature = "ascii")]
            RenderTarget::Ascii(request) => {
                render_ascii_target(self, request).map(RenderOutput::Ascii)
            }
            #[cfg(feature = "png")]
            RenderTarget::Png(request) => render_png_target(self, request).map(RenderOutput::Png),
            #[cfg(feature = "jpeg")]
            RenderTarget::Jpeg(request) => {
                render_jpeg_target(self, request).map(RenderOutput::Jpeg)
            }
            #[cfg(feature = "pdf")]
            RenderTarget::Pdf(request) => render_pdf_target(self, request).map(RenderOutput::Pdf),
        }
    }
}

impl RenderOutput {
    fn empty(target: RenderTarget) -> Self {
        match target {
            RenderTarget::Semantic => Self::Semantic(None),
            #[cfg(feature = "svg")]
            RenderTarget::Svg(_) => Self::Svg(None),
            #[cfg(feature = "ascii")]
            RenderTarget::Ascii(_) => Self::Ascii(None),
            #[cfg(feature = "png")]
            RenderTarget::Png(_) => Self::Png(None),
            #[cfg(feature = "jpeg")]
            RenderTarget::Jpeg(_) => Self::Jpeg(None),
            #[cfg(feature = "pdf")]
            RenderTarget::Pdf(_) => Self::Pdf(None),
        }
    }
}

#[cfg(feature = "svg")]
fn render_svg_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<String>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request
        .environment
        .begin_session_in_context(operation.context.clone(), operation.control.clone());
    let artifact = merman_render::family::prepare_with_render_policy(
        parsed,
        &request.layout,
        session,
        request.presentation,
    )
    .map_err(map_svg_error)?;
    let rendered = artifact
        .render_svg(&request.options, &request.debug)
        .map_err(map_svg_error)?;
    let rendered = match request.pipeline.as_ref() {
        Some(pipeline) => rendered.apply_pipeline(pipeline).map_err(map_svg_error)?,
        None => rendered,
    };
    let (svg, _, _, _) = rendered.into_parts();
    Ok(Some(svg))
}

#[cfg(feature = "svg")]
fn prepare_resvg_target(
    semantic: SemanticArtifact,
    request: &SvgRequest,
) -> Result<Option<(merman_render::svg::ResvgCompatibleSvg, Operation)>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request
        .environment
        .begin_session_in_context(operation.context.clone(), operation.control.clone());
    let artifact = merman_render::family::prepare_with_render_policy(
        parsed,
        &request.layout,
        session,
        request.presentation,
    )
    .map_err(map_svg_error)?;
    let pipeline = request
        .pipeline
        .clone()
        .unwrap_or_else(SvgPipeline::resvg_safe)
        .into_resvg_safe();
    artifact
        .render_svg(&request.options, &request.debug)
        .map_err(map_svg_error)?
        .finalize_resvg(&pipeline)
        .map(|sealed| (sealed.into_parts().0, operation))
        .map(Some)
        .map_err(map_svg_error)
}

#[cfg(feature = "ascii")]
fn render_ascii_target(
    semantic: SemanticArtifact,
    request: AsciiRequest,
) -> Result<Option<String>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    merman_ascii::render_model_with_operation(
        parsed.model(),
        &request.options,
        &operation.control,
        &operation.context,
        request.resources,
    )
    .map(Some)
    .map_err(map_ascii_error)
}

#[cfg(feature = "png")]
fn render_png_target(
    semantic: SemanticArtifact,
    request: PngRequest,
) -> Result<Option<Vec<u8>>, RenderError> {
    let Some((svg, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::svg_to_png_controlled(&svg, &request.options, operation.control)
        .map(Some)
        .map_err(map_export_error)
}

#[cfg(feature = "jpeg")]
fn render_jpeg_target(
    semantic: SemanticArtifact,
    request: JpegRequest,
) -> Result<Option<Vec<u8>>, RenderError> {
    let Some((svg, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::svg_to_jpeg_controlled(&svg, &request.options, operation.control)
        .map(Some)
        .map_err(map_export_error)
}

#[cfg(feature = "pdf")]
fn render_pdf_target(
    semantic: SemanticArtifact,
    request: PdfRequest,
) -> Result<Option<Vec<u8>>, RenderError> {
    let Some((svg, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::svg_to_pdf_controlled(&svg, &request.options, operation.control)
        .map(Some)
        .map_err(map_export_error)
}
