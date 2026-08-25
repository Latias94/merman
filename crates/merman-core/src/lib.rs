#![forbid(unsafe_code)]
//! Mermaid parser + semantic model (headless).
//!
//! Design goals:
//! - 1:1 parity with the repository's pinned upstream Mermaid baseline
//! - deterministic, testable outputs (semantic snapshot goldens)
//! - runtime-agnostic async APIs (no specific executor required)

pub mod baseline;
pub mod common;
pub mod common_db;
mod compatibility_json;
pub mod config;
pub mod detect;
pub mod diagram;
pub mod diagrams;
pub mod editor;
pub mod entities;
pub mod error;
mod family;
pub mod generated;
pub mod geom;
mod inline_config;
pub mod models;
pub mod operation;
mod parse_pipeline;
pub mod preprocess;
pub mod resources;
pub mod runtime;
pub mod sanitize;
pub mod style;
pub mod svg_security;
#[doc(hidden)]
pub mod terminal_text;
mod theme;
pub mod theme_color;
pub mod time;
pub mod utils;
mod yaml_config;

pub use config::MermaidConfig;
#[cfg(test)]
use config::{ConfigOverlayProvenance, ThemeParseBinding};
use config::{PostDetectionConfigOverlay, PostDetectionConfigOverlayProvider};
pub use detect::{Detector, DetectorRegistry};
pub use diagram::{
    BLOCK_WIDTH_WARNING_RULE_ID, BuiltinRenderSemantic, CapturedPanic, CustomJsonProvenance,
    CustomJsonRenderModel, CustomJsonRenderParser, DiagramParseOutcome, DiagramParseSnapshot,
    DiagramRegistry, DiagramSemanticParser, DiagramSnapshotCapture, DiagramWarningFact,
    FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID, FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID,
    GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID, ParsedDiagram, ParsedDiagramRender,
    ParsedEditorFacts, RenderDiagramRegistry, RenderSemanticModel,
};
pub use editor::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorFamilySemantics, EditorRenamePolicy,
    EditorSemanticCompleteness, EditorSemanticDiagnostic, EditorSemanticDiagnosticKind,
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol, SourceSpan,
};
pub use error::{
    Error, ParseDiagnostic, ParseDiagnosticSpanKind, Result, ThemeEvaluationLimitExceeded,
};
pub use family::{
    BuiltInTypedRenderFamily, DiagramFamilyCapability, DiagramFamilyId, DiagramHeaderFact,
    diagram_type_family_id, diagram_type_metadata_id,
};
pub use operation::{
    CancelReason, OperationCancelled, OperationControl, OperationControlResult,
    OperationLedgerError, OperationPhase, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance,
};
pub use preprocess::{
    PreprocessResult, PreprocessedSource, preprocess_diagram, preprocess_diagram_with_known_type,
};
pub use theme::{MermaidThemeId, MermaidThemeIdParseError};

/// Workspace-internal compatibility seam used while typed family adapters replace the legacy
/// Mermaid config bridge.
///
/// This module is intentionally outside Merman's supported API. Its opaque plans and evidence
/// keep the temporary overlay graph, provider trait, contribution identifiers, and parse binding
/// private to `merman-core`.
#[doc(hidden)]
pub mod __private {
    use std::collections::BTreeMap;
    use std::fmt;
    use std::sync::Arc;

    use crate::config::{
        ConfigOverlayContribution, ConfigOverlayContributionProvenance, ConfigOverlayError,
        FrozenThemeCompatibilityField, PostDetectionConfigOverlay,
        PostDetectionConfigOverlayProvider, ThemeCompatibilityFieldKind as ConfigFieldKind,
        ThemeParseBinding, ThemeParseBindingError,
    };
    use crate::{
        Engine, FallbackPostDetectionConfigOverlay, MermaidConfig, OperationControl,
        OperationControlResult, ParseMetadata,
    };

    /// Opaque normalized identity for one compiled theme's parse compatibility contract.
    #[derive(Clone, PartialEq, Eq)]
    pub struct ThemeCompatibilityRecipe(ThemeParseBinding);

