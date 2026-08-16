//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

#[cfg(feature = "svg")]
use std::sync::Arc;

use crate::operation_runner::Operation;
#[cfg(feature = "svg")]
use crate::operation_runner::OperationExecution;
use merman_core::{
    Engine, OperationCancelled, OperationControl, ParseOptions, resources::InputResourcePolicy,
    runtime::RuntimePolicyError,
};

#[cfg(feature = "svg")]
mod environment;
#[cfg(feature = "svg")]
mod evidence;
#[cfg(feature = "svg")]
mod target_admission;

#[cfg(feature = "svg")]
pub use environment::SvgEnvironment;
#[cfg(feature = "svg")]
pub use evidence::{RenderEvidence, ThemeEvidenceStatus, ThemeEvidenceSummary};
#[cfg(all(feature = "svg", feature = "internal-theme-acceptance"))]
pub(crate) use evidence::{ThemeAcceptanceEvidenceProjection, ThemeEvidenceScopeProjection};
#[cfg(feature = "pdf")]
use target_admission::native_pdf_admission;
#[cfg(any(feature = "png", feature = "jpeg"))]
use target_admission::native_raster_admission;
#[cfg(feature = "svg")]
pub use target_admission::{
    DocumentPortabilityReport, RenderArtifactKind, TargetAdmissionError, TargetAdmissionReason,
    TargetAdmissionReceipt, TargetAdmissionStatus, TargetFontSource,
};
#[cfg(feature = "svg")]
use target_admission::{
    artifact_digest, document_digest, document_portability_report, enforce_portability_requirement,
    standalone_svg_admission,
};

#[cfg(feature = "ascii")]
use merman_ascii::{AsciiError, AsciiRenderOptions, AsciiResourcePolicy};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_export::ExportError;
#[cfg(feature = "svg")]
use merman_render::{
    LayoutOptions, ResourceLimitExceeded as SvgResourceLimitExceeded,
    diagram_theme::DiagramTheme,
    svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions},
};

pub use crate::operation_runner::SemanticArtifact;

/// Successful SVG output and the evidence for the operation that produced it.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutput {
    svg: String,
    evidence: RenderEvidence,
    admission: TargetAdmissionReceipt,
}

/// Completed graphical document retained as the single source for every terminal target.
///
/// The document owns its sealed SVG, frozen family/session evidence, and standalone-SVG target
/// admission. PNG, JPEG, and PDF projections reuse this exact object; they never re-enter parsing,
/// layout, SVG emission, or terminal finalization. Completing the document does not enforce the
/// standalone-SVG receipt: each terminal target owns its own admission decision, so a
/// host-dependent SVG cannot preempt an otherwise portable native projection.
#[cfg(feature = "svg")]
#[derive(Debug, PartialEq, Eq)]
pub struct RenderedDocument {
    svg: merman_render::svg::StandaloneSvgArtifact,
    evidence: Arc<RenderEvidence>,
    portability: DocumentPortabilityReport,
    standalone_svg_admission: TargetAdmissionReceipt,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
struct PreparedRasterTarget<'a> {
    document: &'a RenderedDocument,
    export: merman_export::PreparedRaster,
    report: merman_export::RasterExportReport,
}

/// PNG export prepared from one completed document but not yet encoded.
#[cfg(feature = "png")]
pub struct PreparedPngExport<'a> {
    inner: PreparedRasterTarget<'a>,
}

#[cfg(feature = "png")]
impl PreparedPngExport<'_> {
    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.inner.report
    }

    pub const fn plan(&self) -> merman_export::RasterPlan {
        self.inner.report.raster()
    }

    pub fn encode(self) -> Result<RasterOutput, RenderError> {
        let PreparedRasterTarget {
            document,
            export,
            report: planned_report,
        } = self.inner;
        let (bytes, report) = export.encode_png_with_report().map_err(map_export_error)?;
        debug_assert_eq!(report, planned_report);
        document.finish_raster_export(bytes, report)
    }
}

