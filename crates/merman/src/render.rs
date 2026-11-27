//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

use crate::operation_runner::Operation;
#[cfg(feature = "svg")]
use crate::operation_runner::OperationExecution;
#[cfg(feature = "svg")]
use crate::theme::{ThemeOutcome, ThemeOutcomeNotice};
use crate::{TerminalDiagnostic, TerminalRuntimePolicyError};
use merman_core::{ParseOptions, SemanticParser};
#[cfg(any(feature = "ascii", feature = "svg"))]
use merman_model::OperationPhase;
use merman_model::{
    OperationCancelled, OperationControl, OperationResourceDomain, OperationResourceOverride,
    OperationResourceProvenance, resources::InputResourcePolicy,
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
mod ergonomic;
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
#[cfg(feature = "svg")]
use document::finish_standalone_svg_target;
#[cfg(feature = "pdf")]
pub use document::{PdfOutput, PreparedPdfExport};
#[cfg(feature = "svg")]
pub use document::{RenderedDocument, SvgOutput};
#[cfg(feature = "svg")]
pub use environment::SvgEnvironment;
pub use ergonomic::{LayoutBackend, RendererBuilder, RendererBuilderError};
#[cfg(feature = "native-fonts")]
pub use evidence::NativeFontDiagnostics;
#[cfg(feature = "svg")]
pub use evidence::RenderEvidence;

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
#[cfg(feature = "svg")]
use merman_render::{
    LayoutOptions, ResourceLimitExceeded as SvgResourceLimitExceeded,
    svg::{SvgDebugOptions, SvgOutputProfile, SvgPipeline, SvgRenderOptions},
    theme::ThemePortabilityRequirement,
};
#[cfg(feature = "svg")]
pub use target_admission::{
    DocumentPortabilityReport, RenderArtifactKind, TargetAdmissionError, TargetAdmissionReason,
    TargetAdmissionReceipt, TargetAdmissionStatus, TargetFontSource,
};

#[cfg(feature = "presentation")]
pub use crate::operation_runner::PresentationArtifact;
pub use crate::operation_runner::SemanticArtifact;
#[cfg(feature = "presentation")]
pub(crate) mod prepared;
#[cfg(feature = "presentation")]
use merman_core::MermaidEngine;
#[cfg(feature = "presentation")]
pub use prepared::{PrepareSourceRequest, PreparedSource};
#[cfg(feature = "presentation")]
use std::sync::OnceLock;

/// Successful SVG layout inspection output.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq)]
pub struct SvgLayoutOutput {
    layout: merman_model::ManagedJson,
    gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
}

#[cfg(feature = "svg")]
impl SvgLayoutOutput {
    fn new(
        layout: merman_model::ManagedJson,
        gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) -> Self {
        Self {
            layout,
            gantt_time_axis,
        }
    }

    pub fn layout(&self) -> &merman_model::ManagedJson {
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
        merman_model::ManagedJson,
        Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) {
        (self.layout, self.gantt_time_axis)
    }
}

/// Structured error returned by the canonical rendering facade.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    #[cfg(any(feature = "svg", feature = "ascii"))]
    #[error(transparent)]
    Theme(#[from] crate::theme::ThemeError),
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

impl From<merman_model::runtime::RuntimePolicyError> for RenderError {
    fn from(error: merman_model::runtime::RuntimePolicyError) -> Self {
        Self::RuntimePolicy(TerminalRuntimePolicyError::from(error))
    }
}

#[cfg(feature = "svg")]
impl From<merman_render::Error> for RenderError {
    fn from(error: merman_render::Error) -> Self {
        match error {
            merman_render::Error::Cancelled(cancelled) => Self::Cancelled(cancelled),
            merman_render::Error::SemanticProjection(error) => {
                let (diagram_type, message) = error.into_parts();
                Self::from(merman_core::Error::diagram_parse_fallback(
                    diagram_type,
                    message,
                ))
            }
            merman_render::Error::PortabilityUnavailableForTarget { target } => {
                Self::PortabilityUnavailableForTarget { target }
            }
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
        match (self.id, self.cause) {
            ("native_font_bytes", ResourceLimitCause::Ceiling) => {
                ". Select smaller fonts or increase NativeFontLimits::max_bytes via NativeFontOptions::with_limits before loading fonts."
            }
            ("native_font_measurements", ResourceLimitCause::Ceiling) => {
                "; simplify the diagram or increase NativeFontLimits::max_measurements via NativeFontOptions::with_limits before loading fonts; render resource profiles do not change this limit"
            }
            _ => "",
        }
    }

    pub(crate) fn from_input(error: merman_model::resources::InputResourceLimitExceeded) -> Self {
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
    #[cfg(feature = "presentation")]
    Presentation,
    #[cfg(feature = "svg")]
    Svg(SvgRequest),
    #[cfg(feature = "svg")]
    Document(SvgRequest),
    #[cfg(feature = "svg")]
    /// Emits versioned layout geometry.
    LayoutJson(SvgRequest),
    #[cfg(feature = "svg")]
    /// Emits geometry, metadata, compatibility semantics, and effective configuration.
    LayoutSnapshot(SvgRequest),
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
    #[cfg(feature = "svg")]
    fn svg_request_mut(&mut self) -> Option<&mut SvgRequest> {
        match self {
            Self::Svg(request)
            | Self::Document(request)
            | Self::LayoutJson(request)
            | Self::LayoutSnapshot(request)
            | Self::EdgeGeometryJson(request)
            | Self::SvgPlan(request) => Some(request),
            #[cfg(feature = "png")]
            Self::Png(request) => Some(&mut request.svg),
            #[cfg(feature = "jpeg")]
            Self::Jpeg(request) => Some(&mut request.svg),
            #[cfg(feature = "pdf")]
            Self::Pdf(request) => Some(&mut request.svg),
            _ => None,
        }
    }

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
    fn activate_native_font_context(self, control: &OperationControl) -> Self {
        fn activate(environment: &mut SvgEnvironment, control: &OperationControl) {
            if let Some(selected) = environment.selected_native_font_context() {
                *environment = environment
                    .clone()
                    .with_active_native_font_context(selected.for_operation(control));
            }
        }

        match self {
            #[cfg(feature = "svg")]
            Self::Svg(mut request) => {
                activate(&mut request.environment, control);
                Self::Svg(request)
            }
            #[cfg(feature = "svg")]
            Self::Document(mut request) => {
                activate(&mut request.environment, control);
                Self::Document(request)
            }
            #[cfg(feature = "svg")]
            Self::LayoutJson(mut request) => {
                activate(&mut request.environment, control);
                Self::LayoutJson(request)
            }
            #[cfg(feature = "svg")]
            Self::LayoutSnapshot(mut request) => {
                activate(&mut request.environment, control);
                Self::LayoutSnapshot(request)
            }
            #[cfg(feature = "svg")]
            Self::EdgeGeometryJson(mut request) => {
                activate(&mut request.environment, control);
                Self::EdgeGeometryJson(request)
            }
            #[cfg(feature = "svg")]
            Self::SvgPlan(mut request) => {
                activate(&mut request.environment, control);
                Self::SvgPlan(request)
            }
            #[cfg(feature = "png")]
            Self::Png(mut request) => {
                activate(&mut request.svg.environment, control);
                Self::Png(request)
            }
            #[cfg(feature = "jpeg")]
            Self::Jpeg(mut request) => {
                activate(&mut request.svg.environment, control);
                Self::Jpeg(request)
            }
            #[cfg(feature = "pdf")]
            Self::Pdf(mut request) => {
                activate(&mut request.svg.environment, control);
                Self::Pdf(request)
            }
            other => other,
        }
    }

    #[cfg(feature = "native-fonts")]
    fn native_font_svg_request_mut(&mut self) -> Option<&mut SvgRequest> {
        let request = match self {
            Self::Svg(request)
            | Self::Document(request)
            | Self::LayoutJson(request)
            | Self::LayoutSnapshot(request)
            | Self::EdgeGeometryJson(request)
            | Self::SvgPlan(request) => request,
            #[cfg(feature = "png")]
            Self::Png(request) => &mut request.svg,
            #[cfg(feature = "jpeg")]
            Self::Jpeg(request) => &mut request.svg,
            #[cfg(feature = "pdf")]
            Self::Pdf(request) => &mut request.svg,
            _ => return None,
        };
        request.environment.native_fonts.as_ref()?;
        Some(request)
    }

    #[cfg(feature = "presentation")]
    fn svg_input_resource_policy(&self) -> Option<InputResourcePolicy> {
        #[cfg(feature = "svg")]
        let input_policy =
            |request: &SvgRequest| *request.environment.resource_policy().input_policy();
        match self {
            Self::Semantic => None,
            #[cfg(feature = "presentation")]
            Self::Presentation => None,
            #[cfg(feature = "ascii")]
            Self::Ascii(_) => None,
            #[cfg(feature = "svg")]
            Self::Svg(request)
            | Self::Document(request)
            | Self::LayoutJson(request)
            | Self::LayoutSnapshot(request)
            | Self::EdgeGeometryJson(request)
            | Self::SvgPlan(request) => Some(input_policy(request)),
            #[cfg(feature = "png")]
            Self::Png(request) => Some(input_policy(&request.svg)),
            #[cfg(feature = "jpeg")]
            Self::Jpeg(request) => Some(input_policy(&request.svg)),
            #[cfg(feature = "pdf")]
            Self::Pdf(request) => Some(input_policy(&request.svg)),
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
    let measurement = if environment.text_measurement_is_default {
        environment::NativeFontMeasurementState::Default
    } else {
        environment::NativeFontMeasurementState::Explicit
    };
    let context = environment::resolve_native_font_context(
        environment.native_fonts.as_ref(),
        painting.as_ref(),
        measurement,
    )?;
    if environment.native_fonts.is_none()
        && let Some(context) = &context
    {
        *environment = environment
            .clone()
            .with_native_font_context(context.clone())?;
    }
    *painting = context;
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
    /// Selects the standalone SVG producer independently of any postprocessing pipeline.
    pub output_profile: SvgOutputProfile,
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
            output_profile: SvgOutputProfile::default(),
        }
    }
}

#[cfg(feature = "svg")]
impl SvgRequest {
    /// Selects the producer for standalone SVG. Layout-only targets do not emit SVG.
    ///
    /// The native compact profile requires an enabled renderer for the diagram. It does not
    /// fall back to Mermaid SVG when that renderer or a requested diagram construct is unsupported.
    pub fn with_output_profile(mut self, profile: SvgOutputProfile) -> Self {
        self.output_profile = profile;
        self
    }

    /// Selects a native theme for this graphical target, including prepared-source renders.
    pub fn with_native_theme(mut self, theme: crate::theme::ThemeSelection) -> Self {
        self.environment.native_theme = Some(theme);
        self
    }

    /// Replaces the canvas base in both native and Mermaid rendering.
    pub fn with_background(mut self, color: crate::theme::Color) -> Self {
        self.environment.background = Some(color);
        self
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
    /// Native palette selection for colored output.
    pub native_theme: Option<crate::theme::ThemeSelection>,
}

#[cfg(feature = "ascii")]
impl AsciiRequest {
    /// Projects the native theme into the terminal palette when color is enabled.
    pub fn with_native_theme(mut self, theme: crate::theme::ThemeSelection) -> Self {
        self.native_theme = Some(theme);
        self
    }

    fn apply_native_palette(
        &mut self,
        config: &merman_model::MermaidConfig,
        effective_config: &merman_model::MermaidConfig,
        diagram_type: &str,
    ) -> Result<(), RenderError> {
        if self.options.color_mode != merman_ascii::AsciiColorMode::Plain
            && let Some(selection) = &self.native_theme
            && let Some(theme) = selection.resolve(config, diagram_type)?
        {
            use merman_ascii::AsciiColorTheme;
            let family = merman_model::family::diagram_type_presentation_family_id(
                diagram_type,
                effective_config.get_str("layout"),
            );
            self.options.color_theme = match family {
                Some(family) => AsciiColorTheme::from_family_v2(
                    theme.family(family.as_str()),
                    theme.appearance(),
                ),
                None => AsciiColorTheme::from_resolved_v2(&theme),
            };
        }
        Ok(())
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
    source: &'a str,
    target: RenderTarget,
    control: OperationControl,
    parse_options: Option<ParseOptions>,
    resources: Option<InputResourcePolicy>,
    allow_missing_diagram: bool,
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
            allow_missing_diagram: false,
        }
    }

    pub fn semantic(source: &'a str, control: OperationControl) -> Self {
        Self::new(source, RenderTarget::Semantic, control)
    }

    /// Selects full Mermaid presentation and its explicit handoff APIs.
    #[cfg(feature = "presentation")]
    pub fn presentation(source: &'a str, control: OperationControl) -> Self {
        Self::new(source, RenderTarget::Presentation, control)
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
    /// Requests the versioned geometry response.
    pub fn layout_json(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::LayoutJson(request), control)
    }

    #[cfg(feature = "svg")]
    /// Requests the full layout snapshot, including compatibility semantics and configuration.
    pub fn layout_snapshot(
        source: &'a str,
        control: OperationControl,
        request: SvgRequest,
    ) -> Self {
        Self::new(source, RenderTarget::LayoutSnapshot(request), control)
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

    /// Selects a native theme for the graphical or terminal target.
    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub fn with_native_theme(mut self, theme: crate::theme::ThemeSelection) -> Self {
        #[cfg(feature = "svg")]
        if let Some(request) = self.target.svg_request_mut() {
            request.environment.native_theme = Some(theme);
            return self;
        }
        #[cfg(feature = "ascii")]
        if let RenderTarget::Ascii(request) = &mut self.target {
            request.native_theme = Some(theme);
        }
        self
    }
}

/// Successful output from a canonical request.
#[derive(Debug)]
#[non_exhaustive]
#[allow(
    clippy::large_enum_variant,
    reason = "Output variants preserve typed ownership of target-specific render artifacts."
)]
pub enum RenderOutput {
    Semantic(Option<SemanticArtifact>),
    #[cfg(feature = "presentation")]
    Presentation(Option<PresentationArtifact>),
    #[cfg(feature = "svg")]
    Svg(Option<SvgOutput>),
    #[cfg(feature = "svg")]
    Document(Option<RenderedDocument>),
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
    parser: SemanticParser,
    #[cfg(feature = "presentation")]
    presentation_engine: PresentationEngineSource,
    parse_options: ParseOptions,
    resources: InputResourcePolicy,
    resources_explicit: bool,
    #[cfg(any(feature = "svg", feature = "ascii"))]
    ergonomic_defaults: Option<std::sync::Arc<ergonomic::ErgonomicDefaults>>,
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone)]
enum PresentationEngineSource {
    Inherited(OnceLock<MermaidEngine>),
    Explicit(MermaidEngine),
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            parser: SemanticParser::new(),
            #[cfg(feature = "presentation")]
            presentation_engine: PresentationEngineSource::Inherited(OnceLock::new()),
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
            resources_explicit: false,
            #[cfg(any(feature = "svg", feature = "ascii"))]
            ergonomic_defaults: None,
        }
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects the canonical parser used by semantic and terminal operations.
    /// Inherited presentation defaults follow the new parser even after earlier full operations.
    /// Full-only adapters installed with `with_presentation_engine` are independent.
    pub fn with_semantic_parser(mut self, parser: SemanticParser) -> Self {
        self.parser = parser;
        #[cfg(feature = "presentation")]
        if matches!(
            self.presentation_engine,
            PresentationEngineSource::Inherited(_)
        ) {
            self.presentation_engine = PresentationEngineSource::Inherited(OnceLock::new());
        }
        self
    }

    /// Installs a full-only Mermaid engine, including its custom adapters and configuration.
    /// Semantic and ASCII requests continue to use the explicitly selected canonical parser.
    #[cfg(feature = "presentation")]
    pub fn with_presentation_engine(mut self, engine: MermaidEngine) -> Self {
        self.presentation_engine = PresentationEngineSource::Explicit(engine);
        self
    }

    pub fn with_runtime_policy(mut self, policy: merman_model::runtime::RuntimePolicy) -> Self {
        self.parser = self.parser.with_runtime_policy(policy.clone());
        #[cfg(feature = "presentation")]
        match &mut self.presentation_engine {
            PresentationEngineSource::Inherited(cache) => {
                if let Some(engine) = cache.get_mut() {
                    *engine = engine.clone().with_runtime_policy(policy);
                }
            }
            PresentationEngineSource::Explicit(engine) => {
                *engine = engine.clone().with_runtime_policy(policy);
            }
        }
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

    pub fn semantic_parser(&self) -> &SemanticParser {
        &self.parser
    }

    /// Initializes full presentation defaults only when a full operation requests them.
    #[cfg(feature = "presentation")]
    pub fn presentation_engine(&self) -> &MermaidEngine {
        match &self.presentation_engine {
            PresentationEngineSource::Explicit(engine) => engine,
            PresentationEngineSource::Inherited(cache) => cache.get_or_init(|| {
                MermaidEngine::new()
                    .with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_config(
                            self.parser.site_config_overrides().clone(),
                        ),
                    )
                    .with_runtime_policy(self.parser.runtime_policy().clone())
            }),
        }
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
        match &request.target {
            RenderTarget::Semantic => {
                let (artifact, target) = self.parse_semantic_request(request)?;
                match artifact {
                    Some(artifact) => artifact.render(target),
                    None => Ok(RenderOutput::empty(target)),
                }
            }
            #[cfg(feature = "ascii")]
            RenderTarget::Ascii(_) => {
                let (artifact, target) = self.parse_semantic_request(request)?;
                artifact.ok_or(RenderError::NoDiagram)?.render(target)
            }
            #[cfg(feature = "presentation")]
            _ => {
                let (artifact, target) = self.parse_presentation_request(request)?;
                match artifact {
                    Some(artifact) => artifact.render(target),
                    None => Ok(RenderOutput::empty(target)),
                }
            }
        }
    }

    /// Renders an SVG request into its bytes without constructing unrequested output metadata.
    ///
    /// Parse, resource, pipeline, native-font and cancellation behavior matches [`Self::render`].
    /// `RequirePortable` still evaluates the actual selected output and returns the same admission
    /// errors. This method accepts only requests constructed for [`RenderTarget::Svg`].
    #[cfg(feature = "svg")]
    pub fn render_svg_data(
        &self,
        request: RenderRequest<'_>,
    ) -> Result<Option<String>, RenderError> {
        if !matches!(&request.target, RenderTarget::Svg(_)) {
            return Err(RenderError::UnsupportedTarget(
                "render_svg_data requires an SVG request",
            ));
        }
        let (semantic, target) = self.parse_presentation_request(request)?;
        let Some(semantic) = semantic else {
            return Ok(None);
        };
        let RenderTarget::Svg(request) = semantic.admit_target(target)? else {
            unreachable!("SVG data request retains its target during admission");
        };
        render_svg_data_target(semantic, request).map(Some)
    }

    /// Writes one [`RenderTarget::LayoutJson`] request as compact or pretty JSON.
    ///
    /// Returns `Ok(false)` for an input with no diagram, or `Ok(true)` after writing the value.
    /// The writer receives no trailing newline and is left for the caller to flush. Admission,
    /// layout and the final cancellation check precede emission. Writer failures can leave
    /// partial output.
    #[cfg(feature = "svg")]
    pub fn write_layout_json(
        &self,
        request: RenderRequest<'_>,
        writer: impl std::io::Write,
        pretty: bool,
    ) -> Result<bool, RenderError> {
        if !matches!(&request.target, RenderTarget::LayoutJson(_)) {
            return Err(RenderError::UnsupportedTarget(
                "write_layout_json requires a layout JSON request",
            ));
        }
        let (semantic, target) = self.parse_presentation_request(request)?;
        let Some(semantic) = semantic else {
            return Ok(false);
        };
        let RenderTarget::LayoutJson(request) = semantic.admit_target(target)? else {
            unreachable!("layout JSON writer request retains its target during admission");
        };
        #[cfg(feature = "native-fonts")]
        let control = semantic.control().clone();
        with_prepared_layout_json(semantic, &request, |artifact| {
            #[cfg(feature = "native-fonts")]
            {
                crate::operation_runner::checkpoint(&control, OperationPhase::Emit)?;
                request.environment.check_native_fonts()?;
            }
            artifact.write_layout_geometry_json(writer, pretty)?;
            Ok(true)
        })
    }

    #[cfg(feature = "presentation")]
    fn parse_presentation_request(
        &self,
        request: RenderRequest<'_>,
    ) -> Result<(Option<PresentationArtifact>, RenderTarget), RenderError> {
        let RenderRequest {
            source,
            target,
            control,
            parse_options,
            resources,
            allow_missing_diagram,
        } = request;
        let resources = resources.unwrap_or_else(|| {
            if self.resources_explicit {
                self.resources
            } else {
                target.svg_input_resource_policy().unwrap_or(self.resources)
            }
        });
        let operation_engine = self.presentation_engine();
        let operation = Operation::begin(
            operation_engine.runtime_policy(),
            source,
            control,
            resources,
        )?;
        let semantic = operation.parse_presentation(
            operation_engine,
            source,
            parse_options.unwrap_or(self.parse_options),
            allow_missing_diagram,
        )?;
        Ok((semantic, target))
    }

    fn parse_semantic_request(
        &self,
        request: RenderRequest<'_>,
    ) -> Result<(Option<SemanticArtifact>, RenderTarget), RenderError> {
        let resources = request.resources.unwrap_or(self.resources);
        let operation = Operation::begin(
            self.parser.runtime_policy(),
            request.source,
            request.control,
            resources,
        )?;
        let artifact = match operation.parse_semantic(
            &self.parser,
            request.source,
            request.parse_options.unwrap_or(self.parse_options),
        ) {
            Err(RenderError::Parse(diagnostic))
                if request.allow_missing_diagram && diagnostic.is_missing_diagram() =>
            {
                None
            }
            result => result?,
        };
        Ok((artifact, request.target))
    }

    /// Parses canonical meaning without presentation or compatibility projections.
    pub fn parse_semantic(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        self.parse_semantic_request(RenderRequest::semantic(source, control))
            .map(|(artifact, _)| artifact)
    }

    /// Prepares full Mermaid presentation for a later output in the same operation.
    #[cfg(feature = "presentation")]
    pub fn prepare_presentation(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<Option<PresentationArtifact>, RenderError> {
        self.prepare_presentation_with(source, control, self.parse_options, self.resources)
    }

    #[cfg(feature = "presentation")]
    pub fn prepare_presentation_with(
        &self,
        source: &str,
        control: OperationControl,
        parse_options: ParseOptions,
        resources: InputResourcePolicy,
    ) -> Result<Option<PresentationArtifact>, RenderError> {
        self.parse_presentation_request(
            RenderRequest::presentation(source, control)
                .with_parse_options(parse_options)
                .with_resource_policy(resources),
        )
        .map(|(artifact, _)| artifact)
    }
}

#[cfg(feature = "native-fonts")]
fn ensure_native_font_theme_typography(
    diagram_type: &str,
    config: &merman_model::presentation::PresentationConfig,
) -> Result<(), RenderError> {
    let family = match diagram_type {
        "flowchart" | "flowchart-v2" | "flowchart-elk" | "agentflow" => "Flowchart",
        "classDiagram" | "class" => "Class",
        _ => return Ok(()),
    };
    // These families measure explicit px sizes but preserve the theme value in root CSS.
    // Other families resolve or override their sizes independently before SVG emission.
    if let Some(size) = config.get_path(&["themeVariables", "fontSize"]) {
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
            .get_str("fontWeight")
            .is_some_and(|weight| !matches!(weight.trim(), "" | "normal" | "400"))
    {
        return Err(merman_export::NativeFontError::Measurement(
            "native Flowchart measurement does not support root fontWeight; use node or class font-weight styles".to_owned(),
        ).into());
    }
    Ok(())
}

impl SemanticArtifact {
    /// Consumes canonical meaning into semantic inspection or terminal output.
    /// Graphical targets require an independently requested full presentation parse.
    pub fn render(self, target: RenderTarget) -> Result<RenderOutput, RenderError> {
        crate::operation_runner::checkpoint(
            self.control(),
            merman_model::OperationPhase::Semantic,
        )?;
        match target {
            RenderTarget::Semantic => Ok(RenderOutput::Semantic(Some(self))),
            #[cfg(feature = "ascii")]
            RenderTarget::Ascii(request) => {
                render_ascii_target(self, request).map(RenderOutput::Ascii)
            }
            #[cfg(feature = "presentation")]
            _ => Err(RenderError::UnsupportedTarget(
                "graphical output requires full presentation",
            )),
        }
    }
}

#[cfg(feature = "ascii")]
fn render_ascii_target(
    semantic: SemanticArtifact,
    mut request: AsciiRequest,
) -> Result<Option<AsciiOutput>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
    let metadata = &parsed.metadata;
    request.apply_native_palette(
        &metadata.config,
        &metadata.semantic_config,
        &metadata.diagram_type,
    )?;
    let renderer = merman_ascii::AsciiRenderer::new(request.options).map_err(map_ascii_error)?;
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
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

#[cfg(feature = "presentation")]
impl PresentationArtifact {
    /// Returns Mermaid's compatibility semantic JSON projection without exposing family internals.
    #[cfg(feature = "compat-json")]
    pub fn compatibility_json(&self) -> Result<merman_model::ManagedJson, RenderError> {
        self.parsed()
            .compatibility_json_controlled(self.control())
            .map_err(RenderError::Cancelled)?
            .map_err(merman_core::Error::from)
            .map_err(RenderError::from)
    }

    fn admit_target(&self, target: RenderTarget) -> Result<RenderTarget, RenderError> {
        #[cfg(feature = "svg")]
        self.validate_svg_output_profile(&target)?;
        #[cfg(feature = "native-fonts")]
        let mut target = target
            .normalize_native_font_context()?
            .activate_native_font_context(self.control());
        #[cfg(feature = "native-fonts")]
        if let Some(request) = target.native_font_svg_request_mut() {
            let metadata = self.parsed().metadata();
            request.environment.native_render_config = match &request.environment.native_theme {
                Some(selection)
                    if !selection.prefer_source(&metadata.config, &metadata.diagram_type) =>
                {
                    merman_render::theme::NativeRenderConfig::prepare(
                        &metadata.effective_config,
                        self.control(),
                    )?
                }
                _ => None,
            };
            let config = request
                .environment
                .native_render_config
                .as_ref()
                .map_or(&metadata.effective_config, |projected| projected.config());
            ensure_native_font_theme_typography(&metadata.diagram_type, config)?;
            if config
                .get_str("themeCSS")
                .is_some_and(|css| !css.trim().is_empty())
                || request
                    .pipeline
                    .as_ref()
                    .is_some_and(SvgPipeline::has_scoped_css)
            {
                return Err(merman_export::NativeFontError::Measurement(
                    "native font measurement does not support themeCSS or scoped CSS applied after layout; configure fontFamily through Mermaid configuration".to_owned(),
                ).into());
            }
            if let Some(property) =
                merman_render::__private::unsupported_native_html_style_with_config(
                    self.parsed(),
                    config,
                    self.control(),
                )?
            {
                return Err(merman_export::NativeFontError::Measurement(format!(
                    "native font measurement does not support the inline HTML text geometry style `{property}`; use deterministic text measurement to preserve compatibility"
                )).into());
            }
            if let Some(property) =
                merman_render::__private::unsupported_native_text_style_with_config(
                    self.parsed(),
                    config,
                    self.control(),
                )?
            {
                return Err(merman_export::NativeFontError::Measurement(format!(
                    "native font measurement does not support the text geometry style `{property}`"
                ))
                .into());
            }
        }
        Ok(target)
    }

    #[cfg(feature = "svg")]
    fn validate_svg_output_profile(&self, target: &RenderTarget) -> Result<(), RenderError> {
        let request = match target {
            RenderTarget::Svg(request) => {
                if request.output_profile == SvgOutputProfile::NativeCompact {
                    crate::operation_runner::checkpoint(self.control(), OperationPhase::Layout)?;
                    merman_render::family::validate_svg_output_profile(
                        request.output_profile,
                        self.family_id(),
                        request.pipeline.as_ref(),
                    )?;
                }
                return Ok(());
            }
            RenderTarget::Document(request) => request,
            #[cfg(feature = "png")]
            RenderTarget::Png(request) => &request.svg,
            #[cfg(feature = "jpeg")]
            RenderTarget::Jpeg(request) => &request.svg,
            #[cfg(feature = "pdf")]
            RenderTarget::Pdf(request) => &request.svg,
            _ => return Ok(()),
        };
        if request.output_profile == SvgOutputProfile::NativeCompact {
            crate::operation_runner::checkpoint(self.control(), OperationPhase::Export)?;
            return Err(merman_render::Error::UnsupportedSvgOutputProfile {
                profile: request.output_profile,
                reason: "the compact profile is available only for standalone SVG",
            }
            .into());
        }
        Ok(())
    }

    /// Consumes this operation-owned presentation artifact into one typed output target.
    pub fn render(self, target: RenderTarget) -> Result<RenderOutput, RenderError> {
        #[cfg(feature = "svg")]
        if let RenderTarget::EdgeGeometryJson(request) = &target {
            crate::operation_runner::checkpoint(self.control(), OperationPhase::Layout)?;
            if !merman_render::family::has_edge_geometry(self.parsed())? {
                check_edge_geometry_portability(request)?;
                merman_render::family::validate_layout_options(self.parsed(), &request.layout)?;
                crate::operation_runner::checkpoint(self.control(), OperationPhase::Emit)?;
                return Ok(RenderOutput::EdgeGeometryJson(Some(
                    merman_render::model::EdgePaintGeometryOutput::new(
                        self.parsed().metadata().diagram_type.clone(),
                        Vec::new(),
                    ),
                )));
            }
        }
        let target = self.admit_target(target)?;
        match target {
            RenderTarget::Semantic => Err(RenderError::UnsupportedTarget(
                "canonical semantic output requires a semantic parse operation",
            )),
            RenderTarget::Presentation => {
                crate::operation_runner::checkpoint(
                    self.control(),
                    merman_model::OperationPhase::Semantic,
                )?;
                Ok(RenderOutput::Presentation(Some(self)))
            }
            #[cfg(feature = "svg")]
            RenderTarget::Svg(request) => render_svg_target(self, request).map(RenderOutput::Svg),
            #[cfg(feature = "svg")]
            RenderTarget::Document(request) => {
                render_document_target(self, request).map(RenderOutput::Document)
            }
            #[cfg(feature = "svg")]
            RenderTarget::LayoutJson(request) => {
                render_layout_json_target(self, request, false).map(RenderOutput::LayoutJson)
            }
            #[cfg(feature = "svg")]
            RenderTarget::LayoutSnapshot(request) => {
                render_layout_json_target(self, request, true).map(RenderOutput::LayoutJson)
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
                render_presentation_ascii_target(self, request).map(RenderOutput::Ascii)
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
            #[cfg(feature = "presentation")]
            RenderTarget::Presentation => Self::Presentation(None),
            #[cfg(feature = "svg")]
            RenderTarget::Svg(_) => Self::Svg(None),
            #[cfg(feature = "svg")]
            RenderTarget::Document(_) => Self::Document(None),
            #[cfg(feature = "svg")]
            RenderTarget::LayoutJson(_) | RenderTarget::LayoutSnapshot(_) => Self::LayoutJson(None),
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
    semantic: PresentationArtifact,
    request: SvgRequest,
) -> Result<Option<SvgOutput>, RenderError> {
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    let (rendered, operation, theme_outcome) =
        prepare_rendered_family_svg(semantic, &request, crate::svg::SvgGeneration::Full, true)?;
    let required_capabilities = rendered.required_capabilities().to_vec();
    let shoal_layout_frame = rendered.shoal_layout_frame();
    let finalized = rendered.finalize_standalone(request.pipeline.as_ref())?;
    let completion = finalized.into_completion();
    let layout_adaptation = completion.layout_adaptation();
    let (svg, family) = completion.into_output_and_report();
    let output = finish_standalone_svg_target(
        svg,
        family,
        required_capabilities,
        &operation.control,
        layout_adaptation,
        shoal_layout_frame,
        theme_outcome,
    );
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    let output = output?;
    #[cfg(feature = "native-fonts")]
    let output = output.with_native_font_diagnostics(request.environment.native_fonts.as_ref());
    Ok(Some(output))
}

#[cfg(feature = "svg")]
fn render_svg_data_target(
    semantic: PresentationArtifact,
    request: SvgRequest,
) -> Result<String, RenderError> {
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    let (rendered, operation, _) =
        prepare_rendered_family_svg(semantic, &request, crate::svg::SvgGeneration::Full, false)?;
    let finalized = rendered.finalize_standalone(request.pipeline.as_ref())?;
    let result = finish_standalone_svg_data(
        finalized,
        request.environment.theme_portability_requirement(),
        &operation.control,
    );
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    result
}

#[cfg(feature = "svg")]
fn finish_standalone_svg_data(
    finalized: merman_render::family::RenderedStandaloneSvg,
    requirement: ThemePortabilityRequirement,
    control: &OperationControl,
) -> Result<String, RenderError> {
    crate::operation_runner::checkpoint(control, OperationPhase::Postprocess)?;
    let admission = if requirement == ThemePortabilityRequirement::RequirePortable {
        Some(target_admission::standalone_svg_admission_from_summary(
            finalized.artifact(),
            finalized.family_id(),
            finalized.has_host_measurements(),
        ))
    } else {
        None
    };
    crate::operation_runner::checkpoint(control, OperationPhase::Postprocess)?;
    if let Some(admission) = admission {
        target_admission::enforce_required_portability(&admission)?;
    }
    Ok(finalized.into_string())
}

#[cfg(feature = "svg")]
fn render_layout_json_target(
    semantic: PresentationArtifact,
    request: SvgRequest,
    snapshot: bool,
) -> Result<Option<SvgLayoutOutput>, RenderError> {
    with_prepared_layout_json(semantic, &request, |artifact| {
        let gantt_time_axis = artifact.gantt_time_axis_diagnostics();
        let layout = if snapshot {
            artifact.layout_json()
        } else {
            artifact.layout_geometry_json()
        }?;
        #[cfg(feature = "native-fonts")]
        request.environment.check_native_fonts()?;
        Ok(Some(SvgLayoutOutput::new(layout, gantt_time_axis)))
    })
}

#[cfg(feature = "svg")]
fn with_prepared_layout_json<T>(
    semantic: PresentationArtifact,
    request: &SvgRequest,
    project: impl FnOnce(&merman_render::__private::FamilyLayoutArtifact<'_>) -> Result<T, RenderError>,
) -> Result<T, RenderError> {
    // A prepared artifact keeps the operation's sticky terminal state. Observe it before
    // rejecting a target-specific portability requirement so cancellation/deadline errors cannot
    // be masked by a later target-admission error.
    crate::operation_runner::checkpoint(semantic.control(), OperationPhase::Layout)?;
    if request.environment.theme_portability_requirement()
        == ThemePortabilityRequirement::RequirePortable
    {
        return Err(RenderError::PortabilityUnavailableForTarget {
            target: "layout-json",
        });
    }
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        parsed.metadata(),
        operation.context,
        operation.control,
    )?;
    let artifact =
        merman_render::__private::prepare_layout(parsed.parsed(), &request.layout, session)?;
    project(&artifact)
}

#[cfg(feature = "svg")]
fn check_edge_geometry_portability(request: &SvgRequest) -> Result<(), RenderError> {
    if request.environment.theme_portability_requirement()
        == ThemePortabilityRequirement::RequirePortable
    {
        return Err(RenderError::PortabilityUnavailableForTarget {
            target: "edge-geometry-json",
        });
    }
    Ok(())
}

#[cfg(feature = "svg")]
fn render_edge_geometry_json_target(
    semantic: PresentationArtifact,
    request: SvgRequest,
) -> Result<Option<merman_render::model::EdgePaintGeometryOutput>, RenderError> {
    crate::operation_runner::checkpoint(semantic.control(), OperationPhase::Layout)?;
    check_edge_geometry_portability(&request)?;
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        parsed.metadata(),
        operation.context,
        operation.control,
    )?;
    let artifact = merman_render::family::prepare(parsed.parsed(), &request.layout, session)
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
    semantic: PresentationArtifact,
    request: SvgRequest,
) -> Result<Option<merman_render::family::RenderCapabilityPlan>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        parsed.metadata(),
        operation.context,
        operation.control,
    )?;
    merman_render::family::plan_render_with_options(parsed.parsed(), &request.layout, &session)
        .map(Some)
        .map_err(RenderError::from)
}

#[cfg(feature = "svg")]
fn prepare_document_target(
    semantic: PresentationArtifact,
    request: SvgRequest,
    default_generation: crate::svg::SvgGeneration,
) -> Result<(RenderedDocument, OperationExecution), RenderError> {
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    let theme_matte = mermaid_theme_matte(semantic.metadata(), &request.environment);
    let (rendered, operation, theme_outcome) =
        prepare_rendered_family_svg(semantic, &request, default_generation, true)?;
    let required_capabilities = rendered.required_capabilities().to_vec();
    let pipeline = request
        .pipeline
        .unwrap_or_else(SvgPipeline::resvg_safe)
        .into_resvg_safe();
    let sealed = rendered
        .finalize_resvg(&pipeline)
        .map_err(RenderError::from);
    #[cfg(feature = "native-fonts")]
    request.environment.check_native_fonts()?;
    let completion = sealed?.into_completion();
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Postprocess)?;
    let document = RenderedDocument::new(
        completion,
        required_capabilities,
        theme_outcome,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        theme_matte,
        #[cfg(feature = "native-fonts")]
        request.environment.native_fonts,
        #[cfg(all(
            feature = "native-fonts",
            any(feature = "png", feature = "jpeg", feature = "pdf")
        ))]
        request.environment.selected_native_fonts,
    );
    Ok((document, operation))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn mermaid_theme_matte(
    metadata: &merman_model::presentation::PresentationMetadata,
    environment: &SvgEnvironment,
) -> Option<crate::theme::Color> {
    use crate::MermaidThemeId;
    use merman_markup::theme_color::{ThemeColor, is_dark};

    let config = &metadata.effective_config;
    if environment.background.is_some() {
        return None;
    }
    let theme = MermaidThemeId::parse(config.get_str("theme")?).ok()?;
    let paint = config.get_str("themeVariables.background")?;
    if !theme.is_dark()
        && !(theme == MermaidThemeId::Base
            && config.get_bool("themeVariables.darkMode") == Some(true)
            && is_dark(paint).unwrap_or(false))
    {
        return None;
    }
    let [r, g, b, a] = ThemeColor::parse(paint).ok()?.rgba_channels().to_rgba8();
    Some(crate::theme::Color::rgba(r, g, b, a))
}

