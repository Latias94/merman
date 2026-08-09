pub use merman_ascii::{
    ASCII_RESOURCE_LIMIT_COUNT, ASCII_RESOURCE_LIMIT_DESCRIPTORS, AsciiCapability,
    AsciiCapabilityEvidence, AsciiCharset, AsciiColorMode, AsciiColorTheme, AsciiDirection,
    AsciiError, AsciiEvidenceKind, AsciiPrimaryProjection, AsciiRenderOptions, AsciiRenderer,
    AsciiResourceLimitDescriptor, AsciiResourceLimitExceeded, AsciiResourceLimitId,
    AsciiResourceLimitOverrideError, AsciiResourceLimitPhase, AsciiResourcePolicy, AsciiRgb,
    AsciiSemanticCoverage, AsciiSupportLevel, AsciiTerminalPalette,
    MAX_ASCII_DOCUMENT_CELLS_RESOURCE_LIMIT_ID, MAX_ASCII_GRAPHEME_BYTES_RESOURCE_LIMIT_ID,
    MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID, MAX_ASCII_LAYOUT_WORK_UNITS_RESOURCE_LIMIT_ID,
    MAX_ASCII_NESTING_DEPTH_RESOURCE_LIMIT_ID, MAX_ASCII_OUTPUT_BYTES_RESOURCE_LIMIT_ID,
    TerminalWidthProfile, ascii_capabilities, ascii_diagrammatic_diagram_types,
    ascii_resource_profile_value, ascii_supported_diagram_types, render_class, render_er,
    render_flowchart, render_gantt, render_gantt_with_local_time_zone, render_git_graph,
    render_journey, render_kanban, render_mindmap, render_model, render_model_with_local_time_zone,
    render_packet, render_sequence, render_state, render_timeline, render_tree_view,
    render_xychart,
};
pub use merman_ascii::{normalize_terminal_diagnostic, normalize_terminal_text};

/// Machine-readable context for a headless ASCII failure.
///
/// Authored string fields are terminal-safe and bounded. Byte spans remain separate from the
/// human-readable message so bindings do not need to parse display text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HeadlessAsciiDiagnosticDetails {
    pub code: String,
    pub span: Option<merman_core::SourceSpan>,
    pub span_kind: Option<merman_core::ParseDiagnosticSpanKind>,
    pub field: Option<String>,
    pub diagram_type: Option<String>,
}

pub enum HeadlessAsciiError {
    Parse(merman_core::Error),
    Ascii(merman_ascii::AsciiError),
    RuntimePolicy(merman_core::runtime::RuntimePolicyError),
    Resource(merman_core::resources::InputResourceLimitExceeded),
}

impl HeadlessAsciiError {
    pub fn terminal_safe_message(&self) -> String {
        match self {
            Self::Parse(error) => safe_parse_error(error),
            Self::Ascii(error) => safe_ascii_error(error),
            Self::RuntimePolicy(error) => safe_runtime_policy_error(error),
            Self::Resource(error) => normalize_terminal_diagnostic(&error.to_string()),
        }
    }

    pub fn terminal_diagnostic_details(&self) -> Option<HeadlessAsciiDiagnosticDetails> {
        match self {
            Self::Parse(error) => Some(safe_parse_details(error)),
            Self::Ascii(error) => safe_ascii_details(error),
            Self::RuntimePolicy(_) | Self::Resource(_) => None,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Parse(_) => "parse",
            Self::Ascii(_) => "ascii",
            Self::RuntimePolicy(_) => "runtime_policy",
            Self::Resource(_) => "resource",
        }
    }
}

impl std::fmt::Debug for HeadlessAsciiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HeadlessAsciiError")
            .field("kind", &self.kind())
            .field("message", &self.terminal_safe_message())
            .finish()
    }
}

impl std::fmt::Display for HeadlessAsciiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.terminal_safe_message())
    }
}

// The wrapped errors may retain authored source or host-adapter messages. This public display
// boundary therefore intentionally does not expose them through `Error::source()`.
impl std::error::Error for HeadlessAsciiError {}

impl From<merman_core::Error> for HeadlessAsciiError {
    fn from(error: merman_core::Error) -> Self {
        Self::Parse(error)
    }
}

impl From<merman_ascii::AsciiError> for HeadlessAsciiError {
    fn from(error: merman_ascii::AsciiError) -> Self {
        Self::Ascii(error)
    }
}

