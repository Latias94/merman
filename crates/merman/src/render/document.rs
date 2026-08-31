use std::sync::Arc;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_core::OperationControl;
#[cfg(all(feature = "png", feature = "internal-theme-acceptance"))]
use sha2::{Digest as _, Sha256};

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

#[cfg(all(feature = "png", feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone)]
pub struct ThemeRoutePngCutoverPair {
    solid: RasterOutput,
    transparent: RasterOutput,
    receipt: ThemeRoutePngCutoverReceipt,
}

#[cfg(all(feature = "png", feature = "internal-theme-acceptance"))]
impl ThemeRoutePngCutoverPair {
    pub(crate) const fn solid(&self) -> &RasterOutput {
        &self.solid
    }

    pub(crate) const fn transparent(&self) -> &RasterOutput {
        &self.transparent
    }

    pub(crate) const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt.digest()
    }
}

#[cfg(all(feature = "png", feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThemeRoutePngCutoverReceipt {
    solid_route_receipt_digest: [u8; 32],
    transparent_route_receipt_digest: [u8; 32],
    solid_svg_artifact_digest: [u8; 32],
    transparent_svg_artifact_digest: [u8; 32],
    solid_png_receipt_digest: [u8; 32],
    transparent_png_receipt_digest: [u8; 32],
    raster_receipt_digest: [u8; 32],
    digest: [u8; 32],
}

#[cfg(all(feature = "png", feature = "internal-theme-acceptance"))]
impl ThemeRoutePngCutoverReceipt {
    fn seal(
        solid_route_receipt_digest: [u8; 32],
        transparent_route_receipt_digest: [u8; 32],
        solid_svg_artifact_digest: [u8; 32],
        transparent_svg_artifact_digest: [u8; 32],
        solid_png_receipt_digest: [u8; 32],
        transparent_png_receipt_digest: [u8; 32],
        raster_receipt_digest: [u8; 32],
        control_css: &str,
    ) -> Option<Self> {
        let facts = [
            solid_route_receipt_digest,
            transparent_route_receipt_digest,
            solid_svg_artifact_digest,
            transparent_svg_artifact_digest,
            solid_png_receipt_digest,
            transparent_png_receipt_digest,
            raster_receipt_digest,
        ];
        if facts.iter().any(|digest| *digest == [0; 32]) || control_css.is_empty() {
            return None;
        }
        let mut hasher = Sha256::new();
        hasher.update(b"merman.theme-route-png-cutover-receipt.v1\0");
        for digest in facts {
            hasher.update(digest);
        }
        hasher.update((control_css.len() as u64).to_be_bytes());
        hasher.update(control_css.as_bytes());
        let digest = hasher.finalize().into();
        Some(Self {
            solid_route_receipt_digest,
            transparent_route_receipt_digest,
            solid_svg_artifact_digest,
            transparent_svg_artifact_digest,
            solid_png_receipt_digest,
            transparent_png_receipt_digest,
            raster_receipt_digest,
            digest,
        })
    }

