use crate::rules::{
    RESOURCE_LIMIT_RULE_ID, internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{AnalysisDiagnostic, AnalysisResult, AnalysisStatus, SourceDescriptor, SourceMap};

pub(crate) fn source_limit_result(
    source: &str,
    descriptor: SourceDescriptor,
    max_source_bytes: Option<usize>,
) -> Option<AnalysisResult> {
    let diagnostics = source_limit_diagnostics(source, max_source_bytes)?;
    Some(AnalysisResult::new(
        descriptor,
        SourceMap::new(""),
        diagnostics,
        Vec::new(),
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
    let source_len = source.len();
    let message = format!("source is {source_len} bytes, exceeding max_source_bytes {limit}");
    let span = crate::source_map::whole_text_span_without_source_copy(source);
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
