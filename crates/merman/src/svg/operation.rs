use super::{
    LayoutOptions, Result, ResvgCompatibleSvg, SvgDebugOptions, SvgPipeline, SvgRenderOptions,
};
use merman_render::{
    __private::{
        FamilyEvidenceStatus, NativeSvgFilterReceipt, family_applied_theme_capabilities,
        family_evidence, family_native_filter_receipt, family_prepared_text_label_ledger,
        svg_text_fonts_are_self_contained,
    },
    ResourceLimitExceeded,
    diagram_theme::{FontSource, RootThemeVerification, ThemeCapability},
    environment::{RenderEnvironment, RenderSession},
};

/// Stable identity of the operation that produced a retained render result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderExecutionPath {
    HeadlessOperationTyped,
}

impl RenderExecutionPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeadlessOperationTyped => "headless-operation-typed",
        }
    }
}

/// Immutable evidence emitted only after a typed headless SVG operation completes successfully.
///
/// A fresh render session yields [`super::RenderSessionReport`], which cannot be substituted for this
/// completed operation report. Its completion fields are private, so external callers cannot
/// forge one from a fresh session. The public [`crate::svg`] module documentation carries the
/// compile-fail contract tests for both boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOperationReport {
    execution_path: RenderExecutionPath,
    family: merman_render::family::FamilyRenderReport,
}

impl RenderOperationReport {
    pub const fn execution_path(&self) -> RenderExecutionPath {
        self.execution_path
    }

    /// Returns the authoritative family selected before layout for this completed operation.
    pub const fn family_kind(&self) -> super::RenderFamilyKind {
        self.family.family_kind()
    }

    /// Returns root canvas/effect evaluation retained through terminal SVG completion.
    pub const fn root_theme_report(&self) -> &super::RootThemeReport {
        self.family.root_theme_report()
    }

    pub fn measurement_routes(&self) -> &[super::TextMeasurementRoute; 4] {
        self.family.session_report().measurement_routes()
    }

    pub fn measurement(&self) -> &super::TextMeasurementReport {
        self.family.session_report().measurement()
    }

    pub fn operation_context(&self) -> &merman_core::runtime::OperationContext {
        self.family.session_report().operation_context()
    }

    pub const fn unix_millis(&self) -> i64 {
        self.family.session_report().unix_millis()
    }

    pub const fn local_date(&self) -> merman_core::time::CivilDate {
        self.family.session_report().local_date()
    }

    pub fn local_time_zone(&self) -> &merman_core::time::LocalTimeZoneProvenance {
        self.family.session_report().local_time_zone()
    }

    pub fn render_seed(&self) -> std::num::NonZeroU64 {
        self.family.session_report().render_seed()
    }

    /// Returns the deterministic owner-accounted layout and geometry work consumed by the
    /// completed operation. The value supports policy calibration and is not a latency estimate.
    pub const fn layout_work_units(&self) -> usize {
        self.family.session_report().layout_work_units()
    }

    /// Returns the canonical recipe identity frozen for this completed operation.
    pub fn theme_recipe_fingerprint(&self) -> Option<super::ThemeRecipeFingerprint> {
        self.family.theme_recipe_fingerprint()
    }

    /// Returns output-independent theme recipe evidence without retaining runtime services.
    pub const fn theme_recipe_report(&self) -> Option<&super::ThemeRecipeReport> {
        self.family.session_report().theme_recipe_report()
    }

    /// Returns the effective host policy that admitted the selected recipe.
    pub const fn theme_host_admission_report(&self) -> Option<&super::ThemeHostAdmissionReport> {
        self.family.session_report().theme_host_admission_report()
    }

    /// Returns the catalog identity retained by the completed render operation.
    pub const fn font_catalog_fingerprint(&self) -> super::FontCatalogFingerprint {
        self.family.session_report().font_catalog_fingerprint()
    }

    /// Returns the host-owned font-source order frozen by the completed render operation.
    pub const fn font_source_policy(&self) -> &super::FontSourcePolicy {
        self.family.session_report().font_source_policy()
    }

    pub const fn trusted_theme_lanes(&self) -> &super::TrustedThemeLanes {
        self.family.session_report().trusted_theme_lanes()
    }

    pub const fn used_trusted_theme_lanes(&self) -> &super::TrustedThemeLanes {
        self.family.session_report().used_trusted_theme_lanes()
    }

    /// Returns the catalog preparation attestation used by layout, when the operation selected a
    /// custom font catalog.
    pub fn prepared_text_layout(&self) -> Option<&super::PreparedTextLayoutReport> {
        self.family.session_report().prepared_text_layout()
    }
}

/// Stage that contributes a blocking residual to a terminal document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DocumentResidualStage {
    RootTheme,
    FamilyTheme,
    FamilyCompatibility,
    MermaidCompatibility,
    SourceStyle,
    TextLayout,
    TrustedLane,
    Postprocess,
}

impl DocumentResidualStage {
    pub const fn id(self) -> &'static str {
        match self {
            Self::RootTheme => "root-theme",
            Self::FamilyTheme => "family-theme",
            Self::FamilyCompatibility => "family-compatibility",
            Self::MermaidCompatibility => "mermaid-compatibility",
            Self::SourceStyle => "source-style",
            Self::TextLayout => "text-layout",
            Self::TrustedLane => "trusted-lane",
            Self::Postprocess => "postprocess",
        }
    }
}

impl std::fmt::Display for DocumentResidualStage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Why a terminal document cannot claim that every requested mechanism was carried through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DocumentResidualReason {
    Unadapted,
    Unverified,
    Incomplete,
    Failed,
}

impl DocumentResidualReason {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unadapted => "unadapted",
            Self::Unverified => "unverified",
            Self::Incomplete => "incomplete",
            Self::Failed => "failed",
        }
    }
}

impl std::fmt::Display for DocumentResidualReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Frozen document-level residual. A zero count is meaningful for `Incomplete`: it records a
/// missing proof rather than an unsupported mechanism that was observed at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentResidual {
    stage: DocumentResidualStage,
    reason: DocumentResidualReason,
    count: usize,
}

impl DocumentResidual {
    pub const fn stage(&self) -> DocumentResidualStage {
        self.stage
    }

    pub const fn reason(&self) -> DocumentResidualReason {
        self.reason
    }

    pub const fn count(&self) -> usize {
        self.count
    }
}

impl std::fmt::Display for DocumentResidual {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {} ({})", self.stage, self.reason, self.count)
    }
}

/// Output target whose admission is evaluated after terminal document evidence exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RenderTargetKind {
    Svg,
    Png,
    Jpeg,
    Pdf,
}

impl RenderTargetKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Pdf => "pdf",
        }
    }
}

impl std::fmt::Display for RenderTargetKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Final target classification. This is intentionally separate from family verification: a
/// family can be semantically verified while a concrete target still depends on host fonts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetAdmissionStatus {
    Portable,
    HostDependent,
    Rejected,
}

/// Why a target was downgraded or rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TargetAdmissionReason {
    DocumentResidual,
    UnsupportedThemeCapability,
    UnsealedSvg,
    HostTextMeasurement,
    PreparedHostBackend,
    PreparedSystemFont,
    ExternalSvgFontResolution,
    ExportSystemFont,
    ExportUnresolvedFont,
    PreparedTextEvidenceMismatch,
    PreparedTextTerminalProofIncomplete,
    ResourceFingerprintMismatch,
}

impl TargetAdmissionReason {
    pub const fn id(self) -> &'static str {
        match self {
            Self::DocumentResidual => "document-residual",
            Self::UnsupportedThemeCapability => "unsupported-theme-capability",
            Self::UnsealedSvg => "unsealed-svg",
            Self::HostTextMeasurement => "host-text-measurement",
            Self::PreparedHostBackend => "prepared-host-backend",
            Self::PreparedSystemFont => "prepared-system-font",
            Self::ExternalSvgFontResolution => "external-svg-font-resolution",
            Self::ExportSystemFont => "export-system-font",
            Self::ExportUnresolvedFont => "export-unresolved-font",
            Self::PreparedTextEvidenceMismatch => "prepared-text-evidence-mismatch",
            Self::PreparedTextTerminalProofIncomplete => "prepared-text-terminal-proof-incomplete",
            Self::ResourceFingerprintMismatch => "resource-fingerprint-mismatch",
        }
    }
}

/// Frozen target admission decision. The exporter report remains separate because its shape is
/// target-specific; this report only records the cross-stage decision and reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAdmissionReport {
    target: RenderTargetKind,
    status: TargetAdmissionStatus,
    reasons: Box<[TargetAdmissionReason]>,
}

/// Complete PNG/JPEG evidence assembled without collapsing document proof into exporter details.
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, PartialEq)]
pub struct RasterRenderReport {
    operation: RenderOperationReport,
    document: DocumentRenderReport,
    export: merman_export::RasterExportReport,
    target_admission: TargetAdmissionReport,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterRenderReport {
    pub(super) fn new(
        operation: RenderOperationReport,
        document: DocumentRenderReport,
        export: merman_export::RasterExportReport,
        target_admission: TargetAdmissionReport,
    ) -> Self {
        Self {
            operation,
            document,
            export,
            target_admission,
        }
    }

    pub const fn operation_report(&self) -> &RenderOperationReport {
        &self.operation
    }

    pub const fn document_report(&self) -> &DocumentRenderReport {
        &self.document
    }

    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.export
    }

