use crate::diagnostic_projection::{
    core_error_diagnostic, flowchart_facts_projection_diagnostic, no_diagram_diagnostic,
    panic_diagnostic,
};
use crate::recovery::{
    AnalysisRecoveryDiagnostic, editor_recovery_diagnostics, merge_recovery_diagnostics,
};
use crate::rules::AnalysisRuleConfig;
use crate::{
    AnalysisDiagnostic, AnalysisFlowchartFacts, AnalysisOutcome, AnalysisPayload,
    AnalysisRejection, AnalysisResult, AnalysisSyntaxFacts, AnalyzedDiagram, DocumentDiagram,
    FenceTextIndex, SourceDescriptor, SourceMap,
};
use merman_core::{
    DiagramParseOutcome, EditorSemanticFacts, Engine, Error as CoreError, MermaidConfig,
    ParseMetadata, ParsedEditorFacts,
};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

use crate::result::DiagramAnalysisEvidence;

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisOptions {
    pub source: SourceDescriptor,
    pub site_config: Option<MermaidConfig>,
    pub runtime_policy: merman_core::runtime::RuntimePolicy,
    pub max_source_bytes: Option<usize>,
    pub rule_config: AnalysisRuleConfig,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            source: SourceDescriptor::diagram(),
            site_config: None,
            runtime_policy: merman_core::runtime::RuntimePolicy::deterministic(),
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
        self.runtime_policy = self.runtime_policy.with_fixed_today(today);
        self
    }

    pub fn try_with_fixed_today_at_local_midnight(
        mut self,
        today: chrono::NaiveDate,
    ) -> Result<Self, merman_core::runtime::RuntimePolicyError> {
        self.runtime_policy = self
            .runtime_policy
            .try_with_fixed_today_at_local_midnight(today)?;
        Ok(self)
    }

    pub fn try_with_fixed_local_offset_minutes(
        mut self,
        offset_minutes: i32,
    ) -> Result<Self, merman_core::runtime::RuntimePolicyError> {
        self.runtime_policy = self
            .runtime_policy
            .try_with_fixed_local_offset_minutes(offset_minutes)?;
        Ok(self)
    }

    pub fn with_runtime_policy(
        mut self,
        runtime_policy: merman_core::runtime::RuntimePolicy,
    ) -> Self {
        self.runtime_policy = runtime_policy;
        self
    }

    pub fn with_operation_context(self, context: merman_core::runtime::OperationContext) -> Self {
        self.with_runtime_policy(merman_core::runtime::RuntimePolicy::from_operation_context(
            context,
        ))
    }

    pub fn try_native() -> Result<Self, merman_core::runtime::RuntimePolicyError> {
        Ok(Self::default().with_runtime_policy(merman_core::runtime::RuntimePolicy::try_native()?))
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
            && self.runtime_policy == other.runtime_policy
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

    /// Creates an analyzer with a customized engine while keeping `options` authoritative.
    ///
    /// Registry customizations on `engine` are preserved. Runtime policy and site configuration
    /// are always taken from `options`, so snapshots cannot describe a different environment from
    /// the one used by the parser.
    pub fn with_engine(engine: Engine, options: AnalysisOptions) -> Self {
        let engine = apply_options_to_engine(engine, &options);
        Self { engine, options }
    }

    pub fn options(&self) -> &AnalysisOptions {
        &self.options
    }

    pub(crate) fn try_for_operation(
        &self,
    ) -> Result<Self, merman_core::runtime::RuntimePolicyError> {
        let context = self.engine.begin_operation()?;
        Ok(Self {
            engine: self.engine.clone().with_operation_context(context.clone()),
            options: self.options.clone().with_operation_context(context),
        })
    }

    pub(crate) fn runtime_policy_diagnostics(
        &self,
        error: merman_core::runtime::RuntimePolicyError,
        source_map: &SourceMap,
    ) -> Vec<AnalysisDiagnostic> {
        core_error_diagnostic(
            &CoreError::from(error),
            source_map,
            &self.options.rule_config,
        )
        .diagnostic
        .into_iter()
        .collect()
    }

    pub fn analyze_result(&self, source: &str) -> AnalysisOutcome {
        if let Some(rejection) = self.source_limit_rejection(source, self.options.source.clone()) {
            return AnalysisOutcome::Rejected(rejection);
        }

        let source_text = Arc::<str>::from(source);
        let source_map = SourceMap::new(Arc::clone(&source_text));
        let diagram = crate::document::whole_document_diagram(source_text, &self.options.source);
        let analyzed = self.analyze_diagram(&diagram, &source_map);
        AnalysisOutcome::Ready(AnalysisResult::new(
            self.options.source.clone(),
            source_map,
            analyzed.diagnostics.clone(),
            vec![analyzed],
        ))
    }

    pub fn analyze(&self, source: &str) -> AnalysisPayload {
        if let Some(rejection) = self.source_limit_rejection(source, self.options.source.clone()) {
            return rejection.into_payload();
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

    /// Reprojects diagnostics from one canonical rich analysis generation.
    ///
    /// Only this analyzer's rule configuration participates in reprojection. Parsing, Mermaid
    /// configuration, runtime policy, and editor facts remain frozen in `result`.
    pub fn reproject_payload(&self, result: &AnalysisResult) -> AnalysisPayload {
        let diagnostics = if let Some(error) = result.document_error() {
            self.runtime_error_diagnostics(error, result.source_map())
        } else {
            result
                .diagrams()
                .iter()
                .flat_map(|diagram| {
                    let source_map = if diagram.kind == crate::DocumentDiagramKind::MermaidFence {
                        SourceMap::new(diagram.text.as_str())
                    } else {
                        result.source_map().clone()
                    };
                    let local = self.project_evidence(
                        diagram.text.as_str(),
                        &source_map,
                        diagram.evidence.as_ref(),
                        AnalysisMode::DiagnosticReprojection,
                    );
                    crate::document::remap_analyzed_diagnostics(
                        result.source_map(),
                        result.payload().source.kind,
                        diagram,
                        local.diagnostics,
                    )
                })
                .collect()
        };
        AnalysisPayload::new(result.payload().source.clone(), diagnostics)
    }

    pub(crate) fn analyze_diagram(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
    ) -> AnalyzedDiagram {
        let source_map = if diagram.is_fence() {
            SourceMap::new(diagram.text.as_str())
        } else {
            document_source_map.clone()
        };
        let evidence = Arc::new(self.capture_evidence(diagram.text.as_str()));
        let local = self.project_evidence(
            diagram.text.as_str(),
            &source_map,
            evidence.as_ref(),
            AnalysisMode::RichFacts,
        );
        AnalyzedDiagram::from_document_diagram_with_evidence(
            diagram,
            local.diagnostics,
            local.syntax,
            evidence,
        )
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
        let source_map = if diagram.is_fence() {
            SourceMap::new(diagram.text.as_str())
        } else {
            document_source_map.clone()
        };
        let evidence = self.capture_evidence(diagram.text.as_str());
        self.project_evidence(diagram.text.as_str(), &source_map, &evidence, mode)
    }

    fn analyze_local_with_source_map(
        &self,
        source: &str,
        source_map: &SourceMap,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let evidence = self.capture_evidence(source);
        self.project_evidence(source, source_map, &evidence, mode)
    }

    fn capture_evidence(&self, source: &str) -> DiagramAnalysisEvidence {
        if self.source_limit_diagnostics(source).is_some() {
            return DiagramAnalysisEvidence::SourceLimit;
        }
        if source.trim().is_empty() {
            return DiagramAnalysisEvidence::EmptySource;
        }
        let parse_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine.parse_diagram_snapshot_sync(source)
        }));
        match parse_result {
            Err(panic_payload) => DiagramAnalysisEvidence::Panic {
                message: panic_message(panic_payload.as_ref()).to_string(),
            },
            Ok(parse_result) => match parse_result {
                Ok(Some(snapshot)) => {
                    let (meta, outcome, editor_facts) = snapshot.into_parts();
                    let editor_facts = match editor_facts {
                        ParsedEditorFacts::Available(facts) => Some(Arc::new(facts)),
                        ParsedEditorFacts::Unavailable => None,
                    };
                    match outcome {
                        DiagramParseOutcome::Parsed(model) => DiagramAnalysisEvidence::Parsed {
                            metadata: meta,
                            model: Arc::new(model),
                            editor_facts,
                        },
                        DiagramParseOutcome::Failed(error) => {
                            DiagramAnalysisEvidence::ParseFailed {
                                metadata: meta,
                                error: Arc::new(error),
                                editor_facts,
                            }
                        }
                    }
                }
                Ok(None) => DiagramAnalysisEvidence::NoSnapshot,
                Err(error) => DiagramAnalysisEvidence::OperationError {
                    error: Arc::new(error),
                },
            },
        }
    }

    fn project_evidence(
        &self,
        source: &str,
        source_map: &SourceMap,
        evidence: &DiagramAnalysisEvidence,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        if matches!(evidence, DiagramAnalysisEvidence::SourceLimit) {
            return LocalAnalysis::empty_syntax(
                self.source_limit_diagnostics(source).unwrap_or_default(),
            );
        }

        let source_lints =
            crate::rules::source_lint_diagnostics(source, source_map, &self.options.rule_config);
        match evidence {
            DiagramAnalysisEvidence::SourceLimit => unreachable!("handled above"),
            DiagramAnalysisEvidence::EmptySource => {
                let diagnostics = no_diagram_diagnostic(source_map, &self.options.rule_config)
                    .into_iter()
                    .collect();
                mode.unavailable_syntax(None, diagnostics)
            }
            DiagramAnalysisEvidence::Panic { message } => {
                let mut diagnostics = source_lints;
                if let Some(diagnostic) =
                    panic_diagnostic(message, source_map, &self.options.rule_config)
                {
                    diagnostics.push(diagnostic);
                }
                mode.unavailable_syntax(None, diagnostics)
            }
            DiagramAnalysisEvidence::NoSnapshot => mode.unavailable_syntax(None, source_lints),
            DiagramAnalysisEvidence::OperationError { error } => {
                self.analyze_operation_error(source_map, source_lints, error, mode)
            }
            DiagramAnalysisEvidence::Parsed {
                metadata,
                model,
                editor_facts,
            } => self.analyze_parsed_diagram(
                source,
                source_map,
                metadata,
                model,
                editor_facts.as_deref(),
                source_lints,
                mode,
            ),
            DiagramAnalysisEvidence::ParseFailed {
                metadata,
                error,
                editor_facts,
            } => self.analyze_parse_error(
                source_map,
                source_lints,
                metadata,
                editor_facts.as_deref(),
                error,
                mode,
            ),
        }
    }

    fn analyze_parsed_diagram(
        &self,
        source: &str,
        source_map: &SourceMap,
        metadata: &ParseMetadata,
        model: &serde_json::Value,
        editor_facts: Option<&EditorSemanticFacts>,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let diagram_type = metadata.diagram_type.as_str();
        diagnostics.extend(crate::rules::parsed_source_lint_diagnostics(
            source,
            source_map,
            &self.options.rule_config,
            diagram_type,
        ));
        diagnostics.extend(crate::rules::semantic_warning_diagnostics(
            diagram_type,
            model,
            source_map,
            &self.options.rule_config,
        ));

        match mode {
            AnalysisMode::Diagnostics => {
                LocalAnalysis::empty_syntax_with_type(Some(diagram_type.to_string()), diagnostics)
            }
            AnalysisMode::RichFacts | AnalysisMode::DiagnosticReprojection => {
                let effective_layout = metadata
                    .effective_config
                    .get_str("layout")
                    .map(str::to_owned);
                let flowchart_projection =
                    self.flowchart_facts_projection(model, diagram_type, source_map);
                diagnostics.extend(flowchart_projection.diagnostics);
                let editor_projection = self.editor_facts_projection_from_facts(
                    diagram_type,
                    source_map,
                    mode,
                    editor_facts,
                );
                diagnostics.extend(
                    editor_projection
                        .diagnostics
                        .into_iter()
                        .map(|recovery| recovery.diagnostic),
                );
                if mode == AnalysisMode::DiagnosticReprojection {
                    return LocalAnalysis::empty_syntax_with_type(
                        Some(diagram_type.to_string()),
                        diagnostics,
                    );
                }
                LocalAnalysis {
                    diagnostics,
                    syntax: AnalysisSyntaxFacts::new(
                        Some(diagram_type.to_string()),
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
        meta: &ParseMetadata,
        editor_facts: Option<&EditorSemanticFacts>,
        error: &CoreError,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let core_diagnostic = core_error_diagnostic(error, source_map, &self.options.rule_config);
        if let Some(diagnostic) = core_diagnostic.diagnostic {
            diagnostics.push(diagnostic);
        }
        let diagram_type = meta.diagram_type.as_str();
        let editor_projection =
            self.editor_facts_projection_from_facts(diagram_type, source_map, mode, editor_facts);
        merge_recovery_diagnostics(
            &mut diagnostics,
            editor_projection.diagnostics,
            core_diagnostic.parse_location,
        );
        let effective_layout = meta.effective_config.get_str("layout").map(str::to_owned);
        let syntax =
            AnalysisSyntaxFacts::new(Some(diagram_type.to_string()), editor_projection.text_index)
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
        error: &CoreError,
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
        facts: Option<&EditorSemanticFacts>,
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
                AnalysisMode::Diagnostics | AnalysisMode::DiagnosticReprojection => {
                    FenceTextIndex::default()
                }
                AnalysisMode::RichFacts => FenceTextIndex::from_core_facts(facts.clone()),
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

    pub(crate) fn source_limit_rejection(
        &self,
        source: &str,
        descriptor: SourceDescriptor,
    ) -> Option<AnalysisRejection> {
        crate::source_limits::source_limit_rejection(
            source,
            descriptor,
            self.options.max_source_bytes,
        )
    }

    fn source_limit_diagnostics(&self, source: &str) -> Option<Vec<AnalysisDiagnostic>> {
        crate::source_limits::source_limit_diagnostics(source, self.options.max_source_bytes)
    }

    fn runtime_error_diagnostics(
        &self,
        error: &CoreError,
        source_map: &SourceMap,
    ) -> Vec<AnalysisDiagnostic> {
        core_error_diagnostic(error, source_map, &self.options.rule_config)
            .diagnostic
            .into_iter()
            .collect()
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("panic while analyzing Mermaid source")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisMode {
    Diagnostics,
    DiagnosticReprojection,
    RichFacts,
}

impl AnalysisMode {
    fn unavailable_syntax(
        self,
        diagram_type: Option<String>,
        diagnostics: Vec<AnalysisDiagnostic>,
    ) -> LocalAnalysis {
        match self {
            Self::Diagnostics | Self::DiagnosticReprojection => {
                LocalAnalysis::empty_syntax_with_type(diagram_type, diagnostics)
            }
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
    apply_options_to_engine(Engine::new(), options)
}

fn apply_options_to_engine(mut engine: Engine, options: &AnalysisOptions) -> Engine {
    engine = engine.with_runtime_policy(options.runtime_policy.clone());

    if let Some(site_config) = options.site_config.clone() {
        engine = engine.with_site_config(site_config);
    }

    engine
}

#[cfg(test)]
mod tests;
