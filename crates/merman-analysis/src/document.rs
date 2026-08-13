use crate::analyzer::{CaptureCancellation, CaptureMode};
use crate::diagnostic_projection::{
    DiagnosticCandidate, append_diagnostic_candidates_cancellable,
    materialize_diagnostic_candidates,
};
use crate::{
    AnalysisCaptureOutcome, AnalysisDiagnostic, AnalysisGeneration, AnalysisPayload, Analyzer,
    DiagnosticFix, DiagnosticFixEdit, DiagnosticRelated, DiagnosticSpan, SourceDescriptor,
    SourceKind, SourceMap,
};
use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;

pub use crate::source_map::SharedTextSlice;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceDelimiterSpans {
    pub opening: std::ops::Range<usize>,
    pub closing: Option<std::ops::Range<usize>>,
}

impl FenceDelimiter {
    pub const fn new(marker: FenceMarker, len: usize) -> Self {
        Self { marker, len }
    }

    pub const fn marker(self) -> FenceMarker {
        self.marker
    }

    pub const fn marker_len(self) -> usize {
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
    pub text: SharedTextSlice,
    pub fence_delimiter: Option<FenceDelimiter>,
    pub fence_delimiter_spans: Option<FenceDelimiterSpans>,
}

#[derive(Debug, Clone)]
pub struct DocumentSource {
    source: SourceDescriptor,
    text: Arc<str>,
    source_map: SourceMap,
    diagrams: Vec<DocumentDiagram>,
}

enum DocumentSourceBuildOutcome {
    Ready(DocumentSource),
    DiagramLimitExceeded(MarkdownDocumentDiagramLimitExceeded),
}

enum CaptureText<'a> {
    Borrowed(&'a str),
    Shared(Arc<str>),
}

impl CaptureText<'_> {
    fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(text) => text,
            Self::Shared(text) => text.as_ref(),
        }
    }

    fn to_shared(&self) -> Arc<str> {
        match self {
            Self::Borrowed(text) => Arc::from(*text),
            Self::Shared(text) => Arc::clone(text),
        }
    }
}

enum CapturedSource {
    Rejected(crate::AnalysisRejection),
    Diagnostics(Vec<DiagnosticCandidate>),
    Generation(AnalysisGeneration),
}

impl DocumentSource {
    pub fn new(text: impl Into<Arc<str>>, source: SourceDescriptor) -> Self {
        let text = text.into();
        let source_map = SourceMap::new(Arc::clone(&text));
        let diagrams = match source.kind {
            SourceKind::Markdown | SourceKind::Mdx => {
                let cancellation = crate::AnalysisCancellationToken::new();
                extract_markdown_diagrams(&text, &source, &cancellation)
                    .expect("a private analysis cancellation token cannot be cancelled")
            }
            SourceKind::Diagram => vec![whole_document_diagram(Arc::clone(&text), &source)],
        };

        Self {
            source,
            text,
            source_map,
            diagrams,
        }
    }