#[cfg(feature = "svg")]
fn prepare_rendered_family_svg(
    semantic: PresentationArtifact,
    request: &SvgRequest,
    default_generation: crate::svg::SvgGeneration,
    include_theme_outcome: bool,
) -> Result<
    (
        merman_render::family::RenderedFamilySvg,
        OperationExecution,
        ThemeOutcome,
    ),
    RenderError,
> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        parsed.metadata(),
        operation.context.clone(),
        operation.control.clone(),
    )?;
    let theme_outcome = match &request.environment.native_theme {
        Some(selection) if include_theme_outcome => {
            let resolved = session.native_theme().map(|theme| theme.resolved());
            let mut outcome = selection.outcome(resolved);
            if outcome.preset_id.is_none()
                && let (Some(theme), Some(family_id)) = (resolved, parsed.family_id())
            {
                outcome.notices.extend(
                    merman_render::theme::unsupported_features(theme, family_id)
                        .map(ThemeOutcomeNotice::Rendering),
                );
            }
            outcome
        }
        _ => ThemeOutcome::default(),
    };
    let artifact = merman_render::family::prepare(parsed.parsed(), &request.layout, session)
        .map_err(RenderError::from)?;
    let rendered = artifact
        .render_svg_for_target(
            &request.options,
            &request.debug,
            request.pipeline.as_ref(),
            default_generation,
            request.output_profile,
        )
        .map_err(RenderError::from)?;
    Ok((rendered, operation, theme_outcome))
}

