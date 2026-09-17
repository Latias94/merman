mod digest;

use super::{RenderEvidence, ThemeEvidenceSummary};
#[cfg(feature = "pdf")]
use digest::pdf_target_evidence_digest;
#[cfg(any(feature = "png", feature = "jpeg"))]
use digest::raster_target_evidence_digest;
pub(super) use digest::{
    artifact_digest, document_digest, standalone_target_evidence_digest,
    target_admission_receipt_digest,
};

/// Artifact kind covered by one target-owned admission receipt.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RenderArtifactKind {
    Svg,
    Png,
    Jpeg,
    Pdf,
}

#[cfg(feature = "svg")]
impl RenderArtifactKind {
    /// Complete target vocabulary for this API revision, independent of enabled Cargo features.
    pub const ALL: &'static [Self] = &[Self::Svg, Self::Png, Self::Jpeg, Self::Pdf];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Pdf => "pdf",
        }
    }
}

/// Terminal portability classification produced by the target adapter itself.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetAdmissionStatus {
    /// This operation did not request target compatibility observation.
    Unverified,
    Portable,
    HostDependent,
    Rejected,
}

#[cfg(feature = "svg")]
impl TargetAdmissionStatus {
    pub const ALL: &'static [Self] = &[
        Self::Unverified,
        Self::Portable,
        Self::HostDependent,
        Self::Rejected,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::Portable => "portable",
            Self::HostDependent => "host_dependent",
            Self::Rejected => "rejected",
        }
    }

    pub const fn is_portable(self) -> bool {
        matches!(self, Self::Portable)
    }
}

/// Font-source summary observed by one concrete output target.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetFontSource {
    None,
    Embedded,
    System,
    Mixed,
}

#[cfg(feature = "svg")]
impl TargetFontSource {
    pub const ALL: &'static [Self] = &[Self::None, Self::Embedded, Self::System, Self::Mixed];

    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Embedded => "embedded",
            Self::System => "system",
            Self::Mixed => "mixed",
        }
    }
}

/// Machine-readable reason attached to document and target admission evidence.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetAdmissionReason {
    ThemeEvidenceIncomplete,
    PreparedTextLayoutFailed,
    HostDependentTextLayout,
    PreparedTextEvidenceInvalid,
    PreparedMathEvidenceInvalid,
    PreparedMathTerminalProofIncomplete,
    SvgFontsNotSelfContained,
    SvgTerminalValidationFailed,
    SvgTerminalUnverified,
    PreparedTextEvidenceMismatch,
    PreparedTextTerminalProofIncomplete,
    FontResolutionIncomplete,
    SystemOrHostFontDependency,
    ResourceFingerprintMismatch,
    FontCatalogFingerprintMismatch,
    TargetKindMismatch,
    NativeRootCapabilityUnsupported,
    NativeFilterReceiptMismatch,
    PdfNativeFilterNotLocalized,
}