    pub const fn target_admission(&self) -> &TargetAdmissionReport {
        &self.target_admission
    }
}

/// Complete PDF evidence assembled without collapsing document proof into exporter details.
#[cfg(feature = "pdf")]
#[derive(Debug, Clone, PartialEq)]
pub struct PdfRenderReport {
    operation: RenderOperationReport,
    document: DocumentRenderReport,
    export: merman_export::PdfExportReport,
    target_admission: TargetAdmissionReport,
}

#[cfg(feature = "pdf")]
impl PdfRenderReport {
    pub(super) fn new(
        operation: RenderOperationReport,
        document: DocumentRenderReport,
        export: merman_export::PdfExportReport,
        target_admission: TargetAdmissionReport,
    ) -> Self {
        Self {
            operation,
            document,
            export,
            target_admission,
        }
    }

    pub const fn operation_report(&self) -> &RenderOperationReport {
        &self.operation
    }

    pub const fn document_report(&self) -> &DocumentRenderReport {
        &self.document
    }

    pub const fn export_report(&self) -> merman_export::PdfExportReport {
        self.export
    }

    pub const fn target_admission(&self) -> &TargetAdmissionReport {
        &self.target_admission
    }
}

impl TargetAdmissionReport {
    fn unsealed_svg() -> Self {
        Self {
            target: RenderTargetKind::Svg,
            status: TargetAdmissionStatus::Rejected,
            reasons: Box::new([TargetAdmissionReason::UnsealedSvg]),
        }
    }

    pub(super) fn rejected_svg_document_residual() -> Self {
        Self {
            target: RenderTargetKind::Svg,
            status: TargetAdmissionStatus::Rejected,
            reasons: Box::new([TargetAdmissionReason::DocumentResidual]),
        }
    }

    pub const fn target(&self) -> RenderTargetKind {
        self.target
    }

    pub const fn status(&self) -> TargetAdmissionStatus {
        self.status
    }

    pub fn reasons(&self) -> &[TargetAdmissionReason] {
        &self.reasons
    }
}

impl std::fmt::Display for TargetAdmissionReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} target is {:?}", self.target, self.status)?;
        if !self.reasons.is_empty() {
            formatter.write_str(" (")?;
            for (index, reason) in self.reasons.iter().enumerate() {
                if index != 0 {
                    formatter.write_str(", ")?;
                }
                formatter.write_str(reason.id())?;
            }
            formatter.write_str(")")?;
        }
        Ok(())
    }
}

/// Terminal document evidence assembled from one completed typed operation and one sealed SVG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRenderReport {
    svg_finalization: super::SvgFinalizationReport,
    resource_fingerprint: super::SvgResourceFingerprint,
    residuals: Box<[DocumentResidual]>,
    applied_theme_capabilities: Box<[ThemeCapability]>,
    native_filter_receipt: Option<NativeSvgFilterReceipt>,
    host_text_measurement_count: u64,
    prepared_text_used_font_sources: Box<[FontSource]>,
    prepared_text_host_dependent: bool,
    prepared_text_label_count: usize,
}

impl DocumentRenderReport {
    fn new(svg: &ResvgCompatibleSvg, operation: &RenderOperationReport) -> Result<Self> {
        let finalization = svg.finalization_report().clone();
        if finalization.preset() != super::SvgPipelinePreset::ResvgSafe
            || finalization.reference_plan() != svg.reference_plan()
            || finalization.resource_closure() != svg.resource_closure()
        {
            return Err(merman_render::Error::InvalidModel {
                message: "terminal SVG finalization evidence does not match the sealed artifact"
                    .to_string(),
            }
            .into());
        }

        let mut residuals = Vec::new();
        let root = operation.root_theme_report();
        match root.verification() {
            RootThemeVerification::NotApplicable | RootThemeVerification::Verified => {}
            RootThemeVerification::Unverified => residuals.push(DocumentResidual {
                stage: DocumentResidualStage::RootTheme,
                reason: DocumentResidualReason::Unverified,
                count: root.residuals().len(),
            }),
            RootThemeVerification::Incomplete => residuals.push(DocumentResidual {
                stage: DocumentResidualStage::RootTheme,
                reason: DocumentResidualReason::Incomplete,
                count: root.residuals().len(),
            }),
            _ => residuals.push(DocumentResidual {
                stage: DocumentResidualStage::RootTheme,
                reason: DocumentResidualReason::Incomplete,
                count: root.residuals().len(),
            }),
        }

        let family = family_evidence(&operation.family);
        let compatibility_count = family.compatibility_residual_count();
        if compatibility_count != 0 {
            residuals.push(DocumentResidual {
                stage: DocumentResidualStage::FamilyCompatibility,
                reason: DocumentResidualReason::Unverified,
                count: compatibility_count,
            });
        }
        match family.status() {
            FamilyEvidenceStatus::NotApplicable | FamilyEvidenceStatus::Verified => {}
            FamilyEvidenceStatus::Unadapted => residuals.push(DocumentResidual {
                stage: DocumentResidualStage::FamilyTheme,
                reason: DocumentResidualReason::Unadapted,
                count: family.required_count(),
            }),
            FamilyEvidenceStatus::Unverified => {
                if family.theme_residual_count() != 0 {
                    residuals.push(DocumentResidual {
                        stage: DocumentResidualStage::FamilyTheme,
                        reason: DocumentResidualReason::Unverified,
                        count: family.theme_residual_count(),
                    });
                }
            }
            FamilyEvidenceStatus::Incomplete => residuals.push(DocumentResidual {
                stage: DocumentResidualStage::FamilyTheme,
                reason: DocumentResidualReason::Incomplete,
                count: family.incomplete_count(),
            }),
        }
        // Source-style residuals are an independent evidence lane. A family can be incomplete
        // because a theme facet is still outside its proof boundary while also retaining an
        // unverified source declaration; do not let the family verification enum hide that fact.
        if family.source_residual_count() != 0 {
            residuals.push(DocumentResidual {
                stage: DocumentResidualStage::SourceStyle,
                reason: DocumentResidualReason::Unverified,
                count: family.source_residual_count(),
            });
        }

        if operation
            .family
            .session_report()
            .text_layout_failure()
            .is_some()
        {
            residuals.push(DocumentResidual {
                stage: DocumentResidualStage::TextLayout,
                reason: DocumentResidualReason::Failed,
                count: 1,
            });
        }
        if operation.theme_recipe_report().is_some_and(|report| {
            report.required_text_capabilities().next().is_some()
                && operation.prepared_text_layout().is_none()
                && operation
                    .family
                    .session_report()
                    .text_layout_failure()
                    .is_none()
        }) {
            residuals.push(DocumentResidual {
                stage: DocumentResidualStage::TextLayout,
                reason: DocumentResidualReason::Incomplete,
                count: 0,
            });
        }
        let mermaid_compatibility_count = family.mermaid_compatibility_residual_count();
        if mermaid_compatibility_count != 0 {
            residuals.push(DocumentResidual {
                stage: DocumentResidualStage::MermaidCompatibility,
                reason: DocumentResidualReason::Unverified,
                count: mermaid_compatibility_count,
            });
        }
        let used_trusted_lanes = operation
            .family
            .session_report()
            .used_trusted_theme_lanes()
            .allowed()
            .count();
        if used_trusted_lanes != 0 {
            residuals.push(DocumentResidual {
                stage: DocumentResidualStage::TrustedLane,
                reason: DocumentResidualReason::Unverified,
                count: used_trusted_lanes,
            });
        }
        if !finalization.postprocessor_names().is_empty() {
            residuals.push(DocumentResidual {
                stage: DocumentResidualStage::Postprocess,
                reason: DocumentResidualReason::Unverified,
                count: finalization.postprocessor_names().len(),
            });
        }

        let host_text_measurement_count = operation
            .measurement()
            .entries()
            .iter()
            .filter(|entry| entry.provenance().source == super::TextMeasurementSource::Host)
            .map(|entry| entry.count())
            .fold(0u64, u64::saturating_add);
        let prepared_text_ledger = family_prepared_text_label_ledger(&operation.family);
        let mut prepared_text_used_font_sources = Vec::new();
        for source in prepared_text_ledger
            .iter()
            .flat_map(|entry| entry.used_font_sources())
        {
            if !prepared_text_used_font_sources.contains(&source) {
                prepared_text_used_font_sources.push(source);
            }
        }
        let prepared_text_host_dependent = prepared_text_ledger
            .iter()
            .any(|entry| entry.provenance().is_host_dependent());

        let applied_theme_capabilities = root
            .applied_capabilities()
            .chain(family_applied_theme_capabilities(&operation.family))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let native_filter_receipt = family_native_filter_receipt(&operation.family);

        Ok(Self {
            svg_finalization: finalization,
            resource_fingerprint: svg.resource_fingerprint(),
            residuals: residuals.into_boxed_slice(),
            applied_theme_capabilities,
            native_filter_receipt,
            host_text_measurement_count,
            prepared_text_used_font_sources: prepared_text_used_font_sources.into_boxed_slice(),
            prepared_text_host_dependent,
            prepared_text_label_count: prepared_text_ledger.len(),
        })
    }

