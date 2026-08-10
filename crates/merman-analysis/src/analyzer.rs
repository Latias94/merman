use crate::diagnostic_projection::{
    DiagnosticCandidate, core_error_candidate, flowchart_facts_projection_candidate,
    materialize_diagnostic_candidates, no_diagram_candidate, panic_candidate,
};
use crate::recovery::editor_recovery_candidates;
use crate::rules::AnalysisRuleConfig;
use crate::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisCaptureOutcome, AnalysisDiagnostic,
    AnalysisFlowchartFacts, AnalysisGeneration, AnalysisPayload, AnalysisRejection,
    AnalysisSyntaxFacts, AnalyzedDiagram, DiagramParseDisposition, DocumentDiagram, FenceTextIndex,
    SourceDescriptor, SourceKind, SourceMap,
};
use merman_core::{
    DiagramParseOutcome, DiagramSnapshotCapture, DiagramWarningFact, EditorSemanticFacts, Engine,
    Error as CoreError, MermaidConfig, ParseMetadata, ParsedEditorFacts,
    preprocess::SourceConfigEvidence,
};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

/// Stable binding metadata for the host-document limits owned by analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnalysisResourceLimitDescriptor {
    pub stable_id: &'static str,
    pub phase: &'static str,
    pub description: &'static str,
    pub overridable: bool,
    pub minimum_value: usize,
}

pub const MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID: &str = "max_document_diagrams";
pub const ANALYSIS_RESOURCE_LIMIT_DESCRIPTORS: [AnalysisResourceLimitDescriptor; 1] =
    [AnalysisResourceLimitDescriptor {
        stable_id: MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID,
        phase: "document_scan",
        description: "Maximum Mermaid fences admitted from one Markdown or MDX host document",
        overridable: true,
        minimum_value: 0,
    }];

/// Returns the profile value for an analysis-owned resource limit.
pub fn analysis_resource_profile_value(
    profile: merman_core::resources::ResourceProfile,
    stable_id: &str,
) -> Option<usize> {
    if stable_id != MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID {
        return None;
    }
    match profile {
        merman_core::resources::ResourceProfile::Interactive => Some(256),
        merman_core::resources::ResourceProfile::Constrained => Some(128),
        merman_core::resources::ResourceProfile::TrustedNative => Some(1_024),
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput => None,
    }
}

/// Complete analysis configuration, split by generation and diagnostic invalidation scope.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnalysisOptions {
    /// Inputs that require a new canonical parse generation when changed.
    snapshot: AnalysisSnapshotPolicy,
    /// Inputs that can reproject diagnostics from an existing canonical generation.
    diagnostics: AnalysisDiagnosticPolicy,
}

/// Inputs frozen into one canonical analysis generation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AnalysisSnapshotPolicy {
    /// Host document identity and source kind.
    pub source: SourceDescriptor,
    /// Mermaid site configuration applied from pinned defaults.
    pub site_config: Option<MermaidConfig>,
    /// Deterministic or host-backed runtime values used during parsing.
    pub runtime_policy: merman_core::runtime::RuntimePolicy,
    /// Deterministic admission budgets enforced before parser-owned work begins.
    pub resources: AnalysisResourceLimits,
}

/// Lightweight analysis admission budgets that are safe to copy into worker tickets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnalysisResourceLimits {
    max_source_bytes: Option<usize>,
    max_document_diagrams: Option<usize>,
}

impl AnalysisResourceLimits {
    pub const fn max_source_bytes(self) -> Option<usize> {
        self.max_source_bytes
    }

    pub const fn max_document_diagrams(self) -> Option<usize> {
        self.max_document_diagrams
    }

    pub fn preflight_document(
        self,
        source: &str,
        descriptor: &SourceDescriptor,
    ) -> Option<AnalysisRejection> {
        let cancellation = AnalysisCancellationToken::new();
        self.preflight_document_cancellable(source, descriptor, &cancellation)
            .expect("a private analysis cancellation token cannot be cancelled")
    }

