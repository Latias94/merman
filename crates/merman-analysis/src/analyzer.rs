use crate::rules::AnalysisRuleConfig;
use crate::{
    AnalysisDiagnostic, AnalysisPayload, AnalysisStatus, DiagnosticCategory, DiagnosticSeverity,
    SourceDescriptor, SourceMap,
};
use merman_core::{
    EditorSemanticDiagnostic, Engine, Error as CoreError, MermaidConfig, ParseOptions,
};
use std::panic::{self, AssertUnwindSafe};

const NO_DIAGRAM_MESSAGE: &str = "no Mermaid diagram detected";

#[derive(Debug, Clone)]
pub struct AnalysisOptions {
    pub parse: ParseOptions,
    pub source: SourceDescriptor,
    pub site_config: Option<MermaidConfig>,
    pub fixed_today: Option<chrono::NaiveDate>,
    pub fixed_local_offset_minutes: Option<i32>,
    pub max_source_bytes: Option<usize>,
    pub rule_config: AnalysisRuleConfig,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            parse: ParseOptions::strict(),
            source: SourceDescriptor::diagram(),
            site_config: None,
            fixed_today: None,
            fixed_local_offset_minutes: None,
            max_source_bytes: None,
            rule_config: AnalysisRuleConfig::default(),
        }
    }
}

impl AnalysisOptions {
    pub fn with_parse_options(mut self, parse: ParseOptions) -> Self {
        self.parse = parse;
        self
    }

    pub fn with_source(mut self, source: SourceDescriptor) -> Self {
        self.source = source;
        self
    }

    pub fn with_site_config(mut self, site_config: MermaidConfig) -> Self {
        self.site_config = Some(site_config);
        self
    }

    pub fn with_fixed_today(mut self, today: Option<chrono::NaiveDate>) -> Self {
        self.fixed_today = today;
        self
    }

    pub fn with_fixed_local_offset_minutes(mut self, offset_minutes: Option<i32>) -> Self {
        self.fixed_local_offset_minutes = offset_minutes;
        self
    }

    pub fn with_max_source_bytes(mut self, max_source_bytes: Option<usize>) -> Self {
        self.max_source_bytes = max_source_bytes;
        self
    }

    pub fn with_rule_config(mut self, rule_config: AnalysisRuleConfig) -> Self {
        self.rule_config = rule_config;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Analyzer {
    engine: Engine,
    options: AnalysisOptions,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    pub fn new() -> Self {
        Self::with_options(AnalysisOptions::default())
    }

    pub fn with_options(options: AnalysisOptions) -> Self {
        let engine = engine_from_options(&options);
        Self { engine, options }
    }

    pub fn with_engine_and_options(engine: Engine, options: AnalysisOptions) -> Self {
        Self { engine, options }
    }

    pub fn analyze(&self, source: &str) -> AnalysisPayload {
        let source_map = SourceMap::new(source);

        if source.trim().is_empty() {
            return self.payload(vec![no_diagram_diagnostic(&source_map)]);
        }

        if let Some(limit) = self.options.max_source_bytes {
            if source.len() > limit {
                return self.payload(vec![source_limit_diagnostic(
                    source.len(),
                    limit,
                    &source_map,
                )]);
            }
        }

        let source_lints =
            crate::rules::source_lint_diagnostics(source, &source_map, &self.options.rule_config);

        let parse_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine.parse_diagram_sync(source, self.options.parse)
        }));