#[cfg(feature = "svg")]
fn render_document_target(
    semantic: PresentationArtifact,
    request: SvgRequest,
) -> Result<Option<RenderedDocument>, RenderError> {
    let (document, operation) =
        prepare_document_target(semantic, request, crate::svg::SvgGeneration::Full)?;
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Postprocess)?;
    Ok(Some(document))
}

#[cfg(all(feature = "ascii", feature = "presentation"))]
fn ascii_diagram_from_full(
    model: &merman_model::presentation::RenderSemanticModel,
) -> merman_ascii::AsciiDiagram<'_> {
    use merman_ascii::AsciiDiagram;
    use merman_model::presentation::RenderSemanticModel;

    match model {
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => AsciiDiagram::Agentflow(model),
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(model) => AsciiDiagram::Architecture(model),
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(model) => AsciiDiagram::Zenuml(model),
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(model) => AsciiDiagram::Block(&model.semantic),
        #[cfg(feature = "diagram-c4")]
        RenderSemanticModel::C4(model) => AsciiDiagram::C4(model),
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => AsciiDiagram::Class(Box::new(model.topology_view())),
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => AsciiDiagram::Er(model),
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => AsciiDiagram::Flowchart(model),
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => AsciiDiagram::Gantt(
            merman_model::families::gantt::GanttTerminalView::from_render(model),
        ),
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => AsciiDiagram::GitGraph(model),
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => AsciiDiagram::Journey(model),
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => AsciiDiagram::Kanban(
            merman_model::families::kanban::KanbanTerminalView::from_render(model),
        ),
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => AsciiDiagram::Mindmap(
            merman_model::families::mindmap::MindmapTerminalView::from_render(model),
        ),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => AsciiDiagram::Packet(model),
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(model) => AsciiDiagram::Pie(model),
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => AsciiDiagram::Requirement(model),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => AsciiDiagram::Sequence(model),
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => AsciiDiagram::State(
            merman_model::families::state::StateTerminalView::from_render(model),
        ),
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => AsciiDiagram::Timeline(model),
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => AsciiDiagram::TreeView(model),
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => AsciiDiagram::Usecase(model),
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => AsciiDiagram::XyChart {
            model: &model.semantic,
            display: model.display,
        },
        _ => AsciiDiagram::Unsupported(model.kind()),
    }
}

