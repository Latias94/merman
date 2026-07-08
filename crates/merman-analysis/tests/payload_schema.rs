use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, SourceDescriptor, SourceMap,
};
use serde_json::{Value, json};

#[test]
fn analysis_payload_matches_adr_0070_schema_shape() {
    let source = "";
    let map = SourceMap::new(source);
    let diagnostic = AnalysisDiagnostic::error(
        "merman.parse.no_diagram",
        DiagnosticCategory::Parse,
        "no Mermaid diagram detected",
    )
    .with_code(4, "MERMAN_NO_DIAGRAM")
    .with_span(map.whole_source_span().unwrap());
    let payload = AnalysisPayload::new(SourceDescriptor::diagram(), vec![diagnostic]);
    let value: Value = serde_json::from_slice(&payload.to_json_bytes().unwrap()).unwrap();

    assert_eq!(
        value,
        json!({
            "version": 1,
            "valid": false,
            "summary": {
                "errors": 1,
                "warnings": 0,
                "infos": 0,
                "hints": 0
            },
            "source": {
                "kind": "diagram",
                "path": null,
                "diagram_index": null,
                "language": "mermaid"
            },
            "diagnostics": [
                {
                    "id": "merman.parse.no_diagram",
                    "severity": "error",
                    "category": "parse",
                    "message": "no Mermaid diagram detected",
                    "code": 4,
                    "code_name": "MERMAN_NO_DIAGRAM",
                    "diagram_type": null,
                    "span": {
                        "byte_start": 0,
                        "byte_end": 0,
                        "line": 1,
                        "column": 1,
                        "end_line": 1,
                        "end_column": 1,
                        "lsp_range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        }
                    },
                    "related": [],
                    "help": null
                }
            ]
        })
    );
}

#[test]
fn payload_summary_counts_all_severities() {
    let diagnostics = vec![
        AnalysisDiagnostic::error("merman.parse.a", DiagnosticCategory::Parse, "a"),
        AnalysisDiagnostic {
            severity: merman_analysis::DiagnosticSeverity::Warning,
            ..AnalysisDiagnostic::error("merman.compat.b", DiagnosticCategory::Compatibility, "b")
        },
        AnalysisDiagnostic {
            severity: merman_analysis::DiagnosticSeverity::Info,
            ..AnalysisDiagnostic::error("merman.config.c", DiagnosticCategory::Config, "c")
        },
        AnalysisDiagnostic {
            severity: merman_analysis::DiagnosticSeverity::Hint,
            ..AnalysisDiagnostic::error("merman.semantic.d", DiagnosticCategory::Semantic, "d")
        },
    ];
    let payload = AnalysisPayload::new(SourceDescriptor::diagram(), diagnostics);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 1);
    assert_eq!(payload.summary.infos, 1);
    assert_eq!(payload.summary.hints, 1);
}

#[test]
fn diagnostic_category_internal_serializes_as_internal() {
    assert_eq!(DiagnosticCategory::Internal.as_str(), "internal");
    assert_eq!(
        serde_json::to_value(DiagnosticCategory::Internal).unwrap(),
        json!("internal")
    );
}