        match parse_result {
            Err(panic_payload) => self.payload(with_source_lints(
                vec![panic_diagnostic(panic_payload, &source_map)],
                source_lints,
            )),
            Ok(parse_result) => match parse_result {
                Ok(Some(parsed)) => {
                    let mut diagnostics = source_lints;
                    diagnostics.extend(crate::rules::semantic_warning_diagnostics(
                        &parsed.meta.diagram_type,
                        &parsed.model,
                        &source_map,
                    ));
                    diagnostics.extend(self.editor_recovery_diagnostics(
                        source,
                        &parsed.meta.diagram_type,
                        &source_map,
                    ));
                    self.payload(diagnostics)
                }
                Ok(None) => self.payload(with_source_lints(
                    vec![no_diagram_diagnostic(&source_map)],
                    source_lints,
                )),
                Err(error) => {
                    let mut diagnostics = with_source_lints(
                        vec![core_error_diagnostic(error, &source_map)],
                        source_lints,
                    );
                    let diagram_type = diagnostics
                        .first()
                        .and_then(|diagnostic| diagnostic.diagram_type.as_deref())
                        .map(str::to_owned);
                    if let Some(diagram_type) = diagram_type {
                        diagnostics.extend(self.editor_recovery_diagnostics(
                            source,
                            &diagram_type,
                            &source_map,
                        ));
                    }
                    self.payload(diagnostics)
                }
            },
        }
    }

    pub fn analyze_json(&self, source: &str) -> Result<Vec<u8>, serde_json::Error> {
        self.analyze(source).to_json_bytes()
    }

    fn payload(&self, diagnostics: Vec<AnalysisDiagnostic>) -> AnalysisPayload {
        AnalysisPayload::new(self.options.source.clone(), diagnostics)
    }

    fn editor_recovery_diagnostics(
        &self,
        source: &str,
        diagram_type: &str,
        source_map: &SourceMap,
    ) -> Vec<AnalysisDiagnostic> {
        let facts_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine.parse_editor_semantic_facts_with_type_sync(
                diagram_type,
                source,
                self.options.parse,
            )
        }));

        match facts_result {
            Err(panic_payload) => vec![panic_diagnostic(panic_payload, source_map)],
            Ok(Ok(Some(facts))) => facts
                .diagnostics
                .into_iter()
                .map(|diagnostic| recovered_editor_diagnostic(diagnostic, diagram_type, source_map))
                .collect(),
            Ok(Ok(None) | Err(_)) => Vec::new(),
        }
    }
}

fn with_source_lints(
    mut diagnostics: Vec<AnalysisDiagnostic>,
    source_lints: Vec<AnalysisDiagnostic>,
) -> Vec<AnalysisDiagnostic> {
    diagnostics.extend(source_lints);
    diagnostics
}

fn engine_from_options(options: &AnalysisOptions) -> Engine {
    let mut engine = Engine::new()
        .with_fixed_today(options.fixed_today)
        .with_fixed_local_offset_minutes(options.fixed_local_offset_minutes);

    if let Some(site_config) = options.site_config.clone() {
        engine = engine.with_site_config(site_config);
    }

    engine
}

fn no_diagram_diagnostic(source_map: &SourceMap) -> AnalysisDiagnostic {
    diagnostic(
        "merman.parse.no_diagram",
        DiagnosticSeverity::Error,
        DiagnosticCategory::Parse,
        NO_DIAGRAM_MESSAGE,
        AnalysisStatus::NoDiagram,
        source_map,
    )
}

fn source_limit_diagnostic(
    source_len: usize,
    limit: usize,
    source_map: &SourceMap,
) -> AnalysisDiagnostic {
    diagnostic(
        "merman.resource.source_bytes_exceeded",
        DiagnosticSeverity::Error,
        DiagnosticCategory::Resource,
        format!("source is {source_len} bytes, exceeding max_source_bytes {limit}"),
        AnalysisStatus::ResourceLimitExceeded,
        source_map,
    )
}

fn core_error_diagnostic(error: CoreError, source_map: &SourceMap) -> AnalysisDiagnostic {
    match error {
        CoreError::DetectType(_) => no_diagram_diagnostic(source_map),
        CoreError::UnsupportedDiagram { diagram_type } => diagnostic(
            "merman.compatibility.unsupported_diagram",
            DiagnosticSeverity::Error,
            DiagnosticCategory::Compatibility,
            format!("unsupported diagram type: {diagram_type}"),
            AnalysisStatus::UnsupportedFormat,
            source_map,
        )
        .with_diagram_type(diagram_type),
        CoreError::DiagramParse {
            diagram_type,
            message,
        } => diagnostic(
            "merman.parse.diagram_parse",
            DiagnosticSeverity::Error,
            DiagnosticCategory::Parse,
            message,
            AnalysisStatus::ParseError,
            source_map,
        )
        .with_diagram_type(diagram_type),
        CoreError::MalformedFrontMatter => diagnostic(
            "merman.config.malformed_front_matter",
            DiagnosticSeverity::Error,
            DiagnosticCategory::Config,
            CoreError::MalformedFrontMatter.to_string(),
            AnalysisStatus::ParseError,
            source_map,
        ),
        CoreError::InvalidDirectiveJson { message } => diagnostic(
            "merman.config.invalid_directive_json",
            DiagnosticSeverity::Error,
            DiagnosticCategory::Config,
            format!("invalid directive JSON: {message}"),
            AnalysisStatus::ParseError,
            source_map,
        ),
        CoreError::InvalidFrontMatterYaml { message } => diagnostic(
            "merman.config.invalid_front_matter_yaml",
            DiagnosticSeverity::Error,
            DiagnosticCategory::Config,
            format!("invalid YAML front-matter: {message}"),
            AnalysisStatus::ParseError,
            source_map,
        ),
    }
}

