//! Recognition survives parser omission without inventing recovered semantic facts.
use merman_analysis::{Analyzer, DiagramParseDisposition, FenceTextIndexSource};

#[test]
fn disabled_and_unknown_input_have_distinct_diagnostics() {
    if merman_core::diagram_family_capabilities()
        .iter()
        .any(|family| family.diagram_type == "sequence" && family.has_semantic_parser)
    {
        return;
    }
    let analyzer = Analyzer::new();
    let disabled = analyzer
        .analyze_generation("sequenceDiagram\nAlice->>Bob: Hello\n")
        .into_ready()
        .unwrap();
    let diagram = &disabled.diagrams()[0];
    assert_eq!(diagram.syntax().diagram_type.as_deref(), Some("sequence"));
    assert_eq!(
        diagram.parse_disposition(),
        DiagramParseDisposition::Unavailable
    );
    assert_eq!(diagram.syntax().source(), FenceTextIndexSource::Unavailable);
    let disabled_payload = disabled.project(analyzer.options().diagnostic_policy());
    assert!(disabled_payload.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unsupported diagram type: sequence")
    }));
    let unknown = analyzer.analyze("not-a-mermaid-diagram\n");
    assert!(
        unknown
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported diagram type"))
    );
    assert!(!unknown.diagnostics.is_empty());
}
