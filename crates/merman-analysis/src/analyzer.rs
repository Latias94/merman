use crate::diagnostic_projection::{
    core_error_diagnostic, flowchart_facts_projection_diagnostic, no_diagram_diagnostic,
    panic_diagnostic,
};
use crate::recovery::{
    AnalysisRecoveryDiagnostic, editor_recovery_diagnostics, merge_recovery_diagnostics,
};
use crate::rules::AnalysisRuleConfig;
use crate::{
    AnalysisDiagnostic, AnalysisFlowchartFacts, AnalysisPayload, AnalysisResult,
    AnalysisSyntaxFacts, AnalyzedDiagram, DocumentDiagram, FenceTextIndex, SourceDescriptor,
    SourceMap,
};
use merman_core::{
    DiagramParseOutcome, EditorSemanticFacts, Engine, Error as CoreError, MermaidConfig,
    ParseMetadata, ParsedDiagram, ParsedEditorFacts,
};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisOptions {
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
        self.site_config == other.site_config
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

        let source_text = Arc::<str>::from(source);
        let source_map = SourceMap::new(Arc::clone(&source_text));
        let diagram = crate::document::whole_document_diagram(source_text, &self.options.source);
        let analyzed = self.analyze_diagram(&diagram, &source_map);
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

    pub(crate) fn analyze_diagram(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
    ) -> AnalyzedDiagram {
        let local =
            self.analyze_diagram_local(diagram, document_source_map, AnalysisMode::RichFacts);
        AnalyzedDiagram::from_document_diagram(diagram, local.diagnostics, local.syntax)
    }

    pub(crate) fn analyze_diagram_diagnostics(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
    ) -> Vec<AnalysisDiagnostic> {
        self.analyze_diagram_local(diagram, document_source_map, AnalysisMode::Diagnostics)
            .diagnostics
    }

    pub(crate) fn analyze_source_diagnostics(&self, source: &str) -> Vec<AnalysisDiagnostic> {
        self.analyze_local(source, AnalysisMode::Diagnostics)
            .diagnostics
    }

    fn analyze_local(&self, source: &str, mode: AnalysisMode) -> LocalAnalysis {
        let source_map = SourceMap::new(source);
        self.analyze_local_with_source_map(source, &source_map, mode)
    }

    fn analyze_diagram_local(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        if diagram.is_fence() {
            self.analyze_local(&diagram.text, mode)
        } else {
            self.analyze_local_with_source_map(&diagram.text, document_source_map, mode)
        }
    }

    fn analyze_local_with_source_map(
        &self,
        source: &str,
        source_map: &SourceMap,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        if let Some(diagnostics) = self.source_limit_diagnostics(source) {
            return LocalAnalysis::empty_syntax(diagnostics);
        }

        if source.trim().is_empty() {
            let diagnostics = no_diagram_diagnostic(source_map, &self.options.rule_config)
                .into_iter()
                .collect();
            return mode.unavailable_syntax(None, diagnostics);
        }

        let source_lints =
            crate::rules::source_lint_diagnostics(source, source_map, &self.options.rule_config);

        let parse_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine.parse_diagram_snapshot_sync(source)
        }));

        match parse_result {
            Err(panic_payload) => {
                let mut diagnostics = source_lints;
                if let Some(diagnostic) =
                    panic_diagnostic(panic_payload, source_map, &self.options.rule_config)
                {
                    diagnostics.push(diagnostic);
                }
                mode.unavailable_syntax(None, diagnostics)
            }
            Ok(parse_result) => match parse_result {
                Ok(Some(snapshot)) => {
                    let (meta, outcome, editor_facts) = snapshot.into_parts();
                    match outcome {
                        DiagramParseOutcome::Parsed(model) => self.analyze_parsed_diagram(
                            source,
                            source_map,
                            ParsedDiagram { meta, model },
                            editor_facts,
                            source_lints,
                            mode,
                        ),
                        DiagramParseOutcome::Failed(error) => self.analyze_parse_error(
                            source_map,
                            source_lints,
                            meta,
                            editor_facts,
                            error,
                            mode,
                        ),
                    }
                }
                Ok(None) => mode.unavailable_syntax(None, source_lints),
                Err(error) => self.analyze_operation_error(source_map, source_lints, error, mode),
            },
        }
    }

    fn analyze_parsed_diagram(
        &self,
        source: &str,
        source_map: &SourceMap,
        parsed: ParsedDiagram,
        editor_facts: ParsedEditorFacts,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let diagram_type = parsed.meta.diagram_type;
        diagnostics.extend(crate::rules::parsed_source_lint_diagnostics(
            source,
            source_map,
            &self.options.rule_config,
            &diagram_type,
        ));
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
                let effective_layout = parsed
                    .meta
                    .effective_config
                    .get_str("layout")
                    .map(str::to_owned);
                let flowchart_projection =
                    self.flowchart_facts_projection(&parsed.model, &diagram_type, source_map);
                diagnostics.extend(flowchart_projection.diagnostics);
                let editor_projection = match editor_facts {
                    ParsedEditorFacts::Available(facts) => self.editor_facts_projection_from_facts(
                        &diagram_type,
                        source_map,
                        mode,
                        Some(facts),
                    ),
                    ParsedEditorFacts::Unavailable => self.editor_facts_projection_from_facts(
                        &diagram_type,
                        source_map,
                        mode,
                        None,
                    ),
                };
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
                    .with_effective_layout(effective_layout)
                    .with_flowchart(flowchart_projection.facts),
                }
            }
        }
    }

    fn analyze_parse_error(
        &self,
        source_map: &SourceMap,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        meta: ParseMetadata,
        editor_facts: ParsedEditorFacts,
        error: CoreError,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let core_diagnostic = core_error_diagnostic(error, source_map, &self.options.rule_config);
        if let Some(diagnostic) = core_diagnostic.diagnostic {
            diagnostics.push(diagnostic);
        }
        let diagram_type = meta.diagram_type;
        let editor_projection = match editor_facts {
            ParsedEditorFacts::Available(facts) => self.editor_facts_projection_from_facts(
                &diagram_type,
                source_map,
                mode,
                Some(facts),
            ),
            ParsedEditorFacts::Unavailable => {
                self.editor_facts_projection_from_facts(&diagram_type, source_map, mode, None)
            }
        };
        merge_recovery_diagnostics(
            &mut diagnostics,
            editor_projection.diagnostics,
            core_diagnostic.parse_location,
        );
        let effective_layout = meta.effective_config.get_str("layout").map(str::to_owned);
        let syntax = AnalysisSyntaxFacts::new(Some(diagram_type), editor_projection.text_index)
            .with_effective_layout(effective_layout);
        LocalAnalysis {
            diagnostics,
            syntax,
        }
    }

    fn analyze_operation_error(
        &self,
        source_map: &SourceMap,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        error: CoreError,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let core_diagnostic = core_error_diagnostic(error, source_map, &self.options.rule_config);
        if let Some(diagnostic) = core_diagnostic.diagnostic {
            diagnostics.push(diagnostic);
        }
        mode.unavailable_syntax(core_diagnostic.diagram_type, diagnostics)
    }

    fn editor_facts_projection_from_facts(
        &self,
        diagram_type: &str,
        source_map: &SourceMap,
        mode: AnalysisMode,
        facts: Option<EditorSemanticFacts>,
    ) -> EditorFactsProjection {
        let Some(facts) = facts else {
            return EditorFactsProjection::unavailable(Vec::new());
        };

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

    pub(crate) fn source_limit_result(
        &self,
        source: &str,
        descriptor: SourceDescriptor,
    ) -> Option<AnalysisResult> {
        crate::source_limits::source_limit_result(source, descriptor, self.options.max_source_bytes)
    }

    fn source_limit_diagnostics(&self, source: &str) -> Option<Vec<AnalysisDiagnostic>> {
        crate::source_limits::source_limit_diagnostics(source, self.options.max_source_bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisMode {
    Diagnostics,
    RichFacts,
}

impl AnalysisMode {
    fn unavailable_syntax(
        self,
        diagram_type: Option<String>,
        diagnostics: Vec<AnalysisDiagnostic>,
    ) -> LocalAnalysis {
        match self {
            Self::Diagnostics => LocalAnalysis::empty_syntax_with_type(diagram_type, diagnostics),
            Self::RichFacts => LocalAnalysis::unavailable_syntax(diagram_type, diagnostics),
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

    fn unavailable_syntax(
        diagram_type: Option<String>,
        diagnostics: Vec<AnalysisDiagnostic>,
    ) -> Self {
        Self {
            diagnostics,
            syntax: AnalysisSyntaxFacts::unavailable(diagram_type),
        }
    }
}

#[derive(Debug, Clone)]
struct EditorFactsProjection {
    text_index: FenceTextIndex,
    diagnostics: Vec<AnalysisRecoveryDiagnostic>,
}

impl EditorFactsProjection {
    fn unavailable(diagnostics: Vec<AnalysisRecoveryDiagnostic>) -> Self {
        Self {
            text_index: FenceTextIndex::default(),
            diagnostics,
        }
    }
}

#[derive(Debug, Clone)]
struct FlowchartFactsProjection {
    facts: Option<AnalysisFlowchartFacts>,
    diagnostics: Vec<AnalysisDiagnostic>,
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

#[cfg(test)]
mod tests;
