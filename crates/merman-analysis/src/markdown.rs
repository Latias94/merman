use crate::{
    AnalysisDiagnostic, AnalysisPayload, Analyzer, DiagnosticSpan, DocumentSource,
    SourceDescriptor, SourceKind, SourceMap,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MarkdownChart {
    pub start: usize,
    pub body_start: usize,
    pub body_end: usize,
    pub end: usize,
    pub definition: String,
    pub source_id: String,
}

pub fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown" | "mdx")
    )
}

pub fn markdown_source_kind(path: Option<&str>) -> SourceKind {
    match path.and_then(|path| Path::new(path).extension().and_then(|ext| ext.to_str())) {
        Some("mdx") => SourceKind::Mdx,
        _ => SourceKind::Markdown,
    }
}

pub fn markdown_language(path: Option<&str>) -> &'static str {
    match markdown_source_kind(path) {
        SourceKind::Mdx => "mdx",
        SourceKind::Markdown | SourceKind::Diagram => "markdown",
    }
}

pub fn markdown_source_descriptor(path: Option<&str>) -> SourceDescriptor {
    SourceDescriptor {
        kind: markdown_source_kind(path),
        path: path.map(ToString::to_string),
        diagram_index: None,
        language: markdown_language(path).to_string(),
    }
}

pub fn extract_charts_with_spans(source: &str) -> Vec<MarkdownChart> {
    let document = DocumentSource::new(source, markdown_source_descriptor(None));
    document
        .diagrams()
        .iter()
        .map(|diagram| MarkdownChart {
            start: diagram.start,
            body_start: diagram.body_start,
            body_end: diagram.body_end,
            end: diagram.end,
            definition: diagram.text.clone(),
            source_id: diagram.id.clone(),
        })
        .collect()
}

pub fn analyze_markdown_source(
    text: &str,
    analyzer: &Analyzer,
    document_source: SourceDescriptor,
) -> AnalysisPayload {
    crate::document::analyze_document(text, analyzer, document_source)
}

pub fn remap_markdown_diagnostic(
    mut diagnostic: AnalysisDiagnostic,
    document_map: &SourceMap,
    body_start: usize,
    fence_span: Option<DiagnosticSpan>,
    diagram_index: usize,
) -> AnalysisDiagnostic {
    diagnostic.span = diagnostic
        .span
        .and_then(|span| remap_span_to_document(document_map, span, body_start))
        .or(fence_span.clone());

    for fix in &mut diagnostic.fixes {
        fix.edits = fix
            .edits
            .drain(..)
            .filter_map(|mut edit| {
                edit.span = remap_span_to_document(document_map, edit.span, body_start)?;
                Some(edit)
            })
            .collect();
    }
    diagnostic.fixes.retain(|fix| !fix.edits.is_empty());

    if let Some(span) = fence_span {
        diagnostic.related.push(crate::DiagnosticRelated {
            message: format!("Mermaid fence {}", diagram_index + 1),
            span: Some(span),
        });
    }

    diagnostic
}

pub fn remap_span_to_document(
    document_map: &SourceMap,
    span: DiagnosticSpan,
    body_start: usize,
) -> Option<DiagnosticSpan> {
    document_map
        .span(span.byte_start + body_start, span.byte_end + body_start)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisDiagnostic, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit};

    #[test]
    fn extracts_backtick_tilde_and_colon_mermaid_blocks() {
        let source = "before\n```mermaid\nflowchart LR\nA-->B\n```\n~~~mermaid\nsequenceDiagram\nA->>B: Hi\n~~~~\n:::mermaid\npie\n:::\nafter";
        let charts = extract_charts_with_spans(source);

        assert_eq!(charts.len(), 3);
        assert_eq!(charts[0].source_id, "mermaid-fence-1");
        assert_eq!(charts[1].source_id, "mermaid-fence-2");
        assert_eq!(charts[2].source_id, "mermaid-fence-3");
        assert!(charts[0].definition.contains("flowchart LR"));
        assert!(charts[1].definition.contains("sequenceDiagram"));
        assert!(charts[2].definition.contains("pie"));
    }

    #[test]
    fn remaps_diagnostics_back_into_host_document_coordinates() {
        let source = "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter";
        let document_map = SourceMap::new(source);
        let chart = extract_charts_with_spans(source)
            .into_iter()
            .next()
            .unwrap();
        let local_map = SourceMap::new(&chart.definition);
        let start = local_map.source().find('A').unwrap();
        let end = local_map.source().find("-->").unwrap();
        let diagnostic = AnalysisDiagnostic::error(
            "merman.parse.diagram_parse",
            DiagnosticCategory::Parse,
            "boom",
        )
        .with_span(local_map.span(start, end).unwrap());

        let remapped = remap_markdown_diagnostic(
            diagnostic,
            &document_map,
            chart.body_start,
            document_map.span(chart.start, chart.end).ok(),
            0,
        );

        assert_eq!(remapped.span.unwrap().line, 4);
        assert_eq!(remapped.related.len(), 1);
    }

    #[test]
    fn remaps_fix_edits_back_into_host_document_coordinates() {
        let source = "before\n```mermaid\n%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n```\nafter";
        let document_map = SourceMap::new(source);
        let chart = extract_charts_with_spans(source)
            .into_iter()
            .next()
            .unwrap();
        let local_map = SourceMap::new(&chart.definition);
        let start = local_map.source().find("initialize").unwrap();
        let end = start + "initialize".len();
        let local_span = local_map.span(start, end).unwrap();
        let diagnostic = AnalysisDiagnostic::error(
            crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
            DiagnosticCategory::Config,
            "prefer init",
        )
        .with_span(local_span.clone())
        .with_fix(DiagnosticFix::new(
            "Replace `initialize` with `init`",
            vec![DiagnosticFixEdit::new(local_span, "init")],
        ));

        let remapped = remap_markdown_diagnostic(
            diagnostic,
            &document_map,
            chart.body_start,
            document_map.span(chart.start, chart.end).ok(),
            0,
        );
        let edit_span = &remapped.fixes[0].edits[0].span;

        assert_eq!(
            &source[edit_span.byte_start..edit_span.byte_end],
            "initialize"
        );
        assert_eq!(edit_span.line, 3);
        assert_eq!(remapped.fixes[0].edits[0].replacement, "init");
    }
}
