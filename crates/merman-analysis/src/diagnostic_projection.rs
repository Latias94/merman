use crate::rules::{
    DIAGRAM_PARSE_RULE_ID, FLOWCHART_FACTS_PROJECTION_RULE_ID, INVALID_DIRECTIVE_JSON_RULE_ID,
    INVALID_FRONT_MATTER_YAML_RULE_ID, INVALID_THEME_COLOR_RULE_ID, MALFORMED_FRONT_MATTER_RULE_ID,
    NO_DIAGRAM_RULE_ID, PANIC_RULE_ID, PARSER_CONTRACT_VIOLATION_RULE_ID, RuleDescriptor,
    UNSUPPORTED_DIAGRAM_RULE_ID, internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisDiagnostic, AnalysisDiagnosticPolicy,
    AnalysisStatus, SourceMap,
};
use merman_core::{
    EditorSemanticDiagnosticKind, Error as CoreError, ParseDiagnostic, ParseDiagnosticSpanKind,
};
const NO_DIAGRAM_MESSAGE: &str = "no Mermaid diagram detected";

/// A complete diagnostic before rule filtering and severity resolution.
#[derive(Debug, Clone)]
pub(crate) struct DiagnosticCandidate {
    diagnostic: AnalysisDiagnostic,
    descriptor: RuleDescriptor,
    suppressor_ids: &'static [&'static str],
    parse_location: Option<ParseDiagnosticLocation>,
    recovery_kind: Option<EditorSemanticDiagnosticKind>,
    trailing_source_context_count: usize,
}

impl DiagnosticCandidate {
    pub(crate) fn new(diagnostic: AnalysisDiagnostic) -> Self {
        let descriptor = rule_descriptor(&diagnostic.id)
            .expect("analysis diagnostics must reference a registered rule");
        Self {
            diagnostic,
            descriptor,
            suppressor_ids: &[],
            parse_location: None,
            recovery_kind: None,
            trailing_source_context_count: 0,
        }
    }

