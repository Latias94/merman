use crate::payload::{DiagnosticSpan, Utf16Position};
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    pub column: usize,
}

impl LineCol {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

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
    line_metrics: Arc<[OnceLock<LineMetric>]>,
}

impl SourceMap {
    pub fn new(source: impl Into<Arc<str>>) -> Self {
        let source = source.into();
        let line_starts = line_starts(source.as_ref());
        let line_metrics = (0..line_starts.len())
            .map(|_| OnceLock::new())
            .collect::<Vec<_>>();
        Self {
            source,
            line_starts: Arc::from(line_starts.into_boxed_slice()),
            line_metrics: Arc::from(line_metrics.into_boxed_slice()),
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
            start,
            end,
            start_lc.line,
            start_lc.column,
            end_lc.line,
            end_lc.column,
            lsp_start,
            lsp_end,
        ))
    }

    pub fn whole_source_span(&self) -> Result<DiagnosticSpan, SourceMapError> {
        self.span(0, self.source.len())
    }

    pub fn line_bounds(&self, line_index: usize) -> Option<(usize, usize)> {
        let line = self.line_metric(line_index)?;
        Some((line.start, line.content_end))
    }

    pub fn byte_offset_for_utf16_position(&self, position: Utf16Position) -> Option<usize> {
        let line = self.line_metric(position.line)?;
        match line.utf16_columns.binary_search(&position.character) {
            Ok(boundary_index) => Some(line.start + line.byte_boundaries[boundary_index]),
            Err(boundary_index) if boundary_index >= line.utf16_columns.len() => {
                Some(line.content_end)
            }
            Err(_) => None,
        }
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
        let boundary_index = line
            .byte_boundaries
            .binary_search(&relative)
            .expect("validated source offset should map to a cached line boundary");

        Ok(OffsetMetrics {
            line_index,
            char_column: boundary_index,
            utf16_column: line.utf16_columns[boundary_index],
        })
    }

    fn line_metric(&self, line_index: usize) -> Option<&LineMetric> {
        let slot = self.line_metrics.get(line_index)?;
        Some(slot.get_or_init(|| {
            let start = self.line_starts[line_index];
            let next_start = self
                .line_starts
                .get(line_index + 1)
                .copied()
                .unwrap_or(self.source.len());
            line_metric(self.source.as_ref(), start, next_start)
        }))
    }

    #[cfg(test)]
    fn cached_line_metric_count(&self) -> usize {
        self.line_metrics
            .iter()
            .filter(|metric| metric.get().is_some())
            .count()
    }

    #[cfg(test)]
    fn cached_line_boundary_count(&self, line_index: usize) -> Option<usize> {
        self.line_metrics
            .get(line_index)
            .and_then(OnceLock::get)
            .map(|line| line.byte_boundaries.len())
    }
}

#[derive(Debug, Clone)]
struct LineMetric {
    start: usize,
    content_end: usize,
    byte_boundaries: Vec<usize>,
    utf16_columns: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OffsetMetrics {
    line_index: usize,
    char_column: usize,
    utf16_column: usize,
}

pub(crate) fn whole_text_span_without_source_copy(text: &str) -> DiagnosticSpan {
    let mut end_line = 1usize;
    let mut end_column = 1usize;
    let mut end_lsp_line = 0usize;
    let mut end_lsp_character = 0usize;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
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
    }

    DiagnosticSpan::new(
        0,
        text.len(),
        1,
        1,
        end_line,
        end_column,
        Utf16Position {
            line: 0,
            character: 0,
        },
        Utf16Position {
            line: end_lsp_line,
            character: end_lsp_character,
        },
    )
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    let bytes = source.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
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
    starts
}

fn line_metric(source: &str, start: usize, next_start: usize) -> LineMetric {
    let content_end = line_content_end(source.as_bytes(), start, next_start);
    let line = &source[start..content_end];
    let mut byte_boundaries = Vec::with_capacity(line.chars().count() + 1);
    let mut utf16_columns = Vec::with_capacity(byte_boundaries.capacity());
    let mut utf16 = 0usize;

    byte_boundaries.push(0);
    utf16_columns.push(0);

    for (relative, ch) in line.char_indices() {
        utf16 += ch.len_utf16();
        byte_boundaries.push(relative + ch.len_utf8());
        utf16_columns.push(utf16);
    }

    LineMetric {
        start,
        content_end,
        byte_boundaries,
        utf16_columns,
    }
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
}
