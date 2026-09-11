#[cfg(all(feature = "png", merman_internal_theme_acceptance))]
pub struct ThemeRoutePngCutoverPairView {
    inner: crate::render::ThemeRoutePngCutoverPair,
}

#[cfg(all(feature = "png", merman_internal_theme_acceptance))]
impl ThemeRoutePngCutoverPairView {
    pub fn solid(&self) -> TargetArtifactView<'_> {
        TargetArtifactView::from_raster_output(self.inner.solid())
    }

    pub fn transparent(&self) -> TargetArtifactView<'_> {
        TargetArtifactView::from_raster_output(self.inner.transparent())
    }

    pub const fn receipt_digest(&self) -> [u8; 32] {
        self.inner.receipt_digest()
    }
}

#[cfg(all(feature = "png", merman_internal_theme_acceptance))]
pub fn export_theme_route_cutover_png_pair(
    solid_document: &crate::RenderedDocument,
    transparent_document: &crate::RenderedDocument,
    solid_route: merman_render::__private::ThemeRouteCutoverDescriptor,
    transparent_route: merman_render::__private::ThemeRouteCutoverDescriptor,
    options: &merman_export::RasterOptions,
    control: merman_core::OperationControl,
) -> Result<ThemeRoutePngCutoverPairView, crate::RenderError> {
    crate::render::RenderedDocument::export_theme_route_cutover_png_pair(
        solid_document,
        transparent_document,
        solid_route,
        transparent_route,
        options,
        control,
    )
    .map(|inner| ThemeRoutePngCutoverPairView { inner })
}

/// Returns the renderer-owned route and dispatch inventory for the legacy family bridge.
#[cfg(merman_internal_theme_acceptance)]
pub fn legacy_family_theme_bridge_inventory()
-> merman_render::__private::LegacyFamilyThemeBridgeInventory {
    merman_render::__private::legacy_family_theme_bridge_inventory()
}

/// Borrowed production-owned artifact and its inseparable target admission receipt.
///
/// The acceptance harness may inspect the exact bytes, but it cannot pair arbitrary bytes with
/// a receipt or reconstruct the production target seal. A finalized document view also carries
/// the generic SVG observation. Raster and PDF views intentionally expose only their own
/// target receipt; callers that need source-document SVG facts must retain the originating
/// `RenderedDocument` view.
#[derive(Debug, Clone, Copy)]
pub struct TargetArtifactView<'a> {
    bytes: &'a [u8],
    receipt: &'a crate::TargetAdmissionReceipt,
    svg_artifact_receipt: Option<&'a merman_render::__private::SvgArtifactReceipt>,
}

impl<'a> TargetArtifactView<'a> {
    pub fn from_rendered_document(document: &'a crate::RenderedDocument) -> Self {
        Self {
            bytes: document.svg().as_bytes(),
            receipt: document.standalone_svg_admission(),
            svg_artifact_receipt: document.svg_artifact_receipt(),
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub fn from_raster_output(output: &'a crate::RasterOutput) -> Self {
        Self {
            bytes: output.bytes(),
            receipt: output.admission(),
            svg_artifact_receipt: None,
        }
    }

    #[cfg(feature = "pdf")]
    pub fn from_pdf_output(output: &'a crate::PdfOutput) -> Self {
        Self {
            bytes: output.bytes(),
            receipt: output.admission(),
            svg_artifact_receipt: None,
        }
    }

    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub const fn receipt(self) -> &'a crate::TargetAdmissionReceipt {
        self.receipt
    }

    pub const fn svg_artifact_receipt(
        self,
    ) -> Option<&'a merman_render::__private::SvgArtifactReceipt> {
        self.svg_artifact_receipt
    }

