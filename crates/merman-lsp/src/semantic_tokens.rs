use crate::client_profile::{ClientProtocolProfile, SemanticTokenProjection};
#[cfg(test)]
use crate::syntax_highlighting::SyntaxTokenKind;
use crate::syntax_highlighting::{SyntaxCapture, SyntaxDocumentState, SyntaxHighlightError};
use merman_analysis::AnalysisCancellationToken;
use std::cmp::Reverse;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range as ByteRange;
use tower_lsp_server::ls_types::{
    Position, Range, SemanticToken, SemanticTokensDelta, SemanticTokensEdit,
    SemanticTokensFullDeltaResult, SemanticTokensOptions,
};

const SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyntaxTokenPlan {
    packed: Vec<u32>,
}

impl SyntaxTokenPlan {
    pub(crate) fn packed(&self) -> &[u32] {
        &self.packed
    }

    pub(crate) fn into_packed(self) -> Vec<u32> {
        self.packed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SemanticTokenError {
    InvalidRange(String),
    PositionOverflow { value: usize },
    Syntax(SyntaxHighlightError),
}

impl SemanticTokenError {
    pub(crate) const fn is_invalid_range(&self) -> bool {
        matches!(self, Self::InvalidRange(_))
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        matches!(self, Self::Syntax(error) if error.is_cancelled())
    }
}

impl std::fmt::Display for SemanticTokenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRange(message) => formatter.write_str(message),
            Self::PositionOverflow { value } => {
                write!(
                    formatter,
                    "token position {value} exceeds the LSP u32 contract"
                )
            }
            Self::Syntax(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticTokenError {}

impl From<SyntaxHighlightError> for SemanticTokenError {
    fn from(value: SyntaxHighlightError) -> Self {
        Self::Syntax(value)
    }
}

pub(crate) fn semantic_tokens_options_with_profile(
    profile: &ClientProtocolProfile,
) -> Option<SemanticTokensOptions> {
    profile
        .semantic_tokens
        .as_ref()
        .map(SemanticTokenProjection::options)
}

pub(crate) fn semantic_token_plan_for_document_with_profile(
    document: &SyntaxDocumentState,
    cancellation: &AnalysisCancellationToken,
    profile: &ClientProtocolProfile,
) -> Result<Option<SyntaxTokenPlan>, SemanticTokenError> {
    let projection = profile.semantic_tokens.as_ref();
    let Some(projection) = projection.filter(|projection| projection.supports_full()) else {
        return Ok(None);
    };
    token_plan(document, None, cancellation, projection).map(Some)
}

pub(crate) fn semantic_token_plan_for_document_range_with_profile(
    document: &SyntaxDocumentState,
    range: Range,
    cancellation: &AnalysisCancellationToken,
    profile: &ClientProtocolProfile,
) -> Result<Option<SyntaxTokenPlan>, SemanticTokenError> {
    let projection = profile.semantic_tokens.as_ref();
    let Some(projection) = projection.filter(|projection| projection.supports_range()) else {
        return Ok(None);
    };
    token_plan(document, Some(range), cancellation, projection).map(Some)
}

pub(crate) fn semantic_tokens_delta_result(
    previous_packed: &[u32],
    current_packed: &[u32],
    result_id: String,
) -> SemanticTokensFullDeltaResult {
    let Some(edit) = semantic_tokens_delta_edit(previous_packed, current_packed) else {
        return SemanticTokensFullDeltaResult::TokensDelta(SemanticTokensDelta {
            result_id: Some(result_id),
            edits: Vec::new(),
        });
    };

    SemanticTokensFullDeltaResult::TokensDelta(SemanticTokensDelta {
        result_id: Some(result_id),
        edits: vec![edit],
    })
}

pub(crate) fn semantic_tokens_result_id(document: &SyntaxDocumentState, packed: &[u32]) -> String {
    let mut hasher = DefaultHasher::new();
    document.version().hash(&mut hasher);
    packed.hash(&mut hasher);
    format!(
        "{}:tree-sitter:{:016x}",
        document.version(),
        hasher.finish()
    )
}

fn token_plan(
    document: &SyntaxDocumentState,
    requested: Option<Range>,
    cancellation: &AnalysisCancellationToken,
    projection: &SemanticTokenProjection,
) -> Result<SyntaxTokenPlan, SemanticTokenError> {
    let source = document.source();
    let index = SourceIndex::new(source);
    let requested = requested.map(|range| index.byte_range(range)).transpose()?;
    let captures = document.captures(requested, cancellation)?;
    let packed = project_captures(source, &index, &captures, projection)?;
    Ok(SyntaxTokenPlan { packed })
}

#[derive(Debug, Clone, Copy)]
struct ProjectedCapture {
    start_byte: usize,
    end_byte: usize,
    token_type: u32,
    specificity: usize,
    pattern_index: usize,
    capture_name_rank: usize,
}

#[derive(Debug, Clone, Copy)]
struct ByteToken {
    start_byte: usize,
    end_byte: usize,
    token_type: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AbsoluteToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
}

fn project_captures(
    source: &str,
    index: &SourceIndex,
    captures: &[SyntaxCapture],
    projection: &SemanticTokenProjection,
) -> Result<Vec<u32>, SemanticTokenError> {
    let projected = captures
        .iter()
        .filter_map(|capture| {
            projection
                .token_type(capture.kind)
                .map(|token_type| ProjectedCapture {
                    start_byte: capture.start_byte,
                    end_byte: capture.end_byte,
                    token_type,
                    specificity: capture.specificity,
                    pattern_index: capture.pattern_index,
                    capture_name_rank: capture.capture_name_rank,
                })
        })
        .collect::<Vec<_>>();
    let byte_tokens = resolve_overlaps(&projected);
    let mut absolute = Vec::new();
    let mut utf16 = Utf16Cursor::default();
    for token in byte_tokens {
        split_token_by_line(source, index, token, &mut utf16, &mut absolute)?;
    }
    merge_adjacent_tokens(&mut absolute);
    pack_tokens(&absolute)
}

fn resolve_overlaps(captures: &[ProjectedCapture]) -> Vec<ByteToken> {
    if captures.is_empty() {
        return Vec::new();
    }
    let mut events = Vec::with_capacity(captures.len() * 2);
    for (index, capture) in captures.iter().enumerate() {
        if capture.start_byte < capture.end_byte {
            events.push((capture.start_byte, true, index));
            events.push((capture.end_byte, false, index));
        }
    }
    events.sort_unstable_by_key(|(position, starts, index)| (*position, *starts, *index));

    let mut active = Vec::<usize>::new();
    let mut output = Vec::<ByteToken>::new();
    let mut cursor = 0usize;
    while cursor < events.len() {
        let position = events[cursor].0;
        while cursor < events.len() && events[cursor].0 == position && !events[cursor].1 {
            let index = events[cursor].2;
            active.retain(|active_index| *active_index != index);
            cursor += 1;
        }
        while cursor < events.len() && events[cursor].0 == position && events[cursor].1 {
            active.push(events[cursor].2);
            cursor += 1;
        }
        let Some(next_position) = events.get(cursor).map(|event| event.0) else {
            break;
        };
        if position == next_position {
            continue;
        }
        let Some(winner) = active.iter().copied().max_by_key(|index| {
            let capture = captures[*index];
            (
                capture.specificity,
                capture.pattern_index,
                Reverse(capture.end_byte - capture.start_byte),
                capture.capture_name_rank,
            )
        }) else {
            continue;
        };
        let capture = captures[winner];
        if let Some(previous) = output.last_mut()
            && previous.end_byte == position
            && previous.token_type == capture.token_type
        {
            previous.end_byte = next_position;
        } else {
            output.push(ByteToken {
                start_byte: position,
                end_byte: next_position,
                token_type: capture.token_type,
            });
        }
    }
    output
}

fn split_token_by_line(
    source: &str,
    index: &SourceIndex,
    token: ByteToken,
    utf16: &mut Utf16Cursor,
    output: &mut Vec<AbsoluteToken>,
) -> Result<(), SemanticTokenError> {
    let mut cursor = token.start_byte;
    while cursor < token.end_byte {
        let line_index = index.line_for_byte(cursor);
        let line = index.lines[line_index];
        if cursor >= line.content_end {
            cursor = line.end.max(cursor.saturating_add(1)).min(token.end_byte);
            continue;
        }
        let segment_end = token.end_byte.min(line.content_end);
        let start = utf16.advance_to(source, line_index, line, cursor);
        let end = utf16.advance_to(source, line_index, line, segment_end);
        let length = end - start;
        if length > 0 {
            output.push(AbsoluteToken {
                line: checked_u32(line_index)?,
                start: checked_u32(start)?,
                length: checked_u32(length)?,
                token_type: token.token_type,
            });
        }
        cursor = if segment_end < token.end_byte && segment_end == line.content_end {
            line.end
        } else {
            segment_end
        };
    }
    Ok(())
}

#[derive(Debug, Default)]
struct Utf16Cursor {
    line_index: Option<usize>,
    byte: usize,
    units: usize,
}

impl Utf16Cursor {
    fn advance_to(
        &mut self,
        source: &str,
        line_index: usize,
        line: SourceLine,
        byte: usize,
    ) -> usize {
        if self.line_index != Some(line_index) || byte < self.byte {
            self.line_index = Some(line_index);
            self.byte = line.start;
            self.units = 0;
        }
        self.units += source[self.byte..byte].encode_utf16().count();
        self.byte = byte;
        self.units
    }
}

fn merge_adjacent_tokens(tokens: &mut Vec<AbsoluteToken>) {
    if tokens.len() < 2 {
        return;
    }
    let mut merged: Vec<AbsoluteToken> = Vec::with_capacity(tokens.len());
    for token in tokens.drain(..) {
        if let Some(previous) = merged.last_mut()
            && previous.line == token.line
            && previous.token_type == token.token_type
            && previous.start.saturating_add(previous.length) == token.start
        {
            previous.length = previous.length.saturating_add(token.length);
        } else {
            merged.push(token);
        }
    }
    *tokens = merged;
}

fn pack_tokens(tokens: &[AbsoluteToken]) -> Result<Vec<u32>, SemanticTokenError> {
    let mut packed = Vec::with_capacity(tokens.len() * SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN);
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    for token in tokens {
        let delta_line = token.line - previous_line;
        let delta_start = if delta_line == 0 {
            token.start - previous_start
        } else {
            token.start
        };
        packed.extend_from_slice(&[delta_line, delta_start, token.length, token.token_type, 0]);
        previous_line = token.line;
        previous_start = token.start;
    }
    Ok(packed)
}

fn checked_u32(value: usize) -> Result<u32, SemanticTokenError> {
    u32::try_from(value).map_err(|_| SemanticTokenError::PositionOverflow { value })
}

#[derive(Debug, Clone, Copy)]
struct SourceLine {
    start: usize,
    content_end: usize,
    end: usize,
}

#[derive(Debug)]
struct SourceIndex<'a> {
    source: &'a str,
    lines: Vec<SourceLine>,
}

impl<'a> SourceIndex<'a> {
    fn new(source: &'a str) -> Self {
        let bytes = source.as_bytes();
        let mut lines = Vec::new();
        let mut line_start = 0usize;
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\n' => {
                    lines.push(SourceLine {
                        start: line_start,
                        content_end: cursor,
                        end: cursor + 1,
                    });
                    cursor += 1;
                    line_start = cursor;
                }
                b'\r' => {
                    let end = if bytes.get(cursor + 1) == Some(&b'\n') {
                        cursor + 2
                    } else {
                        cursor + 1
                    };
                    lines.push(SourceLine {
                        start: line_start,
                        content_end: cursor,
                        end,
                    });
                    cursor = end;
                    line_start = cursor;
                }
                _ => cursor += 1,
            }
        }
        lines.push(SourceLine {
            start: line_start,
            content_end: source.len(),
            end: source.len(),
        });
        Self { source, lines }
    }

    fn byte_range(&self, range: Range) -> Result<ByteRange<usize>, SemanticTokenError> {
        if range.start.line > range.end.line
            || (range.start.line == range.end.line && range.start.character > range.end.character)
        {
            return Err(SemanticTokenError::InvalidRange(format!(
                "semantic token range start {}:{} is after end {}:{}",
                range.start.line, range.start.character, range.end.line, range.end.character
            )));
        }
        let start = self.byte_offset(range.start, "start")?;
        let end = self.byte_offset(range.end, "end")?;
        Ok(start..end)
    }

    fn byte_offset(&self, position: Position, endpoint: &str) -> Result<usize, SemanticTokenError> {
        let line_index = position.line as usize;
        let Some(line) = self.lines.get(line_index).copied() else {
            return Err(SemanticTokenError::InvalidRange(format!(
                "semantic token range {endpoint} line {} is outside the {}-line document",
                position.line,
                self.lines.len()
            )));
        };
        let source = self.source_line(line);
        let target = position.character as usize;
        let mut utf16 = 0usize;
        for (relative_byte, character) in source.char_indices() {
            if utf16 == target {
                return Ok(line.start + relative_byte);
            }
            let next = utf16 + character.len_utf16();
            if target < next {
                return Err(SemanticTokenError::InvalidRange(format!(
                    "semantic token range {endpoint} character {} splits a UTF-16 surrogate pair on line {}",
                    position.character, position.line
                )));
            }
            utf16 = next;
        }
        if utf16 == target {
            Ok(line.content_end)
        } else {
            Err(SemanticTokenError::InvalidRange(format!(
                "semantic token range {endpoint} character {} is outside line {} with UTF-16 length {utf16}",
                position.character, position.line
            )))
        }
    }

    fn source_line(&self, line: SourceLine) -> &str {
        &self.source[line.start..line.content_end]
    }

    fn line_for_byte(&self, byte: usize) -> usize {
        self.lines
            .partition_point(|line| line.start <= byte)
            .saturating_sub(1)
            .min(self.lines.len().saturating_sub(1))
    }
}