/// JPEG export prepared from one completed document but not yet encoded.
#[cfg(feature = "jpeg")]
pub struct PreparedJpegExport<'a> {
    inner: PreparedRasterTarget<'a>,
}

#[cfg(feature = "jpeg")]
impl PreparedJpegExport<'_> {
    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.inner.report
    }

    pub const fn plan(&self) -> merman_export::RasterPlan {
        self.inner.report.raster()
    }

    pub fn encode(self) -> Result<RasterOutput, RenderError> {
        let PreparedRasterTarget {
            document,
            export,
            report: planned_report,
        } = self.inner;
        let (bytes, report) = export.encode_jpeg_with_report().map_err(map_export_error)?;
        debug_assert_eq!(report, planned_report);
        document.finish_raster_export(bytes, report)
    }
}

/// PDF export prepared from one completed document but not yet encoded.
#[cfg(feature = "pdf")]
pub struct PreparedPdfExport<'a> {
    document: &'a RenderedDocument,
    export: merman_export::PreparedPdf,
    report: merman_export::PdfExportReport,
}

#[cfg(feature = "pdf")]
impl PreparedPdfExport<'_> {
    pub const fn export_report(&self) -> merman_export::PdfExportReport {
        self.report
    }

    pub const fn plan(&self) -> merman_export::PdfFilterImagePlan {
        self.report.filters()
    }

    pub fn encode(self) -> Result<PdfOutput, RenderError> {
        let Self {
            document,
            export,
            report: planned_report,
        } = self;
        let (bytes, report) = export.encode_with_report().map_err(map_export_error)?;
        debug_assert_eq!(report, planned_report);
        document.finish_pdf_export(bytes, report)
    }
}

#[cfg(feature = "svg")]
impl RenderedDocument {
    fn new(
        svg: merman_render::svg::ResvgCompatibleSvg,
        family: merman_render::family::FamilyRenderReport,
    ) -> Self {
        let svg = merman_render::svg::StandaloneSvgArtifact::from(svg);
        let evidence = Arc::new(RenderEvidence::from_family(family));
        let portability = document_portability_report(&svg, &evidence);
        let public_svg_digest = artifact_digest(svg.as_str().as_bytes());
        let native_svg = svg.native_export_svg();
        let native_svg_digest = if native_svg == svg.as_str() {
            public_svg_digest
        } else {
            artifact_digest(native_svg.as_bytes())
        };
        let document_digest = document_digest(
            public_svg_digest,
            native_svg_digest,
            svg.resource_fingerprint(),
            &evidence,
            &portability,
        );
        let standalone_svg_admission = standalone_svg_admission(
            &svg,
            &evidence,
            &portability,
            document_digest,
            public_svg_digest,
        );
        Self {
            svg,
            evidence,
            portability,
            standalone_svg_admission,
        }
    }

    pub fn svg(&self) -> &str {
        self.svg.as_str()
    }