impl From<merman_core::runtime::RuntimePolicyError> for HeadlessAsciiError {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self::RuntimePolicy(error)
    }
}

impl From<merman_core::resources::InputResourceLimitExceeded> for HeadlessAsciiError {
    fn from(error: merman_core::resources::InputResourceLimitExceeded) -> Self {
        Self::Resource(error)
    }
}

fn safe_parse_error(error: &merman_core::Error) -> String {
    match error {
        merman_core::Error::ParseCancelled(_) => "parse operation cancelled".to_string(),
        merman_core::Error::ThemeColor(error) => match error {
            merman_core::theme_color::ColorError::UnsupportedFormat { input } => {
                bounded_message("Unsupported color format: \"", input, "\"")
            }
            merman_core::theme_color::ColorError::MixedColorSpaces => {
                "Cannot change both RGB and HSL channels at the same time".to_string()
            }
        },
        merman_core::Error::RuntimePolicy(error) => safe_runtime_policy_error(error),
        merman_core::Error::DetectType(_) => "No Mermaid diagram type detected".to_string(),
        merman_core::Error::UnsupportedDiagram { diagram_type } => {
            bounded_message("Unsupported diagram type: ", diagram_type, "")
        }
        merman_core::Error::DiagramParse {
            diagram_type,
            diagnostic,
        } => bounded_two_field_message(
            "Diagram parse error (",
            diagram_type,
            "): ",
            diagnostic.message(),
        ),
        merman_core::Error::MalformedFrontMatter => {
            "Malformed YAML front-matter. If you were trying to use a YAML front-matter, please ensure that you've correctly opened and closed the YAML front-matter with un-indented `---` blocks".to_string()
        }
        merman_core::Error::InvalidDirectiveJson { message } => {
            bounded_message("Invalid directive JSON: ", message, "")
        }
        merman_core::Error::InvalidFrontMatterYaml { message } => {
            bounded_message("Invalid YAML front-matter: ", message, "")
        }
    }
}

fn safe_parse_details(error: &merman_core::Error) -> HeadlessAsciiDiagnosticDetails {
    let mut details = HeadlessAsciiDiagnosticDetails {
        code: "merman.ascii.parse".to_string(),
        span: None,
        span_kind: None,
        field: None,
        diagram_type: None,
    };
    match error {
        merman_core::Error::ParseCancelled(_) => {
            details.code = "merman.ascii.parse.cancelled".to_string();
        }
        merman_core::Error::ThemeColor(_) => {
            details.code = "merman.ascii.theme_color".to_string();
            details.field = Some("theme_color".to_string());
        }
        merman_core::Error::RuntimePolicy(_) => {
            details.code = "merman.ascii.runtime_policy".to_string();
        }
        merman_core::Error::DetectType(_) => {
            details.code = "merman.ascii.no_diagram_detected".to_string();
        }
        merman_core::Error::UnsupportedDiagram { diagram_type } => {
            details.code = "merman.ascii.unsupported_diagram".to_string();
            details.diagram_type = Some(normalize_terminal_diagnostic(diagram_type));
        }
        merman_core::Error::DiagramParse {
            diagram_type,
            diagnostic,
        } => {
            details.code = diagnostic
                .code()
                .map(normalize_terminal_diagnostic)
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| "merman.ascii.diagram_parse".to_string());
            details.span = diagnostic.span();
            details.span_kind = diagnostic.span().map(|_| diagnostic.span_kind());
            details.diagram_type = Some(normalize_terminal_diagnostic(diagram_type));
        }
        merman_core::Error::MalformedFrontMatter => {
            details.code = "merman.ascii.front_matter.malformed".to_string();
            details.field = Some("front_matter".to_string());
        }
        merman_core::Error::InvalidDirectiveJson { .. } => {
            details.code = "merman.ascii.directive.invalid_json".to_string();
            details.field = Some("directive".to_string());
        }
        merman_core::Error::InvalidFrontMatterYaml { .. } => {
            details.code = "merman.ascii.front_matter.invalid_yaml".to_string();
            details.field = Some("front_matter".to_string());
        }
    }
    details
}

