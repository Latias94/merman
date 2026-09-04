//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

use crate::operation_runner::Operation;
#[cfg(feature = "svg")]
use crate::operation_runner::OperationExecution;
use crate::{TerminalDiagnostic, TerminalRuntimePolicyError};
#[cfg(feature = "ascii")]
use merman_core::OperationPhase;
use merman_core::{
    Engine, OperationCancelled, OperationControl, OperationResourceDomain,
    OperationResourceOverride, OperationResourceProvenance, ParseOptions,
    resources::InputResourcePolicy,
};

#[cfg(feature = "ascii")]
use merman_ascii::{
    AsciiError, AsciiOutput, AsciiRenderOptions, AsciiResourcePolicy, AsciiViewportPolicy,
};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_export::ExportError;
#[cfg(feature = "svg")]
mod document;
#[cfg(feature = "svg")]
mod environment;
#[cfg(feature = "svg")]
mod evidence;
#[cfg(feature = "svg")]
mod target_admission;

#[cfg(feature = "jpeg")]
pub use document::PreparedJpegExport;
#[cfg(feature = "png")]
pub use document::PreparedPngExport;
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use document::RasterOutput;
#[cfg(all(feature = "png", feature = "internal-theme-acceptance"))]
pub(crate) use document::ThemeRoutePngCutoverPair;
#[cfg(feature = "svg")]
use document::finish_standalone_svg_target;
#[cfg(feature = "pdf")]
pub use document::{PdfOutput, PreparedPdfExport};
#[cfg(feature = "svg")]
pub use document::{RenderedDocument, SvgOutput};
#[cfg(feature = "svg")]
pub use environment::SvgEnvironment;
#[cfg(feature = "svg")]
pub use evidence::{RenderEvidence, ThemeEvidenceStatus, ThemeEvidenceSummary};

/// Identifies the canonical facade path that produced a completed render artifact.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OperationExecutionPath {
    Renderer,
}

#[cfg(feature = "svg")]
impl OperationExecutionPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Renderer => "renderer",
        }
    }
}
#[cfg(all(feature = "svg", feature = "internal-theme-acceptance"))]
pub(crate) use evidence::{ThemeAcceptanceEvidenceProjection, ThemeEvidenceScopeProjection};
#[cfg(feature = "svg")]
use merman_render::{
    LayoutOptions, ResourceLimitExceeded as SvgResourceLimitExceeded,
    diagram_theme::DiagramTheme,
    svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions},
};
#[cfg(feature = "svg")]
pub use target_admission::{
    DocumentPortabilityReport, RenderArtifactKind, TargetAdmissionError, TargetAdmissionReason,
    TargetAdmissionReceipt, TargetAdmissionStatus, TargetFontSource,
};

pub use crate::operation_runner::SemanticArtifact;

/// Successful SVG layout inspection output.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq)]
pub struct SvgLayoutOutput {
    layout: serde_json::Value,
    gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
}

#[cfg(feature = "svg")]
impl SvgLayoutOutput {
    fn new(
        layout: serde_json::Value,
        gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) -> Self {
        Self {
            layout,
            gantt_time_axis,
        }
    }

    pub fn layout(&self) -> &serde_json::Value {
        &self.layout
    }

    pub fn gantt_time_axis_diagnostics(
        &self,
    ) -> Option<merman_render::family::GanttTimeAxisDiagnostics> {
        self.gantt_time_axis
    }

    pub fn into_parts(
        self,
    ) -> (
        serde_json::Value,
        Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) {
        (self.layout, self.gantt_time_axis)
    }
}