    impl fmt::Debug for ThemeCompatibilityRecipe {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("ThemeCompatibilityRecipe")
                .field("recipe_identity", &self.0.recipe_identity())
                .finish_non_exhaustive()
        }
    }

    /// Invalid bounded Mermaid compatibility input for an internal compiled theme.
    #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
    #[error(transparent)]
    pub struct ThemeCompatibilityPlanError(#[from] ThemeParseBindingError);

    /// Invalid bounded family compatibility contribution.
    #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
    #[error(transparent)]
    pub struct ThemeCompatibilityOverlayError(#[from] ConfigOverlayError);

    /// Opaque selected-family overlay returned by a lazy compatibility resolver.
    #[derive(Debug, Clone)]
    pub struct ThemeFamilyCompatibilityOverlay(Arc<PostDetectionConfigOverlay>);

    impl Default for ThemeFamilyCompatibilityOverlay {
        fn default() -> Self {
            Self(Arc::new(PostDetectionConfigOverlay::new()))
        }
    }

    impl ThemeFamilyCompatibilityOverlay {
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
    }

    /// Bounded builder for one selected family's temporary compatibility contributions.
    #[derive(Debug)]
    pub struct ThemeFamilyCompatibilityOverlayBuilder {
        family: String,
        contribution_prefix: String,
        overlay: PostDetectionConfigOverlay,
    }

    impl ThemeFamilyCompatibilityOverlayBuilder {
        pub fn new(family: impl Into<String>, contribution_prefix: impl Into<String>) -> Self {
            Self {
                family: family.into(),
                contribution_prefix: contribution_prefix.into(),
                overlay: PostDetectionConfigOverlay::new(),
            }
        }

        pub fn try_push(
            &mut self,
            contribution_label: &str,
            patch: MermaidConfig,
        ) -> Result<(), ThemeCompatibilityOverlayError> {
            let opaque_id = format!(
                "{}{}.{}",
                self.contribution_prefix, self.family, contribution_label
            );
            let contribution = ConfigOverlayContribution::new(opaque_id, patch)?;
            self.overlay = std::mem::take(&mut self.overlay)
                .with_family_contribution(self.family.clone(), contribution)?;
            Ok(())
        }

        pub fn finish(self) -> ThemeFamilyCompatibilityOverlay {
            ThemeFamilyCompatibilityOverlay(Arc::new(self.overlay))
        }
    }

    type ThemeCompatibilityResolver = dyn Fn(
            &str,
            &OperationControl,
        ) -> OperationControlResult<Option<ThemeFamilyCompatibilityOverlay>>
        + Send
        + Sync;

    struct ThemeCompatibilityProvider {
        resolver: Arc<ThemeCompatibilityResolver>,
    }

    impl fmt::Debug for ThemeCompatibilityProvider {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("ThemeCompatibilityProvider")
                .finish_non_exhaustive()
        }
    }

    impl PostDetectionConfigOverlayProvider for ThemeCompatibilityProvider {
        fn overlay_for_family(
            &self,
            family: &str,
            control: &OperationControl,
        ) -> OperationControlResult<Option<Arc<PostDetectionConfigOverlay>>> {
            (self.resolver)(family, control).map(|overlay| overlay.map(|overlay| overlay.0))
        }
    }

    /// Opaque parse/install plan owned by one compiled diagram theme.
    #[derive(Clone)]
    pub struct ThemeCompatibilityPlan {
        recipe: ThemeCompatibilityRecipe,
        provider: Arc<dyn PostDetectionConfigOverlayProvider>,
    }

    impl fmt::Debug for ThemeCompatibilityPlan {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("ThemeCompatibilityPlan")
                .field("recipe", &self.recipe)
                .finish_non_exhaustive()
        }
    }

    impl ThemeCompatibilityPlan {
        pub fn try_new(
            recipe_identity: [u8; 32],
            compatibility_config: MermaidConfig,
            resolver: impl Fn(
                &str,
                &OperationControl,
            )
                -> OperationControlResult<Option<ThemeFamilyCompatibilityOverlay>>
            + Send
            + Sync
            + 'static,
        ) -> Result<Self, ThemeCompatibilityPlanError> {
            let recipe = ThemeCompatibilityRecipe(ThemeParseBinding::try_new(
                recipe_identity,
                compatibility_config,
            )?);
            Ok(Self {
                recipe,
                provider: Arc::new(ThemeCompatibilityProvider {
                    resolver: Arc::new(resolver),
                }),
            })
        }

        pub fn recipe(&self) -> &ThemeCompatibilityRecipe {
            &self.recipe
        }
    }

    /// Installs one compiled theme's complete parse compatibility plan on an engine.
    pub fn install_theme_compatibility(
        mut engine: Engine,
        plan: &ThemeCompatibilityPlan,
    ) -> Engine {
        engine.theme_compatibility_config = Some(MermaidConfig::from_theme_parse_binding(
            plan.recipe.0.clone(),
        ));
        engine.rebuild_site_config();
        engine.fallback_post_detection_config_overlay = Some(
            FallbackPostDetectionConfigOverlay::Provider(Arc::clone(&plan.provider)),
        );
        engine
    }

    /// One bounded compatibility contribution and the assignments that survived finalization.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ThemeCompatibilityContributionEvidence {
        provenance: ConfigOverlayContributionProvenance,
        surviving_assignments: Arc<[ThemeCompatibilityAssignmentEvidence]>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ThemeCompatibilityAssignmentEvidence {
        path: Arc<str>,
        value: Arc<serde_json::Value>,
    }

    impl ThemeCompatibilityContributionEvidence {
        fn from_frozen(
            provenance: ConfigOverlayContributionProvenance,
            effective_config: &MermaidConfig,
        ) -> Self {
            let surviving_assignments = provenance
                .surviving_assignment_paths()
                .filter_map(|path| {
                    config_value_at_path(effective_config, path).map(|value| {
                        ThemeCompatibilityAssignmentEvidence {
                            path: Arc::from(path),
                            value: Arc::new(value.clone()),
                        }
                    })
                })
                .collect::<Vec<_>>()
                .into();
            Self {
                provenance,
                surviving_assignments,
            }
        }

        pub fn opaque_id(&self) -> &str {
            self.provenance.opaque_id()
        }

        pub fn surviving_assignment_paths(&self) -> impl ExactSizeIterator<Item = &str> {
            self.surviving_assignments
                .iter()
                .map(|assignment| assignment.path.as_ref())
        }

        /// Returns the post-finalization value owned by this contribution at one surviving path.
        pub fn surviving_assignment_value(
            &self,
            assignment_path: &str,
        ) -> Option<&serde_json::Value> {
            self.surviving_assignments
                .iter()
                .find(|assignment| assignment.path.as_ref() == assignment_path)
                .map(|assignment| assignment.value.as_ref())
        }
    }

    fn config_value_at_path<'a>(
        config: &'a MermaidConfig,
        dotted_path: &str,
    ) -> Option<&'a serde_json::Value> {
        let mut current = config.as_value();
        for segment in dotted_path.split('.') {
            current = current.as_object()?.get(segment)?;
        }
        Some(current)
    }

    /// Conceptual Mermaid compatibility field retained after config precedence is finalized.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ThemeCompatibilityFieldKind {
        Theme,
        DarkMode,
        Variable,
    }

    impl ThemeCompatibilityFieldKind {
        fn from_config(kind: ConfigFieldKind) -> Self {
            match kind {
                ConfigFieldKind::Theme => Self::Theme,
                ConfigFieldKind::DarkMode => Self::DarkMode,
                ConfigFieldKind::Variable => Self::Variable,
            }
        }
    }

    /// Frozen identity and surviving physical paths for one conceptual compatibility field.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ThemeCompatibilityFieldEvidence {
        opaque_id: Arc<str>,
        kind: ThemeCompatibilityFieldKind,
        surviving_assignment_paths: Arc<[Arc<str>]>,
    }

    impl ThemeCompatibilityFieldEvidence {
        fn from_frozen(field: &FrozenThemeCompatibilityField) -> Self {
            Self {
                opaque_id: Arc::from(field.opaque_id()),
                kind: ThemeCompatibilityFieldKind::from_config(field.kind()),
                surviving_assignment_paths: field.surviving_paths().to_vec().into(),
            }
        }

        pub fn opaque_id(&self) -> &str {
            &self.opaque_id
        }

        pub const fn kind(&self) -> ThemeCompatibilityFieldKind {
            self.kind
        }

        pub fn surviving_assignment_paths(&self) -> impl ExactSizeIterator<Item = &str> {
            self.surviving_assignment_paths
                .iter()
                .map(|path| path.as_ref())
        }

        /// Records one actual physical path accounted by a family terminal consumer.
        pub fn consume_assignment_path(
            &self,
            assignment_path: &str,
            disposition: ThemeCompatibilityConsumptionDisposition,
        ) -> Option<ThemeCompatibilityFieldConsumption> {
            self.surviving_assignment_paths
                .iter()
                .find(|path| path.as_ref() == assignment_path)
                .map(|path| ThemeCompatibilityFieldConsumption {
                    field_opaque_id: Arc::clone(&self.opaque_id),
                    assignment_path: Arc::clone(path),
                    disposition,
                })
        }

        /// Records all surviving physical paths for one conceptual field.
        pub fn consume_all(
            &self,
            disposition: ThemeCompatibilityConsumptionDisposition,
        ) -> impl ExactSizeIterator<Item = ThemeCompatibilityFieldConsumption> + '_ {
            self.surviving_assignment_paths.iter().map(move |path| {
                ThemeCompatibilityFieldConsumption {
                    field_opaque_id: Arc::clone(&self.opaque_id),
                    assignment_path: Arc::clone(path),
                    disposition,
                }
            })
        }
    }

    /// Why a terminal family adapter can retire one compatibility assignment path.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ThemeCompatibilityConsumptionDisposition {
        ReplacedByTypedSurface,
        NotReadByFamily,
    }

    /// One path-level compatibility consumption event tied to a conceptual field identity.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ThemeCompatibilityFieldConsumption {
        field_opaque_id: Arc<str>,
        assignment_path: Arc<str>,
        disposition: ThemeCompatibilityConsumptionDisposition,
    }

    /// Result of reconciling terminal consumption against frozen conceptual fields.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ThemeCompatibilityReconciliation {
        remaining_field_count: usize,
        consumed_field_count: usize,
    }

    impl ThemeCompatibilityReconciliation {
        pub const fn remaining_field_count(self) -> usize {
            self.remaining_field_count
        }

        pub const fn consumed_field_count(self) -> usize {
            self.consumed_field_count
        }
    }

    /// Opaque parse evidence consumed by the render session admission boundary.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ThemeParseEvidence {
        recipe: Option<ThemeCompatibilityRecipe>,
        mermaid_fields: Arc<[ThemeCompatibilityFieldEvidence]>,
        fallback_contributions: Arc<[ThemeCompatibilityContributionEvidence]>,
    }

    impl ThemeParseEvidence {
        pub fn matches_recipe(&self, recipe: Option<&ThemeCompatibilityRecipe>) -> bool {
            self.recipe.as_ref() == recipe
        }

        pub fn mermaid_residual_count(&self) -> usize {
            self.mermaid_fields.len()
        }

        pub fn mermaid_residual_path_survives(&self, dotted_path: &str) -> bool {
            self.mermaid_fields.iter().any(|field| {
                field
                    .surviving_assignment_paths()
                    .any(|path| path == dotted_path)
            })
        }

        pub fn mermaid_theme_field_survives(&self) -> bool {
            self.mermaid_fields
                .iter()
                .any(|field| field.kind() == ThemeCompatibilityFieldKind::Theme)
        }

        pub fn mermaid_dark_mode_field_survives(&self) -> bool {
            self.mermaid_fields
                .iter()
                .any(|field| field.kind() == ThemeCompatibilityFieldKind::DarkMode)
        }

        pub fn mermaid_fields(
            &self,
        ) -> impl ExactSizeIterator<Item = &ThemeCompatibilityFieldEvidence> {
            self.mermaid_fields.iter()
        }

        /// Reconciles path-level terminal events without double-counting conceptual fields.
        ///
        /// Duplicate identical events are idempotent. Conflicting dispositions fail closed, and
        /// a conceptual field is consumed only when every surviving physical path is covered.
        pub fn reconcile_mermaid_consumptions<'a>(
            &self,
            consumptions: impl IntoIterator<Item = &'a ThemeCompatibilityFieldConsumption>,
        ) -> ThemeCompatibilityReconciliation {
            let mut coverage = BTreeMap::<
                (Arc<str>, Arc<str>),
                Option<ThemeCompatibilityConsumptionDisposition>,
            >::new();
            for consumption in consumptions {
                let key = (
                    Arc::clone(&consumption.field_opaque_id),
                    Arc::clone(&consumption.assignment_path),
                );
                match coverage.entry(key) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(Some(consumption.disposition));
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry)
                        if entry.get() != &Some(consumption.disposition) =>
                    {
                        entry.insert(None);
                    }
                    std::collections::btree_map::Entry::Occupied(_) => {}
                }
            }

            let consumed_field_count = self
                .mermaid_fields
                .iter()
                .filter(|field| {
                    field.surviving_assignment_paths.iter().all(|path| {
                        coverage
                            .get(&(Arc::clone(&field.opaque_id), Arc::clone(path)))
                            .is_some_and(|disposition| disposition.is_some())
                    })
                })
                .count();
            ThemeCompatibilityReconciliation {
                remaining_field_count: self
                    .mermaid_fields
                    .len()
                    .saturating_sub(consumed_field_count),
                consumed_field_count,
            }
        }

        pub fn fallback_contribution_count(&self) -> usize {
            self.fallback_contributions.len()
        }

        pub fn fallback_contributions(
            &self,
        ) -> impl ExactSizeIterator<Item = &ThemeCompatibilityContributionEvidence> {
            self.fallback_contributions.iter()
        }
    }

    /// Freezes the compatibility identity and surviving residual evidence of a parsed artifact.
    pub fn theme_parse_evidence(metadata: &ParseMetadata) -> ThemeParseEvidence {
        let mermaid_fields = metadata
            .effective_config
            .mermaid_compatibility_fields()
            .map(|fields| {
                fields
                    .iter()
                    .map(ThemeCompatibilityFieldEvidence::from_frozen)
                    .collect::<Vec<_>>()
                    .into()
            })
            .unwrap_or_else(|| Arc::from(Vec::<ThemeCompatibilityFieldEvidence>::new()));
        ThemeParseEvidence {
            recipe: metadata
                .effective_config
                .theme_parse_binding()
                .cloned()
                .map(ThemeCompatibilityRecipe),
            mermaid_fields,
            fallback_contributions: metadata
                .effective_config
                .overlay_provenance()
                .fallback_contributions()
                .cloned()
                .map(|contribution| {
                    ThemeCompatibilityContributionEvidence::from_frozen(
                        contribution,
                        &metadata.effective_config,
                    )
                })
                .collect::<Vec<_>>()
                .into(),
        }
    }

    /// Reports whether surviving Mermaid input outranks a typed theme default at this path.
    pub fn config_path_overrides_typed_default(config: &MermaidConfig, dotted_path: &str) -> bool {
        config.config_path_overrides_typed_default(dotted_path)
    }

    /// Reports whether site or source configuration explicitly owns this path.
    pub fn explicit_config_owns_path(config: &MermaidConfig, dotted_path: &str) -> bool {
        config.explicit_config_owns_path(dotted_path)
    }
}

