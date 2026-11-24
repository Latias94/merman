#![forbid(unsafe_code)]

//! Headless layout + rendering for Mermaid diagrams.
//!
//! This crate consumes `merman-model` presentation data and produces:
//! - a layout JSON (geometry + routes)
//! - Mermaid-like SVG output with DOM parity checks against upstream baselines
//!
//! This low-level crate has no default diagram families. Select `diagram-*` features or
//! `all-diagrams` explicitly; optional layout engines and math remain independent selectors.
//! Family-owned typed layout APIs are conditional on those selections. Use
//! [`family::supports_diagram_type`] to inspect this renderer's local handler availability:
//! another dependency may enable additional model families without enabling their renderers.

#[cfg(all(
    feature = "diagram-architecture",
    any(feature = "layout-cytoscape", feature = "layout-shoal")
))]
pub mod architecture;
#[cfg(all(
    feature = "diagram-architecture",
    any(feature = "layout-cytoscape", feature = "layout-shoal")
))]
pub(crate) mod architecture_metrics;
#[cfg(feature = "diagram-block")]
pub mod block;
#[cfg(feature = "diagram-c4")]
pub mod c4;
#[cfg(feature = "diagram-xychart")]
mod chart_palette;
#[cfg(feature = "diagram-class")]
pub mod class;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod config;
#[cfg(feature = "diagram-cynefin")]
pub mod cynefin;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-state"
))]
mod dagre;
#[cfg(feature = "layout-elk")]
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod elk_adapter;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod elk_edge_geometry;
#[cfg(feature = "layout-elk")]
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod elk_feedback_edges;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod elk_geometry;
#[cfg(feature = "layout-elk")]
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod elk_hierarchy;
#[cfg(feature = "layout-elk")]
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod elk_options;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod elk_terminal_jogs;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod entities;
pub mod environment;
#[cfg(feature = "diagram-er")]
pub mod er;
pub mod error;
#[cfg(feature = "diagram-event-modeling")]
pub mod eventmodeling;
pub mod family;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub mod flowchart;
#[cfg(feature = "diagram-gantt")]
pub mod gantt;
mod generated;
pub mod geom;
#[cfg(feature = "diagram-git-graph")]
pub mod gitgraph;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-block",
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-mindmap"
))]
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod graph_label;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-requirement",
    feature = "diagram-state",
    feature = "diagram-usecase",
    all(
        any(
            feature = "diagram-architecture",
            feature = "diagram-block",
            feature = "diagram-c4"
        ),
        feature = "layout-shoal"
    )
))]
mod graph_layout;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared HTML controls have different callers in each diagram selection."
    )
)]
mod html;
#[cfg(feature = "diagram-info")]
pub mod info;
#[cfg(feature = "diagram-ishikawa")]
pub mod ishikawa;
#[cfg(feature = "diagram-journey")]
pub mod journey;
#[cfg(feature = "diagram-kanban")]
pub mod kanban;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod layout_backend;
mod layout_frame;
pub use layout_frame::ShoalLayoutFrame;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod layout_work;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
pub mod math;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod mermaid_style;
#[cfg(feature = "diagram-mindmap")]
pub mod mindmap;
pub mod model;
mod number_format;
#[cfg(feature = "diagram-packet")]
pub mod packet;
#[cfg(feature = "diagram-pie")]
pub mod pie;
#[cfg(feature = "diagram-quadrant-chart")]
pub mod quadrantchart;
#[cfg(feature = "diagram-radar")]
pub mod radar;
#[cfg(feature = "diagram-railroad")]
pub mod railroad;
mod raster_data_url;
#[cfg(feature = "diagram-requirement")]
pub mod requirement;
pub mod resources;
#[cfg(feature = "diagram-sankey")]
pub mod sankey;
#[cfg(feature = "diagram-sequence")]
pub mod sequence;
#[cfg(feature = "diagram-state")]
pub mod state;
pub mod svg;
pub mod svg_security;
#[cfg(any(
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-mindmap"
))]
pub mod swimlane;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
pub mod text;
pub mod theme;
#[cfg(feature = "diagram-timeline")]
pub mod timeline;
#[cfg(feature = "diagram-tree-view")]
pub mod tree_view;
#[cfg(feature = "diagram-treemap")]
pub mod treemap;

