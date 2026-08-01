use crate::rules::{
    DOCUMENT_DIAGRAM_LIMIT_RULE_ID, internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisDiagnostic, AnalysisRejection,
    AnalysisStatus, DiagnosticSpan, SourceDescriptor, SourceKind,
};

pub(crate) fn document_diagram_limit_rejection_cancellable(
    source: &str,
    descriptor: &SourceDescriptor,
    max_document_diagrams: Option<usize>,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<AnalysisRejection>, AnalysisCancelled> {
    let Some(limit) = max_document_diagrams else {
        return Ok(None);
    };
    if !matches!(descriptor.kind, SourceKind::Markdown | SourceKind::Mdx) {
        return Ok(None);
    }

    let Some(exceeded) = crate::document::markdown_document_diagram_limit_exceeded_cancellable(
        source,
        limit,
        cancellation,
    )?
    else {
        return Ok(None);
    };
    document_diagram_limit_rejection_from_exceeded_cancellable(
        source,
        descriptor,
        limit,
        exceeded,
        cancellation,
    )
    .map(Some)
}

pub(crate) fn document_diagram_limit_rejection_from_exceeded_cancellable(
    source: &str,
    descriptor: &SourceDescriptor,
    limit: usize,
    exceeded: crate::document::MarkdownDocumentDiagramLimitExceeded,
    cancellation: &AnalysisCancellationToken,
) -> Result<AnalysisRejection, AnalysisCancelled> {
    let span = crate::source_map::byte_range_span_without_source_copy_cancellable(
        source,
        exceeded.opening_marker,
        cancellation,
    )?;
    let diagnostic =
        document_diagram_limit_diagnostic(exceeded.observed_document_diagrams, limit, span);
    Ok(AnalysisRejection::document_diagram_limit(
        descriptor.clone(),
        vec![diagnostic],
        exceeded.observed_document_diagrams,
        limit,
    ))
}

fn document_diagram_limit_diagnostic(
    observed_document_diagrams: usize,
    limit: usize,
    span: DiagnosticSpan,
) -> AnalysisDiagnostic {
    let message =
        format!("Mermaid fence {observed_document_diagrams} exceeds max_document_diagrams {limit}");
    let Some(descriptor) = rule_descriptor(DOCUMENT_DIAGRAM_LIMIT_RULE_ID) else {
        return internal_rule_registry_gap_diagnostic(
            format!(
                "unknown analysis rule id `{DOCUMENT_DIAGRAM_LIMIT_RULE_ID}` while emitting diagnostic: {message}"
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
