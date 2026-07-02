use crate::rules::AnalysisRuleConfig;
use crate::rules::{
    DIAGRAM_PARSE_RULE_ID, INVALID_DIRECTIVE_JSON_RULE_ID, INVALID_FRONT_MATTER_YAML_RULE_ID,
    MALFORMED_FRONT_MATTER_RULE_ID, NO_DIAGRAM_RULE_ID, PANIC_RULE_ID,
    RECOVERED_EDITOR_FACTS_RULE_ID, RESOURCE_LIMIT_RULE_ID, UNSUPPORTED_DIAGRAM_RULE_ID,
    internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{
    AnalysisDiagnostic, AnalysisPayload, AnalysisResult, AnalysisStatus, AnalysisSyntaxFacts,
    AnalyzedDiagram, DocumentDiagram, FenceTextIndex, SourceDescriptor, SourceMap,
};
use merman_core::{
    EditorSemanticDiagnostic, EditorSemanticDiagnosticKind, Engine, Error as CoreError,
    MermaidConfig, ParseDiagnostic, ParseDiagnosticSpanKind, ParseOptions,
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

    pub fn analyze_result(&self, source: &str) -> AnalysisResult {
        let source_map = SourceMap::new(source);
        let diagram = crate::document::whole_document_diagram(source, &self.options.source);
        let analyzed = self.analyze_diagram(&diagram);
        AnalysisResult::new(
            self.options.source.clone(),
            source_map,
            analyzed.diagnostics.clone(),
            vec![analyzed],
        )
    }

    pub fn analyze(&self, source: &str) -> AnalysisPayload {
        self.analyze_result(source).into_payload()
    }

    pub fn analyze_json(&self, source: &str) -> Result<Vec<u8>, serde_json::Error> {
        self.analyze(source).to_json_bytes()
    }

    pub(crate) fn analyze_diagram(&self, diagram: &DocumentDiagram) -> AnalyzedDiagram {
        let local = self.analyze_local(&diagram.text);
        AnalyzedDiagram::from_document_diagram(diagram, local.diagnostics, local.syntax)
    }

    fn analyze_local(&self, source: &str) -> LocalAnalysis {
        let source_map = SourceMap::new(source);

        if source.trim().is_empty() {
            let diagnostics = no_diagram_diagnostic(&source_map, &self.options.rule_config)
                .into_iter()
                .collect();
            return LocalAnalysis::text_scan(source, None, diagnostics);
        }

        if let Some(limit) = self.options.max_source_bytes
            && source.len() > limit
            && let Some(diagnostic) =
                source_limit_diagnostic(source.len(), limit, &source_map, &self.options.rule_config)
        {
            return LocalAnalysis::text_scan(source, None, vec![diagnostic]);
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
                LocalAnalysis::text_scan(source, None, diagnostics)
            }
            Ok(parse_result) => match parse_result {
                Ok(Some(parsed)) => {
                    let diagram_type = parsed.meta.diagram_type;
                    let editor_projection =
                        self.editor_facts_projection(source, &diagram_type, &source_map);
                    let mut diagnostics = source_lints;
                    diagnostics.extend(crate::rules::semantic_warning_diagnostics(
                        &diagram_type,
                        &parsed.model,
                        &source_map,
                        &self.options.rule_config,
                    ));
                    diagnostics.extend(
                        editor_projection
                            .diagnostics
                            .into_iter()
                            .map(|recovery| recovery.diagnostic),
                    );
                    LocalAnalysis {
                        diagnostics,
                        syntax: AnalysisSyntaxFacts::new(
                            Some(diagram_type),
                            editor_projection.text_index,
                        ),
                    }
                }
                Ok(None) => LocalAnalysis::text_scan(source, None, source_lints),
                Err(error) => {
                    let core_diagnostic =
                        core_error_diagnostic(error, &source_map, &self.options.rule_config);
                    let mut diagnostics = source_lints;
                    if let Some(diagnostic) = core_diagnostic.diagnostic {
                        diagnostics.push(diagnostic);
                    }
                    let syntax = if let Some(diagram_type) = core_diagnostic.diagram_type {
                        let editor_projection =
                            self.editor_facts_projection(source, &diagram_type, &source_map);
                        merge_recovery_diagnostics(
                            &mut diagnostics,
                            editor_projection.diagnostics,
                            core_diagnostic.parse_location,
                        );
                        AnalysisSyntaxFacts::new(Some(diagram_type), editor_projection.text_index)
                    } else {
                        AnalysisSyntaxFacts::text_scan(source, None)
                    };
                    LocalAnalysis {
                        diagnostics,
                        syntax,
                    }
                }
            },
        }
    }

    fn editor_facts_projection(
        &self,
        source: &str,
        diagram_type: &str,
        source_map: &SourceMap,
    ) -> EditorFactsProjection {
        let facts_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine.parse_editor_semantic_facts_with_type_sync(
                diagram_type,
                source,
                self.options.parse,
            )
        }));

        match facts_result {
            Err(panic_payload) => {
                let diagnostics =
                    panic_diagnostic(panic_payload, source_map, &self.options.rule_config)
                        .map(AnalysisRecoveryDiagnostic::plain)
                        .into_iter()
                        .collect();
                EditorFactsProjection::text_scan(source, Some(diagram_type), diagnostics)
            }
            Ok(Ok(Some(facts))) => {
                let diagnostics = facts
                    .diagnostics
                    .iter()
                    .cloned()
                    .filter_map(|diagnostic| {
                        recovered_editor_diagnostic(
                            diagnostic,
                            diagram_type,
                            source_map,
                            &self.options.rule_config,
                        )
                    })
                    .collect();
                EditorFactsProjection {
                    text_index: FenceTextIndex::from_core_facts(facts),
                    diagnostics,
                }
            }
            Ok(Ok(None) | Err(_)) => {
                EditorFactsProjection::text_scan(source, Some(diagram_type), Vec::new())
            }
        }
    }
}

