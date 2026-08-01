pub use merman_ascii::{
    ASCII_RESOURCE_LIMIT_DESCRIPTORS, AsciiCapability, AsciiCapabilityEvidence, AsciiCharset,
    AsciiColorMode, AsciiColorTheme, AsciiDirection, AsciiError, AsciiEvidenceKind,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitDescriptor, AsciiRgb, AsciiSupportLevel,
    AsciiTerminalPalette, MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID, ascii_capabilities,
    ascii_resource_profile_value, ascii_supported_diagram_types, render_class, render_er,
    render_flowchart, render_gantt, render_gantt_with_local_time_zone, render_git_graph,
    render_journey, render_kanban, render_mindmap, render_model, render_model_with_local_time_zone,
    render_packet, render_sequence, render_timeline, render_tree_view, render_xychart,
};

#[derive(Debug, thiserror::Error)]
pub enum HeadlessAsciiError {
    #[error(transparent)]
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    Ascii(#[from] merman_ascii::AsciiError),
    #[error(transparent)]
    RuntimePolicy(#[from] merman_core::runtime::RuntimePolicyError),
    #[error(transparent)]
    Resource(#[from] merman_core::resources::InputResourceLimitExceeded),
}

pub type Result<T> = std::result::Result<T, HeadlessAsciiError>;

fn render_model_with_engine_time(
    engine: &merman_core::Engine,
    model: &merman_core::diagram::RenderSemanticModel,
    ascii_options: &AsciiRenderOptions,
) -> Result<String> {
    let context = engine.begin_operation()?;
    Ok(merman_ascii::render_model_with_local_time_zone(
        model,
        ascii_options,
        context.local_time_zone(),
    )?)
}

/// Synchronous ASCII/Unicode render helper (executor-free).
///
/// The Mermaid source is parsed by `merman-core`; the typed render model is then rendered by
/// `merman-ascii`. Supported diagram families currently include flowchart, sequenceDiagram,
/// classDiagram, erDiagram, stateDiagram, xychart, mindmap, treeView, timeline, gantt, journey,
/// kanban, packet, and gitGraph.
pub fn render_ascii_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    ascii_options: &AsciiRenderOptions,
) -> Result<Option<String>> {
    render_ascii_with_resource_policy_sync(
        engine,
        text,
        parse_options,
        ascii_options,
        &merman_core::resources::InputResourcePolicy::default(),
    )
}

fn render_ascii_with_resource_policy_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    ascii_options: &AsciiRenderOptions,
    resources: &merman_core::resources::InputResourcePolicy,
) -> Result<Option<String>> {
    resources.check_source_bytes(text)?;
    let context = engine.begin_operation()?;
    let operation_engine = engine.clone().with_operation_context(context.clone());
    let Some(parsed) = operation_engine.parse_diagram_for_render_model_sync(text, parse_options)?
    else {
        return Ok(None);
    };
    resources.check_render_model(parsed.model())?;

    Ok(Some(merman_ascii::render_model_with_local_time_zone(
        parsed.model(),
        ascii_options,
        context.local_time_zone(),
    )?))
}

pub async fn render_ascii(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    ascii_options: &AsciiRenderOptions,
) -> Result<Option<String>> {
    // This async API is runtime-agnostic: rendering is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    render_ascii_sync(engine, text, parse_options, ascii_options)
}

/// Convenience wrapper that bundles an [`merman_core::Engine`] and ASCII render options.
///
/// This is intended for terminal, log, documentation, and chat-surface integrations that want
/// stable text output without wiring parsing and rendering parameters on every call.
#[derive(Clone)]
pub struct HeadlessAsciiRenderer {
    pub engine: merman_core::Engine,
    pub parse: merman_core::ParseOptions,
    pub ascii: AsciiRenderOptions,
    resources: merman_core::resources::InputResourcePolicy,
}

impl Default for HeadlessAsciiRenderer {
    fn default() -> Self {
        Self {
            engine: merman_core::Engine::new(),
            parse: merman_core::ParseOptions::default(),
            ascii: AsciiRenderOptions::default(),
            resources: merman_core::resources::InputResourcePolicy::default(),
        }
    }
}

impl HeadlessAsciiRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_native() -> Result<Self> {
        Ok(Self::new().with_runtime_policy(merman_core::runtime::RuntimePolicy::try_native()?))
    }

    pub fn with_engine(mut self, engine: merman_core::Engine) -> Self {
        self.engine = engine;
        self
    }

    pub fn with_site_config(mut self, site_config: merman_core::MermaidConfig) -> Self {
        self.engine = self.engine.with_site_config(site_config);
        self
    }

    pub fn with_runtime_policy(mut self, policy: merman_core::runtime::RuntimePolicy) -> Self {
        self.engine = self.engine.with_runtime_policy(policy);
        self
    }

    pub fn with_operation_context(
        mut self,
        context: merman_core::runtime::OperationContext,
    ) -> Self {
        self.engine = self.engine.with_operation_context(context);
        self
    }

    pub fn with_parse_options(mut self, parse: merman_core::ParseOptions) -> Self {
        self.parse = parse;
        self
    }

    pub fn with_strict_parsing(self) -> Self {
        self.with_parse_options(merman_core::ParseOptions::strict())
    }

    pub fn with_lenient_parsing(self) -> Self {
        self.with_parse_options(merman_core::ParseOptions::lenient())
    }

