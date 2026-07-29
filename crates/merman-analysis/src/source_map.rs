use crate::payload::{DiagnosticSpan, LspRange, SourcePosition, Utf16Position};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub type LineCol = SourcePosition;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceMapError {
    #[error("byte offset {offset} is outside source length {source_len}")]
    OffsetOutOfBounds { offset: usize, source_len: usize },
    #[error("byte offset {offset} is not a UTF-8 character boundary")]
    OffsetNotCharBoundary { offset: usize },
    #[error("range start {start} is after range end {end}")]
    ReversedRange { start: usize, end: usize },
}

#[derive(Debug, Clone)]
pub struct SourceMap {
    source: Arc<str>,
    line_starts: Arc<[usize]>,
    line_metrics: Arc<RwLock<HashMap<usize, Arc<LineMetric>>>>,
}

impl SourceMap {
    pub fn new(source: impl Into<Arc<str>>) -> Self {
        let source = source.into();
        let line_starts = line_starts(source.as_ref());
        Self::from_source_and_line_starts(source, line_starts)
    }

    pub(crate) fn new_cancellable(
        source: impl Into<Arc<str>>,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Self, crate::AnalysisCancelled> {
        let source = source.into();
        let line_starts = line_starts_cancellable(source.as_ref(), cancellation)?;
        cancellation.checkpoint()?;
        Ok(Self::from_source_and_line_starts(source, line_starts))
    }

    fn from_source_and_line_starts(source: Arc<str>, line_starts: Vec<usize>) -> Self {
        Self {
            source,
            line_starts: Arc::from(line_starts.into_boxed_slice()),
            line_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn source(&self) -> &str {
        self.source.as_ref()
    }

    pub fn source_arc(&self) -> Arc<str> {
        Arc::clone(&self.source)
    }

    pub fn source_len(&self) -> usize {
        self.source.len()
    }

    pub fn line_starts(&self) -> &[usize] {
        &self.line_starts
    }

    pub fn line_col(&self, offset: usize) -> Result<LineCol, SourceMapError> {
        let metrics = self.offset_metrics(offset)?;
        Ok(LineCol::new(
            metrics.line_index + 1,
            metrics.char_column + 1,
        ))
    }

    pub fn utf16_position(&self, offset: usize) -> Result<Utf16Position, SourceMapError> {
        let metrics = self.offset_metrics(offset)?;
        Ok(Utf16Position {
            line: metrics.line_index,
            character: metrics.utf16_column,
        })
    }

    pub fn span(&self, start: usize, end: usize) -> Result<DiagnosticSpan, SourceMapError> {
        if start > end {
            return Err(SourceMapError::ReversedRange { start, end });
        }

        let start_lc = self.line_col(start)?;
        let end_lc = self.line_col(end)?;
        let lsp_start = self.utf16_position(start)?;
        let lsp_end = self.utf16_position(end)?;

        Ok(DiagnosticSpan::new(
            start..end,
            start_lc,
            end_lc,
            LspRange::new(lsp_start, lsp_end),
        ))
    }

    pub(crate) fn span_cancellable(
        &self,
        start: usize,
        end: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<DiagnosticSpan, SourceMapError>, crate::AnalysisCancelled> {
        if start > end {
            return Ok(Err(SourceMapError::ReversedRange { start, end }));
        }

        let start_lc = match self.line_col_cancellable(start, cancellation)? {
            Ok(position) => position,
            Err(error) => return Ok(Err(error)),
        };
        let end_lc = match self.line_col_cancellable(end, cancellation)? {
            Ok(position) => position,
            Err(error) => return Ok(Err(error)),
        };
        let lsp_start = match self.utf16_position_cancellable(start, cancellation)? {
            Ok(position) => position,
            Err(error) => return Ok(Err(error)),
        };
        let lsp_end = match self.utf16_position_cancellable(end, cancellation)? {
            Ok(position) => position,
            Err(error) => return Ok(Err(error)),
        };

        Ok(Ok(DiagnosticSpan::new(
            start..end,
            start_lc,
            end_lc,
            LspRange::new(lsp_start, lsp_end),
        )))
    }

    pub fn whole_source_span(&self) -> Result<DiagnosticSpan, SourceMapError> {
        self.span(0, self.source.len())
    }

    pub(crate) fn whole_source_span_cancellable(
        &self,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<DiagnosticSpan, SourceMapError>, crate::AnalysisCancelled> {
        self.span_cancellable(0, self.source.len(), cancellation)
    }

    pub fn line_bounds(&self, line_index: usize) -> Option<(usize, usize)> {
        let line = self.line_metric(line_index)?;
        Some((line.start, line.content_end))
    }

    pub fn byte_offset_for_utf16_position(&self, position: Utf16Position) -> Option<usize> {
        let line = self.line_metric(position.line)?;
        line.byte_offset_for_utf16_column(position.character)
    }

    fn validate_offset(&self, offset: usize) -> Result<(), SourceMapError> {
        if offset > self.source.len() {
            return Err(SourceMapError::OffsetOutOfBounds {
                offset,
                source_len: self.source.len(),
            });
        }
        if !self.source.is_char_boundary(offset) {
            return Err(SourceMapError::OffsetNotCharBoundary { offset });
        }
        Ok(())
    }

    fn line_index_for_offset(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(0) => 0,
            Err(index) => index - 1,
        }
    }

    fn offset_metrics(&self, offset: usize) -> Result<OffsetMetrics, SourceMapError> {
        self.validate_offset(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let line = self
            .line_metric(line_index)
            .expect("validated source offset should map to a cached line");
        let clamped = offset.clamp(line.start, line.content_end);
        let relative = clamped - line.start;
        let (char_column, utf16_column) = line
            .columns_for_relative_offset(relative)
            .expect("validated source offset should map to a cached line boundary");

        Ok(OffsetMetrics {
            line_index,
            char_column,
            utf16_column,
        })
    }

    fn line_col_cancellable(
        &self,
        offset: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<LineCol, SourceMapError>, crate::AnalysisCancelled> {
        Ok(self
            .offset_metrics_cancellable(offset, cancellation)?
            .map(|metrics| LineCol::new(metrics.line_index + 1, metrics.char_column + 1)))
    }

    fn utf16_position_cancellable(
        &self,
        offset: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<Utf16Position, SourceMapError>, crate::AnalysisCancelled> {
        Ok(self
            .offset_metrics_cancellable(offset, cancellation)?
            .map(|metrics| Utf16Position {
                line: metrics.line_index,
                character: metrics.utf16_column,
            }))
    }

    fn offset_metrics_cancellable(
        &self,
        offset: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<OffsetMetrics, SourceMapError>, crate::AnalysisCancelled> {
        if let Err(error) = self.validate_offset(offset) {
            return Ok(Err(error));
        }
        let line_index = self.line_index_for_offset(offset);
        let line = self
            .line_metric_cancellable(line_index, cancellation)?
            .expect("validated source offset should map to a cached line");
        let clamped = offset.clamp(line.start, line.content_end);
        let relative = clamped - line.start;
        let (char_column, utf16_column) = line
            .columns_for_relative_offset(relative)
            .expect("validated source offset should map to a cached line boundary");

        Ok(Ok(OffsetMetrics {
            line_index,
            char_column,
            utf16_column,
        }))
    }

    fn line_metric(&self, line_index: usize) -> Option<Arc<LineMetric>> {
        self.line_metric_with_checkpoint(line_index, || Ok::<_, std::convert::Infallible>(()))
            .expect("infallible line-metric scan")
    }

    fn line_metric_cancellable(
        &self,
        line_index: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Option<Arc<LineMetric>>, crate::AnalysisCancelled> {
        self.line_metric_with_checkpoint(line_index, || cancellation.checkpoint())
    }

    fn line_metric_with_checkpoint<E>(
        &self,
        line_index: usize,
        mut checkpoint: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<Arc<LineMetric>>, E> {
        checkpoint()?;
        let Some(&start) = self.line_starts.get(line_index) else {
            return Ok(None);
        };
        if let Some(metric) = self
            .line_metrics
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&line_index)
            .cloned()
        {
            return Ok(Some(metric));
        }

        let next_start = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(self.source.len());
        let computed = Arc::new(line_metric_with_checkpoint(
            self.source.as_ref(),
            start,
            next_start,
            &mut checkpoint,
        )?);
        let mut metrics = self
            .line_metrics
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(Some(metrics.entry(line_index).or_insert(computed).clone()))
    }

    #[cfg(test)]
    fn cached_line_metric_count(&self) -> usize {
        self.line_metrics
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    #[cfg(test)]
    fn cached_line_boundary_count(&self, line_index: usize) -> Option<usize> {
        self.line_metrics
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&line_index)
            .map(|line| line.stored_boundary_count())
    }
}

#[derive(Debug, Clone)]
struct LineMetric {
    start: usize,
    content_end: usize,
    columns: LineColumns,
}

#[derive(Debug, Clone)]
enum LineColumns {
    Ascii,
    Unicode {
        byte_boundaries: Vec<usize>,
        utf16_columns: Vec<usize>,
    },
}

impl LineMetric {
    fn byte_offset_for_utf16_column(&self, column: usize) -> Option<usize> {
        match &self.columns {
            LineColumns::Ascii => Some(self.start.saturating_add(column).min(self.content_end)),
            LineColumns::Unicode {
                byte_boundaries,
                utf16_columns,
            } => match utf16_columns.binary_search(&column) {
                Ok(boundary_index) => Some(self.start + byte_boundaries[boundary_index]),
                Err(boundary_index) if boundary_index >= utf16_columns.len() => {
                    Some(self.content_end)
                }
                Err(_) => None,
            },
        }
    }

    fn columns_for_relative_offset(&self, relative: usize) -> Option<(usize, usize)> {
        match &self.columns {
            LineColumns::Ascii => Some((relative, relative)),
            LineColumns::Unicode {
                byte_boundaries,
                utf16_columns,
            } => {
                let boundary_index = byte_boundaries.binary_search(&relative).ok()?;
                Some((boundary_index, utf16_columns[boundary_index]))
            }
        }
    }

    #[cfg(test)]
    fn stored_boundary_count(&self) -> usize {
        match &self.columns {
            LineColumns::Ascii => 0,
            LineColumns::Unicode {
                byte_boundaries, ..
            } => byte_boundaries.len(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OffsetMetrics {
    line_index: usize,
    char_column: usize,
    utf16_column: usize,
}

pub(crate) fn whole_text_span_without_source_copy(text: &str) -> DiagnosticSpan {
    whole_text_span_without_source_copy_with_checkpoint(text, || {
        Ok::<_, std::convert::Infallible>(())
    })
    .expect("infallible whole-source span scan")
}

pub(crate) fn whole_text_span_without_source_copy_cancellable(
    text: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<DiagnosticSpan, crate::AnalysisCancelled> {
    whole_text_span_without_source_copy_with_checkpoint(text, || cancellation.checkpoint())
}

fn whole_text_span_without_source_copy_with_checkpoint<E>(
    text: &str,
    mut checkpoint: impl FnMut() -> Result<(), E>,
) -> Result<DiagnosticSpan, E> {
    let mut end_line = 1usize;
    let mut end_column = 1usize;
    let mut end_lsp_line = 0usize;
    let mut end_lsp_character = 0usize;
    let mut bytes_since_checkpoint = 0usize;
    let mut chars = text.chars().peekable();

    checkpoint()?;
    while let Some(ch) = chars.next() {
        bytes_since_checkpoint += ch.len_utf8();
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    bytes_since_checkpoint += 1;
                }
                end_line += 1;
                end_column = 1;
                end_lsp_line += 1;
                end_lsp_character = 0;
            }
            '\n' => {
                end_line += 1;
                end_column = 1;
                end_lsp_line += 1;
                end_lsp_character = 0;
            }
            _ => {
                end_column += 1;
                end_lsp_character += ch.len_utf16();
            }
        }
        if bytes_since_checkpoint >= 4096 {
            checkpoint()?;
            bytes_since_checkpoint = 0;
        }
    }
    checkpoint()?;

    Ok(DiagnosticSpan::new(
        0..text.len(),
        SourcePosition::new(1, 1),
        SourcePosition::new(end_line, end_column),
        LspRange::new(
            Utf16Position {
                line: 0,
                character: 0,
            },
            Utf16Position {
                line: end_lsp_line,
                character: end_lsp_character,
            },
        ),
    ))
}

fn line_starts(source: &str) -> Vec<usize> {
    line_starts_with_checkpoint(source, |_| Ok::<_, std::convert::Infallible>(()))
        .expect("infallible line-start scan")
}

fn line_starts_cancellable(
    source: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<usize>, crate::AnalysisCancelled> {
    line_starts_with_checkpoint(source, |_| {
        cancellation.checkpoint()?;
        Ok(())
    })
}

fn line_starts_with_checkpoint<E>(
    source: &str,
    mut checkpoint: impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<usize>, E> {
    let mut starts = vec![0];
    let bytes = source.as_bytes();
    let mut idx = 0usize;
    let mut next_checkpoint = 0usize;
    while idx < bytes.len() {
        if idx >= next_checkpoint {
            checkpoint(idx)?;
            next_checkpoint = idx.saturating_add(4096);
        }
        match bytes[idx] {
            b'\r' => {
                idx += 1;
                if bytes.get(idx) == Some(&b'\n') {
                    idx += 1;
                }
                starts.push(idx);
            }
            b'\n' => {
                idx += 1;
                starts.push(idx);
            }
            _ => {
                idx += 1;
            }
        }
    }
    checkpoint(idx)?;
    Ok(starts)
}

fn line_metric_with_checkpoint<E>(
    source: &str,
    start: usize,
    next_start: usize,
    mut checkpoint: impl FnMut() -> Result<(), E>,
) -> Result<LineMetric, E> {
    let content_end = line_content_end(source.as_bytes(), start, next_start);
    let line = &source[start..content_end];
    let mut ascii = true;
    for chunk in line.as_bytes().chunks(4096) {
        checkpoint()?;
        if !chunk.is_ascii() {
            ascii = false;
            break;
        }
    }
    checkpoint()?;
    if ascii {
        return Ok(LineMetric {
            start,
            content_end,
            columns: LineColumns::Ascii,
        });
    }

    let mut byte_boundaries = Vec::new();
    let mut utf16_columns = Vec::new();
    let mut utf16 = 0usize;
    let mut next_checkpoint = 0usize;

    byte_boundaries.push(0);
    utf16_columns.push(0);

    for (relative, ch) in line.char_indices() {
        if relative >= next_checkpoint {
            checkpoint()?;
            next_checkpoint = relative.saturating_add(4096);
        }
        utf16 += ch.len_utf16();
        byte_boundaries.push(relative + ch.len_utf8());
        utf16_columns.push(utf16);
    }
    checkpoint()?;

    Ok(LineMetric {
        start,
        content_end,
        columns: LineColumns::Unicode {
            byte_boundaries,
            utf16_columns,
        },
    })
}

fn line_content_end(bytes: &[u8], start: usize, next_start: usize) -> usize {
    let mut end = next_start;
    if end > start && bytes.get(end - 1) == Some(&b'\n') {
        end -= 1;
    }
    if end > start && bytes.get(end - 1) == Some(&b'\r') {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_ascii_offsets_to_one_based_cli_positions() {
        let map = SourceMap::new("flowchart TD\nA-->B\n");

        assert_eq!(map.line_col(0).unwrap(), LineCol::new(1, 1));
        assert_eq!(map.line_col(13).unwrap(), LineCol::new(2, 1));
        assert_eq!(map.line_col(map.source_len()).unwrap(), LineCol::new(3, 1));
    }

    #[test]
    fn maps_utf8_offsets_to_lsp_utf16_positions() {
        let map = SourceMap::new("flowchart TD\nA[🤓]-->B\n");
        let emoji_start = map.source().find('🤓').unwrap();
        let emoji_end = emoji_start + "🤓".len();
        let after_bracket = emoji_end + 1;

        assert_eq!(
            map.utf16_position(emoji_start).unwrap(),
            Utf16Position {
                line: 1,
                character: 2
            }
        );
        assert_eq!(
            map.utf16_position(after_bracket).unwrap(),
            Utf16Position {
                line: 1,
                character: 5
            }
        );
    }

    #[test]
    fn crlf_line_bounds_and_positions_ignore_carriage_return() {
        let source = "flowchart TD\r\nA[🤓]-->B\r\n";
        let map = SourceMap::new(source);
        let first_cr = source.find('\r').unwrap();
        let first_lf = source.find('\n').unwrap();

        assert_eq!(map.line_bounds(0), Some((0, first_cr)));
        assert_eq!(
            map.utf16_position(first_cr).unwrap(),
            Utf16Position {
                line: 0,
                character: "flowchart TD".len(),
            }
        );
        assert_eq!(
            map.utf16_position(first_lf).unwrap(),
            Utf16Position {
                line: 0,
                character: "flowchart TD".len(),
            }
        );
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 0,
                character: "flowchart TD".len(),
            }),
            Some(first_cr)
        );
    }

    #[test]
    fn bare_cr_line_bounds_and_positions_treat_carriage_return_as_line_ending() {
        let source = "flowchart TD\rA-->B\rC-->D";
        let map = SourceMap::new(source);
        let first_cr = source.find('\r').unwrap();
        let second_line_start = first_cr + 1;
        let second_cr = source[second_line_start..].find('\r').unwrap() + second_line_start;

        assert_eq!(map.line_bounds(0), Some((0, first_cr)));
        assert_eq!(map.line_bounds(1), Some((second_line_start, second_cr)));
        assert_eq!(
            map.utf16_position(second_line_start).unwrap(),
            Utf16Position {
                line: 1,
                character: 0,
            }
        );
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 2,
                character: 0,
            }),
            Some(second_cr + 1)
        );
    }

    #[test]
    fn utf16_position_past_line_end_clamps_to_content_end() {
        let source = "flowchart TD\nA[🤓]-->B\n";
        let map = SourceMap::new(source);
        let second_line_start = source.find("A[").unwrap();
        let second_line_end = source[second_line_start..].find('\n').unwrap() + second_line_start;

        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 1,
                character: 10_000,
            }),
            Some(second_line_end)
        );
    }

    #[test]
    fn dense_span_conversion_uses_cached_line_metrics() {
        let nodes = (0..512)
            .map(|index| format!("N{index}[🤓]"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!("flowchart TD {nodes}");
        let map = SourceMap::new(source.clone());

        assert_eq!(map.cached_line_metric_count(), 0);
        assert_eq!(map.cached_line_boundary_count(0), None);

        for offset in source.match_indices('N').map(|(offset, _)| offset) {
            let end = source[offset..].find('[').map(|len| offset + len).unwrap();
            let span = map.span(offset, end).unwrap();
            assert_eq!(span.lsp_range.start.line, 0);
            assert!(span.lsp_range.end.character > span.lsp_range.start.character);
        }
        assert_eq!(map.cached_line_metric_count(), 1);
        assert_eq!(
            map.cached_line_boundary_count(0),
            Some(source.chars().count() + 1)
        );
    }

    #[test]
    fn dense_line_sources_cache_only_queried_line_metrics() {
        let source = "\n".repeat(100_000);
        let map = SourceMap::new(source);

        assert_eq!(map.line_starts().len(), 100_001);
        assert_eq!(map.cached_line_metric_count(), 0);
        assert_eq!(map.line_bounds(99_999), Some((99_999, 99_999)));
        assert_eq!(map.cached_line_metric_count(), 1);
    }

    #[test]
    fn long_ascii_lines_use_constant_size_column_metrics() {
        let source = "a".repeat(64 * 1024);
        let map = SourceMap::new(source.clone());

        assert_eq!(
            map.utf16_position(source.len()).unwrap(),
            Utf16Position {
                line: 0,
                character: source.len(),
            }
        );
        assert_eq!(map.cached_line_boundary_count(0), Some(0));
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 0,
                character: source.len() + 1,
            }),
            Some(source.len())
        );

        let non_initial_line = SourceMap::new("x\nabc");
        assert_eq!(
            non_initial_line.byte_offset_for_utf16_position(Utf16Position {
                line: 1,
                character: usize::MAX,
            }),
            Some(5)
        );
    }

    #[test]
    fn dense_line_scan_observes_scheduled_cancellation() {
        let source = "\n".repeat(32 * 1024);
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            SourceMap::new_cancellable(source, &cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn long_single_line_metric_observes_scheduled_cancellation() {
        let source = "a".repeat(32 * 1024);
        let map = SourceMap::new(source);
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            map.whole_source_span_cancellable(&cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
        assert_eq!(map.cached_line_metric_count(), 0);
    }

    #[test]
    fn rejects_non_char_boundary_offsets() {
        let map = SourceMap::new("flowchart TD\nA[🤓]\n");
        let inside_emoji = map.source().find('🤓').unwrap() + 1;

        assert_eq!(
            map.line_col(inside_emoji).unwrap_err(),
            SourceMapError::OffsetNotCharBoundary {
                offset: inside_emoji
            }
        );
    }

    #[test]
    fn builds_diagnostic_span_with_cli_and_lsp_positions() {
        let map = SourceMap::new("flowchart TD\nA[🤓]-->B\n");
        let start = map.source().find('A').unwrap();
        let end = map.source().find("-->").unwrap();
        let span = map.span(start, end).unwrap();

        assert_eq!(span.byte_start, start);
        assert_eq!(span.byte_end, end);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, 2);
        assert_eq!(span.end_column, 5);
        assert_eq!(span.lsp_range.start.line, 1);
        assert_eq!(span.lsp_range.start.character, 0);
        assert_eq!(span.lsp_range.end.character, 5);
    }

    #[test]
    fn whole_text_span_without_source_copy_matches_source_map_span() {
        for source in [
            "flowchart TD\nA[🤓]-->B\n",
            "flowchart TD\r\nA[🤓]-->B",
            "flowchart TD\r\nA[🤓]-->B\r",
            "flowchart TD\r\r\nA[🤓]-->B",
        ] {
            assert_eq!(
                whole_text_span_without_source_copy(source),
                SourceMap::new(source).whole_source_span().unwrap()
            );
        }
    }

    #[test]
    fn whole_text_span_without_source_copy_observes_scheduled_cancellation() {
        let source = "a".repeat(32 * 1024);
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            whole_text_span_without_source_copy_cancellable(&source, &cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }
}
