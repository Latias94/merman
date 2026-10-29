use std::sync::Arc;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_core::OperationControl;

use super::RenderError;
use super::evidence::RenderEvidence;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use super::map_export_error;
#[cfg(feature = "pdf")]
use super::target_admission::native_pdf_admission;
#[cfg(any(feature = "png", feature = "jpeg"))]
use super::target_admission::native_raster_admission;
use super::target_admission::{
    DocumentPortabilityReport, TargetAdmissionReceipt, document_portability_report,
    document_svg_admission, enforce_portability_requirement, standalone_svg_admission,
};

/// Successful SVG output and the evidence for the operation that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutput {
    layout_adaptation: Option<merman_render::LayoutAdaptationObservation>,
    svg: String,
    evidence: RenderEvidence,
    admission: TargetAdmissionReceipt,
}

/// Completed graphical document retained as the single source for every terminal target.
///
/// The document owns its sealed native SVG and frozen family/session outcomes. PNG, JPEG, and PDF
/// projections reuse this exact object without repeating parsing, layout, SVG emission, or terminal
/// finalization. Each terminal target evaluates its own portability; completing a document does not
/// construct a standalone-SVG target result.
/// A native font context selected for layout is retained for every native projection. Export
/// options can share that exact context but cannot replace it or add one after layout completed.
#[derive(Debug, PartialEq, Eq)]
pub struct RenderedDocument {
    layout_adaptation: Option<merman_render::LayoutAdaptationObservation>,
    svg: merman_render::svg::ResvgCompatibleSvg,
    evidence: Arc<RenderEvidence>,
    portability: DocumentPortabilityReport,
    #[cfg(all(
        feature = "native-fonts",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    native_fonts: Option<merman_export::NativeFontContext>,
    #[cfg(all(
        feature = "native-fonts",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    selected_native_fonts: Option<merman_export::NativeFontContext>,
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
            report: _,
        } = self.inner;
        let (bytes, report) = export.encode_png_with_report().map_err(map_export_error)?;
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
            report: _,
        } = self.inner;
        let (bytes, report) = export.encode_jpeg_with_report().map_err(map_export_error)?;
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
            report: _,
        } = self;
        let (bytes, report) = export.encode_with_report().map_err(map_export_error)?;
        document.finish_pdf_export(bytes, report)
    }
}

impl RenderedDocument {
    /// Actual width and overflow from explicitly adapted family placement.
    pub const fn layout_adaptation(&self) -> Option<merman_render::LayoutAdaptationObservation> {
        self.layout_adaptation
    }