/// Maximum nested diagram/include depth accepted by recursive parsers.
pub const MAX_DIAGRAM_NESTING_DEPTH: usize = 256;

/// Returns Mermaid theme names supported by the pinned baseline.
pub fn supported_themes() -> &'static [&'static str] {
    MermaidThemeId::NAMES
}

/// Returns the typed Mermaid theme catalog for the pinned baseline.
pub fn supported_theme_ids() -> &'static [MermaidThemeId] {
    MermaidThemeId::ALL
}

/// Returns supported diagram metadata names for binding and host capability discovery.
pub fn supported_diagrams() -> &'static [&'static str] {
    family::supported_diagram_metadata_ids()
}

/// Returns the complete family capability facts for Mermaid diagram ids in the pinned baseline.
pub fn diagram_family_capabilities() -> &'static [DiagramFamilyCapability] {
    family::diagram_family_capabilities()
}

/// Returns each concrete built-in typed render family exactly once.
pub fn built_in_typed_render_families() -> &'static [BuiltInTypedRenderFamily] {
    family::built_in_typed_render_families()
}

/// Returns header completion facts for Mermaid diagram starters in the pinned baseline.
pub fn diagram_header_facts() -> &'static [DiagramHeaderFact] {
    family::diagram_header_facts()
}

fn build_default_effective_config(
    site_config: &MermaidConfig,
) -> std::result::Result<MermaidConfig, theme::ThemeResolutionError> {
    let mut effective_config = site_config.clone();
    theme::apply_theme_defaults(&mut effective_config)?;
    Ok(effective_config)
}

