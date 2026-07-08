use crate::rules::{
    DIAGRAM_PARSE_RULE_ID, FLOWCHART_FACTS_PROJECTION_RULE_ID, INVALID_DIRECTIVE_JSON_RULE_ID,
    INVALID_FRONT_MATTER_YAML_RULE_ID, MALFORMED_FRONT_MATTER_RULE_ID, NO_DIAGRAM_RULE_ID,
    PANIC_RULE_ID, UNSUPPORTED_DIAGRAM_RULE_ID, internal_rule_registry_gap_diagnostic,
    rule_descriptor,
};
use crate::{AnalysisDiagnostic, AnalysisStatus, SourceMap};
use merman_core::{Error as CoreError, ParseDiagnostic, ParseDiagnosticSpanKind};

const NO_DIAGRAM_MESSAGE: &str = "no Mermaid diagram detected";

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
    error: CoreError,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
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

pub(crate) fn panic_diagnostic(
    panic_payload: Box<dyn std::any::Any + Send>,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
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
