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
use merman_ascii::{
    AsciiError, AsciiOutput, AsciiRenderOptions, AsciiResourcePolicy, AsciiViewportPolicy,
};
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

/// SVG host services and source-to-output resource limits.
///
/// Operation time, timezone, randomness, cancellation, and deadlines deliberately do not live
/// here. [`Renderer`] owns those operation-wide concerns and injects the already captured context
/// into the private SVG session. This keeps `SvgRequest` from becoming a second operation owner.
#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub struct SvgEnvironment {
    backend: BackendRenderEnvironment,
    #[cfg(feature = "native-fonts")]
    native_fonts: Option<merman_export::NativeFontContext>,
    #[cfg(all(
        feature = "native-fonts",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    text_measurement_is_default: bool,
    text_measurement_routes: [merman_render::environment::TextMeasurementRoute; 4],
    // Keep the admission projection with the backend policy so the facade can read it
    // before creating an operation or SVG session. Both are set by the same builders.
    input_resources: InputResourcePolicy,
}

#[cfg(feature = "svg")]
impl SvgEnvironment {
    /// Creates the deterministic default SVG service set.
    pub fn deterministic() -> Self {
        let text_measurement = TextMeasurementPolicy::deterministic();
        let resources = RenderResourcePolicy::default();
        Self {
            #[cfg(feature = "native-fonts")]
            native_fonts: None,
            #[cfg(all(
                feature = "native-fonts",
                any(feature = "png", feature = "jpeg", feature = "pdf")
            ))]
            text_measurement_is_default: true,
            backend: BackendRenderEnvironment::deterministic()
                .with_text_measurement_policy(text_measurement.clone())
                .with_resource_policy(resources),
            text_measurement_routes: text_measurement.routes(),
            input_resources: *resources.input_policy(),
        }
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        #[cfg(feature = "native-fonts")]
        {
            self.native_fonts = None;
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            {
                self.text_measurement_is_default = false;
            }
        }
        self.text_measurement_routes = policy.routes();
        self.backend = self.backend.with_text_measurement_policy(policy);
        self
    }

    /// Shares explicit fonts between all text-measurement phases and binary painting.
    /// Missing glyph coverage remains observable through the measurement report.
    #[cfg(feature = "native-fonts")]
    pub fn with_native_font_context(
        self,
        context: merman_export::NativeFontContext,
    ) -> Result<Self, merman_export::NativeFontError> {
        use merman_render::environment::{
            MeasurementProfileId, TextMeasurementPhase, TextMeasurementProfileIdentity,
        };
        let id = MeasurementProfileId::new("native-fonts")
            .map_err(|error| merman_export::NativeFontError::Measurement(error.to_string()))?;
        let identity = TextMeasurementProfileIdentity::new(id, context.identity())
            .map_err(|error| merman_export::NativeFontError::Measurement(error.to_string()))?;
        let policy = TextMeasurementPolicy::host_display(
            identity,
            std::sync::Arc::new(context.clone()),
            TextMeasurementPhase::ALL,
        );
        let mut environment = self.with_text_measurement_policy(policy);
        environment.native_fonts = Some(context);
        Ok(environment)
    }

    #[cfg(feature = "native-fonts")]
    fn check_native_fonts(&self) -> Result<(), RenderError> {
        if let Some(context) = &self.native_fonts {
            context.check()?;
        }
        Ok(())
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

    /// Sets source, model, layout, and SVG limits for graphical targets in one place.
    ///
    /// [`Renderer::render`] uses this policy's input limits unless the renderer or request
    /// explicitly supplies an [`InputResourcePolicy`]. Those input overrides do not change
    /// this environment's layout or SVG limits.
    pub fn with_resource_policy(mut self, policy: RenderResourcePolicy) -> Self {
        self.input_resources = *policy.input_policy();
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
    native_font_faces: Vec<String>,
}

#[cfg(feature = "svg")]
impl RenderEvidence {
    fn from_session(
        session: merman_render::environment::RenderSession,
        required_capabilities: Vec<RenderCapability>,
        native_font_faces: Vec<String>,
    ) -> Self {
        Self {
            execution_path: OperationExecutionPath::Renderer,
            required_capabilities,
            session: Box::new(session.report()),
            native_font_faces,
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

    /// Faces selected by successful native shaping probes in this operation.
    /// This excludes deterministic fallback and native painting-only font selection.
    pub fn native_font_faces(&self) -> &[String] {
        &self.native_font_faces
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
        native_font_faces: Vec<String>,
    ) -> Self {
        Self {
            svg,
            evidence: RenderEvidence::from_session(
                session,
                required_capabilities,
                native_font_faces,
            ),
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
    layout: merman_core::ManagedSemanticJson,
    gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
}

#[cfg(feature = "svg")]
impl SvgLayoutOutput {
    fn new(
        layout: merman_core::ManagedSemanticJson,
        gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) -> Self {
        Self {
            layout,
            gantt_time_axis,
        }
    }

    pub fn layout(&self) -> &merman_core::ManagedSemanticJson {
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
        merman_core::ManagedSemanticJson,
        Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) {
        (self.layout, self.gantt_time_axis)
    }
}

/// Structured error returned by the canonical rendering facade.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    #[cfg(feature = "native-fonts")]
    #[error(transparent)]
    NativeFont(#[from] merman_export::NativeFontError),
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
    "resource limit `{id}` exceeded during {phase}: actual={actual} maximum={maximum} cause={cause}{hint}",
    hint = self.recovery_hint()
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
    fn recovery_hint(&self) -> &'static str {
        if self.id == "native_font_measurements" && self.cause == ResourceLimitCause::Ceiling {
            "; simplify the diagram or increase NativeFontLimits::max_measurements via NativeFontOptions::with_limits before loading fonts; render resource profiles do not change this limit"
        } else {
            ""
        }
    }

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

/// Target request for the canonical facade.
///
/// The semantic target is available in every feature configuration. SVG, ASCII, and terminal
/// export variants are feature-gated leaves of the same dispatch seam.
// Keep the request enum by-value: callers construct a single target request directly and the
// feature-gated variants already own their complete render policy. Boxing only the ASCII leaf
// would make the public constructors inconsistent without reducing the actual render payload.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum RenderTarget {
    Semantic,
    #[cfg(feature = "svg")]
    Svg(SvgRequest),
    #[cfg(feature = "svg")]
    LayoutJson(SvgRequest),
    #[cfg(feature = "svg")]
    EdgeGeometryJson(SvgRequest),
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

impl RenderTarget {
    #[cfg(feature = "native-fonts")]
    fn normalize_native_font_context(self) -> Result<Self, RenderError> {
        match self {
            #[cfg(feature = "png")]
            Self::Png(mut request) => {
                normalize_binary_font_context(
                    &mut request.svg.environment,
                    &mut request.options.font_context,
                )?;
                Ok(Self::Png(request))
            }
            #[cfg(feature = "jpeg")]
            Self::Jpeg(mut request) => {
                normalize_binary_font_context(
                    &mut request.svg.environment,
                    &mut request.options.font_context,
                )?;
                Ok(Self::Jpeg(request))
            }
            #[cfg(feature = "pdf")]
            Self::Pdf(mut request) => {
                normalize_binary_font_context(
                    &mut request.svg.environment,
                    &mut request.options.font_context,
                )?;
                Ok(Self::Pdf(request))
            }
            other => Ok(other),
        }
    }

    #[cfg(feature = "native-fonts")]
    fn activate_native_font_context(self, control: &OperationControl) -> Result<Self, RenderError> {
        fn activate(
            environment: &mut SvgEnvironment,
            control: &OperationControl,
        ) -> Result<(), RenderError> {
            if let Some(selected) = &environment.native_fonts {
                *environment = environment
                    .clone()
                    .with_native_font_context(selected.for_operation(control))?;
            }
            Ok(())
        }

        match self {
            Self::Svg(mut request) => {
                activate(&mut request.environment, control)?;
                Ok(Self::Svg(request))
            }
            Self::LayoutJson(mut request) => {
                activate(&mut request.environment, control)?;
                Ok(Self::LayoutJson(request))
            }
            Self::EdgeGeometryJson(mut request) => {
                activate(&mut request.environment, control)?;
                Ok(Self::EdgeGeometryJson(request))
            }
            Self::SvgPlan(mut request) => {
                activate(&mut request.environment, control)?;
                Ok(Self::SvgPlan(request))
            }
            #[cfg(feature = "png")]
            Self::Png(mut request) => {
                activate(&mut request.svg.environment, control)?;
                request.options.font_context = request.svg.environment.native_fonts.clone();
                Ok(Self::Png(request))
            }
            #[cfg(feature = "jpeg")]
            Self::Jpeg(mut request) => {
                activate(&mut request.svg.environment, control)?;
                request.options.font_context = request.svg.environment.native_fonts.clone();
                Ok(Self::Jpeg(request))
            }
            #[cfg(feature = "pdf")]
            Self::Pdf(mut request) => {
                activate(&mut request.svg.environment, control)?;
                request.options.font_context = request.svg.environment.native_fonts.clone();
                Ok(Self::Pdf(request))
            }
            other => Ok(other),
        }
    }

    #[cfg(feature = "native-fonts")]
    fn native_fonts_are_selected(&self) -> bool {
        match self {
            Self::Svg(request)
            | Self::LayoutJson(request)
            | Self::EdgeGeometryJson(request)
            | Self::SvgPlan(request) => request.environment.native_fonts.is_some(),
            #[cfg(feature = "png")]
            Self::Png(request) => request.svg.environment.native_fonts.is_some(),
            #[cfg(feature = "jpeg")]
            Self::Jpeg(request) => request.svg.environment.native_fonts.is_some(),
            #[cfg(feature = "pdf")]
            Self::Pdf(request) => request.svg.environment.native_fonts.is_some(),
            _ => false,
        }
    }

    #[cfg(feature = "native-fonts")]
    fn native_font_pipeline(&self) -> Option<&SvgPipeline> {
        let request = match self {
            Self::Svg(request)
            | Self::LayoutJson(request)
            | Self::EdgeGeometryJson(request)
            | Self::SvgPlan(request) => request,
            #[cfg(feature = "png")]
            Self::Png(request) => &request.svg,
            #[cfg(feature = "jpeg")]
            Self::Jpeg(request) => &request.svg,
            #[cfg(feature = "pdf")]
            Self::Pdf(request) => &request.svg,
            _ => return None,
        };
        request.pipeline.as_ref()
    }

    fn svg_input_resource_policy(&self) -> Option<InputResourcePolicy> {
        match self {
            Self::Semantic => None,
            #[cfg(feature = "ascii")]
            Self::Ascii(_) => None,
            #[cfg(feature = "svg")]
            Self::Svg(request)
            | Self::LayoutJson(request)
            | Self::EdgeGeometryJson(request)
            | Self::SvgPlan(request) => Some(request.environment.input_resources),
            #[cfg(feature = "png")]
            Self::Png(request) => Some(request.svg.environment.input_resources),
            #[cfg(feature = "jpeg")]
            Self::Jpeg(request) => Some(request.svg.environment.input_resources),
            #[cfg(feature = "pdf")]
            Self::Pdf(request) => Some(request.svg.environment.input_resources),
        }
    }
}

#[cfg(all(
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
fn normalize_binary_font_context(
    environment: &mut SvgEnvironment,
    painting: &mut Option<merman_export::NativeFontContext>,
) -> Result<(), RenderError> {
    match (&environment.native_fonts, painting.as_ref()) {
        (Some(measuring), Some(painting)) if measuring != painting => {
            return Err(merman_export::NativeFontError::Measurement(
                "native font contexts for measurement and painting conflict; share the same context".to_owned(),
            ).into());
        }
        (Some(measuring), None) => *painting = Some(measuring.clone()),
        (None, Some(context)) => {
            if !environment.text_measurement_is_default {
                return Err(merman_export::NativeFontError::Measurement(
                    "native painting font context conflicts with an explicit text measurement policy".to_owned(),
                ).into());
            }
            *environment = environment
                .clone()
                .with_native_font_context(context.clone())?;
        }
        _ => {}
    }
    Ok(())
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
    /// Creates a typed operation request that inherits the renderer's parse defaults.
    ///
    /// Input limits come from an explicit request override, then an explicit renderer policy,
    /// then a graphical target's SVG environment, or otherwise the renderer's default policy.
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
    pub fn edge_geometry_json(
        source: &'a str,
        control: OperationControl,
        request: SvgRequest,
    ) -> Self {
        Self::new(source, RenderTarget::EdgeGeometryJson(request), control)
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

    /// Overrides input admission for this operation, ahead of renderer and target policies.
    /// Target-local layout and output limits remain unchanged.
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
    EdgeGeometryJson(Option<merman_render::model::EdgePaintGeometryOutput>),
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
    resources_explicit: bool,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            engine: Engine::new(),
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
            resources_explicit: false,
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

    /// Sets an explicit input policy ahead of graphical targets' SVG environment defaults.
    /// A request's explicit input policy can still override this policy.
    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = resources;
        self.resources_explicit = true;
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
    ///
    /// Input admission uses the request's explicit policy, then the renderer's explicit policy,
    /// then the graphical target's SVG environment policy. Semantic and ASCII targets use the
    /// renderer's input defaults when neither the request nor the renderer overrides them.
    pub fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput, RenderError> {
        let RenderRequest {
            source,
            target,
            control,
            parse_options,
            resources,
        } = request;
        let resources = resources.unwrap_or_else(|| {
            if self.resources_explicit {
                self.resources
            } else {
                target.svg_input_resource_policy().unwrap_or(self.resources)
            }
        });
        let operation = Operation::begin(&self.engine, source, control, resources)?;
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
    /// This uses the renderer's input policy before any output target is selected.
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

#[cfg(feature = "native-fonts")]
const UNSUPPORTED_NATIVE_TEXT_STYLES: &[&str] = &[
    "letter-spacing",
    "word-spacing",
    "text-transform",
    "white-space",
    "font",
    "font-variant",
    "font-stretch",
    "font-feature-settings",
    "font-variation-settings",
    "font-kerning",
    "line-height",
    "text-indent",
    "writing-mode",
    "direction",
    "unicode-bidi",
    "text-anchor",
    "dominant-baseline",
    "alignment-baseline",
    "baseline-shift",
    "kerning",
    "text-rendering",
    "font-size-adjust",
];

#[cfg(feature = "native-fonts")]
fn ensure_native_font_theme_typography(
    diagram_type: &str,
    config: &serde_json::Value,
) -> Result<(), RenderError> {
    let family = match diagram_type {
        "flowchart" | "flowchart-v2" | "flowchart-elk" | "agentflow" => "Flowchart",
        "classDiagram" | "class" => "Class",
        _ => return Ok(()),
    };
    // These families measure explicit px sizes but preserve the theme value in root CSS.
    // Other families resolve or override their sizes independently before SVG emission.
    if let Some(size) = config.pointer("/themeVariables/fontSize") {
        let supported = size.as_str().is_some_and(|size| {
            let size = size.trim();
            let size = if family == "Class" {
                size.trim_end_matches(';')
                    .trim()
                    .trim_end_matches("!important")
                    .trim()
            } else {
                size
            };
            size.strip_suffix("px")
                .and_then(|number| number.trim().parse::<f64>().ok())
                .is_some_and(|number| number.is_finite() && number >= 1.0)
        }) || size.as_f64() == Some(16.0);
        if !supported {
            return Err(merman_export::NativeFontError::Measurement(format!(
                "native {family} measurement requires themeVariables.fontSize as a positive px string; other spellings have different compatibility layout and CSS sizes; use deterministic text measurement to preserve compatibility"
            )).into());
        }
    }
    if family == "Flowchart"
        && config
            .get("fontWeight")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|weight| !matches!(weight.trim(), "" | "normal" | "400"))
    {
        return Err(merman_export::NativeFontError::Measurement(
            "native Flowchart measurement does not support root fontWeight; use node or class font-weight styles".to_owned(),
        ).into());
    }
    Ok(())
}

#[cfg(all(
    feature = "native-fonts",
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
fn unsupported_native_flowchart_html_style(
    model: &merman_core::diagrams::flowchart::FlowchartModel,
    context: &merman_core::diagrams::flowchart::FlowchartRenderContext,
    config: &merman_core::MermaidConfig,
    swimlane: bool,
    control: &OperationControl,
) -> Result<Option<String>, RenderError> {
    let value = config.as_value();
    // Node shapes use the root switch; edges and ordinary clusters honor the legacy fallback.
    let node_html = value.get("htmlLabels").and_then(serde_json::Value::as_bool);
    let edge_html = node_html
        .or_else(|| {
            value
                .pointer("/flowchart/htmlLabels")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(true);
    let inspect = |label: &str, label_type: Option<&str>| {
        merman_render::text::unsupported_html_label_style(
            label,
            label_type.unwrap_or("text"),
            config,
            UNSUPPORTED_NATIVE_TEXT_STYLES,
            control,
        )
        .map_err(RenderError::Cancelled)
    };
    if node_html.unwrap_or(true) {
        for node in &model.nodes {
            if let Some(property) = inspect(
                context.node_label_for_render(node).unwrap_or(&node.id),
                node.label_type.as_deref(),
            )? {
                return Ok(Some(property));
            }
        }
    }
    if edge_html {
        for edge in &model.edges {
            if let Some(label) = context.edge_label_for_render(edge)
                && let Some(property) = inspect(label, edge.label_type.as_deref())?
            {
                return Ok(Some(property));
            }
        }
    }
    // Dedicated Swimlane titles evaluate only flowchart.htmlLabels and always use Markdown.
    let swimlane_title_html = match value.pointer("/flowchart/htmlLabels") {
        None => true,
        Some(serde_json::Value::Bool(false) | serde_json::Value::Null) => false,
        Some(serde_json::Value::Number(value)) => value.as_f64() != Some(0.0),
        Some(serde_json::Value::String(value)) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "null" | "0"
        ),
        Some(_) => true,
    };
    // Only top-level Swimlane groups use the dedicated renderer; nested groups stay Flowchart.
    let subgraph_children: std::collections::HashSet<&str> = if swimlane {
        model
            .subgraphs
            .iter()
            .flat_map(|subgraph| subgraph.nodes.iter().map(String::as_str))
            .collect()
    } else {
        Default::default()
    };
    for (ordinal, subgraph) in model.subgraphs.iter().enumerate() {
        let (title_html, label_type) =
            if swimlane && !subgraph_children.contains(subgraph.id.as_str()) {
                (swimlane_title_html, Some("markdown"))
            } else {
                (edge_html, subgraph.label_type.as_deref())
            };
        if title_html
            && let Some(property) = inspect(
                context.subgraph_title_for_render(ordinal, subgraph),
                label_type,
            )?
        {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

#[cfg(feature = "native-fonts")]
fn unsupported_native_text_style(model: &serde_json::Value) -> Option<&str> {
    fn declaration(value: &serde_json::Value) -> Option<&str> {
        match value {
            serde_json::Value::String(raw) => raw.split(';').find_map(|declaration| {
                let (property, _) = declaration.split_once(':')?;
                let property = property.trim();
                UNSUPPORTED_NATIVE_TEXT_STYLES
                    .iter()
                    .any(|name| property.eq_ignore_ascii_case(name))
                    .then_some(property)
            }),
            serde_json::Value::Array(values) => values.iter().find_map(declaration),
            _ => None,
        }
    }

    match model {
        serde_json::Value::Object(fields) => fields.iter().find_map(|(key, value)| {
            // Inspect parser-owned declaration slots, never labels or arbitrary source text.
            if matches!(
                key.as_str(),
                "style"
                    | "styles"
                    | "textStyles"
                    | "cssStyle"
                    | "cssStyles"
                    | "cssCompiledStyles"
                    | "labelStyle"
            ) {
                declaration(value)
            } else if key == "classDefs" {
                value.as_object().and_then(|classes| {
                    classes.values().find_map(|class| {
                        declaration(class).or_else(|| unsupported_native_text_style(class))
                    })
                })
            } else {
                unsupported_native_text_style(value)
            }
        }),
        serde_json::Value::Array(values) => values.iter().find_map(unsupported_native_text_style),
        _ => None,
    }
}

impl SemanticArtifact {
    /// Returns Mermaid's compatibility semantic JSON projection without exposing family internals.
    pub fn compatibility_json(&self) -> Result<merman_core::ManagedSemanticJson, RenderError> {
        self.parsed()
            .model()
            .compatibility_json_controlled(self.parsed().metadata(), self.control())
            .map_err(RenderError::Cancelled)?
            .map_err(RenderError::from)
    }

    /// Consumes this operation-owned semantic artifact into one typed output target.
    pub fn render(self, target: RenderTarget) -> Result<RenderOutput, RenderError> {
        #[cfg(feature = "native-fonts")]
        let target = target
            .normalize_native_font_context()?
            .activate_native_font_context(self.control())?;
        #[cfg(feature = "native-fonts")]
        if target.native_fonts_are_selected() {
            let metadata = self.parsed().metadata();
            ensure_native_font_theme_typography(
                &metadata.diagram_type,
                metadata.effective_config.as_value(),
            )?;
            if self
                .parsed()
                .metadata()
                .effective_config
                .get_str("themeCSS")
                .is_some_and(|css| !css.trim().is_empty())
                || target
                    .native_font_pipeline()
                    .is_some_and(SvgPipeline::has_scoped_css)
            {
                return Err(merman_export::NativeFontError::Measurement(
                    "native font measurement does not support themeCSS or scoped CSS applied after layout; configure fontFamily through Mermaid configuration".to_owned(),
                ).into());
            }
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            ))]
            {
                let property = match self.parsed().model() {
                    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
                    merman_core::RenderSemanticModel::Flowchart(model) => {
                        let default_context = Default::default();
                        unsupported_native_flowchart_html_style(
                            model,
                            self.parsed()
                                .flowchart_render_context()
                                .unwrap_or(&default_context),
                            &metadata.effective_config,
                            metadata.effective_config.get_str("layout") == Some("swimlane"),
                            self.control(),
                        )?
                    }
                    #[cfg(feature = "diagram-agentflow")]
                    merman_core::RenderSemanticModel::Agentflow(model) => {
                        let (model, context) = model.to_flowchart_model();
                        unsupported_native_flowchart_html_style(
                            &model,
                            &context,
                            &metadata.effective_config,
                            false,
                            self.control(),
                        )?
                    }
                    _ => None,
                };
                if let Some(property) = property {
                    return Err(merman_export::NativeFontError::Measurement(format!(
                        "native font measurement does not support the inline HTML text geometry style `{property}`; use deterministic text measurement to preserve compatibility"
                    )).into());
                }
            }
            let model = self.compatibility_json()?;
            if let Some(property) = unsupported_native_text_style(&model) {
                return Err(merman_export::NativeFontError::Measurement(format!(
                    "native font measurement does not support the text geometry style `{property}`"
                ))
                .into());
            }
        }
        match target {
            RenderTarget::Semantic => Ok(RenderOutput::Semantic(Some(self))),
            #[cfg(feature = "svg")]
            RenderTarget::Svg(request) => render_svg_target(self, request).map(RenderOutput::Svg),
            #[cfg(feature = "svg")]
            RenderTarget::LayoutJson(request) => {
                render_layout_json_target(self, request).map(RenderOutput::LayoutJson)
            }
            #[cfg(feature = "svg")]
            RenderTarget::EdgeGeometryJson(request) => {
                render_edge_geometry_json_target(self, request).map(RenderOutput::EdgeGeometryJson)
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
            RenderTarget::EdgeGeometryJson(_) => Self::EdgeGeometryJson(None),
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
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
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
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    #[cfg(feature = "native-fonts")]
    let native_font_faces = request
        .environment
        .native_fonts
        .as_ref()
        .map_or_else(Vec::new, merman_export::NativeFontContext::selected_faces);
    #[cfg(not(feature = "native-fonts"))]
    let native_font_faces = Vec::new();
    Ok(Some(SvgOutput::new(
        svg,
        session,
        required_capabilities,
        native_font_faces,
    )))
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
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    Ok(Some(SvgLayoutOutput::new(layout, gantt_time_axis)))
}

#[cfg(feature = "svg")]
fn render_edge_geometry_json_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<merman_render::model::EdgePaintGeometryOutput>, RenderError> {
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
    let diagram_type = artifact.metadata().diagram_type.clone();
    let geometry = artifact.edge_geometry_json().map_err(RenderError::from)?;
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    Ok(Some(merman_render::model::EdgePaintGeometryOutput::new(
        diagram_type,
        geometry,
    )))
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
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
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
    let result = artifact
        .render_svg(&request.options, &request.debug)
        .map_err(RenderError::from)?
        .finalize_resvg(&pipeline)
        .map(|sealed| (sealed.into_parts().0, operation))
        .map(Some)
        .map_err(RenderError::from);
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    result
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