    pub const fn renderer_receipt_digest(self) -> [u8; 32] {
        match self.svg_artifact_receipt {
            Some(receipt) => receipt.digest(),
            None => self.receipt.receipt_digest(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeAcceptanceScopeEvidence {
    projection: crate::render::ThemeEvidenceScopeProjection,
}

impl ThemeAcceptanceScopeEvidence {
    pub const fn status(&self) -> crate::ThemeEvidenceStatus {
        self.projection.status
    }

    pub const fn required_count(&self) -> usize {
        self.projection.required_count
    }

    pub const fn accounted_count(&self) -> usize {
        self.projection.accounted_count
    }

    pub const fn applied_count(&self) -> usize {
        self.projection.applied_count
    }

    pub const fn not_applicable_count(&self) -> usize {
        self.projection.not_applicable_count
    }

    pub const fn incomplete_count(&self) -> usize {
        self.projection.incomplete_count()
    }

    pub const fn residual_count(&self) -> usize {
        self.projection.residual_count
    }

    pub const fn output_mutated(&self) -> bool {
        self.projection.output_mutated
    }

    pub const fn is_verified(&self) -> bool {
        matches!(self.projection.status, crate::ThemeEvidenceStatus::Verified)
            && self.projection.is_satisfied()
    }

    pub const fn is_satisfied(&self) -> bool {
        self.projection.is_satisfied()
    }
}

pub struct ThemeAcceptanceEvidence<'a> {
    projection: crate::render::ThemeAcceptanceEvidenceProjection<'a>,
}

impl<'a> ThemeAcceptanceEvidence<'a> {
    pub const fn root(&self) -> ThemeAcceptanceScopeEvidence {
        ThemeAcceptanceScopeEvidence {
            projection: self.projection.root,
        }
    }

    pub const fn family(&self) -> ThemeAcceptanceScopeEvidence {
        ThemeAcceptanceScopeEvidence {
            projection: self.projection.family,
        }
    }

    pub const fn source_residual_count(&self) -> usize {
        self.projection.source_residual_count
    }

    pub const fn compatibility_residual_count(&self) -> usize {
        self.projection.compatibility_residual_count
    }

    pub const fn mermaid_compatibility_residual_count(&self) -> usize {
        self.projection.mermaid_compatibility_residual_count
    }

    pub const fn recipe_report(
        &self,
    ) -> Option<&'a merman_render::diagram_theme::ThemeRecipeReport> {
        self.projection.recipe_report
    }

    pub const fn host_admission_report(
        &self,
    ) -> Option<&'a merman_render::diagram_theme::ThemeHostAdmissionReport> {
        self.projection.host_admission_report
    }

    pub const fn portability_requirement(
        &self,
    ) -> Option<merman_render::diagram_theme::ThemePortabilityRequirement> {
        match self.projection.host_admission_report {
            Some(report) => Some(report.portability_requirement()),
            None => None,
        }
    }

    pub const fn prepared_text_layout(
        &self,
    ) -> Option<&'a merman_render::text::PreparedTextLayoutReport> {
        self.projection.prepared_text_layout
    }

    pub const fn text_layout_failure(&self) -> Option<merman_render::text::TextLayoutFailure> {
        self.projection.text_layout_failure
    }

    pub const fn effective_theme_resource_policy(
        &self,
    ) -> &'a merman_render::diagram_theme::ThemeResourcePolicy {
        self.projection.effective_theme_resource_policy
    }
}

pub fn theme_acceptance_evidence(evidence: &crate::RenderEvidence) -> ThemeAcceptanceEvidence<'_> {
    ThemeAcceptanceEvidence {
        projection: evidence.theme_acceptance_evidence(),
    }
}

pub fn native_filter_receipt(
    evidence: &crate::RenderEvidence,
) -> Option<merman_render::__private::NativeSvgFilterReceipt> {
    evidence.native_filter_receipt()
}

#[cfg(feature = "layout-cytoscape")]
pub fn architecture_text_cutover_receipt(
    evidence: &crate::RenderEvidence,
) -> Option<&merman_render::__private::ArchitectureTextCutoverReceipt> {
    evidence.architecture_text_cutover_receipt()
}

/// Returns the opaque renderer-owned route receipts for one finalized document.
pub fn theme_route_cutover_receipts(
    document: &crate::RenderedDocument,
) -> &[merman_render::__private::ThemeRouteCutoverReceipt] {
    document.theme_route_cutover_receipts()
}