    /// Borrows the immutable terminal SVG together with its retained export resources and proofs.
    pub fn sealed_svg(&self) -> &merman_render::svg::ResvgCompatibleSvg {
        match self.svg.as_resvg_compatible() {
            Some(svg) => svg,
            None => unreachable!("rendered documents are always finalized through resvg-safe"),
        }
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn portability(&self) -> &DocumentPortabilityReport {
        &self.portability
    }

    pub const fn resource_fingerprint(&self) -> merman_render::svg::SvgResourceFingerprint {
        self.svg.resource_fingerprint()
    }

    /// Returns the experimental identity of this completed document.
    ///
    /// The digest binds the sealed SVG, resources, operation context, and document-stage evidence.
    /// Its canonical encoding remains alpha until the document contract is frozen.
    pub const fn document_digest(&self) -> [u8; 32] {
        self.standalone_svg_admission.document_digest()
    }

    /// Returns the standalone-SVG target receipt without interpreting it as document admission.
    ///
    /// Callers that publish the SVG directly must apply their own required-admission policy to this
    /// receipt. Native `export_*` projections enforce the document's portability requirement using
    /// their own target-owned receipts.
    pub const fn standalone_svg_admission(&self) -> &TargetAdmissionReceipt {
        &self.standalone_svg_admission
    }

    #[cfg(feature = "png")]
    pub fn prepare_png_export(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<PreparedPngExport<'_>, RenderError> {
        let export = merman_export::prepare_raster_controlled(self.sealed_svg(), options, control)
            .map_err(map_export_error)?;
        let report = export.report_for_output(merman_export::RasterOutputKind::Png);
        Ok(PreparedPngExport {
            inner: PreparedRasterTarget {
                document: self,
                export,
                report,
            },
        })
    }

    #[cfg(feature = "png")]
    pub fn export_png(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<RasterOutput, RenderError> {
        self.prepare_png_export(options, control)?.encode()
    }

    #[cfg(feature = "jpeg")]
    pub fn prepare_jpeg_export(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<PreparedJpegExport<'_>, RenderError> {
        let export = merman_export::prepare_raster_controlled(self.sealed_svg(), options, control)
            .map_err(map_export_error)?;
        let report = export.report_for_output(merman_export::RasterOutputKind::Jpeg);
        Ok(PreparedJpegExport {
            inner: PreparedRasterTarget {
                document: self,
                export,
                report,
            },
        })
    }

    #[cfg(feature = "jpeg")]
    pub fn export_jpeg(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<RasterOutput, RenderError> {
        self.prepare_jpeg_export(options, control)?.encode()
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    fn finish_raster_export(
        &self,
        bytes: Vec<u8>,
        export_report: merman_export::RasterExportReport,
    ) -> Result<RasterOutput, RenderError> {
        let artifact_kind = match export_report.output() {
            merman_export::RasterOutputKind::Png => RenderArtifactKind::Png,
            merman_export::RasterOutputKind::Jpeg => RenderArtifactKind::Jpeg,
            _ => unreachable!("prepared raster report uses a known output kind"),
        };
        let admission = native_raster_admission(
            artifact_kind,
            &bytes,
            export_report,
            &self.evidence,
            &self.portability,
            self.document_digest(),
            self.resource_fingerprint(),
        );
        enforce_portability_requirement(&self.evidence, &admission)?;
        Ok(RasterOutput {
            bytes,
            evidence: Arc::clone(&self.evidence),
            export_report,
            admission,
        })
    }

    #[cfg(feature = "pdf")]
    pub fn prepare_pdf_export(
        &self,
        options: &merman_export::PdfOptions,
        control: OperationControl,
    ) -> Result<PreparedPdfExport<'_>, RenderError> {
        let export = merman_export::prepare_pdf_controlled(self.sealed_svg(), options, control)
            .map_err(map_export_error)?;
        let report = export.report();
        Ok(PreparedPdfExport {
            document: self,
            export,
            report,
        })
    }

    #[cfg(feature = "pdf")]
    pub fn export_pdf(
        &self,
        options: &merman_export::PdfOptions,
        control: OperationControl,
    ) -> Result<PdfOutput, RenderError> {
        self.prepare_pdf_export(options, control)?.encode()
    }

    #[cfg(feature = "pdf")]
    fn finish_pdf_export(
        &self,
        bytes: Vec<u8>,
        export_report: merman_export::PdfExportReport,
    ) -> Result<PdfOutput, RenderError> {
        let admission = native_pdf_admission(
            &bytes,
            export_report,
            &self.evidence,
            &self.portability,
            self.document_digest(),
            self.resource_fingerprint(),
        );
        enforce_portability_requirement(&self.evidence, &admission)?;
        Ok(PdfOutput {
            bytes,
            evidence: Arc::clone(&self.evidence),
            export_report,
            admission,
        })
    }
}

#[cfg(feature = "svg")]
impl SvgOutput {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    /// Returns the target-owned admission bound to these exact SVG bytes.
    ///
    /// Every successful standalone SVG has a receipt. The `Option` wrapper remains for source
    /// compatibility with the alpha facade: successful outputs currently always return `Some`.
    /// `BestEffort` returns non-portable evidence to the caller, while `RequirePortable` rejects it
    /// with [`RenderError::TargetAdmission`]. Neither policy changes the selected SVG pipeline.
    pub const fn admission(&self) -> Option<&TargetAdmissionReceipt> {
        Some(&self.admission)
    }

    /// Returns SVG text and evidence, deliberately discarding any target-admission receipt.
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
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    RuntimePolicy(#[from] RuntimePolicyError),
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

    #[cfg(feature = "ascii")]
    fn from_operation(error: merman_core::OperationResourceLimitExceeded) -> Self {
        let phase = if error.id == merman_ascii::MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID {
            merman_ascii::ASCII_RESOURCE_LIMIT_DESCRIPTORS[0].phase
        } else {
            error.phase.as_str()
        };
        Self {
            id: error.id,
            phase,
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

    #[cfg(feature = "svg")]
    fn from_theme(error: merman_render::diagram_theme::ThemeResourceLimitExceeded) -> Self {
        Self {
            id: error.limit,
            phase: error.phase.as_str(),
            actual: u64::try_from(error.actual).unwrap_or(u64::MAX),
            maximum: u64::try_from(error.max).unwrap_or(u64::MAX),
            cause: ResourceLimitCause::Ceiling,
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
        merman_render::Error::ThemeResourceLimitExceeded(resource) => {
            RenderError::from(ResourceLimitExceeded::from_theme(resource))
        }
        other => RenderError::Svg(other),
    }
}

#[cfg(feature = "svg")]
impl From<merman_render::Error> for RenderError {
    fn from(error: merman_render::Error) -> Self {
        map_svg_error(error)
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
                Self::RuntimePolicy(runtime)
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
    bytes: Vec<u8>,
    evidence: Arc<RenderEvidence>,
    export_report: merman_export::RasterExportReport,
    admission: TargetAdmissionReceipt,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn plan(&self) -> merman_export::RasterPlan {
        self.export_report.raster()
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.export_report
    }

    pub const fn admission(&self) -> &TargetAdmissionReceipt {
        &self.admission
    }

    /// Returns the encoded bytes and deliberately discards render and target-admission evidence.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone)]
pub struct PdfOutput {
    bytes: Vec<u8>,
    evidence: Arc<RenderEvidence>,
    export_report: merman_export::PdfExportReport,
    admission: TargetAdmissionReceipt,
}

#[cfg(feature = "pdf")]
impl PdfOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn plan(&self) -> merman_export::PdfFilterImagePlan {
        self.export_report.filters()
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn export_report(&self) -> merman_export::PdfExportReport {
        self.export_report
    }

    pub const fn admission(&self) -> &TargetAdmissionReceipt {
        &self.admission
    }

    /// Returns the encoded bytes and deliberately discards render and target-admission evidence.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
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
            .map_err(RenderError::Parse)
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
    let finalized = rendered
        .finalize_standalone(request.pipeline.as_ref())
        .map_err(map_svg_error)?;
    let (svg, family) = finalized.into_completion().into_output_and_report();
    finish_standalone_svg_target(svg, family).map(Some)
}

#[cfg(feature = "svg")]
fn finish_standalone_svg_target(
    svg: merman_render::svg::StandaloneSvgArtifact,
    family: merman_render::family::FamilyRenderReport,
) -> Result<SvgOutput, RenderError> {
    let evidence = RenderEvidence::from_family(family);
    let portability = document_portability_report(&svg, &evidence);
    let public_svg_digest = artifact_digest(svg.as_str().as_bytes());
    let native_svg = svg.native_export_svg();
    let native_svg_digest = if native_svg == svg.as_str() {
        public_svg_digest
    } else {
        artifact_digest(native_svg.as_bytes())
    };
    let document_digest = document_digest(
        public_svg_digest,
        native_svg_digest,
        svg.resource_fingerprint(),
        &evidence,
        &portability,
    );
    let admission = standalone_svg_admission(
        &svg,
        &evidence,
        &portability,
        document_digest,
        public_svg_digest,
    );
    enforce_portability_requirement(&evidence, &admission)?;
    Ok(SvgOutput {
        svg: svg.into_string(),
        evidence,
        admission,
    })
}

#[cfg(feature = "svg")]
fn render_layout_json_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<SvgLayoutOutput>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context,
        operation.control,
    )?;
    let artifact =
        merman_render::family::prepare(parsed, &request.layout, session).map_err(map_svg_error)?;
    let gantt_time_axis = artifact.gantt_time_axis_diagnostics();
    let layout = artifact.layout_json().map_err(map_svg_error)?;
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
        .map_err(map_svg_error)
}

#[cfg(feature = "svg")]
fn prepare_resvg_target(
    semantic: SemanticArtifact,
    request: &SvgRequest,
) -> Result<
    Option<(
        merman_render::svg::ResvgCompatibleSvg,
        merman_render::family::FamilyRenderReport,
        OperationExecution,
    )>,
    RenderError,
> {
    let (rendered, operation) = prepare_rendered_family_svg(semantic, request)?;
    let pipeline = request
        .pipeline
        .clone()
        .unwrap_or_else(SvgPipeline::resvg_safe)
        .into_resvg_safe();
    rendered
        .finalize_resvg(&pipeline)
        .map(|sealed| {
            let (svg, family) = sealed.into_completion().into_output_and_report();
            (svg, family, operation)
        })
        .map(Some)
        .map_err(map_svg_error)
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
    let artifact =
        merman_render::family::prepare(parsed, &request.layout, session).map_err(map_svg_error)?;
    let rendered = artifact
        .render_svg(&request.options, &request.debug)
        .map_err(map_svg_error)?;
    Ok((rendered, operation))
}

#[cfg(feature = "svg")]
fn render_document_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<RenderedDocument>, RenderError> {
    let Some((svg, family, _operation)) = prepare_resvg_target(semantic, &request)? else {
        unreachable!("semantic artifact always produces a finalized SVG or an error")
    };
    Ok(Some(RenderedDocument::new(svg, family)))
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
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family)
        .export_png(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "jpeg")]
fn render_jpeg_target(
    semantic: SemanticArtifact,
    request: JpegRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family)
        .export_jpeg(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "pdf")]
fn render_pdf_target(
    semantic: SemanticArtifact,
    request: PdfRequest,
) -> Result<Option<PdfOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family)
        .export_pdf(&request.options, operation.control)
        .map(Some)
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::target_admission::{
        standalone_target_evidence_digest, target_admission_receipt_digest,
    };
    use super::{
        RenderError, RenderOutput, RenderRequest, Renderer, ResourceLimitCause, TargetFontSource,
    };
    use merman_core::OperationControl;

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

    #[test]
    fn standalone_target_evidence_digest_binds_font_source() {
        let output = Renderer::new()
            .render(RenderRequest::document(
                "info",
                OperationControl::new(),
                super::SvgRequest::default(),
            ))
            .expect("document should render");
        let RenderOutput::Document(Some(document)) = output else {
            panic!("expected a rendered document");
        };
        let receipt = document.standalone_svg_admission();
        let digest = |font_source| {
            standalone_target_evidence_digest(
                &document.svg,
                document.evidence(),
                receipt.status(),
                receipt.reasons(),
                font_source,
            )
        };

        assert_ne!(
            digest(TargetFontSource::None),
            digest(TargetFontSource::Embedded)
        );
        assert_eq!(
            receipt.receipt_digest(),
            target_admission_receipt_digest(
                receipt.artifact_kind(),
                receipt.status(),
                receipt.reasons(),
                receipt.font_source(),
                receipt.resource_fingerprint().as_bytes(),
                receipt.font_catalog_fingerprint().as_bytes(),
                receipt.document_digest(),
                receipt.target_evidence_digest(),
                receipt.artifact_digest(),
            )
        );
    }
}
