use crate::rules::AnalysisRuleConfig;
use crate::rules::{
    DIAGRAM_PARSE_RULE_ID, FLOWCHART_FACTS_PROJECTION_RULE_ID, INVALID_DIRECTIVE_JSON_RULE_ID,
    INVALID_FRONT_MATTER_YAML_RULE_ID, MALFORMED_FRONT_MATTER_RULE_ID, NO_DIAGRAM_RULE_ID,
    PANIC_RULE_ID, RECOVERED_EDITOR_FACTS_RULE_ID, RESOURCE_LIMIT_RULE_ID,
    UNSUPPORTED_DIAGRAM_RULE_ID, internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{
    AnalysisDiagnostic, AnalysisFlowchartFacts, AnalysisPayload, AnalysisResult, AnalysisStatus,
    AnalysisSyntaxFacts, AnalyzedDiagram, DocumentDiagram, FenceTextIndex, SourceDescriptor,
    SourceMap,
};
use merman_core::{
    EditorSemanticDiagnostic, EditorSemanticDiagnosticKind, EditorSemanticFacts, Engine,
    Error as CoreError, MermaidConfig, ParseDiagnostic, ParseDiagnosticSpanKind, ParseOptions,
    ParsedDiagram,
};
use std::panic::{self, AssertUnwindSafe};

const NO_DIAGRAM_MESSAGE: &str = "no Mermaid diagram detected";

#[derive(Debug, Clone, PartialEq)]
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

    pub fn snapshot_affecting_eq(&self, other: &Self) -> bool {
        self.parse == other.parse
            && self.site_config == other.site_config
            && self.fixed_today == other.fixed_today
            && self.fixed_local_offset_minutes == other.fixed_local_offset_minutes
            && self.max_source_bytes == other.max_source_bytes
            && self.source == other.source
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

    pub fn options(&self) -> &AnalysisOptions {
        &self.options
    }

    pub fn analyze_result(&self, source: &str) -> AnalysisResult {
        if let Some(result) = self.source_limit_result(source, self.options.source.clone()) {
            return result;
        }

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
        if let Some(result) = self.source_limit_result(source, self.options.source.clone()) {
            return result.into_payload();
        }

        AnalysisPayload::new(
            self.options.source.clone(),
            self.analyze_source_diagnostics(source),
        )
    }

    pub fn analyze_facts(&self, source: &str) -> crate::AnalysisFactsPayload {
        self.analyze_result(source).to_facts_payload()
    }

    pub fn analyze_json(&self, source: &str) -> Result<Vec<u8>, serde_json::Error> {
        self.analyze(source).to_json_bytes()
    }

    pub fn analyze_facts_json(&self, source: &str) -> Result<Vec<u8>, serde_json::Error> {
        self.analyze_facts(source).to_json_bytes()
    }

    pub(crate) fn analyze_diagram(&self, diagram: &DocumentDiagram) -> AnalyzedDiagram {
        let local = self.analyze_local(&diagram.text, AnalysisMode::RichFacts);
        AnalyzedDiagram::from_document_diagram(diagram, local.diagnostics, local.syntax)
    }

    pub(crate) fn analyze_diagram_diagnostics(
        &self,
        diagram: &DocumentDiagram,
    ) -> Vec<AnalysisDiagnostic> {
        self.analyze_source_diagnostics(&diagram.text)
    }

    pub(crate) fn analyze_source_diagnostics(&self, source: &str) -> Vec<AnalysisDiagnostic> {
        self.analyze_local(source, AnalysisMode::Diagnostics)
            .diagnostics
    }

    fn analyze_local(&self, source: &str, mode: AnalysisMode) -> LocalAnalysis {
        if let Some(diagnostics) = self.source_limit_diagnostics(source) {
            return LocalAnalysis::empty_syntax(diagnostics);
        }

        let source_map = SourceMap::new(source);

        if source.trim().is_empty() {
            let diagnostics = no_diagram_diagnostic(&source_map, &self.options.rule_config)
                .into_iter()
                .collect();
            return mode.text_scan_or_empty(source, None, diagnostics);
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
                mode.text_scan_or_empty(source, None, diagnostics)
            }
            Ok(parse_result) => match parse_result {
                Ok(Some(parsed)) => {
                    self.analyze_parsed_diagram(source, &source_map, parsed, source_lints, mode)
                }
                Ok(None) => mode.text_scan_or_empty(source, None, source_lints),
                Err(error) => {
                    self.analyze_parse_error(source, &source_map, source_lints, error, mode)
                }
            },
        }
    }

    fn analyze_parsed_diagram(
        &self,
        source: &str,
        source_map: &SourceMap,
        parsed: ParsedDiagram,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let diagram_type = parsed.meta.diagram_type;
        diagnostics.extend(crate::rules::semantic_warning_diagnostics(
            &diagram_type,
            &parsed.model,
            source_map,
            &self.options.rule_config,
        ));

        match mode {
            AnalysisMode::Diagnostics => {
                LocalAnalysis::empty_syntax_with_type(Some(diagram_type), diagnostics)
            }
            AnalysisMode::RichFacts => {
                let flowchart_projection =
                    self.flowchart_facts_projection(&parsed.model, &diagram_type, source_map);
                diagnostics.extend(flowchart_projection.diagnostics);
                let editor_projection =
                    self.editor_facts_projection(source, &diagram_type, source_map, mode);
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
                    )
                    .with_flowchart(flowchart_projection.facts),
                }
            }
        }
    }

    fn analyze_parse_error(
        &self,
        source: &str,
        source_map: &SourceMap,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        error: CoreError,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let core_diagnostic = core_error_diagnostic(error, source_map, &self.options.rule_config);
        if let Some(diagnostic) = core_diagnostic.diagnostic {
            diagnostics.push(diagnostic);
        }
        let syntax = match core_diagnostic.diagram_type {
            Some(diagram_type) => {
                let editor_projection =
                    self.editor_facts_projection(source, &diagram_type, source_map, mode);
                merge_recovery_diagnostics(
                    &mut diagnostics,
                    editor_projection.diagnostics,
                    core_diagnostic.parse_location,
                );
                AnalysisSyntaxFacts::new(Some(diagram_type), editor_projection.text_index)
            }
            None if mode == AnalysisMode::Diagnostics => {
                AnalysisSyntaxFacts::new(None, FenceTextIndex::default())
            }
            None => AnalysisSyntaxFacts::text_scan(source, None),
        };
        LocalAnalysis {
            diagnostics,
            syntax,
        }
    }

    fn editor_facts_projection(
        &self,
        source: &str,
        diagram_type: &str,
        source_map: &SourceMap,
        mode: AnalysisMode,
    ) -> EditorFactsProjection {
        match self.parse_editor_semantic_facts(source, diagram_type, source_map) {
            Err(diagnostics) => {
                EditorFactsProjection::fallback(source, Some(diagram_type), diagnostics, mode)
            }
            Ok(Some(facts)) => {
                let diagnostics = editor_recovery_diagnostics(
                    facts.diagnostics.iter().cloned(),
                    diagram_type,
                    source_map,
                    &self.options.rule_config,
                );
                EditorFactsProjection {
                    text_index: match mode {
                        AnalysisMode::Diagnostics => FenceTextIndex::default(),
                        AnalysisMode::RichFacts => FenceTextIndex::from_core_facts(facts),
                    },
                    diagnostics,
                }
            }
            Ok(None) => {
                EditorFactsProjection::fallback(source, Some(diagram_type), Vec::new(), mode)
            }
        }
    }

    fn flowchart_facts_projection(
        &self,
        model: &serde_json::Value,
        diagram_type: &str,
        source_map: &SourceMap,
    ) -> FlowchartFactsProjection {
        match AnalysisFlowchartFacts::try_from_model(model) {
            Ok(facts) => FlowchartFactsProjection {
                facts,
                diagnostics: Vec::new(),
            },
            Err(error) => FlowchartFactsProjection {
                facts: None,
                diagnostics: flowchart_facts_projection_diagnostic(
                    error,
                    diagram_type,
                    source_map,
                    &self.options.rule_config,
                )
                .into_iter()
                .collect(),
            },
        }
    }

    fn parse_editor_semantic_facts(
        &self,
        source: &str,
        diagram_type: &str,
        source_map: &SourceMap,
    ) -> Result<Option<EditorSemanticFacts>, Vec<AnalysisRecoveryDiagnostic>> {
        let facts_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine.parse_editor_semantic_facts_with_type_sync(
                diagram_type,
                source,
                self.options.parse,
            )
        }));

        match facts_result {
            Err(panic_payload) => {
                Err(
                    panic_diagnostic(panic_payload, source_map, &self.options.rule_config)
                        .map(AnalysisRecoveryDiagnostic::plain)
                        .into_iter()
                        .collect(),
                )
            }
            Ok(Ok(facts)) => Ok(facts),
            Ok(Err(CoreError::UnsupportedDiagram { .. })) => Ok(None),
            Ok(Err(error)) => {
                Err(
                    core_error_diagnostic(error, source_map, &self.options.rule_config)
                        .diagnostic
                        .map(AnalysisRecoveryDiagnostic::plain)
                        .into_iter()
                        .collect(),
                )
            }
        }
    }

    pub(crate) fn source_limit_result(
        &self,
        source: &str,
        descriptor: SourceDescriptor,
    ) -> Option<AnalysisResult> {
        let diagnostics = self.source_limit_diagnostics(source)?;
        Some(AnalysisResult::new(
            descriptor,
            SourceMap::new(""),
            diagnostics,
            Vec::new(),
        ))
    }

    fn source_limit_diagnostics(&self, source: &str) -> Option<Vec<AnalysisDiagnostic>> {
        let limit = self.options.max_source_bytes?;
        if source.len() <= limit {
            return None;
        }

        let source_map = SourceMap::new("");
        let diagnostics =
            source_limit_diagnostic(source.len(), limit, &source_map, &self.options.rule_config)
                .map(|mut diagnostic| {
                    diagnostic.span = Some(crate::source_map::whole_text_span_without_source_copy(
                        source,
                    ));
                    diagnostic
                })
                .into_iter()
                .collect();
        Some(diagnostics)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisMode {
    Diagnostics,
    RichFacts,
}

impl AnalysisMode {
    fn text_scan_or_empty(
        self,
        source: &str,
        diagram_type: Option<String>,
        diagnostics: Vec<AnalysisDiagnostic>,
    ) -> LocalAnalysis {
        match self {
            Self::Diagnostics => LocalAnalysis::empty_syntax_with_type(diagram_type, diagnostics),
            Self::RichFacts => LocalAnalysis::text_scan(source, diagram_type, diagnostics),
        }
    }
}

#[derive(Debug, Clone)]
struct LocalAnalysis {
    diagnostics: Vec<AnalysisDiagnostic>,
    syntax: AnalysisSyntaxFacts,
}

impl LocalAnalysis {
    fn empty_syntax(diagnostics: Vec<AnalysisDiagnostic>) -> Self {
        Self::empty_syntax_with_type(None, diagnostics)
    }

    fn empty_syntax_with_type(
        diagram_type: Option<String>,
        diagnostics: Vec<AnalysisDiagnostic>,
    ) -> Self {
        Self {
            diagnostics,
            syntax: AnalysisSyntaxFacts::new(diagram_type, FenceTextIndex::default()),
        }
    }

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
    fn fallback(
        source: &str,
        diagram_type: Option<&str>,
        diagnostics: Vec<AnalysisRecoveryDiagnostic>,
        mode: AnalysisMode,
    ) -> Self {
        Self {
            text_index: match mode {
                AnalysisMode::Diagnostics => FenceTextIndex::default(),
                AnalysisMode::RichFacts => FenceTextIndex::from_text(source, diagram_type),
            },
            diagnostics,
        }
    }
}

#[derive(Debug, Clone)]
struct FlowchartFactsProjection {
    facts: Option<AnalysisFlowchartFacts>,
    diagnostics: Vec<AnalysisDiagnostic>,
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

pub(crate) fn source_limit_diagnostic(
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

fn flowchart_facts_projection_diagnostic(
    error: impl std::fmt::Display,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    rule_diagnostic(
        FLOWCHART_FACTS_PROJECTION_RULE_ID,
        AnalysisStatus::InternalError,
        format!("failed to project flowchart facts from parser model: {error}"),
        source_map,
        rule_config,
    )
    .map(|diagnostic| diagnostic.with_diagram_type(diagram_type))
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

fn editor_recovery_diagnostics(
    diagnostics: impl IntoIterator<Item = EditorSemanticDiagnostic>,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &AnalysisRuleConfig,
) -> Vec<AnalysisRecoveryDiagnostic> {
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| {
            recovered_editor_diagnostic(diagnostic, diagram_type, source_map, rule_config)
        })
        .collect()
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
mod tests;
