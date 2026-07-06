use crate::diagnostic_projection::{
    ParseDiagnosticLocation, core_error_diagnostic, rule_diagnostic,
    rule_diagnostic_without_default_span,
};
use crate::rules::{DIAGRAM_PARSE_RULE_ID, RECOVERED_EDITOR_FACTS_RULE_ID};
use crate::{AnalysisDiagnostic, AnalysisStatus, SourceMap};
use merman_core::{EditorSemanticDiagnostic, EditorSemanticDiagnosticKind};

#[derive(Debug, Clone)]
pub(crate) struct AnalysisRecoveryDiagnostic {
    pub(crate) diagnostic: AnalysisDiagnostic,
    kind: Option<EditorSemanticDiagnosticKind>,
}

impl AnalysisRecoveryDiagnostic {
    pub(crate) fn parser_backed(
        diagnostic: AnalysisDiagnostic,
        kind: EditorSemanticDiagnosticKind,
    ) -> Self {
        Self {
            diagnostic,
            kind: Some(kind),
        }
    }

    pub(crate) fn plain(diagnostic: AnalysisDiagnostic) -> Self {
        Self {
            diagnostic,
            kind: None,
        }
    }
}

pub(crate) fn merge_recovery_diagnostics(
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

pub(crate) fn editor_recovery_diagnostics(
    diagnostics: impl IntoIterator<Item = EditorSemanticDiagnostic>,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
    source_mapped_spans: bool,
) -> Vec<AnalysisRecoveryDiagnostic> {
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| {
            recovered_editor_diagnostic(
                diagnostic,
                diagram_type,
                source_map,
                rule_config,
                source_mapped_spans,
            )
        })
        .collect()
}

fn recovered_editor_diagnostic(
    diagnostic: EditorSemanticDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
    source_mapped_spans: bool,
) -> Option<AnalysisRecoveryDiagnostic> {
    let kind = diagnostic.kind;
    let mut out = if source_mapped_spans {
        rule_diagnostic(
            RECOVERED_EDITOR_FACTS_RULE_ID,
            AnalysisStatus::ParseError,
            diagnostic.message,
            source_map,
            rule_config,
        )?
    } else {
        rule_diagnostic_without_default_span(
            RECOVERED_EDITOR_FACTS_RULE_ID,
            AnalysisStatus::ParseError,
            diagnostic.message,
            rule_config,
        )?
    }
    .with_diagram_type(diagram_type);

    if source_mapped_spans {
        if let Some(span) = diagnostic
            .span
            .and_then(|span| source_map.span(span.start, span.end).ok())
        {
            out = out.with_span(span);
        }
    }

    Some(AnalysisRecoveryDiagnostic::parser_backed(out, kind))
}

pub(crate) fn core_error_recovery_diagnostics(
    error: merman_core::Error,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> Vec<AnalysisRecoveryDiagnostic> {
    core_error_diagnostic(error, source_map, rule_config)
        .diagnostic
        .map(AnalysisRecoveryDiagnostic::plain)
        .into_iter()
        .collect()
}
