//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

#[cfg(feature = "ascii")]
use merman_core::OperationPhase;
use merman_core::{
    Engine, OperationCancelled, OperationControl, OperationResourceDomain,
    OperationResourceOverride, OperationResourceProvenance, ParseOptions,
    resources::InputResourcePolicy,
};

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::operation_runner::OperationExecution;
use crate::{TerminalDiagnostic, TerminalRuntimePolicyError, operation_runner::Operation};

#[cfg(feature = "ascii")]
use merman_ascii::{AsciiError, AsciiRenderOptions, AsciiResourcePolicy};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_export::ExportError;
#[cfg(feature = "svg")]
use merman_render::{
    LayoutOptions, RenderCapability, RenderCapabilityPolicy,
    ResourceLimitExceeded as SvgResourceLimitExceeded,
    environment::{RenderEnvironment as BackendRenderEnvironment, TextMeasurementPolicy},
    math::MathRenderer,
    presentation::PresentationRenderPolicy,
    resources::RenderResourcePolicy,
    svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions},
};

pub use crate::operation_runner::SemanticArtifact;

/// SVG-only host services and rendering limits.
///
/// Operation time, timezone, randomness, cancellation, and deadlines deliberately do not live
/// here. [`Renderer`] owns those operation-wide concerns and injects the already captured context
/// into the private SVG session. This keeps `SvgRequest` from becoming a second operation owner.
#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub struct SvgEnvironment {
    backend: BackendRenderEnvironment,
    text_measurement_routes: [merman_render::environment::TextMeasurementRoute; 4],
}

#[cfg(feature = "svg")]
impl SvgEnvironment {
    /// Creates the deterministic default SVG service set.
    pub fn deterministic() -> Self {
        let text_measurement = TextMeasurementPolicy::parity();
        Self {
            backend: BackendRenderEnvironment::deterministic()
                .with_text_measurement_policy(text_measurement.clone()),
            text_measurement_routes: text_measurement.routes(),
        }
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        self.text_measurement_routes = policy.routes();
        self.backend = self.backend.with_text_measurement_policy(policy);
        self
    }

    pub fn with_capability_policy(mut self, policy: RenderCapabilityPolicy) -> Self {
        self.backend = self.backend.with_capability_policy(policy);
        self
    }

    pub fn with_compiled_math_renderer(mut self) -> Self {
        self.backend = self.backend.with_compiled_math_renderer();
        self
    }

    pub fn with_math_renderer(
        mut self,
        renderer: std::sync::Arc<dyn MathRenderer + Send + Sync>,
    ) -> Self {
        self.backend = self.backend.with_math_renderer(renderer);
        self
    }

    pub fn without_math_renderer(mut self) -> Self {
        self.backend = self.backend.without_math_renderer();
        self
    }

    pub fn with_icon_registry(mut self, registry: merman_render::svg::IconRegistry) -> Self {
        self.backend = self.backend.with_icon_registry(registry);
        self
    }

    pub fn with_resource_policy(mut self, policy: RenderResourcePolicy) -> Self {
        self.backend = self.backend.with_resource_policy(policy);
        self
    }

    /// Returns the configured text-measurement routes without creating an operation session.
    pub fn text_measurement_routes(&self) -> [merman_render::environment::TextMeasurementRoute; 4] {
        self.text_measurement_routes.clone()
    }

    fn begin_session_in_context(
        &self,
        context: merman_core::runtime::OperationContext,
        control: OperationControl,
    ) -> merman_render::environment::RenderSession {
        self.backend.begin_session_in_context(context, control)
    }
}

#[cfg(feature = "svg")]
impl Default for SvgEnvironment {
    fn default() -> Self {
        Self::deterministic()
    }
}

/// Identifies the canonical source-to-target operation path that produced an artifact.
///
/// This deliberately describes the public facade rather than exposing an implementation type
/// such as the former `HeadlessOperation`.
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

