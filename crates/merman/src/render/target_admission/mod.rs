use super::{RenderEvidence, ThemeEvidenceSummary};

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
    System,
    /// Caller-owned native font context; not a theme-retained or ambient-system classification.
    NativeContext,
}

#[cfg(feature = "svg")]
impl TargetFontSource {
    pub const ALL: &'static [Self] = &[Self::None, Self::System, Self::NativeContext];

    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::System => "system",
            Self::NativeContext => "native-context",
        }
    }
}

/// Machine-readable reason attached to document and target admission evidence.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetAdmissionReason {
    ThemeEvidenceIncomplete,
    HostDependentTextLayout,
    PreparedMathEvidenceInvalid,
    PreparedMathTerminalProofIncomplete,
    SvgFontsNotSelfContained,
    SvgTerminalValidationFailed,
    SvgTerminalUnverified,
    FontResolutionIncomplete,
    SystemOrHostFontDependency,
    NativeRootCapabilityUnsupported,
    NativeFilterReceiptMismatch,
    PdfNativeFilterNotLocalized,
    /// Native filter offsets can differ under a local rotation, shear or reflection.
    NativeDirectionalFilterTransformResidual,
    /// An inferred PDF viewport can clip painted content without a source clipping decision.
    PdfImplicitViewportClippingResidual,
    /// Converted HTML label paint can differ under source CSS. This is not a safety receipt.
    FallbackCssResidual,
}

#[cfg(feature = "svg")]
impl TargetAdmissionReason {
    pub const ALL: &'static [Self] = &[
        Self::ThemeEvidenceIncomplete,
        Self::HostDependentTextLayout,
        Self::PreparedMathEvidenceInvalid,
        Self::PreparedMathTerminalProofIncomplete,
        Self::SvgFontsNotSelfContained,
        Self::SvgTerminalValidationFailed,
        Self::SvgTerminalUnverified,
        Self::FontResolutionIncomplete,
        Self::SystemOrHostFontDependency,
        Self::NativeRootCapabilityUnsupported,
        Self::NativeFilterReceiptMismatch,
        Self::PdfNativeFilterNotLocalized,
        Self::NativeDirectionalFilterTransformResidual,
        Self::PdfImplicitViewportClippingResidual,
        Self::FallbackCssResidual,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::ThemeEvidenceIncomplete => "theme_evidence_incomplete",
            Self::HostDependentTextLayout => "host_dependent_text_layout",
            Self::PreparedMathEvidenceInvalid => "prepared_math_evidence_invalid",
            Self::PreparedMathTerminalProofIncomplete => "prepared_math_terminal_proof_incomplete",
            Self::SvgFontsNotSelfContained => "svg_fonts_not_self_contained",
            Self::SvgTerminalValidationFailed => "svg_terminal_validation_failed",
            Self::SvgTerminalUnverified => "svg_terminal_unverified",
            Self::FontResolutionIncomplete => "font_resolution_incomplete",
            Self::SystemOrHostFontDependency => "system_or_host_font_dependency",
            Self::NativeRootCapabilityUnsupported => "native_root_capability_unsupported",
            Self::NativeFilterReceiptMismatch => "native_filter_receipt_mismatch",
            Self::PdfNativeFilterNotLocalized => "pdf_native_filter_not_localized",
            Self::NativeDirectionalFilterTransformResidual => {
                "native_directional_filter_transform_residual"
            }
            Self::PdfImplicitViewportClippingResidual => "pdf_implicit_viewport_clipping_residual",
            Self::FallbackCssResidual => "fallback_css_residual",
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
    resource_closure_complete: bool,
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

    pub const fn resource_closure_complete(&self) -> bool {
        self.resource_closure_complete
    }

    pub const fn prepared_math_evidence_valid(&self) -> bool {
        self.prepared_math_evidence.evidence_valid()
    }

    pub const fn prepared_math_terminal_proof_complete(&self) -> bool {
        self.prepared_math_evidence.terminal_proof_complete()
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
            && self.prepared_math_evidence.terminal_proof_complete()
            && !self.rejected
    }

    pub fn reasons(&self) -> &[TargetAdmissionReason] {
        &self.reasons
    }
}

/// Target-local outcome of the completed export.
///
/// Output objects retain this result beside their immutable bytes. Content identities, when needed,
/// are computed by the consumer that observes or persists those bytes.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAdmissionReceipt {
    artifact_kind: RenderArtifactKind,
    status: TargetAdmissionStatus,
    reasons: Box<[TargetAdmissionReason]>,
    font_source: TargetFontSource,
}