#[derive(Debug, Clone)]
struct LocalAnalysis {
    diagnostics: Vec<AnalysisDiagnostic>,
    syntax: AnalysisSyntaxFacts,
}

impl LocalAnalysis {
    fn text_scan(
        source: &str,
        diagram_type: Option<String>,
        diagnostics: Vec<AnalysisDiagnostic>,
    ) -> Self {
        Self {
            diagnostics,
            syntax: AnalysisSyntaxFacts::text_scan(source, diagram_type),
        }
    }
}

#[derive(Debug, Clone)]
struct EditorFactsProjection {
    text_index: FenceTextIndex,
    diagnostics: Vec<AnalysisRecoveryDiagnostic>,
}

impl EditorFactsProjection {
    fn text_scan(
        source: &str,
        diagram_type: Option<&str>,
        diagnostics: Vec<AnalysisRecoveryDiagnostic>,
    ) -> Self {
        Self {
            text_index: FenceTextIndex::from_text(source, diagram_type),
            diagnostics,
        }
    }
}

#[derive(Debug, Clone)]
struct AnalysisRecoveryDiagnostic {
    diagnostic: AnalysisDiagnostic,
    kind: Option<EditorSemanticDiagnosticKind>,
}

impl AnalysisRecoveryDiagnostic {
    fn parser_backed(diagnostic: AnalysisDiagnostic, kind: EditorSemanticDiagnosticKind) -> Self {
        Self {
            diagnostic,
            kind: Some(kind),
        }
    }

    fn plain(diagnostic: AnalysisDiagnostic) -> Self {
        Self {
            diagnostic,
            kind: None,
        }
    }
}

fn merge_recovery_diagnostics(
    diagnostics: &mut Vec<AnalysisDiagnostic>,
    recovery_diagnostics: Vec<AnalysisRecoveryDiagnostic>,
    primary_parse_location: Option<ParseDiagnosticLocation>,
) {
    for recovery in recovery_diagnostics {
        if merge_duplicate_parse_recovery_diagnostic(diagnostics, &recovery, primary_parse_location)
        {
            continue;
        }
        diagnostics.push(recovery.diagnostic);
    }
}

fn merge_duplicate_parse_recovery_diagnostic(
    diagnostics: &mut [AnalysisDiagnostic],
    recovery: &AnalysisRecoveryDiagnostic,
    primary_parse_location: Option<ParseDiagnosticLocation>,
) -> bool {
    if recovery.kind != Some(EditorSemanticDiagnosticKind::ParserRecovery) {
        return false;
    }

    let Some(primary) = diagnostics.iter_mut().find(|diagnostic| {
        is_same_parse_recovery_problem(diagnostic, &recovery.diagnostic, primary_parse_location)
    }) else {
        return false;
    };

    if is_better_primary_parse_span(primary, &recovery.diagnostic) {
        if let Some(previous_span) = primary.span.clone() {
            primary.related.push(crate::DiagnosticRelated {
                message: "Parser reported this original parse location before recovery refinement."
                    .to_string(),
                span: Some(previous_span),
            });
        }
        primary.span = recovery.diagnostic.span.clone();
    }
    primary.related.push(crate::DiagnosticRelated {
        message: "Parser recovery produced the same syntax problem while preserving editor facts."
            .to_string(),
        span: recovery.diagnostic.span.clone(),
    });
    true
}

