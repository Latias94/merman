use crate::markdown::analyze_markdown_source;
use crate::{AnalysisPayload, Analyzer, SourceDescriptor, SourceKind};

pub fn analyze_document(
    text: &str,
    analyzer: &Analyzer,
    source: SourceDescriptor,
) -> AnalysisPayload {
    match source.kind {
        SourceKind::Markdown | SourceKind::Mdx => analyze_markdown_source(text, analyzer, source),
        SourceKind::Diagram => {
            let mut payload = analyzer.analyze(text);
            payload.source = source;
            payload
        }
    }
}

#[cfg(test)]
mod tests {
    use super::analyze_document;
    use crate::{Analyzer, SourceDescriptor, markdown::markdown_source_descriptor};

    #[test]
    fn plain_documents_use_plain_analysis_path() {
        let analyzer = Analyzer::new();
        let source = SourceDescriptor::diagram().with_path("file:///tmp/example.mmd");
        let payload = analyze_document("flowchart TD\nA-->B\n", &analyzer, source.clone());

        assert_eq!(payload.source, source);
        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }

    #[test]
    fn markdown_documents_use_fence_analysis_path() {
        let analyzer = Analyzer::new();
        let source = markdown_source_descriptor(Some("file:///tmp/example.md"));
        let payload = analyze_document(
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n",
            &analyzer,
            source.clone(),
        );

        assert_eq!(payload.source, source);
        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }
}