#[cfg(feature = "svg")]
impl TargetAdmissionReason {
    pub const ALL: &'static [Self] = &[
        Self::ThemeEvidenceIncomplete,
        Self::PreparedTextLayoutFailed,
        Self::HostDependentTextLayout,
        Self::PreparedTextEvidenceInvalid,
        Self::PreparedMathEvidenceInvalid,
        Self::PreparedMathTerminalProofIncomplete,
        Self::SvgFontsNotSelfContained,
        Self::SvgTerminalValidationFailed,
        Self::SvgTerminalUnverified,
        Self::PreparedTextEvidenceMismatch,
        Self::PreparedTextTerminalProofIncomplete,
        Self::FontResolutionIncomplete,
        Self::SystemOrHostFontDependency,
        Self::ResourceFingerprintMismatch,
        Self::FontCatalogFingerprintMismatch,
        Self::TargetKindMismatch,
        Self::NativeRootCapabilityUnsupported,
        Self::NativeFilterReceiptMismatch,
        Self::PdfNativeFilterNotLocalized,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::ThemeEvidenceIncomplete => "theme_evidence_incomplete",
            Self::PreparedTextLayoutFailed => "prepared_text_layout_failed",
            Self::HostDependentTextLayout => "host_dependent_text_layout",
            Self::PreparedTextEvidenceInvalid => "prepared_text_evidence_invalid",
            Self::PreparedMathEvidenceInvalid => "prepared_math_evidence_invalid",
            Self::PreparedMathTerminalProofIncomplete => "prepared_math_terminal_proof_incomplete",
            Self::SvgFontsNotSelfContained => "svg_fonts_not_self_contained",
            Self::SvgTerminalValidationFailed => "svg_terminal_validation_failed",
            Self::SvgTerminalUnverified => "svg_terminal_unverified",
            Self::PreparedTextEvidenceMismatch => "prepared_text_evidence_mismatch",
            Self::PreparedTextTerminalProofIncomplete => "prepared_text_terminal_proof_incomplete",
            Self::FontResolutionIncomplete => "font_resolution_incomplete",
            Self::SystemOrHostFontDependency => "system_or_host_font_dependency",
            Self::ResourceFingerprintMismatch => "resource_fingerprint_mismatch",
            Self::FontCatalogFingerprintMismatch => "font_catalog_fingerprint_mismatch",
            Self::TargetKindMismatch => "target_kind_mismatch",
            Self::NativeRootCapabilityUnsupported => "native_root_capability_unsupported",
            Self::NativeFilterReceiptMismatch => "native_filter_receipt_mismatch",
            Self::PdfNativeFilterNotLocalized => "pdf_native_filter_not_localized",
        }
    }
}

/// Coarse document-stage evidence frozen before any target-specific export.
///
/// This report deliberately does not use [`TargetAdmissionStatus`]. Document proof and target
/// admission are separate monotonic stages: targets may add constraints, but cannot erase a
/// document rejection or host dependency.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPortabilityReport {
    family_id: merman_core::DiagramFamilyId,
    theme_evidence: ThemeEvidenceSummary,
    resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
    font_catalog_fingerprint: merman_render::diagram_theme::FontCatalogFingerprint,
    resource_closure_complete: bool,
    prepared_text_evidence_valid: bool,
    prepared_math_evidence: merman_render::__private::PreparedMathEvidenceSummary,
    rejected: bool,
    host_dependent: bool,
    reasons: Box<[TargetAdmissionReason]>,
}

#[cfg(feature = "svg")]
impl DocumentPortabilityReport {
    pub const fn family_id(&self) -> merman_core::DiagramFamilyId {
        self.family_id
    }

    pub const fn theme_evidence(&self) -> ThemeEvidenceSummary {
        self.theme_evidence
    }

    pub const fn resource_fingerprint(&self) -> merman_render::svg::SvgResourceFingerprint {
        self.resource_fingerprint
    }

    pub const fn font_catalog_fingerprint(
        &self,
    ) -> merman_render::diagram_theme::FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub const fn resource_closure_complete(&self) -> bool {
        self.resource_closure_complete
    }

    pub const fn prepared_text_evidence_valid(&self) -> bool {
        self.prepared_text_evidence_valid
    }

    pub const fn prepared_math_evidence_valid(&self) -> bool {
        self.prepared_math_evidence.evidence_valid()
    }

    pub const fn prepared_math_terminal_proof_complete(&self) -> bool {
        self.prepared_math_evidence.terminal_proof_complete()
    }

    pub(crate) const fn prepared_math_evidence(
        &self,
    ) -> merman_render::__private::PreparedMathEvidenceSummary {
        self.prepared_math_evidence
    }

    pub const fn is_host_dependent(&self) -> bool {
        self.host_dependent
    }

    /// Returns whether the document-stage evidence is internally valid.
    ///
    /// A valid document may still depend on host fonts or measurement. Use
    /// [`Self::is_host_dependent`] and the target admission receipt when deciding whether an
    /// artifact is portable.
    pub const fn is_evidence_valid(&self) -> bool {
        self.resource_closure_complete
            && self.prepared_text_evidence_valid
            && self.prepared_math_evidence.terminal_proof_complete()
            && !self.rejected
    }

