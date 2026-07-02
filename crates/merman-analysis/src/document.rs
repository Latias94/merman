use crate::{
    AnalysisDiagnostic, AnalysisPayload, AnalysisResult, Analyzer, DiagnosticRelated,
    DiagnosticSpan, SourceDescriptor, SourceKind, SourceMap,
};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentDiagramKind {
    WholeDocument,
    MermaidFence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceMarker {
    Backtick,
    Tilde,
    Colon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FenceDelimiter {
    marker: FenceMarker,
    len: usize,
}

impl FenceDelimiter {
    pub const fn new(marker: FenceMarker, len: usize) -> Self {
        Self { marker, len }
    }

    pub const fn marker(self) -> FenceMarker {
        self.marker
    }

    pub const fn len(self) -> usize {
        self.len
    }

    const fn marker_byte(self) -> u8 {
        match self.marker {
            FenceMarker::Backtick => b'`',
            FenceMarker::Tilde => b'~',
            FenceMarker::Colon => b':',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDiagram {
    pub id: String,
    pub index: usize,
    pub kind: DocumentDiagramKind,
    pub source: SourceDescriptor,
    pub start: usize,
    pub body_start: usize,
    pub body_end: usize,
    pub end: usize,
    pub text: String,
    pub fence_delimiter: Option<FenceDelimiter>,
}

#[derive(Debug, Clone)]
pub struct DocumentSource {
    source: SourceDescriptor,
    text: String,
    source_map: SourceMap,
    diagrams: Vec<DocumentDiagram>,
}

impl DocumentSource {
    pub fn new(text: impl Into<String>, source: SourceDescriptor) -> Self {
        let text = text.into();
        let source_map = SourceMap::new(text.clone());
        let diagrams = match source.kind {
            SourceKind::Markdown | SourceKind::Mdx => extract_markdown_diagrams(&text, &source),
            SourceKind::Diagram => vec![whole_document_diagram(&text, &source)],
        };

        Self {
            source,
            text,
            source_map,
            diagrams,
        }
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.source
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn diagrams(&self) -> &[DocumentDiagram] {
        &self.diagrams
    }

    pub fn remap_span_to_document(
        &self,
        diagram: &DocumentDiagram,
        span: DiagnosticSpan,
    ) -> Option<DiagnosticSpan> {
        let start = diagram.body_start.checked_add(span.byte_start)?;
        let end = diagram.body_start.checked_add(span.byte_end)?;
        self.source_map.span(start, end).ok()
    }

    pub fn remap_diagnostic_to_document(
        &self,
        diagram: &DocumentDiagram,
        mut diagnostic: AnalysisDiagnostic,
    ) -> AnalysisDiagnostic {
        let fence_span = diagram
            .is_fence()
            .then(|| self.source_map.span(diagram.start, diagram.end).ok())
            .flatten();

        diagnostic.span = diagnostic
            .span
            .and_then(|span| self.remap_span_to_document(diagram, span))
            .or(fence_span.clone());

        for related in &mut diagnostic.related {
            related.span = related
                .span
                .take()
                .and_then(|span| self.remap_span_to_document(diagram, span));
        }

        for fix in &mut diagnostic.fixes {
            fix.edits = fix
                .edits
                .drain(..)
                .filter_map(|mut edit| {
                    edit.span = self.remap_span_to_document(diagram, edit.span)?;
                    Some(edit)
                })
                .collect();
        }
        diagnostic.fixes.retain(|fix| !fix.edits.is_empty());

        if let Some(span) = fence_span {
            diagnostic.related.push(DiagnosticRelated {
                message: format!("Mermaid fence {}", diagram.index + 1),
                span: Some(span),
            });
        }

        diagnostic
    }
}

impl DocumentDiagram {
    pub const fn is_fence(&self) -> bool {
        matches!(self.kind, DocumentDiagramKind::MermaidFence)
    }
}

pub fn source_descriptor_for_kind(path: Option<&str>, kind: SourceKind) -> SourceDescriptor {
    SourceDescriptor {
        kind,
        path: path.map(ToString::to_string),
        diagram_index: None,
        language: source_language(kind).to_string(),
    }
}

pub fn source_descriptor_for_uri(uri: &str) -> SourceDescriptor {
    let path_without_fragment = uri.split(['?', '#']).next().unwrap_or(uri);
    let kind = match Path::new(path_without_fragment)
        .extension()
        .and_then(|ext| ext.to_str())
    {
        Some("md") | Some("markdown") => SourceKind::Markdown,
        Some("mdx") => SourceKind::Mdx,
        _ => SourceKind::Diagram,
    };
    source_descriptor_for_kind(Some(uri), kind)
}

pub fn source_descriptor_for_markdown_path(path: Option<&str>) -> SourceDescriptor {
    let path_without_fragment = path.map(|path| path.split(['?', '#']).next().unwrap_or(path));
    let kind = match path_without_fragment
        .and_then(|path| Path::new(path).extension())
        .and_then(|ext| ext.to_str())
    {
        Some("mdx") => SourceKind::Mdx,
        _ => SourceKind::Markdown,
    };
    source_descriptor_for_kind(path, kind)
}

pub const fn source_language(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Diagram => "mermaid",
        SourceKind::Markdown => "markdown",
        SourceKind::Mdx => "mdx",
    }
}

pub fn analyze_document(
    text: &str,
    analyzer: &Analyzer,
    source: SourceDescriptor,
) -> AnalysisPayload {
    match source.kind {
        SourceKind::Diagram => {
            AnalysisPayload::new(source, analyzer.analyze_source_diagnostics(text))
        }
        SourceKind::Markdown | SourceKind::Mdx => {
            let document = DocumentSource::new(text, source.clone());
            AnalysisPayload::new(source, analyze_document_diagnostics(&document, analyzer))
        }
    }
}

pub fn analyze_document_facts(
    text: &str,
    analyzer: &Analyzer,
    source: SourceDescriptor,
) -> crate::AnalysisFactsPayload {
    analyze_document_result(text, analyzer, source).to_facts_payload()
}

pub fn analyze_document_result(
    text: &str,
    analyzer: &Analyzer,
    source: SourceDescriptor,
) -> AnalysisResult {
    let document = DocumentSource::new(text, source.clone());

    let mut diagnostics = Vec::new();
    let mut analyzed_diagrams = Vec::new();
    for diagram in document.diagrams() {
        let analyzed = analyzer.analyze_diagram(diagram);
        extend_document_diagnostics(
            &mut diagnostics,
            &document,
            diagram,
            analyzed.diagnostics.iter().cloned(),
        );
        analyzed_diagrams.push(analyzed);
    }
    AnalysisResult::new(
        source,
        document.source_map().clone(),
        diagnostics,
        analyzed_diagrams,
    )
}

fn analyze_document_diagnostics(
    document: &DocumentSource,
    analyzer: &Analyzer,
) -> Vec<AnalysisDiagnostic> {
    let mut diagnostics = Vec::new();
    for diagram in document.diagrams() {
        let diagram_diagnostics = analyzer.analyze_diagram_diagnostics(diagram);
        extend_document_diagnostics(&mut diagnostics, document, diagram, diagram_diagnostics);
    }
    diagnostics
}

fn extend_document_diagnostics(
    diagnostics: &mut Vec<AnalysisDiagnostic>,
    document: &DocumentSource,
    diagram: &DocumentDiagram,
    diagram_diagnostics: impl IntoIterator<Item = AnalysisDiagnostic>,
) {
    match document.source().kind {
        SourceKind::Diagram => diagnostics.extend(diagram_diagnostics),
        SourceKind::Markdown | SourceKind::Mdx => diagnostics.extend(
            diagram_diagnostics
                .into_iter()
                .map(|diagnostic| document.remap_diagnostic_to_document(diagram, diagnostic)),
        ),
    }
}

pub(crate) fn whole_document_diagram(text: &str, source: &SourceDescriptor) -> DocumentDiagram {
    DocumentDiagram {
        id: "document".to_string(),
        index: 0,
        kind: DocumentDiagramKind::WholeDocument,
        source: source.clone(),
        start: 0,
        body_start: 0,
        body_end: text.len(),
        end: text.len(),
        text: text.to_string(),
        fence_delimiter: None,
    }
}

fn extract_markdown_diagrams(text: &str, source: &SourceDescriptor) -> Vec<DocumentDiagram> {
    let mut diagrams = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let line_end = next_line_end(text, cursor);
        let line = trim_line_ending(&text[cursor..line_end]);

        if let Some(delimiter) = mermaid_fence_delimiter(line) {
            let body_start = line_end;
            let mut body_end = text.len();
            let mut search_start = body_start;

            while search_start < text.len() {
                let closing_end = next_line_end(text, search_start);
                let closing_line = trim_line_ending(&text[search_start..closing_end]);
                if is_matching_closing_fence(closing_line, delimiter) {
                    body_end = search_start;
                    push_markdown_diagram(
                        &mut diagrams,
                        text,
                        source,
                        cursor,
                        body_start,
                        body_end,
                        closing_end,
                        delimiter,
                    );
                    cursor = closing_end;
                    break;
                }
                search_start = closing_end;
            }

            if body_end == text.len() {
                push_markdown_diagram(
                    &mut diagrams,
                    text,
                    source,
                    cursor,
                    body_start,
                    body_end,
                    text.len(),
                    delimiter,
                );
                break;
            }

            continue;
        }

        cursor = if line_end == cursor {
            text.len()
        } else {
            line_end
        };
    }

    diagrams
}

fn push_markdown_diagram(
    diagrams: &mut Vec<DocumentDiagram>,
    text: &str,
    document_source: &SourceDescriptor,
    start: usize,
    body_start: usize,
    body_end: usize,
    end: usize,
    fence_delimiter: FenceDelimiter,
) {
    let index = diagrams.len();
    diagrams.push(DocumentDiagram {
        id: format!("mermaid-fence-{}", index + 1),
        index,
        kind: DocumentDiagramKind::MermaidFence,
        source: document_source
            .clone()
            .with_diagram_index(index)
            .with_language("mermaid"),
        start,
        body_start,
        body_end,
        end,
        text: text[body_start..body_end].to_string(),
        fence_delimiter: Some(fence_delimiter),
    });
}

fn mermaid_fence_delimiter(line: &str) -> Option<FenceDelimiter> {
    let trimmed = line.trim_start();
    let first = trimmed.as_bytes().first().copied()?;
    let marker = match first {
        b'`' => FenceMarker::Backtick,
        b'~' => FenceMarker::Tilde,
        b':' => FenceMarker::Colon,
        _ => return None,
    };
    let len = repeated_marker_len(trimmed.as_bytes(), first);
    if len < 3 {
        return None;
    }

    let rest = trimmed[len..].trim_start();
    let language = "mermaid";
    let rest = rest.strip_prefix(language)?;
    if rest.chars().all(|ch| ch.is_whitespace()) {
        Some(FenceDelimiter::new(marker, len))
    } else {
        None
    }
}

fn is_matching_closing_fence(line: &str, delimiter: FenceDelimiter) -> bool {
    let trimmed = line.trim_start();
    let marker = delimiter.marker_byte();
    let len = repeated_marker_len(trimmed.as_bytes(), marker);
    if len < delimiter.len() {
        return false;
    }
    trimmed[len..].chars().all(|ch| ch.is_whitespace())
}

fn repeated_marker_len(bytes: &[u8], marker: u8) -> usize {
    bytes.iter().take_while(|byte| **byte == marker).count()
}

fn trim_line_ending(line: &str) -> &str {
    line.strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or(line)
}

fn next_line_end(source: &str, start: usize) -> usize {
    match source[start..].find('\n') {
        Some(relative) => start + relative + 1,
        None => source.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnalysisDiagnostic, Analyzer, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
        DiagnosticRelated,
    };

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
    fn plain_document_source_creates_single_document_diagram() {
        let source = SourceDescriptor::diagram().with_path("file:///tmp/example.mmd");
        let document = DocumentSource::new("flowchart TD\nA-->B\n", source.clone());

        assert_eq!(document.source(), &source);
        assert_eq!(document.diagrams().len(), 1);
        let diagram = &document.diagrams()[0];
        assert_eq!(diagram.id, "document");
        assert_eq!(diagram.kind, DocumentDiagramKind::WholeDocument);
        assert_eq!(diagram.body_start, 0);
        assert_eq!(diagram.body_end, document.text().len());
        assert_eq!(diagram.source, source);
    }

    #[test]
    fn markdown_documents_use_fence_analysis_path() {
        let analyzer = Analyzer::new();
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let payload = analyze_document(
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n",
            &analyzer,
            source.clone(),
        );

        assert_eq!(payload.source, source);
        assert!(payload.valid);
        assert!(payload.diagnostics.is_empty());
    }

    #[test]
    fn markdown_document_source_extracts_stable_fence_sources() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.mdx"));
        let document = DocumentSource::new(
            "before\n```mermaid\nflowchart LR\nA-->B\n```\n~~~mermaid\nsequenceDiagram\nA->>B: Hi\n~~~~\n",
            source.clone(),
        );

        assert_eq!(document.diagrams().len(), 2);
        assert_eq!(document.diagrams()[0].id, "mermaid-fence-1");
        assert_eq!(document.diagrams()[1].id, "mermaid-fence-2");
        assert_eq!(document.diagrams()[0].source.kind, SourceKind::Mdx);
        assert_eq!(document.diagrams()[0].source.diagram_index, Some(0));
        assert_eq!(document.diagrams()[0].source.language, "mermaid");
        assert_eq!(
            document.diagrams()[1].fence_delimiter.unwrap().marker(),
            FenceMarker::Tilde
        );
        assert!(document.diagrams()[0].text.contains("flowchart LR"));
        assert!(document.diagrams()[1].text.contains("sequenceDiagram"));
    }

    #[test]
    fn source_descriptor_for_uri_preserves_uri_and_uses_extension_before_fragment() {
        let source = source_descriptor_for_uri("file:///tmp/example.mdx?rev=1#fence");

        assert_eq!(source.kind, SourceKind::Mdx);
        assert_eq!(
            source.path.as_deref(),
            Some("file:///tmp/example.mdx?rev=1#fence")
        );
        assert_eq!(source.language, "mdx");
    }

    #[test]
    fn markdown_document_source_accepts_commonmark_spaced_info_strings() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new(
            "before\n```` mermaid\nflowchart LR\nA-->B\n````\n~~~ mermaid\nsequenceDiagram\nA->>B: Hi\n~~~\n",
            source,
        );

        assert_eq!(document.diagrams().len(), 2);
        assert!(document.diagrams()[0].text.contains("flowchart LR"));
        assert_eq!(
            document.diagrams()[1].fence_delimiter.unwrap().marker(),
            FenceMarker::Tilde
        );
    }

    #[test]
    fn unclosed_fences_still_create_deterministic_sources() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new("before\n```mermaid\nflowchart TD\nA-->B\n", source);

        assert_eq!(document.diagrams().len(), 1);
        let diagram = &document.diagrams()[0];
        assert_eq!(diagram.end, document.text().len());
        assert_eq!(diagram.body_end, document.text().len());
        assert!(diagram.text.contains("A-->B"));
    }

    #[test]
    fn remaps_diagnostics_back_into_host_document_coordinates() {
        let source = "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter";
        let document = DocumentSource::new(
            source,
            source_descriptor_for_markdown_path(Some("example.md")),
        );
        let diagram = &document.diagrams()[0];
        let local_map = SourceMap::new(&diagram.text);
        let start = local_map.source().find('A').unwrap();
        let end = local_map.source().find("-->").unwrap();
        let diagnostic = AnalysisDiagnostic::error(
            "merman.parse.diagram_parse",
            DiagnosticCategory::Parse,
            "boom",
        )
        .with_span(local_map.span(start, end).unwrap());

        let remapped = document.remap_diagnostic_to_document(diagram, diagnostic);

        assert_eq!(remapped.span.unwrap().line, 4);
        assert_eq!(remapped.related.len(), 1);
    }

    #[test]
    fn remaps_existing_related_spans_back_into_host_document_coordinates() {
        let source = "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter";
        let document = DocumentSource::new(
            source,
            source_descriptor_for_markdown_path(Some("example.md")),
        );
        let diagram = &document.diagrams()[0];
        let local_map = SourceMap::new(&diagram.text);
        let start = local_map.source().find('B').unwrap();
        let end = start + 1;
        let mut diagnostic = AnalysisDiagnostic::error(
            "merman.parse.diagram_parse",
            DiagnosticCategory::Parse,
            "boom",
        );
        diagnostic.related.push(DiagnosticRelated {
            message: "related node".to_string(),
            span: Some(local_map.span(start, end).unwrap()),
        });

        let remapped = document.remap_diagnostic_to_document(diagram, diagnostic);

        let related_span = remapped.related[0].span.as_ref().unwrap();
        assert_eq!(&source[related_span.byte_start..related_span.byte_end], "B");
        assert_eq!(related_span.line, 4);
    }

    #[test]
    fn remaps_fix_edits_back_into_host_document_coordinates() {
        let source = "before\n```mermaid\n%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n```\nafter";
        let document = DocumentSource::new(
            source,
            source_descriptor_for_markdown_path(Some("example.md")),
        );
        let diagram = &document.diagrams()[0];
        let local_map = SourceMap::new(&diagram.text);
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

        let remapped = document.remap_diagnostic_to_document(diagram, diagnostic);
        let edit_span = &remapped.fixes[0].edits[0].span;

        assert_eq!(
            &source[edit_span.byte_start..edit_span.byte_end],
            "initialize"
        );
        assert_eq!(edit_span.line, 3);
        assert_eq!(remapped.fixes[0].edits[0].replacement, "init");
    }
}