/// Structured error returned by the canonical rendering facade.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    #[error("no Mermaid diagram detected")]
    NoDiagram,
    #[error(transparent)]
    Cancelled(#[from] OperationCancelled),
    #[error(transparent)]
    Parse(#[from] TerminalDiagnostic),
    #[error(transparent)]
    RuntimePolicy(#[from] TerminalRuntimePolicyError),
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[cfg(feature = "svg")]
    #[error(transparent)]
    Svg(merman_render::Error),
    #[cfg(feature = "svg")]
    #[error(transparent)]
    SvgEnvironment(merman_render::environment::RenderEnvironmentError),
    #[cfg(feature = "svg")]
    #[error(transparent)]
    TargetAdmission(#[from] TargetAdmissionError),
    #[cfg(feature = "svg")]
    #[error(
        "render target `{target}` does not support RequirePortable because it has no target portability admission"
    )]
    PortabilityUnavailableForTarget { target: &'static str },
    #[cfg(feature = "ascii")]
    #[error(transparent)]
    Ascii(AsciiError),
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[error(transparent)]
    Export(ExportError),
    #[error("render target is not available in this feature configuration: {0}")]
    UnsupportedTarget(&'static str),
}

impl From<merman_core::Error> for RenderError {
    fn from(error: merman_core::Error) -> Self {
        match error {
            merman_core::Error::OperationCancelled(error) => Self::Cancelled(error),
            merman_core::Error::RuntimePolicy(error) => {
                Self::RuntimePolicy(TerminalRuntimePolicyError::from(error))
            }
            error => Self::Parse(TerminalDiagnostic::from(error)),
        }
    }
}

impl From<merman_core::runtime::RuntimePolicyError> for RenderError {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self::RuntimePolicy(TerminalRuntimePolicyError::from(error))
    }
}

#[cfg(feature = "svg")]
impl From<merman_render::Error> for RenderError {
    fn from(error: merman_render::Error) -> Self {
        match error {
            merman_render::Error::Cancelled(cancelled) => Self::Cancelled(cancelled),
            merman_render::Error::ResourceLimitExceeded(resource) => {
                Self::from(ResourceLimitExceeded::from(resource))
            }
            merman_render::Error::OperationResourceTerminal(error) => {
                crate::operation_runner::operation_terminal_error(error)
            }
            merman_render::Error::ThemeResourceLimitExceeded(resource) => {
                Self::from(ResourceLimitExceeded::from_theme(resource))
            }
            other => Self::Svg(other),
        }
    }
}

#[cfg(feature = "svg")]
fn map_svg_error(error: merman_render::Error) -> RenderError {
    RenderError::from(error)
}

/// Transport-neutral resource rejection projected by the common facade.
///
/// Target adapters retain their richer policy types internally. Hosts can classify every
/// source, layout, output, ASCII-grid, and export quota through this stable descriptor without
/// matching backend-specific errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
    pub provenance: Option<OperationResourceProvenance>,
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
            provenance: Some(OperationResourceProvenance::new(
                OperationResourceDomain::Input,
                Some(error.profile),
                error
                    .explicit_overrides
                    .into_iter()
                    .map(|override_| OperationResourceOverride {
                        id: override_.id.as_str(),
                        value: override_.value as u64,
                    }),
            )),
        }
    }

    #[cfg(feature = "ascii")]
    fn from_ascii(error: merman_ascii::AsciiResourceLimitExceeded) -> Self {
        Self {
            id: error.limit.as_str(),
            phase: error.phase().as_str(),
            actual: error.actual as u64,
            maximum: error.max as u64,
            cause: match error.cause {
                merman_ascii::AsciiResourceLimitCause::Ceiling => ResourceLimitCause::Ceiling,
                merman_ascii::AsciiResourceLimitCause::ArithmeticOverflow => {
                    ResourceLimitCause::ArithmeticOverflow
                }
                _ => ResourceLimitCause::Ceiling,
            },
            provenance: None,
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
            provenance: Some(OperationResourceProvenance::new(
                OperationResourceDomain::Render,
                Some(error.profile),
                error
                    .explicit_overrides
                    .into_iter()
                    .map(|override_| OperationResourceOverride {
                        id: override_.id.as_str(),
                        value: override_.value as u64,
                    }),
            )),
        }
    }

    #[cfg(feature = "svg")]
    fn from_theme(error: merman_render::diagram_theme::ThemeResourceLimitExceeded) -> Self {
        Self {
            id: error.limit,
            phase: error.phase.as_str(),
            actual: u64::try_from(error.actual).unwrap_or(u64::MAX),
            maximum: u64::try_from(error.max).unwrap_or(u64::MAX),
            cause: ResourceLimitCause::Ceiling,
            provenance: None,
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn from_export(
        details: merman_export::ExportResourceLimitDetails,
        provenance: OperationResourceProvenance,
    ) -> Self {
        Self {
            id: details.limit_id,
            phase: details.phase,
            actual: details.actual,
            maximum: details.max,
            cause: match details.cause {
                merman_export::ExportResourceLimitCause::Ceiling => ResourceLimitCause::Ceiling,
                merman_export::ExportResourceLimitCause::ArithmeticOverflow => {
                    ResourceLimitCause::ArithmeticOverflow
                }
                _ => ResourceLimitCause::Ceiling,
            },
            provenance: Some(provenance),
        }
    }
}

#[cfg(feature = "svg")]
impl From<SvgResourceLimitExceeded> for ResourceLimitExceeded {
    fn from(error: SvgResourceLimitExceeded) -> Self {
        Self::from_svg(error)
    }
}

#[cfg(feature = "ascii")]
impl From<AsciiError> for RenderError {
    fn from(error: AsciiError) -> Self {
        match error {
            AsciiError::Cancelled(cancelled) => Self::Cancelled(cancelled),
            AsciiError::ResourceLimitExceeded(resource) => {
                Self::from(ResourceLimitExceeded::from_ascii(resource))
            }
            AsciiError::OperationResourceTerminal(error) => {
                crate::operation_runner::operation_terminal_error(error)
            }
            other => Self::Ascii(other),
        }
    }
}

#[cfg(feature = "svg")]
impl From<merman_render::environment::RenderEnvironmentError> for RenderError {
    fn from(error: merman_render::environment::RenderEnvironmentError) -> Self {
        match error {
            merman_render::environment::RenderEnvironmentError::Cancelled(cancelled) => {
                Self::Cancelled(cancelled)
            }
            merman_render::environment::RenderEnvironmentError::Runtime(runtime) => {
                Self::RuntimePolicy(TerminalRuntimePolicyError::from(runtime))
            }
            merman_render::environment::RenderEnvironmentError::ThemeResource(resource) => {
                Self::ResourceLimitExceeded(ResourceLimitExceeded::from_theme(resource))
            }
            other => Self::SvgEnvironment(other),
        }
    }
}

#[cfg(feature = "ascii")]
fn map_ascii_error(error: AsciiError) -> RenderError {
    RenderError::from(error)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl From<ExportError> for RenderError {
    fn from(error: ExportError) -> Self {
        match error {
            ExportError::OperationResourceTerminal(error) => {
                crate::operation_runner::operation_terminal_error(error)
            }
            ExportError::Cancelled(cancelled) => Self::Cancelled(cancelled),
            other => match other.resource_limit_details() {
                Some(details) => match other.resource_limit_provenance() {
                    Some(provenance) => {
                        Self::from(ResourceLimitExceeded::from_export(details, provenance))
                    }
                    None => Self::Export(other),
                },
                None => Self::Export(other),
            },
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn map_export_error(error: ExportError) -> RenderError {
    RenderError::from(error)
}

/// Target request for the canonical facade.
///
/// The semantic target is available in every feature configuration. SVG, ASCII, and terminal
/// export variants are feature-gated leaves of the same dispatch seam.
// Keep the request enum by-value: callers construct a single target request directly and the
// feature-gated variants already own their complete render policy. Boxing only the ASCII leaf
// would make the public constructors inconsistent without reducing the actual render payload.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RenderTarget {
    Semantic,
    #[cfg(feature = "svg")]
    Svg(SvgRequest),
    #[cfg(feature = "svg")]
    Document(SvgRequest),
    #[cfg(feature = "svg")]
    LayoutJson(SvgRequest),
    #[cfg(feature = "svg")]
    SvgPlan(SvgRequest),
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
    pub environment: SvgEnvironment,
    pub layout: LayoutOptions,
    pub options: SvgRenderOptions,
    pub debug: SvgDebugOptions,
    pub pipeline: Option<SvgPipeline>,
}

#[cfg(feature = "svg")]
impl Default for SvgRequest {
    fn default() -> Self {
        Self {
            environment: SvgEnvironment::deterministic(),
            layout: LayoutOptions::headless_svg_defaults(),
            options: SvgRenderOptions::default(),
            debug: SvgDebugOptions::default(),
            pipeline: None,
        }
    }
}

#[cfg(feature = "ascii")]
/// Target-local configuration for one ASCII render operation.
///
/// Presentation and family layout settings remain reusable in `options`; resource budgets belong
/// to this request and are passed unchanged to the model backend.
#[derive(Debug, Clone, Default)]
pub struct AsciiRequest {
    pub options: AsciiRenderOptions,
    pub resources: AsciiResourcePolicy,
    pub viewport: AsciiViewportPolicy,
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
    source: &'a str,
    target: RenderTarget,
    control: OperationControl,
    parse_options: Option<ParseOptions>,
    resources: Option<InputResourcePolicy>,
    #[cfg(feature = "svg")]
    theme: Option<DiagramTheme>,
}

impl<'a> RenderRequest<'a> {
    /// Creates a typed operation request that inherits the renderer's parse and input-resource
    /// defaults until an explicit request override is applied.
    pub fn new(source: &'a str, target: RenderTarget, control: OperationControl) -> Self {
        Self {
            source,
            target,
            control,
            parse_options: None,
            resources: None,
            #[cfg(feature = "svg")]
            theme: None,
        }
    }

    pub fn semantic(source: &'a str, control: OperationControl) -> Self {
        Self::new(source, RenderTarget::Semantic, control)
    }

    #[cfg(feature = "svg")]
    pub fn svg(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::Svg(request), control)
    }

    /// Requests one completed graphical document that can project every enabled native target.
    #[cfg(feature = "svg")]
    pub fn document(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::Document(request), control)
    }

    #[cfg(feature = "svg")]
    pub fn layout_json(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::LayoutJson(request), control)
    }

    #[cfg(feature = "svg")]
    pub fn svg_plan(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::SvgPlan(request), control)
    }

    #[cfg(feature = "ascii")]
    pub fn ascii(source: &'a str, control: OperationControl, request: AsciiRequest) -> Self {
        Self::new(source, RenderTarget::Ascii(request), control)
    }

    #[cfg(feature = "png")]
    pub fn png(source: &'a str, control: OperationControl, request: PngRequest) -> Self {
        Self::new(source, RenderTarget::Png(request), control)
    }

    #[cfg(feature = "jpeg")]
    pub fn jpeg(source: &'a str, control: OperationControl, request: JpegRequest) -> Self {
        Self::new(source, RenderTarget::Jpeg(request), control)
    }

    #[cfg(feature = "pdf")]
    pub fn pdf(source: &'a str, control: OperationControl, request: PdfRequest) -> Self {
        Self::new(source, RenderTarget::Pdf(request), control)
    }

    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = Some(parse_options);
        self
    }

    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = Some(resources);
        self
    }

    /// Binds one compiled theme to the whole parse-to-artifact operation.
    ///
    /// The same recipe supplies parse compatibility and family rendering evidence. Keeping the
    /// theme on the operation prevents a semantic artifact parsed under one recipe from being
    /// rendered under another.
    #[cfg(feature = "svg")]
    pub fn with_theme(mut self, theme: DiagramTheme) -> Self {
        self.theme = Some(theme);
        self
    }
}

