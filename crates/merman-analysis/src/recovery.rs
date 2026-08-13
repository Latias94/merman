use crate::diagnostic_projection::{DiagnosticCandidate, ParseDiagnosticLocation, rule_candidate};
use crate::rules::{
    DIAGRAM_PARSE_RULE_ID, RECOVERED_EDITOR_FACTS_RULE, RECOVERED_EDITOR_FACTS_RULE_ID,
};
use crate::{AnalysisDiagnostic, AnalysisStatus, SourceMap};
use merman_core::{EditorSemanticDiagnostic, EditorSemanticDiagnosticKind};

pub(crate) fn merge_duplicate_parse_recovery_diagnostic(
    primary: &mut AnalysisDiagnostic,
    primary_trailing_source_context_count: usize,
    recovery: &AnalysisDiagnostic,
    recovery_kind: EditorSemanticDiagnosticKind,
    primary_parse_location: Option<ParseDiagnosticLocation>,
) -> bool {
    if recovery_kind != EditorSemanticDiagnosticKind::ParserRecovery {
        return false;
    }

    if !is_same_parse_recovery_problem(primary, recovery, primary_parse_location) {
        return false;
    }

    let mut related_insertion_index = primary
        .related
        .len()
        .saturating_sub(primary_trailing_source_context_count);

    // Host-document context is normalized onto candidates during capture but remains the final
    // related location on the wire. Insert recovery refinements immediately before that tail.
    if is_better_primary_parse_span(primary, recovery) {
        if let Some(previous_span) = primary.span {
            primary.related.insert(
                related_insertion_index,
                crate::DiagnosticRelated {
                    message:
                        "Parser reported this original parse location before recovery refinement."
                            .to_string(),
                    span: Some(previous_span),
                },
            );
            related_insertion_index = related_insertion_index.saturating_add(1);
        }
        primary.span = recovery.span;
    }
    primary.related.insert(
        related_insertion_index,
        crate::DiagnosticRelated {
            message:
                "Parser recovery produced the same syntax problem while preserving editor facts."
                    .to_string(),
            span: recovery.span,
        },
    );
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

#[cfg(test)]
pub(crate) fn editor_recovery_diagnostics(
    diagnostics: impl IntoIterator<Item = EditorSemanticDiagnostic>,
    diagram_type: &str,
    source_map: &SourceMap,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> Vec<AnalysisDiagnostic> {
    let cancellation = crate::AnalysisCancellationToken::new();
    let candidates = editor_recovery_candidates(diagnostics, diagram_type, source_map);
    crate::diagnostic_projection::project_diagnostic_candidates(
        &candidates,
        &crate::AnalysisDiagnosticPolicy {
            rule_config: rule_config.clone(),
        },
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

pub(crate) fn editor_recovery_candidates(
    diagnostics: impl IntoIterator<Item = EditorSemanticDiagnostic>,
    diagram_type: &str,
    source_map: &SourceMap,
) -> Vec<crate::diagnostic_projection::DiagnosticCandidate> {
    diagnostics
        .into_iter()
        .map(|diagnostic| recovered_editor_candidate(diagnostic, diagram_type, source_map))
        .collect()
}

fn recovered_editor_candidate(
    diagnostic: EditorSemanticDiagnostic,
    diagram_type: &str,
    source_map: &SourceMap,
) -> DiagnosticCandidate {
    let kind = diagnostic.kind;
    let mut out = rule_candidate(
        RECOVERED_EDITOR_FACTS_RULE,
        AnalysisStatus::ParseError,
        diagnostic.message,
        source_map,
    )
    .with_diagram_type(diagram_type);

    if let Some(span) = diagnostic
        .span
        .and_then(|span| source_map.span(span.start, span.end).ok())
    {
        out = out.with_span(span);
    }

    out.with_recovery_kind(kind)
}