#[cfg(feature = "diagram-usecase")]
pub mod usecase;
#[cfg(feature = "diagram-venn")]
pub mod venn;
#[cfg(feature = "diagram-wardley")]
pub mod wardley;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod xml;
#[rustfmt::skip]
#[allow(
    clippy::collapsible_if,
    clippy::filter_next,
    reason = "Keep the pinned upstream XML implementation directly comparable to its source."
)]
#[allow(
    dead_code,
    reason = "The private XML owner retains the upstream API for source review and retirement."
)]
mod xml_dom;
#[cfg(feature = "diagram-xychart")]
pub mod xychart;
#[cfg(feature = "diagram-zenuml")]
pub mod zenuml;

pub use merman_model::family::DiagramFamilyId;

/// Workspace-internal implementation seams with no compatibility promise.
///
/// Bounded facade and actual-output projections may evolve independently of the public renderer API.
#[doc(hidden)]
pub mod __private {
    pub use crate::family::native_typography::{
        unsupported_native_html_style, unsupported_native_text_style,
    };
    pub use crate::family::{FamilyLayoutArtifact, prepare_layout};

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

    /// Reports whether terminal SVG text is fully resolved by a renderer-owned font seal.
    pub const fn svg_text_fonts_are_self_contained(
        report: &crate::svg::SvgFinalizationReport,
    ) -> bool {
        report.font_seal().is_complete()
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

/// Reports whether the experimental Shoal layout backend is compiled into this renderer.
pub const fn layout_shoal_available() -> bool {
    cfg!(feature = "layout-shoal")
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
    LayoutShoal,
}

impl RenderCapability {
    const fn bit(self) -> u8 {
        match self {
            Self::LayoutCytoscape => 1 << 0,
            Self::LayoutElk => 1 << 1,
            Self::Math => 1 << 2,
            Self::LayoutShoal => 1 << 3,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::LayoutCytoscape => "layout-cytoscape",
            Self::LayoutElk => "layout-elk",
            Self::Math => "math",
            Self::LayoutShoal => "layout-shoal",
        }
    }
}

const ALL_RENDER_CAPABILITY_BITS: u8 = RenderCapability::LayoutCytoscape.bit()
    | RenderCapability::LayoutElk.bit()
    | RenderCapability::Math.bit()
    | RenderCapability::LayoutShoal.bit();

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

    /// Denies one optional capability while preserving the other permissions.
    #[must_use]
    pub const fn with_denied(mut self, capability: RenderCapability) -> Self {
        self.allowed_bits &= !capability.bit();
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
use merman_model::OperationPhase;
#[cfg(feature = "diagram-class")]
use merman_model::families::class::ClassDiagram;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
use merman_model::families::flowchart::FlowchartModel;

pub use resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE, RenderResourceLimitId,
    RenderResourcePolicy, RenderResourceProfile, RenderResourceProfileDescriptor,
    ResourceLimitCause, ResourceLimitDescriptor, ResourceLimitExceeded, ResourceLimitId,
    ResourceLimitOverride, ResourceLimitOverrideError, ResourceLimitPhase,
    ResourcePolicyRestrictionError, resource_limit_descriptors, resource_profile_descriptors,
};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Cancelled(#[from] merman_model::OperationCancelled),
    #[error("unsupported diagram type for layout: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },
    #[error("layout backend `{backend}` is unsupported for diagram `{diagram_type}`")]
    UnsupportedLayoutBackend {
        backend: &'static str,
        diagram_type: String,
    },
    #[error("layout backend `{backend}` does not support {construct}")]
    UnsupportedLayoutConstruct {
        backend: &'static str,
        construct: &'static str,
    },
    #[error("layout backend `{backend}` could not allocate layout storage")]
    LayoutAllocationFailed { backend: &'static str },
    #[error("layout adaptation `{adaptation}` is unsupported for diagram `{diagram_type}`")]
    UnsupportedLayoutAdaptation {
        adaptation: &'static str,
        diagram_type: String,
    },
    #[error("fit-width layout.container_width must be finite and positive")]
    InvalidAdaptationWidth,
    #[error("render session lacks capability `{capability}` required by diagram `{diagram_type}`")]
    MissingCapability {
        capability: RenderCapability,
        diagram_type: String,
    },
    #[error(
        "portable native theme rendering for `{family_id}` cannot apply a requested feature. {message}"
    )]
    UnsupportedNativeThemeFeature {
        family_id: DiagramFamilyId,
        message: &'static str,
    },
    #[error(transparent)]
    SemanticProjection(merman_model::presentation::compat::CompatProjectionError),
    #[error(transparent)]
    PresentationConfig(#[from] merman_model::presentation::PresentationConfigError),
    #[error("invalid semantic model: {message}")]
    InvalidModel { message: String },
    #[error(
        "render target `{target}` does not support RequirePortable because it has no target portability admission"
    )]
    PortabilityUnavailableForTarget { target: &'static str },
    #[error("trusted theme lane `raw-theme-css` is disabled by the host")]
    UntrustedThemeCss,
    #[error(
        "custom JSON model `{model_name}` from {provenance:?} cannot render diagram type `{diagram_type}`"
    )]
    NonRenderableCustomModel {
        diagram_type: String,
        model_name: String,
        provenance: merman_model::presentation::CustomJsonProvenance,
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
    OperationResourceTerminal(merman_model::OperationLedgerError),
    #[error(transparent)]
    Color(#[from] merman_markup::theme_color::ColorError),
    #[error("semantic model JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    OperationTimingUnavailable(#[from] merman_model::runtime::OperationTimingUnavailable),
}

pub type Result<T> = std::result::Result<T, Error>;

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

    #[cfg_attr(
        not(feature = "all-diagrams"),
        allow(
            dead_code,
            reason = "Shared operation support has different consumers in each diagram selection."
        )
    )]
    pub(crate) fn invalid_icon_output(message: impl Into<String>) -> Self {
        Self::InvalidIconOutput {
            message: message.into(),
        }
    }

    #[cfg_attr(
        not(feature = "all-diagrams"),
        allow(
            dead_code,
            reason = "Shared operation support has different consumers in each diagram selection."
        )
    )]
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
}