    pub(super) fn new(
        completion: merman_render::family::FamilyRenderCompletion<
            merman_render::svg::ResvgCompatibleSvg,
        >,
        required_capabilities: Vec<merman_render::RenderCapability>,
        #[cfg(feature = "native-fonts")] native_fonts: Option<merman_export::NativeFontContext>,
        #[cfg(all(
            feature = "native-fonts",
            any(feature = "png", feature = "jpeg", feature = "pdf")
        ))]
        selected_native_fonts: Option<merman_export::NativeFontContext>,
    ) -> Self {
        let layout_adaptation = completion.layout_adaptation();
        let (svg, family) = completion.into_output_and_report();
        let evidence = RenderEvidence::from_family(family, required_capabilities);
        #[cfg(feature = "native-fonts")]
        let evidence = evidence.with_native_font_diagnostics(native_fonts.as_ref());
        let evidence = Arc::new(evidence);
        let portability = document_portability_report(&svg, &evidence);
        Self {
            layout_adaptation,
            svg,
            evidence,
            portability,
            #[cfg(all(
                feature = "native-fonts",
                any(feature = "png", feature = "jpeg", feature = "pdf")
            ))]
            native_fonts,
            #[cfg(all(
                feature = "native-fonts",
                any(feature = "png", feature = "jpeg", feature = "pdf")
            ))]
            selected_native_fonts,
        }
    }

    pub fn svg(&self) -> &str {
        self.svg.as_str()
    }

    /// Borrows the immutable native SVG and resources used by every export.
    pub fn sealed_svg(&self) -> &merman_render::svg::ResvgCompatibleSvg {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn portability(&self) -> &DocumentPortabilityReport {
        &self.portability
    }

    /// Evaluates the standalone SVG target only when the caller requests that projection.
    ///
    /// Native exports use their own target outcomes and do not need this result.
    pub fn standalone_svg_admission(&self) -> TargetAdmissionReceipt {
        document_svg_admission(&self.svg, &self.portability)
    }

    #[cfg(feature = "png")]
    pub fn prepare_png_export(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<PreparedPngExport<'_>, RenderError> {
        #[cfg(feature = "native-fonts")]
        let options = &self.raster_options(options, &control)?;
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
        #[cfg(feature = "native-fonts")]
        let options = &self.raster_options(options, &control)?;
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
    pub(crate) fn finish_raster_export(
        &self,
        bytes: Vec<u8>,
        export_report: merman_export::RasterExportReport,
    ) -> Result<RasterOutput, RenderError> {
        let admission = native_raster_admission(export_report, &self.evidence, &self.portability);
        enforce_portability_requirement(&self.evidence, &admission)?;
        Ok(RasterOutput {
            layout_adaptation: self.layout_adaptation,
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
        #[cfg(feature = "native-fonts")]
        let options = &self.pdf_options(options, &control)?;
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

    #[cfg(all(
        feature = "native-fonts",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    fn export_font_context(
        &self,
        painting: Option<&merman_export::NativeFontContext>,
        control: &OperationControl,
    ) -> Result<Option<merman_export::NativeFontContext>, RenderError> {
        crate::operation_runner::checkpoint(control, merman_core::OperationPhase::Export)?;
        if let (Some(selected), Some(painting)) = (&self.selected_native_fonts, painting)
            && selected != painting
        {
            return Err(merman_export::NativeFontError::Measurement(
                "native font contexts for measurement and painting conflict; share the same context"
                    .to_owned(),
            )
            .into());
        }
        if self.selected_native_fonts.is_none() && painting.is_some() {
            return Err(merman_export::NativeFontError::Measurement(
                "native painting font context cannot be added to a completed document; select it before text measurement and native admission"
                    .to_owned(),
            )
            .into());
        }
        self.native_fonts
            .as_ref()
            .map(|fonts| fonts.for_export(control))
            .transpose()
            .map_err(RenderError::from)
    }

    #[cfg(all(feature = "native-fonts", any(feature = "png", feature = "jpeg")))]
    fn raster_options(
        &self,
        options: &merman_export::RasterOptions,
        control: &OperationControl,
    ) -> Result<merman_export::RasterOptions, RenderError> {
        let mut options = options.clone();
        options.font_context = self.export_font_context(options.font_context.as_ref(), control)?;
        Ok(options)
    }

    #[cfg(all(feature = "native-fonts", feature = "pdf"))]
    fn pdf_options(
        &self,
        options: &merman_export::PdfOptions,
        control: &OperationControl,
    ) -> Result<merman_export::PdfOptions, RenderError> {
        let mut options = options.clone();
        options.font_context = self.export_font_context(options.font_context.as_ref(), control)?;
        Ok(options)
    }

    #[cfg(feature = "pdf")]
    fn finish_pdf_export(
        &self,
        bytes: Vec<u8>,
        export_report: merman_export::PdfExportReport,
    ) -> Result<PdfOutput, RenderError> {
        let admission = native_pdf_admission(export_report, &self.evidence, &self.portability);
        enforce_portability_requirement(&self.evidence, &admission)?;
        Ok(PdfOutput {
            layout_adaptation: self.layout_adaptation,
            bytes,
            evidence: Arc::clone(&self.evidence),
            export_report,
            admission,
        })
    }
}

impl SvgOutput {
    #[cfg(feature = "native-fonts")]
    pub(super) fn with_native_font_diagnostics(
        mut self,
        context: Option<&merman_export::NativeFontContext>,
    ) -> Self {
        self.evidence = self.evidence.with_native_font_diagnostics(context);
        self
    }

    /// Actual width and overflow from explicitly adapted family placement.
    pub const fn layout_adaptation(&self) -> Option<merman_render::LayoutAdaptationObservation> {
        self.layout_adaptation
    }

    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    /// Returns the target-owned admission bound to these exact SVG bytes.
    ///
    /// Every successful standalone SVG has a receipt. Ordinary `BestEffort` output is unverified
    /// for native compatibility; an explicitly resvg-safe pipeline still observes that contract.
    /// Receipts may also be host-dependent or rejected. `RequirePortable` rejects nonportable output with
    /// [`RenderError::TargetAdmission`]. Callers must inspect the receipt status, for example with
    /// `output.admission().status().is_portable()`, rather than treating receipt presence as
    /// portability. Neither policy changes the selected SVG pipeline.
    pub const fn admission(&self) -> &TargetAdmissionReceipt {
        &self.admission
    }

    /// Returns SVG text and evidence, deliberately discarding any target-admission receipt.
    pub fn into_parts(self) -> (String, RenderEvidence) {
        (self.svg, self.evidence)
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone)]
pub struct RasterOutput {
    layout_adaptation: Option<merman_render::LayoutAdaptationObservation>,
    bytes: Vec<u8>,
    evidence: Arc<RenderEvidence>,
    export_report: merman_export::RasterExportReport,
    admission: TargetAdmissionReceipt,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterOutput {
    /// Actual width and overflow from explicitly adapted family placement.
    pub const fn layout_adaptation(&self) -> Option<merman_render::LayoutAdaptationObservation> {
        self.layout_adaptation
    }

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
    layout_adaptation: Option<merman_render::LayoutAdaptationObservation>,
    bytes: Vec<u8>,
    evidence: Arc<RenderEvidence>,
    export_report: merman_export::PdfExportReport,
    admission: TargetAdmissionReceipt,
}

#[cfg(feature = "pdf")]
impl PdfOutput {
    /// Actual width and overflow from explicitly adapted family placement.
    pub const fn layout_adaptation(&self) -> Option<merman_render::LayoutAdaptationObservation> {
        self.layout_adaptation
    }

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

pub(super) fn finish_standalone_svg_target(
    svg: merman_render::svg::StandaloneSvgArtifact,
    family: merman_render::family::FamilyRenderReport,
    required_capabilities: Vec<merman_render::RenderCapability>,
    control: &merman_core::OperationControl,
    layout_adaptation: Option<merman_render::LayoutAdaptationObservation>,
) -> Result<SvgOutput, RenderError> {
    crate::operation_runner::checkpoint(control, merman_core::OperationPhase::Postprocess)?;
    let evidence = RenderEvidence::from_family(family, required_capabilities);
    let admission = standalone_svg_admission(&svg, &evidence);
    crate::operation_runner::checkpoint(control, merman_core::OperationPhase::Postprocess)?;
    enforce_portability_requirement(&evidence, &admission)?;
    Ok(SvgOutput {
        layout_adaptation,
        svg: svg.into_string(),
        evidence,
        admission,
    })
}
