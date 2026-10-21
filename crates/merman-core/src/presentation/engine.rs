use super::PresentationConfig;
#[cfg(test)]
use crate::config::presentation::ThemeParseBinding;
use crate::{
    DetectorRegistry, Error, MermaidConfig, MermaidThemeId, OperationControl,
    OperationControlResult, ParsedDiagramRender, RenderDiagramRegistry, Result, generated, runtime,
    theme, time,
};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::{
    DiagramParseOutcome, DiagramParseSnapshot, DiagramSnapshotCapture, EditorSemanticFacts,
    ParsedEditorFacts, family,
};
#[cfg(feature = "compat-json")]
use crate::{DiagramRegistry, ParsedDiagram};

#[path = "parse_pipeline.rs"]
pub(crate) mod parse_pipeline;

/// Workspace-internal compiled theme binding and Mermaid ownership evidence.
///
/// This module is intentionally outside Merman's supported API. Its opaque plans and evidence
/// keep normalized recipe identity and parse binding private to `merman-core`.
#[doc(hidden)]
pub mod __private {
    use std::collections::BTreeMap;
    use std::fmt;
    use std::sync::{Arc, OnceLock};

    use super::{
        MermaidEngine, PresentationConfig, PresentationMetadata, SiteConfigInitialization,
    };
    use crate::MermaidConfig;
    use crate::config::presentation::{
        FrozenThemeCompatibilityField, ThemeCompatibilityFieldKind as ConfigFieldKind,
        ThemeParseBinding, ThemeParseBindingError,
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

    /// Opaque parse/install plan owned by one compiled diagram theme.
    #[derive(Clone)]
    pub struct ThemeCompatibilityPlan {
        recipe: ThemeCompatibilityRecipe,
        initialization: Arc<OnceLock<SiteConfigInitialization>>,
        compatibility_config: PresentationConfig,
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
        /// Builds a bounded parse recipe from compiled Mermaid compatibility input.
        pub fn try_new(
            recipe_identity: [u8; 32],
            compatibility_config: MermaidConfig,
        ) -> Result<Self, ThemeCompatibilityPlanError> {
            let recipe = ThemeCompatibilityRecipe(ThemeParseBinding::try_new(
                recipe_identity,
                compatibility_config,
            )?);
            let compatibility_config =
                PresentationConfig::from_theme_parse_binding(recipe.0.clone());
            let initialization = Arc::new(OnceLock::new());
            Ok(Self {
                recipe,
                initialization,
                compatibility_config,
            })
        }

        pub fn recipe(&self) -> &ThemeCompatibilityRecipe {
            &self.recipe
        }
    }

    pub use crate::config::presentation::{
        ErPaintInput, ErPaintInputs, GitGraphPaintInput, GitGraphPaintInputs,
    };

    /// Returns frozen raw Mermaid paint authority for the selected GitGraph family.
    pub fn gitgraph_paint_inputs(config: &PresentationConfig) -> Option<GitGraphPaintInputs> {
        config.gitgraph_paint_inputs()
    }

    /// Returns frozen raw Mermaid paint authority for the selected ER family.
    pub fn er_paint_inputs(config: &PresentationConfig) -> Option<ErPaintInputs> {
        config.er_paint_inputs()
    }

    /// Installs one compiled theme's complete parse compatibility plan on an engine.
    pub fn install_theme_compatibility(
        mut engine: MermaidEngine,
        plan: &ThemeCompatibilityPlan,
    ) -> MermaidEngine {
        engine.theme_compatibility_config = Some(plan.compatibility_config.clone());
        if engine.site_config_overrides.is_pristine_empty_object() {
            // Prepare after MermaidEngine has initialized the pinned defaults. Keep errors in the
            // cached Result so operation admission and parse configuration retain their order.
            let initialized = plan
                .initialization
                .get_or_init(|| {
                    SiteConfigInitialization::new(
                        Some(plan.compatibility_config.clone()),
                        &engine.site_config_overrides,
                    )
                })
                .for_pristine_install(&engine.site_config_overrides);
            engine.site_config = initialized.site_config;
            engine.initialization_inputs = initialized.initialization_inputs;
            engine.default_effective_config = initialized.default_effective_config;
        } else {
            engine.rebuild_site_config();
        }
        engine
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
    }

    /// Compares the complete frozen parse recipe without constructing an evidence snapshot.
    pub fn theme_parse_matches_recipe(
        metadata: &PresentationMetadata,
        recipe: Option<&ThemeCompatibilityRecipe>,
    ) -> bool {
        metadata.effective_config.theme_parse_binding() == recipe.map(|recipe| &recipe.0)
    }

    /// Counts frozen conceptual Mermaid fields without copying their paths or identities.
    pub fn theme_parse_mermaid_residual_count(metadata: &PresentationMetadata) -> usize {
        metadata
            .effective_config
            .mermaid_compatibility_residual_count()
    }

    /// Freezes the compatibility identity and surviving residual evidence of a parsed artifact.
    pub fn theme_parse_evidence(metadata: &PresentationMetadata) -> ThemeParseEvidence {
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
        }
    }