/// Successful output from a canonical request.
#[derive(Debug)]
#[non_exhaustive]
pub enum RenderOutput {
    Semantic(Option<SemanticArtifact>),
    #[cfg(feature = "svg")]
    Svg(Option<SvgOutput>),
    #[cfg(feature = "svg")]
    Document(Option<RenderedDocument>),
    #[cfg(feature = "svg")]
    LayoutJson(Option<SvgLayoutOutput>),
    #[cfg(feature = "svg")]
    SvgPlan(Option<merman_render::family::RenderCapabilityPlan>),
    #[cfg(feature = "ascii")]
    Ascii(Option<AsciiOutput>),
    #[cfg(feature = "png")]
    Png(Option<RasterOutput>),
    #[cfg(feature = "jpeg")]
    Jpeg(Option<RasterOutput>),
    #[cfg(feature = "pdf")]
    Pdf(Option<PdfOutput>),
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
        let RenderRequest {
            source,
            target,
            control,
            parse_options,
            resources,
            #[cfg(feature = "svg")]
            theme,
        } = request;
        #[cfg(feature = "svg")]
        let operation_engine = theme
            .as_ref()
            .map(|theme| {
                merman_render::__private::install_parse_compatibility(theme, self.engine.clone())
            })
            .unwrap_or_else(|| self.engine.clone());
        #[cfg(not(feature = "svg"))]
        let operation_engine = self.engine.clone();
        let operation = Operation::begin(
            &operation_engine,
            source,
            control,
            resources.unwrap_or(self.resources),
            #[cfg(feature = "svg")]
            theme,
        )?;
        let semantic =
            operation.parse_render_model(source, parse_options.unwrap_or(self.parse_options))?;
        let Some(semantic) = semantic else {
            #[cfg(feature = "ascii")]
            if matches!(&target, RenderTarget::Ascii(_)) {
                return Err(RenderError::NoDiagram);
            }
            return Ok(RenderOutput::empty(target));
        };
        semantic.render(target)
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
        let operation = Operation::begin(
            &self.engine,
            source,
            control,
            resources,
            #[cfg(feature = "svg")]
            None,
        )?;
        operation.parse_render_model(source, parse_options)
    }
}