fn safe_ascii_error(error: &merman_ascii::AsciiError) -> String {
    match error {
        merman_ascii::AsciiError::InvalidOption { field, message } => {
            bounded_two_field_message("invalid ASCII render option `", field, "`: ", message)
        }
        merman_ascii::AsciiError::UnsupportedDiagram { diagram_type } => bounded_message(
            "ASCII rendering does not support diagram type `",
            diagram_type,
            "`",
        ),
        merman_ascii::AsciiError::UnsupportedFeature {
            diagram_type,
            feature,
        } => {
            let feature = normalize_terminal_diagnostic(feature);
            let diagram_type = normalize_terminal_diagnostic(diagram_type);
            normalize_terminal_diagnostic(&format!(
                "ASCII rendering does not support `{feature}` for `{diagram_type}` yet"
            ))
        }
        merman_ascii::AsciiError::ResourceLimitExceeded(details) => details.to_string(),
        _ => "ASCII rendering failed".to_string(),
    }
}

fn safe_ascii_details(error: &merman_ascii::AsciiError) -> Option<HeadlessAsciiDiagnosticDetails> {
    let (code, field, diagram_type) = match error {
        merman_ascii::AsciiError::InvalidOption { field, .. } => (
            "merman.ascii.invalid_option",
            Some(normalize_terminal_diagnostic(field)),
            None,
        ),
        merman_ascii::AsciiError::UnsupportedDiagram { diagram_type } => (
            "merman.ascii.unsupported_diagram",
            None,
            Some(normalize_terminal_diagnostic(diagram_type)),
        ),
        merman_ascii::AsciiError::UnsupportedFeature {
            diagram_type,
            feature,
        } => (
            "merman.ascii.unsupported_feature",
            Some(normalize_terminal_diagnostic(feature)),
            Some(normalize_terminal_diagnostic(diagram_type)),
        ),
        merman_ascii::AsciiError::ResourceLimitExceeded(_) => return None,
        _ => ("merman.ascii.render", None, None),
    };
    Some(HeadlessAsciiDiagnosticDetails {
        code: code.to_string(),
        span: None,
        span_kind: None,
        field,
        diagram_type,
    })
}

fn safe_runtime_policy_error(error: &merman_core::runtime::RuntimePolicyError) -> String {
    match error {
        merman_core::runtime::RuntimePolicyError::SystemTimeZone(message) => {
            bounded_message("system time-zone adapter failed: ", message, "")
        }
        merman_core::runtime::RuntimePolicyError::SystemRandom(message) => {
            bounded_message("system random adapter failed: ", message, "")
        }
        _ => normalize_terminal_diagnostic(&error.to_string()),
    }
}

fn bounded_message(prefix: &str, value: &str, suffix: &str) -> String {
    let value = normalize_terminal_diagnostic(value);
    normalize_terminal_diagnostic(&format!("{prefix}{value}{suffix}"))
}

