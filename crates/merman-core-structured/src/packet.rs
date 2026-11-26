//! Packet headers, field scanning and recovery, and bounded expansion into rows.

use merman_model::families::packet::{
    PacketBitsPerRowError, PacketBlock, PacketDiagramModel, validate_packet_bits_per_row,
};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

const MAX_PACKET_ROWS: usize = 10_000;

/// One Packet field before DB expansion, with spans relative to the lexer input.
#[derive(Debug)]
pub struct PacketField {
    start: Option<f64>,
    end: Option<f64>,
    bits: Option<f64>,
    label: String,
    numeric_span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    label_span: SourceSpan,
}

#[cfg(feature = "editor-facts")]
impl PacketField {
    pub fn numeric_span(&self) -> SourceSpan {
        self.numeric_span
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn label_span(&self) -> SourceSpan {
        self.label_span
    }
}

/// A header or statement rejected by the Packet lexer.
#[derive(Debug)]
pub struct PacketSyntaxDiagnostic {
    pub message: String,
    /// The visible part of the rejected physical line in the supplied source, if available.
    pub recovery_span: Option<SourceSpan>,
}

/// Invalid row configuration or a semantic failure located at a field's numeric token.
#[derive(Debug)]
pub enum PacketExpansionDiagnostic {
    InvalidRowWidth(PacketBitsPerRowError),
    Field {
        span: SourceSpan,
        error: PacketFieldError,
    },
}

/// Typed arguments for Packet's JavaScript Number diagnostics.
#[derive(Debug)]
pub enum PacketFieldError {
    ReversedRange { start: f64, end: f64 },
    NotContiguous { start: f64, end: f64, expected: f64 },
    ZeroBitField { start: f64 },
    InvertedFragment { start: f64, end: f64 },
    RowLimit { max_rows: usize },
    Nonprogress,
}

fn parse_packet_integer(token: &str) -> Option<f64> {
    if token.len() > 1 && token.starts_with('0') {
        return None;
    }
    token.parse().ok()
}

const PACKET_SCAN_CHECKPOINT_BYTES: usize = 1024;

/// Packet source cursor with physical-line recovery across multiline labels.
///
/// Start with [`Self::after_header`], then visit fields and diagnostics with [`Self::next_field`].
pub struct PacketLexer<'a> {
    source: &'a str,
    offset: usize,
    on_header_line: bool,
    control: &'a OperationControl,
    checkpoint_bytes: usize,
    recovery: PacketRecoveryLine,
    // EOF has no later closing delimiter of this kind. Reuse that failure after recovery rather
    // than scanning the same suffix again; there are only two quote kinds to retain.
    exhausted_quotes: [Option<usize>; 2],
    #[cfg(test)]
    scanned_bytes: usize,
}

struct PacketRecoveryLine {
    span: SourceSpan,
    next_offset: Option<usize>,
    comment: bool,
    quote: Option<char>,
    escaped: bool,
}

impl PacketRecoveryLine {
    fn new(start: usize) -> Self {
        Self {
            span: SourceSpan::new(start, start),
            next_offset: None,
            comment: false,
            quote: None,
            escaped: false,
        }
    }
}

impl<'a> PacketLexer<'a> {
    fn new(source: &'a str, offset: usize, control: &'a OperationControl) -> Self {
        Self {
            source,
            offset,
            on_header_line: true,
            control,
            checkpoint_bytes: 0,
            recovery: PacketRecoveryLine::new(offset),
            exhausted_quotes: [None; 2],
            #[cfg(test)]
            scanned_bytes: 0,
        }
    }