/// Explicit placement adaptation requested by the host.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutAdaptation {
    #[default]
    None,
    FitWidth,
}

impl LayoutAdaptation {
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::FitWidth => "fit-width",
        }
    }
}

/// Actual layout width observed after explicit adaptation, before SVG viewport scaling.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct LayoutAdaptationObservation {
    mode: LayoutAdaptation,
    requested_width: f64,
    actual_width: f64,
    overflowed: bool,
}

// Both widths are finite at construction.
impl Eq for LayoutAdaptationObservation {}

impl LayoutAdaptationObservation {
    #[cfg(any(feature = "diagram-kanban", feature = "layout-shoal"))]
    pub(crate) fn fit_width(requested_width: f64, actual_width: f64) -> Self {
        debug_assert!(requested_width.is_finite() && requested_width > 0.0);
        debug_assert!(actual_width.is_finite() && actual_width >= 0.0);
        Self {
            mode: LayoutAdaptation::FitWidth,
            requested_width,
            actual_width,
            overflowed: actual_width > requested_width,
        }
    }

    pub const fn mode(self) -> LayoutAdaptation {
        self.mode
    }
    pub const fn requested_width(self) -> f64 {
        self.requested_width
    }
    pub const fn actual_width(self) -> f64 {
        self.actual_width
    }
    pub const fn overflowed(self) -> bool {
        self.overflowed
    }
}

/// Host-provided geometry available to family layout algorithms.
///
/// This models the element that owns diagram layout, not a browser page viewport and not the
/// final SVG viewport emitted after layout.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LayoutOptions {
    /// Opt-in family placement behavior. Defaults retain Mermaid-compatible layout.
    pub adaptation: LayoutAdaptation,
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
    /// Allow eligible Shoal leaf fans to occupy multiple rows during fit-width layout.
    pub fold_wide_ranks: bool,
    adaptation_if_supported: bool,
    pub(crate) capture_shoal_frame: bool,
    pub(crate) previous_shoal_frame: Option<std::sync::Arc<ShoalLayoutFrame>>,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            adaptation: LayoutAdaptation::None,
            container_width: 800.0,
            container_height: 600.0,
            screen_available_width: None,
            fold_wide_ranks: false,
            adaptation_if_supported: false,
            capture_shoal_frame: false,
            previous_shoal_frame: None,
        }
    }
}

impl LayoutOptions {
    /// Returns geometry defaults suitable for headless SVG rendering.
    pub fn headless_svg_defaults() -> Self {
        Self::default()
    }

    /// Selects explicit family placement adaptation.
    pub fn with_adaptation(mut self, adaptation: LayoutAdaptation) -> Self {
        self.adaptation = adaptation;
        self.adaptation_if_supported = false;
        self
    }

