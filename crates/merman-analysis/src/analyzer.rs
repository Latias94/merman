use crate::diagnostic_projection::{
    core_error_diagnostic, flowchart_facts_projection_diagnostic, no_diagram_diagnostic,
    panic_diagnostic,
};
use crate::recovery::{
    AnalysisRecoveryDiagnostic, editor_recovery_diagnostics, merge_recovery_diagnostics,
};
use crate::rules::AnalysisRuleConfig;
use crate::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisCaptureOutcome, AnalysisDiagnostic,
    AnalysisFlowchartFacts, AnalysisGeneration, AnalysisPayload, AnalysisRejection,
    AnalysisSyntaxFacts, AnalyzedDiagram, DocumentDiagram, FenceTextIndex, SourceDescriptor,
    SourceMap,
};
use merman_core::{
    DiagramParseOutcome, EditorSemanticFacts, Engine, Error as CoreError, MermaidConfig,
    ParseMetadata, ParsedEditorFacts,
};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

use crate::result::DiagramAnalysisEvidence;

/// Complete analysis configuration, split by generation and diagnostic invalidation scope.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisOptions {
    /// Inputs that require a new canonical parse generation when changed.
    pub snapshot: AnalysisSnapshotPolicy,
    /// Inputs that can reproject diagnostics from an existing canonical generation.
    pub diagnostics: AnalysisDiagnosticPolicy,
}

/// Inputs frozen into one canonical analysis generation.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisSnapshotPolicy {
    /// Host document identity and source kind.
    pub source: SourceDescriptor,
    /// Mermaid site configuration applied from pinned defaults.
    pub site_config: Option<MermaidConfig>,
    /// Deterministic or host-backed runtime values used during parsing.
    pub runtime_policy: merman_core::runtime::RuntimePolicy,
    /// Optional source-size ceiling enforced before parsing.
    pub max_source_bytes: Option<usize>,
}

/// Inputs that reproject diagnostics without rebuilding parser-owned facts.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisDiagnosticPolicy {
    /// Enabled rules, profiles, and severity overrides.
    pub rule_config: AnalysisRuleConfig,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            snapshot: AnalysisSnapshotPolicy {
                source: SourceDescriptor::diagram(),
                site_config: None,
                runtime_policy: merman_core::runtime::RuntimePolicy::deterministic(),
                max_source_bytes: None,
            },
            diagnostics: AnalysisDiagnosticPolicy {
                rule_config: AnalysisRuleConfig::default(),
            },
        }
    }
}

impl AnalysisOptions {
    pub fn with_source(mut self, source: SourceDescriptor) -> Self {
        self.snapshot.source = source;
        self
    }

    pub fn with_site_config(mut self, site_config: MermaidConfig) -> Self {
        self.snapshot.site_config = Some(site_config);
        self
    }

    pub fn with_fixed_today(mut self, today: Option<chrono::NaiveDate>) -> Self {
        self.snapshot.runtime_policy = self.snapshot.runtime_policy.with_fixed_today(today);
        self
    }

    pub fn try_with_fixed_today_at_local_midnight(
        mut self,
        today: chrono::NaiveDate,
    ) -> Result<Self, merman_core::runtime::RuntimePolicyError> {
        self.snapshot.runtime_policy = self
            .snapshot
            .runtime_policy
            .try_with_fixed_today_at_local_midnight(today)?;
        Ok(self)
    }

    pub fn try_with_fixed_local_offset_minutes(
        mut self,
        offset_minutes: i32,
    ) -> Result<Self, merman_core::runtime::RuntimePolicyError> {
        self.snapshot.runtime_policy = self
            .snapshot
            .runtime_policy
            .try_with_fixed_local_offset_minutes(offset_minutes)?;
        Ok(self)
    }

    pub fn with_runtime_policy(
        mut self,
        runtime_policy: merman_core::runtime::RuntimePolicy,
    ) -> Self {
        self.snapshot.runtime_policy = runtime_policy;
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
        self.snapshot.max_source_bytes = max_source_bytes;
        self
    }

    pub fn with_rule_config(mut self, rule_config: AnalysisRuleConfig) -> Self {
        self.diagnostics.rule_config = rule_config;
        self
    }

    pub fn snapshot_policy(&self) -> &AnalysisSnapshotPolicy {
        &self.snapshot
    }

    pub fn diagnostic_policy(&self) -> &AnalysisDiagnosticPolicy {
        &self.diagnostics
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.snapshot.source
    }