fn merge_site_config_override(target: &mut MermaidConfig, mut site_config: MermaidConfig) {
    config::mirror_legacy_font_family_into_theme_variables(&mut site_config);
    let explicit_secure_policy = site_config
        .as_value()
        .get("secure")
        .filter(|value| value.is_array())
        .map(config::clone_value_nonrecursive);
    target.deep_merge_explicit(site_config.as_value());

    // Merman adds host-level hardening beyond Mermaid's upstream defaults. An explicit site
    // policy is host authority, so it replaces that added list after the source-compatible array
    // merge instead of making the hardening impossible to opt out of.
    if let Some(secure) = explicit_secure_policy {
        target.set_value_explicit("secure", secure);
    }
}

fn merge_theme_compatibility_config(target: &mut MermaidConfig, mut theme_config: MermaidConfig) {
    config::mirror_legacy_font_family_into_theme_variables(&mut theme_config);
    let before = target.clone();
    target.deep_merge(theme_config.as_value());
    theme_config.retain_theme_compatibility_paths_applied_after(target, &before);
    target.adopt_tracking_theme_compatibility_from(&mut theme_config);
}

fn generated_default_effective_config()
-> std::result::Result<MermaidConfig, theme::ThemeResolutionError> {
    static DEFAULT_EFFECTIVE_CONFIG: std::sync::OnceLock<
        std::result::Result<MermaidConfig, theme::ThemeResolutionError>,
    > = std::sync::OnceLock::new();
    DEFAULT_EFFECTIVE_CONFIG
        .get_or_init(|| build_default_effective_config(&generated::default_site_config()))
        .clone()
}

