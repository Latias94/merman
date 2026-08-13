use crate::rules::{
    RESOURCE_LIMIT_RULE_ID, internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{AnalysisDiagnostic, AnalysisRejection, AnalysisStatus, SourceDescriptor};

pub(crate) fn source_limit_rejection_cancellable(
    source: &str,
    descriptor: &SourceDescriptor,
    max_source_bytes: Option<usize>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<AnalysisRejection>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    let Some(limit) = max_source_bytes else {
        return Ok(None);
    };
    if source.len() <= limit {
        return Ok(None);
    }
    let span = source_limit_diagnostic_span_cancellable(source, cancellation)?;
    let diagnostic = source_limit_diagnostic_for_len_and_span(source.len(), limit, span);
    Ok(Some(AnalysisRejection::source_limit(
        descriptor.clone(),
        vec![diagnostic],
        source.len(),
        limit,
    )))
}

/// Captures the exact whole-source coordinates needed to report a rejected source without retaining
/// the source text.
pub fn source_limit_diagnostic_span(source: &str) -> crate::DiagnosticSpan {
    crate::source_map::whole_text_span_without_source_copy(source)
}

/// Captures whole-source rejection coordinates with cooperative cancellation.
pub fn source_limit_diagnostic_span_cancellable(
    source: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<crate::DiagnosticSpan, crate::AnalysisCancelled> {
    crate::source_map::whole_text_span_without_source_copy_cancellable(source, cancellation)
}

pub fn source_limit_diagnostic_for_len(source_len: usize, limit: usize) -> AnalysisDiagnostic {
    source_limit_diagnostic_for_len_and_span(source_len, limit, zero_length_diagnostic_span())
}

pub fn source_limit_diagnostic_for_len_and_span(
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

fn zero_length_diagnostic_span() -> crate::DiagnosticSpan {
    crate::DiagnosticSpan::new(
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
    )
}

pub fn source_discarded_after_limit_change_diagnostic(
    source_len: usize,
    previous_limit: usize,
) -> AnalysisDiagnostic {
    source_discarded_after_limit_change_diagnostic_with_span(
        source_len,
        previous_limit,
        zero_length_diagnostic_span(),
    )
}

pub fn source_discarded_after_limit_change_diagnostic_with_span(
    source_len: usize,
    previous_limit: usize,
    span: crate::DiagnosticSpan,
) -> AnalysisDiagnostic {
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