    pub fn reasons(&self) -> &[TargetAdmissionReason] {
        &self.reasons
    }
}

/// Immutable target-owned admission bound to one completed document and exact artifact bytes.
///
/// This type deliberately has no public constructor or `from_parts` API. Binary output types keep
/// the receipt next to immutable bytes; consuming `into_bytes()` is the explicit lossy escape hatch.
///
/// ```compile_fail
/// use merman::{RenderArtifactKind, TargetAdmissionReceipt, TargetAdmissionStatus};
///
/// let forged = TargetAdmissionReceipt {
///     artifact_kind: RenderArtifactKind::Svg,
///     status: TargetAdmissionStatus::Portable,
///     reasons: Box::new([]),
///     font_source: merman::TargetFontSource::Embedded,
///     resource_fingerprint: todo!(),
///     font_catalog_fingerprint: todo!(),
///     document_digest: [0; 32],
///     target_evidence_digest: [0; 32],
///     artifact_digest: [0; 32],
///     receipt_digest: [0; 32],
/// };
/// ```
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAdmissionReceipt {
    artifact_kind: RenderArtifactKind,
    status: TargetAdmissionStatus,
    reasons: Box<[TargetAdmissionReason]>,
    font_source: TargetFontSource,
    resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
    font_catalog_fingerprint: merman_render::diagram_theme::FontCatalogFingerprint,
    document_digest: [u8; 32],
    target_evidence_digest: [u8; 32],
    artifact_digest: [u8; 32],
    receipt_digest: [u8; 32],
}

#[cfg(feature = "svg")]
impl TargetAdmissionReceipt {
    #[allow(clippy::too_many_arguments)]
    fn seal(
        artifact_kind: RenderArtifactKind,
        status: TargetAdmissionStatus,
        reasons: Vec<TargetAdmissionReason>,
        font_source: TargetFontSource,
        resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
        font_catalog_fingerprint: merman_render::diagram_theme::FontCatalogFingerprint,
        document_digest: [u8; 32],
        target_evidence_digest: [u8; 32],
        artifact_digest: [u8; 32],
    ) -> Self {
        let reasons = reasons.into_boxed_slice();
        let receipt_digest = target_admission_receipt_digest(
            artifact_kind,
            status,
            &reasons,
            font_source,
            resource_fingerprint.as_bytes(),
            font_catalog_fingerprint.as_bytes(),
            document_digest,
            target_evidence_digest,
            artifact_digest,
        );
        Self {
            artifact_kind,
            status,
            reasons,
            font_source,
            resource_fingerprint,
            font_catalog_fingerprint,
            document_digest,
            target_evidence_digest,
            artifact_digest,
            receipt_digest,
        }
    }

    pub const fn artifact_kind(&self) -> RenderArtifactKind {
        self.artifact_kind
    }

    pub const fn status(&self) -> TargetAdmissionStatus {
        self.status
    }

    pub fn reasons(&self) -> &[TargetAdmissionReason] {
        &self.reasons
    }

    pub const fn font_source(&self) -> TargetFontSource {
        self.font_source
    }

    pub const fn resource_fingerprint(&self) -> merman_render::svg::SvgResourceFingerprint {
        self.resource_fingerprint
    }

    pub const fn font_catalog_fingerprint(
        &self,
    ) -> merman_render::diagram_theme::FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub const fn document_digest(&self) -> [u8; 32] {
        self.document_digest
    }

    /// Returns the experimental identity of the target evidence encoded by this receipt.
    ///
    /// This value is suitable for correlating artifacts produced by one Merman API revision. It
    /// is not a stable cross-release cache key until the target-evidence contract is frozen.
    pub const fn target_evidence_digest(&self) -> [u8; 32] {
        self.target_evidence_digest
    }

    pub const fn artifact_digest(&self) -> [u8; 32] {
        self.artifact_digest
    }