/// Parser behavior switches for model-producing parse facades.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseOptions {
    /// Return an `error` diagram model from JSON/render facades when diagram parsing fails.
    pub suppress_errors: bool,
}

impl ParseOptions {
    /// Strict parsing (errors are returned).
    pub fn strict() -> Self {
        Self {
            suppress_errors: false,
        }
    }

    /// Lenient model parsing: return an `error` diagram from JSON/render facades on failure.
    pub fn lenient() -> Self {
        Self {
            suppress_errors: true,
        }
    }
}

/// Metadata extracted before semantic diagram parsing.
#[derive(Debug, Clone)]
pub struct ParseMetadata {
    /// Mermaid diagram type id selected by detection or supplied by a known-type parse entrypoint.
    pub diagram_type: String,
    /// Parsed config overrides extracted from front-matter and directives.
    /// This mirrors Mermaid's `mermaidAPI.parse()` return shape.
    pub config: MermaidConfig,
    /// The effective config used for detection/parsing after applying site defaults.
    pub effective_config: MermaidConfig,
    /// Sanitized Mermaid title from front-matter/directives, when present.
    pub title: Option<String>,
}

impl ParseMetadata {
    /// Returns the post-detection compatibility contributions that survived higher-priority config
    /// layers for this parse operation.
    #[cfg(test)]
    pub(crate) fn config_overlay_provenance(&self) -> &ConfigOverlayProvenance {
        self.effective_config.overlay_provenance()
    }

    /// Returns the compiled theme recipe bound to this parsed artifact, when present.
    #[cfg(test)]
    pub(crate) fn theme_parse_binding(&self) -> Option<&ThemeParseBinding> {
        self.effective_config.theme_parse_binding()
    }

    /// Returns explicit Mermaid compatibility fields still owned by the parsed theme.
    #[cfg(test)]
    pub(crate) fn mermaid_compatibility_residual_count(&self) -> usize {
        self.effective_config.mermaid_compatibility_residual_count()
    }
}

/// Headless Mermaid parser engine.
///
/// An engine owns detector/parser registries and a site-level Mermaid configuration. It is cheap
/// to clone when callers need per-request option variants.
#[derive(Debug, Clone)]
enum FallbackPostDetectionConfigOverlay {
    #[cfg(test)]
    Static(std::sync::Arc<PostDetectionConfigOverlay>),
    Provider(std::sync::Arc<dyn PostDetectionConfigOverlayProvider>),
}

#[derive(Debug, Clone)]
pub struct Engine {
    registry: DetectorRegistry,
    diagram_registry: DiagramRegistry,
    render_diagram_registry: RenderDiagramRegistry,
    site_config: MermaidConfig,
    site_config_overrides: MermaidConfig,
    theme_compatibility_config: Option<MermaidConfig>,
    fallback_overlay_explicit_config: MermaidConfig,
    // Keep the overlay graph finite by giving each owner exactly one bounded lane. The host lane
    // is evaluated first; the theme compatibility lane can only fill paths the host did not own.
    post_detection_config_overlay: Option<std::sync::Arc<PostDetectionConfigOverlay>>,
    fallback_post_detection_config_overlay: Option<FallbackPostDetectionConfigOverlay>,
    default_effective_config: std::result::Result<MermaidConfig, theme::ThemeResolutionError>,
    runtime_policy: runtime::RuntimePolicy,
}

impl Default for Engine {
    fn default() -> Self {
        let site_config = generated::default_site_config();
        let default_effective_config = generated_default_effective_config();

        Self {
            registry: DetectorRegistry::pinned_mermaid_baseline(),
            diagram_registry: DiagramRegistry::pinned_mermaid_baseline(),
            render_diagram_registry: RenderDiagramRegistry::pinned_mermaid_baseline(),
            site_config,
            site_config_overrides: MermaidConfig::empty_object(),
            theme_compatibility_config: None,
            fallback_overlay_explicit_config: MermaidConfig::empty_object(),
            post_detection_config_overlay: None,
            fallback_post_detection_config_overlay: None,
            default_effective_config,
            runtime_policy: runtime::RuntimePolicy::deterministic(),
        }
    }
}

impl Engine {
    /// Creates an engine using the pinned Mermaid baseline registries and default site config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an engine backed by all native system adapters.
    pub fn try_native() -> std::result::Result<Self, runtime::RuntimePolicyError> {
        Ok(Self::new().with_runtime_policy(runtime::RuntimePolicy::try_native()?))
    }

    pub(crate) fn default_effective_config(&self) -> Result<MermaidConfig> {
        self.default_effective_config.clone().map_err(Error::from)
    }

    /// Overrides the "today" value used by diagrams that depend on local time (e.g. Gantt).
    ///
    /// This exists primarily to make fixture snapshots deterministic. The default runtime policy
    /// uses the Unix epoch; native callers must explicitly select [`Engine::try_native`].
    pub fn with_fixed_today(mut self, today: Option<time::CivilDate>) -> Self {
        self.runtime_policy = self.runtime_policy.with_fixed_today(today);
        self
    }

    /// Selects a fixed local UTC offset for diagrams with local-time semantics.
    pub fn try_with_fixed_local_offset_minutes(
        mut self,
        offset_minutes: i32,
    ) -> std::result::Result<Self, runtime::RuntimePolicyError> {
        self.runtime_policy = self
            .runtime_policy
            .try_with_fixed_local_offset_minutes(offset_minutes)?;
        Ok(self)
    }