#[cfg(all(feature = "ascii", feature = "presentation"))]
fn render_presentation_ascii_target(
    semantic: PresentationArtifact,
    mut request: AsciiRequest,
) -> Result<Option<AsciiOutput>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
    let parsed = parsed.parsed();
    let metadata = parsed.metadata();
    request.apply_native_palette(
        &metadata.config,
        metadata.effective_config.as_config(),
        &metadata.diagram_type,
    )?;
    let renderer = merman_ascii::AsciiRenderer::new(request.options);
    crate::operation_runner::checkpoint(&operation.control, OperationPhase::Admission)?;
    let renderer = renderer.map_err(map_ascii_error)?;
    let view = ascii_diagram_from_full(parsed.model());
    let source = merman_ascii::AsciiSourceContext {
        diagram_type: &metadata.diagram_type,
        config: &metadata.config,
        semantic_config: metadata.effective_config.as_config(),
        title: metadata.title.as_deref(),
    };
    let result = renderer.render_model_report(
        &view,
        source,
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
    semantic: PresentationArtifact,
    mut request: PngRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    request
        .svg
        .environment
        .set_export_backdrop(request.options.matte.as_deref(), None);
    let (document, operation) =
        prepare_document_target(semantic, request.svg, crate::svg::SvgGeneration::Static)?;
    document
        .export_png(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "jpeg")]