pub(crate) fn semantic_tokens_from_packed(packed: &[u32]) -> Vec<SemanticToken> {
    debug_assert_eq!(packed.len() % SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN, 0);
    packed
        .chunks_exact(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN)
        .map(|words| SemanticToken {
            delta_line: words[0],
            delta_start: words[1],
            length: words[2],
            token_type: words[3],
            token_modifiers_bitset: words[4],
        })
        .collect()
}

fn semantic_tokens_delta_edit(
    previous_packed: &[u32],
    current_packed: &[u32],
) -> Option<SemanticTokensEdit> {
    debug_assert_eq!(
        previous_packed.len() % SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,
        0
    );
    debug_assert_eq!(
        current_packed.len() % SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,
        0
    );
    let previous_tokens = previous_packed.chunks_exact(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN);
    let current_tokens = current_packed.chunks_exact(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN);
    let previous_token_count = previous_tokens.len();
    let current_token_count = current_tokens.len();
    let prefix_tokens = previous_tokens
        .clone()
        .zip(current_tokens.clone())
        .take_while(|(previous, current)| previous == current)
        .count();

    if prefix_tokens == previous_token_count && prefix_tokens == current_token_count {
        return None;
    }

    let suffix_tokens = previous_tokens
        .skip(prefix_tokens)
        .rev()
        .zip(current_tokens.skip(prefix_tokens).rev())
        .take_while(|(previous, current)| previous == current)
        .count();

    let previous_end = previous_token_count.saturating_sub(suffix_tokens);
    let current_end = current_token_count.saturating_sub(suffix_tokens);
    let flattened_prefix = prefix_tokens
        .checked_mul(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN)
        .and_then(|value| u32::try_from(value).ok())
        .expect("semantic token delta prefix fits the LSP u32 contract");
    let flattened_delete_count = (previous_end - prefix_tokens)
        .checked_mul(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN)
        .and_then(|value| u32::try_from(value).ok())
        .expect("semantic token delta deletion fits the LSP u32 contract");

    Some(SemanticTokensEdit {
        start: flattened_prefix,
        delete_count: flattened_delete_count,
        data: if prefix_tokens < current_end {
            let start = prefix_tokens * SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN;
            let end = current_end * SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN;
            Some(semantic_tokens_from_packed(&current_packed[start..end]))
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_editor_core::DocumentKind;
    use std::sync::Arc;
    use tower_lsp_server::ls_types::ClientCapabilities;

    fn cancellation() -> AnalysisCancellationToken {
        AnalysisCancellationToken::new()
    }

    fn profile(requests: serde_json::Value, token_types: &[&str]) -> ClientProtocolProfile {
        let capabilities: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "textDocument": {
                "semanticTokens": {
                    "requests": requests,
                    "tokenTypes": token_types,
                    "tokenModifiers": [],
                    "formats": ["relative"]
                }
            }
        }))
        .unwrap();
        ClientProtocolProfile::negotiate(&capabilities)
    }

    fn document(version: i32, source: &str) -> SyntaxDocumentState {
        SyntaxDocumentState::parse(
            version,
            DocumentKind::Diagram,
            Arc::from(source),
            &cancellation(),
        )
        .unwrap()
    }

    #[test]
    fn full_sequence_comes_from_tree_sitter_captures() {
        let profile = profile(
            serde_json::json!({ "full": true }),
            &["keyword", "variable"],
        );
        let document = document(1, "flowchart TD\nA --> B\n");
        let plan =
            semantic_token_plan_for_document_with_profile(&document, &cancellation(), &profile)
                .unwrap()
                .unwrap();
        let tokens = semantic_tokens_from_packed(plan.packed());

        assert!(!tokens.is_empty());
        assert!(tokens.iter().all(|token| token.token_type <= 1));
        assert!(tokens.iter().all(|token| token.token_modifiers_bitset == 0));
    }

    #[test]
    fn request_modes_are_negotiated_independently() {
        let document = document(1, "flowchart TD\nA-->B\n");
        let range = Range::new(Position::new(0, 0), Position::new(2, 0));
        let range_only = profile(serde_json::json!({ "range": true }), &["keyword"]);
        assert!(
            semantic_token_plan_for_document_with_profile(&document, &cancellation(), &range_only,)
                .unwrap()
                .is_none()
        );
        assert!(
            semantic_token_plan_for_document_range_with_profile(
                &document,
                range,
                &cancellation(),
                &range_only,
            )
            .unwrap()
            .is_some()
        );
    }

    #[test]
    fn projection_splits_multiline_captures_and_uses_utf16_columns() {
        let source = "🤓alpha\r\nbeta\n";
        let profile = ClientProtocolProfile::permissive();
        let projection = profile.semantic_tokens.as_ref().unwrap();
        let captures = [
            SyntaxCapture {
                start_byte: 0,
                end_byte: source.len(),
                kind: SyntaxTokenKind::String,
                specificity: 1,
                pattern_index: 0,
                capture_name_rank: 0,
            },
            SyntaxCapture {
                start_byte: 0,
                end_byte: "🤓".len(),
                kind: SyntaxTokenKind::Variable,
                specificity: 1,
                pattern_index: 1,
                capture_name_rank: 1,
            },
        ];
        let index = SourceIndex::new(source);
        let tokens = semantic_tokens_from_packed(
            &project_captures(source, &index, &captures, projection).unwrap(),
        );

        assert_eq!(
            decode_tokens(&tokens),
            vec![
                (
                    0,
                    0,
                    2,
                    projection.token_type(SyntaxTokenKind::Variable).unwrap(),
                    0
                ),
                (
                    0,
                    2,
                    5,
                    projection.token_type(SyntaxTokenKind::String).unwrap(),
                    0
                ),
                (
                    1,
                    0,
                    4,
                    projection.token_type(SyntaxTokenKind::String).unwrap(),
                    0
                ),
            ]
        );
    }

    #[test]
    fn overlapping_query_captures_follow_browser_projector_priority() {
        let source = "abcdef";
        let projection = ClientProtocolProfile::permissive()
            .semantic_tokens
            .expect("permissive profile enables syntax tokens");
        let captures = [
            SyntaxCapture {
                start_byte: 0,
                end_byte: 6,
                kind: SyntaxTokenKind::String,
                specificity: 1,
                pattern_index: 99,
                capture_name_rank: 9,
            },
            SyntaxCapture {
                start_byte: 1,
                end_byte: 5,
                kind: SyntaxTokenKind::Property,
                specificity: 2,
                pattern_index: 0,
                capture_name_rank: 0,
            },
            SyntaxCapture {
                start_byte: 1,
                end_byte: 5,
                kind: SyntaxTokenKind::Keyword,
                specificity: 2,
                pattern_index: 1,
                capture_name_rank: 9,
            },
            SyntaxCapture {
                start_byte: 2,
                end_byte: 4,
                kind: SyntaxTokenKind::Number,
                specificity: 2,
                pattern_index: 1,
                capture_name_rank: 0,
            },
        ];

        let index = SourceIndex::new(source);
        let tokens = semantic_tokens_from_packed(
            &project_captures(source, &index, &captures, &projection).unwrap(),
        );

        assert_eq!(
            decode_tokens(&tokens),
            vec![
                (
                    0,
                    0,
                    1,
                    projection.token_type(SyntaxTokenKind::String).unwrap(),
                    0,
                ),
                (
                    0,
                    1,
                    1,
                    projection.token_type(SyntaxTokenKind::Keyword).unwrap(),
                    0,
                ),
                (
                    0,
                    2,
                    2,
                    projection.token_type(SyntaxTokenKind::Number).unwrap(),
                    0,
                ),
                (
                    0,
                    4,
                    1,
                    projection.token_type(SyntaxTokenKind::Keyword).unwrap(),
                    0,
                ),
                (
                    0,
                    5,
                    1,
                    projection.token_type(SyntaxTokenKind::String).unwrap(),
                    0,
                ),
            ]
        );
    }

    #[test]
    fn markdown_range_projects_only_overlapping_fences() {
        let cancellation = cancellation();
        let document = SyntaxDocumentState::parse(
            1,
            DocumentKind::Markdown,
            Arc::from(concat!(
                "intro\n",
                "```mermaid\nflowchart LR\nA-->B\n```\n",
                "middle\n",
                "```mermaid\nsequenceDiagram\nA->>B: Hi 🤓\n```\n",
            )),
            &cancellation,
        )
        .unwrap();
        let profile = profile(
            serde_json::json!({ "range": true }),
            &["keyword", "variable", "string"],
        );
        let plan = semantic_token_plan_for_document_range_with_profile(
            &document,
            Range::new(Position::new(6, 0), Position::new(10, 0)),
            &cancellation,
            &profile,
        )
        .unwrap()
        .unwrap();

        assert!(
            decode_tokens(&semantic_tokens_from_packed(plan.packed()))
                .iter()
                .all(|token| (6..=10).contains(&token.0))
        );
    }

    #[test]
    fn invalid_ranges_are_reported_without_parsing_semantics() {
        let document = document(1, "flowchart TD\nA-->B\n");
        let profile = profile(serde_json::json!({ "range": true }), &["keyword"]);
        let error = semantic_token_plan_for_document_range_with_profile(
            &document,
            Range::new(Position::new(10, 0), Position::new(11, 0)),
            &cancellation(),
            &profile,
        )
        .unwrap_err();

        assert!(error.is_invalid_range());
        assert!(
            error.to_string().contains("end line 11 is outside")
                || error.to_string().contains("start line 10 is outside")
        );
    }

    #[test]
    fn result_id_is_stable_and_changes_with_document_version() {
        let first = document(7, "flowchart TD\nA --> B\n");
        let second = document(8, "flowchart TD\nA --> B\n");
        let packed = [0, 0, 3, 0, 0];

        assert_eq!(
            semantic_tokens_result_id(&first, &packed),
            semantic_tokens_result_id(&first, &packed)
        );
        assert_ne!(
            semantic_tokens_result_id(&first, &packed),
            semantic_tokens_result_id(&second, &packed)
        );
    }

    #[test]
    fn delta_round_trips_insertions_removals_and_noop_streams() {
        let first = vec![semantic_token(0, 0, 3, 0), semantic_token(0, 4, 2, 1)];
        let inserted = vec![
            semantic_token(0, 0, 3, 0),
            semantic_token(0, 4, 1, 2),
            semantic_token(0, 2, 2, 1),
        ];

        for (previous, current) in [(&first, &inserted), (&inserted, &first)] {
            let result = semantic_tokens_delta_result(
                &flatten_tokens(previous),
                &flatten_tokens(current),
                "next".to_string(),
            );
            let SemanticTokensFullDeltaResult::TokensDelta(delta) = result else {
                panic!("expected delta tokens");
            };
            assert_eq!(
                apply_delta(&flatten_tokens(previous), &delta.edits),
                flatten_tokens(current)
            );
        }

        let unchanged = flatten_tokens(&first);
        let SemanticTokensFullDeltaResult::TokensDelta(delta) =
            semantic_tokens_delta_result(&unchanged, &unchanged, "same".to_string())
        else {
            panic!("expected no-op delta tokens");
        };
        assert!(delta.edits.is_empty());
    }

    fn semantic_token(
        delta_line: u32,
        delta_start: u32,
        length: u32,
        token_type: u32,
    ) -> SemanticToken {
        SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        }
    }

    fn flatten_tokens(tokens: &[SemanticToken]) -> Vec<u32> {
        tokens
            .iter()
            .flat_map(|token| {
                [
                    token.delta_line,
                    token.delta_start,
                    token.length,
                    token.token_type,
                    token.token_modifiers_bitset,
                ]
            })
            .collect()
    }

    fn apply_delta(previous: &[u32], edits: &[SemanticTokensEdit]) -> Vec<u32> {
        let mut result = previous.to_vec();
        for edit in edits.iter().rev() {
            let start = edit.start as usize;
            let end = start + edit.delete_count as usize;
            let replacement = edit.data.as_deref().map(flatten_tokens).unwrap_or_default();
            result.splice(start..end, replacement);
        }
        result
    }

    fn decode_tokens(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
        let mut line = 0u32;
        let mut start = 0u32;
        tokens
            .iter()
            .map(|token| {
                line += token.delta_line;
                if token.delta_line == 0 {
                    start += token.delta_start;
                } else {
                    start = token.delta_start;
                }
                (
                    line,
                    start,
                    token.length,
                    token.token_type,
                    token.token_modifiers_bitset,
                )
            })
            .collect()
    }
}
