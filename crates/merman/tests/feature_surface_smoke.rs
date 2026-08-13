use merman::analysis::{AnalysisCancellationToken, Analyzer};
use merman::editor::{
    DocumentKind, Position, analyze_document_context_with_shared_text_cancellable,
    analyze_document_snapshot_with_shared_text, completion_for_snapshot,
};
use std::sync::Arc;

#[test]
fn facade_exposes_callable_analysis_and_editor_apis() {
    let analysis = Analyzer::new().analyze("flowchart TD\nA-->B");
    assert!(analysis.valid, "{:?}", analysis.diagnostics);

    let snapshot = analyze_document_snapshot_with_shared_text(
        &Analyzer::new(),
        "file:///tmp/feature-surface.mmd",
        1,
        Arc::from("flowchart TD\nA-->B\nB-->"),
        DocumentKind::Diagram,
    )
    .expect("fixture source must fit the default editor resource policy");
    let completions = completion_for_snapshot(&snapshot, Position::new(2, 4));

    assert!(completions.items.iter().any(|item| item.label == "A"));

    let shared_source: Arc<str> =
        Arc::from("before\n```mermaid\nsequenceDiagram\nAlice->>Bob: Hi\n```\nafter\n");
    let context = analyze_document_context_with_shared_text_cancellable(
        &Analyzer::new(),
        "file:///tmp/feature-surface.md",
        2,
        Arc::clone(&shared_source),
        DocumentKind::Markdown,
        &AnalysisCancellationToken::new(),
    )
    .expect("live analysis should not be cancelled")
    .expect("fixture source must fit the default editor resource policy");

    assert_eq!(
        context.snapshot().uri().as_str(),
        "file:///tmp/feature-surface.md"
    );
    assert_eq!(context.snapshot().version(), 2);
    assert_eq!(context.snapshot().kind(), DocumentKind::Markdown);
    assert!(Arc::ptr_eq(
        context.snapshot().shared_text(),
        &shared_source
    ));
}