    /// Returns the revision-scoped canonical digest of this sealed target receipt.
    ///
    /// The digest binds artifact kind, status, ordered reasons, font source, resource and font
    /// catalog fingerprints, document identity, target evidence, and artifact identity. Its
    /// encoding remains experimental and may change between Merman API revisions.
    pub const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }
}

/// Strict-portability rejection returned when target-owned evidence is not Portable.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAdmissionError {
    receipt: TargetAdmissionReceipt,
}

#[cfg(feature = "svg")]
impl TargetAdmissionError {
    fn new(receipt: TargetAdmissionReceipt) -> Self {
        Self { receipt }
    }

    pub const fn receipt(&self) -> &TargetAdmissionReceipt {
        &self.receipt
    }
}

#[cfg(feature = "svg")]
impl std::fmt::Display for TargetAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} target requires Portable admission, got {} ({})",
            self.receipt.artifact_kind.id(),
            self.receipt.status.id(),
            self.receipt
                .reasons
                .iter()
                .map(|reason| reason.id())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[cfg(feature = "svg")]
impl std::error::Error for TargetAdmissionError {}

#[cfg(feature = "svg")]
fn classify_target_admission(rejected: bool, host_dependent: bool) -> TargetAdmissionStatus {
    if rejected {
        TargetAdmissionStatus::Rejected
    } else if host_dependent {
        TargetAdmissionStatus::HostDependent
    } else {
        TargetAdmissionStatus::Portable
    }
}

#[cfg(feature = "svg")]
fn classify_common_document_evidence(
    evidence: &RenderEvidence,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    let mut rejected = false;
    let mut host_dependent = false;
    if !evidence.theme_evidence().is_satisfied() {
        rejected = true;
        reasons.push(TargetAdmissionReason::ThemeEvidenceIncomplete);
    }
    if evidence.text_layout_failure().is_some() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextLayoutFailed);
    }
    let prepared_text_is_host_dependent = evidence
        .prepared_text_layout()
        .is_some_and(merman_render::text::PreparedTextLayoutReport::is_host_dependent);
    let actual_measurement_is_host_dependent =
        evidence.measurement().entries().iter().any(|entry| {
            entry.count() > 0
                && entry.provenance().source
                    == merman_render::environment::TextMeasurementSource::Host
        });
    if prepared_text_is_host_dependent || actual_measurement_is_host_dependent {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::HostDependentTextLayout);
    }
    (rejected, host_dependent)
}

#[cfg(feature = "svg")]
pub(super) fn document_portability_report(
    svg: &merman_render::svg::StandaloneSvgArtifact,
    evidence: &RenderEvidence,
) -> DocumentPortabilityReport {
    let mut reasons = Vec::new();
    let (mut rejected, host_dependent) = classify_common_document_evidence(evidence, &mut reasons);
    let resource_closure_complete = svg.finalization_report().is_some();
    match svg.terminal_status() {
        merman_render::svg::StandaloneSvgTerminalStatus::Unverified => {
            reasons.push(TargetAdmissionReason::SvgTerminalUnverified);
        }
        merman_render::svg::StandaloneSvgTerminalStatus::ValidationFailed => {
            rejected = true;
            reasons.push(TargetAdmissionReason::SvgTerminalValidationFailed);
        }
        merman_render::svg::StandaloneSvgTerminalStatus::Compatible => {}
    }
    let prepared_text_evidence_valid = svg.prepared_text_evidence_valid();
    if !prepared_text_evidence_valid {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextEvidenceInvalid);
    }
    let prepared_math_evidence = svg
        .as_resvg_compatible()
        .map(merman_render::__private::prepared_math_evidence)
        .unwrap_or_else(merman_render::__private::PreparedMathEvidenceSummary::not_applicable);
    if !prepared_math_evidence.evidence_valid() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedMathEvidenceInvalid);
    }
    if !prepared_math_evidence.terminal_proof_complete() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedMathTerminalProofIncomplete);
    }

    DocumentPortabilityReport {
        family_id: evidence.family_id(),
        theme_evidence: evidence.theme_evidence(),
        resource_fingerprint: svg.resource_fingerprint(),
        font_catalog_fingerprint: evidence.font_catalog_fingerprint(),
        resource_closure_complete,
        prepared_text_evidence_valid,
        prepared_math_evidence,
        rejected,
        host_dependent,
        reasons: reasons.into_boxed_slice(),
    }
}

