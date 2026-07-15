use merman_analysis::{
    AnalysisDiagnostic, AnalysisFactsPayload, AnalysisPayload, AnalysisResult, AnalysisSyntaxFacts,
    AnalyzedDiagram, Analyzer, DiagnosticCategory, DocumentDiagramKind, SharedTextSlice,
    SourceDescriptor, SourceMap,
};
use serde_json::{Value, json};
use std::sync::Arc;

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
fn analysis_facts_payload_matches_v1_schema_shape() {
    let source = Arc::<str>::from("");
    let source_descriptor = SourceDescriptor::diagram();
    let result = AnalysisResult::new(
        source_descriptor.clone(),
        SourceMap::new(Arc::clone(&source)),
        Vec::new(),
        vec![AnalyzedDiagram {
            source_id: "document".to_string(),
            index: 0,
            kind: DocumentDiagramKind::WholeDocument,
            source: source_descriptor,
            start: 0,
            body_start: 0,
            body_end: 0,
            end: 0,
            text: SharedTextSlice::whole(source),
            fence_delimiter: None,
            diagnostics: Vec::new(),
            syntax: AnalysisSyntaxFacts::unavailable(None),
        }],
    );

    assert_eq!(
        serde_json::to_value(result.to_facts_payload()).unwrap(),
        json!({
            "version": 1,
            "valid": true,
            "summary": {
                "errors": 0,
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
            "diagnostics": [],
            "diagrams": [{
                "source_id": "document",
                "index": 0,
                "kind": "whole_document",
                "source": {
                    "kind": "diagram",
                    "path": null,
                    "diagram_index": null,
                    "language": "mermaid"
                },
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
                "body_span": {
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
                "text_len": 0,
                "fence_delimiter": null,
                "syntax": {
                    "diagram_type": null,
                    "fact_source": "unavailable",
                    "parser_backed": false,
                    "recovered": false,
                    "source_mapped_spans": false,
                    "node_ids": [],
                    "class_names": [],
                    "directive_prefixes": [],
                    "references": [],
                    "outline_items": [],
                    "semantic_items": [],
                    "expected_syntax": []
                }
            }]
        })
    );
}

#[test]
fn analysis_facts_v1_rejects_legacy_text_scan_provenance() {
    let mut value = parser_backed_facts_json();
    value["diagrams"][0]["syntax"]["fact_source"] = json!("text_scan");

    let error = serde_json::from_value::<AnalysisFactsPayload>(value).unwrap_err();
    assert!(error.to_string().contains("text_scan"));
}

#[test]
fn analysis_facts_v1_rejects_other_wire_versions() {
    for version in [0, 2] {
        let mut value = parser_backed_facts_json();
        value["version"] = json!(version);

        let error = serde_json::from_value::<AnalysisFactsPayload>(value).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("unsupported analysis facts payload version {version}; expected 1")
        );
    }
}

#[test]
fn analysis_facts_v1_rejects_semantic_items_without_rename_policy() {
    let mut value = parser_backed_facts_json();
    let semantic_item = value["diagrams"][0]["syntax"]["semantic_items"]
        .as_array_mut()
        .and_then(|items| items.first_mut())
        .expect("flowchart facts should expose a semantic item");
    semantic_item
        .as_object_mut()
        .expect("semantic item should be an object")
        .remove("rename_policy");

    let error = serde_json::from_value::<AnalysisFactsPayload>(value).unwrap_err();
    assert!(error.to_string().contains("rename_policy"));
}

#[test]
fn analysis_payload_v1_rejects_other_wire_versions() {
    let mut value = serde_json::to_value(AnalysisPayload::valid(SourceDescriptor::diagram()))
        .expect("serialize analysis payload");
    value["version"] = json!(2);

    let error = serde_json::from_value::<AnalysisPayload>(value).unwrap_err();
    assert_eq!(
        error.to_string(),
        "unsupported analysis payload version 2; expected 1"
    );
}

fn parser_backed_facts_json() -> Value {
    serde_json::to_value(Analyzer::new().analyze_facts("flowchart TD\nA\n")).unwrap()
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