    pub fn with_ascii_options(mut self, ascii: AsciiRenderOptions) -> Self {
        self.ascii = ascii;
        self
    }

    pub fn with_resource_profile(
        mut self,
        profile: merman_core::resources::ResourceProfile,
    ) -> Self {
        self.resources = merman_core::resources::InputResourcePolicy::for_profile(profile);
        self.ascii.max_grid_cells =
            ascii_resource_profile_value(profile, MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID)
                .unwrap_or(usize::MAX);
        self
    }

    pub fn with_resource_policy(
        mut self,
        resources: merman_core::resources::InputResourcePolicy,
    ) -> Self {
        self.resources = resources;
        self
    }

    pub const fn resource_policy(&self) -> &merman_core::resources::InputResourcePolicy {
        &self.resources
    }

    pub fn with_charset(mut self, charset: AsciiCharset) -> Self {
        self.ascii.charset = charset;
        self
    }

    pub fn parse_metadata_sync(&self, text: &str) -> Result<merman_core::ParseMetadata> {
        self.resources.check_source_bytes(text)?;
        Ok(self.engine.parse_metadata_sync(text)?)
    }

    pub fn parse_diagram_sync(&self, text: &str) -> Result<Option<merman_core::ParsedDiagram>> {
        self.resources.check_source_bytes(text)?;
        Ok(self.engine.parse_diagram_sync(text, self.parse)?)
    }

    pub fn render_model(
        &self,
        model: &merman_core::diagram::RenderSemanticModel,
    ) -> Result<String> {
        self.resources.check_render_model(model)?;
        render_model_with_engine_time(&self.engine, model, &self.ascii)
    }

    pub fn render_ascii_sync(&self, text: &str) -> Result<Option<String>> {
        render_ascii_with_resource_policy_sync(
            &self.engine,
            text,
            self.parse,
            &self.ascii,
            &self.resources,
        )
    }

    pub async fn render_ascii(&self, text: &str) -> Result<Option<String>> {
        self.render_ascii_sync(text)
    }
}

#[cfg(test)]
mod headless_ascii_renderer_tests {
    use super::*;
    use serde_json::Value;

    fn task_by_id<'a>(model: &'a Value, id: &str) -> &'a Value {
        model["tasks"]
            .as_array()
            .expect("Gantt tasks should be an array")
            .iter()
            .find(|task| task["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing Gantt task {id} in {model}"))
    }

    #[test]
    fn headless_ascii_renderer_fixed_time_controls_semantic_parse() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid fixed today");
        let policy = merman_core::runtime::RuntimePolicy::deterministic()
            .try_with_fixed_local_offset_minutes(0)
            .expect("valid UTC offset")
            .with_fixed_today(Some(today));
        let renderer = HeadlessAsciiRenderer::new().with_runtime_policy(policy);
        let parsed = renderer
            .parse_diagram_sync(
                r#"gantt
dateFormat MM-DD
section Demo
Missing year: id1,03-01,1d
Missing ref: id2,after missing,1d
"#,
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            task_by_id(&parsed.model, "id1")["startTime"].as_i64(),
            Some(1_772_323_200_000)
        );
        assert_eq!(
            task_by_id(&parsed.model, "id2")["startTime"].as_i64(),
            Some(1_771_113_600_000)
        );
    }

    #[test]
    fn headless_ascii_renderer_fixed_local_offset_controls_gantt_render_dates() {
        let policy = merman_core::runtime::RuntimePolicy::deterministic()
            .try_with_fixed_local_offset_minutes(14 * 60)
            .expect("valid fixed offset");
        let renderer = HeadlessAsciiRenderer::new()
            .with_strict_parsing()
            .with_runtime_policy(policy);

        let rendered = renderer
            .render_ascii_sync(
                r#"gantt
dateFormat YYYY-MM-DD
section Demo
Task: task1, 2026-01-01, 1d
"#,
            )
            .unwrap()
            .unwrap();

        assert!(
            rendered.contains("  - Task [2026-01-01 -> 2026-01-02]"),
            "{rendered}"
        );
    }

    #[test]
    fn headless_ascii_renderer_owns_source_and_model_resource_checks() {
        let resources = merman_core::resources::InputResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        )
        .with_limit(
            merman_core::resources::InputResourceLimitId::MaxModelItems,
            1,
        )
        .unwrap();
        let renderer = HeadlessAsciiRenderer::new().with_resource_policy(resources);

        let error = renderer
            .render_ascii_sync("flowchart TD\nA --> B")
            .unwrap_err();
        assert!(matches!(error, HeadlessAsciiError::Resource(_)));
    }

    #[test]
    fn headless_ascii_renderer_resource_profile_applies_ascii_grid_budget() {
        let constrained = HeadlessAsciiRenderer::new()
            .with_resource_profile(merman_core::resources::ResourceProfile::Constrained);
        assert_eq!(constrained.ascii.max_grid_cells, 125_000);

        let trusted = HeadlessAsciiRenderer::new()
            .with_resource_profile(merman_core::resources::ResourceProfile::TrustedNative);
        assert_eq!(trusted.ascii.max_grid_cells, 1_000_000);

        let unbounded = HeadlessAsciiRenderer::new().with_resource_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        assert_eq!(unbounded.ascii.max_grid_cells, usize::MAX);
    }
}