impl SemanticArtifact {
    /// Returns Mermaid's compatibility semantic JSON projection without exposing family internals.
    pub fn compatibility_json(&self) -> Result<serde_json::Value, RenderError> {
        self.parsed()
            .model()
            .compatibility_json_controlled(self.parsed().metadata(), self.control())
            .map_err(RenderError::Cancelled)?
            .map_err(RenderError::from)
    }

    /// Consumes this operation-owned semantic artifact into one typed output target.
    pub fn render(self, target: RenderTarget) -> Result<RenderOutput, RenderError> {
        match target {
            RenderTarget::Semantic => Ok(RenderOutput::Semantic(Some(self))),
            #[cfg(feature = "svg")]
            RenderTarget::Svg(request) => render_svg_target(self, request).map(RenderOutput::Svg),
            #[cfg(feature = "svg")]
            RenderTarget::Document(request) => {
                render_document_target(self, request).map(RenderOutput::Document)
            }
            #[cfg(feature = "svg")]
            RenderTarget::LayoutJson(request) => {
                render_layout_json_target(self, request).map(RenderOutput::LayoutJson)
            }
            #[cfg(feature = "svg")]
            RenderTarget::SvgPlan(request) => {
                render_svg_plan_target(self, request).map(RenderOutput::SvgPlan)
            }
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
            #[cfg(feature = "svg")]
            RenderTarget::Document(_) => Self::Document(None),
            #[cfg(feature = "svg")]
            RenderTarget::LayoutJson(_) => Self::LayoutJson(None),
            #[cfg(feature = "svg")]
            RenderTarget::SvgPlan(_) => Self::SvgPlan(None),
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
) -> Result<Option<SvgOutput>, RenderError> {
    let (rendered, _operation) = prepare_rendered_family_svg(semantic, &request)?;
    let required_capabilities = rendered.required_capabilities().to_vec();
    let finalized = merman_render::__private::finalize_standalone_for_target_admission(
        rendered,
        request.pipeline.as_ref(),
    )
    .map_err(RenderError::from)?;
    let (svg, family) = finalized.into_completion().into_output_and_report();
    finish_standalone_svg_target(svg, family, required_capabilities).map(Some)
}

#[cfg(feature = "svg")]
fn render_layout_json_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<SvgLayoutOutput>, RenderError> {
    if request.environment.theme_portability_requirement()
        == merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable
    {
        return Err(RenderError::PortabilityUnavailableForTarget {
            target: "layout-json",
        });
    }
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context,
        operation.control,
    )?;
    let artifact = merman_render::family::prepare(parsed, &request.layout, session)
        .map_err(RenderError::from)?;
    let gantt_time_axis = artifact.gantt_time_axis_diagnostics();
    let layout = artifact.layout_json().map_err(RenderError::from)?;
    Ok(Some(SvgLayoutOutput::new(layout, gantt_time_axis)))
}

#[cfg(feature = "svg")]
fn render_svg_plan_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<merman_render::family::RenderCapabilityPlan>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context,
        operation.control,
    )?;
    merman_render::family::plan_render(&parsed, &session)
        .map(Some)
        .map_err(RenderError::from)
}

