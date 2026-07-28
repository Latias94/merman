use crate::rules::{
    RESOURCE_LIMIT_RULE_ID, internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{AnalysisDiagnostic, AnalysisRejection, AnalysisStatus, SourceDescriptor};

pub(crate) fn source_limit_rejection(
    source: &str,
    descriptor: SourceDescriptor,
    max_source_bytes: Option<usize>,
) -> Option<AnalysisRejection> {
    let limit = max_source_bytes?;
    let diagnostics = source_limit_diagnostics(source, max_source_bytes)?;
    Some(AnalysisRejection::source_limit(
        descriptor,
        diagnostics,
        source.len(),
        limit,
    ))
}

pub(crate) fn source_limit_diagnostics(
    source: &str,
    max_source_bytes: Option<usize>,
) -> Option<Vec<AnalysisDiagnostic>> {
    let limit = max_source_bytes?;
    if source.len() <= limit {
        return None;
    }

    Some(vec![source_limit_diagnostic(source, limit)])
}

fn source_limit_diagnostic(source: &str, limit: usize) -> AnalysisDiagnostic {
    let span = crate::source_map::whole_text_span_without_source_copy(source);
    source_limit_diagnostic_for_len_and_span(source.len(), limit, span)
}

pub fn source_limit_diagnostic_for_len(source_len: usize, limit: usize) -> AnalysisDiagnostic {
    let span = crate::DiagnosticSpan::new(
        0..0,
        crate::SourcePosition::new(1, 1),
        crate::SourcePosition::new(1, 1),
        crate::LspRange::new(
            crate::Utf16Position {
                line: 0,
                character: 0,
            },
            crate::Utf16Position {
                line: 0,
                character: 0,
            },
        ),
    );
    source_limit_diagnostic_for_len_and_span(source_len, limit, span)
}

pub fn source_discarded_after_limit_change_diagnostic(
    source_len: usize,
    previous_limit: usize,
) -> AnalysisDiagnostic {
    let span = crate::DiagnosticSpan::new(
        0..0,
        crate::SourcePosition::new(1, 1),
        crate::SourcePosition::new(1, 1),
        crate::LspRange::new(
            crate::Utf16Position {
                line: 0,
                character: 0,
            },
            crate::Utf16Position {
                line: 0,
                character: 0,
            },
        ),
    );
    let message = format!(
        "source is {source_len} bytes and was discarded after exceeding previous max_source_bytes {previous_limit}; reopen the document or send a full document replacement to analyze it with the current limit"
    );
    let Some(descriptor) = rule_descriptor(RESOURCE_LIMIT_RULE_ID) else {
        return internal_rule_registry_gap_diagnostic(
            format!(
                "unknown analysis rule id `{RESOURCE_LIMIT_RULE_ID}` while emitting diagnostic: {message}"
            ),
            Some(span),
        );
    };

    AnalysisDiagnostic::new(
        descriptor.id,
        descriptor.default_severity,
        descriptor.category,
        message,
    )
    .with_code(
        AnalysisStatus::ResourceLimitExceeded.code(),
        AnalysisStatus::ResourceLimitExceeded.code_name(),
    )
    .with_span(span)
}

fn source_limit_diagnostic_for_len_and_span(
    source_len: usize,
    limit: usize,
    span: crate::DiagnosticSpan,
) -> AnalysisDiagnostic {
    let message = format!("source is {source_len} bytes, exceeding max_source_bytes {limit}");
    let Some(descriptor) = rule_descriptor(RESOURCE_LIMIT_RULE_ID) else {
        return internal_rule_registry_gap_diagnostic(
            format!(
                "unknown analysis rule id `{RESOURCE_LIMIT_RULE_ID}` while emitting diagnostic: {message}"
            ),
            Some(span),
        );
    };

    AnalysisDiagnostic::new(
        descriptor.id,
        descriptor.default_severity,
        descriptor.category,
        message,
    )
    .with_code(
        AnalysisStatus::ResourceLimitExceeded.code(),
        AnalysisStatus::ResourceLimitExceeded.code_name(),
    )
    .with_span(span)
}
