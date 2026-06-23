use merman_analysis::{
    AnalysisOptions, AnalysisStatus, Analyzer, DiagnosticCategory, DiagnosticSeverity,
};

fn analyze(source: &str) -> merman_analysis::AnalysisPayload {
    Analyzer::new().analyze(source)
}

#[test]
fn empty_source_returns_no_diagram_error() {
    let payload = analyze("");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.parse.no_diagram");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.code, Some(AnalysisStatus::NoDiagram.code()));
    assert_eq!(
        diagnostic.code_name.as_deref(),
        Some(AnalysisStatus::NoDiagram.code_name())
    );
    assert_eq!(diagnostic.span.as_ref().unwrap().byte_start, 0);
    assert_eq!(diagnostic.span.as_ref().unwrap().byte_end, 0);
}

#[test]
fn invalid_syntax_returns_parse_error_with_diagram_type() {
    let payload = analyze("flowchart TD\nA -->\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.parse.diagram_parse");
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.code, Some(AnalysisStatus::ParseError.code()));
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
    assert!(diagnostic.span.is_some());
}

#[test]
fn valid_flowchart_returns_no_diagnostics() {
    let payload = analyze("flowchart TD\nA[Hello] --> B[World]\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.errors, 0);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn unsupported_diagram_returns_compatibility_error() {
    let mut engine = merman_core::Engine::new();
    *engine.diagram_registry_mut() = merman_core::diagram::DiagramRegistry::new();

    let payload = Analyzer::with_engine_and_options(engine, AnalysisOptions::default())
        .analyze("flowchart TD\nA-->B\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.compatibility.unsupported_diagram");
    assert_eq!(diagnostic.category, DiagnosticCategory::Compatibility);
    assert_eq!(
        diagnostic.code,
        Some(AnalysisStatus::UnsupportedFormat.code())
    );
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
}

#[test]
fn git_graph_duplicate_commit_id_is_warning() {
    let payload = analyze("gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.git_graph.duplicate_commit_id");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.category, DiagnosticCategory::Semantic);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("gitGraph"));
    assert!(diagnostic.message.contains("already exists"));
}

#[test]
fn block_width_overflow_is_warning() {
    let payload = analyze("block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 2);
    assert!(payload.diagnostics.iter().any(|diagnostic| {
        diagnostic.id == "merman.block.width_exceeds_columns"
            && diagnostic.diagram_type.as_deref() == Some("block")
            && diagnostic
                .message
                .contains("exceeds configured column width")
    }));
}

#[test]
fn source_byte_limit_returns_resource_error() {
    let options = AnalysisOptions::default().with_max_source_bytes(Some(8));
    let payload = Analyzer::with_options(options).analyze("flowchart TD\nA-->B\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.resource.source_bytes_exceeded");
    assert_eq!(diagnostic.category, DiagnosticCategory::Resource);
    assert_eq!(
        diagnostic.code,
        Some(AnalysisStatus::ResourceLimitExceeded.code())
    );
}

#[test]
fn panic_status_matches_binding_protocol() {
    assert_eq!(AnalysisStatus::Panic.code(), 8);
    assert_eq!(AnalysisStatus::Panic.code_name(), "MERMAN_PANIC");
}