#[cfg(feature = "svg")]
fn prepare_resvg_target(
    semantic: SemanticArtifact,
    request: &SvgRequest,
) -> Result<
    Option<(
        merman_render::svg::ResvgCompatibleSvg,
        merman_render::family::FamilyRenderReport,
        Vec<merman_render::RenderCapability>,
        OperationExecution,
    )>,
    RenderError,
> {
    let (rendered, operation) = prepare_rendered_family_svg(semantic, request)?;
    let required_capabilities = rendered.required_capabilities().to_vec();
    let pipeline = request
        .pipeline
        .clone()
        .unwrap_or_else(SvgPipeline::resvg_safe)
        .into_resvg_safe();
    rendered
        .finalize_resvg(&pipeline)
        .map(|sealed| {
            let (svg, family) = sealed.into_completion().into_output_and_report();
            (svg, family, required_capabilities, operation)
        })
        .map(Some)
        .map_err(RenderError::from)
}

#[cfg(feature = "svg")]
fn prepare_rendered_family_svg(
    semantic: SemanticArtifact,
    request: &SvgRequest,
) -> Result<(merman_render::family::RenderedFamilySvg, OperationExecution), RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context.clone(),
        operation.control.clone(),
    )?;
    let artifact = merman_render::family::prepare(parsed, &request.layout, session)
        .map_err(RenderError::from)?;
    let rendered = artifact
        .render_svg(&request.options, &request.debug)
        .map_err(RenderError::from)?;
    Ok((rendered, operation))
}