fn is_same_parse_recovery_problem(
    primary: &AnalysisDiagnostic,
    recovery: &AnalysisDiagnostic,
    primary_parse_location: Option<ParseDiagnosticLocation>,
) -> bool {
    if primary.id != DIAGRAM_PARSE_RULE_ID || recovery.id != RECOVERED_EDITOR_FACTS_RULE_ID {
        return false;
    }
    if primary.diagram_type != recovery.diagram_type {
        return false;
    }
    spans_describe_same_problem(primary.span.as_ref(), recovery.span.as_ref())
        || primary_parse_location == Some(ParseDiagnosticLocation::Fallback)
}

fn is_better_primary_parse_span(
    primary: &AnalysisDiagnostic,
    recovery: &AnalysisDiagnostic,
) -> bool {
    let Some(recovery_span) = recovery.span.as_ref() else {
        return false;
    };
    if recovery_span.byte_start == recovery_span.byte_end {
        return false;
    }
    match primary.span.as_ref() {
        None => true,
        Some(primary_span) if primary_span.byte_start == primary_span.byte_end => true,
        Some(primary_span)
            if primary_span.byte_start == recovery_span.byte_start
                && primary_span.byte_end == recovery_span.byte_end =>
        {
            false
        }
        Some(primary_span) => {
            let primary_len = primary_span
                .byte_end
                .saturating_sub(primary_span.byte_start);
            let recovery_len = recovery_span
                .byte_end
                .saturating_sub(recovery_span.byte_start);
            recovery_len > 0 && recovery_len < primary_len
        }
    }
}

fn spans_describe_same_problem(
    primary: Option<&crate::DiagnosticSpan>,
    recovery: Option<&crate::DiagnosticSpan>,
) -> bool {
    match (primary, recovery) {
        (None, _) | (_, None) => true,
        (Some(primary), Some(recovery)) => {
            primary.byte_start == recovery.byte_start
                || primary.byte_end == recovery.byte_end
                || spans_overlap(primary, recovery)
                || point_touches_span(primary, recovery)
                || point_touches_span(recovery, primary)
        }
    }
}

fn spans_overlap(left: &crate::DiagnosticSpan, right: &crate::DiagnosticSpan) -> bool {
    left.byte_start < right.byte_end && right.byte_start < left.byte_end
}