    /// Recognizes either Packet header after leading whitespace and comments.
    /// `Ok(None)` identifies empty or trivia-only input, separately from a header-only diagram.
    pub fn after_header(
        source: &'a str,
        control: &'a OperationControl,
    ) -> OperationControlResult<Result<Option<Self>, PacketSyntaxDiagnostic>> {
        let mut cursor = Self::new(source, 0, control);
        loop {
            control.checkpoint()?;
            while cursor.peek().is_some_and(char::is_whitespace) {
                cursor.advance()?;
            }
            if cursor.remaining().starts_with("%%") {
                cursor.skip_line()?;
                continue;
            }
            if cursor.remaining().is_empty() {
                return Ok(Ok(None));
            }
            let Some(header_len) = packet_header_token_len(cursor.remaining()) else {
                return Ok(Err(PacketSyntaxDiagnostic {
                    message: "expected packet".into(),
                    recovery_span: None,
                }));
            };
            return Ok(Ok(Some(Self::new(
                source,
                cursor.offset + header_len,
                control,
            ))));
        }
    }

    /// Skips trivia and yields the next Packet field or a recovered statement diagnostic.
    ///
    /// `common_field_bytes` receives the statement start and the first non-trivia byte offset.
    /// Return the byte length of a shared common terminal starting at the latter offset. The
    /// length must be nonzero, in bounds, and end on a UTF-8 boundary to be used.
    /// `decode_escape` applies the caller's shared Langium escape conversion, returning `None`
    /// for an escaped physical line terminator.
    /// After a diagnostic, the next call resumes at the next physical line.
    pub fn next_field(
        &mut self,
        mut common_field_bytes: impl FnMut(usize, usize) -> Option<usize>,
        decode_escape: impl Fn(char) -> Option<char>,
    ) -> OperationControlResult<Option<Result<PacketField, PacketSyntaxDiagnostic>>> {
        while self.offset < self.source.len() {
            self.control.checkpoint()?;
            self.begin_statement();
            let line_start = self.offset;
            let on_header_line = std::mem::take(&mut self.on_header_line);
            self.skip_horizontal_whitespace()?;
            if self.remaining().starts_with("%%") {
                self.skip_line()?;
                continue;
            }
            if self.consume_eol()? {
                continue;
            }
            if let Some(consumed) = common_field_bytes(line_start, self.offset)
                && consumed != 0
                && self.remaining().is_char_boundary(consumed)
            {
                self.offset += consumed;
                continue;
            }
            if let Some(field) = self.field(decode_escape)? {
                return Ok(Some(Ok(field)));
            }

            let span = self.recover_statement()?;
            let message = if on_header_line {
                "expected packet".into()
            } else {
                let text = &self.source[span.start..span.end];
                format!("unexpected packet statement: {text}")
            };
            return Ok(Some(Err(PacketSyntaxDiagnostic {
                message,
                recovery_span: Some(span),
            })));
        }
        Ok(None)
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.offset..]
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn begin_statement(&mut self) {
        self.recovery = PacketRecoveryLine::new(self.offset);
    }

    fn advance(&mut self) -> OperationControlResult<Option<char>> {
        let Some(ch) = self.peek() else {
            return Ok(None);
        };
        let start = self.offset;
        self.offset += ch.len_utf8();
        if self.recovery.next_offset.is_none() {
            if ch == '\n' {
                self.recovery.next_offset = Some(self.offset);
            } else if !self.recovery.comment {
                if self.recovery.escaped {
                    self.recovery.escaped = false;
                } else if self.recovery.quote.is_some() && ch == '\\' {
                    self.recovery.escaped = true;
                } else if Some(ch) == self.recovery.quote {
                    self.recovery.quote = None;
                } else if self.recovery.quote.is_none() {
                    if matches!(ch, '\'' | '"') {
                        self.recovery.quote = Some(ch);
                    } else if ch == '%' && self.remaining().starts_with('%') {
                        self.recovery.comment = true;
                    }
                }
                if !self.recovery.comment && !ch.is_whitespace() {
                    if self.recovery.span.start == self.recovery.span.end {
                        self.recovery.span.start = start;
                    }
                    self.recovery.span.end = self.offset;
                }
            }
        }
        #[cfg(test)]
        {
            self.scanned_bytes += ch.len_utf8();
        }
        self.checkpoint_bytes += ch.len_utf8();
        if self.checkpoint_bytes >= PACKET_SCAN_CHECKPOINT_BYTES {
            self.control.checkpoint()?;
            self.checkpoint_bytes = 0;
        }
        Ok(Some(ch))
    }