#[cfg(feature = "svg")]
fn inherit_document_portability(
    report: &DocumentPortabilityReport,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    reasons.extend(report.reasons().iter().copied());
    (!report.is_evidence_valid(), report.is_host_dependent())
}

#[cfg(feature = "svg")]
fn prepared_text_font_source(evidence: &RenderEvidence) -> TargetFontSource {
    let Some(report) = evidence.prepared_text_layout() else {
        return TargetFontSource::None;
    };
    target_font_source(
        report.uses_font_source(merman_render::diagram_theme::FontSource::Embedded),
        report.uses_font_source(merman_render::diagram_theme::FontSource::System),
    )
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_font_source(fonts: merman_export::ExportFontPlan) -> TargetFontSource {
    target_font_source(fonts.used_embedded_fonts(), fonts.used_system_fonts())
}

#[cfg(feature = "svg")]
const fn target_font_source(embedded: bool, system: bool) -> TargetFontSource {
    match (embedded, system) {
        (false, false) => TargetFontSource::None,
        (true, false) => TargetFontSource::Embedded,
        (false, true) => TargetFontSource::System,
        (true, true) => TargetFontSource::Mixed,
    }
}

#[cfg(feature = "svg")]
pub(super) fn standalone_svg_admission(
    svg: &merman_render::svg::StandaloneSvgArtifact,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
    document_digest: [u8; 32],
    artifact_digest: [u8; 32],
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    reasons.extend(document.reasons().iter().copied());
    let rejected = document.rejected;
    let unverified =
        svg.terminal_status() == merman_render::svg::StandaloneSvgTerminalStatus::Unverified;
    let mut host_dependent = document.is_host_dependent();
    if !unverified && !svg.text_fonts_are_self_contained() {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::SvgFontsNotSelfContained);
    }

    let status = if !rejected && unverified {
        TargetAdmissionStatus::Unverified
    } else {
        classify_target_admission(rejected, host_dependent)
    };
    let font_source = prepared_text_font_source(evidence);
    let target_evidence_digest =
        standalone_target_evidence_digest(svg, evidence, document, status, &reasons, font_source);
    TargetAdmissionReceipt::seal(
        RenderArtifactKind::Svg,
        status,
        reasons,
        font_source,
        svg.resource_fingerprint(),
        evidence.font_catalog_fingerprint(),
        document_digest,
        target_evidence_digest,
        artifact_digest,
    )
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn classify_native_export_fonts(
    fonts: merman_export::ExportFontPlan,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    let mut rejected = false;
    let mut host_dependent = false;
    if !fonts.prepared_text_evidence_matches() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextEvidenceMismatch);
    }
    if !fonts.prepared_text_terminal_proof_complete() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextTerminalProofIncomplete);
    }
    if fonts.unresolved_font_request()
        || fonts.unresolved_glyph_fallback()
        || fonts.unclassified_face_count() != 0
        || fonts.notdef_glyph_count() != 0
    {
        rejected = true;
        reasons.push(TargetAdmissionReason::FontResolutionIncomplete);
    }
    if fonts.is_host_dependent() {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::SystemOrHostFontDependency);
    }
    (rejected, host_dependent)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf", test))]
