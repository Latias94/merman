#![forbid(unsafe_code)]

//! Headless layout + rendering for Mermaid diagrams.
//!
//! This crate consumes `merman-core`'s semantic models and produces:
//! - a layout JSON (geometry + routes)
//! - Mermaid-like SVG output with DOM parity checks against upstream baselines

#[cfg(feature = "layout-cytoscape")]
pub mod architecture;
#[cfg(feature = "layout-cytoscape")]
pub(crate) mod architecture_metrics;
pub mod block;
pub mod c4;
mod chart_palette;
pub mod class;
mod config;
pub mod cynefin;
mod dagre;
pub mod diagram_theme;
mod entities;
pub mod environment;
pub mod er;
pub mod error;
pub mod eventmodeling;
pub mod family;
pub mod flowchart;
pub mod gantt;
mod generated;
pub mod gitgraph;
pub mod info;
pub mod ishikawa;
pub mod journey;
pub mod kanban;
mod layout_work;
pub(crate) mod math;
mod mermaid_style;
pub mod mindmap;
pub mod model;
mod native_filter_receipt;
pub mod packet;
pub mod pie;
pub mod quadrantchart;
pub mod radar;
pub mod railroad;
pub mod requirement;
pub mod resources;
pub mod sankey;
pub mod sequence;
pub mod state;
pub mod svg;
#[cfg(feature = "internal-theme-acceptance")]
mod svg_artifact_receipts;
pub mod swimlane;
pub mod text;
mod theme;
#[cfg(feature = "internal-theme-acceptance")]
mod theme_raster_paint;
mod theme_route_cutover;
pub mod timeline;
pub mod tree_view;
pub mod treemap;
mod trig_tables;
pub mod venn;
pub mod wardley;
mod xml;
pub mod xychart;
pub mod zenuml;

pub use merman_core::DiagramFamilyId;

/// Workspace-internal implementation seams with no compatibility promise.
///
/// Only route-cutover inventory is expected to disappear after family migration; the bounded
/// facade projections may evolve independently of the public renderer API.
#[doc(hidden)]
pub mod __private {
    use crate::family::{FamilyRenderReport, FamilyStyleVerification};

    pub use crate::native_filter_receipt::{NativeSvgFilterReceipt, NativeSvgHardShadow};

    #[cfg(feature = "internal-theme-acceptance")]
    pub use crate::svg_artifact_receipts::{
        SvgArtifactReceipt, SvgAttributeObservation, SvgElementObservation, SvgFontFaceObservation,
        SvgStyleDeclarationObservation, SvgStyleRuleObservation, SvgStylesheetObservation,
    };

    pub use crate::text::__private::{
        PreparedTextFaceKey, PreparedTextLabelEvidence, PreparedTextLabelId,
        PreparedTextLabelLedgerEntry, PreparedTextLabelProvenance, PreparedTextTerminalFace,
        PreparedTextTerminalLabelReceipt, PreparedTextTerminalReceipt,
    };

    #[cfg(all(feature = "internal-theme-acceptance", feature = "layout-cytoscape"))]
    pub use crate::theme_route_cutover::{
        ArchitectureTextCutoverReceipt, ArchitectureTextCutoverRole,
        ArchitectureTextCutoverTerminal,
    };

    #[cfg(feature = "internal-theme-acceptance")]
    pub use crate::theme_route_cutover::{
        ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverId,
        ThemeRouteCutoverInventoryError, ThemeRouteCutoverProjection,
        ThemeRouteCutoverProjectionAction, ThemeRouteCutoverProjectionSet,
        ThemeRouteCutoverReceipt, ThemeRouteCutoverSelector, ThemeRouteCutoverValue,
    };

    #[cfg(feature = "internal-theme-acceptance")]
    pub use crate::theme_raster_paint::{
        ThemeRasterPaintBinding, ThemeRasterPaintBindingReceipt, ThemeRasterPaintLineGeometry,
        ThemeRasterPaintTerminal, ThemeRasterPaintTerminalSemantic,
    };

    #[cfg(feature = "internal-theme-acceptance")]
    pub use crate::diagram_theme::LegacyFamilyThemeBridgeRetirementStatus;

    #[cfg(feature = "internal-theme-acceptance")]
    pub use crate::diagram_theme::{
        ThemeLegacyProjectionKey, ThemeLegacyProjectionRetirementDescriptor,
        ThemeLegacyProjectionRetirementInventoryError, ThemeLegacyProjectionRetirementReceipt,
        ThemeLegacyRouteFacet, ThemeLegacyRouteId, ThemeLegacyRouteSelector,
        ThemeLegacyTombstoneInventoryError,
    };

    /// Returns the renderer-owned KTD23 tombstone identities without consulting the compatibility
    /// bridge or historical projection witness.
    #[cfg(feature = "internal-theme-acceptance")]
    pub fn ktd23_tombstone_theme_routes()
    -> Result<Vec<ThemeLegacyRouteId>, ThemeLegacyTombstoneInventoryError> {
        crate::diagram_theme::ktd23_tombstone_inventory()
    }

    /// Returns every currently typed route that replaces a concrete legacy bridge projection.
    #[cfg(feature = "internal-theme-acceptance")]
    pub fn legacy_replacing_typed_theme_routes()
    -> Result<Vec<ThemeRouteCutoverDescriptor>, ThemeRouteCutoverInventoryError> {
        crate::diagram_theme::legacy_replacing_typed_routes()
    }

    /// Returns the production-owned fixed KTD23 retirement inventory.
    #[cfg(feature = "internal-theme-acceptance")]
    pub fn retired_legacy_theme_projection_inventory() -> Result<
        Vec<ThemeLegacyProjectionRetirementDescriptor>,
        ThemeLegacyProjectionRetirementInventoryError,
    > {
        crate::diagram_theme::legacy_projection_retirement_inventory()
    }

    /// Seals the current empty-bridge receipt for every production-owned KTD23 route.
    #[cfg(feature = "internal-theme-acceptance")]
    pub fn retired_legacy_theme_projection_receipts() -> Result<
        Vec<ThemeLegacyProjectionRetirementReceipt>,
        ThemeLegacyProjectionRetirementInventoryError,
    > {
        crate::diagram_theme::legacy_projection_retirement_receipts()
    }

    /// Returns matrix-derived readiness facts for removing the legacy family bridge.
    ///
    /// This gate is intentionally independent of C6 visual qualification and route artifact
    /// witnesses. It becomes true only after every executable legacy route and bridge dispatch
    /// entry has been retired.
    #[cfg(feature = "internal-theme-acceptance")]
    pub fn legacy_family_theme_bridge_retirement_status()
    -> crate::diagram_theme::LegacyFamilyThemeBridgeRetirementStatus {
        crate::diagram_theme::legacy_family_theme_bridge_retirement_status()
    }