fn bounded_two_field_message(prefix: &str, first: &str, separator: &str, second: &str) -> String {
    let first = normalize_terminal_diagnostic(first);
    let second = normalize_terminal_diagnostic(second);
    normalize_terminal_diagnostic(&format!("{prefix}{first}{separator}{second}"))
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
        let resources = ascii.resources.with_profile(self.resources.profile());
        self.ascii = ascii.with_resource_policy(resources);
        self
    }

    pub fn with_resource_profile(
        mut self,
        profile: merman_core::resources::ResourceProfile,
    ) -> Self {
        self.resources = merman_core::resources::InputResourcePolicy::for_profile(profile);
        self.ascii.resources = self.ascii.resources.with_profile(profile);
        self
    }

    pub fn with_resource_policy(
        mut self,
        resources: merman_core::resources::InputResourcePolicy,
    ) -> Self {
        self.ascii.resources = self.ascii.resources.with_profile(resources.profile());
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

    fn render_with_ascii_limit(
        limit: AsciiResourceLimitId,
        max: usize,
        source: &str,
    ) -> Result<Option<String>> {
        let ascii = AsciiRenderOptions::ascii()
            .with_resource_limit(limit, max)
            .expect("test limit must satisfy the public minimum");
        HeadlessAsciiRenderer::new()
            .with_ascii_options(ascii)
            .render_ascii_sync(source)
    }

    fn ascii_limit_details(error: HeadlessAsciiError) -> AsciiResourceLimitExceeded {
        match error {
            HeadlessAsciiError::Ascii(AsciiError::ResourceLimitExceeded(details)) => details,
            other => panic!("expected typed ASCII resource error, got {other:?}"),
        }
    }

    fn assert_headless_ascii_exact_boundary(limit: AsciiResourceLimitId, source: &str) {
        let mut lower = 0usize;
        let mut candidate = 1usize;
        let upper = loop {
            match render_with_ascii_limit(limit, candidate, source) {
                Ok(Some(output)) => {
                    assert!(!output.is_empty(), "{} produced no output", limit.as_str());
                    break candidate;
                }
                Ok(None) => panic!("{} fixture was not detected", limit.as_str()),
                Err(error) => {
                    let details = ascii_limit_details(error);
                    assert_eq!(details.limit, limit);
                    assert_eq!(details.max, candidate);
                    lower = candidate;
                    candidate = details.actual.max(candidate.saturating_mul(2));
                }
            }
        };

        let mut upper = upper;
        while upper - lower > 1 {
            let candidate = lower + (upper - lower) / 2;
            match render_with_ascii_limit(limit, candidate, source) {
                Ok(Some(_)) => upper = candidate,
                Ok(None) => panic!("{} fixture was not detected", limit.as_str()),
                Err(error) => {
                    let details = ascii_limit_details(error);
                    assert_eq!(details.limit, limit);
                    assert_eq!(details.max, candidate);
                    lower = candidate;
                }
            }
        }

        render_with_ascii_limit(limit, upper, source)
            .unwrap_or_else(|error| panic!("exact {} boundary failed: {error:?}", limit.as_str()))
            .expect("fixture should render at the exact boundary");
        let details = ascii_limit_details(
            render_with_ascii_limit(limit, upper - 1, source)
                .expect_err("one-below headless ASCII boundary must fail"),
        );
        assert_eq!(details.limit, limit);
        assert_eq!(details.phase(), limit.descriptor().phase);
        assert_eq!(details.actual, upper);
        assert_eq!(details.max, upper - 1);
        assert_eq!(details.profile.id(), "interactive");
    }

    fn task_by_id<'a>(model: &'a Value, id: &str) -> &'a Value {
        model["tasks"]
            .as_array()
            .expect("Gantt tasks should be an array")
            .iter()
            .find(|task| task["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing Gantt task {id} in {model}"))
    }

    #[test]
    fn headless_ascii_renderer_proves_every_ascii_limit_at_exact_boundary() {
        let cases = [
            (
                AsciiResourceLimitId::MaxGridCells,
                "flowchart TD\nA[Hello] --> B[World]",
            ),
            (
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                "flowchart TD\nA[Hello] --> B[World]",
            ),
            (
                AsciiResourceLimitId::MaxDocumentCells,
                "gitGraph\n  commit id: \"A\"",
            ),
            (
                AsciiResourceLimitId::MaxOutputBytes,
                "flowchart TD\nA[Hello] --> B[World]",
            ),
            (
                AsciiResourceLimitId::MaxGraphemeBytes,
                "flowchart TD\nA[👨‍👩‍👧‍👦]",
            ),
            (
                AsciiResourceLimitId::MaxNestingDepth,
                "mindmap\n  Root\n    Child",
            ),
        ];

        for (limit, source) in cases {
            assert_headless_ascii_exact_boundary(limit, source);
        }
    }

    #[test]
    fn headless_ascii_error_does_not_echo_undetected_source_or_terminal_controls() {
        let source = "not-a-diagram\u{1b}]8;;https://example.invalid\u{7}link";

        let error = HeadlessAsciiRenderer::new()
            .render_ascii_sync(source)
            .expect_err("source should not detect as Mermaid");
        let message = error.to_string();

        assert_eq!(message, "No Mermaid diagram type detected");
        assert!(!message.contains(source));
        assert!(!message.contains('\u{1b}'));
        assert!(!message.contains('\u{7}'));
    }

    #[test]
    fn headless_ascii_error_debug_and_source_do_not_bypass_terminal_safety() {
        let error = HeadlessAsciiError::from(merman_core::Error::diagram_parse_fallback(
            "flow\u{1b}",
            format!("bad\u{7}{}", "\u{301}".repeat(20_000)),
        ));

        let display = error.to_string();
        let debug = format!("{error:?}");

        assert!(display.len() <= 4 * 1024);
        assert!(!display.contains('\u{1b}'));
        assert!(!display.contains('\u{7}'));
        assert!(!debug.contains('\u{1b}'));
        assert!(!debug.contains('\u{7}'));
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn headless_ascii_error_preserves_bounded_structured_parse_details() {
        let span = merman_core::SourceSpan::new(4, 9);
        let diagnostic = merman_core::ParseDiagnostic::new("bad input")
            .with_span(span, merman_core::ParseDiagnosticSpanKind::Exact)
            .with_code("merman.test\u{1b}");
        let error = HeadlessAsciiError::from(merman_core::Error::diagram_parse_diagnostic(
            "flow\u{7}",
            diagnostic,
        ));

        let details = error
            .terminal_diagnostic_details()
            .expect("parse details should be available");

        assert_eq!(details.code, "merman.test\\u{1B}");
        assert_eq!(details.span, Some(span));
        assert_eq!(
            details.span_kind,
            Some(merman_core::ParseDiagnosticSpanKind::Exact)
        );
        assert_eq!(details.diagram_type.as_deref(), Some("flow\\u{7}"));
        assert_eq!(details.field, None);
    }

    #[test]
    fn headless_ascii_error_classifies_renderer_failures_without_unsafe_text() {
        let cases = [
            (
                HeadlessAsciiError::from(merman_ascii::AsciiError::InvalidOption {
                    field: "box_border_padding",
                    message: "must be positive",
                }),
                "merman.ascii.invalid_option",
                Some("box_border_padding"),
                None,
            ),
            (
                HeadlessAsciiError::from(merman_ascii::AsciiError::UnsupportedDiagram {
                    diagram_type: "radar\u{1b}".to_string(),
                }),
                "merman.ascii.unsupported_diagram",
                None,
                Some("radar\\u{1B}"),
            ),
            (
                HeadlessAsciiError::from(merman_ascii::AsciiError::UnsupportedFeature {
                    diagram_type: "flowchart",
                    feature: "missing endpoint nodes",
                }),
                "merman.ascii.unsupported_feature",
                Some("missing endpoint nodes"),
                Some("flowchart"),
            ),
        ];

        for (error, code, field, diagram_type) in cases {
            let message = error.terminal_safe_message();
            let details = error
                .terminal_diagnostic_details()
                .expect("renderer failure should have diagnostic details");

            assert!(!message.contains('\u{1b}'));
            assert_eq!(details.code, code);
            assert_eq!(details.field.as_deref(), field);
            assert_eq!(details.diagram_type.as_deref(), diagram_type);
        }
    }

    #[test]
    fn headless_ascii_renderer_fixed_time_controls_semantic_parse() {
        let today = merman_core::time::CivilDate::new(2026, 2, 15).expect("valid fixed today");
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

        assert!(rendered.contains("Task [id=task1"), "{rendered}");
        assert!(
            rendered.contains("range=2026-01-01 -> 2026-01-02"),
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
        assert_eq!(
            constrained
                .ascii
                .resources
                .value(AsciiResourceLimitId::MaxGridCells),
            Some(125_000)
        );

        let trusted = HeadlessAsciiRenderer::new()
            .with_resource_profile(merman_core::resources::ResourceProfile::TrustedNative);
        assert_eq!(
            trusted
                .ascii
                .resources
                .value(AsciiResourceLimitId::MaxGridCells),
            Some(1_000_000)
        );

        let unbounded = HeadlessAsciiRenderer::new().with_resource_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        assert_eq!(
            unbounded
                .ascii
                .resources
                .value(AsciiResourceLimitId::MaxGridCells),
            None
        );
    }

    #[test]
    fn ascii_options_and_resource_profile_are_order_independent() {
        let ascii = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxGridCells, 42)
            .expect("valid ASCII override");
        let profile = merman_core::resources::ResourceProfile::Constrained;

        let profile_then_options = HeadlessAsciiRenderer::new()
            .with_resource_profile(profile)
            .with_ascii_options(ascii);
        let options_then_profile = HeadlessAsciiRenderer::new()
            .with_ascii_options(ascii)
            .with_resource_profile(profile);

        assert_eq!(profile_then_options.ascii, options_then_profile.ascii);
        assert_eq!(
            profile_then_options
                .ascii
                .resources
                .value(AsciiResourceLimitId::MaxGridCells),
            Some(42)
        );
        assert_eq!(
            profile_then_options
                .ascii
                .resources
                .value(AsciiResourceLimitId::MaxOutputBytes),
            Some(8 * 1024 * 1024)
        );
    }
}
