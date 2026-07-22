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
mod parse_pipeline;
pub mod preprocess;
pub mod resources;
mod runtime;
pub mod sanitize;
mod theme;
pub mod theme_color;
pub mod time;
pub mod utils;
mod yaml_config;

pub use config::MermaidConfig;
pub use detect::{Detector, DetectorRegistry};
pub use diagram::{
    BLOCK_WIDTH_WARNING_RULE_ID, BuiltinRenderSemantic, CustomJsonProvenance,
    CustomJsonRenderModel, CustomJsonRenderParser, DiagramParseOutcome, DiagramParseSnapshot,
    DiagramRegistry, DiagramSemanticParser, DiagramWarningFact,
    FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID, FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID,
    GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID, ParsedDiagram, ParsedDiagramRender,
    ParsedEditorFacts, RenderDiagramRegistry, RenderSemanticModel,
};
pub use editor::{
    EditorCompletionDialect, EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorLexeme,
    EditorLexemeFailure, EditorLexemeKind, EditorLexemeModifier, EditorLexemeModifiers,
    EditorLexemeProducer, EditorLexemeProducerKind, EditorRenamePolicy, EditorSemanticCompleteness,
    EditorSemanticDiagnostic, EditorSemanticDiagnosticKind, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol, EditorSpanCoordinateSpace,
    SourceSpan,
};
pub use error::{Error, ParseDiagnostic, ParseDiagnosticSpanKind, Result};
pub use family::{
    DiagramFamilyCapability, DiagramFamilyId, DiagramHeaderFact, diagram_type_family_kind,
    diagram_type_metadata_id, diagram_type_render_model_kind,
};
pub use preprocess::{
    PreprocessResult, PreprocessedSource, preprocess_diagram, preprocess_diagram_with_known_type,
};

/// Maximum nested diagram/include depth accepted by recursive parsers.
pub const MAX_DIAGRAM_NESTING_DEPTH: usize = 256;

/// Returns Mermaid theme names supported by the pinned baseline.
pub fn supported_themes() -> &'static [&'static str] {
    theme::SUPPORTED_THEME_NAMES
}

/// Returns supported diagram metadata names for binding and host capability discovery.
pub fn supported_diagrams() -> &'static [&'static str] {
    family::supported_diagram_metadata_ids()
}

/// Returns the complete family capability facts for Mermaid diagram ids in the pinned baseline.
pub fn diagram_family_capabilities() -> &'static [DiagramFamilyCapability] {
    family::diagram_family_capabilities()
}

/// Returns header completion facts for Mermaid diagram starters in the pinned baseline.
pub fn diagram_header_facts() -> &'static [DiagramHeaderFact] {
    family::diagram_header_facts()
}

fn build_default_effective_config(
    site_config: &MermaidConfig,
) -> std::result::Result<MermaidConfig, theme_color::ColorError> {
    let mut effective_config = site_config.clone();
    theme::apply_theme_defaults(&mut effective_config)?;
    Ok(effective_config)
}

fn generated_default_effective_config()
-> std::result::Result<MermaidConfig, theme_color::ColorError> {
    static DEFAULT_EFFECTIVE_CONFIG: std::sync::OnceLock<
        std::result::Result<MermaidConfig, theme_color::ColorError>,
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

/// Headless Mermaid parser engine.
///
/// An engine owns detector/parser registries and a site-level Mermaid configuration. It is cheap
/// to clone when callers need per-request option variants.
#[derive(Debug, Clone)]
pub struct Engine {
    registry: DetectorRegistry,
    diagram_registry: DiagramRegistry,
    render_diagram_registry: RenderDiagramRegistry,
    site_config: MermaidConfig,
    default_effective_config: std::result::Result<MermaidConfig, theme_color::ColorError>,
    fixed_today_local: Option<chrono::NaiveDate>,
    local_time_zone: std::result::Result<time::LocalTimeZone, time::LocalTimeZoneError>,
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
            default_effective_config,
            fixed_today_local: None,
            local_time_zone: Ok(time::LocalTimeZone::ambient()),
        }
    }
}