    /// Prefers adapted placement for diagrams whose selected layout supports it.
    pub fn with_adaptation_if_supported(mut self, adaptation: LayoutAdaptation) -> Self {
        self.adaptation = adaptation;
        self.adaptation_if_supported = true;
        self
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

    /// Sets wide-rank fan folding for a Shoal fit-width operation.
    pub fn with_fold_wide_ranks(mut self, fold: bool) -> Self {
        self.fold_wide_ranks = fold;
        self
    }

    /// Requests a Shoal preview frame and optionally reuses hints from the same diagram.
    pub fn with_shoal_layout_frame(
        mut self,
        previous: Option<std::sync::Arc<ShoalLayoutFrame>>,
    ) -> Self {
        self.capture_shoal_frame = true;
        self.previous_shoal_frame = previous;
        self
    }
}

#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared operation support has different consumers in each diagram selection."
    )
)]
pub(crate) struct LayoutExecution<'a> {
    request: &'a LayoutOptions,
    family: crate::family::FamilyExecutionView<'a>,
    text_measurer: RoutedTextMeasurer<'a>,
}

#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared operation support has different consumers in each diagram selection."
    )
)]
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

    pub(crate) fn text_measurer(&self) -> &dyn crate::text::TextMeasurer {
        &self.text_measurer
    }

    #[allow(dead_code, reason = "Used by enabled diagram families.")]
    pub(crate) fn native_family_theme<A: crate::theme::FamilyThemeAdapter>(
        &self,
    ) -> Option<&crate::theme::FamilyTheme> {
        self.family.native_theme::<A>()
    }

    pub(crate) fn math_renderer(&self) -> Option<&(dyn crate::math::MathRenderer + Send + Sync)> {
        self.family.session().math_renderer()
    }

    pub(crate) fn math_backend(&self) -> Option<&crate::math::ConfiguredMathBackend> {
        self.family.session().math_backend()
    }

    pub(crate) fn work_meter(&self) -> std::sync::Arc<crate::resources::OperationWorkMeter> {
        std::sync::Arc::clone(self.family.session().work_meter())
    }

    pub(crate) fn work_meter_ref(&self) -> &crate::resources::OperationWorkMeter {
        self.family.session().work_meter().as_ref()
    }

    /// Transfers scratch only from the preview frame selected by this operation.
    #[cfg(feature = "layout-shoal")]
    pub(crate) fn shoal_preview_scratch(
        &self,
        previous: Option<&ShoalLayoutFrame>,
    ) -> merman_layout_shoal::LayoutScratch {
        use merman_layout_shoal::LayoutScratch;

        let policy = self.family.session().resource_policy();
        let limit = ResourceLimitId::MaxLayoutWorkUnits;
        let constrained = RenderResourcePolicy::constrained().value(limit);
        if policy.profile() == RenderResourceProfile::Constrained
            || policy
                .value(limit)
                .is_some_and(|units| constrained.is_some_and(|cap| units <= cap))
        {
            return LayoutScratch::with_max_retained_bytes(0);
        }
        previous
            .and_then(ShoalLayoutFrame::take_scratch)
            .unwrap_or_default()
    }

    pub(crate) fn local_time_zone(&self) -> &merman_model::time::LocalTimeZone {
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

#[cfg(feature = "diagram-class")]
pub(crate) fn layout_class_typed_by_engine(
    model: &ClassDiagram,
    effective_config: &merman_model::presentation::PresentationConfig,
    options: &LayoutExecution<'_>,
    render_config: &crate::class::ClassRenderConfig,
) -> Result<(
    model::ClassDiagramLayout,
    class::ClassPreparedLabels,
    graph_layout::RouteContract,
)> {
    use graph_layout::RouteContract;

    options
        .work_meter_ref()
        .preflight_class_complexity(model, OperationPhase::Layout)?;
    let mut work_control = layout_work::OperationLayoutWorkControl::new(options.work_meter());
    let preparation_work = class::class_layout_work_units(model, &work_control)?;
    work_control.charge_adapter(preparation_work)?;
    let selection = layout_backend::resolve_graph_layout(effective_config);
    selection.validate_rootless_graph()?;
    #[cfg(feature = "layout-shoal")]
    if selection.backend == layout_backend::GraphLayoutBackend::Shoal {
        return class::shoal::layout(
            model,
            effective_config,
            options,
            render_config,
            &mut work_control,
        )
        .map(|(layout, labels, routes)| (layout, labels, RouteContract::canonical_graph(routes)));
    }
    #[cfg(feature = "layout-elk")]
    if selection.backend == layout_backend::GraphLayoutBackend::Elk {
        return class::layout_class_diagram_elk_typed_with_config_and_operation_seed(
            model,
            effective_config,
            options.text_measurer(),
            options.math_renderer(),
            options.elk_operation_seed(),
            render_config,
            &mut work_control,
        )
        .map(|(layout, labels)| (layout, labels, RouteContract::ElkParity));
    }
    class::layout_class_diagram_typed_with_config(
        model,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
        render_config,
        &mut work_control,
    )
    .map(|(layout, labels)| (layout, labels, RouteContract::DagreParity))
}

#[cfg(all(test, feature = "layout-elk"))]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn layout_flowchart_typed_by_engine(
    model: &FlowchartModel,
    effective_config: &merman_model::presentation::PresentationConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartLayout> {
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
        model,
        effective_config,
        false,
        options.work_meter_ref(),
    )?;
    layout_flowchart_typed_with_render_labels_by_engine(
        model,
        &merman_model::families::flowchart::FlowchartRenderContext::default(),
        &flowchart::FlowchartClassDefinitions::new(&model.class_defs),
        effective_config,
        options,
        None,
        &edge_style_plan,
    )
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn layout_flowchart_typed_with_render_labels_by_engine(
    model: &FlowchartModel,
    render_label_sources: &merman_model::families::flowchart::FlowchartRenderContext,
    class_defs: &flowchart::FlowchartClassDefinitions<'_>,
    effective_config: &merman_model::presentation::PresentationConfig,
    options: &LayoutExecution<'_>,
    svg_label_sidecar: Option<&flowchart::FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<model::FlowchartLayout> {
    options
        .family
        .session()
        .checkpoint(OperationPhase::Layout)?;
    let selection = layout_backend::resolve_graph_layout(effective_config);
    selection.validate_rootless_graph()?;
    if selection.backend == layout_backend::GraphLayoutBackend::Shoal {
        #[cfg(feature = "layout-shoal")]
        return flowchart::shoal::layout(
            model,
            render_label_sources,
            class_defs,
            effective_config,
            options,
            svg_label_sidecar,
            edge_style_plan,
        );
        #[cfg(not(feature = "layout-shoal"))]
        return Err(Error::MissingCapability {
            capability: RenderCapability::LayoutShoal,
            diagram_type: options.family_id().as_str().to_owned(),
        });
    }
    #[cfg(feature = "layout-elk")]
    if selection.backend == layout_backend::GraphLayoutBackend::Elk {
        return flowchart::elk::layout_flowchart_elk_typed_with_render_labels_and_operation_seed(
            model,
            render_label_sources,
            class_defs,
            effective_config,
            flowchart::elk::FlowchartElkLayoutExecution::new(
                options.text_measurer(),
                options.math_renderer(),
                options.elk_operation_seed(),
                svg_label_sidecar,
                edge_style_plan,
                options.work_meter(),
            ),
        );
    }

    flowchart::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter(
        model,
        render_label_sources,
        class_defs,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
        svg_label_sidecar,
        edge_style_plan,
        options.work_meter(),
    )
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests {
    use super::*;
    use merman_core::{MermaidEngine, ParseOptions};
    #[cfg(feature = "layout-elk")]
    use merman_model::presentation::PresentationDiagram;
    #[cfg(feature = "layout-elk")]
    use merman_model::presentation::RenderSemanticModel;

    #[test]
    fn render_capability_ids_and_error_accessor_are_stable() {
        for (capability, stable_id) in [
            (RenderCapability::LayoutCytoscape, "layout-cytoscape"),
            (RenderCapability::LayoutElk, "layout-elk"),
            (RenderCapability::Math, "math"),
            (RenderCapability::LayoutShoal, "layout-shoal"),
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
            RenderCapability::LayoutShoal,
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
        parsed: &PresentationDiagram,
        options: &LayoutOptions,
        session: &RenderSession,
    ) -> model::FlowchartLayout {
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected flowchart render model");
        };
        layout_flowchart_typed_by_engine(
            model,
            &parsed.metadata().effective_config,
            &LayoutExecution::unthemed_for_test(options, session, DiagramFamilyId::FLOWCHART),
        )
        .expect("flowchart layout")
    }

    #[cfg(feature = "layout-elk")]
    fn class_layout(
        parsed: &PresentationDiagram,
        options: &LayoutOptions,
        session: &RenderSession,
    ) -> model::ClassDiagramLayout {
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected class render model");
        };
        let typography_theme =
            crate::class::ClassTextThemePlan::resolve(None, &parsed.metadata().effective_config);
        let render_config = crate::class::ClassRenderConfig::resolve(
            &parsed.metadata().effective_config,
            &typography_theme,
        );
        layout_class_typed_by_engine(
            model,
            &parsed.metadata().effective_config,
            &LayoutExecution::unthemed_for_test(options, session, DiagramFamilyId::CLASS),
            &render_config,
        )
        .expect("class layout")
        .0
    }

    fn render_source(
        source: &str,
        layout_options: &LayoutOptions,
        svg_options: &crate::svg::SvgRenderOptions,
    ) -> String {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse")
            .expect("diagram")
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        crate::family::prepare(&parsed, layout_options, session)
            .expect("prepare")
            .render_svg(svg_options, &crate::svg::SvgDebugOptions::default())
            .expect("render")
            .svg()
            .to_owned()
    }

    #[cfg(feature = "layout-elk")]
    fn element_with_id_suffix_position(svg: &str, tag: &str, suffix: &str) -> usize {
        let needle = format!("<{tag}");
        svg.match_indices(&needle)
            .find_map(|(position, _)| {
                let end = svg[position..].find('>')? + position;
                let element = &svg[position..=end];
                let id = element
                    .split_once(r#"id="#)
                    .and_then(|(_, value)| value.strip_prefix('"'))
                    .and_then(|value| value.split('"').next())?;
                id.ends_with(suffix).then_some(position)
            })
            .unwrap_or_else(|| panic!("missing <{tag}> with id suffix {suffix:?}"))
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn elk_operation_seed_is_captured_once_per_render_operation() {
        fn capture(seed: u64) -> merman_layout_elk::ElkOperationSeed {
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_runtime_policy(
                    merman_model::runtime::RuntimePolicy::deterministic().with_fixed_seed(seed),
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
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();

        let artifact = crate::family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        assert_eq!(artifact.family_id(), crate::DiagramFamilyId::FLOWCHART);
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_model_dispatch_uses_elk_for_scoped_flowchart_layout() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  flowchart:
    layout: elk
---
flowchart TD
A-->B
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();

        assert_eq!(parsed.metadata().diagram_type, "flowchart-v2");
        let layout = flowchart_layout(&parsed, &LayoutOptions::default(), &session);
        assert!(layout.uses_elk_adapter_dom);
        let a = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = layout.nodes.iter().find(|node| node.id == "B").unwrap();
        assert!(b.y > a.y);
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_model_dispatch_rejects_flowchart_over_node_resource_limit() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(&parsed, &options, session) {
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
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nsubgraph Cluster\nA\nend\nA-->B",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(&parsed, &LayoutOptions::default(), session) {
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
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("class source should produce a render model")
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let error = match crate::family::prepare(&parsed, &LayoutOptions::default(), session) {
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
        let parsed = MermaidEngine::new()
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
            .unwrap()
            .into_presentation();

        assert_eq!(parsed.metadata().diagram_type, "classDiagram");
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
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nAnimal <|-- Duck",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(&parsed, &options, session) {
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
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA-->B\nB-->C\nC-->D",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 2)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(&parsed, &options, session) {
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
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nAnimal <|-- Duck\nDuck <|-- Mallard",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxModelItems, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(&parsed, &options, session) {
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

        assert!(svg.contains(r#"aria-roledescription="flowchart-elk""#));
        assert!(svg.contains("elk-smoke-merman-flowchart-document_flowchart-elk-pointEnd"));
        assert!(!svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(svg.contains(r#"<g class="root""#));

        let marker_pos = element_with_id_suffix_position(&svg, "marker", "-pointEnd");
        let root_pos = svg.find(r#"<g class="root">"#).expect("ELK root group");
        let clusters_pos = svg
            .find(r#"<g class="clusters"/>"#)
            .expect("ELK clusters group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let defs_pos = element_with_id_suffix_position(&svg, "filter", "-drop-shadow");

        assert!(marker_pos < root_pos);
        assert!(root_pos < clusters_pos);
        assert!(clusters_pos < edges_pos);
        assert!(edges_pos < labels_pos);
        assert!(labels_pos < nodes_pos);
        assert!(nodes_pos < defs_pos);
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

        assert!(svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(svg.contains("layout-elk-smoke-merman-flowchart-document_flowchart-v2-pointEnd"));
        assert!(svg.contains(r#"<g class="root""#));

        let marker_pos = element_with_id_suffix_position(&svg, "marker", "-pointEnd");
        let root_pos = svg.find(r#"<g class="root">"#).expect("ELK root group");
        let clusters_pos = svg
            .find(r#"<g class="clusters"/>"#)
            .expect("ELK clusters group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let defs_pos = element_with_id_suffix_position(&svg, "filter", "-drop-shadow");

        assert!(marker_pos < root_pos);
        assert!(root_pos < clusters_pos);
        assert!(clusters_pos < edges_pos);
        assert!(edges_pos < labels_pos);
        assert!(labels_pos < nodes_pos);
        assert!(nodes_pos < defs_pos);
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
        assert_eq!(edge_attr_value(path, "data-id"), "L_A_A_0");
    }

    #[test]
    fn graph_layout_unregistered_loader_falls_back_to_dagre() {
        let mut requests = vec!["unknown", "ELK", "elk.layered", "elk.radial"];
        if !cfg!(feature = "layout-elk") {
            requests.extend([
                "elk",
                "elk.stress",
                "elk.force",
                "elk.mrtree",
                "elk.sporeOverlap",
                "elk.box",
                "elk.rectpacking",
            ]);
        }
        for body in [
            "flowchart TD\nA[Alpha] --> B[Beta]\nB --> C[Gamma]",
            "classDiagram\nAnimal <|-- Duck",
            "erDiagram\nCUSTOMER ||--o{ ORDER : places",
        ] {
            let render = |layout: &str| {
                let source = format!("---\nconfig:\n  layout: {layout}\n---\n{body}\n");
                let parsed = MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = crate::environment::RenderEnvironment::deterministic()
                    .begin_session()
                    .unwrap();
                let plan = crate::family::plan_render(&parsed, &session).unwrap();
                assert!(
                    !plan
                        .required_capabilities()
                        .contains(&RenderCapability::LayoutElk)
                );
                assert!(plan.missing_capabilities().is_empty());
                let artifact = crate::family::prepare(&parsed, &LayoutOptions::default(), session)
                    .expect("unregistered loader falls back before capability admission");
                let geometry = artifact.layout_json().unwrap()["layout"].clone();
                let svg = artifact
                    .render_svg(
                        &crate::svg::SvgRenderOptions {
                            diagram_id: Some("fallback-test".into()),
                            ..Default::default()
                        },
                        &crate::svg::SvgDebugOptions::default(),
                    )
                    .unwrap()
                    .svg()
                    .to_owned();
                (geometry, svg)
            };
            let expected = render("dagre");
            for request in &requests {
                assert_eq!(render(request), expected, "{request}: {body}");
            }
        }
    }

    #[cfg(not(feature = "layout-elk"))]
    #[test]
    fn flowchart_elk_alias_uses_dagre_geometry_when_loader_is_absent() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let artifact = crate::family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let geometry = artifact.layout_json().unwrap()["layout"].clone();
        let dagre_parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let dagre_session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let dagre_artifact =
            crate::family::prepare(&dagre_parsed, &LayoutOptions::default(), dagre_session)
                .unwrap();
        assert_eq!(geometry, dagre_artifact.layout_json().unwrap()["layout"]);
        let svg = artifact
            .render_svg(
                &crate::svg::SvgRenderOptions::default(),
                &crate::svg::SvgDebugOptions::default(),
            )
            .unwrap()
            .svg()
            .to_owned();
        assert!(svg.contains("edgePath"), "{svg}");
    }

    #[test]
    fn registered_cose_graph_failure_is_not_replaced_by_dagre() {
        for body in [
            "flowchart LR\nA-->B\nA-->C",
            "agentflow-beta\nA-->B",
            "usecase-beta\nactor A\nA --> B",
            "classDiagram\nA --> B",
            "stateDiagram-v2\nA --> B",
            "erDiagram\nA ||--o{ B : relates",
            "requirementDiagram\nrequirement r {\nid: 1\ntext: Demo\nrisk: low\nverifymethod: test\n}",
        ] {
            for layout in ["cose-bilkent", "unknown-layout-probe"] {
                let source = format!("---\nconfig:\n  layout: {layout}\n---\n{body}\n");
                let parsed = MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = crate::environment::RenderEnvironment::deterministic()
                    .begin_session()
                    .unwrap();
                let result = crate::family::prepare(&parsed, &LayoutOptions::default(), session);
                if layout == "cose-bilkent" && cfg!(feature = "layout-cytoscape") {
                    assert!(
                        matches!(result, Err(Error::InvalidModel { message }) if message.contains("Root node is required")),
                        "{body}"
                    );
                } else {
                    result.unwrap_or_else(|error| panic!("{body}: {layout}: {error}"));
                }
            }
        }
    }

    #[test]
    fn shoal_rejects_mindmap_during_planning_and_execution() {
        let source = "---\nconfig:\n  layout: shoal\n---\nmindmap\n  root((Demo))\n    Leaf\n";
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for error in [
            family::plan_render(&parsed, &session).err(),
            family::prepare(&parsed, &LayoutOptions::default(), session).err(),
        ] {
            let Some(Error::UnsupportedLayoutBackend {
                backend,
                diagram_type,
            }) = error
            else {
                panic!("expected an unsupported Shoal layout for Mindmap, got {error:?}");
            };
            assert_eq!(backend, "shoal");
            assert_eq!(diagram_type, parsed.metadata().diagram_type);
        }
    }

    #[test]
    fn shoal_respects_scoped_source_site_and_keyword_selection() {
        use merman_core::presentation::SourcePresentationConfig;
        let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
            serde_json::json!({ "layout": "shoal", "flowchart": { "layout": "dagre" } }),
        ));
        for (source, expected) in [
            ("flowchart LR\nA --> B", None),
            (
                "---\nconfig:\n  layout: shoal\n---\ngraph LR\nA --> B",
                Some(RenderCapability::LayoutShoal),
            ),
            (
                "---\nconfig:\n  layout: dagre\n  flowchart:\n    layout: shoal\n---\nflowchart LR\nA --> B",
                Some(RenderCapability::LayoutShoal),
            ),
            ("classDiagram\nA --> B", Some(RenderCapability::LayoutShoal)),
            (
                "stateDiagram-v2\nA --> B",
                Some(RenderCapability::LayoutShoal),
            ),
            (
                "erDiagram\nA ||--o{ B : relates",
                Some(RenderCapability::LayoutShoal),
            ),
            (
                "requirementDiagram\nrequirement r {\nid: 1\ntext: Demo\nrisk: low\nverifymethod: test\n}",
                Some(RenderCapability::LayoutShoal),
            ),
            (
                "usecase-beta\nactor A\nA --> B",
                Some(RenderCapability::LayoutShoal),
            ),
            (
                "---\nconfig:\n  flowchart:\n    layout: shoal\n---\nflowchart-elk LR\nA --> B",
                layout_elk_available().then_some(RenderCapability::LayoutElk),
            ),
            ("sequenceDiagram\nA->>B: Hello", None),
            (
                "swimlane-beta LR\nA --> B",
                Some(RenderCapability::LayoutShoal),
            ),
            (
                "architecture-beta\nservice api(server)[API]",
                Some(RenderCapability::LayoutShoal),
            ),
            (
                "---\nconfig:\n  layout: cose-bilkent\n---\narchitecture-beta\nservice api(server)[API]",
                Some(RenderCapability::LayoutCytoscape),
            ),
        ] {
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert_eq!(
                plan.required_capabilities(),
                expected.as_slice(),
                "{source}"
            );
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    #[test]
    fn registered_cose_host_denial_precedes_missing_root_failure() {
        let source = "---\nconfig:\n  layout: cose-bilkent\n---\nusecase-beta\nactor A\nA --> B\n";
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_capability_policy(RenderCapabilityPolicy::deny_all())
            .begin_session()
            .unwrap();
        let plan = crate::family::plan_render(&parsed, &session).unwrap();
        assert_eq!(
            plan.missing_capabilities(),
            &[RenderCapability::LayoutCytoscape]
        );
        assert!(matches!(
            crate::family::prepare(&parsed, &LayoutOptions::default(), session),
            Err(Error::MissingCapability {
                capability: RenderCapability::LayoutCytoscape,
                ..
            })
        ));
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn registered_elk_denied_by_host_policy_does_not_fall_back() {
        for body in [
            "flowchart TD\nA-->B",
            "classDiagram\nAnimal <|-- Duck",
            "erDiagram\nCUSTOMER ||--o{ ORDER : places",
        ] {
            for layout in [
                "elk",
                "elk.stress",
                "elk.force",
                "elk.mrtree",
                "elk.sporeOverlap",
                "elk.box",
                "elk.rectpacking",
            ] {
                let source = format!("---\nconfig:\n  layout: {layout}\n---\n{body}\n");
                let parsed = MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = crate::environment::RenderEnvironment::deterministic()
                    .with_capability_policy(RenderCapabilityPolicy::deny_all())
                    .begin_session()
                    .unwrap();
                let plan = crate::family::plan_render(&parsed, &session).unwrap();
                assert_eq!(plan.missing_capabilities(), &[RenderCapability::LayoutElk]);
                assert!(matches!(
                    crate::family::prepare(&parsed, &LayoutOptions::default(), session),
                    Err(Error::MissingCapability {
                        capability: RenderCapability::LayoutElk,
                        ..
                    })
                ));
            }
        }
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

#[cfg(feature = "diagram-class")]
pub use resources::ClassComplexity;

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub use resources::FlowchartComplexity;

#[cfg(feature = "diagram-mindmap")]
pub use resources::MindmapComplexity;

#[cfg(feature = "diagram-sequence")]
pub use resources::SequenceComplexity;

#[cfg(feature = "diagram-zenuml")]
pub use resources::ZenumlComplexity;

#[cfg(test)]
mod test_support;
