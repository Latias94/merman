use merman_analysis::{
    AnalysisDiagnostic, AnalysisFactsPayload, AnalysisPayload, Analyzer, DiagnosticCategory,
    SourceDescriptor, SourceMap,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

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
fn analysis_facts_payload_matches_v2_schema_shape() {
    let value = serde_json::to_value(Analyzer::new().analyze_facts("flowchart TD\nA\n")).unwrap();
    let root = value
        .as_object()
        .expect("facts payload should be an object");
    assert_eq!(
        root.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "diagnostics",
            "diagrams",
            "source",
            "summary",
            "valid",
            "version",
        ])
    );
    assert_eq!(value["version"], json!(2));
    assert_eq!(value["valid"], json!(true));

    let diagram = value["diagrams"][0]
        .as_object()
        .expect("diagram facts should be an object");
    assert_eq!(
        diagram.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "body_span",
            "fence_delimiter",
            "index",
            "kind",
            "parse_disposition",
            "source",
            "source_id",
            "span",
            "syntax",
            "text_len",
        ])
    );
    assert_eq!(diagram["parse_disposition"], json!("parsed"));

    let syntax = diagram["syntax"]
        .as_object()
        .expect("syntax facts should be an object");
    assert_eq!(
        syntax.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "class_names",
            "diagram_type",
            "directive_prefixes",
            "effective_layout",
            "expected_syntax",
            "fact_source",
            "node_ids",
            "outline_items",
            "parser_backed",
            "recovered",
            "references",
            "semantic_items",
            "source_mapped_spans",
        ])
    );
    assert_eq!(syntax["fact_source"], json!("parser_complete"));
    assert_eq!(syntax["parser_backed"], json!(true));
}

#[test]
fn analysis_facts_v2_rejects_legacy_text_scan_provenance() {
    let mut value = parser_backed_facts_json();
    value["diagrams"][0]["syntax"]["fact_source"] = json!("text_scan");

    let error = serde_json::from_value::<AnalysisFactsPayload>(value).unwrap_err();
    assert!(error.to_string().contains("text_scan"));
}

#[test]
fn analysis_facts_v2_rejects_other_wire_versions() {
    for version in [0, 1, 3] {
        let mut value = parser_backed_facts_json();
        value["version"] = json!(version);

        let error = serde_json::from_value::<AnalysisFactsPayload>(value).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("unsupported analysis facts payload version {version}; expected 2")
        );
    }
}

#[test]
fn analysis_facts_v1_is_rejected_before_deep_payload_deserialization() {
    let mut value = serde_json::Map::new();
    value.insert("valid".to_string(), json!("not a boolean"));
    value.insert("summary".to_string(), json!("not a summary"));
    value.insert("version".to_string(), json!(1));

    let error = serde_json::from_value::<AnalysisFactsPayload>(Value::Object(value))
        .expect_err("schema-1 facts must be rejected at the version boundary");
    assert_eq!(
        error.to_string(),
        "unsupported analysis facts payload version 1; expected 2"
    );
}

#[test]
fn analysis_facts_v2_accepts_payload_without_additive_effective_layout() {
    let mut value = parser_backed_facts_json();
    let syntax = value["diagrams"][0]["syntax"]
        .as_object_mut()
        .expect("syntax facts should be an object");
    assert_eq!(syntax.remove("effective_layout"), Some(json!("dagre")));

    let payload = serde_json::from_value::<AnalysisFactsPayload>(value)
        .expect("a compatible facts v2 payload should remain readable");
    assert_eq!(payload.version, 2);
    assert_eq!(payload.diagrams[0].syntax.effective_layout, None);
}

#[test]
fn analysis_facts_v2_defaults_missing_additive_parse_disposition_to_unavailable() {
    let mut value = parser_backed_facts_json();
    let diagram = value["diagrams"][0]
        .as_object_mut()
        .expect("diagram facts should be an object");
    assert_eq!(diagram.remove("parse_disposition"), Some(json!("parsed")));

    let payload = serde_json::from_value::<AnalysisFactsPayload>(value)
        .expect("a compatible facts v2 payload should remain readable");
    assert_eq!(
        payload.diagrams[0].parse_disposition,
        merman_analysis::DiagramParseDisposition::Unavailable
    );
}

#[test]
fn analysis_facts_v2_disables_rename_when_compatible_payload_omits_policy() {
    let mut value = parser_backed_facts_json();
    let semantic_item = value["diagrams"][0]["syntax"]["semantic_items"]
        .as_array_mut()
        .and_then(|items| items.first_mut())
        .expect("flowchart facts should expose a semantic item");
    semantic_item
        .as_object_mut()
        .expect("semantic item should be an object")
        .remove("rename_policy");

    let payload = serde_json::from_value::<AnalysisFactsPayload>(value)
        .expect("a compatible facts v2 payload should remain readable");
    assert_eq!(
        payload.diagrams[0].syntax.semantic_items[0].rename_policy,
        merman_analysis::FenceRenamePolicy::None
    );
}

#[test]
fn analysis_facts_v2_writers_always_emit_rename_policy() {
    let value = parser_backed_facts_json();
    assert!(
        value["diagrams"][0]["syntax"]["semantic_items"][0]
            .get("rename_policy")
            .is_some()
    );
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