    /// Reports whether surviving Mermaid input outranks a typed theme default at this path.
    pub fn config_path_overrides_typed_default(
        config: &PresentationConfig,
        dotted_path: &str,
    ) -> bool {
        config.config_path_overrides_typed_default(dotted_path)
    }

    /// Reports whether authored inputs or a selected external palette own this paint value.
    pub fn config_paint_overrides_typed_default(
        config: &PresentationConfig,
        dotted_path: &str,
    ) -> bool {
        config.config_paint_overrides_typed_default(dotted_path)
    }

    /// Reports whether site or source configuration explicitly owns this path.
    pub fn explicit_config_owns_path(config: &PresentationConfig, dotted_path: &str) -> bool {
        config.explicit_config_owns_path(dotted_path)
    }

    /// Resolves Mermaid's bounded `THEME_COLOR_LIMIT` loop count using the same coercion and
    /// ceiling as theme materialization. A missing value is represented by `None`; callers that
    /// model an unmaterialized config may apply the Mermaid default before calling this helper.
    pub fn theme_color_iterations(
        raw: Option<&serde_json::Value>,
    ) -> Result<usize, crate::ThemeEvaluationLimitExceeded> {
        crate::theme::theme_color_iterations(raw)
    }
}

#[derive(Clone)]
struct SiteConfigInitialization {
    site_config: PresentationConfig,
    initialization_inputs: PresentationConfig,
    default_effective_config: std::result::Result<PresentationConfig, theme::ThemeResolutionError>,
}

impl SiteConfigInitialization {
    fn for_pristine_install(&self, overrides: &PresentationConfig) -> Self {
        let mut initialized = self.clone();
        initialized
            .initialization_inputs
            .refresh_initialization_mutations_after(overrides);
        initialized
            .site_config
            .refresh_initialization_mutations_after(&generated::default_presentation_config());
        if let Ok(effective) = &mut initialized.default_effective_config {
            effective.inherit_initialization_mutation_epochs(&initialized.site_config);
        }
        initialized
    }

    fn new(theme_config: Option<PresentationConfig>, overrides: &PresentationConfig) -> Self {
        let mut site_config = generated::default_presentation_config();
        let mut initialization_inputs = PresentationConfig::empty_object();
        if let Some(theme_config) = theme_config {
            initialization_inputs.deep_merge(theme_config.as_value());
            merge_theme_compatibility_config(&mut site_config, theme_config);
        }
        initialization_inputs.deep_merge(overrides.as_value());
        merge_site_config_override(&mut site_config, overrides.clone());
        let default_effective_config = build_default_effective_config(&site_config);
        Self {
            site_config,
            initialization_inputs,
            default_effective_config,
        }
    }
}

fn build_default_effective_config(
    site_config: &PresentationConfig,
) -> std::result::Result<PresentationConfig, theme::ThemeResolutionError> {
    let mut effective_config = site_config.clone();
    theme::apply_theme_defaults(&mut effective_config)?;
    Ok(effective_config)
}

fn merge_site_config_override(
    target: &mut PresentationConfig,
    mut site_config: PresentationConfig,
) {
    site_config.mirror_legacy_font_family();
    // initialize() normalizes an unknown global theme before it becomes a user layer.
    // Scoped themes instead fall through during diagram appearance resolution.
    if site_config.as_value().get("theme").is_some_and(|value| {
        !value.is_null()
            && !value
                .as_str()
                .is_some_and(|name| name == "null" || MermaidThemeId::NAMES.contains(&name))
    }) && let Some(theme) = generated::upstream_default_config().as_value().get("theme")
    {
        site_config.set_value("theme", theme.clone());
    }
    let explicit_secure_policy = site_config
        .as_value()
        .get("secure")
        .filter(|value| value.is_array())
        .map(crate::json_value::clone_value_nonrecursive);
    target.deep_merge_explicit(site_config.as_value());

    // Merman adds host-level hardening beyond Mermaid's upstream defaults. An explicit site
    // policy is host authority, so it replaces that added list after the source-compatible array
    // merge instead of making the hardening impossible to opt out of.
    if let Some(secure) = explicit_secure_policy {
        target.set_value_explicit("secure", secure);
    }
}

