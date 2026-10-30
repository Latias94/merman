#[cfg(feature = "fixes")]
use crate::repair::{FixProjection, RepairEvidence};
use crate::rules::INTERNAL_RULE_REGISTRY_GAP_RULE;
use crate::rules::{
    DIAGRAM_PARSE_RULE, DIAGRAM_PARSE_RULE_ID, INTERNAL_FAILURE_RULE, INVALID_DIRECTIVE_JSON_RULE,
    INVALID_FRONT_MATTER_YAML_RULE, MALFORMED_FRONT_MATTER_RULE, NO_DIAGRAM_RULE, PANIC_RULE,
    PARSER_CONTRACT_VIOLATION_RULE, RuleDescriptor, UNSUPPORTED_DIAGRAM_RULE, rule_descriptor,
};
#[cfg(feature = "compat-json")]
use crate::rules::{INVALID_THEME_COLOR_RULE, THEME_EVALUATION_LIMIT_RULE};
use crate::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisDiagnostic, AnalysisDiagnosticPolicy,
    AnalysisStatus, DiagnosticRelated, DiagnosticSpan, SourceMap,
};
use merman_core::{
    EditorSemanticDiagnosticKind, Error as CoreError, ParseDiagnostic, ParseDiagnosticSpanKind,
};

const NO_DIAGRAM_MESSAGE: &str = "no Mermaid diagram detected";

/// Policy-neutral diagnostic meaning retained by an analysis generation.
#[derive(Debug, Clone)]
pub(crate) struct DiagnosticCandidate {
    descriptor: RuleDescriptor,
    status: Option<AnalysisStatus>,
    message: String,
    diagram_type: Option<String>,
    span: Option<DiagnosticSpan>,
    related: Vec<DiagnosticRelated>,
    help: Option<String>,
    #[cfg(feature = "fixes")]
    repair: Option<RepairEvidence>,
    suppressors: &'static [RuleDescriptor],
    parse_location: Option<ParseDiagnosticLocation>,
    recovery_kind: Option<EditorSemanticDiagnosticKind>,
    trailing_source_context_count: usize,
}

impl DiagnosticCandidate {
    pub(crate) fn new(descriptor: RuleDescriptor, message: impl Into<String>) -> Self {
        Self {
            descriptor,
            status: None,
            message: message.into(),
            diagram_type: None,
            span: None,
            related: Vec::new(),
            help: None,
            #[cfg(feature = "fixes")]
            repair: None,
            suppressors: &[],
            parse_location: None,
            recovery_kind: None,
            trailing_source_context_count: 0,
        }
    }

