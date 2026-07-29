use crate::rules::{
    DIAGRAM_PARSE_RULE_ID, FLOWCHART_FACTS_PROJECTION_RULE_ID, INVALID_DIRECTIVE_JSON_RULE_ID,
    INVALID_FRONT_MATTER_YAML_RULE_ID, INVALID_THEME_COLOR_RULE_ID, MALFORMED_FRONT_MATTER_RULE_ID,
    NO_DIAGRAM_RULE_ID, PANIC_RULE_ID, RuleDescriptor, UNSUPPORTED_DIAGRAM_RULE_ID,
    internal_rule_registry_gap_diagnostic, rule_descriptor,
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

    pub(crate) fn map_diagnostic(
        mut self,
        map: impl FnOnce(AnalysisDiagnostic) -> AnalysisDiagnostic,
    ) -> Self {
        self.diagnostic = map(self.diagnostic);
        self
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
    let rule_config = &policy.rule_config;
    let primary_parse_location = candidates.iter().find_map(|candidate| {
        (candidate.descriptor.id == DIAGRAM_PARSE_RULE_ID
            && candidate_enabled(candidate, rule_config))
        .then_some(candidate.parse_location)
        .flatten()
    });
    let mut diagnostics = Vec::with_capacity(candidates.len());

    for (index, candidate) in candidates.iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        if !candidate_enabled(candidate, rule_config) {
            continue;
        }

        let diagnostic = candidate.materialize(candidate.descriptor.default_severity);
        if let Some(kind) = candidate.recovery_kind {
            crate::recovery::merge_recovery_diagnostics(
                &mut diagnostics,
                vec![crate::recovery::AnalysisRecoveryDiagnostic::parser_backed(
                    diagnostic, kind,
                )],
                primary_parse_location,
            );
        } else {
            diagnostics.push(diagnostic);
        }
    }
    for (index, diagnostic) in diagnostics.iter_mut().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        let descriptor = rule_descriptor(&diagnostic.id)
            .expect("projected diagnostics must reference a registered rule");
        diagnostic.severity = rule_config.severity_for(descriptor);
    }
    cancellation.checkpoint()?;
    Ok(diagnostics)
}

pub(crate) fn candidates_from_diagnostics(
    diagnostics: impl IntoIterator<Item = AnalysisDiagnostic>,
) -> Vec<DiagnosticCandidate> {
    diagnostics
        .into_iter()
        .map(DiagnosticCandidate::new)
        .collect()
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
                DIAGRAM_PARSE_RULE_ID,
                AnalysisStatus::ParseError,
                error.to_string(),
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
        rules::{AnalysisRuleConfig, INVALID_THEME_COLOR_RULE_ID},
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
}