    /// Seals renderer-owned route receipts against both finalized SVG representations.
    #[cfg(feature = "internal-theme-acceptance")]
    pub fn seal_theme_route_cutover_receipts(
        report: &FamilyRenderReport,
        artifact_digest: [u8; 32],
        native_artifact_digest: [u8; 32],
    ) -> Vec<ThemeRouteCutoverReceipt> {
        if !report.style_report().is_verified() {
            return Vec::new();
        }
        crate::theme_route_cutover::seal_theme_route_cutover_receipts(
            report.theme_route_cutover_facts(),
            artifact_digest,
            native_artifact_digest,
        )
    }

    /// Seals renderer-owned raster paint terminal bindings against the finalized native SVG.
    #[cfg(feature = "internal-theme-acceptance")]
    pub fn seal_theme_raster_paint_binding_receipts(
        report: &FamilyRenderReport,
        native_artifact_digest: [u8; 32],
    ) -> Vec<ThemeRasterPaintBindingReceipt> {
        if !report.style_report().is_verified() {
            return Vec::new();
        }
        crate::theme_raster_paint::seal_theme_raster_paint_binding_receipts(
            report.theme_raster_paint_binding_facts(),
            native_artifact_digest,
        )
    }

    /// Returns the renderer-owned Architecture Text cutover facts sealed by the final writer.
    #[cfg(all(feature = "internal-theme-acceptance", feature = "layout-cytoscape"))]
    pub fn architecture_text_cutover_receipt(
        report: &FamilyRenderReport,
    ) -> Option<&ArchitectureTextCutoverReceipt> {
        report.architecture_text_cutover_receipt()
    }

    /// Coarse family-evidence state used by the workspace facade.
    ///
    /// This type is intentionally isolated from the stable renderer surface. It carries only the
    /// terminal projection needed to build document admission and must not grow family mechanism
    /// keys, selectors, contribution identifiers, or per-element evidence.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FamilyEvidenceStatus {
        NotApplicable,
        Verified,
        Unverified,
        Unadapted,
        Incomplete,
    }

    impl FamilyEvidenceStatus {
        pub(crate) const fn from_verification(verification: FamilyStyleVerification) -> Self {
            match verification {
                FamilyStyleVerification::NotApplicable => Self::NotApplicable,
                FamilyStyleVerification::Verified => Self::Verified,
                FamilyStyleVerification::Unverified => Self::Unverified,
                FamilyStyleVerification::Unadapted => Self::Unadapted,
                FamilyStyleVerification::Incomplete => Self::Incomplete,
            }
        }
    }

    /// Bounded terminal family evidence projected for the workspace facade.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FamilyEvidenceSummary {
        status: FamilyEvidenceStatus,
        required_count: usize,
        accounted_count: usize,
        applied_count: usize,
        not_applicable_count: usize,
        theme_residual_count: usize,
        source_residual_count: usize,
        compatibility_residual_count: usize,
        mermaid_compatibility_residual_count: usize,
        output_mutated: bool,
    }

    impl FamilyEvidenceSummary {
        #[allow(clippy::too_many_arguments)]
        pub(crate) const fn new(
            status: FamilyEvidenceStatus,
            required_count: usize,
            accounted_count: usize,
            applied_count: usize,
            not_applicable_count: usize,
            theme_residual_count: usize,
            source_residual_count: usize,
            compatibility_residual_count: usize,
            mermaid_compatibility_residual_count: usize,
            output_mutated: bool,
        ) -> Self {
            Self {
                status,
                required_count,
                accounted_count,
                applied_count,
                not_applicable_count,
                theme_residual_count,
                source_residual_count,
                compatibility_residual_count,
                mermaid_compatibility_residual_count,
                output_mutated,
            }
        }

        pub const fn status(self) -> FamilyEvidenceStatus {
            self.status
        }

        pub const fn required_count(self) -> usize {
            self.required_count
        }

        pub const fn accounted_count(self) -> usize {
            self.accounted_count
        }

        pub const fn applied_count(self) -> usize {
            self.applied_count
        }

        pub const fn not_applicable_count(self) -> usize {
            self.not_applicable_count
        }

        pub const fn theme_residual_count(self) -> usize {
            self.theme_residual_count
        }

        pub const fn source_residual_count(self) -> usize {
            self.source_residual_count
        }

        pub const fn compatibility_residual_count(self) -> usize {
            self.compatibility_residual_count
        }

        pub const fn mermaid_compatibility_residual_count(self) -> usize {
            self.mermaid_compatibility_residual_count
        }

        pub const fn output_mutated(self) -> bool {
            self.output_mutated
        }
    }

    /// Projects renderer-private family evidence into the bounded workspace facade summary.
    pub fn family_evidence(report: &FamilyRenderReport) -> FamilyEvidenceSummary {
        report.evidence_summary()
    }

    /// Bounded root-evidence projection used by the workspace facade.
    ///
    /// Mechanism keys, per-capability ledgers, and residual identities remain renderer-private.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RootEvidenceSummary {
        status: crate::diagram_theme::RootThemeVerification,
        required_count: usize,
        accounted_count: usize,
        applied_count: usize,
        residual_count: usize,
        output_mutated: bool,
    }

    impl RootEvidenceSummary {
        pub const fn status(self) -> crate::diagram_theme::RootThemeVerification {
            self.status
        }

        pub const fn required_count(self) -> usize {
            self.required_count
        }

        pub const fn accounted_count(self) -> usize {
            self.accounted_count
        }

        pub const fn applied_count(self) -> usize {
            self.applied_count
        }

        pub const fn residual_count(self) -> usize {
            self.residual_count
        }

        pub const fn output_mutated(self) -> bool {
            self.output_mutated
        }
    }

    /// Projects renderer-private root evidence without exporting the mechanism ledger.
    pub fn root_evidence(report: &FamilyRenderReport) -> RootEvidenceSummary {
        let root = report.root_theme_report();
        let required_count = root.required_mechanisms().len();
        let applied_count = root.applied_mechanisms().len();
        let residual_count = root.residuals().len();
        RootEvidenceSummary {
            status: root.verification(),
            required_count,
            accounted_count: applied_count.saturating_add(residual_count),
            applied_count,
            residual_count,
            output_mutated: root.residuals().iter().any(|residual| {
                residual.reason() == crate::diagram_theme::RootThemeResidualReason::OutputMutation
            }),
        }
    }

    /// Returns the bounded set of root capabilities proved by the terminal SVG consumer.
    pub fn root_applied_capabilities(
        report: &FamilyRenderReport,
    ) -> Box<[crate::diagram_theme::ThemeCapability]> {
        report
            .root_theme_report()
            .applied_capabilities()
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    /// Returns the exact State hard-shadow receipt frozen after SVG emission.
    pub fn family_native_filter_receipt(
        report: &FamilyRenderReport,
    ) -> Option<NativeSvgFilterReceipt> {
        report.native_filter_receipt()
    }

    /// Returns the exact host/theme resource-policy intersection captured by the session.
    pub fn effective_theme_resource_policy(
        report: &crate::environment::RenderSessionReport,
    ) -> &crate::diagram_theme::ThemeResourcePolicy {
        report.effective_theme_resource_policy()
    }

    /// Returns the terminal SVG carrying renderer-owned prepared-label locators.
    pub fn native_export_svg(svg: &crate::svg::ResvgCompatibleSvg) -> &str {
        svg.native_export_svg()
    }

    /// Returns per-label native-export evidence retained by the sealed SVG.
    pub fn prepared_text_label_ledger(
        svg: &crate::svg::ResvgCompatibleSvg,
    ) -> &[PreparedTextLabelLedgerEntry] {
        svg.prepared_text_label_ledger()
    }

    /// Returns the renderer-owned terminal receipt bound to the exact native SVG artifact.
    pub fn prepared_text_terminal_receipt(
        svg: &crate::svg::ResvgCompatibleSvg,
    ) -> Option<&PreparedTextTerminalReceipt> {
        svg.prepared_text_terminal_receipt()
    }

    /// Returns the number of prepared labels retained by the sealed native artifact.
    pub fn prepared_text_label_count(svg: &crate::svg::ResvgCompatibleSvg) -> usize {
        svg.prepared_text_label_ledger().len()
    }

    /// Reports whether the terminal pipeline preserved prepared-label locators.
    pub const fn prepared_text_evidence_valid(svg: &crate::svg::ResvgCompatibleSvg) -> bool {
        svg.prepared_text_evidence_valid()
    }

    /// Bounded prepared-math projection used by document and target admission.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PreparedMathEvidenceSummary {
        evidence_valid: bool,
        expected_occurrence_count: usize,
        terminal_occurrence_count: usize,
        terminal_artifact_digest: Option<[u8; 32]>,
    }

    impl PreparedMathEvidenceSummary {
        pub const fn not_applicable() -> Self {
            Self {
                evidence_valid: true,
                expected_occurrence_count: 0,
                terminal_occurrence_count: 0,
                terminal_artifact_digest: None,
            }
        }

        pub const fn evidence_valid(self) -> bool {
            self.evidence_valid
        }

        pub const fn expected_occurrence_count(self) -> usize {
            self.expected_occurrence_count
        }

        pub const fn terminal_occurrence_count(self) -> usize {
            self.terminal_occurrence_count
        }

        pub const fn terminal_artifact_digest(self) -> Option<[u8; 32]> {
            self.terminal_artifact_digest
        }

        pub const fn terminal_proof_complete(self) -> bool {
            self.evidence_valid
                && self.expected_occurrence_count == self.terminal_occurrence_count
                && (self.expected_occurrence_count == 0 || self.terminal_artifact_digest.is_some())
        }
    }

    /// Projects renderer-owned prepared-math evidence from the sealed native SVG.
    pub fn prepared_math_evidence(
        svg: &crate::svg::ResvgCompatibleSvg,
    ) -> PreparedMathEvidenceSummary {
        let receipt = svg.prepared_math_terminal_receipt();
        PreparedMathEvidenceSummary {
            evidence_valid: svg.prepared_math_evidence_valid(),
            expected_occurrence_count: svg.prepared_math_evidence_count(),
            terminal_occurrence_count: receipt.map_or(0, |receipt| receipt.occurrence_count()),
            terminal_artifact_digest: receipt.map(|receipt| receipt.artifact_digest()),
        }
    }

    /// Finalizes a standalone SVG while deferring the caller's portability requirement to the
    /// workspace facade's target-owned admission receipt.
    pub fn finalize_standalone_for_target_admission(
        rendered: crate::family::RenderedFamilySvg,
        pipeline: Option<&crate::svg::SvgPipeline>,
    ) -> crate::Result<crate::family::RenderedStandaloneSvg> {
        rendered.finalize_standalone_for_target_admission(pipeline)
    }

    /// Reports whether terminal SVG text is fully resolved by a renderer-owned font seal.
    pub const fn svg_text_fonts_are_self_contained(
        report: &crate::svg::SvgFinalizationReport,
    ) -> bool {
        report.font_seal().is_complete()
    }

    /// Installs a compiled theme's explicit Mermaid compatibility and selected-family bridge.
    pub fn install_parse_compatibility(
        theme: &crate::diagram_theme::DiagramTheme,
        engine: merman_core::Engine,
    ) -> merman_core::Engine {
        merman_core::__private::install_theme_compatibility(engine, theme.parse_compatibility())
    }
}