    pub const fn svg_finalization_report(&self) -> &super::SvgFinalizationReport {
        &self.svg_finalization
    }

    pub const fn resource_fingerprint(&self) -> super::SvgResourceFingerprint {
        self.resource_fingerprint
    }

    pub fn residuals(&self) -> &[DocumentResidual] {
        &self.residuals
    }

    pub const fn host_text_measurement_count(&self) -> u64 {
        self.host_text_measurement_count
    }

    pub fn prepared_text_used_font_sources(&self) -> &[FontSource] {
        &self.prepared_text_used_font_sources
    }

    pub const fn prepared_text_is_host_dependent(&self) -> bool {
        self.prepared_text_host_dependent
    }

    pub const fn prepared_text_label_count(&self) -> usize {
        self.prepared_text_label_count
    }

    pub fn admit_svg(&self) -> TargetAdmissionReport {
        let mut reasons = self
            .residuals
            .iter()
            .map(|_| TargetAdmissionReason::DocumentResidual)
            .collect::<Vec<_>>();
        if self.host_text_measurement_count != 0 {
            reasons.push(TargetAdmissionReason::HostTextMeasurement);
        }
        if self.prepared_text_host_dependent {
            reasons.push(TargetAdmissionReason::PreparedHostBackend);
        }
        if self
            .prepared_text_used_font_sources
            .contains(&FontSource::System)
        {
            reasons.push(TargetAdmissionReason::PreparedSystemFont);
        }
        if self.svg_finalization.text_element_count() != 0
            && !svg_text_fonts_are_self_contained(&self.svg_finalization)
        {
            reasons.push(TargetAdmissionReason::ExternalSvgFontResolution);
        }
        reasons.sort_unstable();
        reasons.dedup();
        let status = if !self.residuals.is_empty() {
            TargetAdmissionStatus::Rejected
        } else if reasons.is_empty() {
            TargetAdmissionStatus::Portable
        } else {
            TargetAdmissionStatus::HostDependent
        };
        TargetAdmissionReport {
            target: RenderTargetKind::Svg,
            status,
            reasons: reasons.into_boxed_slice(),
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn admit_native(
        &self,
        target: RenderTargetKind,
        resource_fingerprint: super::SvgResourceFingerprint,
        fonts: merman_export::ExportFontPlan,
        native_filters: NativeFilterAdmissionEvidence,
    ) -> TargetAdmissionReport {
        let mut reasons = Vec::new();
        if !self.residuals.is_empty() {
            reasons.push(TargetAdmissionReason::DocumentResidual);
        }
        if self
            .applied_theme_capabilities
            .iter()
            .filter(|capability| {
                !matches!(
                    capability,
                    ThemeCapability::Shadow | ThemeCapability::SvgFilter
                )
            })
            .any(|capability| !native_target_supports_theme_capability(target, *capability))
            || !self.native_filter_capabilities_are_supported(native_filters)
        {
            reasons.push(TargetAdmissionReason::UnsupportedThemeCapability);
        }
        if resource_fingerprint != self.resource_fingerprint {
            reasons.push(TargetAdmissionReason::ResourceFingerprintMismatch);
        }
        if fonts.unresolved_font_request()
            || fonts.unresolved_glyph_fallback()
            || fonts.unclassified_face_count() != 0
            || fonts.notdef_glyph_count() != 0
        {
            reasons.push(TargetAdmissionReason::ExportUnresolvedFont);
        }
        if fonts.prepared_label_mismatch_count() != 0
            || fonts.prepared_label_verified_count() != fonts.prepared_label_expected_count()
            || fonts.prepared_label_expected_count() != self.prepared_text_label_count
        {
            reasons.push(TargetAdmissionReason::PreparedTextEvidenceMismatch);
        }
        if fonts.prepared_label_terminal_incomplete_count() != 0 {
            reasons.push(TargetAdmissionReason::PreparedTextTerminalProofIncomplete);
        }
        if self.host_text_measurement_count != 0 {
            reasons.push(TargetAdmissionReason::HostTextMeasurement);
        }
        if self.prepared_text_host_dependent {
            reasons.push(TargetAdmissionReason::PreparedHostBackend);
        }
        if self
            .prepared_text_used_font_sources
            .contains(&FontSource::System)
        {
            reasons.push(TargetAdmissionReason::PreparedSystemFont);
        }
        if fonts.used_system_fonts() {
            reasons.push(TargetAdmissionReason::ExportSystemFont);
        }
        reasons.sort_unstable();
        reasons.dedup();
        let status = if reasons.iter().any(|reason| {
            matches!(
                reason,
                TargetAdmissionReason::DocumentResidual
                    | TargetAdmissionReason::UnsupportedThemeCapability
                    | TargetAdmissionReason::ResourceFingerprintMismatch
                    | TargetAdmissionReason::ExportUnresolvedFont
                    | TargetAdmissionReason::PreparedTextEvidenceMismatch
            )
        }) {
            TargetAdmissionStatus::Rejected
        } else if reasons.is_empty() {
            TargetAdmissionStatus::Portable
        } else {
            TargetAdmissionStatus::HostDependent
        };
        TargetAdmissionReport {
            target,
            status,
            reasons: reasons.into_boxed_slice(),
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn native_filter_capabilities_are_supported(
        &self,
        evidence: NativeFilterAdmissionEvidence,
    ) -> bool {
        let has_shadow = self
            .applied_theme_capabilities
            .contains(&ThemeCapability::Shadow);
        let has_svg_filter = self
            .applied_theme_capabilities
            .contains(&ThemeCapability::SvgFilter);
        match (has_shadow, has_svg_filter) {
            (false, false) => evidence.receipt.is_none(),
            (true, true) => {
                evidence.fully_localized
                    && self.native_filter_receipt.is_some()
                    && self.native_filter_receipt == evidence.receipt
            }
            (false, true) | (true, false) => false,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, Default)]
struct NativeFilterAdmissionEvidence {
    receipt: Option<NativeSvgFilterReceipt>,
    fully_localized: bool,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl NativeFilterAdmissionEvidence {
    const fn new(receipt: Option<NativeSvgFilterReceipt>, fully_localized: bool) -> Self {
        Self {
            receipt,
            fully_localized,
        }
    }
}

fn native_target_supports_theme_capability(
    target: RenderTargetKind,
    capability: ThemeCapability,
) -> bool {
    if !matches!(
        target,
        RenderTargetKind::Png | RenderTargetKind::Jpeg | RenderTargetKind::Pdf
    ) {
        return false;
    }
    matches!(
        capability,
        ThemeCapability::SemanticTokens
            | ThemeCapability::SemanticRules
            | ThemeCapability::OrdinalPalette
            | ThemeCapability::SolidPaint
            | ThemeCapability::TransparentPaint
            | ThemeCapability::LayeredCanvas
            | ThemeCapability::CanvasLayerPlacement
            | ThemeCapability::BlendMode
            | ThemeCapability::Typography
            | ThemeCapability::BorderStyling
            | ThemeCapability::DashStyling
            | ThemeCapability::RoundedGeometry
            | ThemeCapability::LetterSpacing
            | ThemeCapability::TextTransform
            | ThemeCapability::ContentPadding
            | ThemeCapability::Opacity
            | ThemeCapability::TextDecoration
            | ThemeCapability::WhiteSpaceWrapping
            | ThemeCapability::WordSpacing
    )
}

struct CompletedTypedHeadlessSvg {
    family: merman_render::family::FamilyRenderReport,
}

impl CompletedTypedHeadlessSvg {
    fn new(family: merman_render::family::FamilyRenderReport) -> Self {
        Self { family }
    }

    fn into_report(self) -> RenderOperationReport {
        RenderOperationReport {
            execution_path: RenderExecutionPath::HeadlessOperationTyped,
            family: self.family,
        }
    }
}

pub(super) struct HeadlessOperation<'a> {
    engine: merman_core::Engine,
    text: &'a str,
    parse_options: merman_core::ParseOptions,
    layout_options: &'a LayoutOptions,
    session: RenderSession,
}

/// A canonical typed parse that has not started layout yet.
///
/// Callers can inspect metadata and the family-owned semantic model to apply admission policy.
/// Continuing to layout consumes this stage, so the same parsed model cannot start layout twice.
pub struct PreparedSemantic {
    parsed: merman_core::ParsedDiagramRender,
    layout_options: LayoutOptions,
    session: RenderSession,
}

impl PreparedSemantic {
    /// Returns preprocessed metadata for admission and diagnostics.
    pub fn metadata(&self) -> &merman_core::ParseMetadata {
        self.parsed.metadata()
    }

    /// Returns the family-owned typed semantic variant name without exposing mutable pairing.
    pub fn semantic_kind(&self) -> &'static str {
        self.parsed.model().kind()
    }

    /// Reports required and missing renderer capabilities without starting layout.
    pub fn render_plan(&self) -> Result<merman_render::family::RenderCapabilityPlan> {
        Ok(merman_render::family::plan_render(
            &self.parsed,
            &self.session,
        )?)
    }

    /// Runs layout exactly once and advances to the pre-SVG render stage.
    pub fn continue_layout(self) -> Result<PreparedRender> {
        let Self {
            parsed,
            layout_options,
            session,
        } = self;
        let artifact = merman_render::family::prepare(parsed, &layout_options, session)?;
        Ok(PreparedRender { artifact })
    }
}

/// A canonical typed render that has completed parsing and layout exactly once.
///
/// The artifact exposes metadata, a stable semantic family label, and compatibility layout JSON.
/// SVG rendering consumes it, which prevents the prepared parse/layout result from being rendered
/// more than once while keeping the typed semantic/layout pair opaque and internally consistent.
/// The public [`crate::svg`] module documentation carries the compile-fail contract test for
/// this consuming boundary.
pub struct PreparedRender {
    artifact: merman_render::family::FamilyRenderArtifact,
}

/// Descriptive metadata extracted from the terminal SVG produced by one render operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SvgOutputMetadata {
    title: Option<String>,
    description: Option<String>,
}

impl SvgOutputMetadata {
    fn from_svg(svg: &str) -> Self {
        use quick_xml::XmlVersion;
        use quick_xml::events::Event;

        let mut reader = quick_xml::Reader::from_str(svg);
        let mut metadata = Self::default();
        let mut capture = None;
        let mut depth = 0usize;
        let mut root_seen = false;

        loop {
            let Ok(event) = reader.read_event() else {
                return Self::default();
            };
            match event {
                Event::Start(element) => {
                    let is_root_child = root_seen && depth == 1;
                    let Some(next_depth) = depth.checked_add(1) else {
                        return Self::default();
                    };
                    depth = next_depth;
                    if !root_seen {
                        root_seen = true;
                    } else if is_root_child && capture.is_none() {
                        capture = SvgMetadataCapture::start(
                            element.local_name().as_ref(),
                            depth,
                            &metadata,
                        );
                    }
                }
                Event::Empty(_) => {
                    if !root_seen {
                        root_seen = true;
                    }
                }
                Event::End(_) => {
                    if capture.as_ref().is_some_and(|active| active.depth == depth) {
                        capture
                            .take()
                            .expect("checked metadata capture")
                            .finish(&mut metadata);
                        if metadata.title.is_some() && metadata.description.is_some() {
                            return metadata;
                        }
                    }
                    let Some(next_depth) = depth.checked_sub(1) else {
                        return Self::default();
                    };
                    depth = next_depth;
                }
                Event::Text(text) => {
                    let Some(active) = capture.as_mut() else {
                        continue;
                    };
                    let Ok(text) = text.xml_content(XmlVersion::Implicit1_0) else {
                        return Self::default();
                    };
                    active.text.push_str(&text);
                }
                Event::CData(text) => {
                    let Some(active) = capture.as_mut() else {
                        continue;
                    };
                    let Ok(text) = text.xml_content(XmlVersion::Implicit1_0) else {
                        return Self::default();
                    };
                    active.text.push_str(&text);
                }
                Event::GeneralRef(reference) => {
                    let Some(active) = capture.as_mut() else {
                        continue;
                    };
                    let Some(value) = svg_reference_value(&reference) else {
                        return Self::default();
                    };
                    active.text.push(value);
                }
                Event::Eof => return metadata,
                _ => {}
            }
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[derive(Debug, Clone, Copy)]
enum SvgMetadataField {
    Title,
    Description,
}

struct SvgMetadataCapture {
    field: SvgMetadataField,
    depth: usize,
    text: String,
}

impl SvgMetadataCapture {
    fn start(local_name: &[u8], depth: usize, metadata: &SvgOutputMetadata) -> Option<Self> {
        let field = match local_name {
            b"title" if metadata.title.is_none() => SvgMetadataField::Title,
            b"desc" if metadata.description.is_none() => SvgMetadataField::Description,
            _ => return None,
        };
        Some(Self {
            field,
            depth,
            text: String::new(),
        })
    }

    fn finish(self, metadata: &mut SvgOutputMetadata) {
        let text = self.text.trim();
        let value = (!text.is_empty()).then(|| text.to_string());
        match self.field {
            SvgMetadataField::Title => metadata.title = value,
            SvgMetadataField::Description => metadata.description = value,
        }
    }
}

fn svg_reference_value(reference: &quick_xml::events::BytesRef<'_>) -> Option<char> {
    if let Some(value) = reference.resolve_char_ref().ok()? {
        return Some(value);
    }
    let name = reference.decode().ok()?;
    match name.as_ref() {
        "amp" => Some('&'),
        "apos" => Some('\''),
        "gt" => Some('>'),
        "lt" => Some('<'),
        "quot" => Some('"'),
        _ => None,
    }
}

/// Complete SVG output plus immutable evidence from the operation-owned session.
pub struct RenderedSvg {
    svg: String,
    metadata: SvgOutputMetadata,
    report: RenderOperationReport,
    target_admission: TargetAdmissionReport,
}

/// SVG admitted under the operation's portability policy.
///
/// A best-effort operation may intentionally return an admitted artifact whose target status is
/// `HostDependent` or `Rejected`; the status and reasons remain available through
/// [`Self::target_admission`]. A `RequirePortable` operation cannot construct this value unless
/// the target is portable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedSvg {
    document: RenderedDocument,
    target_admission: TargetAdmissionReport,
}

impl AdmittedSvg {
    pub fn as_str(&self) -> &str {
        self.document.svg.as_str()
    }

    pub const fn report(&self) -> &RenderOperationReport {
        &self.document.report
    }

    pub const fn document_report(&self) -> &DocumentRenderReport {
        &self.document.document_report
    }

    pub const fn target_admission(&self) -> &TargetAdmissionReport {
        &self.target_admission
    }

    pub const fn metadata(&self) -> &SvgOutputMetadata {
        &self.document.metadata
    }

    /// Returns the bounded raster allocation plan without exposing the low-level SVG artifact.
    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub fn raster_plan(
        &self,
        options: &merman_export::RasterOptions,
    ) -> std::result::Result<merman_export::RasterPlan, merman_export::ExportError> {
        Ok(merman_export::svg_raster_plan(&self.document.svg, options)?)
    }

    pub fn into_string(self) -> String {
        self.document.svg.into_string()
    }
}

/// Canonical completed Mermaid document for SVG and native export.
///
/// Construction is private to the consuming typed render operation, so callers cannot combine a
/// sealed SVG with a report or font catalog from another operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedDocument {
    svg: ResvgCompatibleSvg,
    metadata: SvgOutputMetadata,
    report: RenderOperationReport,
    document_report: DocumentRenderReport,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
struct PreparedRasterTarget {
    document: RenderedDocument,
    export: merman_export::PreparedRaster,
    export_report: merman_export::RasterExportReport,
    target_admission: TargetAdmissionReport,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedRasterKind {
    #[cfg(feature = "png")]
    Png,
    #[cfg(feature = "jpeg")]
    Jpeg,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl PreparedRasterKind {
    const fn output(self) -> merman_export::RasterOutputKind {
        match self {
            #[cfg(feature = "png")]
            Self::Png => merman_export::RasterOutputKind::Png,
            #[cfg(feature = "jpeg")]
            Self::Jpeg => merman_export::RasterOutputKind::Jpeg,
        }
    }

    const fn target(self) -> RenderTargetKind {
        match self {
            #[cfg(feature = "png")]
            Self::Png => RenderTargetKind::Png,
            #[cfg(feature = "jpeg")]
            Self::Jpeg => RenderTargetKind::Jpeg,
        }
    }
}

/// PNG export prepared through target-specific admission but not yet encoded.
#[cfg(feature = "png")]
pub struct PreparedPngExport {
    inner: PreparedRasterTarget,
}

#[cfg(feature = "png")]
impl PreparedPngExport {
    pub const fn operation_report(&self) -> &RenderOperationReport {
        &self.inner.document.report
    }

    pub const fn document_report(&self) -> &DocumentRenderReport {
        &self.inner.document.document_report
    }

    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.inner.export_report
    }

    pub const fn target_admission(&self) -> &TargetAdmissionReport {
        &self.inner.target_admission
    }

    pub const fn metadata(&self) -> &SvgOutputMetadata {
        &self.inner.document.metadata
    }

    pub fn encode(self) -> merman_export::Result<(Vec<u8>, RasterRenderReport)> {
        let PreparedRasterTarget {
            document,
            export,
            export_report: planned_report,
            target_admission,
        } = self.inner;
        let (bytes, export_report) = export.encode_png_with_report()?;
        debug_assert_eq!(export_report, planned_report);
        let (operation_report, document_report) = document.into_reports();
        Ok((
            bytes,
            RasterRenderReport::new(
                operation_report,
                document_report,
                export_report,
                target_admission,
            ),
        ))
    }
}

/// JPEG export prepared through target-specific admission but not yet encoded.
#[cfg(feature = "jpeg")]
pub struct PreparedJpegExport {
    inner: PreparedRasterTarget,
}

#[cfg(feature = "jpeg")]
impl PreparedJpegExport {
    pub const fn operation_report(&self) -> &RenderOperationReport {
        &self.inner.document.report
    }

    pub const fn document_report(&self) -> &DocumentRenderReport {
        &self.inner.document.document_report
    }

    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.inner.export_report
    }

    pub const fn target_admission(&self) -> &TargetAdmissionReport {
        &self.inner.target_admission
    }

    pub const fn metadata(&self) -> &SvgOutputMetadata {
        &self.inner.document.metadata
    }

    pub fn encode(self) -> merman_export::Result<(Vec<u8>, RasterRenderReport)> {
        let PreparedRasterTarget {
            document,
            export,
            export_report: planned_report,
            target_admission,
        } = self.inner;
        let (bytes, export_report) = export.encode_jpeg_with_report()?;
        debug_assert_eq!(export_report, planned_report);
        let (operation_report, document_report) = document.into_reports();
        Ok((
            bytes,
            RasterRenderReport::new(
                operation_report,
                document_report,
                export_report,
                target_admission,
            ),
        ))
    }
}

/// PDF export prepared through target-specific admission but not yet encoded.
#[cfg(feature = "pdf")]
pub struct PreparedPdfExport {
    document: RenderedDocument,
    export: merman_export::PreparedPdf,
    export_report: merman_export::PdfExportReport,
    target_admission: TargetAdmissionReport,
}

#[cfg(feature = "pdf")]
impl PreparedPdfExport {
    pub const fn operation_report(&self) -> &RenderOperationReport {
        &self.document.report
    }

    pub const fn document_report(&self) -> &DocumentRenderReport {
        &self.document.document_report
    }

    pub const fn export_report(&self) -> merman_export::PdfExportReport {
        self.export_report
    }

    pub const fn target_admission(&self) -> &TargetAdmissionReport {
        &self.target_admission
    }

    pub const fn metadata(&self) -> &SvgOutputMetadata {
        &self.document.metadata
    }

    pub fn encode(self) -> merman_export::Result<(Vec<u8>, PdfRenderReport)> {
        let Self {
            document,
            export,
            export_report: planned_report,
            target_admission,
        } = self;
        let (bytes, export_report) = export.encode_with_report()?;
        debug_assert_eq!(export_report, planned_report);
        let (operation_report, document_report) = document.into_reports();
        Ok((
            bytes,
            PdfRenderReport::new(
                operation_report,
                document_report,
                export_report,
                target_admission,
            ),
        ))
    }
}

impl RenderedDocument {
    pub const fn report(&self) -> &RenderOperationReport {
        &self.report
    }

    /// Returns evidence assembled only after terminal SVG validation and resource closure.
    pub const fn document_report(&self) -> &DocumentRenderReport {
        &self.document_report
    }

    pub const fn metadata(&self) -> &SvgOutputMetadata {
        &self.metadata
    }

    /// Returns the SVG target decision for the sealed document.
    pub fn svg_target_admission(&self) -> TargetAdmissionReport {
        self.document_report.admit_svg()
    }

    /// Consumes the pending document after enforcing its operation-level portability policy.
    pub fn admit_svg(self) -> std::result::Result<AdmittedSvg, TargetAdmissionReport> {
        let target_admission = self.document_report.admit_svg();
        if operation_requires_portable(&self.report)
            && target_admission.status() != TargetAdmissionStatus::Portable
        {
            return Err(target_admission);
        }
        Ok(AdmittedSvg {
            document: self,
            target_admission,
        })
    }

    #[cfg(feature = "png")]
    pub fn prepare_png_export(
        self,
        options: &merman_export::RasterOptions,
    ) -> super::OutputResult<PreparedPngExport> {
        Ok(PreparedPngExport {
            inner: self.prepare_raster_export(options, PreparedRasterKind::Png)?,
        })
    }

    #[cfg(feature = "jpeg")]
    pub fn prepare_jpeg_export(
        self,
        options: &merman_export::RasterOptions,
    ) -> super::OutputResult<PreparedJpegExport> {
        Ok(PreparedJpegExport {
            inner: self.prepare_raster_export(options, PreparedRasterKind::Jpeg)?,
        })
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    fn prepare_raster_export(
        self,
        options: &merman_export::RasterOptions,
        kind: PreparedRasterKind,
    ) -> super::OutputResult<PreparedRasterTarget> {
        let export = merman_export::prepare_raster(self.sealed_svg(), options)?;
        let export_report = export.report_for_output(kind.output());
        let fonts = export_report.fonts();
        let target_admission = self.document_report.admit_native(
            kind.target(),
            export_report.resource_fingerprint(),
            fonts,
            NativeFilterAdmissionEvidence::new(export_report.native_filter_receipt(), true),
        );
        self.enforce_target_admission(&target_admission)
            .map_err(|report| super::OutputError::TargetAdmissionRejected { report })?;
        Ok(PreparedRasterTarget {
            document: self,
            export,
            export_report,
            target_admission,
        })
    }

    #[cfg(feature = "pdf")]
    pub fn prepare_pdf_export(
        self,
        options: &merman_export::PdfOptions,
    ) -> super::OutputResult<PreparedPdfExport> {
        let export = merman_export::prepare_pdf(self.sealed_svg(), options)?;
        let export_report = export.report();
        let fonts = export_report.fonts();
        let target_admission = self.document_report.admit_native(
            RenderTargetKind::Pdf,
            export_report.resource_fingerprint(),
            fonts,
            NativeFilterAdmissionEvidence::new(
                export_report.native_filter_receipt(),
                export_report.native_filter_fully_localized(),
            ),
        );
        self.enforce_target_admission(&target_admission)
            .map_err(|report| super::OutputError::TargetAdmissionRejected { report })?;
        Ok(PreparedPdfExport {
            document: self,
            export,
            export_report,
            target_admission,
        })
    }

    pub(super) fn sealed_svg(&self) -> &ResvgCompatibleSvg {
        &self.svg
    }

    pub(super) fn enforce_target_admission(
        &self,
        admission: &TargetAdmissionReport,
    ) -> std::result::Result<(), TargetAdmissionReport> {
        if operation_requires_portable(&self.report)
            && admission.status() != TargetAdmissionStatus::Portable
        {
            return Err(admission.clone());
        }
        Ok(())
    }

    pub const fn family_kind(&self) -> super::RenderFamilyKind {
        self.report.family_kind()
    }

    pub const fn root_theme_report(&self) -> &super::RootThemeReport {
        self.report.root_theme_report()
    }

    pub fn theme_recipe_fingerprint(&self) -> Option<super::ThemeRecipeFingerprint> {
        self.report.theme_recipe_fingerprint()
    }

    pub const fn theme_recipe_report(&self) -> Option<&super::ThemeRecipeReport> {
        self.report.theme_recipe_report()
    }

    pub const fn theme_host_admission_report(&self) -> Option<&super::ThemeHostAdmissionReport> {
        self.report.theme_host_admission_report()
    }

    pub const fn resource_closure(&self) -> &super::SvgResourceClosure {
        self.svg.resource_closure()
    }

    pub const fn resource_fingerprint(&self) -> super::SvgResourceFingerprint {
        self.svg.resource_fingerprint()
    }

    pub const fn font_catalog(&self) -> &super::FontCatalog {
        self.svg.font_catalog()
    }

    pub const fn font_source_policy(&self) -> &super::FontSourcePolicy {
        self.svg.font_source_policy()
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(super) fn into_reports(self) -> (RenderOperationReport, DocumentRenderReport) {
        (self.report, self.document_report)
    }
}

impl RenderedSvg {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn report(&self) -> &RenderOperationReport {
        &self.report
    }

    pub const fn metadata(&self) -> &SvgOutputMetadata {
        &self.metadata
    }

    /// Raw SVG has not crossed the terminal resource-closure boundary and therefore cannot carry
    /// a positive portability decision. Use `RenderedDocument` for a sealed SVG decision.
    pub const fn target_admission(&self) -> &TargetAdmissionReport {
        &self.target_admission
    }

    fn ensure_portable(&self) -> std::result::Result<(), TargetAdmissionReport> {
        if operation_requires_portable(&self.report)
            && self.target_admission.status() != TargetAdmissionStatus::Portable
        {
            return Err(self.target_admission.clone());
        }
        Ok(())
    }

    pub const fn family_kind(&self) -> super::RenderFamilyKind {
        self.report.family_kind()
    }

    /// Discards the completed operation report and returns only the SVG string.
    ///
    /// Use [`Self::into_parts`] when family, theme, measurement, or resource evidence must be
    /// retained.
    pub fn into_svg(self) -> String {
        self.svg
    }

    pub fn into_parts(self) -> (String, RenderOperationReport, TargetAdmissionReport) {
        (self.svg, self.report, self.target_admission)
    }
}

fn operation_requires_portable(report: &RenderOperationReport) -> bool {
    report.theme_host_admission_report().is_some_and(|host| {
        host.portability_requirement() == super::ThemePortabilityRequirement::RequirePortable
    })
}

impl PreparedRender {
    /// Returns preprocessed metadata for diagnostics and request policy.
    pub fn metadata(&self) -> &merman_core::ParseMetadata {
        self.artifact.metadata()
    }

    /// Returns the paired built-in render family without exposing its semantic or layout types.
    pub fn family_kind(&self) -> super::RenderFamilyKind {
        self.artifact.family_kind()
    }

    /// Returns an owned inverse projection of the Gantt time axis for parity diagnostics.
    pub fn gantt_time_axis_diagnostics(
        &self,
    ) -> Option<merman_render::family::GanttTimeAxisDiagnostics> {
        self.artifact.gantt_time_axis_diagnostics()
    }

    /// Serializes metadata, compatibility semantics, and typed layout from this artifact.
    pub fn layout_json(&self) -> Result<serde_json::Value> {
        Ok(self.artifact.layout_json()?)
    }

    /// Renders SVG once from the prepared semantic model and layout.
    ///
    /// Request-scoped values in `svg_options` affect geometry and SVG identity only. Production
    /// dependencies and deterministic policy come from the operation-owned render session.
    pub fn render_svg(self, svg_options: &SvgRenderOptions) -> Result<String> {
        self.render_svg_with_debug(svg_options, &SvgDebugOptions::default())
    }

    pub fn render_svg_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<String> {
        self.render_svg_report_with_debug(svg_options, debug_options)
            .map(RenderedSvg::into_svg)
    }

    pub fn render_svg_report(self, svg_options: &SvgRenderOptions) -> Result<RenderedSvg> {
        self.render_svg_report_with_debug(svg_options, &SvgDebugOptions::default())
    }

    pub fn render_svg_report_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<RenderedSvg> {
        let rendered = self
            .render_svg_parts(svg_options, debug_options)
            .map(RenderedSvgParts::into_rendered_svg)?;
        rendered
            .ensure_portable()
            .map_err(|report| super::HeadlessError::TargetAdmissionRejected { report })?;
        Ok(rendered)
    }

    /// Renders SVG once and applies the supplied postprocessing pipeline.
    pub fn render_svg_with_pipeline(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<String> {
        self.render_svg_with_pipeline_and_debug(svg_options, &SvgDebugOptions::default(), pipeline)
    }

    pub fn render_svg_with_pipeline_and_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<String> {
        self.render_svg_with_pipeline_report_and_debug(svg_options, debug_options, pipeline)
            .map(RenderedSvg::into_svg)
    }

    pub fn render_svg_with_pipeline_report(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedSvg> {
        self.render_svg_with_pipeline_report_and_debug(
            svg_options,
            &SvgDebugOptions::default(),
            pipeline,
        )
    }

    pub fn render_svg_with_pipeline_report_and_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedSvg> {
        let rendered = self
            .render_svg_parts(svg_options, debug_options)?
            .into_pipeline_svg(pipeline)?;
        rendered
            .ensure_portable()
            .map_err(|report| super::HeadlessError::TargetAdmissionRejected { report })?;
        Ok(rendered)
    }

    /// Renders and terminally finalizes SVG for resvg/raster consumers.
    ///
    /// The supplied pipeline contributes draft transformations. Its terminal preset is always
    /// replaced with `resvg_safe`, so no custom pass can run after compatibility validation.
    pub fn render_resvg_compatible_svg(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<AdmittedSvg> {
        let document =
            self.render_document_with_debug(svg_options, &SvgDebugOptions::default(), pipeline)?;
        let admission = document.svg_target_admission();
        document
            .enforce_target_admission(&admission)
            .map_err(|report| super::HeadlessError::TargetAdmissionRejected { report })?;
        document
            .admit_svg()
            .map_err(|report| super::HeadlessError::TargetAdmissionRejected { report })
    }

    /// Renders and terminally seals one canonical document for all export projections.
    pub fn render_document(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedDocument> {
        self.render_document_with_debug(svg_options, &SvgDebugOptions::default(), pipeline)
    }

    pub fn render_document_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedDocument> {
        let pipeline = pipeline.clone().into_resvg_safe();
        self.render_svg_parts(svg_options, debug_options)?
            .into_document(&pipeline)
    }

    fn render_svg_parts(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<RenderedSvgParts> {
        let rendered = self.artifact.render_svg(svg_options, debug_options)?;
        Ok(RenderedSvgParts { rendered })
    }
}

impl<'a> HeadlessOperation<'a> {
    pub(super) fn new(
        engine: &merman_core::Engine,
        text: &'a str,
        parse_options: merman_core::ParseOptions,
        layout_options: &'a LayoutOptions,
        environment: &RenderEnvironment,
    ) -> Result<Self> {
        Self::new_with_theme(
            engine,
            text,
            parse_options,
            layout_options,
            environment,
            None,
        )
    }

    pub(super) fn new_with_theme(
        engine: &merman_core::Engine,
        text: &'a str,
        parse_options: merman_core::ParseOptions,
        layout_options: &'a LayoutOptions,
        environment: &RenderEnvironment,
        theme: Option<&super::DiagramTheme>,
    ) -> Result<Self> {
        let session = begin_session(environment, theme)?;
        let engine = super::engine_with_session_context(engine, &session);
        Ok(Self {
            engine,
            text,
            parse_options,
            layout_options,
            session,
        })
    }

    pub(super) fn layout_json(self) -> Result<Option<serde_json::Value>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };
        Ok(Some(prepared.layout_json()?))
    }

    pub(super) fn prepare_render(self) -> Result<Option<PreparedRender>> {
        let Some(semantic) = self.prepare_semantic()? else {
            return Ok(None);
        };

        Ok(Some(semantic.continue_layout()?))
    }

    pub(super) fn plan_render(self) -> Result<Option<merman_render::family::RenderCapabilityPlan>> {
        let Some(semantic) = self.prepare_semantic()? else {
            return Ok(None);
        };
        Ok(Some(semantic.render_plan()?))
    }

    pub(super) fn prepare_semantic(self) -> Result<Option<PreparedSemantic>> {
        self.session
            .resource_policy()
            .check_source_bytes(self.text)
            .map_err(resource_limit_error)?;
        let Some(parsed) = self
            .engine
            .parse_diagram_for_render_model_sync(self.text, self.parse_options)?
        else {
            return Ok(None);
        };

        Ok(Some(PreparedSemantic {
            parsed,
            layout_options: self.layout_options.clone(),
            session: self.session,
        }))
    }

    pub(super) fn render_svg(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<Option<String>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };

        Ok(Some(
            prepared.render_svg_with_debug(svg_options, debug_options)?,
        ))
    }

    pub(super) fn render_svg_with_pipeline(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };

        Ok(Some(prepared.render_svg_with_pipeline_and_debug(
            svg_options,
            debug_options,
            pipeline,
        )?))
    }
}

struct RenderedSvgParts {
    rendered: merman_render::family::RenderedFamilySvg,
}

impl RenderedSvgParts {
    fn into_rendered_svg(self) -> RenderedSvg {
        let completion = self.rendered.into_completion();
        let (svg, family) = completion.into_output_and_report();
        let metadata = SvgOutputMetadata::from_svg(&svg);
        let report = CompletedTypedHeadlessSvg::new(family).into_report();
        RenderedSvg {
            svg,
            metadata,
            report,
            target_admission: TargetAdmissionReport::unsealed_svg(),
        }
    }

    fn into_pipeline_svg(self, pipeline: &SvgPipeline) -> Result<RenderedSvg> {
        let rendered = self.rendered.apply_pipeline(pipeline)?;
        let completion = rendered.into_completion();
        let (svg, family) = completion.into_output_and_report();
        let metadata = SvgOutputMetadata::from_svg(&svg);
        let report = CompletedTypedHeadlessSvg::new(family).into_report();
        Ok(RenderedSvg {
            svg,
            metadata,
            report,
            target_admission: TargetAdmissionReport::unsealed_svg(),
        })
    }

    fn into_document(self, pipeline: &SvgPipeline) -> Result<RenderedDocument> {
        let rendered = self.rendered.finalize_resvg(pipeline)?;
        let completion = rendered.into_completion();
        let (svg, family) = completion.into_output_and_report();
        let report = CompletedTypedHeadlessSvg::new(family).into_report();
        debug_assert_eq!(
            svg.font_catalog().fingerprint(),
            report.font_catalog_fingerprint(),
            "sealed SVG and operation report must retain the same catalog"
        );
        debug_assert_eq!(
            svg.font_source_policy(),
            report.font_source_policy(),
            "sealed SVG and operation report must retain the same font-source policy"
        );
        if let Some(theme_report) = report.theme_recipe_report() {
            debug_assert_eq!(
                theme_report.font_catalog_fingerprint(),
                report.font_catalog_fingerprint(),
                "theme and operation report must retain the same catalog"
            );
            debug_assert_eq!(
                theme_report.font_catalog_fingerprint(),
                svg.font_catalog().fingerprint(),
                "theme and sealed SVG must retain the same catalog"
            );
        }
        let document_report = DocumentRenderReport::new(&svg, &report)?;
        let metadata = SvgOutputMetadata::from_svg(svg.as_str());
        Ok(RenderedDocument {
            svg,
            metadata,
            report,
            document_report,
        })
    }
}

pub(super) fn begin_session(
    environment: &RenderEnvironment,
    theme: Option<&super::DiagramTheme>,
) -> Result<RenderSession> {
    Ok(match theme {
        Some(theme) => environment.begin_session_with_theme(theme)?,
        None => environment.begin_session()?,
    })
}

pub(super) fn resource_limit_error(err: ResourceLimitExceeded) -> super::HeadlessError {
    merman_render::Error::ResourceLimitExceeded(err).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    use merman_render::__private::NativeSvgHardShadow;

    fn legacy_flowchart_edge_theme() -> crate::svg::DiagramTheme {
        crate::svg::DiagramThemeCompiler::new()
            .compile(
                crate::svg::DiagramThemeSpec::new().with_styles(
                    crate::svg::ThemeRuleSet::default().with_rule(
                        crate::svg::ThemeRule::new(
                            crate::svg::ThemeTarget::Edge,
                            crate::svg::ThemeStylePatch::default().with_stroke(
                                crate::svg::CanvasPaint::solid("#ef4444")
                                    .expect("valid test color"),
                            ),
                        )
                        .for_family(crate::svg::RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("legacy Flowchart bridge theme should compile")
    }

    fn typed_flowchart_node_theme() -> crate::svg::DiagramTheme {
        crate::svg::DiagramThemeCompiler::new()
            .compile(
                crate::svg::DiagramThemeSpec::new().with_styles(
                    crate::svg::ThemeRuleSet::default().with_rule(
                        crate::svg::ThemeRule::new(
                            crate::svg::ThemeTarget::Node,
                            crate::svg::ThemeStylePatch::default()
                                .with_fill(
                                    crate::svg::CanvasPaint::solid("#ef4444")
                                        .expect("valid test fill"),
                                )
                                .with_stroke(
                                    crate::svg::CanvasPaint::solid("#2563eb")
                                        .expect("valid test stroke"),
                                ),
                        )
                        .for_family(crate::svg::RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("typed Flowchart Node theme should compile")
    }

    fn explicit_mermaid_variable_theme() -> crate::svg::DiagramTheme {
        let compatibility = crate::svg::MermaidThemeCompatibility::default()
            .with_variable("useGradient", true)
            .expect("bounded Mermaid compatibility variable");
        crate::svg::DiagramThemeCompiler::new()
            .compile(crate::svg::DiagramThemeSpec::new().with_mermaid_compatibility(compatibility))
            .expect("Mermaid compatibility theme should compile")
    }

    fn explicit_mermaid_root_theme() -> crate::svg::DiagramTheme {
        let compatibility = crate::svg::MermaidThemeCompatibility::default()
            .with_theme("dark")
            .expect("bounded Mermaid theme id")
            .with_dark_mode(true)
            .expect("canonical Mermaid dark-mode value");
        crate::svg::DiagramThemeCompiler::new()
            .compile(crate::svg::DiagramThemeSpec::new().with_mermaid_compatibility(compatibility))
            .expect("Mermaid root compatibility theme should compile")
    }

    #[test]
    fn terminal_svg_metadata_ignores_nested_element_descriptions() {
        let metadata = SvgOutputMetadata::from_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg">
                <g><title>Node title</title><desc>Node description</desc></g>
                <title>Diagram title</title>
                <desc>Diagram description</desc>
            </svg>"#,
        );

        assert_eq!(metadata.title(), Some("Diagram title"));
        assert_eq!(metadata.description(), Some("Diagram description"));
    }

    #[test]
    fn legacy_family_compatibility_is_a_terminal_document_residual() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(legacy_flowchart_edge_theme())
            .render_document_sync("flowchart TD\n  A[Themed]")
            .expect("best-effort render should succeed")
            .expect("Flowchart source should be detected");
        let compatibility_count = document
            .document_report()
            .residuals()
            .iter()
            .find(|residual| residual.stage() == DocumentResidualStage::FamilyCompatibility)
            .map(DocumentResidual::count)
            .expect("legacy compatibility should remain a terminal residual");

        assert_ne!(compatibility_count, 0);
        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .any(|residual| {
                    residual.stage() == DocumentResidualStage::FamilyCompatibility
                        && residual.reason() == DocumentResidualReason::Unverified
                        && residual.count() == compatibility_count
                })
        );

        let admission = document.svg_target_admission();
        assert_eq!(admission.status(), TargetAdmissionStatus::Rejected);
        assert!(
            admission
                .reasons()
                .contains(&TargetAdmissionReason::DocumentResidual)
        );
    }

    #[test]
    fn family_theme_and_source_style_residuals_remain_independent() {
        let gradient = crate::svg::LinearGradient::new(
            90.0,
            [
                crate::svg::GradientStop::new(
                    0.0,
                    crate::svg::ThemeColorValue::parse("#0f172a")
                        .expect("valid gradient start color"),
                )
                .expect("valid gradient start"),
                crate::svg::GradientStop::new(
                    1.0,
                    crate::svg::ThemeColorValue::parse("#22d3ee")
                        .expect("valid gradient end color"),
                )
                .expect("valid gradient end"),
            ],
        )
        .expect("valid gradient");
        let theme = crate::svg::DiagramThemeCompiler::new()
            .compile(
                crate::svg::DiagramThemeSpec::new().with_styles(
                    crate::svg::ThemeRuleSet::default().with_rule(
                        crate::svg::ThemeRule::new(
                            crate::svg::ThemeTarget::Node,
                            crate::svg::ThemeStylePatch::default()
                                .with_stroke(crate::svg::CanvasPaint::LinearGradient(gradient)),
                        )
                        .for_family(crate::svg::RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile unsupported Flowchart gradient theme");
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(theme)
            .render_document_sync(
                "flowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n",
            )
            .expect("best-effort render should succeed")
            .expect("Flowchart source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .any(|residual| {
                    residual.stage() == DocumentResidualStage::FamilyTheme
                        && residual.reason() == DocumentResidualReason::Unverified
                        && residual.count() != 0
                })
        );
        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .any(|residual| {
                    residual.stage() == DocumentResidualStage::SourceStyle
                        && residual.reason() == DocumentResidualReason::Unverified
                        && residual.count() != 0
                })
        );
    }

    #[test]
    fn typed_flowchart_node_evidence_survives_document_completion() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(typed_flowchart_node_theme())
            .render_document_sync("flowchart LR\nA[Alpha]\n")
            .expect("typed Flowchart render should succeed")
            .expect("Flowchart source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .all(|residual| {
                    !matches!(
                        residual.stage(),
                        DocumentResidualStage::FamilyTheme | DocumentResidualStage::SourceStyle
                    )
                })
        );
        assert_ne!(
            document.svg_target_admission().status(),
            TargetAdmissionStatus::Rejected
        );
    }

    #[test]
    fn source_only_flowchart_residual_stays_out_of_the_family_theme_lane() {
        let document = crate::svg::HeadlessRenderer::new()
            .render_document_sync(
                "flowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n",
            )
            .expect("best-effort source-style render should succeed")
            .expect("Flowchart source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .any(|residual| residual.stage() == DocumentResidualStage::SourceStyle)
        );
        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .all(|residual| residual.stage() != DocumentResidualStage::FamilyTheme)
        );
        assert_eq!(
            document.svg_target_admission().status(),
            TargetAdmissionStatus::Rejected
        );
    }

    #[test]
    fn explicit_mermaid_variables_are_terminal_document_residuals() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(explicit_mermaid_variable_theme())
            .render_document_sync("stateDiagram-v2\n  [*] --> Ready")
            .expect("best-effort render should succeed")
            .expect("State source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .any(|residual| {
                    residual.stage() == DocumentResidualStage::MermaidCompatibility
                        && residual.reason() == DocumentResidualReason::Unverified
                        && residual.count() == 1
                })
        );
        assert_eq!(
            document.svg_target_admission().status(),
            TargetAdmissionStatus::Rejected
        );
    }

    #[test]
    fn explicit_mermaid_root_fields_are_terminal_document_residuals() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(explicit_mermaid_root_theme())
            .render_document_sync("stateDiagram-v2\n  [*] --> Ready")
            .expect("best-effort render should succeed")
            .expect("State source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .any(|residual| {
                    residual.stage() == DocumentResidualStage::MermaidCompatibility
                        && residual.reason() == DocumentResidualReason::Unverified
                        && residual.count() == 2
                })
        );
        assert_eq!(
            document.svg_target_admission().status(),
            TargetAdmissionStatus::Rejected
        );
    }

    #[test]
    fn overridden_mermaid_variable_is_not_a_terminal_document_residual() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(explicit_mermaid_variable_theme())
            .with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "useGradient": false }
            })))
            .render_document_sync("stateDiagram-v2\n  [*] --> Ready")
            .expect("best-effort render should succeed")
            .expect("State source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .all(|residual| residual.stage() != DocumentResidualStage::MermaidCompatibility)
        );
    }

    #[test]
    fn same_value_site_override_owns_mermaid_compatibility_field() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(explicit_mermaid_variable_theme())
            .with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "useGradient": true }
            })))
            .render_document_sync("stateDiagram-v2\n  [*] --> Ready")
            .expect("best-effort render should succeed")
            .expect("State source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .all(|residual| residual.stage() != DocumentResidualStage::MermaidCompatibility)
        );
    }

    #[test]
    fn same_value_source_override_owns_mermaid_compatibility_field() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(explicit_mermaid_variable_theme())
            .with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
                "secure": []
            })))
            .render_document_sync(
                "%%{init: {\"themeVariables\": {\"useGradient\": true}}}%%\nstateDiagram-v2\n  [*] --> Ready",
            )
            .expect("best-effort render should succeed")
            .expect("State source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .all(|residual| residual.stage() != DocumentResidualStage::MermaidCompatibility)
        );
    }

    #[test]
    fn base_engine_host_config_outranks_compiled_theme_compatibility() {
        let base_engine = merman_core::Engine::new().with_site_config(
            merman_core::MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "useGradient": true }
            })),
        );
        let document = crate::svg::HeadlessRenderer::new()
            .with_engine(base_engine)
            .with_theme(explicit_mermaid_variable_theme())
            .render_document_sync("stateDiagram-v2\n  [*] --> Ready")
            .expect("best-effort render should succeed")
            .expect("State source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .all(|residual| residual.stage() != DocumentResidualStage::MermaidCompatibility)
        );
    }

    #[test]
    fn overridden_mermaid_root_fields_are_not_terminal_document_residuals() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(explicit_mermaid_root_theme())
            .with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
                "theme": "neutral",
                "darkMode": false,
                "themeVariables": { "darkMode": false }
            })))
            .render_document_sync("stateDiagram-v2\n  [*] --> Ready")
            .expect("best-effort render should succeed")
            .expect("State source should be detected");

        assert!(
            document
                .document_report()
                .residuals()
                .iter()
                .all(|residual| residual.stage() != DocumentResidualStage::MermaidCompatibility)
        );
    }

    #[cfg(feature = "png")]
    #[test]
    fn legacy_family_compatibility_rejects_every_native_target() {
        let document = crate::svg::HeadlessRenderer::new()
            .with_theme(legacy_flowchart_edge_theme())
            .render_document_sync("flowchart TD\n  A[Themed]")
            .expect("best-effort render should succeed")
            .expect("Flowchart source should be detected");
        let report = document.document_report().clone();
        let export = merman_export::prepare_raster(
            document.sealed_svg(),
            &merman_export::RasterOptions::default(),
        )
        .expect("native raster preflight should succeed");
        let fonts = export
            .report_for_output(merman_export::RasterOutputKind::Png)
            .fonts();

        for target in [
            RenderTargetKind::Png,
            RenderTargetKind::Jpeg,
            RenderTargetKind::Pdf,
        ] {
            let admission = report.admit_native(
                target,
                report.resource_fingerprint(),
                fonts,
                NativeFilterAdmissionEvidence::default(),
            );

            assert_eq!(admission.status(), TargetAdmissionStatus::Rejected);
            assert!(
                admission
                    .reasons()
                    .contains(&TargetAdmissionReason::DocumentResidual),
                "{target} admission must retain the compatibility document residual"
            );
        }
    }

    #[cfg(feature = "png")]
    #[test]
    fn native_admission_retains_prepared_and_export_system_font_evidence() {
        let document = crate::svg::HeadlessRenderer::new()
            .render_document_sync("info")
            .expect("info render should succeed")
            .expect("info source should be detected");
        let mut report = document.document_report().clone();
        report.prepared_text_used_font_sources = vec![FontSource::System].into_boxed_slice();
        let export = merman_export::prepare_raster(
            document.sealed_svg(),
            &merman_export::RasterOptions::default(),
        )
        .expect("native raster preflight should succeed");
        let fonts = export
            .report_for_output(merman_export::RasterOutputKind::Png)
            .fonts();

        let admission = report.admit_native(
            RenderTargetKind::Png,
            report.resource_fingerprint(),
            fonts,
            NativeFilterAdmissionEvidence::default(),
        );

        assert!(
            admission
                .reasons()
                .contains(&TargetAdmissionReason::PreparedSystemFont)
        );
        assert!(
            admission
                .reasons()
                .contains(&TargetAdmissionReason::ExportSystemFont)
        );
    }

    #[test]
    fn native_theme_capability_policy_is_explicit_and_fail_closed() {
        for target in [
            RenderTargetKind::Png,
            RenderTargetKind::Jpeg,
            RenderTargetKind::Pdf,
        ] {
            for capability in [
                ThemeCapability::SolidPaint,
                ThemeCapability::TransparentPaint,
                ThemeCapability::LayeredCanvas,
                ThemeCapability::CanvasLayerPlacement,
                ThemeCapability::BlendMode,
                ThemeCapability::Opacity,
            ] {
                assert!(
                    native_target_supports_theme_capability(target, capability),
                    "{target} should support {capability}"
                );
            }
            for capability in [
                ThemeCapability::GradientPaint,
                ThemeCapability::PatternPaint,
                ThemeCapability::CanvasBleed,
                ThemeCapability::Shadow,
                ThemeCapability::SvgFilter,
                ThemeCapability::Noise,
                ThemeCapability::Displacement,
            ] {
                assert!(
                    !native_target_supports_theme_capability(target, capability),
                    "{target} must not admit {capability} without exporter proof"
                );
            }
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn native_filter_capabilities_require_an_exact_localized_exporter_receipt() {
        let document = crate::svg::HeadlessRenderer::new()
            .render_document_sync("info")
            .expect("info render should succeed")
            .expect("info source should be detected");
        let mut report = document.document_report().clone();
        let receipt = NativeSvgFilterReceipt::from_hard_shadows([NativeSvgHardShadow::new(
            "state-ready-theme-effect-hard-shadow",
            [-0.2, -0.2, 1.4, 1.4],
            [4.0, 5.0],
            "#111827",
            1,
        )
        .expect("valid hard-shadow receipt")])
        .expect("non-empty hard-shadow receipt");
        let mismatched_receipt =
            NativeSvgFilterReceipt::from_hard_shadows([NativeSvgHardShadow::new(
                "state-ready-theme-effect-hard-shadow",
                [-0.2, -0.2, 1.4, 1.4],
                [5.0, 5.0],
                "#111827",
                1,
            )
            .expect("valid mismatched hard-shadow receipt")])
            .expect("non-empty mismatched hard-shadow receipt");
        report.applied_theme_capabilities =
            vec![ThemeCapability::Shadow, ThemeCapability::SvgFilter].into_boxed_slice();
        report.native_filter_receipt = Some(receipt);

        assert!(report.native_filter_capabilities_are_supported(
            NativeFilterAdmissionEvidence::new(Some(receipt), true)
        ));
        assert!(!report.native_filter_capabilities_are_supported(
            NativeFilterAdmissionEvidence::new(Some(receipt), false)
        ));
        assert!(!report.native_filter_capabilities_are_supported(
            NativeFilterAdmissionEvidence::new(Some(mismatched_receipt), true)
        ));
        assert!(
            !report
                .native_filter_capabilities_are_supported(NativeFilterAdmissionEvidence::default())
        );
    }

    #[cfg(feature = "png")]
    #[test]
    fn native_admission_rejects_an_unproved_theme_capability() {
        let document = crate::svg::HeadlessRenderer::new()
            .render_document_sync("info")
            .expect("info render should succeed")
            .expect("info source should be detected");
        let mut report = document.document_report().clone();
        report.applied_theme_capabilities = vec![ThemeCapability::GradientPaint].into_boxed_slice();
        let export = merman_export::prepare_raster(
            document.sealed_svg(),
            &merman_export::RasterOptions::default(),
        )
        .expect("native raster preflight should succeed");
        let fonts = export
            .report_for_output(merman_export::RasterOutputKind::Png)
            .fonts();

        let admission = report.admit_native(
            RenderTargetKind::Png,
            report.resource_fingerprint(),
            fonts,
            NativeFilterAdmissionEvidence::default(),
        );

        assert_eq!(admission.status(), TargetAdmissionStatus::Rejected);
        assert!(
            admission
                .reasons()
                .contains(&TargetAdmissionReason::UnsupportedThemeCapability)
        );
    }
}