    fn skip_horizontal_whitespace(&mut self) -> OperationControlResult<()> {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.advance()?;
        }
        Ok(())
    }

    fn integer(&mut self) -> OperationControlResult<Option<(f64, SourceSpan)>> {
        let start = self.offset;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.advance()?;
        }
        Ok(parse_packet_integer(&self.source[start..self.offset])
            .map(|value| (value, SourceSpan::new(start, self.offset))))
    }

    fn field(
        &mut self,
        decode_escape: impl Fn(char) -> Option<char>,
    ) -> OperationControlResult<Option<PacketField>> {
        let relative = self.peek() == Some('+');
        if relative {
            self.advance()?;
            self.skip_horizontal_whitespace()?;
        }
        let Some((first, first_span)) = self.integer()? else {
            return Ok(None);
        };
        self.skip_horizontal_whitespace()?;
        let (start, end, bits, numeric_span) = if relative {
            (None, None, Some(first), first_span)
        } else if self.peek() == Some('-') {
            self.advance()?;
            self.skip_horizontal_whitespace()?;
            let Some((end, end_span)) = self.integer()? else {
                return Ok(None);
            };
            self.skip_horizontal_whitespace()?;
            (
                Some(first),
                Some(end),
                None,
                SourceSpan::new(first_span.start, end_span.end),
            )
        } else {
            (Some(first), None, None, first_span)
        };
        if self.peek() != Some(':') {
            return Ok(None);
        }
        self.advance()?;
        self.skip_horizontal_whitespace()?;
        let Some((label, _label_span)) = self.label(decode_escape)? else {
            return Ok(None);
        };
        self.skip_horizontal_whitespace()?;
        if self.remaining().starts_with("%%") {
            self.skip_line()?;
        } else if !self.consume_eol()? {
            return Ok(None);
        }
        Ok(Some(PacketField {
            start,
            end,
            bits,
            label,
            numeric_span,
            #[cfg(feature = "editor-facts")]
            label_span: _label_span,
        }))
    }

    fn label(
        &mut self,
        decode_escape: impl Fn(char) -> Option<char>,
    ) -> OperationControlResult<Option<(String, SourceSpan)>> {
        let quote = match self.peek() {
            Some(quote @ ('\'' | '"')) => quote,
            _ => return Ok(None),
        };
        let slot = usize::from(quote == '\'');
        let opening = self.offset;
        if self.exhausted_quotes[slot].is_some_and(|start| opening >= start) {
            return Ok(None);
        }
        self.advance()?;
        let start = self.offset;
        let mut value = String::new();
        let mut escaped = false;
        while let Some(ch) = self.advance()? {
            if escaped {
                let Some(ch) = decode_escape(ch) else {
                    return Ok(None);
                };
                value.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                return Ok(Some((value, SourceSpan::new(start, self.offset - 1))));
            } else {
                value.push(ch);
            }
        }
        self.exhausted_quotes[slot] = Some(opening);
        Ok(None)
    }

    fn consume_eol(&mut self) -> OperationControlResult<bool> {
        if self.remaining().starts_with("\r\n") {
            self.advance()?;
            self.advance()?;
            return Ok(true);
        }
        if self.peek() == Some('\n') {
            self.advance()?;
            return Ok(true);
        }
        Ok(self.offset == self.source.len())
    }

    fn skip_line(&mut self) -> OperationControlResult<()> {
        while let Some(ch) = self.advance()? {
            if ch == '\n' {
                break;
            }
        }
        Ok(())
    }

    fn recover_statement(&mut self) -> OperationControlResult<SourceSpan> {
        while self.recovery.next_offset.is_none() && self.advance()?.is_some() {}
        self.offset = self.recovery.next_offset.unwrap_or(self.source.len());
        Ok(self.recovery.span)
    }
}