/// Immutable evidence captured by a completed SVG operation.
///
/// The evidence is created only by the renderer after SVG emission/postprocessing succeeds. It
/// is intentionally a narrow projection: callers can inspect preparation-owned capability
/// requirements, measurement provenance, and runtime identity without gaining access to SVG
/// session services or family layout internals.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEvidence {
    execution_path: OperationExecutionPath,
    required_capabilities: Vec<RenderCapability>,
    session: Box<merman_render::environment::RenderSessionReport>,
}

#[cfg(feature = "svg")]
impl RenderEvidence {
    fn from_session(
        session: merman_render::environment::RenderSession,
        required_capabilities: Vec<RenderCapability>,
    ) -> Self {
        Self {
            execution_path: OperationExecutionPath::Renderer,
            required_capabilities,
            session: Box::new(session.report()),
        }
    }

    pub const fn execution_path(&self) -> OperationExecutionPath {
        self.execution_path
    }

    /// Returns the optional renderer capabilities selected by this operation's actual preparation.
    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required_capabilities
    }

    pub fn measurement_routes(&self) -> &[merman_render::environment::TextMeasurementRoute; 4] {
        self.session.measurement_routes()
    }

    pub fn measurement(&self) -> &merman_render::environment::TextMeasurementReport {
        self.session.measurement()
    }

    pub fn operation_context(&self) -> &merman_core::runtime::OperationContext {
        self.session.operation_context()
    }

    pub const fn unix_millis(&self) -> i64 {
        self.session.unix_millis()
    }

    pub const fn local_date(&self) -> merman_core::time::CivilDate {
        self.session.local_date()
    }

    pub fn local_time_zone(&self) -> &merman_core::time::LocalTimeZoneProvenance {
        self.session.local_time_zone()
    }

    pub fn render_seed(&self) -> std::num::NonZeroU64 {
        self.session.render_seed()
    }

    pub const fn layout_work_units(&self) -> usize {
        self.session.layout_work_units()
    }
}

/// Successful SVG output and the evidence for the operation that produced it.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutput {
    svg: String,
    evidence: RenderEvidence,
}

#[cfg(feature = "svg")]
impl SvgOutput {
    fn new(
        svg: String,
        session: merman_render::environment::RenderSession,
        required_capabilities: Vec<RenderCapability>,
    ) -> Self {
        Self {
            svg,
            evidence: RenderEvidence::from_session(session, required_capabilities),
        }
    }

    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub fn into_parts(self) -> (String, RenderEvidence) {
        (self.svg, self.evidence)
    }
}

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
            other => Self::Svg(other),
        }
    }
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

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn from_export(details: merman_export::ExportResourceLimitDetails) -> Self {
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
            provenance: None,
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

#[cfg(feature = "ascii")]
fn map_ascii_error(error: AsciiError) -> RenderError {
    RenderError::from(error)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl From<ExportError> for RenderError {
    fn from(error: ExportError) -> Self {
        if let Some(details) = error.resource_limit_details() {
            return Self::from(ResourceLimitExceeded::from_export(details));
        }
        match error {
            ExportError::Cancelled(cancelled) => Self::Cancelled(cancelled),
            other => Self::Export(other),
        }
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
    pub presentation: PresentationRenderPolicy,
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
            presentation: PresentationRenderPolicy::default(),
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

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone)]
pub struct RasterOutput {
    pub bytes: Vec<u8>,
    pub plan: merman_export::RasterPlan,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub plan: merman_export::PdfFilterImagePlan,
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
        }
    }

    pub fn semantic(source: &'a str, control: OperationControl) -> Self {
        Self::new(source, RenderTarget::Semantic, control)
    }

    #[cfg(feature = "svg")]
    pub fn svg(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::Svg(request), control)
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
}