fn panic_diagnostic(
    panic_payload: Box<dyn std::any::Any + Send>,
    source_map: &SourceMap,
) -> AnalysisDiagnostic {
    let message = panic_payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic_payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("panic while analyzing Mermaid source");

    diagnostic(
        "merman.internal.panic",
        DiagnosticSeverity::Error,
        DiagnosticCategory::Internal,
        message,
        AnalysisStatus::Panic,
        source_map,
    )
}

fn recovered_editor_diagnostic(
    diagnostic: EditorSemanticDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
) -> AnalysisDiagnostic {
    let mut out = AnalysisDiagnostic::new(
        "merman.parse.recovered_editor_facts",
        DiagnosticSeverity::Warning,
        DiagnosticCategory::Parse,
        diagnostic.message,
    )
    .with_code(
        AnalysisStatus::ParseError.code(),
        AnalysisStatus::ParseError.code_name(),
    )
    .with_diagram_type(diagram_type);

    if let Some(span) = diagnostic
        .span
        .and_then(|span| source_map.span(span.start, span.end).ok())
    {
        out = out.with_span(span);
    }

    out
}

fn diagnostic(
    id: impl Into<String>,
    severity: DiagnosticSeverity,
    category: DiagnosticCategory,
    message: impl Into<String>,
    status: AnalysisStatus,
    source_map: &SourceMap,
) -> AnalysisDiagnostic {
    let mut diagnostic = AnalysisDiagnostic::new(id, severity, category, message)
        .with_code(status.code(), status.code_name());
    if let Ok(span) = source_map.whole_source_span() {
        diagnostic = diagnostic.with_span(span);
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::{AnalysisOptions, Analyzer};
    use crate::rules::AnalysisRuleConfig;
    use crate::{DiagnosticCategory, DiagnosticSeverity};

    #[test]
    fn analyze_state_parse_failure_surfaces_recovery_diagnostic() {
        let analyzer = Analyzer::new();
        let source = "stateDiagram-v2\nIdle --> Running\nRunning -->";
        let payload = analyzer.analyze(source);

        assert!(!payload.valid);
        assert_eq!(payload.summary.errors, 1);
        assert_eq!(payload.summary.warnings, 1);

        let parse_error = payload
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == "merman.parse.diagram_parse")
            .expect("parse error diagnostic");
        assert_eq!(parse_error.severity, DiagnosticSeverity::Error);
        assert_eq!(parse_error.category, DiagnosticCategory::Parse);
        assert_eq!(parse_error.diagram_type.as_deref(), Some("stateDiagram"));

        let recovery = payload
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == "merman.parse.recovered_editor_facts")
            .expect("recovery diagnostic");
        assert_eq!(recovery.severity, DiagnosticSeverity::Warning);
        assert_eq!(recovery.category, DiagnosticCategory::Parse);
        assert_eq!(recovery.diagram_type.as_deref(), Some("stateDiagram"));
        assert!(
            recovery
                .message
                .contains("state parser recovered after parse error")
        );
        assert!(recovery.span.is_some());
    }

    #[test]
    fn analyze_init_directive_alias_emits_safe_fix() {
        let analyzer = Analyzer::new();
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let payload = analyzer.analyze(source);

        assert!(payload.valid);
        assert_eq!(payload.summary.hints, 1);
        let diagnostic = payload
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == "merman.config.prefer_init_directive")
            .expect("init directive alias diagnostic");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
        assert_eq!(diagnostic.category, DiagnosticCategory::Config);
        let span = diagnostic.span.as_ref().expect("keyword span");
        assert_eq!(&source[span.byte_start..span.byte_end], "initialize");
        assert_eq!(diagnostic.fixes.len(), 1);
        assert_eq!(diagnostic.fixes[0].edits[0].replacement, "init");
    }

    #[test]
    fn analysis_rule_config_can_disable_source_lints() {
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_disabled(crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID),
            ),
        );
        let payload =
            analyzer.analyze("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n");

        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }

    #[test]
    fn analysis_rule_config_can_override_source_lint_severity() {
        let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default().with_rule_severity(
                crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
                DiagnosticSeverity::Warning,
            ),
        ));
        let payload =
            analyzer.analyze("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n");

        assert!(payload.valid);
        assert_eq!(payload.summary.hints, 0);
        assert_eq!(payload.summary.warnings, 1);
        assert_eq!(
            payload.diagnostics[0].id,
            crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID
        );
    }
}
