use crate::config::presentation::SourcePresentationConfig;
use crate::{
    DetectorRegistry, Error, MermaidConfig, MermaidThemeId, OperationControl,
    OperationControlResult, ParsedDiagramRender, RenderDiagramRegistry, Result, generated, theme,
    time,
};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::{
    DiagramParseOutcome, DiagramParseSnapshot, DiagramSnapshotCapture, EditorSemanticFacts,
    ParsedEditorFacts, family,
};
#[cfg(feature = "compat-json")]
use crate::{DiagramRegistry, ParsedDiagram};
use merman_model::presentation::PresentationMetadata;
#[cfg(test)]
use merman_model::presentation::ThemeCompatibilityRecipe;

#[path = "parse_pipeline.rs"]
pub(crate) mod parse_pipeline;

#[path = "theme_plan.rs"]
mod theme_plan;
pub use theme_plan::{ThemeCompatibilityPlan, ThemeCompatibilityPlanError};

#[derive(Clone)]
struct SiteConfigInitialization {
    site_config: SourcePresentationConfig,
    initialization_inputs: SourcePresentationConfig,
    default_effective_config:
        std::result::Result<SourcePresentationConfig, theme::ThemeResolutionError>,
}

impl SiteConfigInitialization {
    fn for_pristine_install(&self, overrides: &SourcePresentationConfig) -> Self {
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

    fn new(
        theme_config: Option<SourcePresentationConfig>,
        overrides: &SourcePresentationConfig,
    ) -> Self {
        let mut site_config = generated::default_presentation_config();
        let mut initialization_inputs = SourcePresentationConfig::empty_object();
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
    site_config: &SourcePresentationConfig,
) -> std::result::Result<SourcePresentationConfig, theme::ThemeResolutionError> {
    let mut effective_config = site_config.clone();
    theme::apply_theme_defaults(&mut effective_config)?;
    Ok(effective_config)
}

fn merge_site_config_override(
    target: &mut SourcePresentationConfig,
    mut site_config: SourcePresentationConfig,
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
    target: &mut SourcePresentationConfig,
    mut theme_config: SourcePresentationConfig,
) {
    theme_config.mirror_legacy_font_family();
    let before = target.clone();
    target.deep_merge(theme_config.as_value());
    theme_config.retain_theme_compatibility_paths_applied_after(target, &before);
    target.adopt_tracking_theme_compatibility_from(&mut theme_config);
}

fn generated_default_effective_config()
-> std::result::Result<SourcePresentationConfig, theme::ThemeResolutionError> {
    static DEFAULT_EFFECTIVE_CONFIG: std::sync::OnceLock<
        std::result::Result<SourcePresentationConfig, theme::ThemeResolutionError>,
    > = std::sync::OnceLock::new();
    DEFAULT_EFFECTIVE_CONFIG
        .get_or_init(|| build_default_effective_config(&generated::default_presentation_config()))
        .clone()
}

pub use crate::ParseOptions;

/// Source-phase metadata used by grammar, DB construction, and custom source callbacks.
///
/// This value retains source construction state and must never enter a shared model artifact.
/// Completed metadata is produced by the controlled freeze boundary after source postprocessing.
#[derive(Debug, Clone)]
pub struct SourcePresentationMetadata {
    pub diagram_type: String,
    pub config: MermaidConfig,
    pub effective_config: SourcePresentationConfig,
    pub title: Option<String>,
}

impl SourcePresentationMetadata {
    /// Borrows only immutable values consumed by a family compatibility projector.
    #[cfg(feature = "compat-json")]
    pub(crate) fn compat_json_metadata(
        &self,
    ) -> merman_model::presentation::compat::CompatJsonMetadata<'_> {
        merman_model::presentation::compat::CompatJsonMetadata {
            diagram_type: &self.diagram_type,
            config: self.effective_config.as_config(),
            authored_config: &self.config,
            title: self.title.as_deref(),
            diagram_id: None,
        }
    }

    pub(crate) fn freeze_controlled(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<PresentationMetadata>> {
        use crate::config::presentation::{
            presentation_config_error, source_config_construction_policy,
        };
        let effective_config = match self
            .effective_config
            .into_presentation_config_controlled(&source_config_construction_policy(), control)?
        {
            Ok(config) => config,
            Err(error) => return Ok(Err(presentation_config_error(error))),
        };
        control.checkpoint()?;
        Ok(Ok(PresentationMetadata {
            diagram_type: self.diagram_type,
            config: self.config,
            effective_config,
            title: self.title,
        }))
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
    site_config: SourcePresentationConfig,
    site_config_overrides: SourcePresentationConfig,
    theme_compatibility_config: Option<SourcePresentationConfig>,
    initialization_inputs: SourcePresentationConfig,
    default_effective_config:
        std::result::Result<SourcePresentationConfig, theme::ThemeResolutionError>,
    runtime_policy: merman_model::runtime::RuntimePolicy,
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
            site_config_overrides: SourcePresentationConfig::empty_object(),
            theme_compatibility_config: None,
            initialization_inputs: SourcePresentationConfig::empty_object(),
            default_effective_config,
            runtime_policy: merman_model::runtime::RuntimePolicy::deterministic(),
        }
    }
}

impl MermaidEngine {
    /// Creates an engine using the pinned Mermaid baseline registries and default site config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an engine backed by all native system adapters.
    pub fn try_native() -> std::result::Result<Self, merman_model::runtime::RuntimePolicyError> {
        Ok(Self::new().with_runtime_policy(merman_model::runtime::RuntimePolicy::try_native()?))
    }