/// Successful output from a canonical request.
#[derive(Debug)]
pub enum RenderOutput {
    Semantic(Option<SemanticArtifact>),
    #[cfg(feature = "svg")]
    Svg(Option<SvgOutput>),
    #[cfg(feature = "svg")]
    LayoutJson(Option<SvgLayoutOutput>),
    #[cfg(feature = "svg")]
    SvgPlan(Option<merman_render::family::RenderCapabilityPlan>),
    #[cfg(feature = "ascii")]
    Ascii(Option<String>),
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
        } = request;
        let operation = Operation::begin(
            &self.engine,
            source,
            control,
            resources.unwrap_or(self.resources),
        )?;
        let semantic =
            operation.parse_render_model(source, parse_options.unwrap_or(self.parse_options))?;
        let Some(semantic) = semantic else {
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
        let operation = Operation::begin(&self.engine, source, control, resources)?;
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
    .map_err(RenderError::from)?;
    let rendered = artifact
        .render_svg(&request.options, &request.debug)
        .map_err(RenderError::from)?;
    let rendered = match request.pipeline.as_ref() {
        Some(pipeline) => rendered
            .apply_pipeline(pipeline)
            .map_err(RenderError::from)?,
        None => rendered,
    };
    let required_capabilities = rendered.required_capabilities().to_vec();
    let (svg, _, _, session) = rendered.into_parts();
    Ok(Some(SvgOutput::new(svg, session, required_capabilities)))
}

#[cfg(feature = "svg")]
fn render_layout_json_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<SvgLayoutOutput>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request
        .environment
        .begin_session_in_context(operation.context, operation.control);
    let artifact = merman_render::family::prepare_with_render_policy(
        parsed,
        &request.layout,
        session,
        request.presentation,
    )
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
    let session = request
        .environment
        .begin_session_in_context(operation.context, operation.control);
    merman_render::family::plan_render_with_policy(&parsed, &session, request.presentation)
        .map(Some)
        .map_err(RenderError::from)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prepare_resvg_target(
    semantic: SemanticArtifact,
    request: &SvgRequest,
) -> Result<Option<(merman_render::svg::ResvgCompatibleSvg, OperationExecution)>, RenderError> {
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
    .map_err(RenderError::from)?;
    let pipeline = request
        .pipeline
        .clone()
        .unwrap_or_else(SvgPipeline::resvg_safe)
        .into_resvg_safe();
    artifact
        .render_svg(&request.options, &request.debug)
        .map_err(RenderError::from)?
        .finalize_resvg(&pipeline)
        .map(|sealed| (sealed.into_parts().0, operation))
        .map(Some)
        .map_err(RenderError::from)
}

#[cfg(feature = "ascii")]
fn render_ascii_target(
    semantic: SemanticArtifact,
    request: AsciiRequest,
) -> Result<Option<String>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
    let renderer = merman_ascii::AsciiRenderer::new(request.options);
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
    let renderer = renderer.map_err(map_ascii_error)?;
    let result = renderer.render_model(
        parsed.model(),
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
    let Some((svg, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::svg_to_png_with_plan_controlled(&svg, &request.options, operation.control)
        .map(|(bytes, plan)| Some(RasterOutput { bytes, plan }))
        .map_err(RenderError::from)
}

#[cfg(feature = "jpeg")]
fn render_jpeg_target(
    semantic: SemanticArtifact,
    request: JpegRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::svg_to_jpeg_with_plan_controlled(&svg, &request.options, operation.control)
        .map(|(bytes, plan)| Some(RasterOutput { bytes, plan }))
        .map_err(RenderError::from)
}

#[cfg(feature = "pdf")]
fn render_pdf_target(
    semantic: SemanticArtifact,
    request: PdfRequest,
) -> Result<Option<PdfOutput>, RenderError> {
    let Some((svg, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::svg_to_pdf_with_plan_controlled(&svg, &request.options, operation.control)
        .map(|(bytes, plan)| Some(PdfOutput { bytes, plan }))
        .map_err(RenderError::from)
}