/// Reports whether the Cytoscape-derived layout backend is present in this compiled renderer.
pub const fn layout_cytoscape_available() -> bool {
    cfg!(feature = "layout-cytoscape")
}

/// Reports whether the ELK layout backend is present in this compiled renderer.
pub const fn layout_elk_available() -> bool {
    cfg!(feature = "layout-elk")
}

/// Reports whether the built-in math renderer is present in this compiled renderer.
pub const fn math_available() -> bool {
    cfg!(feature = "math")
}

/// Optional renderer capabilities that a typed diagram operation may require.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderCapability {
    LayoutCytoscape,
    LayoutElk,
    Math,
}

impl RenderCapability {
    const fn bit(self) -> u8 {
        match self {
            Self::LayoutCytoscape => 1 << 0,
            Self::LayoutElk => 1 << 1,
            Self::Math => 1 << 2,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::LayoutCytoscape => "layout-cytoscape",
            Self::LayoutElk => "layout-elk",
            Self::Math => "math",
        }
    }
}

const ALL_RENDER_CAPABILITY_BITS: u8 = RenderCapability::LayoutCytoscape.bit()
    | RenderCapability::LayoutElk.bit()
    | RenderCapability::Math.bit();

/// Operation-level permission for optional renderer capabilities.
///
/// The policy does not claim that a backend is compiled or that a service is installed. Effective
/// availability requires both this permission and the concrete backend/service. The unrestricted
/// default preserves the direct Rust renderer API, while artifact facades can project their exact
/// owner-selected capability contract without depending on binding-specific types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCapabilityPolicy {
    allowed_bits: u8,
}

impl RenderCapabilityPolicy {
    /// Allows every optional renderer capability that is otherwise available.
    pub const fn unrestricted() -> Self {
        Self {
            allowed_bits: ALL_RENDER_CAPABILITY_BITS,
        }
    }

    /// Denies every optional renderer capability until explicitly allowed.
    pub const fn deny_all() -> Self {
        Self { allowed_bits: 0 }
    }

    /// Allows one optional renderer capability.
    #[must_use]
    pub const fn with_allowed(mut self, capability: RenderCapability) -> Self {
        self.allowed_bits |= capability.bit();
        self
    }

    /// Reports whether this policy permits the capability.
    pub const fn allows(self, capability: RenderCapability) -> bool {
        self.allowed_bits & capability.bit() != 0
    }
}

impl Default for RenderCapabilityPolicy {
    fn default() -> Self {
        Self::unrestricted()
    }
}

impl std::fmt::Display for RenderCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
use crate::environment::RenderSession;
use crate::environment::{RoutedTextMeasurer, TextMeasurementPhase};
use merman_core::OperationPhase;
use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::models::class_diagram::ClassDiagram;