    pub(crate) const fn with_status(mut self, status: AnalysisStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub(crate) fn with_diagram_type(mut self, diagram_type: impl Into<String>) -> Self {
        self.diagram_type = Some(diagram_type.into());
        self
    }

    pub(crate) const fn with_span(mut self, span: DiagnosticSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub(crate) fn with_related(mut self, related: DiagnosticRelated) -> Self {
        self.related.push(related);
        self
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    #[cfg(feature = "fixes")]
    pub(crate) fn with_repair(mut self, repair: RepairEvidence) -> Self {
        self.repair = Some(repair);
        self
    }

    #[cfg(feature = "fixes")]
    pub(crate) fn set_repair(&mut self, repair: RepairEvidence) {
        self.repair = Some(repair);
    }

    #[cfg(feature = "fixes")]
    pub(crate) fn map_repair_to_document(
        mut self,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        if let Some(repair) = self.repair.take() {
            self.repair = repair.map_to_document(source_map, body_start, cancellation)?;
        }
        Ok(self)
    }

    pub(crate) fn with_suppressors(mut self, suppressors: &'static [RuleDescriptor]) -> Self {
        self.suppressors = suppressors;
        self
    }

    pub(crate) fn with_parse_location(
        mut self,
        parse_location: Option<ParseDiagnosticLocation>,
    ) -> Self {
        self.parse_location = parse_location;
        self
    }

    pub(crate) fn with_recovery_kind(
        mut self,
        recovery_kind: EditorSemanticDiagnosticKind,
    ) -> Self {
        self.recovery_kind = Some(recovery_kind);
        self
    }

    pub(crate) fn try_map_locations<E>(
        mut self,
        map: impl FnOnce(&mut Option<DiagnosticSpan>, &mut Vec<DiagnosticRelated>) -> Result<(), E>,
    ) -> Result<Self, E> {
        map(&mut self.span, &mut self.related)?;
        Ok(self)
    }

    pub(crate) fn with_trailing_source_context(
        mut self,
        context: crate::DiagnosticRelated,
    ) -> Self {
        self.related.push(context);
        self.trailing_source_context_count = self.trailing_source_context_count.saturating_add(1);
        self
    }

    pub(crate) fn add_estimated_owned_heap_bytes(
        &self,
        weight: &mut crate::payload::DiagnosticRetainedWeight,
    ) {
        weight.add_candidate(crate::payload::DiagnosticDynamicWeight {
            tags_capacity: 0,
            message_capacity: self.message.capacity(),
            diagram_type_capacity: self.diagram_type.as_ref().map(String::capacity),
            help_capacity: self.help.as_ref().map(String::capacity),
            related: &self.related,
            related_capacity: self.related.capacity(),
            fixes: &[],
            fixes_capacity: 0,
        });
        #[cfg(feature = "fixes")]
        if let Some(repair) = &self.repair {
            weight.add_repair_evidence(repair);
        }
    }

    fn materialize(&self, severity: crate::DiagnosticSeverity) -> AnalysisDiagnostic {
        AnalysisDiagnostic {
            id: self.descriptor.id.to_string(),
            severity,
            category: self.descriptor.category,
            tags: self.descriptor.tags.to_vec(),
            message: self.message.clone(),
            code: self.status.map(AnalysisStatus::code),
            code_name: self.status.map(|status| status.code_name().to_string()),
            diagram_type: self.diagram_type.clone(),
            span: self.span,
            related: self.related.clone(),
            help: self.help.clone(),
            fixes: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) const fn rule_id(&self) -> &'static str {
        self.descriptor.id
    }
}

#[cfg(test)]
pub(crate) fn project_diagnostic_candidates(
    candidates: &[DiagnosticCandidate],
    policy: &AnalysisDiagnosticPolicy,
    cancellation: &AnalysisCancellationToken,
) -> Result<Vec<AnalysisDiagnostic>, AnalysisCancelled> {
    let mut diagnostics: Vec<AnalysisDiagnostic> = Vec::with_capacity(candidates.len());
    append_projected_diagnostic_candidates(&mut diagnostics, candidates, policy, cancellation)?;
    Ok(diagnostics)
}

pub(crate) fn append_projected_diagnostic_candidates(
    diagnostics: &mut Vec<AnalysisDiagnostic>,
    candidates: &[DiagnosticCandidate],
    policy: &AnalysisDiagnosticPolicy,
    cancellation: &AnalysisCancellationToken,
) -> Result<(), AnalysisCancelled> {
    let rule_config = &policy.rule_config;
    let mut primary_parse: Option<ProjectedPrimaryParse> = None;
    #[cfg(feature = "fixes")]
    let mut fixes = FixProjection::default();

    for (index, candidate) in candidates.iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        if !candidate_enabled(candidate, rule_config) {
            continue;
        }

        let diagnostic = candidate.materialize(rule_config.severity_for(candidate.descriptor));
        #[cfg(feature = "fixes")]
        let mut diagnostic = diagnostic;
        #[cfg(feature = "fixes")]
        if policy.fix_generation == crate::FixGeneration::Generate
            && let Some(repair) = &candidate.repair
            && let Some(fix) = fixes.materialize(repair, cancellation)?
        {
            diagnostic.fixes.push(fix);
        }
        if let Some(kind) = candidate.recovery_kind {
            let merged = primary_parse.is_some_and(|primary: ProjectedPrimaryParse| {
                crate::recovery::merge_duplicate_parse_recovery_diagnostic(
                    &mut diagnostics[primary.diagnostic_index],
                    primary.trailing_source_context_count,
                    &diagnostic,
                    kind,
                    primary.parse_location,
                )
            });
            if merged {
                continue;
            }
            diagnostics.push(diagnostic);
            continue;
        }

        if candidate.descriptor.id == DIAGRAM_PARSE_RULE_ID {
            debug_assert!(
                primary_parse.is_none(),
                "one captured diagram must emit at most one primary parse diagnostic"
            );
            if primary_parse.is_none() {
                // Recovery candidates belong to this same captured diagram, so retaining the
                // primary slot turns every later recovery merge into constant-time lookup.
                primary_parse = Some(ProjectedPrimaryParse {
                    diagnostic_index: diagnostics.len(),
                    parse_location: candidate.parse_location,
                    trailing_source_context_count: candidate.trailing_source_context_count,
                });
            }
        }
        diagnostics.push(diagnostic);
    }
    cancellation.checkpoint()?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ProjectedPrimaryParse {
    diagnostic_index: usize,
    parse_location: Option<ParseDiagnosticLocation>,
    trailing_source_context_count: usize,
}

#[cfg(test)]
pub(crate) fn materialize_diagnostic_candidates(
    candidates: &[DiagnosticCandidate],
    policy: &AnalysisDiagnosticPolicy,
) -> Vec<AnalysisDiagnostic> {
    let cancellation = AnalysisCancellationToken::new();
    project_diagnostic_candidates(candidates, policy, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

pub(crate) fn append_diagnostic_candidates_cancellable(
    target: &mut Vec<DiagnosticCandidate>,
    candidates: Vec<DiagnosticCandidate>,
    cancellation: &AnalysisCancellationToken,
) -> Result<(), AnalysisCancelled> {
    for (index, candidate) in candidates.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        target.push(candidate);
    }
    cancellation.checkpoint()?;
    Ok(())
}

fn candidate_enabled(
    candidate: &DiagnosticCandidate,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> bool {
    rule_config.is_rule_enabled(candidate.descriptor)
        && !candidate
            .suppressors
            .iter()
            .copied()
            .any(|descriptor| rule_config.is_rule_enabled(descriptor))
}

#[derive(Debug)]
pub(crate) struct CoreErrorCandidate {
    pub(crate) candidate: DiagnosticCandidate,
    pub(crate) diagram_type: Option<String>,
    pub(crate) parse_location: Option<ParseDiagnosticLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseDiagnosticLocation {
    Precise,
    Fallback,
}

struct ParseDiagnosticProjection {
    candidate: DiagnosticCandidate,
    location: ParseDiagnosticLocation,
}

pub(crate) fn core_error_candidate(
    error: &CoreError,
    source_map: &SourceMap,
) -> CoreErrorCandidate {
    match error {
        CoreError::OperationCancelled(error) => CoreErrorCandidate {
            candidate: rule_candidate(
                PARSER_CONTRACT_VIOLATION_RULE,
                AnalysisStatus::InternalError,
                format!(
                    "custom parser returned cancellation to a non-cancellable analysis facade: {error}"
                ),
                source_map,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::Internal(error) => CoreErrorCandidate {
            candidate: rule_candidate_without_default_span(
                INTERNAL_FAILURE_RULE,
                AnalysisStatus::InternalError,
                error.message(),
            ),
            diagram_type: None,
            parse_location: None,
        },
        #[cfg(feature = "compat-json")]
        CoreError::ThemeColor(error) => CoreErrorCandidate {
            candidate: rule_candidate_without_default_span(
                INVALID_THEME_COLOR_RULE,
                AnalysisStatus::ParseError,
                error.to_string(),
            ),
            diagram_type: None,
            parse_location: None,
        },
        #[cfg(feature = "compat-json")]
        CoreError::ThemeEvaluationLimit(error) => CoreErrorCandidate {
            candidate: rule_candidate_without_default_span(
                THEME_EVALUATION_LIMIT_RULE,
                AnalysisStatus::ResourceLimitExceeded,
                error.to_string(),
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::RuntimePolicy(error) => CoreErrorCandidate {
            candidate: rule_candidate(
                DIAGRAM_PARSE_RULE,
                AnalysisStatus::ParseError,
                error.to_string(),
                source_map,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::DetectType(_) => CoreErrorCandidate {
            candidate: no_diagram_candidate(source_map),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::UnsupportedDiagram { diagram_type } => CoreErrorCandidate {
            candidate: rule_candidate(
                UNSUPPORTED_DIAGRAM_RULE,
                AnalysisStatus::UnsupportedFormat,
                format!("unsupported diagram type: {diagram_type}"),
                source_map,
            )
            .with_diagram_type(diagram_type.clone()),
            diagram_type: Some(diagram_type.clone()),
            parse_location: None,
        },
        CoreError::DiagramParse {
            diagram_type,
            diagnostic,
        } => {
            let projection = parse_diagnostic(diagnostic, diagram_type, source_map);
            CoreErrorCandidate {
                candidate: projection.candidate,
                diagram_type: Some(diagram_type.clone()),
                parse_location: Some(projection.location),
            }
        }
        CoreError::MalformedFrontMatter => CoreErrorCandidate {
            candidate: rule_candidate(
                MALFORMED_FRONT_MATTER_RULE,
                AnalysisStatus::ParseError,
                CoreError::MalformedFrontMatter.to_string(),
                source_map,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::InvalidDirectiveJson { message } => CoreErrorCandidate {
            candidate: rule_candidate(
                INVALID_DIRECTIVE_JSON_RULE,
                AnalysisStatus::ParseError,
                format!("invalid directive JSON: {message}"),
                source_map,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::InvalidFrontMatterYaml { message } => CoreErrorCandidate {
            candidate: rule_candidate(
                INVALID_FRONT_MATTER_YAML_RULE,
                AnalysisStatus::ParseError,
                format!("invalid YAML front-matter: {message}"),
                source_map,
            ),
            diagram_type: None,
            parse_location: None,
        },
        _ => CoreErrorCandidate {
            candidate: rule_candidate_without_default_span(
                INTERNAL_RULE_REGISTRY_GAP_RULE,
                AnalysisStatus::InternalError,
                "unclassified core error while projecting analysis diagnostics",
            ),
            diagram_type: None,
            parse_location: None,
        },
    }
}

fn parse_diagnostic(
    diagnostic: &ParseDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
) -> ParseDiagnosticProjection {
    let descriptor = diagnostic
        .code()
        .and_then(rule_descriptor)
        .unwrap_or(DIAGRAM_PARSE_RULE);
    let mut out = rule_candidate_without_default_span(
        descriptor,
        AnalysisStatus::ParseError,
        diagnostic.message().to_string(),
    )
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
                out = out.with_span(span).with_related(crate::DiagnosticRelated {
                    message: "Parser reported a fallback location for this syntax error."
                        .to_string(),
                    span: Some(span),
                });
                location = ParseDiagnosticLocation::Fallback;
            }
        }
    } else if let Ok(span) = source_map.whole_source_span() {
        out = out.with_span(span).with_related(crate::DiagnosticRelated {
            message: "Parser did not report a precise source location for this syntax error."
                .to_string(),
            span: Some(span),
        });
        location = ParseDiagnosticLocation::Fallback;
    } else {
        location = ParseDiagnosticLocation::Fallback;
    }

    ParseDiagnosticProjection {
        candidate: out,
        location,
    }
}

pub(crate) fn panic_candidate(message: &str, source_map: &SourceMap) -> DiagnosticCandidate {
    rule_candidate(PANIC_RULE, AnalysisStatus::Panic, message, source_map)
}

pub(crate) fn no_diagram_candidate(source_map: &SourceMap) -> DiagnosticCandidate {
    rule_candidate(
        NO_DIAGRAM_RULE,
        AnalysisStatus::NoDiagram,
        NO_DIAGRAM_MESSAGE,
        source_map,
    )
}

#[cfg(test)]
pub(crate) fn candidate_for_rule_id(
    rule_id: &str,
    status: AnalysisStatus,
    message: impl Into<String>,
    source_map: &SourceMap,
) -> DiagnosticCandidate {
    let message = message.into();
    let Some(descriptor) = rule_descriptor(rule_id) else {
        let mut candidate = rule_candidate_without_default_span(
            INTERNAL_RULE_REGISTRY_GAP_RULE,
            AnalysisStatus::InternalError,
            format!("unknown analysis rule id `{rule_id}` while emitting diagnostic: {message}"),
        );
        if let Ok(span) = source_map.whole_source_span() {
            candidate = candidate.with_span(span);
        }
        return candidate;
    };

    rule_candidate(descriptor, status, message, source_map)
}

pub(crate) fn rule_candidate(
    descriptor: RuleDescriptor,
    status: AnalysisStatus,
    message: impl Into<String>,
    source_map: &SourceMap,
) -> DiagnosticCandidate {
    let mut candidate = rule_candidate_without_default_span(descriptor, status, message);
    if let Ok(span) = source_map.whole_source_span() {
        candidate = candidate.with_span(span);
    }
    candidate
}

pub(crate) fn rule_candidate_without_default_span(
    descriptor: RuleDescriptor,
    status: AnalysisStatus,
    message: impl Into<String>,
) -> DiagnosticCandidate {
    DiagnosticCandidate::new(descriptor, message).with_status(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "compat-json")]
    use crate::rules::{INVALID_THEME_COLOR_RULE_ID, THEME_EVALUATION_LIMIT_RULE_ID};
    use crate::{
        DiagnosticCategory, DiagnosticSeverity,
        rules::{
            DIAGRAM_PARSE_RULE_ID, INTERNAL_FAILURE_RULE_ID, INVALID_THEME_COLOR_RULE,
            RECOVERED_EDITOR_FACTS_RULE,
        },
    };
    #[cfg(feature = "compat-json")]
    use merman_core::theme_color::ColorError;

    #[cfg(feature = "compat-json")]
    #[test]
    fn theme_color_errors_are_config_diagnostics_without_fabricated_source_ownership() {
        let projection = core_error_candidate(
            &CoreError::ThemeColor(ColorError::UnsupportedFormat {
                input: "not-a-color".to_string(),
            }),
            &SourceMap::new("flowchart TD\nA-->B\n"),
        );

        let diagnostic = projection.candidate.materialize(DiagnosticSeverity::Error);
        assert_eq!(diagnostic.id, INVALID_THEME_COLOR_RULE_ID);
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.category, DiagnosticCategory::Config);
        assert_eq!(diagnostic.code, Some(AnalysisStatus::ParseError.code()));
        assert!(diagnostic.message.contains("not-a-color"));
        assert_eq!(diagnostic.span, None);
        assert_eq!(projection.diagram_type, None);
        assert_eq!(projection.parse_location, None);
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn theme_evaluation_limits_are_resource_diagnostics_without_source_ownership() {
        let projection = core_error_candidate(
            &CoreError::ThemeEvaluationLimit(merman_core::ThemeEvaluationLimitExceeded {
                limit: "THEME_COLOR_LIMIT",
                requested: "65".to_string(),
                max: 64,
            }),
            &SourceMap::new("flowchart TD\nA-->B\n"),
        );

        let diagnostic = projection.candidate.materialize(DiagnosticSeverity::Error);
        assert_eq!(diagnostic.id, THEME_EVALUATION_LIMIT_RULE_ID);
        assert_eq!(diagnostic.category, DiagnosticCategory::Resource);
        assert_eq!(
            diagnostic.code,
            Some(AnalysisStatus::ResourceLimitExceeded.code())
        );
        assert_eq!(
            diagnostic.code_name.as_deref(),
            Some(AnalysisStatus::ResourceLimitExceeded.code_name())
        );
        assert!(diagnostic.message.contains("THEME_COLOR_LIMIT"));
        assert_eq!(diagnostic.span, None);
        assert_eq!(projection.diagram_type, None);
        assert_eq!(projection.parse_location, None);
    }

    #[test]
    fn internal_failures_have_no_source_ownership() {
        let error = merman_core::Error::Internal(merman_core::InternalFailure::new(
            "class subsystem failure",
        ));
        let projection = core_error_candidate(&error, &SourceMap::new("classDiagram\nA <|-- B\n"));

        let diagnostic = projection.candidate.materialize(DiagnosticSeverity::Error);
        assert_eq!(diagnostic.id, INTERNAL_FAILURE_RULE_ID);
        assert_eq!(diagnostic.category, DiagnosticCategory::Internal);
        assert_eq!(diagnostic.code, Some(AnalysisStatus::InternalError.code()));
        assert!(diagnostic.message.contains("class"));
        assert_eq!(diagnostic.span, None);
        assert_eq!(projection.diagram_type, None);
        assert_eq!(projection.parse_location, None);
    }

    #[test]
    fn recovery_projection_indexes_the_primary_parse_diagnostic_once() {
        const FILLER_COUNT: usize = 1_024;
        const RECOVERY_COUNT: usize = 1_024;

        let source_map = SourceMap::new("flowchart TD\nA[unterminated\n");
        let span = source_map.whole_source_span().unwrap();
        let mut candidates = Vec::with_capacity(FILLER_COUNT + RECOVERY_COUNT + 2);
        candidates.extend((0..FILLER_COUNT).map(|index| {
            DiagnosticCandidate::new(INVALID_THEME_COLOR_RULE, format!("filler {index}"))
        }));
        candidates.push(
            DiagnosticCandidate::new(DIAGRAM_PARSE_RULE, "primary parse failure")
                .with_diagram_type("flowchart-v2")
                .with_span(span)
                .with_parse_location(Some(ParseDiagnosticLocation::Fallback)),
        );
        candidates.extend((0..RECOVERY_COUNT).map(|index| {
            DiagnosticCandidate::new(RECOVERED_EDITOR_FACTS_RULE, format!("recovery {index}"))
                .with_diagram_type("flowchart-v2")
                .with_span(span)
                .with_recovery_kind(EditorSemanticDiagnosticKind::ParserRecovery)
        }));
        candidates.push(DiagnosticCandidate::new(INVALID_THEME_COLOR_RULE, "tail"));

        let analyzer = crate::Analyzer::new();
        let diagnostics = project_diagnostic_candidates(
            &candidates,
            analyzer.options().diagnostic_policy(),
            &AnalysisCancellationToken::new(),
        )
        .unwrap();

        assert_eq!(diagnostics.len(), FILLER_COUNT + 2);
        assert_eq!(diagnostics[FILLER_COUNT].id, DIAGRAM_PARSE_RULE_ID);
        assert_eq!(
            diagnostics[FILLER_COUNT]
                .related
                .iter()
                .filter(|related| related.message.contains("Parser recovery produced"))
                .count(),
            RECOVERY_COUNT
        );
        assert_eq!(diagnostics.last().unwrap().message, "tail");
    }

    #[test]
    fn candidate_retained_weight_counts_dynamic_fields() {
        let mut weight = crate::payload::DiagnosticRetainedWeight::default();
        let base = DiagnosticCandidate::new(INVALID_THEME_COLOR_RULE, "message");
        base.add_estimated_owned_heap_bytes(&mut weight);
        let base_weight = weight.finish();
        let rich = DiagnosticCandidate::new(INVALID_THEME_COLOR_RULE, "message".repeat(128))
            .with_help("help".repeat(128))
            .with_related(DiagnosticRelated {
                message: "related".repeat(128),
                span: None,
            });
        let mut weight = crate::payload::DiagnosticRetainedWeight::default();
        rich.add_estimated_owned_heap_bytes(&mut weight);
        assert!(weight.finish() > base_weight);
    }

    #[test]
    fn candidate_append_observes_cancellation_during_large_moves() {
        let candidates = (0..1_024)
            .map(|index| {
                DiagnosticCandidate::new(INVALID_THEME_COLOR_RULE, format!("candidate {index}"))
            })
            .collect();
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);
        let mut appended = Vec::new();

        assert!(matches!(
            append_diagnostic_candidates_cancellable(&mut appended, candidates, &cancellation,),
            Err(AnalysisCancelled)
        ));
        assert!(appended.len() < 1_024);
    }
}
