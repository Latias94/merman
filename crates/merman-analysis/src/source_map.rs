use crate::payload::{DiagnosticSpan, Utf16Position};

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
    source: String,
    line_starts: Vec<usize>,
}

impl SourceMap {
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        let line_starts = line_starts(&source);
        Self {
            source,
            line_starts,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_len(&self) -> usize {
        self.source.len()
    }

    pub fn line_starts(&self) -> &[usize] {
        &self.line_starts
    }

    pub fn line_col(&self, offset: usize) -> Result<LineCol, SourceMapError> {
        self.validate_offset(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let line_start = self.line_starts[line_index];
        let line_prefix = &self.source[line_start..offset];
        Ok(LineCol::new(
            line_index + 1,
            line_prefix.chars().count() + 1,
        ))
    }

    pub fn utf16_position(&self, offset: usize) -> Result<Utf16Position, SourceMapError> {
        self.validate_offset(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let line_start = self.line_starts[line_index];
        let line_prefix = &self.source[line_start..offset];
        Ok(Utf16Position {
            line: line_index,
            character: line_prefix.encode_utf16().count(),
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
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, byte) in source.bytes().enumerate() {
        if byte == b'\n' && idx + 1 < source.len() {
            starts.push(idx + 1);
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_ascii_offsets_to_one_based_cli_positions() {
        let map = SourceMap::new("flowchart TD\nA-->B\n");

        assert_eq!(map.line_col(0).unwrap(), LineCol::new(1, 1));
        assert_eq!(map.line_col(13).unwrap(), LineCol::new(2, 1));
        assert_eq!(map.line_col(map.source_len()).unwrap(), LineCol::new(2, 7));
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
}