impl Engine {
    pub(crate) fn parse_timing_enabled() -> bool {
        #[cfg(feature = "host-timing")]
        {
            Self::parse_timing_enabled_from_env()
        }

        #[cfg(not(feature = "host-timing"))]
        false
    }

    #[cfg(feature = "host-timing")]
    fn parse_timing_enabled_from_env() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| {
            matches!(
                std::env::var("MERMAN_PARSE_TIMING").as_deref(),
                Ok("1") | Ok("true")
            )
        })
    }

    /// Creates an engine using the pinned Mermaid baseline registries and default site config.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn default_effective_config(&self) -> Result<MermaidConfig> {
        self.default_effective_config.clone().map_err(Error::from)
    }

    /// Overrides the "today" value used by diagrams that depend on local time (e.g. Gantt).
    ///
    /// This exists primarily to make fixture snapshots deterministic. By default, Mermaid uses the
    /// current local date.
    pub fn with_fixed_today(mut self, today: Option<chrono::NaiveDate>) -> Self {
        self.fixed_today_local = today;
        self
    }

    /// Overrides the local timezone offset (in minutes) used by diagrams that depend on local time
    /// semantics (notably Gantt).
    ///
    /// This exists primarily to make fixture snapshots deterministic across CI runners. When
    /// `None`, the system local timezone is used.
    pub fn with_fixed_local_offset_minutes(mut self, offset_minutes: Option<i32>) -> Self {
        self.local_time_zone = match offset_minutes {
            Some(offset_minutes) => time::LocalTimeZone::fixed(offset_minutes),
            None => Ok(time::LocalTimeZone::ambient()),
        };
        self
    }

    /// Installs an already-resolved local timezone without consulting ambient process state.
    pub fn with_local_time_zone(mut self, time_zone: time::LocalTimeZone) -> Self {
        self.local_time_zone = Ok(time_zone);
        self
    }

    /// Returns the fixed local timezone offset configured for this engine.
    pub fn fixed_local_offset_minutes(&self) -> Option<i32> {
        self.local_time_zone
            .as_ref()
            .ok()
            .and_then(time::LocalTimeZone::fixed_offset_minutes)
    }

    pub fn local_time_zone(
        &self,
    ) -> std::result::Result<&time::LocalTimeZone, &time::LocalTimeZoneError> {
        self.local_time_zone.as_ref()
    }

    /// Applies site-level Mermaid config defaults.
    pub fn with_site_config(mut self, mut site_config: MermaidConfig) -> Self {
        if site_config.is_empty_object() {
            return self;
        }
        // Merge overrides onto Mermaid schema defaults so detectors keep working.
        config::mirror_legacy_font_family_into_theme_variables(&mut site_config);
        self.site_config.deep_merge(site_config.as_value());
        self.default_effective_config = build_default_effective_config(&self.site_config);
        self
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

    /// Captures semantic JSON or its original error and parser-backed editor facts in one operation.
    ///
    /// This is intended for editor integrations that need both diagnostics/facts and the
    /// Mermaid-compatible model. Once preprocessing and detection succeed, family parse errors are
    /// retained inside the snapshot alongside metadata and recovery facts. Consumers must project
    /// that failure state directly rather than parsing the source again.
    /// Error suppression is deliberately absent from this API; suppression remains limited to
    /// model-producing JSON and render facades.
    pub fn parse_diagram_snapshot_sync(&self, text: &str) -> Result<Option<DiagramParseSnapshot>> {
        parse_pipeline::ParsePipeline::detect(self, text, ParseOptions::strict())
            .parse_editor_snapshot(parse_pipeline::ParseTiming::Json)
    }

    /// Captures one editor-facing parse operation when the diagram type is already known.
    ///
    /// This has the same closed snapshot contract as [`Engine::parse_diagram_snapshot_sync`], but
    /// skips automatic detection. Family parse failures remain inside the returned snapshot.
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