    pub fn site_config(&self) -> Option<&MermaidConfig> {
        self.snapshot.site_config.as_ref()
    }

    pub fn runtime_policy(&self) -> &merman_core::runtime::RuntimePolicy {
        &self.snapshot.runtime_policy
    }

    pub fn max_source_bytes(&self) -> Option<usize> {
        self.snapshot.max_source_bytes
    }

    pub fn rule_config(&self) -> &AnalysisRuleConfig {
        &self.diagnostics.rule_config
    }
}

/// Opaque identity for one analyzer environment and its snapshot-affecting policy.
#[derive(Clone)]
pub struct AnalysisEnvironmentIdentity(Arc<()>);

impl AnalysisEnvironmentIdentity {
    fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl fmt::Debug for AnalysisEnvironmentIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AnalysisEnvironmentIdentity(..)")
    }
}

impl PartialEq for AnalysisEnvironmentIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for AnalysisEnvironmentIdentity {}

impl Hash for AnalysisEnvironmentIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct Analyzer {
    engine: Engine,
    options: AnalysisOptions,
    environment_identity: AnalysisEnvironmentIdentity,
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
        Self {
            engine,
            options,
            environment_identity: AnalysisEnvironmentIdentity::new(),
        }
    }

    /// Creates an analyzer with a customized engine while keeping `options` authoritative.
    ///
    /// Registry customizations on `engine` are preserved. Runtime policy and site configuration
    /// are always taken from `options`, so snapshots cannot describe a different environment from
    /// the one used by the parser.
    pub fn with_engine(engine: Engine, options: AnalysisOptions) -> Self {
        let engine = apply_options_to_engine(engine, &options);
        Self {
            engine,
            options,
            environment_identity: AnalysisEnvironmentIdentity::new(),
        }
    }

    pub fn options(&self) -> &AnalysisOptions {
        &self.options
    }

    pub fn environment_identity(&self) -> &AnalysisEnvironmentIdentity {
        &self.environment_identity
    }

    /// Derives a diagnostics-only view without changing the parser environment identity.
    pub fn with_diagnostic_policy(&self, policy: AnalysisDiagnosticPolicy) -> Self {
        let mut options = self.options.clone();
        options.diagnostics = policy;
        Self {
            engine: self.engine.clone(),
            options,
            environment_identity: self.environment_identity.clone(),
        }
    }

    /// Derives an analyzer with an exact snapshot policy and a new environment identity.
    pub fn with_snapshot_policy(&self, policy: AnalysisSnapshotPolicy) -> Self {
        let options = AnalysisOptions {
            snapshot: policy,
            diagnostics: self.options.diagnostics.clone(),
        };
        Self {
            engine: apply_options_to_engine(self.engine.clone(), &options),
            options,
            environment_identity: AnalysisEnvironmentIdentity::new(),
        }
    }

    pub(crate) fn with_capture_source(&self, source: SourceDescriptor) -> Self {
        let mut options = self.options.clone();
        options.snapshot.source = source;
        Self {
            engine: self.engine.clone(),
            options,
            environment_identity: AnalysisEnvironmentIdentity::new(),
        }
    }

    pub(crate) fn try_for_operation(
        &self,
    ) -> Result<Self, merman_core::runtime::RuntimePolicyError> {
        let context = self.engine.begin_operation()?;
        Ok(Self {
            engine: self.engine.clone().with_operation_context(context.clone()),
            options: self.options.clone().with_operation_context(context),
            environment_identity: self.environment_identity.clone(),
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
            self.options.rule_config(),
        )
        .diagnostic
        .into_iter()
        .collect()
    }

    pub fn analyze_generation(&self, source: &str) -> AnalysisCaptureOutcome {
        if let Some(rejection) = self.source_limit_rejection(source, self.options.source().clone())
        {
            return AnalysisCaptureOutcome::Rejected(rejection);
        }

        let source_text = Arc::<str>::from(source);
        let source_map = SourceMap::new(Arc::clone(&source_text));
        let operation_analyzer = match self.try_for_operation() {
            Ok(analyzer) => analyzer,
            Err(error) => {
                return AnalysisCaptureOutcome::Ready(
                    AnalysisGeneration::new(source_map, Vec::new(), self)
                        .with_document_error(Arc::new(CoreError::from(error))),
                );
            }
        };
        let diagram = crate::document::whole_document_diagram(source_text, self.options.source());
        let analyzed = operation_analyzer.analyze_diagram(&diagram, &source_map);
        AnalysisCaptureOutcome::Ready(AnalysisGeneration::new(
            source_map,
            vec![analyzed],
            &operation_analyzer,
        ))
    }

    pub fn analyze_generation_cancellable(
        &self,
        source: &str,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<AnalysisCaptureOutcome, AnalysisCancelled> {
        cancellation.checkpoint()?;
        if let Some(rejection) = self.source_limit_rejection(source, self.options.source().clone())
        {
            return Ok(AnalysisCaptureOutcome::Rejected(rejection));
        }

        let source_text = Arc::<str>::from(source);
        let source_map = SourceMap::new_cancellable(Arc::clone(&source_text), cancellation)?;
        let operation_analyzer = match self.try_for_operation() {
            Ok(analyzer) => analyzer,
            Err(error) => {
                return Ok(AnalysisCaptureOutcome::Ready(
                    AnalysisGeneration::new(source_map, Vec::new(), self)
                        .with_document_error(Arc::new(CoreError::from(error))),
                ));
            }
        };
        let diagram = crate::document::whole_document_diagram(source_text, self.options.source());
        let analyzed =
            operation_analyzer.analyze_diagram_cancellable(&diagram, &source_map, cancellation)?;
        cancellation.checkpoint()?;
        Ok(AnalysisCaptureOutcome::Ready(AnalysisGeneration::new(
            source_map,
            vec![analyzed],
            &operation_analyzer,
        )))
    }

    pub fn analyze(&self, source: &str) -> AnalysisPayload {
        if let Some(rejection) = self.source_limit_rejection(source, self.options.source().clone())
        {
            return rejection.into_payload();
        }

        AnalysisPayload::new(
            self.options.source().clone(),
            self.analyze_source_diagnostics(source),
        )
    }

    pub fn analyze_facts(&self, source: &str) -> crate::AnalysisFactsPayload {
        match self.analyze_generation(source) {
            AnalysisCaptureOutcome::Ready(generation) => {
                generation.to_facts_payload(self.options.diagnostic_policy())
            }
            AnalysisCaptureOutcome::Rejected(rejection) => {
                crate::AnalysisFactsPayload::from_rejection(&rejection)
            }
        }
    }

    pub fn analyze_json(&self, source: &str) -> Result<Vec<u8>, serde_json::Error> {
        self.analyze(source).to_json_bytes()
    }

    pub fn analyze_facts_json(&self, source: &str) -> Result<Vec<u8>, serde_json::Error> {
        self.analyze_facts(source).to_json_bytes()
    }

    pub(crate) fn project_generation_cancellable(
        &self,
        generation: &AnalysisGeneration,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<AnalysisPayload, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let diagnostics = if let Some(error) = generation.document_error() {
            self.runtime_error_diagnostics(error, generation.source_map())
        } else {
            let mut diagnostics = Vec::new();
            for diagram in generation.diagrams() {
                cancellation.checkpoint()?;
                let source_map = if diagram.kind == crate::DocumentDiagramKind::MermaidFence {
                    SourceMap::new_cancellable(diagram.text.as_str(), cancellation)?
                } else {
                    generation.source_map().clone()
                };
                let local = self.project_evidence_cancellable(
                    diagram.text.as_str(),
                    &source_map,
                    diagram.evidence.as_ref(),
                    AnalysisMode::DiagnosticReprojection,
                    cancellation,
                )?;
                diagnostics.extend(crate::document::remap_analyzed_diagnostics(
                    generation.source_map(),
                    generation.snapshot_policy().source.kind,
                    diagram,
                    local.diagnostics,
                ));
            }
            diagnostics
        };
        cancellation.checkpoint()?;
        Ok(AnalysisPayload::new(
            generation.snapshot_policy().source.clone(),
            diagnostics,
        ))
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
        AnalyzedDiagram::from_document_diagram_with_evidence(diagram, local.syntax, evidence)
    }

    pub(crate) fn analyze_diagram_cancellable(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<AnalyzedDiagram, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let source_map = if diagram.is_fence() {
            SourceMap::new_cancellable(diagram.text.as_str(), cancellation)?
        } else {
            document_source_map.clone()
        };
        let evidence =
            Arc::new(self.capture_evidence_cancellable(diagram.text.as_str(), cancellation)?);
        cancellation.checkpoint()?;
        let local = self.project_evidence_cancellable(
            diagram.text.as_str(),
            &source_map,
            evidence.as_ref(),
            AnalysisMode::RichFacts,
            cancellation,
        )?;
        cancellation.checkpoint()?;
        Ok(AnalyzedDiagram::from_document_diagram_with_evidence(
            diagram,
            local.syntax,
            evidence,
        ))
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
        let cancellation = AnalysisCancellationToken::new();
        self.capture_evidence_cancellable(source, &cancellation)
            .unwrap_or_else(|_| DiagramAnalysisEvidence::OperationError {
                error: Arc::new(CoreError::from(merman_core::ParseCancelled)),
            })
    }

    fn capture_evidence_cancellable(
        &self,
        source: &str,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<DiagramAnalysisEvidence, AnalysisCancelled> {
        cancellation.checkpoint()?;
        if self.source_limit_diagnostics(source).is_some() {
            return Ok(DiagramAnalysisEvidence::SourceLimit);
        }
        if source.trim().is_empty() {
            return Ok(DiagramAnalysisEvidence::EmptySource);
        }
        let parse_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine
                .parse_diagram_snapshot_controlled_sync(source, cancellation.parse_control())
        }));
        let evidence = match parse_result {
            Err(panic_payload) => DiagramAnalysisEvidence::Panic {
                message: panic_message(panic_payload.as_ref()).to_string(),
            },
            Ok(Err(_)) => return Err(AnalysisCancelled),
            Ok(Ok(parse_result)) => match parse_result {
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
        };
        cancellation.checkpoint()?;
        Ok(evidence)
    }

    fn project_evidence(
        &self,
        source: &str,
        source_map: &SourceMap,
        evidence: &DiagramAnalysisEvidence,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let cancellation = AnalysisCancellationToken::new();
        self.project_evidence_cancellable(source, source_map, evidence, mode, &cancellation)
            .expect("a private analysis cancellation token cannot be cancelled")
    }

    fn project_evidence_cancellable(
        &self,
        source: &str,
        source_map: &SourceMap,
        evidence: &DiagramAnalysisEvidence,
        mode: AnalysisMode,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<LocalAnalysis, AnalysisCancelled> {
        cancellation.checkpoint()?;
        if matches!(evidence, DiagramAnalysisEvidence::SourceLimit) {
            return Ok(LocalAnalysis::empty_syntax(
                self.source_limit_diagnostics(source).unwrap_or_default(),
            ));
        }

        let source_lints = crate::rules::source_lint_diagnostics_cancellable(
            source,
            source_map,
            self.options.rule_config(),
            cancellation,
        )?;
        cancellation.checkpoint()?;
        match evidence {
            DiagramAnalysisEvidence::SourceLimit => unreachable!("handled above"),
            DiagramAnalysisEvidence::EmptySource => {
                let diagnostics = no_diagram_diagnostic(source_map, self.options.rule_config())
                    .into_iter()
                    .collect();
                Ok(mode.unavailable_syntax(None, diagnostics))
            }
            DiagramAnalysisEvidence::Panic { message } => {
                let mut diagnostics = source_lints;
                if let Some(diagnostic) =
                    panic_diagnostic(message, source_map, self.options.rule_config())
                {
                    diagnostics.push(diagnostic);
                }
                Ok(mode.unavailable_syntax(None, diagnostics))
            }
            DiagramAnalysisEvidence::NoSnapshot => Ok(mode.unavailable_syntax(None, source_lints)),
            DiagramAnalysisEvidence::OperationError { error } => {
                Ok(self.analyze_operation_error(source_map, source_lints, error, mode))
            }
            DiagramAnalysisEvidence::Parsed {
                metadata,
                model,
                editor_facts,
            } => self.analyze_parsed_diagram_cancellable(
                DiagramProjectionInput {
                    source,
                    source_map,
                    metadata,
                    editor_facts: editor_facts.as_deref(),
                },
                model,
                source_lints,
                mode,
                cancellation,
            ),
            DiagramAnalysisEvidence::ParseFailed {
                metadata,
                error,
                editor_facts,
            } => self.analyze_parse_error_cancellable(
                DiagramProjectionInput {
                    source,
                    source_map,
                    metadata,
                    editor_facts: editor_facts.as_deref(),
                },
                source_lints,
                error,
                mode,
                cancellation,
            ),
        }
    }

    #[cfg(test)]
    fn analyze_parsed_diagram(
        &self,
        input: DiagramProjectionInput<'_>,
        model: &serde_json::Value,
        diagnostics: Vec<AnalysisDiagnostic>,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let cancellation = AnalysisCancellationToken::new();
        self.analyze_parsed_diagram_cancellable(input, model, diagnostics, mode, &cancellation)
            .expect("a private analysis cancellation token cannot be cancelled")
    }

    fn analyze_parsed_diagram_cancellable(
        &self,
        input: DiagramProjectionInput<'_>,
        model: &serde_json::Value,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        mode: AnalysisMode,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<LocalAnalysis, AnalysisCancelled> {
        let DiagramProjectionInput {
            source,
            source_map,
            metadata,
            editor_facts,
        } = input;
        cancellation.checkpoint()?;
        let diagram_type = metadata.diagram_type.as_str();
        diagnostics.extend(crate::rules::parsed_source_lint_diagnostics_cancellable(
            source,
            source_map,
            self.options.rule_config(),
            diagram_type,
            cancellation,
        )?);
        diagnostics.extend(crate::rules::semantic_warning_diagnostics_cancellable(
            diagram_type,
            model,
            source_map,
            self.options.rule_config(),
            cancellation,
        )?);
        let editor_diagnostics = self.editor_recovery_diagnostics_from_facts_cancellable(
            diagram_type,
            source_map,
            editor_facts,
            cancellation,
        )?;
        let flowchart_projection = Some(self.flowchart_facts_projection_cancellable(
            model,
            diagram_type,
            source_map,
            cancellation,
        )?);
        diagnostics.extend(
            flowchart_projection
                .as_ref()
                .into_iter()
                .flat_map(|projection| projection.diagnostics.iter().cloned()),
        );

        let local = match mode {
            AnalysisMode::Diagnostics | AnalysisMode::DiagnosticReprojection => {
                diagnostics.extend(
                    editor_diagnostics
                        .into_iter()
                        .map(|recovery| recovery.diagnostic),
                );
                LocalAnalysis::empty_syntax_with_type(Some(diagram_type.to_string()), diagnostics)
            }
            AnalysisMode::RichFacts => {
                let effective_layout = metadata
                    .effective_config
                    .get_str("layout")
                    .map(str::to_owned);
                diagnostics.extend(
                    editor_diagnostics
                        .into_iter()
                        .map(|recovery| recovery.diagnostic),
                );
                LocalAnalysis {
                    diagnostics,
                    syntax: AnalysisSyntaxFacts::new(
                        Some(diagram_type.to_string()),
                        Self::editor_text_index_from_facts_cancellable(editor_facts, cancellation)?,
                    )
                    .with_effective_layout(effective_layout)
                    .with_flowchart(flowchart_projection.and_then(|projection| projection.facts)),
                }
            }
        };
        cancellation.checkpoint()?;
        Ok(local)
    }

    fn analyze_parse_error_cancellable(
        &self,
        input: DiagramProjectionInput<'_>,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        error: &CoreError,
        mode: AnalysisMode,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<LocalAnalysis, AnalysisCancelled> {
        let DiagramProjectionInput {
            source: _,
            source_map,
            metadata: meta,
            editor_facts,
        } = input;
        cancellation.checkpoint()?;
        let core_diagnostic = core_error_diagnostic(error, source_map, self.options.rule_config());
        if let Some(diagnostic) = core_diagnostic.diagnostic {
            diagnostics.push(diagnostic);
        }
        let diagram_type = meta.diagram_type.as_str();
        let editor_diagnostics = self.editor_recovery_diagnostics_from_facts_cancellable(
            diagram_type,
            source_map,
            editor_facts,
            cancellation,
        )?;
        merge_recovery_diagnostics(
            &mut diagnostics,
            editor_diagnostics,
            core_diagnostic.parse_location,
        );
        let local = match mode {
            AnalysisMode::Diagnostics | AnalysisMode::DiagnosticReprojection => {
                LocalAnalysis::empty_syntax_with_type(Some(diagram_type.to_string()), diagnostics)
            }
            AnalysisMode::RichFacts => {
                let effective_layout = meta.effective_config.get_str("layout").map(str::to_owned);
                let syntax = AnalysisSyntaxFacts::new(
                    Some(diagram_type.to_string()),
                    Self::editor_text_index_from_facts_cancellable(editor_facts, cancellation)?,
                )
                .with_effective_layout(effective_layout);
                LocalAnalysis {
                    diagnostics,
                    syntax,
                }
            }
        };
        cancellation.checkpoint()?;
        Ok(local)
    }

    fn analyze_operation_error(
        &self,
        source_map: &SourceMap,
        mut diagnostics: Vec<AnalysisDiagnostic>,
        error: &CoreError,
        mode: AnalysisMode,
    ) -> LocalAnalysis {
        let core_diagnostic = core_error_diagnostic(error, source_map, self.options.rule_config());
        if let Some(diagnostic) = core_diagnostic.diagnostic {
            diagnostics.push(diagnostic);
        }
        mode.unavailable_syntax(core_diagnostic.diagram_type, diagnostics)
    }

    fn editor_recovery_diagnostics_from_facts_cancellable(
        &self,
        diagram_type: &str,
        source_map: &SourceMap,
        facts: Option<&EditorSemanticFacts>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Vec<AnalysisRecoveryDiagnostic>, AnalysisCancelled> {
        let Some(facts) = facts else {
            return Ok(Vec::new());
        };

        let mut diagnostics = Vec::with_capacity(facts.diagnostics.len());
        for batch in facts.diagnostics.chunks(128) {
            cancellation.checkpoint()?;
            diagnostics.extend(editor_recovery_diagnostics(
                batch.iter().cloned(),
                diagram_type,
                source_map,
                self.options.rule_config(),
            ));
        }
        cancellation.checkpoint()?;
        Ok(diagnostics)
    }

    fn editor_text_index_from_facts_cancellable(
        facts: Option<&EditorSemanticFacts>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<FenceTextIndex, AnalysisCancelled> {
        match facts {
            Some(facts) => FenceTextIndex::from_core_facts_cancellable(facts, cancellation),
            None => Ok(FenceTextIndex::default()),
        }
    }

    fn flowchart_facts_projection_cancellable(
        &self,
        model: &serde_json::Value,
        diagram_type: &str,
        source_map: &SourceMap,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<FlowchartFactsProjection, AnalysisCancelled> {
        let projection =
            match AnalysisFlowchartFacts::try_from_model_cancellable(model, cancellation)? {
                Ok(facts) => FlowchartFactsProjection {
                    facts,
                    diagnostics: Vec::new(),
                },
                Err(error) => {
                    let _ = source_map.whole_source_span_cancellable(cancellation)?;
                    FlowchartFactsProjection {
                        facts: None,
                        diagnostics: flowchart_facts_projection_diagnostic(
                            error,
                            diagram_type,
                            source_map,
                            self.options.rule_config(),
                        )
                        .into_iter()
                        .collect(),
                    }
                }
            };
        cancellation.checkpoint()?;
        Ok(projection)
    }

    pub(crate) fn source_limit_rejection(
        &self,
        source: &str,
        descriptor: SourceDescriptor,
    ) -> Option<AnalysisRejection> {
        crate::source_limits::source_limit_rejection(
            source,
            descriptor,
            self.options.max_source_bytes(),
        )
    }

    fn source_limit_diagnostics(&self, source: &str) -> Option<Vec<AnalysisDiagnostic>> {
        crate::source_limits::source_limit_diagnostics(source, self.options.max_source_bytes())
    }

    fn runtime_error_diagnostics(
        &self,
        error: &CoreError,
        source_map: &SourceMap,
    ) -> Vec<AnalysisDiagnostic> {
        core_error_diagnostic(error, source_map, self.options.rule_config())
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

#[derive(Clone, Copy)]
struct DiagramProjectionInput<'a> {
    source: &'a str,
    source_map: &'a SourceMap,
    metadata: &'a ParseMetadata,
    editor_facts: Option<&'a EditorSemanticFacts>,
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
struct FlowchartFactsProjection {
    facts: Option<AnalysisFlowchartFacts>,
    diagnostics: Vec<AnalysisDiagnostic>,
}

pub fn engine_from_options(options: &AnalysisOptions) -> Engine {
    apply_options_to_engine(Engine::new(), options)
}

fn apply_options_to_engine(engine: Engine, options: &AnalysisOptions) -> Engine {
    engine
        .with_exact_site_config(options.site_config().cloned())
        .with_runtime_policy(options.runtime_policy().clone())
}

#[cfg(test)]
mod tests;