    /// Installs an already-resolved local timezone without consulting ambient process state.
    pub fn with_local_time_zone(mut self, time_zone: time::LocalTimeZone) -> Self {
        self.runtime_policy = self.runtime_policy.with_local_time_zone(time_zone);
        self
    }

    /// Replaces the complete runtime policy used to begin future operations.
    pub fn with_runtime_policy(mut self, policy: runtime::RuntimePolicy) -> Self {
        self.runtime_policy = policy;
        self
    }

    /// Replays a previously frozen runtime context without consulting system state.
    pub fn with_operation_context(self, context: runtime::OperationContext) -> Self {
        self.with_runtime_policy(runtime::RuntimePolicy::from_operation_context(context))
    }

    pub fn runtime_policy(&self) -> &runtime::RuntimePolicy {
        &self.runtime_policy
    }

    pub fn begin_operation(
        &self,
    ) -> std::result::Result<runtime::OperationContext, runtime::RuntimePolicyError> {
        self.runtime_policy.begin_operation()
    }

    /// Returns the fixed local timezone offset configured for this engine.
    pub fn fixed_local_offset_minutes(&self) -> Option<i32> {
        self.runtime_policy.fixed_local_offset_minutes()
    }

    pub fn local_time_zone(&self) -> &time::LocalTimeZone {
        self.runtime_policy.local_time_zone()
    }

    /// Applies site-level Mermaid config defaults.
    pub fn with_site_config(mut self, site_config: MermaidConfig) -> Self {
        if site_config.is_empty_object() && !site_config.has_tracking_theme_compatibility() {
            return self;
        }
        merge_site_config_override(&mut self.site_config_overrides, site_config);
        self.rebuild_site_config();
        self
    }

    /// Installs the lower-priority Mermaid compatibility layer for one compiled theme.
    ///
    /// Existing and future host site config remains authoritative over this layer. The metadata
    /// binding is frozen into parsed artifacts so render sessions cannot consume a parse created
    /// by another theme recipe.
    #[cfg(test)]
    pub(crate) fn with_theme_compatibility(mut self, binding: ThemeParseBinding) -> Self {
        self.theme_compatibility_config = Some(MermaidConfig::from_theme_parse_binding(binding));
        self.rebuild_site_config();
        self
    }

    /// Replaces the high-priority host-owned overlay lane applied after diagram detection.
    #[cfg(test)]
    pub(crate) fn with_post_detection_config_overlay(
        mut self,
        overlay: PostDetectionConfigOverlay,
    ) -> Self {
        self.post_detection_config_overlay =
            (!overlay.is_empty()).then(|| std::sync::Arc::new(overlay));
        self
    }

    /// Replaces the fallback overlay lane without changing the host-owned lane.
    #[cfg(test)]
    pub(crate) fn with_fallback_post_detection_config_overlay(
        mut self,
        overlay: PostDetectionConfigOverlay,
    ) -> Self {
        self.fallback_post_detection_config_overlay = (!overlay.is_empty())
            .then(|| FallbackPostDetectionConfigOverlay::Static(std::sync::Arc::new(overlay)));
        self
    }

    /// Replaces the static fallback lane with a family-lazy provider.
    #[cfg(test)]
    pub(crate) fn with_fallback_post_detection_config_overlay_provider(
        mut self,
        provider: impl PostDetectionConfigOverlayProvider + 'static,
    ) -> Self {
        self.fallback_post_detection_config_overlay = Some(
            FallbackPostDetectionConfigOverlay::Provider(std::sync::Arc::new(provider)),
        );
        self
    }

    /// Replaces the complete site-config environment while preserving custom registries.
    ///
    /// `None` restores the pinned Mermaid defaults. An explicit config is merged onto those
    /// defaults without inheriting values from the engine's previous site config.
    pub fn with_exact_site_config(mut self, site_config: Option<MermaidConfig>) -> Self {
        self.site_config_overrides = MermaidConfig::empty_object();
        if let Some(site_config) = site_config {
            merge_site_config_override(&mut self.site_config_overrides, site_config);
        }
        self.rebuild_site_config();
        self
    }

    fn rebuild_site_config(&mut self) {
        let mut site_config = generated::default_site_config();
        let mut fallback_overlay_explicit_config = MermaidConfig::empty_object();
        if let Some(theme_config) = self.theme_compatibility_config.clone() {
            fallback_overlay_explicit_config.deep_merge(theme_config.as_value());
            merge_theme_compatibility_config(&mut site_config, theme_config);
        }
        fallback_overlay_explicit_config.deep_merge(self.site_config_overrides.as_value());
        merge_site_config_override(&mut site_config, self.site_config_overrides.clone());
        self.default_effective_config = build_default_effective_config(&site_config);
        self.site_config = site_config;
        self.fallback_overlay_explicit_config = fallback_overlay_explicit_config;
    }

    /// Returns the detector registry used for automatic diagram type detection.
    pub fn registry(&self) -> &DetectorRegistry {
        &self.registry
    }

    /// Returns a mutable detector registry for custom diagram detection.
    pub fn registry_mut(&mut self) -> &mut DetectorRegistry {
        &mut self.registry
    }

    /// Returns the semantic JSON parser registry.
    pub fn diagram_registry(&self) -> &DiagramRegistry {
        &self.diagram_registry
    }

    /// Returns a mutable semantic JSON parser registry for custom diagram adapters.
    pub fn diagram_registry_mut(&mut self) -> &mut DiagramRegistry {
        &mut self.diagram_registry
    }

    /// Returns the typed render-model parser registry.
    pub fn render_diagram_registry(&self) -> &RenderDiagramRegistry {
        &self.render_diagram_registry
    }

    /// Returns a mutable typed render-model parser registry.
    pub fn render_diagram_registry_mut(&mut self) -> &mut RenderDiagramRegistry {
        &mut self.render_diagram_registry
    }