#[cfg(feature = "svg")]
impl TargetAdmissionReceipt {
    fn new(
        artifact_kind: RenderArtifactKind,
        status: TargetAdmissionStatus,
        reasons: Vec<TargetAdmissionReason>,
        font_source: TargetFontSource,
    ) -> Self {
        Self {
            artifact_kind,
            status,
            reasons: reasons.into_boxed_slice(),
            font_source,
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
    theme_evidence: ThemeEvidenceSummary,
    host_measurements: bool,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    let mut rejected = false;
    let mut host_dependent = false;
    if !theme_evidence.is_satisfied() {
        rejected = true;
        reasons.push(TargetAdmissionReason::ThemeEvidenceIncomplete);
    }
    if host_measurements {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::HostDependentTextLayout);
    }
    (rejected, host_dependent)
}

#[cfg(feature = "svg")]
pub(super) fn document_portability_report(
    svg: &merman_render::svg::ResvgCompatibleSvg,
    evidence: &RenderEvidence,
) -> DocumentPortabilityReport {
    portability_report(
        evidence,
        Some(svg),
        merman_render::svg::StandaloneSvgTerminalStatus::Compatible,
    )
}

#[cfg(feature = "svg")]
fn portability_report(
    evidence: &RenderEvidence,
    native_svg: Option<&merman_render::svg::ResvgCompatibleSvg>,
    terminal_status: merman_render::svg::StandaloneSvgTerminalStatus,
) -> DocumentPortabilityReport {
    let host_measurements = evidence.measurement().entries().iter().any(|entry| {
        entry.count() > 0
            && entry.provenance().source == merman_render::environment::TextMeasurementSource::Host
    });
    portability_report_from_summary(
        evidence.family_id(),
        evidence.theme_evidence(),
        host_measurements,
        native_svg,
        terminal_status,
    )
}

#[cfg(feature = "svg")]
fn portability_report_from_summary(
    family_id: merman_core::DiagramFamilyId,
    theme_evidence: ThemeEvidenceSummary,
    host_measurements: bool,
    native_svg: Option<&merman_render::svg::ResvgCompatibleSvg>,
    terminal_status: merman_render::svg::StandaloneSvgTerminalStatus,
) -> DocumentPortabilityReport {
    let mut reasons = Vec::new();
    let (mut rejected, host_dependent) =
        classify_common_document_evidence(theme_evidence, host_measurements, &mut reasons);
    match terminal_status {
        merman_render::svg::StandaloneSvgTerminalStatus::Unverified => {
            reasons.push(TargetAdmissionReason::SvgTerminalUnverified);
        }
        merman_render::svg::StandaloneSvgTerminalStatus::ValidationFailed => {
            rejected = true;
            reasons.push(TargetAdmissionReason::SvgTerminalValidationFailed);
        }
        merman_render::svg::StandaloneSvgTerminalStatus::Compatible => {}
    }
    let prepared_math_evidence = native_svg
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
        family_id,
        theme_evidence,
        resource_closure_complete: terminal_status
            == merman_render::svg::StandaloneSvgTerminalStatus::Compatible,
        prepared_math_evidence,
        rejected,
        host_dependent,
        reasons: reasons.into_boxed_slice(),
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn inherit_document_portability(
    report: &DocumentPortabilityReport,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    reasons.extend(report.reasons().iter().copied());
    (!report.is_evidence_valid(), report.is_host_dependent())
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_font_source(fonts: merman_export::ExportFontPlan) -> TargetFontSource {
    if matches!(
        fonts.source_mode(),
        merman_export::ExportFontMode::NativeContext
    ) {
        TargetFontSource::NativeContext
    } else if fonts.used_system_fonts() {
        TargetFontSource::System
    } else {
        TargetFontSource::None
    }
}

#[cfg(feature = "svg")]
pub(super) fn standalone_svg_admission(
    svg: &merman_render::svg::StandaloneSvgArtifact,
    evidence: &RenderEvidence,
) -> TargetAdmissionReceipt {
    let document = portability_report(evidence, svg.as_resvg_compatible(), svg.terminal_status());
    svg_admission(svg.terminal_status(), svg.finalization_report(), document)
}

#[cfg(feature = "svg")]
pub(super) fn standalone_svg_admission_from_summary(
    svg: &merman_render::svg::StandaloneSvgArtifact,
    family_id: merman_core::DiagramFamilyId,
    theme_evidence: ThemeEvidenceSummary,
    host_measurements: bool,
) -> TargetAdmissionReceipt {
    let document = portability_report_from_summary(
        family_id,
        theme_evidence,
        host_measurements,
        svg.as_resvg_compatible(),
        svg.terminal_status(),
    );
    svg_admission(svg.terminal_status(), svg.finalization_report(), document)
}

#[cfg(feature = "svg")]
pub(super) fn document_svg_admission(
    svg: &merman_render::svg::ResvgCompatibleSvg,
    document: &DocumentPortabilityReport,
) -> TargetAdmissionReceipt {
    svg_admission(
        merman_render::svg::StandaloneSvgTerminalStatus::Compatible,
        Some(svg.finalization_report()),
        document.clone(),
    )
}

#[cfg(feature = "svg")]
fn svg_admission(
    terminal_status: merman_render::svg::StandaloneSvgTerminalStatus,
    report: Option<&merman_render::svg::SvgFinalizationReport>,
    document: DocumentPortabilityReport,
) -> TargetAdmissionReceipt {
    let mut host_dependent = document.is_host_dependent();
    let mut reasons = document.reasons.into_vec();
    let mut rejected = document.rejected;
    if report.is_some_and(|report| report.fallback_css_residual()) {
        rejected = true;
        reasons.push(TargetAdmissionReason::FallbackCssResidual);
    }
    let unverified = terminal_status == merman_render::svg::StandaloneSvgTerminalStatus::Unverified;
    if !unverified
        && !report.is_some_and(merman_render::__private::svg_text_fonts_are_self_contained)
    {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::SvgFontsNotSelfContained);
    }
    let status = if !rejected && unverified {
        TargetAdmissionStatus::Unverified
    } else {
        classify_target_admission(rejected, host_dependent)
    };
    TargetAdmissionReceipt::new(
        RenderArtifactKind::Svg,
        status,
        reasons,
        TargetFontSource::None,
    )
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn classify_native_export_fonts(
    fonts: merman_export::ExportFontPlan,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    let mut rejected = false;
    let mut host_dependent = false;
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
    capabilities
        .into_iter()
        .all(|capability| NATIVE_ROOT_CAPABILITY_WHITELIST.contains(&capability))
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
    report: merman_export::RasterExportReport,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    let (mut rejected, mut host_dependent) = inherit_document_portability(document, &mut reasons);
    rejected |= classify_native_root_capabilities(evidence, &mut reasons);
    let (fonts_rejected, fonts_host_dependent) =
        classify_native_export_fonts(report.fonts(), &mut reasons);
    rejected |= fonts_rejected;
    host_dependent |= fonts_host_dependent;
    if report.directional_filter_transform_residual() {
        rejected = true;
        reasons.push(TargetAdmissionReason::NativeDirectionalFilterTransformResidual);
    }
    if report.fallback_css_residual() {
        rejected = true;
        reasons.push(TargetAdmissionReason::FallbackCssResidual);
    }
    if report.native_filter_receipt() != evidence.native_filter_receipt() {
        rejected = true;
        reasons.push(TargetAdmissionReason::NativeFilterReceiptMismatch);
    }
    let status = classify_target_admission(rejected, host_dependent);
    let artifact_kind = match report.output() {
        merman_export::RasterOutputKind::Png => RenderArtifactKind::Png,
        merman_export::RasterOutputKind::Jpeg => RenderArtifactKind::Jpeg,
        _ => unreachable!("prepared raster report uses a known output kind"),
    };
    TargetAdmissionReceipt::new(
        artifact_kind,
        status,
        reasons,
        export_font_source(report.fonts()),
    )
}

#[cfg(feature = "pdf")]
pub(super) fn native_pdf_admission(
    report: merman_export::PdfExportReport,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    let (mut rejected, mut host_dependent) = inherit_document_portability(document, &mut reasons);
    rejected |= classify_native_root_capabilities(evidence, &mut reasons);
    let (fonts_rejected, fonts_host_dependent) =
        classify_native_export_fonts(report.fonts(), &mut reasons);
    rejected |= fonts_rejected;
    host_dependent |= fonts_host_dependent;
    if report.directional_filter_transform_residual() {
        rejected = true;
        reasons.push(TargetAdmissionReason::NativeDirectionalFilterTransformResidual);
    }
    if report.implicit_viewport_clipping_residual() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PdfImplicitViewportClippingResidual);
    }
    if report.fallback_css_residual() {
        rejected = true;
        reasons.push(TargetAdmissionReason::FallbackCssResidual);
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
    TargetAdmissionReceipt::new(
        RenderArtifactKind::Pdf,
        status,
        reasons,
        export_font_source(report.fonts()),
    )
}

#[cfg(feature = "svg")]
pub(super) fn enforce_portability_requirement(
    evidence: &RenderEvidence,
    receipt: &TargetAdmissionReceipt,
) -> Result<(), TargetAdmissionError> {
    if evidence.portability_requirement()
        == merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable
    {
        return enforce_required_portability(receipt);
    }
    Ok(())
}

#[cfg(feature = "svg")]
pub(super) fn enforce_required_portability(
    receipt: &TargetAdmissionReceipt,
) -> Result<(), TargetAdmissionError> {
    if !receipt.status().is_portable() {
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
                TargetAdmissionReason::HostDependentTextLayout,
                TargetAdmissionReason::PreparedMathEvidenceInvalid,
                TargetAdmissionReason::PreparedMathTerminalProofIncomplete,
                TargetAdmissionReason::SvgFontsNotSelfContained,
                TargetAdmissionReason::SvgTerminalValidationFailed,
                TargetAdmissionReason::SvgTerminalUnverified,
                TargetAdmissionReason::FontResolutionIncomplete,
                TargetAdmissionReason::SystemOrHostFontDependency,
                TargetAdmissionReason::NativeRootCapabilityUnsupported,
                TargetAdmissionReason::NativeFilterReceiptMismatch,
                TargetAdmissionReason::PdfNativeFilterNotLocalized,
                TargetAdmissionReason::NativeDirectionalFilterTransformResidual,
                TargetAdmissionReason::PdfImplicitViewportClippingResidual,
                TargetAdmissionReason::FallbackCssResidual,
            ]
        );
    }
}