fn packet_header_token_len(line: &str) -> Option<usize> {
    if let Some(rest) = line.strip_prefix("packet-beta")
        && keyword_boundary(rest)
    {
        return Some("packet-beta".len());
    }
    if let Some(rest) = line.strip_prefix("packet")
        && keyword_boundary(rest)
    {
        return Some("packet".len());
    }
    None
}

fn keyword_boundary(rest: &str) -> bool {
    rest.is_empty()
        || rest.starts_with("%%")
        || rest.chars().next().is_some_and(char::is_whitespace)
}

/// Expands the lexed fields into the canonical Packet rows, leaving common DB fields empty.
///
/// Row width is an arithmetic input. The materialized row count and floating-point progress
/// are checked separately from accepting a JavaScript Number as a field value.
pub fn expand_fields(
    fields: Vec<PacketField>,
    bits_per_row: Option<f64>,
    control: &OperationControl,
) -> OperationControlResult<Result<PacketDiagramModel, PacketExpansionDiagnostic>> {
    let bits_per_row = match validate_packet_bits_per_row(bits_per_row) {
        Ok(bits_per_row) => bits_per_row,
        Err(error) => return Ok(Err(PacketExpansionDiagnostic::InvalidRowWidth(error))),
    };
    let mut packet = Vec::new();
    let mut last_bit = -1.0;
    let mut word = Vec::new();
    let mut row = 1.0;

    for (block_index, block) in fields.into_iter().enumerate() {
        if block_index % 128 == 0 {
            control.checkpoint()?;
        }
        if let (Some(start), Some(end)) = (block.start, block.end)
            && end < start
        {
            return Ok(Err(PacketExpansionDiagnostic::Field {
                span: block.numeric_span,
                error: PacketFieldError::ReversedRange { start, end },
            }));
        }

        let expected_start = last_bit + 1.0;
        let start = block.start.unwrap_or(expected_start);
        let end_for_msg = block.end.unwrap_or(start);
        if start != expected_start {
            return Ok(Err(PacketExpansionDiagnostic::Field {
                span: block.numeric_span,
                error: PacketFieldError::NotContiguous {
                    start,
                    end: end_for_msg,
                    expected: expected_start,
                },
            }));
        }

        if block.bits == Some(0.0) {
            return Ok(Err(PacketExpansionDiagnostic::Field {
                span: block.numeric_span,
                error: PacketFieldError::ZeroBitField { start },
            }));
        }

        // Keep the source's left-associative Number operations and its explicit relative width.
        // `start + bits - 1` differs from `start + (bits - 1)` at binary64 boundaries.
        let end = block
            .end
            .unwrap_or_else(|| start + block.bits.unwrap_or(1.0) - 1.0);
        let bits = block.bits.unwrap_or(end - start + 1.0);
        last_bit = end;

        let mut cur = PacketBlock {
            start,
            end,
            bits,
            label: block.label,
        };

        let mut split_count = 0usize;
        loop {
            if split_count.is_multiple_of(128) {
                control.checkpoint()?;
            }
            split_count = split_count.saturating_add(1);
            if packet.len() >= MAX_PACKET_ROWS {
                return Ok(Err(PacketExpansionDiagnostic::Field {
                    span: block.numeric_span,
                    error: PacketFieldError::RowLimit {
                        max_rows: MAX_PACKET_ROWS,
                    },
                }));
            }
            let (fitting, next) =
                match get_next_fitting_block(cur, row, bits_per_row, block.numeric_span) {
                    Ok(next) => next,
                    Err(error) => return Ok(Err(error)),
                };
            let reached_row_end = fitting.end + 1.0 == row * bits_per_row;
            word.push(fitting);
            if reached_row_end {
                packet.push(std::mem::take(&mut word));
                // The materialized row budget bounds this counter, independently of bit values.
                row += 1.0;
            }
            let Some(next) = next else {
                break;
            };
            cur = next;
        }
    }

    if !word.is_empty() {
        packet.push(word);
    }

    control.checkpoint()?;
    Ok(Ok(PacketDiagramModel {
        packet,
        ..PacketDiagramModel::default()
    }))
}

