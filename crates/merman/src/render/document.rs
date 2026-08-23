use std::sync::Arc;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_core::OperationControl;

use super::RenderError;
use super::evidence::RenderEvidence;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use super::map_export_error;
#[cfg(any(feature = "png", feature = "jpeg"))]
use super::target_admission::RenderArtifactKind;
#[cfg(feature = "pdf")]
use super::target_admission::native_pdf_admission;
#[cfg(any(feature = "png", feature = "jpeg"))]
use super::target_admission::native_raster_admission;
use super::target_admission::{
    DocumentPortabilityReport, TargetAdmissionReceipt, artifact_digest, document_digest,
    document_portability_report, enforce_portability_requirement, standalone_svg_admission,
};

/// Successful SVG output and the evidence for the operation that produced it.
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
#[derive(Debug, PartialEq, Eq)]
pub struct RenderedDocument {
    svg: merman_render::svg::StandaloneSvgArtifact,
    evidence: Arc<RenderEvidence>,
    portability: DocumentPortabilityReport,
    standalone_svg_admission: TargetAdmissionReceipt,
    #[cfg(feature = "internal-theme-acceptance")]
    theme_route_cutover_receipts: Box<[merman_render::__private::ThemeRouteCutoverReceipt]>,
    #[cfg(feature = "internal-theme-acceptance")]
    svg_artifact_receipt: Option<merman_render::__private::SvgArtifactReceipt>,
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

impl RenderedDocument {
    pub(super) fn new(
        svg: merman_render::svg::ResvgCompatibleSvg,
        family: merman_render::family::FamilyRenderReport,
    ) -> Self {
        let svg = merman_render::svg::StandaloneSvgArtifact::from(svg);
        let public_svg_digest = artifact_digest(svg.as_str().as_bytes());
        #[cfg(feature = "internal-theme-acceptance")]
        let theme_route_cutover_receipts =
            merman_render::__private::seal_theme_route_cutover_receipts(&family, public_svg_digest)
                .into_boxed_slice();
        #[cfg(feature = "internal-theme-acceptance")]
        let svg_artifact_receipt = merman_render::__private::SvgArtifactReceipt::observe_svg(
            svg.as_str(),
            public_svg_digest,
        );
        let evidence = Arc::new(RenderEvidence::from_family(family));
        let portability = document_portability_report(&svg, &evidence);
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
            #[cfg(feature = "internal-theme-acceptance")]
            theme_route_cutover_receipts,
            #[cfg(feature = "internal-theme-acceptance")]
            svg_artifact_receipt,
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

    #[cfg(feature = "internal-theme-acceptance")]
    pub(crate) fn theme_route_cutover_receipts(
        &self,
    ) -> &[merman_render::__private::ThemeRouteCutoverReceipt] {
        &self.theme_route_cutover_receipts
    }

    #[cfg(feature = "internal-theme-acceptance")]
    pub(crate) fn svg_artifact_receipt(
        &self,
    ) -> Option<&merman_render::__private::SvgArtifactReceipt> {
        self.svg_artifact_receipt.as_ref()
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

impl SvgOutput {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    /// Returns the target-owned admission bound to these exact SVG bytes.
    ///
    /// Every successful standalone SVG has a receipt. `BestEffort` returns the receipt even when
    /// its status is host-dependent or rejected, while `RequirePortable` rejects such output with
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

pub(super) fn finish_standalone_svg_target(
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

#[cfg(test)]
mod tests {
    use merman_core::OperationControl;

    use super::super::target_admission::{
        standalone_target_evidence_digest, target_admission_receipt_digest,
    };
    use super::super::{RenderOutput, RenderRequest, Renderer, SvgRequest, TargetFontSource};

    #[test]
    fn standalone_target_evidence_digest_binds_font_source() {
        let output = Renderer::new()
            .render(RenderRequest::document(
                "info",
                OperationControl::new(),
                SvgRequest::default(),
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
                document.portability(),
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