    /// Synchronous variant of [`Engine::parse_metadata`].
    ///
    /// This is useful for UI render pipelines that are synchronous (e.g. immediate-mode UI),
    /// where introducing an async executor would be awkward. The parsing work is CPU-bound and
    /// does not perform I/O.
    pub fn parse_metadata_sync(&self, text: &str) -> Result<ParseMetadata> {
        parse_pipeline::ParsePipeline::detect(self, text, ParseOptions::strict()).metadata()
    }

    /// Parses metadata for an already-known diagram type (skips type detection).
    ///
    /// This is intended for integrations that already know the diagram type, e.g. Markdown fences
    /// like ````mermaid` / ` ```flowchart` / ` ```sequenceDiagram`.
    ///
    /// ## Example (Markdown fence)
    ///
    /// ```no_run
    /// use merman_core::Engine;
    ///
    /// let engine = Engine::new();
    ///
    /// // Your markdown parser provides the fence info string (e.g. "flowchart", "sequenceDiagram").
    /// let fence = "sequenceDiagram";
    /// let diagram = r#"sequenceDiagram
    ///   Alice->>Bob: Hello
    /// "#;
    ///
    /// // Map fence info strings to merman's internal diagram ids.
    /// let diagram_type = match fence {
    ///     "sequenceDiagram" => "sequence",
    ///     "flowchart" | "graph" => "flowchart-v2",
    ///     "stateDiagram" | "stateDiagram-v2" => "stateDiagram",
    ///     other => other,
    /// };
    ///
    /// let meta = engine
    ///     .parse_metadata_with_type_sync(diagram_type, diagram)?;
    /// # Ok::<(), merman_core::Error>(())
    /// ```
    pub fn parse_metadata_with_type_sync(
        &self,
        diagram_type: &str,
        text: &str,
    ) -> Result<ParseMetadata> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, ParseOptions::strict())
            .metadata()
    }

    /// Parses editor-facing semantic facts when a family has a parser-backed implementation.
    ///
    /// Returned spans are byte offsets in the `text` supplied to this method.
    pub fn parse_editor_semantic_facts_with_type_sync(
        &self,
        diagram_type: &str,
        text: &str,
    ) -> Result<Option<EditorSemanticFacts>> {
        let Some(snapshot) = self.parse_diagram_snapshot_with_type_sync(diagram_type, text)? else {
            return Ok(None);
        };
        let (_, outcome, editor_facts) = snapshot.into_parts();
        match editor_facts {
            ParsedEditorFacts::Available(facts) => Ok(Some(facts)),
            ParsedEditorFacts::Unavailable => match outcome {
                DiagramParseOutcome::Failed(error @ Error::UnsupportedDiagram { .. })
                    if family::is_builtin_diagram_type(diagram_type) =>
                {
                    Err(error)
                }
                _ => Ok(None),
            },
        }
    }

    /// Async facade for [`Engine::parse_metadata_sync`].
    ///
    /// The work is CPU-bound and executes synchronously; this method exists for callers that
    /// prefer an async-shaped API.
    pub async fn parse_metadata(&self, text: &str) -> Result<ParseMetadata> {
        self.parse_metadata_sync(text)
    }

    /// Async facade for [`Engine::parse_metadata_with_type_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    pub async fn parse_metadata_with_type(
        &self,
        diagram_type: &str,
        text: &str,
    ) -> Result<ParseMetadata> {
        self.parse_metadata_with_type_sync(diagram_type, text)
    }

    /// Synchronous variant of [`Engine::parse_diagram`].
    ///
    /// Note: callers that want “always returns a diagram” behavior can set
    /// [`ParseOptions::suppress_errors`] to `true` to get an `error` diagram on parse failures.
    pub fn parse_diagram_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        parse_pipeline::ParsePipeline::detect(self, text, options)
            .parse_json(parse_pipeline::ParseTiming::Json)
    }

    /// Captures semantic JSON or its original error, parser-owned typed warning facts, and
    /// parser-backed editor facts in one operation.
    ///
    /// This is intended for editor integrations that need both diagnostics/facts and the
    /// Mermaid-compatible model. Once preprocessing and detection succeed, family parse errors and
    /// panics are retained inside the snapshot alongside metadata and recovery facts. Successful
    /// snapshots expose warnings through the `warning_facts` field of
    /// [`DiagramParseOutcome::Parsed`]; consumers should not decode the compatibility model's
    /// `warningFacts` field. Consumers must project a retained failure state directly rather than
    /// parsing the source again.
    /// Error suppression is deliberately absent from this API; suppression remains limited to
    /// model-producing JSON and render facades.
    pub fn parse_diagram_snapshot_sync(&self, text: &str) -> Result<Option<DiagramParseSnapshot>> {
        let control = OperationControl::new();
        self.parse_diagram_snapshot_controlled_sync(text, &control)
            .map_err(Error::from)?
    }

    /// Captures an editor-facing parse operation with cooperative cancellation.
    ///
    /// Cancellation is returned through the outer [`OperationControlResult`] and is never converted
    /// into a Mermaid parse error, failed snapshot, or recovery diagnostic.
    pub fn parse_diagram_snapshot_controlled_sync(
        &self,
        text: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Option<DiagramParseSnapshot>>> {
        parse_pipeline::ParsePipeline::detect(self, text, ParseOptions::strict())
            .parse_editor_snapshot_controlled(parse_pipeline::ParseTiming::Json, control)
    }

    /// Captures an editor parse operation while retaining preprocessing source-configuration
    /// evidence on non-cancellation failures and on panics after preprocessing completes.
    /// Cooperative cancellation remains the outer error channel and never yields a partial
    /// capture.
    pub fn capture_diagram_snapshot_controlled_sync(
        &self,
        text: &str,
        control: &OperationControl,
    ) -> OperationControlResult<DiagramSnapshotCapture> {
        parse_pipeline::ParsePipeline::detect(self, text, ParseOptions::strict())
            .capture_editor_snapshot_controlled(parse_pipeline::ParseTiming::Json, control)
    }

    /// Captures one editor-facing parse operation when the diagram type is already known.
    ///
    /// This has the same closed snapshot contract, including parser-owned typed warning facts, as
    /// [`Engine::parse_diagram_snapshot_sync`], but skips automatic detection. Family parse
    /// failures and panics remain inside the returned snapshot.
    pub fn parse_diagram_snapshot_with_type_sync(
        &self,
        diagram_type: &str,
        text: &str,
    ) -> Result<Option<DiagramParseSnapshot>> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, ParseOptions::strict())
            .parse_editor_snapshot(parse_pipeline::ParseTiming::Json)
    }

    /// Async facade for [`Engine::parse_diagram_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    pub async fn parse_diagram(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_sync(text, options)
    }

    /// Parses a diagram into a typed semantic model optimized for headless layout + SVG rendering.
    ///
    /// Unlike [`Engine::parse_diagram_sync`], this avoids constructing large
    /// `serde_json::Value` object trees for high-impact typed-first diagrams and instead returns
    /// typed semantic structs that the renderer can consume directly.
    ///
    /// Callers that need the semantic JSON model should continue using
    /// [`Engine::parse_diagram_sync`].
    pub fn parse_diagram_for_render_model_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        parse_pipeline::ParsePipeline::detect(self, text, options).parse_render_model()
    }

    /// Controlled variant of [`Self::parse_diagram_for_render_model_sync`].
    ///
    /// The caller owns the cloneable operation control and may cancel it from another thread or
    /// task. Cancellation is returned through the outer result and is never folded into a parse
    /// error or a partial render model.
    pub fn parse_diagram_for_render_model_controlled_sync(
        &self,
        text: &str,
        options: ParseOptions,
        operation: &OperationControl,
    ) -> OperationControlResult<Result<Option<ParsedDiagramRender>>> {
        parse_pipeline::ParsePipeline::detect(self, text, options)
            .parse_render_model_controlled(operation)
    }

    /// Parses a typed render model inside a caller-owned runtime context and operation.
    ///
    /// Higher-level render facades use this composition seam to keep runtime values and
    /// cancellation state identical across parsing and target-specific layout/output stages.
    /// Unlike [`Self::parse_diagram_for_render_model_controlled_sync`], this method does not
    /// begin a replacement runtime operation.
    #[doc(hidden)]
    pub fn parse_diagram_for_render_model_controlled_in_context_sync(
        &self,
        text: &str,
        options: ParseOptions,
        operation: &OperationControl,
        operation_context: &runtime::OperationContext,
    ) -> OperationControlResult<Result<Option<ParsedDiagramRender>>> {
        parse_pipeline::ParsePipeline::detect(self, text, options)
            .parse_render_model_controlled_in_context(operation, operation_context)
    }

    /// Async facade for [`Engine::parse_diagram_for_render_model_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    pub async fn parse_diagram_for_render_model(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        self.parse_diagram_for_render_model_sync(text, options)
    }

    /// Parses a diagram into a typed semantic render model when the diagram type is already known
    /// (skips type detection).
    ///
    /// This is the preferred entrypoint for Markdown renderers and editors that already know the
    /// diagram type from the code fence info string. It avoids the detection pass and can reduce a
    /// small fixed overhead in tight render loops.
    pub fn parse_diagram_for_render_model_with_type_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, options)
            .parse_render_model()
    }

    /// Controlled known-type variant of [`Self::parse_diagram_for_render_model_with_type_sync`].
    pub fn parse_diagram_for_render_model_with_type_controlled_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
        operation: &OperationControl,
    ) -> OperationControlResult<Result<Option<ParsedDiagramRender>>> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, options)
            .parse_render_model_controlled(operation)
    }

    /// Known-type composition seam for a caller-owned runtime context and operation.
    #[doc(hidden)]
    pub fn parse_diagram_for_render_model_with_type_controlled_in_context_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
        operation: &OperationControl,
        operation_context: &runtime::OperationContext,
    ) -> OperationControlResult<Result<Option<ParsedDiagramRender>>> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, options)
            .parse_render_model_controlled_in_context(operation, operation_context)
    }

    /// Async facade for [`Engine::parse_diagram_for_render_model_with_type_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    pub async fn parse_diagram_for_render_model_with_type(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        self.parse_diagram_for_render_model_with_type_sync(diagram_type, text, options)
    }

    /// Parses a diagram when the diagram type is already known (skips type detection).
    ///
    /// This is the preferred entrypoint for Markdown renderers and editors that already know the
    /// diagram type from the code fence info string. It avoids the detection pass and can reduce a
    /// small fixed overhead in tight render loops.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// use merman_core::{Engine, ParseOptions};
    ///
    /// let engine = Engine::new();
    /// let input = "flowchart TD; A-->B;";
    ///
    /// let parsed = engine
    ///     .parse_diagram_with_type_sync("flowchart-v2", input, ParseOptions::strict())?
    ///     .expect("diagram detected");
    ///
    /// assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
    /// # Ok::<(), merman_core::Error>(())
    /// ```
    pub fn parse_diagram_with_type_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, options)
            .parse_json(parse_pipeline::ParseTiming::None)
    }

    /// Async facade for [`Engine::parse_diagram_with_type_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    pub async fn parse_diagram_with_type(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_with_type_sync(diagram_type, text, options)
    }
}

#[cfg(test)]
mod tests;