fn merge_theme_compatibility_config(
    target: &mut PresentationConfig,
    mut theme_config: PresentationConfig,
) {
    theme_config.mirror_legacy_font_family();
    let before = target.clone();
    target.deep_merge(theme_config.as_value());
    theme_config.retain_theme_compatibility_paths_applied_after(target, &before);
    target.adopt_tracking_theme_compatibility_from(&mut theme_config);
}

fn generated_default_effective_config()
-> std::result::Result<PresentationConfig, theme::ThemeResolutionError> {
    static DEFAULT_EFFECTIVE_CONFIG: std::sync::OnceLock<
        std::result::Result<PresentationConfig, theme::ThemeResolutionError>,
    > = std::sync::OnceLock::new();
    DEFAULT_EFFECTIVE_CONFIG
        .get_or_init(|| build_default_effective_config(&generated::default_presentation_config()))
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
pub struct PresentationMetadata {
    /// Mermaid diagram type id selected by detection or supplied by a known-type parse entrypoint.
    pub diagram_type: String,
    /// Parsed config overrides extracted from front-matter and directives.
    /// This mirrors Mermaid's `mermaidAPI.parse()` return shape.
    pub config: MermaidConfig,
    /// The effective config used for detection/parsing after applying site defaults.
    pub effective_config: PresentationConfig,
    /// Sanitized Mermaid title from front-matter/directives, when present.
    pub title: Option<String>,
}

impl PresentationMetadata {
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
pub struct MermaidEngine {
    registry: DetectorRegistry,
    #[cfg(feature = "compat-json")]
    diagram_registry: DiagramRegistry,
    render_diagram_registry: RenderDiagramRegistry,
    site_config: PresentationConfig,
    site_config_overrides: PresentationConfig,
    theme_compatibility_config: Option<PresentationConfig>,
    initialization_inputs: PresentationConfig,
    default_effective_config: std::result::Result<PresentationConfig, theme::ThemeResolutionError>,
    runtime_policy: runtime::RuntimePolicy,
}

impl Default for MermaidEngine {
    fn default() -> Self {
        let site_config = generated::default_presentation_config();
        let default_effective_config = generated_default_effective_config();

        Self {
            registry: DetectorRegistry::pinned_mermaid_baseline(),
            #[cfg(feature = "compat-json")]
            diagram_registry: DiagramRegistry::pinned_mermaid_baseline(),
            render_diagram_registry: RenderDiagramRegistry::pinned_mermaid_baseline(),
            site_config,
            site_config_overrides: PresentationConfig::empty_object(),
            theme_compatibility_config: None,
            initialization_inputs: PresentationConfig::empty_object(),
            default_effective_config,
            runtime_policy: runtime::RuntimePolicy::deterministic(),
        }
    }
}

impl MermaidEngine {
    /// Creates an engine using the pinned Mermaid baseline registries and default site config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an engine backed by all native system adapters.
    pub fn try_native() -> std::result::Result<Self, runtime::RuntimePolicyError> {
        Ok(Self::new().with_runtime_policy(runtime::RuntimePolicy::try_native()?))
    }

    pub(crate) fn default_effective_config(&self) -> Result<PresentationConfig> {
        self.default_effective_config.clone().map_err(Error::from)
    }

    /// Overrides the "today" value used by diagrams that depend on local time (e.g. Gantt).
    ///
    /// This exists primarily to make fixture snapshots deterministic. The default runtime policy
    /// uses the Unix epoch; native callers must explicitly select [`MermaidEngine::try_native`].
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
    pub fn with_site_config(mut self, site_config: PresentationConfig) -> Self {
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
        self.theme_compatibility_config =
            Some(PresentationConfig::from_theme_parse_binding(binding));
        self.rebuild_site_config();
        self
    }

    /// Replaces the complete site-config environment while preserving custom registries.
    ///
    /// `None` restores the pinned Mermaid defaults. An explicit config is merged onto those
    /// defaults without inheriting values from the engine's previous site config.
    pub fn with_exact_site_config(mut self, site_config: Option<PresentationConfig>) -> Self {
        self.site_config_overrides = PresentationConfig::empty_object();
        if let Some(site_config) = site_config {
            merge_site_config_override(&mut self.site_config_overrides, site_config);
        }
        self.rebuild_site_config();
        self
    }

    fn rebuild_site_config(&mut self) {
        let initialized = SiteConfigInitialization::new(
            self.theme_compatibility_config.clone(),
            &self.site_config_overrides,
        );
        self.site_config = initialized.site_config;
        self.initialization_inputs = initialized.initialization_inputs;
        self.default_effective_config = initialized.default_effective_config;
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
    #[cfg(feature = "compat-json")]
    pub fn diagram_registry(&self) -> &DiagramRegistry {
        &self.diagram_registry
    }

    /// Returns a mutable semantic JSON parser registry for custom diagram adapters.
    #[cfg(feature = "compat-json")]
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

    /// Synchronous variant of [`MermaidEngine::parse_metadata`].
    ///
    /// This is useful for UI render pipelines that are synchronous (e.g. immediate-mode UI),
    /// where introducing an async executor would be awkward. The parsing work is CPU-bound and
    /// does not perform I/O.
    pub fn parse_metadata_sync(&self, text: &str) -> Result<PresentationMetadata> {
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
    /// use merman_core::MermaidEngine;
    ///
    /// let engine = MermaidEngine::new();
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
    ) -> Result<PresentationMetadata> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, ParseOptions::strict())
            .metadata()
    }

    /// Parses editor-facing semantic facts when a family has a parser-backed implementation.
    ///
    /// Returned spans are byte offsets in the `text` supplied to this method.
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
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

    /// Async facade for [`MermaidEngine::parse_metadata_sync`].
    ///
    /// The work is CPU-bound and executes synchronously; this method exists for callers that
    /// prefer an async-shaped API.
    pub async fn parse_metadata(&self, text: &str) -> Result<PresentationMetadata> {
        self.parse_metadata_sync(text)
    }

    /// Async facade for [`MermaidEngine::parse_metadata_with_type_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    pub async fn parse_metadata_with_type(
        &self,
        diagram_type: &str,
        text: &str,
    ) -> Result<PresentationMetadata> {
        self.parse_metadata_with_type_sync(diagram_type, text)
    }

    /// Synchronous variant of [`MermaidEngine::parse_diagram`].
    ///
    /// Note: callers that want “always returns a diagram” behavior can set
    /// [`ParseOptions::suppress_errors`] to `true` to get an `error` diagram on parse failures.
    #[cfg(feature = "compat-json")]
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
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    pub fn parse_diagram_snapshot_sync(&self, text: &str) -> Result<Option<DiagramParseSnapshot>> {
        let control = OperationControl::new();
        self.parse_diagram_snapshot_controlled_sync(text, &control)
            .map_err(Error::from)?
    }

    /// Captures an editor-facing parse operation with cooperative cancellation.
    ///
    /// Cancellation is returned through the outer [`OperationControlResult`] and is never converted
    /// into a Mermaid parse error, failed snapshot, or recovery diagnostic.
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
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
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
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
    /// [`MermaidEngine::parse_diagram_snapshot_sync`], but skips automatic detection. Family parse
    /// failures and panics remain inside the returned snapshot.
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    pub fn parse_diagram_snapshot_with_type_sync(
        &self,
        diagram_type: &str,
        text: &str,
    ) -> Result<Option<DiagramParseSnapshot>> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, ParseOptions::strict())
            .parse_editor_snapshot(parse_pipeline::ParseTiming::Json)
    }

    /// Async facade for [`MermaidEngine::parse_diagram_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    #[cfg(feature = "compat-json")]
    pub async fn parse_diagram(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_sync(text, options)
    }

    /// Parses a diagram into a typed semantic model optimized for headless layout + SVG rendering.
    ///
    /// Returns family-owned typed structs that the renderer can consume directly, avoiding a
    /// compatibility JSON projection. Explicit custom render adapters retain their named payload.
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

    /// Async facade for [`MermaidEngine::parse_diagram_for_render_model_sync`].
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

    /// Async facade for [`MermaidEngine::parse_diagram_for_render_model_with_type_sync`].
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
    /// use merman_core::{MermaidEngine, ParseOptions};
    ///
    /// let engine = MermaidEngine::new();
    /// let input = "flowchart TD; A-->B;";
    ///
    /// let parsed = engine
    ///     .parse_diagram_with_type_sync("flowchart-v2", input, ParseOptions::strict())?
    ///     .expect("diagram detected");
    ///
    /// assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
    /// # Ok::<(), merman_core::Error>(())
    /// ```
    #[cfg(feature = "compat-json")]
    pub fn parse_diagram_with_type_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        parse_pipeline::ParsePipeline::known_type(self, diagram_type, text, options)
            .parse_json(parse_pipeline::ParseTiming::None)
    }

    /// Async facade for [`MermaidEngine::parse_diagram_with_type_sync`].
    ///
    /// The work is CPU-bound and executes synchronously.
    #[cfg(feature = "compat-json")]
    pub async fn parse_diagram_with_type(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_with_type_sync(diagram_type, text, options)
    }
}