fn point_touches_span(point: &crate::DiagnosticSpan, span: &crate::DiagnosticSpan) -> bool {
    point.byte_start == point.byte_end
        && span.byte_start <= point.byte_start
        && point.byte_start <= span.byte_end
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

#[derive(Debug)]
struct CoreErrorDiagnostic {
    diagnostic: Option<AnalysisDiagnostic>,
    diagram_type: Option<String>,
    parse_location: Option<ParseDiagnosticLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseDiagnosticLocation {
    Precise,
    Fallback,
}

struct ParseDiagnosticProjection {
    diagnostic: AnalysisDiagnostic,
    location: ParseDiagnosticLocation,
}

fn core_error_diagnostic(
    error: CoreError,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> CoreErrorDiagnostic {
    match error {
        CoreError::DetectType(_) => CoreErrorDiagnostic {
            diagnostic: no_diagram_diagnostic(source_map, rule_config),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::UnsupportedDiagram { diagram_type } => CoreErrorDiagnostic {
            diagnostic: rule_diagnostic(
                UNSUPPORTED_DIAGRAM_RULE_ID,
                AnalysisStatus::UnsupportedFormat,
                format!("unsupported diagram type: {diagram_type}"),
                source_map,
                rule_config,
            )
            .map(|diagnostic| diagnostic.with_diagram_type(diagram_type.clone())),
            diagram_type: Some(diagram_type),
            parse_location: None,
        },
        CoreError::DiagramParse {
            diagram_type,
            diagnostic,
        } => {
            let (diagnostic, parse_location) =
                match parse_diagnostic(diagnostic, &diagram_type, source_map, rule_config) {
                    Some(projection) => (Some(projection.diagnostic), Some(projection.location)),
                    None => (None, None),
                };
            CoreErrorDiagnostic {
                diagnostic,
                diagram_type: Some(diagram_type),
                parse_location,
            }
        }
        CoreError::MalformedFrontMatter => CoreErrorDiagnostic {
            diagnostic: rule_diagnostic(
                MALFORMED_FRONT_MATTER_RULE_ID,
                AnalysisStatus::ParseError,
                CoreError::MalformedFrontMatter.to_string(),
                source_map,
                rule_config,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::InvalidDirectiveJson { message } => CoreErrorDiagnostic {
            diagnostic: rule_diagnostic(
                INVALID_DIRECTIVE_JSON_RULE_ID,
                AnalysisStatus::ParseError,
                format!("invalid directive JSON: {message}"),
                source_map,
                rule_config,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::InvalidFrontMatterYaml { message } => CoreErrorDiagnostic {
            diagnostic: rule_diagnostic(
                INVALID_FRONT_MATTER_YAML_RULE_ID,
                AnalysisStatus::ParseError,
                format!("invalid YAML front-matter: {message}"),
                source_map,
                rule_config,
            ),
            diagram_type: None,
            parse_location: None,
        },
    }
}

fn parse_diagnostic(
    diagnostic: ParseDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<ParseDiagnosticProjection> {
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
    let location;

    if let Some(span) = diagnostic
        .span()
        .and_then(|span| source_map.span(span.start, span.end).ok())
    {
        match diagnostic.span_kind() {
            ParseDiagnosticSpanKind::Exact | ParseDiagnosticSpanKind::InsertionPoint => {
                out = out.with_span(span);
                location = ParseDiagnosticLocation::Precise;
            }
            ParseDiagnosticSpanKind::Fallback => {
                out.related.push(crate::DiagnosticRelated {
                    message: "Parser reported a fallback location for this syntax error."
                        .to_string(),
                    span: Some(span.clone()),
                });
                out = out.with_span(span);
                location = ParseDiagnosticLocation::Fallback;
            }
        }
    } else if let Ok(span) = source_map.whole_source_span() {
        out.related.push(crate::DiagnosticRelated {
            message: "Parser did not report a precise source location for this syntax error."
                .to_string(),
            span: Some(span.clone()),
        });
        out = out.with_span(span);
        location = ParseDiagnosticLocation::Fallback;
    } else {
        location = ParseDiagnosticLocation::Fallback;
    }

    Some(ParseDiagnosticProjection {
        diagnostic: out,
        location,
    })
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
) -> Option<AnalysisRecoveryDiagnostic> {
    let kind = diagnostic.kind;
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

    Some(AnalysisRecoveryDiagnostic::parser_backed(out, kind))
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
        assert!(parse_error.related.iter().any(|related| {
            related
                .message
                .contains("Parser recovery produced the same syntax problem")
        }));
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
    fn fallback_recovery_merge_uses_structured_location_metadata() {
        let source_map = SourceMap::new("flowchart TD\nA[unterminated");
        let span = source_map.whole_source_span().unwrap();
        let primary = super::rule_diagnostic_without_default_span(
            crate::rules::DIAGRAM_PARSE_RULE_ID,
            AnalysisStatus::ParseError,
            "primary parser message",
            &AnalysisRuleConfig::default(),
        )
        .unwrap()
        .with_diagram_type("flowchart-v2")
        .with_span(span.clone());
        let recovery = super::rule_diagnostic_without_default_span(
            crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID,
            AnalysisStatus::ParseError,
            "recovered parser message",
            &AnalysisRuleConfig::default(),
        )
        .unwrap()
        .with_diagram_type("flowchart-v2")
        .with_span(span);
        let mut diagnostics = vec![primary];

        super::merge_recovery_diagnostics(
            &mut diagnostics,
            vec![super::AnalysisRecoveryDiagnostic::parser_backed(
                recovery,
                merman_core::EditorSemanticDiagnosticKind::ParserRecovery,
            )],
            Some(super::ParseDiagnosticLocation::Fallback),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "primary parser message");
        assert!(diagnostics[0].related.iter().any(|related| {
            related
                .message
                .contains("Parser recovery produced the same syntax problem")
        }));
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