pub use resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, ClassComplexity, FlowchartComplexity,
    GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE, MindmapComplexity, RenderResourceLimitId,
    RenderResourcePolicy, RenderResourceProfile, RenderResourceProfileDescriptor,
    ResourceLimitCause, ResourceLimitDescriptor, ResourceLimitExceeded, ResourceLimitId,
    ResourceLimitOverride, ResourceLimitOverrideError, ResourceLimitPhase,
    ResourcePolicyRestrictionError, ZenumlComplexity, resource_limit_descriptors,
    resource_profile_descriptors,
};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Cancelled(#[from] merman_core::OperationCancelled),
    #[error("unsupported diagram type for layout: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },
    #[error("render session lacks capability `{capability}` required by diagram `{diagram_type}`")]
    MissingCapability {
        capability: RenderCapability,
        diagram_type: String,
    },
    #[error(
        "portable theme rendering rejected {residual_count} unverified family style residual(s) for `{family_id}`"
    )]
    UnverifiedFamilyStyle {
        family_id: DiagramFamilyId,
        residual_count: usize,
    },
    #[error(
        "portable theme rendering rejected {residual_count} structured family theme residual(s) for `{family_id}`"
    )]
    UnverifiedFamilyTheme {
        family_id: DiagramFamilyId,
        residual_count: usize,
    },
    #[error(
        "portable theme rendering rejected SVG output mutation after family `{family_id}` emitted its evidence"
    )]
    UnverifiedFamilyOutputMutation { family_id: DiagramFamilyId },
    #[error(
        "portable theme rendering rejected {residual_count} legacy Mermaid compatibility contribution(s) for `{family_id}`"
    )]
    LegacyFamilyThemeCompatibility {
        family_id: DiagramFamilyId,
        residual_count: usize,
    },
    #[error(
        "portable theme rendering rejected {residual_count} explicit Mermaid compatibility field(s) for `{family_id}`"
    )]
    MermaidThemeCompatibility {
        family_id: DiagramFamilyId,
        residual_count: usize,
    },
    #[error(
        "portable theme rendering cannot evaluate structured family styles for unadapted family `{family_id}`"
    )]
    UnadaptedFamilyTheme { family_id: DiagramFamilyId },
    #[error(
        "portable theme rendering has incomplete structured family evidence for `{family_id}`: accounted for {accounted_count} of {required_count} mechanism(s)"
    )]
    IncompleteFamilyTheme {
        family_id: DiagramFamilyId,
        required_count: usize,
        accounted_count: usize,
    },
    #[error(
        "portable theme rendering rejected root theme verification {verification:?} with {residual_count} residual(s)"
    )]
    RejectedRootTheme {
        verification: crate::diagram_theme::RootThemeVerification,
        residual_count: usize,
    },
    #[error("invalid semantic model: {message}")]
    InvalidModel { message: String },
    #[error("parsed diagram is bound to a different theme session")]
    ThemeParseBindingMismatch,
    #[error(transparent)]
    TextLayout(crate::text::TextLayoutFailure),
    #[error(transparent)]
    ThemeAdmission(#[from] crate::diagram_theme::ThemeAdmissionError),
    #[error(
        "custom JSON model `{model_name}` from {provenance:?} cannot render diagram type `{diagram_type}`"
    )]
    NonRenderableCustomModel {
        diagram_type: String,
        model_name: String,
        provenance: merman_core::CustomJsonProvenance,
    },
    #[error("SVG postprocessor `{pass}` failed: {message}")]
    SvgPostprocess { pass: String, message: String },
    #[error("external icon output is invalid: {message}")]
    InvalidIconOutput { message: String },
    #[error("icon rendering failed internally: {message}")]
    IconProcessing { message: String },
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[error(transparent)]
    ThemeResourceLimitExceeded(#[from] crate::diagram_theme::ThemeResourceLimitExceeded),
    #[error(transparent)]
    OperationResourceTerminal(merman_core::OperationLedgerError),
    #[error(transparent)]
    Color(#[from] merman_core::theme_color::ColorError),
    #[error("semantic model JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    OperationTimingUnavailable(#[from] merman_core::runtime::OperationTimingUnavailable),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<crate::text::TextLayoutError> for Error {
    fn from(error: crate::text::TextLayoutError) -> Self {
        Self::TextLayout(crate::text::TextLayoutFailure::from(&error))
    }
}

impl From<dugong::LayoutError> for Error {
    fn from(error: dugong::LayoutError) -> Self {
        Self::InvalidModel {
            message: error.to_string(),
        }
    }
}

impl From<crate::resources::OperationWorkError> for Error {
    fn from(error: crate::resources::OperationWorkError) -> Self {
        match error {
            crate::resources::OperationWorkError::Cancelled(error) => Self::Cancelled(error),
            crate::resources::OperationWorkError::ResourceLimitExceeded(error) => {
                Self::ResourceLimitExceeded(error)
            }
            crate::resources::OperationWorkError::ForeignResourceTerminal(error) => {
                Self::OperationResourceTerminal(error)
            }
        }
    }
}

impl Error {
    pub fn svg_postprocess(pass: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SvgPostprocess {
            pass: pass.into(),
            message: message.into(),
        }
    }

    pub(crate) fn invalid_icon_output(message: impl Into<String>) -> Self {
        Self::InvalidIconOutput {
            message: message.into(),
        }
    }

    pub(crate) fn icon_processing(message: impl Into<String>) -> Self {
        Self::IconProcessing {
            message: message.into(),
        }
    }

    pub const fn missing_capability(&self) -> Option<RenderCapability> {
        match self {
            Self::MissingCapability { capability, .. } => Some(*capability),
            _ => None,
        }
    }

    pub const fn unverified_family_style(&self) -> Option<(DiagramFamilyId, usize)> {
        match self {
            Self::UnverifiedFamilyStyle {
                family_id,
                residual_count,
            } => Some((*family_id, *residual_count)),
            _ => None,
        }
    }

    pub const fn unadapted_family_theme(&self) -> Option<DiagramFamilyId> {
        match self {
            Self::UnadaptedFamilyTheme { family_id } => Some(*family_id),
            _ => None,
        }
    }

    pub const fn incomplete_family_theme(&self) -> Option<(DiagramFamilyId, usize, usize)> {
        match self {
            Self::IncompleteFamilyTheme {
                family_id,
                required_count,
                accounted_count,
            } => Some((*family_id, *required_count, *accounted_count)),
            _ => None,
        }
    }

    pub const fn unverified_family_theme(&self) -> Option<(DiagramFamilyId, usize)> {
        match self {
            Self::UnverifiedFamilyTheme {
                family_id,
                residual_count,
            } => Some((*family_id, *residual_count)),
            _ => None,
        }
    }

    pub const fn rejected_root_theme(
        &self,
    ) -> Option<(crate::diagram_theme::RootThemeVerification, usize)> {
        match self {
            Self::RejectedRootTheme {
                verification,
                residual_count,
            } => Some((*verification, *residual_count)),
            _ => None,
        }
    }
}

/// Host-provided geometry available to family layout algorithms.
///
/// This models the element that owns diagram layout, not a browser page viewport and not the
/// final SVG viewport emitted after layout.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LayoutOptions {
    /// Width of the host layout container in CSS pixels.
    ///
    /// Families whose Mermaid renderer reads DOM-available width use this value.
    pub container_width: f64,
    /// Height of the host layout container in CSS pixels.
    pub container_height: f64,
    /// Browser `screen.availWidth` in CSS pixels when the host has a screen environment.
    ///
    /// Mermaid's C4 renderer uses the available screen width rather than its container width.
    /// `None` keeps headless rendering deterministic by falling back to `container_width`.
    pub screen_available_width: Option<f64>,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            container_width: 800.0,
            container_height: 600.0,
            screen_available_width: None,
        }
    }
}