    const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

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
    theme_raster_paint_binding_receipts:
        Box<[merman_render::__private::ThemeRasterPaintBindingReceipt]>,
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
        required_capabilities: Vec<merman_render::RenderCapability>,
    ) -> Self {
        let svg = merman_render::svg::StandaloneSvgArtifact::from(svg);
        let public_svg_digest = artifact_digest(svg.as_str().as_bytes());
        let native_svg = svg.native_export_svg();
        let native_svg_digest = if native_svg == svg.as_str() {
            public_svg_digest
        } else {
            artifact_digest(native_svg.as_bytes())
        };
        #[cfg(feature = "internal-theme-acceptance")]
        let theme_route_cutover_receipts =
            merman_render::__private::seal_theme_route_cutover_receipts(
                &family,
                public_svg_digest,
                native_svg_digest,
            )
            .into_boxed_slice();
        #[cfg(feature = "internal-theme-acceptance")]
        let theme_raster_paint_binding_receipts =
            merman_render::__private::seal_theme_raster_paint_binding_receipts(
                &family,
                native_svg_digest,
            )
            .into_boxed_slice();
        let evidence = Arc::new(RenderEvidence::from_family(family, required_capabilities));
        let portability = document_portability_report(&svg, &evidence);
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
            theme_raster_paint_binding_receipts,
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
    pub(crate) fn theme_raster_paint_binding_receipts(
        &self,
    ) -> &[merman_render::__private::ThemeRasterPaintBindingReceipt] {
        &self.theme_raster_paint_binding_receipts
    }

    #[cfg(feature = "internal-theme-acceptance")]
    pub(crate) fn svg_artifact_receipt(
        &self,
    ) -> Option<&merman_render::__private::SvgArtifactReceipt> {
        self.svg.svg_artifact_receipt()
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
    pub(crate) fn finish_raster_export(
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

    #[cfg(all(feature = "png", feature = "internal-theme-acceptance"))]
    pub(crate) fn export_theme_route_cutover_png_pair(
        solid_document: &RenderedDocument,
        transparent_document: &RenderedDocument,
        solid_route: merman_render::__private::ThemeRouteCutoverDescriptor,
        transparent_route: merman_render::__private::ThemeRouteCutoverDescriptor,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<ThemeRoutePngCutoverPair, RenderError> {
        use merman_render::__private::{ThemeRouteCutoverFacet, ThemeRouteCutoverValue};

        let same_route_except_value = solid_route.family_id() == transparent_route.family_id()
            && solid_route.target() == transparent_route.target()
            && solid_route.selector() == transparent_route.selector()
            && solid_route.facet() == transparent_route.facet()
            && solid_route.projections() == transparent_route.projections();
        if !same_route_except_value
            || solid_route.value() != ThemeRouteCutoverValue::Solid
            || transparent_route.value() != ThemeRouteCutoverValue::Transparent
        {
            return Err(map_export_error(
                merman_export::ExportError::RasterPaintCutover(
                    "solid and transparent route descriptors are not one value pair",
                ),
            ));
        }

        let solid_svg_artifact_digest = solid_document.standalone_svg_admission.artifact_digest();
        let transparent_svg_artifact_digest = transparent_document
            .standalone_svg_admission
            .artifact_digest();
        let solid_route_receipt = solid_document
            .theme_route_cutover_receipts()
            .iter()
            .find(|receipt| receipt.descriptor() == solid_route)
            .copied();
        let transparent_route_receipt = transparent_document
            .theme_route_cutover_receipts()
            .iter()
            .find(|receipt| receipt.descriptor() == transparent_route)
            .copied();
        let (Some(solid_route_receipt), Some(transparent_route_receipt)) =
            (solid_route_receipt, transparent_route_receipt)
        else {
            return Err(map_export_error(
                merman_export::ExportError::RasterPaintCutover(
                    "renderer did not seal both route receipts",
                ),
            ));
        };
        let solid_native_svg_digest = artifact_digest(
            merman_render::__private::native_export_svg(solid_document.sealed_svg()).as_bytes(),
        );
        let transparent_native_svg_digest = artifact_digest(
            merman_render::__private::native_export_svg(transparent_document.sealed_svg())
                .as_bytes(),
        );
        if !solid_route_receipt.proves_artifacts(solid_svg_artifact_digest, solid_native_svg_digest)
            || !transparent_route_receipt.proves_artifacts(
                transparent_svg_artifact_digest,
                transparent_native_svg_digest,
            )
        {
            return Err(map_export_error(
                merman_export::ExportError::RasterPaintCutover(
                    "route receipt is not bound to its finalized SVG artifact",
                ),
            ));
        }

        let renderer_bindings = |document: &RenderedDocument,
                                 route: merman_render::__private::ThemeRouteCutoverDescriptor,
                                 native_digest: [u8; 32]| {
            let matching = document
                .theme_raster_paint_binding_receipts()
                .iter()
                .filter(|receipt| receipt.matches_route_identity(route))
                .collect::<Vec<_>>();
            if matching.len() > 1 {
                return Err(map_export_error(
                    merman_export::ExportError::RasterPaintCutover(
                        "renderer sealed duplicate raster terminal routes",
                    ),
                ));
            }
            let Some(receipt) = matching.into_iter().next() else {
                if route.requires_renderer_raster_binding_receipt() {
                    return Err(map_export_error(
                        merman_export::ExportError::RasterPaintCutover(
                            "renderer did not seal the required raster terminal route",
                        ),
                    ));
                }
                return Ok(Vec::new());
            };
            if !receipt.proves_route(route, native_digest) {
                return Err(map_export_error(
                    merman_export::ExportError::RasterPaintCutover(
                        "renderer raster terminal receipt is not bound to its finalized SVG",
                    ),
                ));
            }
            receipt
                .terminals()
                .iter()
                .map(|terminal| {
                    let binding = match terminal.binding() {
                        merman_render::__private::ThemeRasterPaintBinding::Native => {
                            merman_export::RasterPaintSemanticBinding::Native
                        }
                        merman_render::__private::ThemeRasterPaintBinding::FillFromStroke => {
                            merman_export::RasterPaintSemanticBinding::FillFromStroke
                        }
                        merman_render::__private::ThemeRasterPaintBinding::FillAndStrokeFromStroke => {
                            merman_export::RasterPaintSemanticBinding::FillAndStrokeFromStroke
                        }
                    };
                    let (x1, y1, x2, y2, authored_stroke_width, effective_stroke_width) =
                        match terminal.semantic() {
                        merman_render::__private::ThemeRasterPaintTerminalSemantic::SequenceLifeline {
                            geometry,
                        } => (
                            geometry.x1(),
                            geometry.y1(),
                            geometry.x2(),
                            geometry.y2(),
                            geometry.authored_stroke_width(),
                            geometry.effective_stroke_width(),
                        ),
                    };
                    merman_export::RasterPaintTerminalBinding::line_lifeline_with_effective_width(
                        terminal.terminal_id(),
                        binding,
                        x1,
                        y1,
                        x2,
                        y2,
                        authored_stroke_width,
                        effective_stroke_width,
                    )
                    .ok_or_else(|| {
                        map_export_error(merman_export::ExportError::RasterPaintCutover(
                            "renderer emitted an empty raster terminal id",
                        ))
                    })
                })
                .collect::<Result<Vec<_>, RenderError>>()
        };
        let solid_bindings =
            renderer_bindings(solid_document, solid_route, solid_native_svg_digest)?;
        let transparent_bindings = renderer_bindings(
            transparent_document,
            transparent_route,
            transparent_native_svg_digest,
        )?;
        if solid_bindings != transparent_bindings {
            return Err(map_export_error(
                merman_export::ExportError::RasterPaintCutover(
                    "solid and transparent renderer terminal bindings differ",
                ),
            ));
        }

        let facet = match solid_route.facet() {
            ThemeRouteCutoverFacet::Fill => merman_export::RasterPaintCutoverFacet::Fill,
            ThemeRouteCutoverFacet::Stroke => merman_export::RasterPaintCutoverFacet::Stroke,
        };
        let control_css = solid_route.raster_control_css();
        let pair = merman_export::encode_png_paint_cutover_pair_controlled(
            solid_document.sealed_svg(),
            transparent_document.sealed_svg(),
            options,
            control,
            facet,
            &control_css,
            &solid_bindings,
        )
        .map_err(map_export_error)?;
        let ((solid_bytes, solid_report), (transparent_bytes, transparent_report), raster_receipt) =
            pair.into_parts();
        let raster_receipt_digest = raster_receipt.digest();
        let solid = solid_document.finish_raster_export(solid_bytes, solid_report)?;
        let transparent =
            transparent_document.finish_raster_export(transparent_bytes, transparent_report)?;
        let receipt = ThemeRoutePngCutoverReceipt::seal(
            solid_route_receipt.digest(),
            transparent_route_receipt.digest(),
            solid_svg_artifact_digest,
            transparent_svg_artifact_digest,
            solid.admission().receipt_digest(),
            transparent.admission().receipt_digest(),
            raster_receipt_digest,
            &control_css,
        )
        .ok_or_else(|| {
            map_export_error(merman_export::ExportError::RasterPaintCutover(
                "route-local PNG receipt facts are incomplete",
            ))
        })?;

        Ok(ThemeRoutePngCutoverPair {
            solid,
            transparent,
            receipt,
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
    required_capabilities: Vec<merman_render::RenderCapability>,
) -> Result<SvgOutput, RenderError> {
    let evidence = RenderEvidence::from_family(family, required_capabilities);
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