    pub(crate) fn with_suppressors(mut self, suppressor_ids: &'static [&'static str]) -> Self {
        self.suppressor_ids = suppressor_ids;
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

    pub(crate) fn try_map_diagnostic<E>(
        mut self,
        map: impl FnOnce(AnalysisDiagnostic) -> Result<AnalysisDiagnostic, E>,
    ) -> Result<Self, E> {
        self.diagnostic = map(self.diagnostic)?;
        Ok(self)
    }

    pub(crate) fn with_trailing_source_context(
        mut self,
        context: crate::DiagnosticRelated,
    ) -> Self {
        self.diagnostic.related.push(context);
        self.trailing_source_context_count = self.trailing_source_context_count.saturating_add(1);
        self
    }

    pub(crate) fn add_estimated_owned_heap_bytes(
        &self,
        weight: &mut crate::payload::DiagnosticRetainedWeight,
    ) {
        weight.add_diagnostic(&self.diagnostic);
    }

    fn materialize(&self, severity: crate::DiagnosticSeverity) -> AnalysisDiagnostic {
        let mut diagnostic = self.diagnostic.clone();
        diagnostic.severity = severity;
        diagnostic
    }

    #[cfg(test)]
    pub(crate) const fn rule_id(&self) -> &'static str {
        self.descriptor.id
    }
}

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

    for (index, candidate) in candidates.iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        if !candidate_enabled(candidate, rule_config) {
            continue;
        }

        let diagnostic = candidate.materialize(rule_config.severity_for(candidate.descriptor));
        if let Some(kind) = candidate.recovery_kind {
            let recovery =
                crate::recovery::AnalysisRecoveryDiagnostic::parser_backed(diagnostic, kind);
            let merged = primary_parse.is_some_and(|primary: ProjectedPrimaryParse| {
                crate::recovery::merge_duplicate_parse_recovery_diagnostic(
                    &mut diagnostics[primary.diagnostic_index],
                    primary.trailing_source_context_count,
                    &recovery,
                    primary.parse_location,
                )
            });
            if merged {
                continue;
            }
            diagnostics.push(recovery.diagnostic);
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

pub(crate) fn materialize_diagnostic_candidates(
    candidates: &[DiagnosticCandidate],
    policy: &AnalysisDiagnosticPolicy,
) -> Vec<AnalysisDiagnostic> {
    let cancellation = AnalysisCancellationToken::new();
    project_diagnostic_candidates(candidates, policy, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

pub(crate) fn candidates_from_diagnostics_cancellable(
    diagnostics: impl IntoIterator<Item = AnalysisDiagnostic>,
    cancellation: &AnalysisCancellationToken,
) -> Result<Vec<DiagnosticCandidate>, AnalysisCancelled> {
    let mut candidates = Vec::new();
    extend_candidates_from_diagnostics_cancellable(&mut candidates, diagnostics, cancellation)?;
    Ok(candidates)
}

pub(crate) fn candidates_from_diagnostics(
    diagnostics: impl IntoIterator<Item = AnalysisDiagnostic>,
) -> Vec<DiagnosticCandidate> {
    let cancellation = AnalysisCancellationToken::new();
    candidates_from_diagnostics_cancellable(diagnostics, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

pub(crate) fn extend_candidates_from_diagnostics_cancellable(
    candidates: &mut Vec<DiagnosticCandidate>,
    diagnostics: impl IntoIterator<Item = AnalysisDiagnostic>,
    cancellation: &AnalysisCancellationToken,
) -> Result<(), AnalysisCancelled> {
    for (index, diagnostic) in diagnostics.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        candidates.push(DiagnosticCandidate::new(diagnostic));
    }
    cancellation.checkpoint()?;
    Ok(())
}

fn candidate_enabled(
    candidate: &DiagnosticCandidate,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> bool {
    rule_config.is_rule_enabled(candidate.descriptor)
        && !candidate.suppressor_ids.iter().any(|rule_id| {
            rule_descriptor(rule_id)
                .is_some_and(|descriptor| rule_config.is_rule_enabled(descriptor))
        })
}

#[derive(Debug)]
pub(crate) struct CoreErrorDiagnostic {
    pub(crate) diagnostic: Option<AnalysisDiagnostic>,
    pub(crate) diagram_type: Option<String>,
    pub(crate) parse_location: Option<ParseDiagnosticLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseDiagnosticLocation {
    Precise,
    Fallback,
}

struct ParseDiagnosticProjection {
    diagnostic: AnalysisDiagnostic,
    location: ParseDiagnosticLocation,
}

pub(crate) fn core_error_diagnostic(
    error: &CoreError,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> CoreErrorDiagnostic {
    match error {
        CoreError::ParseCancelled(error) => CoreErrorDiagnostic {
            diagnostic: rule_diagnostic(
                PARSER_CONTRACT_VIOLATION_RULE_ID,
                AnalysisStatus::InternalError,
                format!(
                    "custom parser returned cancellation to a non-cancellable analysis facade: {error}"
                ),
                source_map,
                rule_config,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::ThemeColor(error) => CoreErrorDiagnostic {
            diagnostic: rule_diagnostic_without_default_span(
                INVALID_THEME_COLOR_RULE_ID,
                AnalysisStatus::ParseError,
                error.to_string(),
                rule_config,
            ),
            diagram_type: None,
            parse_location: None,
        },
        CoreError::RuntimePolicy(error) => CoreErrorDiagnostic {
            diagnostic: rule_diagnostic(
                DIAGRAM_PARSE_RULE_ID,
                AnalysisStatus::ParseError,
                error.to_string(),
                source_map,
                rule_config,
            ),
            diagram_type: None,
            parse_location: None,
        },
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
            diagram_type: Some(diagram_type.clone()),
            parse_location: None,
        },
        CoreError::DiagramParse {
            diagram_type,
            diagnostic,
        } => {
            let (diagnostic, parse_location) =
                match parse_diagnostic(diagnostic, diagram_type, source_map, rule_config) {
                    Some(projection) => (Some(projection.diagnostic), Some(projection.location)),
                    None => (None, None),
                };
            CoreErrorDiagnostic {
                diagnostic,
                diagram_type: Some(diagram_type.clone()),
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
    diagnostic: &ParseDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
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
                    span: Some(span),
                });
                out = out.with_span(span);
                location = ParseDiagnosticLocation::Fallback;
            }
        }
    } else if let Ok(span) = source_map.whole_source_span() {
        out.related.push(crate::DiagnosticRelated {
            message: "Parser did not report a precise source location for this syntax error."
                .to_string(),
            span: Some(span),
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

pub(crate) fn panic_diagnostic(
    message: &str,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    rule_diagnostic(
        PANIC_RULE_ID,
        AnalysisStatus::Panic,
        message,
        source_map,
        rule_config,
    )
}

pub(crate) fn flowchart_facts_projection_diagnostic(
    error: impl std::fmt::Display,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
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

pub(crate) fn no_diagram_diagnostic(
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> Option<AnalysisDiagnostic> {
    rule_diagnostic(
        NO_DIAGRAM_RULE_ID,
        AnalysisStatus::NoDiagram,
        NO_DIAGRAM_MESSAGE,
        source_map,
        rule_config,
    )
}

pub(crate) fn rule_diagnostic(
    rule_id: &'static str,
    status: AnalysisStatus,
    message: impl Into<String>,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
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

pub(crate) fn rule_diagnostic_without_default_span(
    rule_id: &'static str,
    status: AnalysisStatus,
    message: impl Into<String>,
    rule_config: &crate::rules::AnalysisRuleConfig,
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
    use super::*;
    use crate::{
        DiagnosticCategory, DiagnosticSeverity,
        rules::{
            AnalysisRuleConfig, DIAGRAM_PARSE_RULE_ID, INVALID_THEME_COLOR_RULE_ID,
            RECOVERED_EDITOR_FACTS_RULE_ID,
        },
    };
    use merman_core::theme_color::ColorError;

    #[test]
    fn theme_color_errors_are_config_diagnostics_without_fabricated_source_ownership() {
        let projection = core_error_diagnostic(
            &CoreError::ThemeColor(ColorError::UnsupportedFormat {
                input: "not-a-color".to_string(),
            }),
            &SourceMap::new("flowchart TD\nA-->B\n"),
            &AnalysisRuleConfig::default(),
        );

        let diagnostic = projection.diagnostic.expect("theme color diagnostic");
        assert_eq!(diagnostic.id, INVALID_THEME_COLOR_RULE_ID);
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.category, DiagnosticCategory::Config);
        assert_eq!(diagnostic.code, Some(AnalysisStatus::ParseError.code()));
        assert!(diagnostic.message.contains("not-a-color"));
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
            DiagnosticCandidate::new(AnalysisDiagnostic::new(
                INVALID_THEME_COLOR_RULE_ID,
                DiagnosticSeverity::Error,
                DiagnosticCategory::Config,
                format!("filler {index}"),
            ))
        }));
        candidates.push(
            DiagnosticCandidate::new(
                AnalysisDiagnostic::error(
                    DIAGRAM_PARSE_RULE_ID,
                    DiagnosticCategory::Parse,
                    "primary parse failure",
                )
                .with_diagram_type("flowchart-v2")
                .with_span(span),
            )
            .with_parse_location(Some(ParseDiagnosticLocation::Fallback)),
        );
        candidates.extend((0..RECOVERY_COUNT).map(|index| {
            DiagnosticCandidate::new(
                AnalysisDiagnostic::error(
                    RECOVERED_EDITOR_FACTS_RULE_ID,
                    DiagnosticCategory::Parse,
                    format!("recovery {index}"),
                )
                .with_diagram_type("flowchart-v2")
                .with_span(span),
            )
            .with_recovery_kind(EditorSemanticDiagnosticKind::ParserRecovery)
        }));
        candidates.push(DiagnosticCandidate::new(AnalysisDiagnostic::new(
            INVALID_THEME_COLOR_RULE_ID,
            DiagnosticSeverity::Error,
            DiagnosticCategory::Config,
            "tail",
        )));

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
    fn diagnostic_candidate_conversion_observes_cancellation() {
        let diagnostics = (0..1_024).map(|index| {
            AnalysisDiagnostic::new(
                INVALID_THEME_COLOR_RULE_ID,
                DiagnosticSeverity::Error,
                DiagnosticCategory::Config,
                format!("diagnostic {index}"),
            )
        });
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            candidates_from_diagnostics_cancellable(diagnostics, &cancellation),
            Err(AnalysisCancelled)
        ));
    }
}
