use crate::rules::{
    RESOURCE_LIMIT_RULE_ID, internal_rule_registry_gap_diagnostic, rule_descriptor,
};
use crate::{AnalysisDiagnostic, AnalysisResult, AnalysisStatus, SourceDescriptor, SourceMap};

pub(crate) fn source_limit_result(
    source: &str,
    descriptor: SourceDescriptor,
    max_source_bytes: Option<usize>,
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> Option<AnalysisResult> {
    let diagnostics = source_limit_diagnostics(source, max_source_bytes, rule_config)?;
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
    rule_config: &crate::rules::AnalysisRuleConfig,
) -> Option<Vec<AnalysisDiagnostic>> {
    let limit = max_source_bytes?;
    if source.len() <= limit {
        return None;
    }

    let source_map = SourceMap::new("");
    let mut diagnostic = source_limit_diagnostic(source.len(), limit, &source_map, rule_config);
    diagnostic.span = Some(crate::source_map::whole_text_span_without_source_copy(
        source,
    ));
    Some(vec![diagnostic])
}

fn source_limit_diagnostic(
    source_len: usize,
    limit: usize,
    source_map: &SourceMap,
    _rule_config: &crate::rules::AnalysisRuleConfig,
) -> AnalysisDiagnostic {
    let message = format!("source is {source_len} bytes, exceeding max_source_bytes {limit}");
    let Some(descriptor) = rule_descriptor(RESOURCE_LIMIT_RULE_ID) else {
        return internal_rule_registry_gap_diagnostic(
            format!(
                "unknown analysis rule id `{RESOURCE_LIMIT_RULE_ID}` while emitting diagnostic: {message}"
            ),
            source_map.whole_source_span().ok(),
        );
    };

    let mut diagnostic = AnalysisDiagnostic::new(
        descriptor.id,
        descriptor.default_severity,
        descriptor.category,
        message,
    )
    .with_code(
        AnalysisStatus::ResourceLimitExceeded.code(),
        AnalysisStatus::ResourceLimitExceeded.code_name(),
    );
    if let Ok(span) = source_map.whole_source_span() {
        diagnostic = diagnostic.with_span(span);
    }
    diagnostic
}