    fn new_bounded_cancellable(
        input: &CaptureText<'_>,
        source: SourceDescriptor,
        max_document_diagrams: Option<usize>,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<DocumentSourceBuildOutcome, crate::AnalysisCancelled> {
        cancellation.checkpoint()?;
        // Keep borrowed input unpromoted until the bounded document scan admits it. Shared input
        // retains the caller's Arc allocation through the same path.
        let scanned_diagrams = match source.kind {
            SourceKind::Markdown | SourceKind::Mdx => {
                match scan_markdown_diagrams_bounded(
                    input.as_str(),
                    max_document_diagrams,
                    cancellation,
                )? {
                    Ok(diagrams) => Some(diagrams),
                    Err(exceeded) => {
                        return Ok(DocumentSourceBuildOutcome::DiagramLimitExceeded(exceeded));
                    }
                }
            }
            SourceKind::Diagram => None,
        };
        cancellation.checkpoint()?;
        let text = input.to_shared();
        let diagrams = match scanned_diagrams {
            Some(diagrams) => {
                materialize_markdown_diagrams(&text, &source, diagrams, cancellation)?
            }
            None => vec![whole_document_diagram(Arc::clone(&text), &source)],
        };
        let source_map = SourceMap::new_cancellable(Arc::clone(&text), cancellation)?;
        cancellation.checkpoint()?;
        Ok(DocumentSourceBuildOutcome::Ready(Self {
            source,
            text,
            source_map,
            diagrams,
        }))
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.source
    }

    pub fn text(&self) -> &str {
        self.text.as_ref()
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
        diagnostic: AnalysisDiagnostic,
    ) -> AnalysisDiagnostic {
        let source_context = fence_source_context(&self.source_map, diagram);
        let mut diagnostic = remap_diagnostic_spans(
            &self.source_map,
            diagram,
            source_context.as_ref().and_then(|related| related.span),
            diagnostic,
        );
        diagnostic.related.extend(source_context);
        diagnostic
    }
}

fn remap_diagnostic_spans(
    source_map: &SourceMap,
    diagram: &DocumentDiagram,
    fence_span: Option<DiagnosticSpan>,
    diagnostic: AnalysisDiagnostic,
) -> AnalysisDiagnostic {
    let cancellation = crate::AnalysisCancellationToken::new();
    let mut remapped_fix_edits = BTreeMap::new();
    remap_diagnostic_spans_cancellable(
        source_map,
        diagram,
        fence_span,
        diagnostic,
        &mut remapped_fix_edits,
        &cancellation,
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

fn remap_diagnostic_spans_cancellable(
    source_map: &SourceMap,
    diagram: &DocumentDiagram,
    fence_span: Option<DiagnosticSpan>,
    mut diagnostic: AnalysisDiagnostic,
    remapped_fix_edits: &mut BTreeMap<usize, Arc<[DiagnosticFixEdit]>>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<AnalysisDiagnostic, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    remap_diagnostic_locations_cancellable(
        source_map,
        diagram,
        fence_span,
        DiagnosticLocationsMut {
            span: &mut diagnostic.span,
            related: &mut diagnostic.related,
            fixes: &mut diagnostic.fixes,
        },
        remapped_fix_edits,
        cancellation,
    )?;
    Ok(diagnostic)
}

struct DiagnosticLocationsMut<'a> {
    span: &'a mut Option<DiagnosticSpan>,
    related: &'a mut [DiagnosticRelated],
    fixes: &'a mut Vec<DiagnosticFix>,
}

fn remap_diagnostic_locations_cancellable(
    source_map: &SourceMap,
    diagram: &DocumentDiagram,
    fence_span: Option<DiagnosticSpan>,
    locations: DiagnosticLocationsMut<'_>,
    remapped_fix_edits: &mut BTreeMap<usize, Arc<[DiagnosticFixEdit]>>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<(), crate::AnalysisCancelled> {
    *locations.span = match locations.span.take() {
        Some(span) => remap_span_to_document_cancellable(source_map, diagram, span, cancellation)?
            .or(fence_span),
        None => fence_span,
    };

    for (index, related) in locations.related.iter_mut().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        related.span = match related.span.take() {
            Some(span) => {
                remap_span_to_document_cancellable(source_map, diagram, span, cancellation)?
            }
            None => None,
        };
    }

    for (fix_index, fix) in locations.fixes.iter_mut().enumerate() {
        if fix_index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        let source_allocation = Arc::as_ptr(&fix.edits) as *const DiagnosticFixEdit as usize;
        if let Some(edits) = remapped_fix_edits.get(&source_allocation) {
            fix.edits = Arc::clone(edits);
            continue;
        }

        let mut remapped = Vec::with_capacity(fix.edits.len());
        for (edit_index, edit) in fix.edits.iter().enumerate() {
            if edit_index.is_multiple_of(128) {
                cancellation.checkpoint()?;
            }
            let mut edit = edit.clone();
            let Some(span) =
                remap_span_to_document_cancellable(source_map, diagram, edit.span, cancellation)?
            else {
                remapped.clear();
                break;
            };
            edit.span = span;
            remapped.push(edit);
        }
        let remapped = Arc::<[DiagnosticFixEdit]>::from(remapped);
        remapped_fix_edits.insert(source_allocation, Arc::clone(&remapped));
        fix.edits = remapped;
    }
    locations.fixes.retain(|fix| !fix.edits.is_empty());

    cancellation.checkpoint()?;
    Ok(())
}

fn remap_span_to_document_cancellable(
    source_map: &SourceMap,
    diagram: &DocumentDiagram,
    span: DiagnosticSpan,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<DiagnosticSpan>, crate::AnalysisCancelled> {
    let Some(start) = diagram.body_start.checked_add(span.byte_start) else {
        return Ok(None);
    };
    let Some(end) = diagram.body_start.checked_add(span.byte_end) else {
        return Ok(None);
    };
    Ok(source_map.span_cancellable(start, end, cancellation)?.ok())
}

fn fence_source_context(
    source_map: &SourceMap,
    diagram: &DocumentDiagram,
) -> Option<DiagnosticRelated> {
    let cancellation = crate::AnalysisCancellationToken::new();
    fence_source_context_cancellable(source_map, diagram, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

fn fence_source_context_cancellable(
    source_map: &SourceMap,
    diagram: &DocumentDiagram,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<DiagnosticRelated>, crate::AnalysisCancelled> {
    if !diagram.is_fence() {
        return Ok(None);
    }
    let Some(span) = source_map
        .span_cancellable(diagram.start, diagram.end, cancellation)?
        .ok()
    else {
        return Ok(None);
    };
    Ok(Some(DiagnosticRelated {
        message: format!("Mermaid fence {}", diagram.index + 1),
        span: Some(span),
    }))
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
        Some(ext) if crate::markdown::is_mdx_extension(ext) => SourceKind::Mdx,
        Some(ext) if crate::markdown::is_markdown_extension(ext) => SourceKind::Markdown,
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
        Some(ext) if crate::markdown::is_mdx_extension(ext) => SourceKind::Mdx,
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
    let cancellation = crate::AnalysisCancellationToken::new();
    match capture_source_cancellable(
        CaptureText::Borrowed(text),
        analyzer,
        source.clone(),
        CaptureMode::DiagnosticsOnly,
        CaptureCancellation::RecoverParserCancellation(&cancellation),
    )
    .expect("a private analysis cancellation token cannot be cancelled")
    {
        CapturedSource::Rejected(rejection) => rejection.into_payload(),
        CapturedSource::Diagnostics(candidates) => AnalysisPayload::new(
            source,
            materialize_diagnostic_candidates(&candidates, analyzer.options().diagnostic_policy()),
        ),
        CapturedSource::Generation(_) => {
            unreachable!("diagnostics-only capture cannot produce a rich generation")
        }
    }
}

pub fn analyze_document_facts(
    text: &str,
    analyzer: &Analyzer,
    source: SourceDescriptor,
) -> crate::AnalysisFactsPayload {
    match analyze_document_generation(text, analyzer, source) {
        AnalysisCaptureOutcome::Ready(generation) => {
            generation.to_facts_payload(analyzer.options().diagnostic_policy())
        }
        AnalysisCaptureOutcome::Rejected(rejection) => {
            crate::AnalysisFactsPayload::from_rejection(&rejection)
        }
    }
}

pub fn analyze_document_generation(
    text: &str,
    analyzer: &Analyzer,
    source: SourceDescriptor,
) -> AnalysisCaptureOutcome {
    let cancellation = crate::AnalysisCancellationToken::new();
    capture_generation_cancellable(
        CaptureText::Borrowed(text),
        analyzer,
        source,
        CaptureCancellation::RecoverParserCancellation(&cancellation),
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

pub fn analyze_document_generation_shared(
    text: Arc<str>,
    analyzer: &Analyzer,
    source: SourceDescriptor,
) -> AnalysisCaptureOutcome {
    let cancellation = crate::AnalysisCancellationToken::new();
    capture_generation_cancellable(
        CaptureText::Shared(text),
        analyzer,
        source,
        CaptureCancellation::RecoverParserCancellation(&cancellation),
    )
    .expect("a private analysis cancellation token cannot be cancelled")
}

pub fn analyze_document_generation_shared_cancellable(
    text: Arc<str>,
    analyzer: &Analyzer,
    source: SourceDescriptor,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<AnalysisCaptureOutcome, crate::AnalysisCancelled> {
    capture_generation_cancellable(
        CaptureText::Shared(text),
        analyzer,
        source,
        CaptureCancellation::Propagate(cancellation),
    )
}

fn capture_generation_cancellable(
    input: CaptureText<'_>,
    analyzer: &Analyzer,
    source: SourceDescriptor,
    control: CaptureCancellation<'_>,
) -> Result<AnalysisCaptureOutcome, crate::AnalysisCancelled> {
    match capture_source_cancellable(input, analyzer, source, CaptureMode::RichFacts, control)? {
        CapturedSource::Rejected(rejection) => Ok(AnalysisCaptureOutcome::Rejected(rejection)),
        CapturedSource::Generation(generation) => Ok(AnalysisCaptureOutcome::Ready(generation)),
        CapturedSource::Diagnostics(_) => {
            unreachable!("rich capture cannot produce diagnostics-only storage")
        }
    }
}

fn capture_source_cancellable(
    input: CaptureText<'_>,
    analyzer: &Analyzer,
    source: SourceDescriptor,
    mode: CaptureMode,
    control: CaptureCancellation<'_>,
) -> Result<CapturedSource, crate::AnalysisCancelled> {
    let cancellation = control.cancellation();
    cancellation.checkpoint()?;
    let resource_limits = analyzer.options().resource_limits();
    if let Some(rejection) = crate::source_limits::source_limit_rejection_cancellable(
        input.as_str(),
        &source,
        resource_limits.max_source_bytes(),
        cancellation,
    )? {
        return Ok(CapturedSource::Rejected(rejection));
    }

    let document = DocumentSource::new_bounded_cancellable(
        &input,
        source.clone(),
        resource_limits.max_document_diagrams(),
        cancellation,
    )?;
    let document = match document {
        DocumentSourceBuildOutcome::Ready(document) => document,
        DocumentSourceBuildOutcome::DiagramLimitExceeded(exceeded) => {
            let rejection =
                crate::document_limits::document_diagram_limit_rejection_from_exceeded_cancellable(
                    input.as_str(),
                    &source,
                    resource_limits
                        .max_document_diagrams()
                        .expect("a diagram limit must exist when extraction exceeds it"),
                    exceeded,
                    cancellation,
                )?;
            return Ok(CapturedSource::Rejected(rejection));
        }
    };
    let request_analyzer = analyzer.with_capture_source(source);
    cancellation.checkpoint()?;

    if document.diagrams().is_empty() {
        return Ok(match mode {
            CaptureMode::DiagnosticsOnly => CapturedSource::Diagnostics(Vec::new()),
            CaptureMode::RichFacts => CapturedSource::Generation(AnalysisGeneration::new(
                document.source_map().clone(),
                Vec::new(),
                &request_analyzer,
            )),
        });
    }
    let operation_analyzer = match request_analyzer.try_for_operation() {
        Ok(analyzer) => analyzer,
        Err(error) => {
            let candidates =
                request_analyzer.runtime_policy_candidates(error, document.source_map());
            return Ok(match mode {
                CaptureMode::DiagnosticsOnly => CapturedSource::Diagnostics(candidates),
                CaptureMode::RichFacts => CapturedSource::Generation(
                    AnalysisGeneration::new(
                        document.source_map().clone(),
                        Vec::new(),
                        &request_analyzer,
                    )
                    .with_document_candidates(candidates),
                ),
            });
        }
    };
    cancellation.checkpoint()?;

    let mut diagnostic_candidates = Vec::new();
    let mut analyzed_diagrams = Vec::new();
    for diagram in document.diagrams() {
        cancellation.checkpoint()?;
        let captured = operation_analyzer.capture_document_diagram_cancellable(
            diagram,
            document.source_map(),
            mode,
            control,
        )?;
        match mode {
            CaptureMode::DiagnosticsOnly => {
                append_diagnostic_candidates_cancellable(
                    &mut diagnostic_candidates,
                    captured.candidates,
                    cancellation,
                )?;
            }
            CaptureMode::RichFacts => {
                analyzed_diagrams.push(crate::AnalyzedDiagram::from_document_diagram(
                    diagram,
                    captured
                        .syntax
                        .expect("rich diagram capture must retain syntax facts"),
                    captured.candidates,
                    captured.parse_disposition,
                ))
            }
        }
        cancellation.checkpoint()?;
    }
    cancellation.checkpoint()?;
    Ok(match mode {
        CaptureMode::DiagnosticsOnly => CapturedSource::Diagnostics(diagnostic_candidates),
        CaptureMode::RichFacts => CapturedSource::Generation(AnalysisGeneration::new(
            document.source_map().clone(),
            analyzed_diagrams,
            &operation_analyzer,
        )),
    })
}

pub(crate) fn normalize_document_diagnostic_candidates_cancellable(
    source_map: &SourceMap,
    diagram: &DocumentDiagram,
    candidates: Vec<crate::diagnostic_projection::DiagnosticCandidate>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<crate::diagnostic_projection::DiagnosticCandidate>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    if candidates.is_empty() || !diagram.is_fence() {
        return Ok(candidates);
    }

    let source_context = fence_source_context_cancellable(source_map, diagram, cancellation)?;
    let fence_span = source_context.as_ref().and_then(|related| related.span);
    let mut normalized = Vec::with_capacity(candidates.len());
    let mut remapped_fix_edits = BTreeMap::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        let candidate = candidate.try_map_locations(|span, related, fixes| {
            remap_diagnostic_locations_cancellable(
                source_map,
                diagram,
                fence_span,
                DiagnosticLocationsMut {
                    span,
                    related,
                    fixes,
                },
                &mut remapped_fix_edits,
                cancellation,
            )
        })?;
        normalized.push(match &source_context {
            Some(context) => candidate.with_trailing_source_context(context.clone()),
            None => candidate,
        });
    }
    cancellation.checkpoint()?;
    Ok(normalized)
}

pub(crate) fn whole_document_diagram(text: Arc<str>, source: &SourceDescriptor) -> DocumentDiagram {
    let len = text.len();
    DocumentDiagram {
        id: "document".to_string(),
        index: 0,
        kind: DocumentDiagramKind::WholeDocument,
        source: source.clone(),
        start: 0,
        body_start: 0,
        body_end: len,
        end: len,
        text: SharedTextSlice::new(text, 0, len),
        fence_delimiter: None,
        fence_delimiter_spans: None,
    }
}

fn extract_markdown_diagrams(
    text: &Arc<str>,
    source: &SourceDescriptor,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DocumentDiagram>, crate::AnalysisCancelled> {
    match scan_markdown_diagrams_bounded(text.as_ref(), None, cancellation)? {
        Ok(diagrams) => materialize_markdown_diagrams(text, source, diagrams, cancellation),
        Err(_) => unreachable!("an unlimited extraction cannot exceed a diagram limit"),
    }
}

fn scan_markdown_diagrams_bounded(
    text: &str,
    max_document_diagrams: Option<usize>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<
    Result<Vec<ScannedMarkdownDiagram>, MarkdownDocumentDiagramLimitExceeded>,
    crate::AnalysisCancelled,
> {
    let mut diagrams = Vec::new();
    let mut observed_document_diagrams = 0usize;
    let result = visit_markdown_diagrams(
        text,
        cancellation,
        |opening_marker| {
            observed_document_diagrams = observed_document_diagrams.saturating_add(1);
            if max_document_diagrams.is_some_and(|limit| observed_document_diagrams > limit) {
                ControlFlow::Break(MarkdownDocumentDiagramLimitExceeded {
                    observed_document_diagrams,
                    opening_marker,
                })
            } else {
                ControlFlow::Continue(())
            }
        },
        |bounds, delimiter| {
            diagrams.push(ScannedMarkdownDiagram { bounds, delimiter });
        },
    )?;
    cancellation.checkpoint()?;
    Ok(match result {
        ControlFlow::Continue(()) => Ok(diagrams),
        ControlFlow::Break(exceeded) => Err(exceeded),
    })
}

fn materialize_markdown_diagrams(
    text: &Arc<str>,
    source: &SourceDescriptor,
    diagrams: Vec<ScannedMarkdownDiagram>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<DocumentDiagram>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    let mut materialized = Vec::with_capacity(diagrams.len());
    for (index, diagram) in diagrams.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        push_markdown_diagram(
            &mut materialized,
            Arc::clone(text),
            source,
            diagram.bounds,
            diagram.delimiter,
        );
    }
    cancellation.checkpoint()?;
    Ok(materialized)
}

pub(crate) struct MarkdownDocumentDiagramLimitExceeded {
    pub(crate) observed_document_diagrams: usize,
    pub(crate) opening_marker: std::ops::Range<usize>,
}

pub(crate) fn markdown_document_diagram_limit_exceeded_cancellable(
    document_text: &str,
    max_document_diagrams: usize,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<MarkdownDocumentDiagramLimitExceeded>, crate::AnalysisCancelled> {
    let mut observed_document_diagrams = 0usize;
    let result = visit_markdown_diagrams(
        document_text,
        cancellation,
        |opening_marker| {
            observed_document_diagrams = observed_document_diagrams.saturating_add(1);
            if observed_document_diagrams > max_document_diagrams {
                ControlFlow::Break(MarkdownDocumentDiagramLimitExceeded {
                    observed_document_diagrams,
                    opening_marker,
                })
            } else {
                ControlFlow::Continue(())
            }
        },
        |_, _| {},
    )?;
    Ok(match result {
        ControlFlow::Continue(()) => None,
        ControlFlow::Break(exceeded) => Some(exceeded),
    })
}

fn visit_markdown_diagrams<B>(
    document_text: &str,
    cancellation: &crate::AnalysisCancellationToken,
    mut visit_opening: impl FnMut(std::ops::Range<usize>) -> ControlFlow<B>,
    mut visit: impl FnMut(MarkdownFenceBounds, FenceDelimiter),
) -> Result<ControlFlow<B>, crate::AnalysisCancelled> {
    let mut cursor = 0;

    while cursor < document_text.len() {
        cancellation.checkpoint()?;
        let line_end = next_line_end_cancellable(document_text, cursor, cancellation)?;
        let line = trim_line_ending(&document_text[cursor..line_end]);

        if let Some(opening) = markdown_fence_opening(line, cancellation)? {
            if !opening.is_mermaid {
                cursor =
                    skip_markdown_fence(document_text, line_end, opening.delimiter, cancellation)?;
                continue;
            }

            let delimiter = opening.delimiter;
            let opening_marker = cursor + opening.marker_offset
                ..cursor + opening.marker_offset + delimiter.marker_len();
            if let ControlFlow::Break(value) = visit_opening(opening_marker.clone()) {
                return Ok(ControlFlow::Break(value));
            }
            let body_start = line_end;
            let mut body_end = document_text.len();
            let mut search_start = body_start;

            while search_start < document_text.len() {
                cancellation.checkpoint()?;
                let closing_end =
                    next_line_end_cancellable(document_text, search_start, cancellation)?;
                let closing_line = trim_line_ending(&document_text[search_start..closing_end]);
                if let Some(closing_marker) =
                    matching_closing_fence_marker(closing_line, delimiter, cancellation)?
                {
                    body_end = search_start;
                    visit(
                        MarkdownFenceBounds {
                            fence: cursor..closing_end,
                            body: body_start..body_end,
                            opening_marker: opening_marker.clone(),
                            closing_marker: Some(
                                search_start + closing_marker.start
                                    ..search_start + closing_marker.end,
                            ),
                        },
                        delimiter,
                    );
                    cursor = closing_end;
                    break;
                }
                search_start = closing_end;
            }

            if body_end == document_text.len() {
                visit(
                    MarkdownFenceBounds {
                        fence: cursor..document_text.len(),
                        body: body_start..body_end,
                        opening_marker,
                        closing_marker: None,
                    },
                    delimiter,
                );
                break;
            }

            continue;
        }

        cursor = if line_end == cursor {
            document_text.len()
        } else {
            line_end
        };
    }

    cancellation.checkpoint()?;
    Ok(ControlFlow::Continue(()))
}

struct MarkdownFenceBounds {
    fence: std::ops::Range<usize>,
    body: std::ops::Range<usize>,
    opening_marker: std::ops::Range<usize>,
    closing_marker: Option<std::ops::Range<usize>>,
}

struct ScannedMarkdownDiagram {
    bounds: MarkdownFenceBounds,
    delimiter: FenceDelimiter,
}

fn push_markdown_diagram(
    diagrams: &mut Vec<DocumentDiagram>,
    text: Arc<str>,
    document_source: &SourceDescriptor,
    bounds: MarkdownFenceBounds,
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
        start: bounds.fence.start,
        body_start: bounds.body.start,
        body_end: bounds.body.end,
        end: bounds.fence.end,
        text: SharedTextSlice::new(text, bounds.body.start, bounds.body.end),
        fence_delimiter: Some(fence_delimiter),
        fence_delimiter_spans: Some(FenceDelimiterSpans {
            opening: bounds.opening_marker,
            closing: bounds.closing_marker,
        }),
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkdownFenceOpening {
    delimiter: FenceDelimiter,
    is_mermaid: bool,
    marker_offset: usize,
}

const MARKDOWN_FENCE_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;

fn markdown_fence_opening(
    line: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<MarkdownFenceOpening>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    let Some(trimmed) = trim_fence_indent(line) else {
        return Ok(None);
    };
    let marker_offset = line.len() - trimmed.len();
    let Some(first) = trimmed.as_bytes().first().copied() else {
        return Ok(None);
    };
    let marker = match first {
        b'`' => FenceMarker::Backtick,
        b'~' => FenceMarker::Tilde,
        b':' => FenceMarker::Colon,
        _ => return Ok(None),
    };
    let len = repeated_marker_len_cancellable(trimmed.as_bytes(), first, cancellation)?;
    if len < 3 {
        return Ok(None);
    }

    let rest = trim_start_whitespace_cancellable(&trimmed[len..], cancellation)?;
    if rest.is_empty() {
        return Ok(Some(MarkdownFenceOpening {
            delimiter: FenceDelimiter::new(marker, len),
            is_mermaid: false,
            marker_offset,
        }));
    }

    let language = "mermaid";
    let language_len = language.len();
    if !rest
        .as_bytes()
        .get(..language_len)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(language.as_bytes()))
    {
        return Ok(Some(MarkdownFenceOpening {
            delimiter: FenceDelimiter::new(marker, len),
            is_mermaid: false,
            marker_offset,
        }));
    }
    let tail = &rest[language_len..];
    let is_mermaid = tail.is_empty() || tail.chars().next().is_some_and(char::is_whitespace);
    cancellation.checkpoint()?;
    Ok(Some(MarkdownFenceOpening {
        delimiter: FenceDelimiter::new(marker, len),
        is_mermaid,
        marker_offset,
    }))
}

fn matching_closing_fence_marker(
    line: &str,
    delimiter: FenceDelimiter,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<std::ops::Range<usize>>, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    let Some(trimmed) = trim_fence_indent(line) else {
        return Ok(None);
    };
    let marker_offset = line.len() - trimmed.len();
    let marker = delimiter.marker_byte();
    let len = repeated_marker_len_cancellable(trimmed.as_bytes(), marker, cancellation)?;
    if len < delimiter.marker_len() {
        return Ok(None);
    }
    Ok(all_whitespace_cancellable(&trimmed[len..], cancellation)?
        .then_some(marker_offset..marker_offset + len))
}

fn skip_markdown_fence(
    text: &str,
    mut cursor: usize,
    delimiter: FenceDelimiter,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<usize, crate::AnalysisCancelled> {
    while cursor < text.len() {
        cancellation.checkpoint()?;
        let line_end = next_line_end_cancellable(text, cursor, cancellation)?;
        let line = trim_line_ending(&text[cursor..line_end]);
        if matching_closing_fence_marker(line, delimiter, cancellation)?.is_some() {
            return Ok(line_end);
        }
        cursor = line_end;
    }
    Ok(text.len())
}

fn trim_fence_indent(line: &str) -> Option<&str> {
    let mut spaces = 0usize;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b' ' if spaces < 3 => spaces += 1,
            b' ' => return None,
            b'\t' => return None,
            _ => return Some(&line[index..]),
        }
    }
    Some("")
}

fn repeated_marker_len_cancellable(
    bytes: &[u8],
    marker: u8,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<usize, crate::AnalysisCancelled> {
    let mut index = 0usize;
    let mut next_checkpoint = 0usize;
    while bytes.get(index) == Some(&marker) {
        if index >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = index.saturating_add(MARKDOWN_FENCE_SCAN_CHECKPOINT_BYTES);
        }
        index += 1;
    }
    cancellation.checkpoint()?;
    Ok(index)
}

fn trim_start_whitespace_cancellable<'a>(
    text: &'a str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<&'a str, crate::AnalysisCancelled> {
    let mut trimmed_start = 0usize;
    let mut next_checkpoint = 0usize;
    for (offset, character) in text.char_indices() {
        if offset >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = offset.saturating_add(MARKDOWN_FENCE_SCAN_CHECKPOINT_BYTES);
        }
        if !character.is_whitespace() {
            return Ok(&text[offset..]);
        }
        trimmed_start = offset + character.len_utf8();
    }
    cancellation.checkpoint()?;
    Ok(&text[trimmed_start..])
}

fn all_whitespace_cancellable(
    text: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<bool, crate::AnalysisCancelled> {
    let mut next_checkpoint = 0usize;
    for (offset, character) in text.char_indices() {
        if offset >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = offset.saturating_add(MARKDOWN_FENCE_SCAN_CHECKPOINT_BYTES);
        }
        if !character.is_whitespace() {
            return Ok(false);
        }
    }
    cancellation.checkpoint()?;
    Ok(true)
}

fn trim_line_ending(line: &str) -> &str {
    line.strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or_else(|| line.strip_suffix('\r').unwrap_or(line))
}

fn next_line_end_cancellable(
    source: &str,
    start: usize,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<usize, crate::AnalysisCancelled> {
    let bytes = source.as_bytes();
    let mut index = start;
    let mut next_checkpoint = start;
    while index < bytes.len() {
        if index >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = index.saturating_add(MARKDOWN_FENCE_SCAN_CHECKPOINT_BYTES);
        }
        match bytes[index] {
            b'\n' => return Ok(index + 1),
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => return Ok(index + 2),
            b'\r' => return Ok(index + 1),
            _ => index += 1,
        }
    }
    Ok(source.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnalysisDiagnostic, AnalysisOptions, Analyzer, DiagnosticCategory, DiagnosticFix,
        DiagnosticFixEdit, DiagnosticRelated,
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
    fn cancellable_document_analysis_never_turns_cancellation_into_a_diagnostic() {
        let analyzer = Analyzer::new();
        let source = SourceDescriptor::diagram().with_path("file:///tmp/cancelled.mmd");
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel();

        let result = analyze_document_generation_shared_cancellable(
            Arc::from("flowchart TD\nA-->B\n"),
            &analyzer,
            source,
            &cancellation,
        );

        assert!(matches!(result, Err(crate::AnalysisCancelled)));
    }

    #[test]
    fn fence_candidate_normalization_observes_cancellation() {
        let text = Arc::<str>::from("```mermaid\nflowchart TD\nA-->B\n```\n");
        let source = source_descriptor_for_markdown_path(Some("fixture.md"));
        let document = DocumentSource::new(Arc::clone(&text), source);
        let diagram = &document.diagrams()[0];
        let candidates = (0..512)
            .map(|index| {
                crate::diagnostic_projection::DiagnosticCandidate::new(
                    crate::rules::DIAGRAM_PARSE_RULE,
                    format!("diagnostic {index}"),
                )
            })
            .collect();
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(1);

        assert!(matches!(
            normalize_document_diagnostic_candidates_cancellable(
                document.source_map(),
                diagram,
                candidates,
                &cancellation,
            ),
            Err(crate::AnalysisCancelled)
        ));
    }

    #[test]
    fn document_diagram_limit_stops_at_the_first_excess_opener() {
        let source = format!(
            "```mermaid\nflowchart TD\nA-->B\n```\n```mermaid\n{}",
            "x".repeat(128 * 1024)
        );
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(64);

        let exceeded =
            markdown_document_diagram_limit_exceeded_cancellable(&source, 1, &cancellation)
                .expect("the scanner must stop before traversing the excess fence body")
                .expect("the second opener must exceed the limit");

        assert_eq!(exceeded.observed_document_diagrams, 2);
        assert_eq!(&source[exceeded.opening_marker], "```");
        assert!(!cancellation.is_cancelled());
    }

    #[test]
    fn borrowed_document_build_rejects_before_scanning_or_promoting_the_excess_body() {
        let source = format!(
            "```mermaid\nflowchart TD\nA-->B\n```\n```mermaid\n{}",
            "x".repeat(128 * 1024)
        );
        let descriptor = source_descriptor_for_markdown_path(Some("fixture.md"));
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(64);

        let input = CaptureText::Borrowed(&source);
        let outcome =
            DocumentSource::new_bounded_cancellable(&input, descriptor, Some(1), &cancellation)
                .expect("the borrowed builder must stop at the excess opener");
        let exceeded = match outcome {
            DocumentSourceBuildOutcome::DiagramLimitExceeded(exceeded) => exceeded,
            DocumentSourceBuildOutcome::Ready(_) => panic!("the second opener must be rejected"),
        };

        assert_eq!(exceeded.observed_document_diagrams, 2);
        assert_eq!(&source[exceeded.opening_marker], "```");
        assert!(!cancellation.is_cancelled());
    }

    #[test]
    fn markdown_diagram_materialization_observes_cancellation_after_scanning() {
        let source = (0..256)
            .map(|index| format!("```mermaid\nflowchart TD\nA{index}-->B{index}\n```\n"))
            .collect::<String>();
        let descriptor = source_descriptor_for_markdown_path(Some("fixture.md"));
        let scan_cancellation = crate::AnalysisCancellationToken::new();
        let scanned = match scan_markdown_diagrams_bounded(&source, None, &scan_cancellation)
            .expect("the fence scan must complete")
        {
            Ok(scanned) => scanned,
            Err(_) => panic!("an unlimited scan cannot exceed a diagram limit"),
        };
        assert_eq!(scanned.len(), 256);

        let source: Arc<str> = Arc::from(source);
        let materialization_cancellation = crate::AnalysisCancellationToken::new();
        materialization_cancellation.cancel_after_checkpoints(1);

        assert!(matches!(
            materialize_markdown_diagrams(
                &source,
                &descriptor,
                scanned,
                &materialization_cancellation,
            ),
            Err(crate::AnalysisCancelled)
        ));
        assert!(materialization_cancellation.is_cancelled());
    }

    #[test]
    fn document_diagram_limit_scan_observes_cancellation() {
        let source = "x".repeat(128 * 1024);
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            markdown_document_diagram_limit_exceeded_cancellable(&source, 1, &cancellation,),
            Err(crate::AnalysisCancelled)
        ));
    }

    #[test]
    fn markdown_fence_opening_marker_scan_observes_scheduled_cancellation() {
        let line = format!("{}mermaid", "`".repeat(32 * 1024));
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            markdown_fence_opening(&line, &cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn markdown_fence_opening_info_indent_scan_observes_scheduled_cancellation() {
        let line = format!("```{}mermaid", " ".repeat(32 * 1024));
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(4);

        assert!(matches!(
            markdown_fence_opening(&line, &cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn markdown_fence_closing_tail_scan_observes_scheduled_cancellation() {
        let line = format!("```{}", " ".repeat(32 * 1024));
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(4);

        assert!(matches!(
            matching_closing_fence_marker(
                &line,
                FenceDelimiter::new(FenceMarker::Backtick, 3),
                &cancellation,
            ),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
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
    fn markdown_analysis_supports_the_full_i64_runtime_instant_domain() {
        let analyzer = Analyzer::with_options(AnalysisOptions::default().with_runtime_policy(
            merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(i64::MAX),
        ));
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let text =
            "```mermaid\nflowchart TD\nA-->B\n```\n```mermaid\nsequenceDiagram\nA->>B: hi\n```\n";
        let diagnostics_only = analyze_document(text, &analyzer, source.clone());
        let result = analyze_document_generation(text, &analyzer, source)
            .into_ready()
            .expect("the maximum i64 instant remains representable");

        assert_eq!(result.diagrams().len(), 2);
        let payload = result.project(analyzer.options().diagnostic_policy());
        assert_eq!(diagnostics_only, payload);
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
    fn markdown_document_source_extracts_bare_cr_fences() {
        let source = "before\r```mermaid\rflowchart LR\rA-->B\r```\rafter";
        let document = DocumentSource::new(
            source,
            source_descriptor_for_markdown_path(Some("file:///tmp/example.md")),
        );

        assert_eq!(document.diagrams().len(), 1);
        let diagram = &document.diagrams()[0];
        assert_eq!(diagram.text.as_str(), "flowchart LR\rA-->B\r");

        let local_map = SourceMap::new(diagram.text.as_str());
        let start = local_map.source().find('A').unwrap();
        let end = start + 1;
        let diagnostic = AnalysisDiagnostic::error(
            "merman.parse.diagram_parse",
            DiagnosticCategory::Parse,
            "boom",
        )
        .with_span(local_map.span(start, end).unwrap());

        let remapped = document.remap_diagnostic_to_document(diagram, diagnostic);
        let span = remapped.span.unwrap();
        assert_eq!(&source[span.byte_start..span.byte_end], "A");
        assert_eq!(span.line, 4);
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
    fn source_descriptor_for_uri_treats_markdown_extensions_case_insensitively() {
        let markdown = source_descriptor_for_uri("file:///tmp/README.MD");
        let mdx = source_descriptor_for_uri("file:///tmp/Story.MDX?rev=1#fence");

        assert_eq!(markdown.kind, SourceKind::Markdown);
        assert_eq!(markdown.language, "markdown");
        assert_eq!(mdx.kind, SourceKind::Mdx);
        assert_eq!(mdx.language, "mdx");
    }

    #[test]
    fn markdown_document_source_accepts_commonmark_spaced_info_strings() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new(
            "before\n```` mermaid title=Main\nflowchart LR\nA-->B\n````\n~~~ Mermaid\nsequenceDiagram\nA->>B: Hi\n~~~\n:::MERMAID extra info\npie title Work\n:::\n",
            source,
        );

        assert_eq!(document.diagrams().len(), 3);
        assert!(document.diagrams()[0].text.contains("flowchart LR"));
        assert_eq!(
            document.diagrams()[1].fence_delimiter.unwrap().marker(),
            FenceMarker::Tilde
        );
        assert_eq!(
            document.diagrams()[2].fence_delimiter.unwrap().marker(),
            FenceMarker::Colon
        );
        assert!(document.diagrams()[2].text.contains("pie title Work"));
    }

    #[test]
    fn markdown_fence_parser_records_exact_marker_spans() {
        let text = concat!(
            "  ````mermaid\n",
            "flowchart LR\n",
            "   ``````\n",
            ":::MERMAID\n",
            "pie title Work\n",
            ":::::\n",
            "~~~mermaid\n",
            "sequenceDiagram\n",
        );
        let document = DocumentSource::new(
            text,
            source_descriptor_for_markdown_path(Some("file:///tmp/example.md")),
        );

        assert_eq!(document.diagrams().len(), 3);
        let first = document.diagrams()[0]
            .fence_delimiter_spans
            .as_ref()
            .expect("backtick marker spans");
        assert_eq!(&text[first.opening.clone()], "````");
        assert_eq!(
            &text[first.closing.clone().expect("backtick closing")],
            "``````"
        );
        let second = document.diagrams()[1]
            .fence_delimiter_spans
            .as_ref()
            .expect("colon marker spans");
        assert_eq!(&text[second.opening.clone()], ":::");
        assert_eq!(
            &text[second.closing.clone().expect("colon closing")],
            ":::::"
        );
        let third = document.diagrams()[2]
            .fence_delimiter_spans
            .as_ref()
            .expect("unclosed marker spans");
        assert_eq!(&text[third.opening.clone()], "~~~");
        assert_eq!(third.closing, None);
    }

    #[test]
    fn markdown_document_source_rejects_mermaid_prefix_without_language_boundary() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new("```mermaidx\nflowchart LR\n```\n", source);

        assert!(document.diagrams().is_empty());
    }

    #[test]
    fn markdown_document_source_ignores_unicode_info_strings_without_panicking() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new("```💡💡\nflowchart LR\n```\n", source);

        assert!(document.diagrams().is_empty());
    }

    #[test]
    fn markdown_document_source_ignores_mermaid_examples_inside_other_fences() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new(
            "````text\n```mermaid\nflowchart LR\nA-->B\n```\n````\n```mermaid\nflowchart TD\nC-->D\n```\n",
            source,
        );

        assert_eq!(document.diagrams().len(), 1);
        assert!(document.diagrams()[0].text.contains("flowchart TD"));
        assert!(!document.diagrams()[0].text.contains("flowchart LR"));
    }

    #[test]
    fn markdown_document_source_rejects_indented_mermaid_fences() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new(
            "    ```mermaid\n    flowchart LR\n    ```\n   ```mermaid\nflowchart TD\n```\n",
            source,
        );

        assert_eq!(document.diagrams().len(), 1);
        assert!(document.diagrams()[0].text.contains("flowchart TD"));
    }

    #[test]
    fn markdown_document_source_does_not_treat_tab_indent_as_fence_syntax() {
        let source = source_descriptor_for_markdown_path(Some("file:///tmp/example.md"));
        let document = DocumentSource::new("\t```mermaid\nflowchart BT\n```\n", source);

        assert!(document.diagrams().is_empty());
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
        let spans = diagram
            .fence_delimiter_spans
            .as_ref()
            .expect("unclosed opening marker");
        assert_eq!(&document.text()[spans.opening.clone()], "```");
        assert_eq!(spans.closing, None);
    }

    #[test]
    fn remaps_diagnostics_back_into_host_document_coordinates() {
        let source = "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter";
        let document = DocumentSource::new(
            source,
            source_descriptor_for_markdown_path(Some("example.md")),
        );
        let diagram = &document.diagrams()[0];
        let local_map = SourceMap::new(diagram.text.as_str());
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
        let local_map = SourceMap::new(diagram.text.as_str());
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
        let local_map = SourceMap::new(diagram.text.as_str());
        let start = local_map.source().find("initialize").unwrap();
        let end = start + "initialize".len();
        let local_span = local_map.span(start, end).unwrap();
        let diagnostic = AnalysisDiagnostic::error(
            crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
            DiagnosticCategory::Config,
            "prefer init",
        )
        .with_span(local_span)
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

    #[test]
    fn diagnostic_reprojection_preserves_markdown_spans_and_fixes() {
        let text = "before\n```mermaid\n%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n```\nafter";
        let source = source_descriptor_for_markdown_path(Some("example.md"));
        let result = analyze_document_generation(text, &Analyzer::new(), source.clone())
            .into_ready()
            .expect("source is within the analysis limit");
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                crate::AnalysisRuleConfig::default()
                    .with_profile(crate::AnalysisRuleProfile::Recommended)
                    .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                    .unwrap(),
            ),
        );

        let reprojected = result.project(analyzer.options().diagnostic_policy());
        let freshly_parsed = analyze_document(text, &analyzer, source);

        assert_eq!(reprojected, freshly_parsed);
        let diagnostic = reprojected
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
            .expect("reprojected init directive diagnostic");
        let span = diagnostic.span.as_ref().expect("diagnostic span");
        assert_eq!(&text[span.byte_start..span.byte_end], "initialize");
        let edit = &diagnostic.fixes[0].edits[0];
        assert_eq!(
            &text[edit.span.byte_start..edit.span.byte_end],
            "initialize"
        );
        assert_eq!(edit.replacement, "init");
    }

    #[test]
    fn markdown_remapping_preserves_shared_document_migration_edits() {
        let text = concat!(
            "before\n```mermaid\n",
            "%%{ init: {\"theme\":\"dark\"} }%%\n",
            "%%{ init: {\"flowchart\":{\"curve\":\"linear\"}} }%%\n",
            "flowchart TD\nA-->B\n```\nafter",
        );
        let source = source_descriptor_for_markdown_path(Some("example.md"));
        let generation = analyze_document_generation(text, &Analyzer::new(), source)
            .into_ready()
            .expect("source is within the analysis limit");
        let policy = crate::AnalysisDiagnosticPolicy {
            rule_config: crate::AnalysisRuleConfig::default()
                .with_profile(crate::AnalysisRuleProfile::Recommended),
        };

        let payload = generation.project(&policy);
        let migrations = payload
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.id == crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
            .collect::<Vec<_>>();

        assert_eq!(migrations.len(), 2);
        assert!(std::sync::Arc::ptr_eq(
            &migrations[0].fixes[0].edits,
            &migrations[1].fixes[0].edits,
        ));
        assert!(migrations[0].fixes[0].edits.iter().all(|edit| {
            edit.span.byte_end <= text.len()
                && text.is_char_boundary(edit.span.byte_start)
                && text.is_char_boundary(edit.span.byte_end)
        }));
    }

    #[test]
    fn rejects_an_entire_fix_when_any_edit_cannot_be_remapped() {
        let source = "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter";
        let document = DocumentSource::new(
            source,
            source_descriptor_for_markdown_path(Some("example.md")),
        );
        let diagram = &document.diagrams()[0];
        let local_map = SourceMap::new(diagram.text.as_str());
        let valid_span = local_map.span(0, "flowchart".len()).unwrap();
        let mut invalid_span = valid_span;
        invalid_span.byte_start = usize::MAX;
        invalid_span.byte_end = usize::MAX;
        let diagnostic = AnalysisDiagnostic::error(
            crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
            DiagnosticCategory::Config,
            "atomic fix",
        )
        .with_fix(DiagnosticFix::new(
            "Apply both edits",
            vec![
                DiagnosticFixEdit::new(valid_span, "graph"),
                DiagnosticFixEdit::new(invalid_span, "invalid"),
            ],
        ));

        let remapped = document.remap_diagnostic_to_document(diagram, diagnostic);

        assert!(
            remapped.fixes.is_empty(),
            "a multi-edit fix must not survive partial coordinate mapping"
        );
    }
}