impl LayoutOptions {
    /// Returns geometry defaults suitable for headless SVG rendering.
    pub fn headless_svg_defaults() -> Self {
        Self::default()
    }

    /// Sets the host layout container dimensions in CSS pixels.
    pub fn with_container_size(mut self, width: f64, height: f64) -> Self {
        self.container_width = width;
        self.container_height = height;
        self
    }

    /// Supplies the browser `screen.availWidth` observed by the host.
    pub fn with_screen_available_width(mut self, width: f64) -> Self {
        self.screen_available_width = Some(width);
        self
    }
}

pub(crate) struct LayoutExecution<'a> {
    request: &'a LayoutOptions,
    family: crate::family::FamilyExecutionView<'a>,
    text_measurer: RoutedTextMeasurer<'a>,
}

impl<'a> LayoutExecution<'a> {
    pub(crate) fn new(
        request: &'a LayoutOptions,
        family: crate::family::FamilyExecutionView<'a>,
    ) -> Self {
        Self {
            request,
            family,
            text_measurer: family
                .session()
                .controlled_text_measurer(TextMeasurementPhase::Layout, OperationPhase::Layout),
        }
    }

    #[cfg(test)]
    pub(crate) fn unthemed_for_test(
        request: &'a LayoutOptions,
        session: &'a RenderSession,
        family_id: DiagramFamilyId,
    ) -> Self {
        Self::new(
            request,
            crate::family::FamilyExecutionView::for_test(session, family_id),
        )
    }

    pub(crate) const fn family_id(&self) -> DiagramFamilyId {
        self.family.family_id()
    }

    pub(crate) fn state_style_plan(&self) -> Option<&crate::state::StateStylePlan> {
        self.family.style_plan().and_then(|plan| plan.state())
    }

    pub(crate) fn text_measurer(&self) -> &dyn crate::text::TextMeasurer {
        &self.text_measurer
    }

    pub(crate) fn prepared_text_layout(&self) -> Option<&crate::text::PreparedTextLayout> {
        self.family.session().prepared_text_layout()
    }

    pub(crate) fn resolved_theme(&self) -> Option<&crate::diagram_theme::ResolvedDiagramTheme> {
        self.family.resolved_theme()
    }

    pub(crate) fn math_renderer(&self) -> Option<&(dyn crate::math::MathRenderer + Send + Sync)> {
        self.family.session().math_renderer()
    }

    pub(crate) fn math_backend(&self) -> Option<&crate::math::ConfiguredMathBackend> {
        self.family.session().math_backend()
    }

    pub(crate) const fn resource_policy(&self) -> RenderResourcePolicy {
        self.family.session().resource_policy()
    }

    pub(crate) fn work_meter(&self) -> std::sync::Arc<crate::resources::OperationWorkMeter> {
        std::sync::Arc::clone(self.family.session().work_meter())
    }

    pub(crate) fn work_meter_ref(&self) -> &crate::resources::OperationWorkMeter {
        self.family.session().work_meter().as_ref()
    }

    pub(crate) fn local_time_zone(&self) -> &merman_core::time::LocalTimeZone {
        self.family.session().local_time_zone()
    }

    #[cfg(feature = "layout-cytoscape")]
    pub(crate) fn operation_seed(&self) -> u64 {
        self.family.session().render_seed().get()
    }

    #[cfg(feature = "layout-elk")]
    pub(crate) fn elk_operation_seed(&self) -> merman_layout_elk::ElkOperationSeed {
        // The ELK source port applies its own stable ELK-specific domain and graph-path
        // derivation. This token merely keeps every ELK random boundary tied to one immutable
        // render operation.
        merman_layout_elk::ElkOperationSeed::from_operation_seed(
            self.family.session().render_seed(),
        )
    }
}

impl std::ops::Deref for LayoutExecution<'_> {
    type Target = LayoutOptions;

    fn deref(&self) -> &Self::Target {
        self.request
    }
}

fn uses_elk_layout(effective_config: &merman_core::MermaidConfig) -> bool {
    effective_config.get_str("layout") == Some("elk")
}

pub(crate) fn layout_class_typed_by_engine(
    diagram_type: &str,
    model: &ClassDiagram,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
    typography_theme: &crate::class::ClassTypographyThemePlan,
) -> Result<model::ClassDiagramLayout> {
    if uses_elk_layout(effective_config) {
        return layout_class_elk_typed_by_feature(
            diagram_type,
            model,
            effective_config,
            options,
            typography_theme,
        );
    }

    options
        .work_meter_ref()
        .preflight_class_complexity(model, OperationPhase::Layout)?;
    let mut work_control = layout_work::OperationLayoutWorkControl::new(options.work_meter());
    let preparation_work = class::class_layout_work_units(model, &work_control)?;
    work_control.charge_adapter(preparation_work)?;
    class::layout_class_diagram_typed_with_config(
        model,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
        typography_theme,
        &mut work_control,
    )
}

#[cfg(feature = "layout-elk")]
fn layout_class_elk_typed_by_feature(
    _diagram_type: &str,
    model: &ClassDiagram,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
    typography_theme: &crate::class::ClassTypographyThemePlan,
) -> Result<model::ClassDiagramLayout> {
    options
        .work_meter_ref()
        .preflight_class_complexity(model, OperationPhase::Layout)?;
    let mut work_control = layout_work::OperationLayoutWorkControl::new(options.work_meter());
    let preparation_work = class::class_layout_work_units(model, &work_control)?;
    work_control.charge_adapter(preparation_work)?;
    class::layout_class_diagram_elk_typed_with_config_and_operation_seed(
        model,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
        options.elk_operation_seed(),
        typography_theme,
        &mut work_control,
    )
}

#[cfg(not(feature = "layout-elk"))]
fn layout_class_elk_typed_by_feature(
    diagram_type: &str,
    _model: &ClassDiagram,
    _effective_config: &merman_core::MermaidConfig,
    _options: &LayoutExecution<'_>,
    _typography_theme: &crate::class::ClassTypographyThemePlan,
) -> Result<model::ClassDiagramLayout> {
    Err(Error::MissingCapability {
        capability: RenderCapability::LayoutElk,
        diagram_type: diagram_type.to_string(),
    })
}