fn get_next_fitting_block(
    block: PacketBlock,
    row: f64,
    bits_per_row: f64,
    numeric_span: SourceSpan,
) -> Result<(PacketBlock, Option<PacketBlock>), PacketExpansionDiagnostic> {
    if block.start > block.end {
        return Err(PacketExpansionDiagnostic::Field {
            span: numeric_span,
            error: PacketFieldError::InvertedFragment {
                start: block.start,
                end: block.end,
            },
        });
    }

    let row_start = row * bits_per_row;

    if block.end + 1.0 <= row_start {
        return Ok((block, None));
    }

    let row_end = row_start - 1.0;
    // Without this equality the upstream loop keeps the same row and repeatedly emits the same
    // remainder. Detect that Number-rounding stall before allocating any repeated fragments.
    if row_end + 1.0 != row_start {
        return Err(PacketExpansionDiagnostic::Field {
            span: numeric_span,
            error: PacketFieldError::Nonprogress,
        });
    }
    Ok((
        PacketBlock {
            start: block.start,
            end: row_end,
            label: block.label.clone(),
            bits: row_end - block.start,
        },
        Some(PacketBlock {
            start: row_start,
            end: block.end,
            label: block.label,
            bits: block.end - row_start,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_long_tokens_and_recovery_can_cancel_with_bounded_scan_work() {
        let payload = "雪".repeat(8 * PACKET_SCAN_CHECKPOINT_BYTES);
        for source in [
            format!("0: \"{payload}\nB\""),
            format!("0: '{payload}"),
            format!("+{}: 'bit'", "9".repeat(payload.len())),
            format!("0: 'bit' %% {payload}"),
            format!("invalid {payload}"),
            format!("0: 'bit' garbage {payload}"),
        ] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(1);
            let mut cursor = PacketLexer::new(&source, 0, &control);
            let cancelled = match cursor.field(Some) {
                Err(_) => true,
                Ok(None) => cursor.recover_statement().is_err(),
                Ok(Some(_)) => false,
            };
            assert!(cancelled, "long Packet scan did not cancel");
            assert!(cursor.scanned_bytes <= 2 * (PACKET_SCAN_CHECKPOINT_BYTES + 3));
            assert!(cursor.scanned_bytes < source.len());
        }
    }

    #[test]
    fn packet_recovery_scans_quoted_suffixes_only_linearly() {
        let mut source = String::from("+1: 'unterminated\n");
        for _ in 0..1024 {
            source.push_str("+1: \"unterminated\n");
            source.push_str("+1: \"closed\" garbage\n");
        }
        source.push_str("+1: \"survivor\"\n");
        let control = OperationControl::new();
        let mut cursor = PacketLexer::new(&source, 0, &control);
        let mut labels = Vec::new();
        while let Some(field) = cursor.next_field(|_, _| None, Some).unwrap() {
            if let Ok(field) = field {
                labels.push(field.label);
            }
        }
        assert_eq!(labels, ["survivor"]);
        assert!(cursor.scanned_bytes <= 3 * source.len());
    }

    #[test]
    fn packet_row_expansion_checks_cancellation_inside_a_single_large_field() {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let source = "0-319999: 'bounded expansion'";
        let scan_control = OperationControl::new();
        let mut cursor = PacketLexer::new(source, 0, &scan_control);
        let field = cursor
            .next_field(|_, _| None, Some)
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(expand_fields(vec![field], Some(32.0), &control).is_err());
    }
}
