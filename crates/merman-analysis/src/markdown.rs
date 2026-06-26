use crate::{
    AnalysisDiagnostic, AnalysisPayload, Analyzer, DiagnosticRelated, DiagnosticSpan,
    SourceDescriptor, SourceKind, SourceMap,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MarkdownChart {
    pub start: usize,
    pub body_start: usize,
    pub end: usize,
    pub definition: String,
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
    let mut charts = Vec::new();
    let mut cursor = 0;

    while cursor < source.len() {
        let line_end = next_line_end(source, cursor);
        let line = trim_line_ending(&source[cursor..line_end]);

        if let Some((marker, opener_trimmed_len)) = mermaid_fence_kind(line) {
            let body_start = line_end;
            let mut body_end = source.len();
            let mut search_start = body_start;

            while search_start < source.len() {
                let closing_end = next_line_end(source, search_start);
                let closing_line = trim_line_ending(&source[search_start..closing_end]);
                if is_matching_closing_fence(closing_line, marker) {
                    body_end = search_start;
                    charts.push(MarkdownChart {
                        start: cursor,
                        body_start,
                        end: closing_end,
                        definition: source[body_start..body_end].to_string(),
                    });
                    cursor = closing_end;
                    break;
                }
                search_start = closing_end;
            }

            if body_end == source.len() {
                charts.push(MarkdownChart {
                    start: cursor,
                    body_start,
                    end: source.len(),
                    definition: source[body_start..].to_string(),
                });
                break;
            }

            let _ = opener_trimmed_len;
            continue;
        }

        cursor = if line_end == cursor {
            source.len()
        } else {
            line_end
        };
    }

    charts
}

pub fn analyze_markdown_source(
    text: &str,
    analyzer: &Analyzer,
    document_source: SourceDescriptor,
) -> AnalysisPayload {
    let document_map = SourceMap::new(text);
    let mut diagnostics = Vec::new();

    for (index, chart) in extract_charts_with_spans(text).into_iter().enumerate() {
        let fence_span = document_map.span(chart.start, chart.end).ok();
        let mut payload = analyzer.analyze(&chart.definition);

        diagnostics.extend(payload.diagnostics.drain(..).map(|diagnostic| {
            remap_markdown_diagnostic(
                diagnostic,
                &document_map,
                chart.body_start,
                fence_span.clone(),
                index,
            )
        }));
    }

    AnalysisPayload::new(document_source, diagnostics)
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
        diagnostic.related.push(DiagnosticRelated {
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

fn mermaid_fence_kind(line: &str) -> Option<(&'static str, usize)> {
    let trimmed = line.trim_start();
    let indentation = line.len().saturating_sub(trimmed.len());

    for (marker, prefix) in [("```", "```mermaid"), (":::", ":::mermaid")] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if rest.chars().all(|ch| ch.is_whitespace()) {
                return Some((marker, indentation + prefix.len()));
            }
        }
    }

    None
}

fn is_matching_closing_fence(line: &str, marker: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix(marker) else {
        return false;
    };
    rest.chars().all(|ch| ch.is_whitespace())
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
    use crate::{AnalysisDiagnostic, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit};

    #[test]
    fn extracts_backtick_and_colon_mermaid_blocks() {
        let source = "before\n```mermaid\nflowchart LR\nA-->B\n```\n:::mermaid\nsequenceDiagram\nA->>B: Hi\n:::\nafter";
        let charts = extract_charts_with_spans(source);

        assert_eq!(charts.len(), 2);
        assert!(charts[0].definition.contains("flowchart LR"));
        assert!(charts[1].definition.contains("sequenceDiagram"));
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