#[cfg(all(test, feature = "layout-elk"))]
pub(crate) fn layout_flowchart_typed_by_engine(
    diagram_type: &str,
    model: &FlowchartModel,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartLayout> {
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
        model,
        effective_config,
        false,
        options.work_meter_ref(),
    )?;
    layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_by_engine(
        diagram_type,
        model,
        &merman_core::diagrams::flowchart::FlowchartRenderContext::default(),
        effective_config,
        options,
        None,
        &edge_style_plan,
    )
}

pub(crate) fn layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_by_engine(
    diagram_type: &str,
    model: &FlowchartModel,
    render_label_sources: &merman_core::diagrams::flowchart::FlowchartRenderContext,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
    svg_label_sidecar: Option<&flowchart::FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<model::FlowchartLayout> {
    if uses_elk_layout(effective_config) {
        return layout_flowchart_elk_typed_by_feature(
            diagram_type,
            model,
            render_label_sources,
            effective_config,
            options,
            svg_label_sidecar,
            edge_style_plan,
        );
    }

    flowchart::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter(
        model,
        render_label_sources,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
        svg_label_sidecar,
        edge_style_plan,
        options.work_meter(),
    )
}

#[cfg(feature = "layout-elk")]
fn layout_flowchart_elk_typed_by_feature(
    _diagram_type: &str,
    model: &FlowchartModel,
    render_label_sources: &merman_core::diagrams::flowchart::FlowchartRenderContext,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
    svg_label_sidecar: Option<&flowchart::FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<model::FlowchartLayout> {
    flowchart::elk::layout_flowchart_elk_typed_with_render_labels_and_operation_seed(
        model,
        render_label_sources,
        effective_config,
        flowchart::elk::FlowchartElkLayoutExecution::new(
            options.text_measurer(),
            options.math_renderer(),
            options.elk_operation_seed(),
            svg_label_sidecar,
            edge_style_plan,
            options.work_meter(),
        ),
    )
}

#[cfg(not(feature = "layout-elk"))]
fn layout_flowchart_elk_typed_by_feature(
    diagram_type: &str,
    _model: &FlowchartModel,
    _render_label_sources: &merman_core::diagrams::flowchart::FlowchartRenderContext,
    _effective_config: &merman_core::MermaidConfig,
    _options: &LayoutExecution<'_>,
    _svg_label_sidecar: Option<&flowchart::FlowchartSvgLabelSidecarBuilder>,
    _edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<model::FlowchartLayout> {
    Err(Error::MissingCapability {
        capability: RenderCapability::LayoutElk,
        diagram_type: diagram_type.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "layout-elk")]
    use merman_core::ParsedDiagramRender;
    #[cfg(feature = "layout-elk")]
    use merman_core::RenderSemanticModel;
    use merman_core::{Engine, ParseOptions};

    #[test]
    fn render_capability_ids_and_error_accessor_are_stable() {
        for (capability, stable_id) in [
            (RenderCapability::LayoutCytoscape, "layout-cytoscape"),
            (RenderCapability::LayoutElk, "layout-elk"),
            (RenderCapability::Math, "math"),
        ] {
            assert_eq!(capability.id(), stable_id);
            assert_eq!(capability.to_string(), stable_id);

            let error = Error::MissingCapability {
                capability,
                diagram_type: "contract-test".to_string(),
            };
            assert_eq!(error.missing_capability(), Some(capability));
            assert_eq!(
                error.to_string(),
                format!(
                    "render session lacks capability `{stable_id}` required by diagram `contract-test`"
                )
            );
        }
    }

    #[test]
    fn render_capability_policy_is_an_explicit_allow_mask() {
        let denied = RenderCapabilityPolicy::deny_all();
        for capability in [
            RenderCapability::LayoutCytoscape,
            RenderCapability::LayoutElk,
            RenderCapability::Math,
        ] {
            assert!(!denied.allows(capability));
            assert!(
                RenderCapabilityPolicy::unrestricted().allows(capability),
                "unrestricted policy denied {}",
                capability.id()
            );
        }

        let math_only = denied.with_allowed(RenderCapability::Math);
        assert!(math_only.allows(RenderCapability::Math));
        assert!(!math_only.allows(RenderCapability::LayoutCytoscape));
        assert!(!math_only.allows(RenderCapability::LayoutElk));
    }

    #[cfg(feature = "layout-elk")]
    fn flowchart_layout(
        parsed: &ParsedDiagramRender,
        options: &LayoutOptions,
        session: &RenderSession,
    ) -> model::FlowchartLayout {
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected flowchart render model");
        };
        layout_flowchart_typed_by_engine(
            &parsed.metadata().diagram_type,
            model,
            &parsed.metadata().effective_config,
            &LayoutExecution::unthemed_for_test(options, session, DiagramFamilyId::FLOWCHART),
        )
        .expect("flowchart layout")
    }

    #[cfg(feature = "layout-elk")]
    fn class_layout(
        parsed: &ParsedDiagramRender,
        options: &LayoutOptions,
        session: &RenderSession,
    ) -> model::ClassDiagramLayout {
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected class render model");
        };
        let typography_theme = crate::class::ClassTypographyThemePlan::resolve(
            None,
            &parsed.metadata().effective_config,
        );
        layout_class_typed_by_engine(
            &parsed.metadata().diagram_type,
            model,
            &parsed.metadata().effective_config,
            &LayoutExecution::unthemed_for_test(options, session, DiagramFamilyId::CLASS),
            &typography_theme,
        )
        .expect("class layout")
    }

    fn render_source(
        source: &str,
        layout_options: &LayoutOptions,
        svg_options: &crate::svg::SvgRenderOptions,
    ) -> String {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse")
            .expect("diagram");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        crate::family::prepare(parsed, layout_options, session)
            .expect("prepare")
            .render_svg(svg_options, &crate::svg::SvgDebugOptions::default())
            .expect("render")
            .svg()
            .to_owned()
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn elk_operation_seed_is_captured_once_per_render_operation() {
        fn capture(seed: u64) -> merman_layout_elk::ElkOperationSeed {
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_runtime_policy(
                    merman_core::runtime::RuntimePolicy::deterministic().with_fixed_seed(seed),
                )
                .begin_session()
                .expect("render session");
            LayoutExecution::unthemed_for_test(
                &LayoutOptions::default(),
                &session,
                DiagramFamilyId::ERROR,
            )
            .elk_operation_seed()
        }

        assert_eq!(capture(17), capture(17));
        assert_ne!(capture(17), capture(18));
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_model_dispatch_accepts_diagram_type_aliases() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let artifact = crate::family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
        assert_eq!(artifact.family_id(), crate::DiagramFamilyId::FLOWCHART);
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_model_dispatch_uses_elk_for_flowchart_default_renderer_config() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  flowchart:
    defaultRenderer: elk
---
flowchart TD
A-->B
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "flowchart-elk");
        let layout = flowchart_layout(&parsed, &LayoutOptions::default(), &session);
        let a = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = layout.nodes.iter().find(|node| node.id == "B").unwrap();
        assert!(b.y > a.y);
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_model_dispatch_rejects_flowchart_over_node_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
    }

    #[test]
    fn render_model_dispatch_rejects_flowchart_dagre_work_during_its_first_owner_phase() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nsubgraph Cluster\nA\nend\nA-->B",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("expected flowchart layout work resource limit"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_layout_work_units");
        assert!(limit.actual > 1);
    }

    fn assert_class_layout_work_limit(source: &str) {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let error = match crate::family::prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("Class layout unexpectedly bypassed the work budget"),
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Class layout work resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_layout_work_units");
        assert!(limit.actual > 1);
        assert_eq!(limit.max, 1);
    }

    #[test]
    fn dagre_class_layout_honors_the_public_work_budget() {
        assert_class_layout_work_limit("classDiagram\nA --> B\nA --> C\nA --> D\nB --> C\nC --> D");
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn elk_class_layout_honors_the_public_work_budget() {
        assert_class_layout_work_limit(
            r#"---
config:
  layout: elk
---
classDiagram
A --> B
A --> C
A --> D
B --> C
C --> D"#,
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_model_dispatch_uses_elk_for_class_layout_config() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  layout: elk
---
classDiagram
direction LR
Animal <|-- Duck
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "class");
        let layout = class_layout(&parsed, &LayoutOptions::default(), &session);
        let animal = layout
            .nodes
            .iter()
            .find(|node| node.id == "Animal")
            .unwrap();
        let duck = layout.nodes.iter().find(|node| node.id == "Duck").unwrap();
        assert!(
            duck.x > animal.x,
            "ELK LR class layout should place Duck to the right of Animal; Animal={}, Duck={}",
            animal.x,
            duck.x
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_model_dispatch_rejects_class_over_node_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nAnimal <|-- Duck",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn typed_dispatch_rejects_flowchart_over_edge_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA-->B\nB-->C\nC-->D",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 2)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn typed_dispatch_rejects_class_over_edge_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nAnimal <|-- Duck\nDuck <|-- Mallard",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn canonical_svg_preserves_flowchart_elk_roledescription() {
        let svg = render_source(
            "flowchart-elk TD\nA-->B;",
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("elk-smoke".to_string()),
                ..Default::default()
            },
        );

        let document_scope = "elk-smoke-merman-flowchart-document";
        let marker_id = format!("{document_scope}_flowchart-elk-pointEnd");
        let filter_id = format!("{document_scope}-filter-drop-shadow");
        assert!(svg.contains(r#"aria-roledescription="flowchart-elk""#));
        assert!(svg.contains(&marker_id));
        assert!(!svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(!svg.contains(r#"<g class="root""#));

        let marker_pos = svg
            .find(&format!(r#"<g><marker id="{marker_id}""#))
            .expect("ELK marker group");
        let defs_pos = svg
            .find(&format!(r#"<defs><filter id="{filter_id}""#))
            .expect("ELK shadow defs");
        let subgraphs_pos = svg
            .find(r#"<g class="subgraphs"/>"#)
            .expect("ELK subgraphs group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");

        assert!(marker_pos < defs_pos);
        assert!(defs_pos < subgraphs_pos);
        assert!(subgraphs_pos < nodes_pos);
        assert!(nodes_pos < edges_pos);
        assert!(edges_pos < labels_pos);
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn canonical_svg_uses_elk_adapter_dom_for_flowchart_layout_elk() {
        let svg = render_source(
            r#"---
config:
  layout: elk
---
flowchart LR
A{A} --> B & C
"#,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("layout-elk-smoke".to_string()),
                ..Default::default()
            },
        );

        let document_scope = "layout-elk-smoke-merman-flowchart-document";
        let marker_id = format!("{document_scope}_flowchart-v2-pointEnd");
        let filter_id = format!("{document_scope}-filter-drop-shadow");
        assert!(svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(svg.contains(&marker_id));
        assert!(!svg.contains(r#"<g class="root""#));

        let marker_pos = svg
            .find(&format!(r#"<g><marker id="{marker_id}""#))
            .expect("ELK marker group");
        let defs_pos = svg
            .find(&format!(r#"<defs><filter id="{filter_id}""#))
            .expect("ELK shadow defs");
        let subgraphs_pos = svg
            .find(r#"<g class="subgraphs"/>"#)
            .expect("ELK subgraphs group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");

        assert!(marker_pos < defs_pos);
        assert!(defs_pos < subgraphs_pos);
        assert!(subgraphs_pos < nodes_pos);
        assert!(nodes_pos < edges_pos);
        assert!(edges_pos < labels_pos);
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn canonical_svg_uses_right_angle_edges_for_flowchart_elk() {
        let svg = render_source(
            "flowchart-elk LR\nA --> B\nA --> C",
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions::default(),
        );

        let path = edge_path_chunk(&svg, "L_A_B_0");
        let d = edge_path_d(path);
        assert!(
            d.contains('L') && !d.contains('C'),
            "expected ELK edges to use right-angle paths without smooth curves by default: {d}"
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn canonical_svg_keeps_source_ported_elk_rect_edge_boundary_points() {
        let svg = render_source(
            r#"---
config:
  htmlLabels: true
  flowchart:
    htmlLabels: true
  securityLevel: loose
---
flowchart-elk LR
id1(Start)-->id2(Stop)
"#,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions::default(),
        );

        let path = edge_path_chunk(&svg, "L_id1_id2_0");
        let d = edge_path_d(path);
        assert!(
            !d.contains('Q'),
            "straight ELK roundedRect edge should not gain a rounded corner: {d}"
        );
        let points = edge_data_points(path);
        assert_eq!(
            points.len(),
            2,
            "unexpected ELK edge data-points: {points:?}"
        );
        assert_eq!(points[0].1, points[1].1);
        assert!(
            points[0].0 < points[1].0,
            "ELK edge must travel left to right: {points:?}"
        );
        assert!(
            (points[1].0 - points[0].0 - 40.0).abs() < 1.0e-9,
            "ELK straight edge must retain the source-backed node span: {points:?}"
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn canonical_svg_keeps_source_ported_elk_self_loop_edges() {
        let svg = render_source(
            "flowchart-elk TD\nA --> A",
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions::default(),
        );

        let path = edge_path_chunk(&svg, "L_A_A_0");
        let d = edge_path_d(path);
        assert!(
            d.contains('Q'),
            "ELK self-loop path should be rendered from the source-backed edge: {d}"
        );
        let points = edge_data_points(path);
        assert_eq!(
            points.len(),
            4,
            "unexpected ELK self-loop data-points: {points:?}"
        );
        assert!(
            !svg.contains("A---A---1") && !svg.contains("cyclic-special"),
            "ELK renderer must not reuse Dagre self-loop helper nodes: {svg}"
        );
        assert!(svg.contains(r#"data-id="L_A_A_0" transform="translate(0,0)""#));
    }

    #[cfg(not(feature = "layout-elk"))]
    #[test]
    fn render_model_dispatch_rejects_flowchart_elk_without_feature() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let err = match crate::family::prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("expected unsupported diagram error"),
        };
        assert!(matches!(
            err,
            Error::MissingCapability {
                capability: RenderCapability::LayoutElk,
                diagram_type,
            }
                if diagram_type == "flowchart-elk"
        ));
    }

    #[cfg(not(feature = "layout-elk"))]
    #[test]
    fn render_model_dispatch_rejects_class_elk_without_feature() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  layout: elk
---
classDiagram
Animal <|-- Duck
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let err = match crate::family::prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("expected unsupported diagram error"),
        };
        assert!(matches!(
            err,
            Error::MissingCapability {
                capability: RenderCapability::LayoutElk,
                diagram_type,
            }
                if diagram_type == "class"
        ));
    }

    #[test]
    fn render_model_dispatch_renders_cynefin_svg() {
        let source = r#"cynefin-beta
  title Team Practices
  accTitle: Cynefin map
  accDescr: Practice movement
  complex
    "Pair programming"
  complicated
    "Architecture review"
  complex --> complicated : "Pattern emerges"
"#;
        let svg = render_source(
            source,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("cynefin-test".to_string()),
                ..Default::default()
            },
        );

        assert!(svg.contains(r#"aria-roledescription="cynefin""#));
        assert!(svg.contains(r#"<g class="cynefin-backgrounds">"#));
        assert!(svg.contains(r#"class="cynefinDomain""#));
        assert!(svg.contains(r#"class="cynefinBoundary""#));
        assert!(svg.contains(r#"class="cynefinCliff""#));
        assert!(svg.contains(r#"class="cynefinItem""#));
        assert!(svg.contains("Pair programming"));
        assert!(svg.contains(r#"class="cynefinArrowLine""#));
        assert!(svg.contains("Pattern emerges"));
        assert!(svg.contains(r#"<title id="chart-title-cynefin-test">Cynefin map</title>"#));
        assert!(svg.contains(r#"<desc id="chart-desc-cynefin-test">Practice movement</desc>"#));
        assert!(svg.contains("#cynefin-test .cynefinDomain{stroke:none;}"));
        assert_eq!(svg.matches("<title").count(), 2, "{svg}");
        assert_eq!(svg.matches("<desc").count(), 2, "{svg}");

        let scoped_title = svg
            .find(r#"<title id="chart-title-cynefin-test">"#)
            .expect("scoped accessibility title");
        let scoped_descr = svg
            .find(r#"<desc id="chart-desc-cynefin-test">"#)
            .expect("scoped accessibility description");
        let style = svg.find("<style>").expect("style");
        let framework_group = svg.find("<g/>").expect("Mermaid framework group");
        let renderer_title = svg
            .find("<title>Cynefin map</title>")
            .expect("renderer accessibility title");
        let renderer_descr = svg
            .find("<desc>Practice movement</desc>")
            .expect("renderer accessibility description");
        let root_group = svg
            .find(r#"<g transform="translate("#)
            .expect("cynefin root group");
        let defs = svg.find("<defs>").expect("transition marker defs");

        assert!(scoped_title < scoped_descr, "{svg}");
        assert!(scoped_descr < style, "{svg}");
        assert!(style < framework_group, "{svg}");
        assert!(framework_group < renderer_title, "{svg}");
        assert!(renderer_title < renderer_descr, "{svg}");
        assert!(renderer_descr < root_group, "{svg}");
        assert!(root_group < defs, "{svg}");
    }

    #[test]
    fn render_model_dispatch_keeps_whitespace_cynefin_transition_labels() {
        let source = r#"cynefin-beta
  complex
  complicated
  complex --> complicated : "   "
"#;
        let svg = render_source(
            source,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("cynefin-whitespace".to_string()),
                ..Default::default()
            },
        );

        assert!(
            svg.contains(r#"class="cynefinArrowLabel""#),
            "a whitespace-only label is truthy in JavaScript and must emit a text node: {svg}"
        );
    }

    #[test]
    fn render_model_dispatch_renders_railroad_svg() {
        let source = r#"railroad-beta
accTitle: Railroad grammar
accDescr: Expression grammar
expr = sequence(nonterminal("term"), optional(special("guard")), zeroOrMore(terminal("+"))) ;
"#;
        let svg = render_source(
            source,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("railroad-test".to_string()),
                ..Default::default()
            },
        );

        assert!(svg.contains(r#"aria-roledescription="railroad""#));
        assert!(svg.contains(r#"class="railroad-diagram""#));
        assert!(svg.contains(r#"class="railroad-rule""#));
        assert!(svg.contains(r#"class="railroad-rule-name""#));
        assert!(svg.contains(r#"class="railroad-nonterminal""#));
        assert!(svg.contains(r#"class="railroad-special""#));
        assert!(svg.contains(r#"class="railroad-terminal""#));
        assert!(svg.contains(r#"class="railroad-line""#));
        assert!(svg.contains("term"));
        assert!(svg.contains("? guard ?"));
        assert!(svg.contains("+"));
        assert!(svg.contains(r#"<title id="chart-title-railroad-test">Railroad grammar</title>"#));
        assert!(svg.contains(r#"<desc id="chart-desc-railroad-test">Expression grammar</desc>"#));
        assert!(
            svg.contains("</style><g/><g class=\"railroad-rule\""),
            "{svg}"
        );
    }

    #[cfg(feature = "layout-elk")]
    fn edge_path_chunk<'a>(svg: &'a str, edge_id: &str) -> &'a str {
        let semantic_attrs = format!(r#"data-et="edge" data-id="{edge_id}""#);
        let semantic_start = svg.find(&semantic_attrs).expect("semantic edge id");
        let path_start = svg[..semantic_start]
            .rfind("<path ")
            .expect("edge path start");
        let path_end = svg[semantic_start..].find("/>").expect("edge path end") + semantic_start;
        &svg[path_start..path_end]
    }

    #[cfg(feature = "layout-elk")]
    fn edge_path_d(path: &str) -> &str {
        let d_start = path.find(r#"d=""#).expect("edge path d") + r#"d=""#.len();
        let d_end = path[d_start..].find('"').expect("edge path d end") + d_start;
        &path[d_start..d_end]
    }

    #[cfg(feature = "layout-elk")]
    fn edge_attr_value<'a>(path: &'a str, attr: &str) -> &'a str {
        let needle = format!(r#"{attr}=""#);
        let start = path.find(&needle).expect("edge attr") + needle.len();
        let end = path[start..].find('"').expect("edge attr end") + start;
        &path[start..end]
    }

    #[cfg(feature = "layout-elk")]
    fn edge_data_points(path: &str) -> Vec<(f64, f64)> {
        use base64::Engine as _;

        let b64 = edge_attr_value(path, "data-points");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .expect("data-points base64");
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("data-points JSON payload");
        json.as_array()
            .expect("data-points array")
            .iter()
            .map(|point| {
                (
                    point.get("x").and_then(serde_json::Value::as_f64).unwrap(),
                    point.get("y").and_then(serde_json::Value::as_f64).unwrap(),
                )
            })
            .collect()
    }
}