#[cfg(feature = "svg")]
fn render_document_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<RenderedDocument>, RenderError> {
    let Some((svg, family, required_capabilities, _operation)) =
        prepare_resvg_target(semantic, &request)?
    else {
        unreachable!("semantic artifact always produces a finalized SVG or an error")
    };
    Ok(Some(RenderedDocument::new(
        svg,
        family,
        required_capabilities,
    )))
}

#[cfg(feature = "ascii")]
fn render_ascii_target(
    semantic: SemanticArtifact,
    request: AsciiRequest,
) -> Result<Option<AsciiOutput>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
    let renderer = merman_ascii::AsciiRenderer::new(request.options);
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
    let renderer = renderer.map_err(map_ascii_error)?;
    let result = renderer.render_parsed_report(
        &parsed,
        request.viewport,
        &operation.control,
        &operation.context,
        request.resources,
    );
    match result {
        Ok(output) => Ok(Some(output)),
        Err(error @ AsciiError::ResourceLimitExceeded(_)) => {
            match operation
                .control
                .terminal_checkpoint_at(OperationPhase::Emit)
            {
                Err(terminal) => Err(crate::operation_runner::operation_terminal_error(terminal)),
                Ok(()) => Err(map_ascii_error(error)),
            }
        }
        Err(error) => Err(map_ascii_error(error)),
    }
}

