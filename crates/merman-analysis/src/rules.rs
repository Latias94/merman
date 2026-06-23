use crate::{
    AnalysisDiagnostic, DiagnosticCategory, DiagnosticSeverity, DiagnosticSpan, SourceMap,
};
use serde_json::Value;

pub(crate) fn semantic_warning_diagnostics(
    diagram_type: &str,
    model: &Value,
    source_map: &SourceMap,
) -> Vec<AnalysisDiagnostic> {
    let span = source_map.whole_source_span().ok();
    let Some(warnings) = model.get("warnings").and_then(Value::as_array) else {
        return Vec::new();
    };

    warnings
        .iter()
        .filter_map(Value::as_str)
        .map(|message| warning_for_message(diagram_type, message, span.clone()))
        .collect()
}

fn warning_for_message(
    diagram_type: &str,
    message: &str,
    span: Option<DiagnosticSpan>,
) -> AnalysisDiagnostic {
    let id = warning_id(diagram_type, message);
    let mut diagnostic = AnalysisDiagnostic::new(
        id,
        DiagnosticSeverity::Warning,
        DiagnosticCategory::Semantic,
        message,
    )
    .with_diagram_type(diagram_type);

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(span);
    }

    diagnostic
}

fn warning_id(diagram_type: &str, message: &str) -> &'static str {
    match diagram_type {
        "block" | "block-beta" if is_block_width_warning(message) => {
            "merman.block.width_exceeds_columns"
        }
        "gitGraph" if is_git_graph_duplicate_commit_warning(message) => {
            "merman.git_graph.duplicate_commit_id"
        }
        "block" | "block-beta" => "merman.block.warning",
        "gitGraph" => "merman.git_graph.warning",
        _ => "merman.semantic.warning",
    }
}

fn is_block_width_warning(message: &str) -> bool {
    message.starts_with("Block ") && message.contains(" exceeds configured column width ")
}

fn is_git_graph_duplicate_commit_warning(message: &str) -> bool {
    message.starts_with("Commit ID ") && message.ends_with(" already exists")
}