fn render_jpeg_target(
    semantic: PresentationArtifact,
    mut request: JpegRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    request.svg.environment.set_export_backdrop(
        request.options.matte.as_deref(),
        Some(merman_export::RasterOptions::DEFAULT_JPEG_MATTE),
    );
    let (document, operation) =
        prepare_document_target(semantic, request.svg, crate::svg::SvgGeneration::Static)?;
    document
        .export_jpeg(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "pdf")]
fn render_pdf_target(
    semantic: PresentationArtifact,
    mut request: PdfRequest,
) -> Result<Option<PdfOutput>, RenderError> {
    request
        .svg
        .environment
        .set_export_backdrop(request.options.page_paint.as_deref(), None);
    let (document, operation) =
        prepare_document_target(semantic, request.svg, crate::svg::SvgGeneration::Static)?;
    document
        .export_pdf(&request.options, operation.control)
        .map(Some)
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::RenderError;

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn layout_json_writer_checks_cancellation_at_emit() {
        use super::{OperationControl, OperationPhase, Renderer, SvgRequest};

        let control = OperationControl::new();
        let semantic = Renderer::new()
            .prepare_presentation("flowchart LR\nA --> B", control.clone())
            .unwrap()
            .unwrap();
        let mut written = Vec::new();
        let error =
            super::with_prepared_layout_json(semantic, &SvgRequest::default(), |artifact| {
                control.cancel();
                artifact.write_layout_geometry_json(&mut written, false)?;
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            RenderError::Cancelled(cancellation) if cancellation.phase == OperationPhase::Emit
        ));
        assert!(written.is_empty());
    }

    #[test]
    fn themed_document_input_admission_preserves_request_renderer_environment_precedence() {
        use super::{RenderRequest, Renderer, SvgEnvironment, SvgRequest};
        use crate::theme::{BuiltinPreset, ThemeSelection};
        use merman_model::{
            OperationControl,
            resources::{InputResourceLimitId, InputResourcePolicy},
        };
        use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};

        let theme = ThemeSelection::preset(BuiltinPreset::Studio, None);
        let environment = SvgEnvironment::default().with_resource_policy(
            RenderResourcePolicy::default()
                .with_limit(ResourceLimitId::MaxSourceBytes, 4)
                .unwrap(),
        );
        let request = || {
            RenderRequest::document(
                "flowchart TD\nA --> B",
                OperationControl::new(),
                SvgRequest {
                    environment: environment.clone(),
                    ..Default::default()
                },
            )
            .with_native_theme(theme.clone())
        };
        let input_policy = |limit| {
            InputResourcePolicy::default()
                .with_limit(InputResourceLimitId::MaxSourceBytes, limit)
                .unwrap()
        };
        let explicit_renderer = Renderer::new().with_resource_policy(input_policy(3));
        for (result, maximum) in [
            (Renderer::new().render(request()), 4),
            (explicit_renderer.render(request()), 3),
            (
                explicit_renderer.render(request().with_resource_policy(input_policy(2))),
                2,
            ),
        ] {
            let RenderError::ResourceLimitExceeded(resource) = result.unwrap_err() else {
                panic!("input should be rejected before theme preparation or family admission");
            };
            assert_eq!(resource.id, "max_source_bytes");
            assert_eq!(resource.maximum, maximum);
        }
    }

    #[test]
    fn render_environment_cancellation_maps_to_the_common_cancelled_variant() {
        let cancelled = merman_model::OperationCancelled {
            phase: merman_model::OperationPhase::Layout,
            reason: merman_model::CancelReason::Requested,
        };

        let error = RenderError::from(
            merman_render::environment::RenderEnvironmentError::Cancelled(cancelled),
        );
        let RenderError::Cancelled(mapped) = error else {
            panic!("render-environment cancellation must use the common cancelled variant");
        };
        assert_eq!(mapped, cancelled);
    }
}