    pub(crate) fn default_effective_config(&self) -> Result<SourcePresentationConfig> {
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
    ) -> std::result::Result<Self, merman_model::runtime::RuntimePolicyError> {
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
    pub fn with_runtime_policy(mut self, policy: merman_model::runtime::RuntimePolicy) -> Self {
        self.runtime_policy = policy;
        self
    }

    /// Replays a previously frozen runtime context without consulting system state.
    pub fn with_operation_context(self, context: merman_model::runtime::OperationContext) -> Self {
        self.with_runtime_policy(
            merman_model::runtime::RuntimePolicy::from_operation_context(context),
        )
    }

    pub fn runtime_policy(&self) -> &merman_model::runtime::RuntimePolicy {
        &self.runtime_policy
    }

    pub fn begin_operation(
        &self,
    ) -> std::result::Result<
        merman_model::runtime::OperationContext,
        merman_model::runtime::RuntimePolicyError,
    > {
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
    pub fn with_site_config(mut self, site_config: SourcePresentationConfig) -> Self {
        if site_config.is_empty_object() && !site_config.has_tracking_theme_compatibility() {
            return self;
        }
        merge_site_config_override(&mut self.site_config_overrides, site_config);
        self.rebuild_site_config();
        self
    }

    /// Installs a normalized theme recipe through the source owner's initialization cache.
    /// Host site configuration retains precedence over the recipe's compatibility inputs.
    pub fn with_theme_plan(self, plan: &ThemeCompatibilityPlan) -> Self {
        plan.install(self)
    }

    /// Installs the lower-priority Mermaid compatibility layer for one compiled theme.
    ///
    /// Existing and future host site config remains authoritative over this layer. The metadata
    /// binding is frozen into parsed artifacts so render sessions cannot consume a parse created
    /// by another theme recipe.
    #[cfg(test)]
    pub(crate) fn with_theme_compatibility(mut self, binding: ThemeCompatibilityRecipe) -> Self {
        self.theme_compatibility_config = Some(
            SourcePresentationConfig::from_theme_compatibility_recipe(binding),
        );
        self.rebuild_site_config();
        self
    }

    /// Replaces the complete site-config environment while preserving custom registries.
    ///
    /// `None` restores the pinned Mermaid defaults. An explicit config is merged onto those
    /// defaults without inheriting values from the engine's previous site config.
    pub fn with_exact_site_config(mut self, site_config: Option<SourcePresentationConfig>) -> Self {
        self.site_config_overrides = SourcePresentationConfig::empty_object();
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
        operation_context: &merman_model::runtime::OperationContext,
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
        operation_context: &merman_model::runtime::OperationContext,
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