const NATIVE_ROOT_CAPABILITY_WHITELIST: &[merman_render::diagram_theme::ThemeCapability] = &[
    merman_render::diagram_theme::ThemeCapability::SolidPaint,
    merman_render::diagram_theme::ThemeCapability::TransparentPaint,
    merman_render::diagram_theme::ThemeCapability::GradientPaint,
    merman_render::diagram_theme::ThemeCapability::LayeredCanvas,
    merman_render::diagram_theme::ThemeCapability::PatternPaint,
    merman_render::diagram_theme::ThemeCapability::CanvasLayerPlacement,
    merman_render::diagram_theme::ThemeCapability::BlendMode,
    merman_render::diagram_theme::ThemeCapability::Opacity,
];

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf", test))]
fn native_root_capabilities_are_supported(
    capabilities: impl IntoIterator<Item = merman_render::diagram_theme::ThemeCapability>,
) -> bool {
    capabilities.into_iter().all(|capability| {
        NATIVE_ROOT_CAPABILITY_WHITELIST
            .iter()
            .any(|allowed| *allowed == capability)
    })
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn classify_native_root_capabilities(
    evidence: &RenderEvidence,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> bool {
    if native_root_capabilities_are_supported(evidence.root_applied_capabilities().iter().copied())
    {
        return false;
    }
    reasons.push(TargetAdmissionReason::NativeRootCapabilityUnsupported);
    true
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub(super) fn native_raster_admission(
    artifact_kind: RenderArtifactKind,
    bytes: &[u8],
    report: merman_export::RasterExportReport,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
    document_digest: [u8; 32],
    expected_resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    let (mut rejected, mut host_dependent) = inherit_document_portability(document, &mut reasons);
    rejected |= classify_native_root_capabilities(evidence, &mut reasons);
    let (fonts_rejected, fonts_host_dependent) =
        classify_native_export_fonts(report.fonts(), &mut reasons);
    rejected |= fonts_rejected;
    host_dependent |= fonts_host_dependent;
    if report.resource_fingerprint() != expected_resource_fingerprint {
        rejected = true;
        reasons.push(TargetAdmissionReason::ResourceFingerprintMismatch);
    }
    if report.fonts().catalog_fingerprint() != evidence.font_catalog_fingerprint() {
        rejected = true;
        reasons.push(TargetAdmissionReason::FontCatalogFingerprintMismatch);
    }
    let report_matches_target = matches!(
        (artifact_kind, report.output()),
        (
            RenderArtifactKind::Png,
            merman_export::RasterOutputKind::Png
        ) | (
            RenderArtifactKind::Jpeg,
            merman_export::RasterOutputKind::Jpeg
        )
    );
    if !report_matches_target {
        rejected = true;
        reasons.push(TargetAdmissionReason::TargetKindMismatch);
    }
    if report.native_filter_receipt() != evidence.native_filter_receipt() {
        rejected = true;
        reasons.push(TargetAdmissionReason::NativeFilterReceiptMismatch);
    }
    let status = classify_target_admission(rejected, host_dependent);
    let target_evidence_digest = raster_target_evidence_digest(
        artifact_kind,
        report,
        evidence.native_filter_receipt(),
        evidence.root_applied_capabilities(),
        document.prepared_math_evidence(),
        status,
        &reasons,
    );
    TargetAdmissionReceipt::seal(
        artifact_kind,
        status,
        reasons,
        export_font_source(report.fonts()),
        report.resource_fingerprint(),
        report.fonts().catalog_fingerprint(),
        document_digest,
        target_evidence_digest,
        artifact_digest(bytes),
    )
}

#[cfg(feature = "pdf")]
pub(super) fn native_pdf_admission(
    bytes: &[u8],
    report: merman_export::PdfExportReport,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
    document_digest: [u8; 32],
    expected_resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    let (mut rejected, mut host_dependent) = inherit_document_portability(document, &mut reasons);
    rejected |= classify_native_root_capabilities(evidence, &mut reasons);
    let (fonts_rejected, fonts_host_dependent) =
        classify_native_export_fonts(report.fonts(), &mut reasons);
    rejected |= fonts_rejected;
    host_dependent |= fonts_host_dependent;
    if report.resource_fingerprint() != expected_resource_fingerprint {
        rejected = true;
        reasons.push(TargetAdmissionReason::ResourceFingerprintMismatch);
    }
    if report.fonts().catalog_fingerprint() != evidence.font_catalog_fingerprint() {
        rejected = true;
        reasons.push(TargetAdmissionReason::FontCatalogFingerprintMismatch);
    }
    let expected_native_filter = evidence.native_filter_receipt();
    if report.native_filter_receipt() != expected_native_filter {
        rejected = true;
        reasons.push(TargetAdmissionReason::NativeFilterReceiptMismatch);
    }
    if expected_native_filter.is_some() && !report.native_filter_fully_localized() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PdfNativeFilterNotLocalized);
    }
    let status = classify_target_admission(rejected, host_dependent);
    let target_evidence_digest = pdf_target_evidence_digest(
        report,
        expected_native_filter,
        evidence.root_applied_capabilities(),
        document.prepared_math_evidence(),
        status,
        &reasons,
    );
    TargetAdmissionReceipt::seal(
        RenderArtifactKind::Pdf,
        status,
        reasons,
        export_font_source(report.fonts()),
        report.resource_fingerprint(),
        report.fonts().catalog_fingerprint(),
        document_digest,
        target_evidence_digest,
        artifact_digest(bytes),
    )
}

#[cfg(feature = "svg")]
pub(super) fn enforce_portability_requirement(
    evidence: &RenderEvidence,
    receipt: &TargetAdmissionReceipt,
) -> Result<(), TargetAdmissionError> {
    if evidence.portability_requirement()
        == merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable
        && !receipt.status().is_portable()
    {
        return Err(TargetAdmissionError::new(receipt.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{TargetAdmissionReason, native_root_capabilities_are_supported};
    use merman_render::diagram_theme::ThemeCapability;

    #[test]
    fn native_root_capability_whitelist_is_explicit_and_fail_closed() {
        assert!(native_root_capabilities_are_supported([
            ThemeCapability::SolidPaint,
            ThemeCapability::TransparentPaint,
            ThemeCapability::GradientPaint,
            ThemeCapability::PatternPaint,
            ThemeCapability::LayeredCanvas,
            ThemeCapability::CanvasLayerPlacement,
            ThemeCapability::BlendMode,
            ThemeCapability::Opacity,
        ]));
        for unsupported in [
            ThemeCapability::CanvasBleed,
            ThemeCapability::SemanticRules,
            ThemeCapability::Shadow,
            ThemeCapability::SvgFilter,
        ] {
            assert!(
                !native_root_capabilities_are_supported([unsupported]),
                "native root admission must reject {unsupported} until explicitly qualified"
            );
        }
    }

    #[test]
    fn target_admission_reason_exposes_the_complete_alpha_catalog() {
        assert_eq!(
            TargetAdmissionReason::ALL,
            &[
                TargetAdmissionReason::ThemeEvidenceIncomplete,
                TargetAdmissionReason::PreparedTextLayoutFailed,
                TargetAdmissionReason::HostDependentTextLayout,
                TargetAdmissionReason::PreparedTextEvidenceInvalid,
                TargetAdmissionReason::PreparedMathEvidenceInvalid,
                TargetAdmissionReason::PreparedMathTerminalProofIncomplete,
                TargetAdmissionReason::SvgFontsNotSelfContained,
                TargetAdmissionReason::SvgTerminalValidationFailed,
                TargetAdmissionReason::SvgTerminalUnverified,
                TargetAdmissionReason::PreparedTextEvidenceMismatch,
                TargetAdmissionReason::PreparedTextTerminalProofIncomplete,
                TargetAdmissionReason::FontResolutionIncomplete,
                TargetAdmissionReason::SystemOrHostFontDependency,
                TargetAdmissionReason::ResourceFingerprintMismatch,
                TargetAdmissionReason::FontCatalogFingerprintMismatch,
                TargetAdmissionReason::TargetKindMismatch,
                TargetAdmissionReason::NativeRootCapabilityUnsupported,
                TargetAdmissionReason::NativeFilterReceiptMismatch,
                TargetAdmissionReason::PdfNativeFilterNotLocalized,
            ]
        );
    }
}
