use merman::analysis::Analyzer;
use merman::editor::{DocumentKind, DocumentWorkspace, Position, completion_for_snapshot};

#[test]
fn facade_exposes_callable_analysis_and_editor_apis() {
    let analysis = Analyzer::new().analyze("flowchart TD\nA-->B");
    assert!(analysis.valid, "{:?}", analysis.diagnostics);

    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/feature-surface.mmd",
        1,
        "flowchart TD\nA-->B\nB-->".to_string(),
        DocumentKind::Diagram,
    );
    let completions = completion_for_snapshot(&snapshot, Position::new(2, 4));

    assert!(completions.items.iter().any(|item| item.label == "A"));
}
