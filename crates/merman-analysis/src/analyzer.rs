use crate::rules::AnalysisRuleConfig;
use crate::rules::{
    DIAGRAM_PARSE_RULE_ID, INVALID_DIRECTIVE_JSON_RULE_ID, INVALID_FRONT_MATTER_YAML_RULE_ID,
    MALFORMED_FRONT_MATTER_RULE_ID, NO_DIAGRAM_RULE_ID, PANIC_RULE_ID,
    RECOVERED_EDITOR_FACTS_RULE_ID, RESOURCE_LIMIT_RULE_ID, UNSUPPORTED_DIAGRAM_RULE_ID,
    internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{AnalysisDiagnostic, AnalysisPayload, AnalysisStatus, SourceDescriptor, SourceMap};
use merman_core::{
    EditorSemanticDiagnostic, Engine, Error as CoreError, MermaidConfig, ParseDiagnostic,
    ParseDiagnosticSpanKind, ParseOptions,
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
            let diagnostics = no_diagram_diagnostic(&source_map, &self.options.rule_config)
                .into_iter()
                .collect();
            return self.payload(diagnostics);
        }

        if let Some(limit) = self.options.max_source_bytes
            && source.len() > limit
            && let Some(diagnostic) =
                source_limit_diagnostic(source.len(), limit, &source_map, &self.options.rule_config)
        {
            return self.payload(vec![diagnostic]);
        }

        let source_lints =
            crate::rules::source_lint_diagnostics(source, &source_map, &self.options.rule_config);

        let parse_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine.parse_diagram_sync(source, self.options.parse)
        }));

        match parse_result {
            Err(panic_payload) => {
                let mut diagnostics = source_lints;
                if let Some(diagnostic) =
                    panic_diagnostic(panic_payload, &source_map, &self.options.rule_config)
                {
                    diagnostics.push(diagnostic);
                }
                self.payload(diagnostics)
            }
            Ok(parse_result) => match parse_result {
                Ok(Some(parsed)) => {
                    let mut diagnostics = source_lints;
                    diagnostics.extend(crate::rules::semantic_warning_diagnostics(
                        &parsed.meta.diagram_type,
                        &parsed.model,
                        &source_map,
                        &self.options.rule_config,
                    ));
                    diagnostics.extend(self.editor_recovery_diagnostics(
                        source,
                        &parsed.meta.diagram_type,
                        &source_map,
                    ));
                    self.payload(diagnostics)
                }
                Ok(None) => self.payload(source_lints),
                Err(error) => {
                    let (core_diagnostic, diagram_type) =
                        core_error_diagnostic(error, &source_map, &self.options.rule_config);
                    let mut diagnostics = source_lints;
                    if let Some(diagnostic) = core_diagnostic {
                        diagnostics.push(diagnostic);
                    }
                    if let Some(diagram_type) = diagram_type {
                        let recovery_diagnostics =
                            self.editor_recovery_diagnostics(source, &diagram_type, &source_map);
                        merge_recovery_diagnostics(&mut diagnostics, recovery_diagnostics);
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
            Err(panic_payload) => {
                panic_diagnostic(panic_payload, source_map, &self.options.rule_config)
                    .into_iter()
                    .collect()
            }
            Ok(Ok(Some(facts))) => facts
                .diagnostics
                .into_iter()
                .filter_map(|diagnostic| {
                    recovered_editor_diagnostic(
                        diagnostic,
                        diagram_type,
                        source_map,
                        &self.options.rule_config,
                    )
                })
                .collect(),
            Ok(Ok(None) | Err(_)) => Vec::new(),
        }
    }
}

fn merge_recovery_diagnostics(
    diagnostics: &mut Vec<AnalysisDiagnostic>,
    recovery_diagnostics: Vec<AnalysisDiagnostic>,
) {
    for recovery in recovery_diagnostics {
        if merge_duplicate_parse_recovery_diagnostic(diagnostics, &recovery) {
            continue;
        }
        diagnostics.push(recovery);
    }
}

fn merge_duplicate_parse_recovery_diagnostic(
    diagnostics: &mut [AnalysisDiagnostic],
    recovery: &AnalysisDiagnostic,
) -> bool {
    if recovery.id != RECOVERED_EDITOR_FACTS_RULE_ID {
        return false;
    }

    let Some(detail) = recovered_parse_error_detail(&recovery.message) else {
        return false;
    };

    let Some(primary) = diagnostics
        .iter_mut()
        .find(|diagnostic| diagnostic.id == DIAGRAM_PARSE_RULE_ID && diagnostic.message == detail)
    else {
        return false;
    };

    if recovery
        .span
        .as_ref()
        .is_some_and(|span| span.byte_start < span.byte_end)
    {
        primary.span = recovery.span.clone();
    }
    true
}

fn recovered_parse_error_detail(message: &str) -> Option<&str> {
    message
        .split_once(" after parse error: ")
        .map(|(_, detail)| detail.trim())
        .filter(|detail| !detail.is_empty())
}

pub fn engine_from_options(options: &AnalysisOptions) -> Engine {
    let mut engine = Engine::new()
        .with_fixed_today(options.fixed_today)
        .with_fixed_local_offset_minutes(options.fixed_local_offset_minutes);

    if let Some(site_config) = options.site_config.clone() {
        engine = engine.with_site_config(site_config);
    }

    engine
}

fn no_diagram_diagnostic(
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    rule_diagnostic(
        NO_DIAGRAM_RULE_ID,
        AnalysisStatus::NoDiagram,
        NO_DIAGRAM_MESSAGE,
        source_map,
        rule_config,
    )
}

fn source_limit_diagnostic(
    source_len: usize,
    limit: usize,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    rule_diagnostic(
        RESOURCE_LIMIT_RULE_ID,
        AnalysisStatus::ResourceLimitExceeded,
        format!("source is {source_len} bytes, exceeding max_source_bytes {limit}"),
        source_map,
        rule_config,
    )
}

fn core_error_diagnostic(
    error: CoreError,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> (Option<AnalysisDiagnostic>, Option<String>) {
    match error {
        CoreError::DetectType(_) => (no_diagram_diagnostic(source_map, rule_config), None),
        CoreError::UnsupportedDiagram { diagram_type } => (
            rule_diagnostic(
                UNSUPPORTED_DIAGRAM_RULE_ID,
                AnalysisStatus::UnsupportedFormat,
                format!("unsupported diagram type: {diagram_type}"),
                source_map,
                rule_config,
            )
            .map(|diagnostic| diagnostic.with_diagram_type(diagram_type.clone())),
            Some(diagram_type),
        ),
        CoreError::DiagramParse {
            diagram_type,
            diagnostic,
        } => (
            parse_diagnostic(diagnostic, &diagram_type, source_map, rule_config),
            Some(diagram_type),
        ),
        CoreError::MalformedFrontMatter => (
            rule_diagnostic(
                MALFORMED_FRONT_MATTER_RULE_ID,
                AnalysisStatus::ParseError,
                CoreError::MalformedFrontMatter.to_string(),
                source_map,
                rule_config,
            ),
            None,
        ),
        CoreError::InvalidDirectiveJson { message } => (
            rule_diagnostic(
                INVALID_DIRECTIVE_JSON_RULE_ID,
                AnalysisStatus::ParseError,
                format!("invalid directive JSON: {message}"),
                source_map,
                rule_config,
            ),
            None,
        ),
        CoreError::InvalidFrontMatterYaml { message } => (
            rule_diagnostic(
                INVALID_FRONT_MATTER_YAML_RULE_ID,
                AnalysisStatus::ParseError,
                format!("invalid YAML front-matter: {message}"),
                source_map,
                rule_config,
            ),
            None,
        ),
    }
}

fn parse_diagnostic(
    diagnostic: ParseDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    let rule_id = diagnostic
        .code()
        .and_then(rule_descriptor)
        .map(|descriptor| descriptor.id)
        .unwrap_or(DIAGRAM_PARSE_RULE_ID);
    let mut out = rule_diagnostic_without_default_span(
        rule_id,
        AnalysisStatus::ParseError,
        diagnostic.message().to_string(),
        rule_config,
    )?
    .with_diagram_type(diagram_type);

    if let Some(span) = diagnostic
        .span()
        .and_then(|span| source_map.span(span.start, span.end).ok())
    {
        match diagnostic.span_kind() {
            ParseDiagnosticSpanKind::Exact | ParseDiagnosticSpanKind::InsertionPoint => {
                out = out.with_span(span);
            }
            ParseDiagnosticSpanKind::Fallback => {
                out.related.push(crate::DiagnosticRelated {
                    message: "Parser reported a fallback location for this syntax error."
                        .to_string(),
                    span: Some(span.clone()),
                });
                out = out.with_span(span);
            }
        }
    } else if let Ok(span) = source_map.whole_source_span() {
        out.related.push(crate::DiagnosticRelated {
            message: "Parser did not report a precise source location for this syntax error."
                .to_string(),
            span: Some(span.clone()),
        });
        out = out.with_span(span);
    }

    Some(out)
}

fn panic_diagnostic(
    panic_payload: Box<dyn std::any::Any + Send>,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    let message = panic_payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic_payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("panic while analyzing Mermaid source");

    rule_diagnostic(
        PANIC_RULE_ID,
        AnalysisStatus::Panic,
        message,
        source_map,
        rule_config,
    )
}

fn recovered_editor_diagnostic(
    diagnostic: EditorSemanticDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    let mut out = rule_diagnostic(
        RECOVERED_EDITOR_FACTS_RULE_ID,
        AnalysisStatus::ParseError,
        diagnostic.message,
        source_map,
        rule_config,
    )?
    .with_diagram_type(diagram_type);

    if let Some(span) = diagnostic
        .span
        .and_then(|span| source_map.span(span.start, span.end).ok())
    {
        out = out.with_span(span);
    }

    Some(out)
}

fn rule_diagnostic(
    rule_id: &'static str,
    status: AnalysisStatus,
    message: impl Into<String>,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    let message = message.into();
    let Some(descriptor) = rule_descriptor(rule_id) else {
        return Some(internal_rule_registry_gap_diagnostic(
            format!("unknown analysis rule id `{rule_id}` while emitting diagnostic: {message}"),
            source_map.whole_source_span().ok(),
        ));
    };

    if !rule_config.is_rule_enabled(descriptor) {
        return None;
    }

    let mut diagnostic =
        rule_diagnostic_without_default_span(rule_id, status, message, rule_config)?;
    if let Ok(span) = source_map.whole_source_span() {
        diagnostic = diagnostic.with_span(span);
    }
    Some(diagnostic)
}

fn rule_diagnostic_without_default_span(
    rule_id: &'static str,
    status: AnalysisStatus,
    message: impl Into<String>,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    let message = message.into();
    let descriptor = rule_descriptor(rule_id)?;

    if !rule_config.is_rule_enabled(descriptor) {
        return None;
    }

    Some(
        AnalysisDiagnostic::new(
            descriptor.id,
            rule_config.severity_for(descriptor),
            descriptor.category,
            message,
        )
        .with_code(status.code(), status.code_name()),
    )
}

#[cfg(test)]
mod tests {
    use super::{AnalysisOptions, Analyzer};
    use crate::rules::{AnalysisRuleConfig, AnalysisRuleProfile};
    use crate::{AnalysisStatus, DiagnosticCategory, DiagnosticSeverity, SourceMap};

    #[test]
    fn analyze_state_parse_failure_deduplicates_matching_recovery_diagnostic() {
        let analyzer = Analyzer::new();
        let source = "stateDiagram-v2\nIdle --> Running\nRunning -->";
        let payload = analyzer.analyze(source);

        assert!(!payload.valid);
        assert_eq!(payload.summary.errors, 1);
        assert_eq!(payload.summary.warnings, 0);
        assert_eq!(payload.diagnostics.len(), 1);

        let parse_error = payload
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == "merman.parse.diagram_parse")
            .expect("parse error diagnostic");
        assert_eq!(parse_error.severity, DiagnosticSeverity::Error);
        assert_eq!(parse_error.category, DiagnosticCategory::Parse);
        assert_eq!(parse_error.diagram_type.as_deref(), Some("stateDiagram"));
        assert!(
            payload
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.id != "merman.parse.recovered_editor_facts")
        );
    }

    #[test]
    fn analyze_flowchart_parse_failure_deduplicates_matching_recovery_diagnostic() {
        let analyzer = Analyzer::new();
        let source = "flowchart TD\nA[unterminated";
        let payload = analyzer.analyze(source);

        assert!(!payload.valid);
        assert_eq!(payload.summary.errors, 1);
        assert_eq!(payload.summary.warnings, 0);
        assert_eq!(payload.diagnostics.len(), 1);

        let diagnostic = &payload.diagnostics[0];
        assert_eq!(diagnostic.id, "merman.parse.diagram_parse");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
        assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
        assert_eq!(diagnostic.message, "Unterminated node label (missing `]`)");
    }

    #[test]
    fn analyze_init_directive_alias_emits_safe_fix() {
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_profile(AnalysisRuleProfile::Recommended)
                    .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID),
            ),
        );
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let payload = analyzer.analyze(source);

        assert!(payload.valid);
        assert_eq!(payload.summary.hints, 1);
        let diagnostic = payload
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
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
                    .with_profile(AnalysisRuleProfile::Recommended)
                    .with_rule_disabled(crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
                    .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID),
            ),
        );
        let payload =
            analyzer.analyze("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n");

        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }

    #[test]
    fn analysis_rule_config_can_disable_no_diagram_rule() {
        let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default().with_rule_disabled(crate::rules::NO_DIAGRAM_RULE_ID),
        ));
        let payload = analyzer.analyze("");

        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }

    #[test]
    fn analysis_rule_config_can_disable_resource_limit_rule() {
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default()
                .with_max_source_bytes(Some(8))
                .with_rule_config(
                    AnalysisRuleConfig::default()
                        .with_rule_disabled(crate::rules::RESOURCE_LIMIT_RULE_ID),
                ),
        );
        let payload = analyzer.analyze("flowchart TD\nA-->B\n");

        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }

    #[test]
    fn analysis_rule_config_can_override_resource_limit_severity() {
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default()
                .with_max_source_bytes(Some(8))
                .with_rule_config(AnalysisRuleConfig::default().with_rule_severity(
                    crate::rules::RESOURCE_LIMIT_RULE_ID,
                    DiagnosticSeverity::Hint,
                )),
        );
        let payload = analyzer.analyze("flowchart TD\nA-->B\n");

        assert!(payload.valid);
        assert_eq!(payload.summary.hints, 1);
        assert_eq!(payload.summary.errors, 0);
        assert_eq!(
            payload.diagnostics[0].id,
            crate::rules::RESOURCE_LIMIT_RULE_ID
        );
        assert_eq!(payload.diagnostics[0].severity, DiagnosticSeverity::Hint);
    }

    #[test]
    fn analysis_rule_config_can_override_source_lint_severity() {
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_profile(AnalysisRuleProfile::Recommended)
                    .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                    .with_rule_severity(
                        crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
                        DiagnosticSeverity::Warning,
                    ),
            ),
        );
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

    #[test]
    fn analysis_rule_config_can_disable_git_graph_warning_rules() {
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_disabled(crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID),
            ),
        );
        let payload = analyzer
            .analyze("gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n");

        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }

    #[test]
    fn analysis_rule_config_can_override_git_graph_warning_severity() {
        let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default().with_rule_severity(
                crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID,
                DiagnosticSeverity::Hint,
            ),
        ));
        let payload = analyzer
            .analyze("gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n");

        assert!(payload.valid);
        assert_eq!(payload.summary.hints, 1);
        assert_eq!(
            payload
                .diagnostics
                .iter()
                .filter(
                    |diagnostic| diagnostic.id == crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID
                )
                .count(),
            1
        );
        assert!(
            payload
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Hint)
        );
    }

    #[test]
    fn analysis_rule_registry_gap_surfaces_as_internal_error() {
        let source_map = SourceMap::new("flowchart TD\nA-->B\n");
        let diagnostic = super::rule_diagnostic(
            "merman.unknown.rule",
            AnalysisStatus::Panic,
            "rule ids must be registered",
            &source_map,
            &AnalysisRuleConfig::default(),
        )
        .expect("internal registry gap diagnostic");

        assert_eq!(
            diagnostic.id,
            crate::rules::INTERNAL_RULE_REGISTRY_GAP_RULE_ID
        );
        assert_eq!(diagnostic.category, DiagnosticCategory::Internal);
        assert_eq!(diagnostic.code, Some(AnalysisStatus::InternalError.code()));
        assert!(
            diagnostic
                .message
                .contains("unknown analysis rule id `merman.unknown.rule`")
        );
    }
}