#[cfg(feature = "png")]
fn render_png_target(
    semantic: SemanticArtifact,
    request: PngRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, family, required_capabilities, operation)) =
        prepare_resvg_target(semantic, &request.svg)?
    else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family, required_capabilities)
        .export_png(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "jpeg")]
fn render_jpeg_target(
    semantic: SemanticArtifact,
    request: JpegRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, family, required_capabilities, operation)) =
        prepare_resvg_target(semantic, &request.svg)?
    else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family, required_capabilities)
        .export_jpeg(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "pdf")]
fn render_pdf_target(
    semantic: SemanticArtifact,
    request: PdfRequest,
) -> Result<Option<PdfOutput>, RenderError> {
    let Some((svg, family, required_capabilities, operation)) =
        prepare_resvg_target(semantic, &request.svg)?
    else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family, required_capabilities)
        .export_pdf(&request.options, operation.control)
        .map(Some)
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::{RenderError, ResourceLimitCause};

    #[test]
    fn theme_resource_environment_error_maps_to_resource_limit_exceeded() {
        let policy = merman_render::diagram_theme::ThemeResourcePolicy::constrained();
        let maximum = policy
            .value(merman_render::diagram_theme::ThemeResourceLimitId::MaxThemeEncodedBytes)
            .expect("constrained theme input ceiling");
        let resource = policy
            .check_theme_encoded_bytes(maximum + 1)
            .expect_err("fixture must exceed the constrained theme input ceiling");

        let error = RenderError::from(
            merman_render::environment::RenderEnvironmentError::ThemeResource(resource),
        );
        let RenderError::ResourceLimitExceeded(resource) = error else {
            panic!("theme resource environment errors must use the common resource variant");
        };
        assert_eq!(resource.id, "max_theme_encoded_bytes");
        assert_eq!(resource.phase, "theme_input");
        assert_eq!(resource.actual, (maximum + 1) as u64);
        assert_eq!(resource.maximum, maximum as u64);
        assert_eq!(resource.cause, ResourceLimitCause::Ceiling);
    }

    #[test]
    fn render_environment_cancellation_maps_to_the_common_cancelled_variant() {
        let cancelled = merman_core::OperationCancelled {
            phase: merman_core::OperationPhase::Layout,
            reason: merman_core::CancelReason::Requested,
        };

        let error = RenderError::from(
            merman_render::environment::RenderEnvironmentError::Cancelled(cancelled),
        );
        let RenderError::Cancelled(mapped) = error else {
            panic!("render-environment cancellation must use the common cancelled variant");
        };
        assert_eq!(mapped, cancelled);
    }

    #[test]
    fn terminal_theme_resource_render_error_maps_to_resource_limit_exceeded() {
        let policy = merman_render::diagram_theme::ThemeResourcePolicy::constrained();
        let maximum = policy
            .value(merman_render::diagram_theme::ThemeResourceLimitId::MaxThemeEncodedBytes)
            .expect("constrained theme input ceiling");
        let resource = policy
            .check_theme_encoded_bytes(maximum + 1)
            .expect_err("fixture must exceed the constrained theme input ceiling");

        let error = RenderError::from(merman_render::Error::ThemeResourceLimitExceeded(resource));
        let RenderError::ResourceLimitExceeded(resource) = error else {
            panic!("terminal theme resource errors must use the common resource variant");
        };
        assert_eq!(resource.id, "max_theme_encoded_bytes");
        assert_eq!(resource.phase, "theme_input");
        assert_eq!(resource.actual, (maximum + 1) as u64);
        assert_eq!(resource.maximum, maximum as u64);
        assert_eq!(resource.cause, ResourceLimitCause::Ceiling);
    }
}