    pub fn preflight_document_cancellable(
        self,
        source: &str,
        descriptor: &SourceDescriptor,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Option<AnalysisRejection>, AnalysisCancelled> {
        cancellation.checkpoint()?;
        if let Some(rejection) = crate::source_limits::source_limit_rejection_cancellable(
            source,
            descriptor,
            self.max_source_bytes,
            cancellation,
        )? {
            return Ok(Some(rejection));
        }
        crate::document_limits::document_diagram_limit_rejection_cancellable(
            source,
            descriptor,
            self.max_document_diagrams,
            cancellation,
        )
    }
}

/// Inputs that reproject diagnostics without rebuilding parser-owned facts.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct AnalysisDiagnosticPolicy {
    /// Enabled rules, profiles, and severity overrides.
    pub rule_config: AnalysisRuleConfig,
}

impl Default for AnalysisSnapshotPolicy {
    fn default() -> Self {
        Self {
            source: SourceDescriptor::diagram(),
            site_config: None,
            runtime_policy: merman_core::runtime::RuntimePolicy::deterministic(),
            resources: AnalysisResourceLimits::default(),
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

    pub fn with_fixed_today(mut self, today: Option<merman_core::time::CivilDate>) -> Self {
        self.snapshot.runtime_policy = self.snapshot.runtime_policy.with_fixed_today(today);
        self
    }

    pub fn try_with_fixed_today_at_local_midnight(
        mut self,
        today: merman_core::time::CivilDate,
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
        self.snapshot.resources.max_source_bytes = max_source_bytes;
        self
    }

    pub fn with_max_document_diagrams(mut self, max_document_diagrams: Option<usize>) -> Self {
        self.snapshot.resources.max_document_diagrams = max_document_diagrams;
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
        self.snapshot.resources.max_source_bytes()
    }

    pub fn max_document_diagrams(&self) -> Option<usize> {
        self.snapshot.resources.max_document_diagrams()
    }

    pub const fn resource_limits(&self) -> AnalysisResourceLimits {
        self.snapshot.resources
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
        if self.options.source() == &source {
            return self.clone();
        }
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

    pub(crate) fn runtime_policy_candidates(
        &self,
        error: merman_core::runtime::RuntimePolicyError,
        source_map: &SourceMap,
    ) -> Vec<DiagnosticCandidate> {
        vec![core_error_candidate(&CoreError::from(error), source_map).candidate]
    }

    pub fn analyze_generation(&self, source: &str) -> AnalysisCaptureOutcome {
        if matches!(
            self.options.source().kind,
            SourceKind::Markdown | SourceKind::Mdx
        ) {
            return crate::document::analyze_document_generation(
                source,
                self,
                self.options.source().clone(),
            );
        }
        if let Some(rejection) = self.source_limit_rejection(source, self.options.source().clone())
        {
            return AnalysisCaptureOutcome::Rejected(rejection);
        }

        self.analyze_generation_shared_preflighted(Arc::from(source))
    }

    /// Captures a generation while retaining the caller's immutable source allocation.
    pub fn analyze_generation_shared(&self, source: Arc<str>) -> AnalysisCaptureOutcome {
        if matches!(
            self.options.source().kind,
            SourceKind::Markdown | SourceKind::Mdx
        ) {
            return crate::document::analyze_document_generation_shared(
                source,
                self,
                self.options.source().clone(),
            );
        }
        if let Some(rejection) =
            self.source_limit_rejection(source.as_ref(), self.options.source().clone())
        {
            return AnalysisCaptureOutcome::Rejected(rejection);
        }

        self.analyze_generation_shared_preflighted(source)
    }

    fn analyze_generation_shared_preflighted(
        &self,
        source_text: Arc<str>,
    ) -> AnalysisCaptureOutcome {
        let source_map = SourceMap::new(Arc::clone(&source_text));
        let operation_analyzer = match self.try_for_operation() {
            Ok(analyzer) => analyzer,
            Err(error) => {
                let candidates = self.runtime_policy_candidates(error, &source_map);
                return AnalysisCaptureOutcome::Ready(
                    AnalysisGeneration::new(source_map, Vec::new(), self)
                        .with_document_candidates(candidates),
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

    /// Captures a generation cooperatively without promoting borrowed text inside the operation.
    pub fn analyze_generation_shared_cancellable(
        &self,
        source: Arc<str>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<AnalysisCaptureOutcome, AnalysisCancelled> {
        cancellation.checkpoint()?;
        if matches!(
            self.options.source().kind,
            SourceKind::Markdown | SourceKind::Mdx
        ) {
            return crate::document::analyze_document_generation_shared_cancellable(
                source,
                self,
                self.options.source().clone(),
                cancellation,
            );
        }
        if let Some(rejection) = crate::source_limits::source_limit_rejection_cancellable(
            source.as_ref(),
            self.options.source(),
            self.options.max_source_bytes(),
            cancellation,
        )? {
            return Ok(AnalysisCaptureOutcome::Rejected(rejection));
        }

        let source_map = SourceMap::new_cancellable(Arc::clone(&source), cancellation)?;
        let operation_analyzer = match self.try_for_operation() {
            Ok(analyzer) => analyzer,
            Err(error) => {
                let candidates = self.runtime_policy_candidates(error, &source_map);
                return Ok(AnalysisCaptureOutcome::Ready(
                    AnalysisGeneration::new(source_map, Vec::new(), self)
                        .with_document_candidates(candidates),
                ));
            }
        };
        let diagram = crate::document::whole_document_diagram(source, self.options.source());
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
        if matches!(
            self.options.source().kind,
            SourceKind::Markdown | SourceKind::Mdx
        ) {
            return crate::document::analyze_document(source, self, self.options.source().clone());
        }
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

    pub(crate) fn analyze_diagram(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
    ) -> AnalyzedDiagram {
        let source_map = source_map_for_diagram(diagram, document_source_map);
        let evidence = self.capture_evidence(diagram.text.as_str());
        let captured = self.capture_from_evidence(
            diagram.text.as_str(),
            &source_map,
            &evidence,
            CaptureMode::RichFacts,
        );
        let candidates = crate::document::normalize_document_diagnostic_candidates(
            document_source_map,
            diagram,
            captured.candidates,
        );
        AnalyzedDiagram::from_document_diagram(
            diagram,
            captured.syntax,
            candidates,
            captured.parse_disposition,
        )
    }

    pub(crate) fn analyze_diagram_cancellable(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<AnalyzedDiagram, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let source_map =
            source_map_for_diagram_cancellable(diagram, document_source_map, cancellation)?;
        let evidence = self.capture_evidence_cancellable(diagram.text.as_str(), cancellation)?;
        cancellation.checkpoint()?;
        let captured = self.capture_from_evidence_cancellable(
            diagram.text.as_str(),
            &source_map,
            &evidence,
            CaptureMode::RichFacts,
            cancellation,
        )?;
        cancellation.checkpoint()?;
        let candidates = crate::document::normalize_document_diagnostic_candidates_cancellable(
            document_source_map,
            diagram,
            captured.candidates,
            cancellation,
        )?;
        cancellation.checkpoint()?;
        Ok(AnalyzedDiagram::from_document_diagram(
            diagram,
            captured.syntax,
            candidates,
            captured.parse_disposition,
        ))
    }

    pub(crate) fn analyze_diagram_diagnostics(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
    ) -> Vec<AnalysisDiagnostic> {
        let captured =
            self.capture_diagram_local(diagram, document_source_map, CaptureMode::DiagnosticsOnly);
        let candidates = crate::document::normalize_document_diagnostic_candidates(
            document_source_map,
            diagram,
            captured.candidates,
        );
        materialize_diagnostic_candidates(&candidates, self.options.diagnostic_policy())
    }

    pub(crate) fn analyze_source_diagnostics(&self, source: &str) -> Vec<AnalysisDiagnostic> {
        let captured = self.capture_local(source, CaptureMode::DiagnosticsOnly);
        captured.project_diagnostics(self.options.diagnostic_policy())
    }

    fn capture_local(&self, source: &str, mode: CaptureMode) -> CapturedDiagram {
        let source_map = SourceMap::new(source);
        self.capture_local_with_source_map(source, &source_map, mode)
    }

    fn capture_diagram_local(
        &self,
        diagram: &DocumentDiagram,
        document_source_map: &SourceMap,
        mode: CaptureMode,
    ) -> CapturedDiagram {
        let source_map = source_map_for_diagram(diagram, document_source_map);
        let evidence = self.capture_evidence(diagram.text.as_str());
        self.capture_from_evidence(diagram.text.as_str(), &source_map, &evidence, mode)
    }

    fn capture_local_with_source_map(
        &self,
        source: &str,
        source_map: &SourceMap,
        mode: CaptureMode,
    ) -> CapturedDiagram {
        let evidence = self.capture_evidence(source);
        self.capture_from_evidence(source, source_map, &evidence, mode)
    }

    fn capture_evidence(&self, source: &str) -> DiagramAnalysisEvidence {
        let cancellation = AnalysisCancellationToken::new();
        self.capture_evidence_cancellable(source, &cancellation)
            .unwrap_or_else(|_| DiagramAnalysisEvidence::OperationError {
                error: CoreError::from(merman_core::ParseCancelled),
                source_config: SourceConfigEvidence::default(),
            })
    }

    fn capture_evidence_cancellable(
        &self,
        source: &str,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<DiagramAnalysisEvidence, AnalysisCancelled> {
        cancellation.checkpoint()?;
        if source_is_blank_cancellable(source, cancellation)? {
            return Ok(DiagramAnalysisEvidence::EmptySource {
                source_config: SourceConfigEvidence::default(),
            });
        }
        let parse_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.engine
                .capture_diagram_snapshot_controlled_sync(source, cancellation.parse_control())
        }));
        let evidence = match parse_result {
            Err(panic_payload) => DiagramAnalysisEvidence::Panic {
                message: panic_message(panic_payload.as_ref()).to_string(),
                metadata: None,
                source_config: SourceConfigEvidence::default(),
            },
            Ok(Err(_)) => return Err(AnalysisCancelled),
            Ok(Ok(parse_result)) => match parse_result {
                DiagramSnapshotCapture::Snapshot(Some(snapshot)) => {
                    let (meta, outcome, editor_facts, source_config) =
                        snapshot.into_parts_with_source_config();
                    let editor_facts = match editor_facts {
                        ParsedEditorFacts::Available(facts) => Some(facts),
                        ParsedEditorFacts::Unavailable => None,
                    };
                    match outcome {
                        DiagramParseOutcome::Parsed {
                            model,
                            warning_facts,
                        } => DiagramAnalysisEvidence::Parsed {
                            metadata: meta,
                            model,
                            warning_facts,
                            editor_facts,
                            source_config,
                        },
                        DiagramParseOutcome::Failed(error) => {
                            DiagramAnalysisEvidence::ParseFailed {
                                metadata: meta,
                                error,
                                editor_facts,
                                source_config,
                            }
                        }
                        DiagramParseOutcome::Panicked(message) => DiagramAnalysisEvidence::Panic {
                            message,
                            metadata: Some(meta),
                            source_config,
                        },
                    }
                }
                DiagramSnapshotCapture::Snapshot(None) => DiagramAnalysisEvidence::NoSnapshot {
                    source_config: SourceConfigEvidence::default(),
                },
                DiagramSnapshotCapture::Failed {
                    error,
                    source_config,
                } => DiagramAnalysisEvidence::OperationError {
                    error,
                    source_config,
                },
            },
        };
        cancellation.checkpoint()?;
        Ok(evidence)
    }

    fn capture_from_evidence(
        &self,
        source: &str,
        source_map: &SourceMap,
        evidence: &DiagramAnalysisEvidence,
        mode: CaptureMode,
    ) -> CapturedDiagram {
        let cancellation = AnalysisCancellationToken::new();
        self.capture_from_evidence_cancellable(source, source_map, evidence, mode, &cancellation)
            .expect("a private analysis cancellation token cannot be cancelled")
    }

    fn capture_from_evidence_cancellable(
        &self,
        source: &str,
        source_map: &SourceMap,
        evidence: &DiagramAnalysisEvidence,
        mode: CaptureMode,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<CapturedDiagram, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let source_lints = crate::rules::source_lint_candidates_cancellable(
            source,
            source_map,
            evidence.config_for_rewrite(),
            evidence.source_config(),
            cancellation,
        )?;
        cancellation.checkpoint()?;
        match evidence {
            DiagramAnalysisEvidence::EmptySource { .. } => {
                let candidates = vec![no_diagram_candidate(source_map)];
                Self::finish_capture(
                    candidates,
                    mode.unavailable_syntax(None),
                    DiagramParseDisposition::Unavailable,
                    cancellation,
                )
            }
            DiagramAnalysisEvidence::Panic { message, .. } => {
                let mut candidates = source_lints;
                candidates.push(panic_candidate(message, source_map));
                Self::finish_capture(
                    candidates,
                    mode.unavailable_syntax(None),
                    DiagramParseDisposition::Unavailable,
                    cancellation,
                )
            }
            DiagramAnalysisEvidence::NoSnapshot { .. } => Self::finish_capture(
                source_lints,
                mode.unavailable_syntax(None),
                DiagramParseDisposition::Unavailable,
                cancellation,
            ),
            DiagramAnalysisEvidence::OperationError { error, .. } => {
                let projection = core_error_candidate(error, source_map);
                let mut candidates = source_lints;
                candidates.push(projection.candidate);
                Self::finish_capture(
                    candidates,
                    mode.unavailable_syntax(projection.diagram_type),
                    DiagramParseDisposition::Unavailable,
                    cancellation,
                )
            }
            DiagramAnalysisEvidence::Parsed {
                metadata,
                model,
                warning_facts,
                editor_facts,
                source_config,
                ..
            } => self.analyze_parsed_diagram_cancellable(
                DiagramCaptureInput {
                    source_map,
                    metadata,
                    editor_facts: editor_facts.as_ref(),
                },
                ParsedDiagramCaptureInput {
                    model,
                    warning_facts,
                    source_config,
                },
                source_lints,
                mode,
                cancellation,
            ),
            DiagramAnalysisEvidence::ParseFailed {
                metadata,
                error,
                editor_facts,
                ..
            } => self.analyze_parse_error_cancellable(
                DiagramCaptureInput {
                    source_map,
                    metadata,
                    editor_facts: editor_facts.as_ref(),
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
        input: DiagramCaptureInput<'_>,
        model: &serde_json::Value,
        candidates: Vec<DiagnosticCandidate>,
        mode: CaptureMode,
    ) -> CapturedDiagram {
        let cancellation = AnalysisCancellationToken::new();
        let source_config = SourceConfigEvidence::default();
        self.analyze_parsed_diagram_cancellable(
            input,
            ParsedDiagramCaptureInput {
                model,
                warning_facts: &[],
                source_config: &source_config,
            },
            candidates,
            mode,
            &cancellation,
        )
        .expect("a private analysis cancellation token cannot be cancelled")
    }

    fn analyze_parsed_diagram_cancellable(
        &self,
        input: DiagramCaptureInput<'_>,
        parsed: ParsedDiagramCaptureInput<'_>,
        mut candidates: Vec<DiagnosticCandidate>,
        mode: CaptureMode,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<CapturedDiagram, AnalysisCancelled> {
        let DiagramCaptureInput {
            source_map,
            metadata,
            editor_facts,
        } = input;
        let ParsedDiagramCaptureInput {
            model,
            warning_facts,
            source_config,
        } = parsed;
        cancellation.checkpoint()?;
        let diagram_type = metadata.diagram_type.as_str();
        candidates.extend(crate::rules::parsed_source_lint_candidates_cancellable(
            source_map,
            diagram_type,
            source_config,
            cancellation,
        )?);
        candidates.extend(crate::rules::semantic_warning_candidates_cancellable(
            diagram_type,
            warning_facts,
            source_map,
            cancellation,
        )?);
        let editor_candidates = Self::editor_recovery_candidates_from_facts_cancellable(
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
        candidates.extend(
            flowchart_projection
                .as_ref()
                .into_iter()
                .flat_map(|projection| projection.candidates.iter().cloned()),
        );

        candidates.extend(editor_candidates);
        let syntax = match mode {
            CaptureMode::DiagnosticsOnly => {
                AnalysisSyntaxFacts::new(Some(diagram_type.to_string()), FenceTextIndex::default())
            }
            CaptureMode::RichFacts => {
                let effective_layout = metadata
                    .effective_config
                    .get_str("layout")
                    .map(str::to_owned);
                AnalysisSyntaxFacts::new(
                    Some(diagram_type.to_string()),
                    Self::editor_text_index_from_facts_cancellable(editor_facts, cancellation)?,
                )
                .with_effective_layout(effective_layout)
                .with_flowchart(flowchart_projection.and_then(|projection| projection.facts))
            }
        };
        Self::finish_capture(
            candidates,
            syntax,
            DiagramParseDisposition::Parsed,
            cancellation,
        )
    }

    fn analyze_parse_error_cancellable(
        &self,
        input: DiagramCaptureInput<'_>,
        mut candidates: Vec<DiagnosticCandidate>,
        error: &CoreError,
        mode: CaptureMode,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<CapturedDiagram, AnalysisCancelled> {
        let DiagramCaptureInput {
            source_map,
            metadata: meta,
            editor_facts,
        } = input;
        cancellation.checkpoint()?;
        let core_diagnostic = core_error_candidate(error, source_map);
        candidates.push(
            core_diagnostic
                .candidate
                .with_parse_location(core_diagnostic.parse_location),
        );
        let diagram_type = meta.diagram_type.as_str();
        candidates.extend(Self::editor_recovery_candidates_from_facts_cancellable(
            diagram_type,
            source_map,
            editor_facts,
            cancellation,
        )?);
        let syntax = match mode {
            CaptureMode::DiagnosticsOnly => {
                AnalysisSyntaxFacts::new(Some(diagram_type.to_string()), FenceTextIndex::default())
            }
            CaptureMode::RichFacts => {
                let effective_layout = meta.effective_config.get_str("layout").map(str::to_owned);
                AnalysisSyntaxFacts::new(
                    Some(diagram_type.to_string()),
                    Self::editor_text_index_from_facts_cancellable(editor_facts, cancellation)?,
                )
                .with_effective_layout(effective_layout)
            }
        };
        Self::finish_capture(
            candidates,
            syntax,
            DiagramParseDisposition::Recovered,
            cancellation,
        )
    }

    fn editor_recovery_candidates_from_facts_cancellable(
        diagram_type: &str,
        source_map: &SourceMap,
        facts: Option<&EditorSemanticFacts>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Vec<DiagnosticCandidate>, AnalysisCancelled> {
        let Some(facts) = facts else {
            return Ok(Vec::new());
        };

        let mut candidates = Vec::with_capacity(facts.diagnostics.len());
        for batch in facts.diagnostics.chunks(128) {
            cancellation.checkpoint()?;
            candidates.extend(editor_recovery_candidates(
                batch.iter().cloned(),
                diagram_type,
                source_map,
            ));
        }
        cancellation.checkpoint()?;
        Ok(candidates)
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
                    candidates: Vec::new(),
                },
                Err(error) => {
                    let _ = source_map.whole_source_span_cancellable(cancellation)?;
                    FlowchartFactsProjection {
                        facts: None,
                        candidates: vec![flowchart_facts_projection_candidate(
                            error,
                            diagram_type,
                            source_map,
                        )],
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

    fn finish_capture(
        candidates: Vec<DiagnosticCandidate>,
        syntax: AnalysisSyntaxFacts,
        parse_disposition: DiagramParseDisposition,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<CapturedDiagram, AnalysisCancelled> {
        cancellation.checkpoint()?;
        Ok(CapturedDiagram {
            candidates,
            syntax,
            parse_disposition,
        })
    }
}

fn source_map_for_diagram(diagram: &DocumentDiagram, document_source_map: &SourceMap) -> SourceMap {
    if diagram.is_fence() {
        SourceMap::from_shared_text(diagram.text.clone())
    } else {
        document_source_map.clone()
    }
}

fn source_map_for_diagram_cancellable(
    diagram: &DocumentDiagram,
    document_source_map: &SourceMap,
    cancellation: &AnalysisCancellationToken,
) -> Result<SourceMap, AnalysisCancelled> {
    if diagram.is_fence() {
        SourceMap::from_shared_text_cancellable(diagram.text.clone(), cancellation)
    } else {
        cancellation.checkpoint()?;
        Ok(document_source_map.clone())
    }
}

fn source_is_blank_cancellable(
    source: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    const CHECKPOINT_INTERVAL_BYTES: usize = 4 * 1024;

    cancellation.checkpoint()?;
    let mut checkpoint_offset = 0;
    for (offset, character) in source.char_indices() {
        if offset.saturating_sub(checkpoint_offset) >= CHECKPOINT_INTERVAL_BYTES {
            cancellation.checkpoint()?;
            checkpoint_offset = offset;
        }
        if !character.is_whitespace() {
            return Ok(false);
        }
    }
    cancellation.checkpoint()?;
    Ok(true)
}

#[derive(Debug)]
enum DiagramAnalysisEvidence {
    EmptySource {
        source_config: SourceConfigEvidence,
    },
    Panic {
        message: String,
        metadata: Option<ParseMetadata>,
        source_config: SourceConfigEvidence,
    },
    NoSnapshot {
        source_config: SourceConfigEvidence,
    },
    OperationError {
        error: CoreError,
        source_config: SourceConfigEvidence,
    },
    Parsed {
        metadata: ParseMetadata,
        model: serde_json::Value,
        warning_facts: Vec<DiagramWarningFact>,
        editor_facts: Option<EditorSemanticFacts>,
        source_config: SourceConfigEvidence,
    },
    ParseFailed {
        metadata: ParseMetadata,
        error: CoreError,
        editor_facts: Option<EditorSemanticFacts>,
        source_config: SourceConfigEvidence,
    },
}

impl DiagramAnalysisEvidence {
    fn source_config(&self) -> &SourceConfigEvidence {
        match self {
            Self::EmptySource { source_config }
            | Self::Panic { source_config, .. }
            | Self::NoSnapshot { source_config }
            | Self::OperationError { source_config, .. }
            | Self::Parsed { source_config, .. }
            | Self::ParseFailed { source_config, .. } => source_config,
        }
    }

    fn config_for_rewrite(&self) -> Option<&MermaidConfig> {
        match self {
            Self::Parsed {
                metadata,
                source_config,
                ..
            }
            | Self::ParseFailed {
                metadata,
                source_config,
                ..
            } if source_config.rewrite_safe() => Some(&metadata.config),
            Self::Panic {
                metadata: Some(metadata),
                source_config,
                ..
            } if source_config.rewrite_safe() => Some(&metadata.config),
            Self::EmptySource { .. }
            | Self::NoSnapshot { .. }
            | Self::OperationError { .. }
            | Self::Panic { .. }
            | Self::Parsed { .. }
            | Self::ParseFailed { .. } => None,
        }
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
enum CaptureMode {
    DiagnosticsOnly,
    RichFacts,
}

#[derive(Clone, Copy)]
struct DiagramCaptureInput<'a> {
    source_map: &'a SourceMap,
    metadata: &'a ParseMetadata,
    editor_facts: Option<&'a EditorSemanticFacts>,
}

#[derive(Clone, Copy)]
struct ParsedDiagramCaptureInput<'a> {
    model: &'a serde_json::Value,
    warning_facts: &'a [DiagramWarningFact],
    source_config: &'a SourceConfigEvidence,
}

impl CaptureMode {
    fn unavailable_syntax(self, diagram_type: Option<String>) -> AnalysisSyntaxFacts {
        AnalysisSyntaxFacts::unavailable(diagram_type)
    }
}

#[derive(Debug, Clone)]
struct CapturedDiagram {
    candidates: Vec<DiagnosticCandidate>,
    syntax: AnalysisSyntaxFacts,
    parse_disposition: DiagramParseDisposition,
}

impl CapturedDiagram {
    fn project_diagnostics(&self, policy: &AnalysisDiagnosticPolicy) -> Vec<AnalysisDiagnostic> {
        materialize_diagnostic_candidates(&self.candidates, policy)
    }
}

#[derive(Debug, Clone)]
struct FlowchartFactsProjection {
    facts: Option<AnalysisFlowchartFacts>,
    candidates: Vec<DiagnosticCandidate>,
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
