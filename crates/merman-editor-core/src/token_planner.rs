use crate::generated::{PlannedTokenKind, PlannedTokenModifier, TokenOverlayKind};
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::Range;
use merman_analysis::{
    ByteSpan, EditorSymbolKind, FenceLexemeFailure, FenceLexemeKind, FenceLexemeModifier,
    FenceSemanticRole, SourceMap,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedToken {
    pub line: u32,
    pub start: u32,
    pub length: u32,
    pub kind: PlannedTokenKind,
    pub modifier_bits: u32,
}

impl PlannedToken {
    pub const fn has_modifier(self, modifier: PlannedTokenModifier) -> bool {
        self.modifier_bits & modifier.bit() != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTokenPlan {
    tokens: Vec<PlannedToken>,
    packed: Vec<u32>,
}

impl SemanticTokenPlan {
    pub fn tokens(&self) -> &[PlannedToken] {
        &self.tokens
    }

    pub fn packed(&self) -> &[u32] {
        &self.packed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPlanError {
    ReversedRange {
        range: Range,
    },
    RangeStartLineOutOfBounds {
        range: Range,
        line_count: usize,
    },
    RangeEndLineOutOfBounds {
        range: Range,
        line_count: usize,
    },
    RangeStartCharacterOutOfBounds {
        range: Range,
        line_length_utf16: usize,
    },
    RangeEndCharacterOutOfBounds {
        range: Range,
        line_length_utf16: usize,
    },
    RangeStartCharacterNotBoundary {
        range: Range,
    },
    RangeEndCharacterNotBoundary {
        range: Range,
    },
    UpstreamLexemeFailure {
        fence_index: usize,
        failure: FenceLexemeFailure,
    },
    InvalidSpan {
        span: ByteSpan,
        source_len: usize,
    },
    UnsortedLexemes {
        previous: ByteSpan,
        current: ByteSpan,
    },
    OverlappingLexemes {
        left: ByteSpan,
        right: ByteSpan,
    },
    DuplicateModifier {
        span: ByteSpan,
        modifier: PlannedTokenModifier,
    },
    UnresolvedOverlap {
        left: ByteSpan,
        left_kind: PlannedTokenKind,
        right: ByteSpan,
        right_kind: PlannedTokenKind,
    },
    InvalidFenceDelimiter {
        fence_index: usize,
    },
    PositionOverflow {
        value: usize,
    },
}

impl fmt::Display for TokenPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReversedRange { range } => write!(
                formatter,
                "semantic token range start {}:{} is after end {}:{}",
                range.start.line, range.start.character, range.end.line, range.end.character
            ),
            Self::RangeStartLineOutOfBounds { range, line_count } => write!(
                formatter,
                "semantic token range start line {} is outside the document's {line_count} lines",
                range.start.line
            ),
            Self::RangeEndLineOutOfBounds { range, line_count } => write!(
                formatter,
                "semantic token range end line {} is outside the document's {line_count} lines",
                range.end.line
            ),
            Self::RangeStartCharacterOutOfBounds {
                range,
                line_length_utf16,
            } => write!(
                formatter,
                "semantic token range start character {} is outside line {}'s UTF-16 length {line_length_utf16}",
                range.start.character, range.start.line
            ),
            Self::RangeEndCharacterOutOfBounds {
                range,
                line_length_utf16,
            } => write!(
                formatter,
                "semantic token range end character {} is outside line {}'s UTF-16 length {line_length_utf16}",
                range.end.character, range.end.line
            ),
            Self::RangeStartCharacterNotBoundary { range } => write!(
                formatter,
                "semantic token range start character {} is not a Unicode scalar boundary on line {}",
                range.start.character, range.start.line
            ),
            Self::RangeEndCharacterNotBoundary { range } => write!(
                formatter,
                "semantic token range end character {} is not a Unicode scalar boundary on line {}",
                range.end.character, range.end.line
            ),
            Self::UpstreamLexemeFailure {
                fence_index,
                failure,
            } => write!(
                formatter,
                "fence {fence_index} contains invalid parser lexemes: {failure:?}"
            ),
            Self::InvalidSpan { span, source_len } => write!(
                formatter,
                "token span {}..{} is invalid for source length {source_len}",
                span.start, span.end
            ),
            Self::UnsortedLexemes { previous, current } => write!(
                formatter,
                "lexeme spans are not sorted: {previous:?} before {current:?}"
            ),
            Self::OverlappingLexemes { left, right } => {
                write!(formatter, "lexeme spans overlap: {left:?} and {right:?}")
            }
            Self::DuplicateModifier { span, modifier } => {
                write!(formatter, "token {span:?} repeats modifier {modifier:?}")
            }
            Self::UnresolvedOverlap {
                left,
                left_kind,
                right,
                right_kind,
            } => write!(
                formatter,
                "token precedence cannot resolve {left_kind:?} {left:?} against {right_kind:?} {right:?}"
            ),
            Self::InvalidFenceDelimiter { fence_index } => {
                write!(
                    formatter,
                    "fence {fence_index} has inconsistent delimiter bounds"
                )
            }
            Self::PositionOverflow { value } => {
                write!(
                    formatter,
                    "token position {value} exceeds the packed u32 contract"
                )
            }
        }
    }
}

impl std::error::Error for TokenPlanError {}

impl TokenPlanError {
    pub const fn is_invalid_range(&self) -> bool {
        matches!(
            self,
            Self::ReversedRange { .. }
                | Self::RangeStartLineOutOfBounds { .. }
                | Self::RangeEndLineOutOfBounds { .. }
                | Self::RangeStartCharacterOutOfBounds { .. }
                | Self::RangeEndCharacterOutOfBounds { .. }
                | Self::RangeStartCharacterNotBoundary { .. }
                | Self::RangeEndCharacterNotBoundary { .. }
        )
    }
}

pub fn plan_semantic_tokens_for_snapshot(
    snapshot: &DocumentSnapshot,
) -> Result<SemanticTokenPlan, TokenPlanError> {
    plan_semantic_tokens(snapshot, None)
}

/// Plans only the token candidates that overlap `range`.
///
/// This is intentionally a planner operation rather than a post-processing filter: Markdown and
/// MDX documents can contain many or very large Mermaid fences, and a range request must not
/// allocate and sort candidates outside the requested lines.
pub fn plan_semantic_tokens_for_snapshot_range(
    snapshot: &DocumentSnapshot,
    range: Range,
) -> Result<SemanticTokenPlan, TokenPlanError> {
    plan_semantic_tokens(snapshot, Some(range))
}

fn plan_semantic_tokens(
    snapshot: &DocumentSnapshot,
    range: Option<Range>,
) -> Result<SemanticTokenPlan, TokenPlanError> {
    let requested = match range {
        Some(range) => RequestedTokenRange::new(snapshot.source_map(), range)?,
        None => None,
    };
    let mut candidates = Vec::new();
    for fence in snapshot.fences() {
        if range.is_some() && requested.is_none() {
            break;
        }
        if let Some(requested) = requested
            && !requested.overlaps(ByteSpan {
                start: fence.document_range().start,
                end: fence.document_range().end,
            })
        {
            continue;
        }
        collect_fence_candidates(snapshot, fence, requested, &mut candidates)?;
    }
    let mut plan = plan_candidates(snapshot.source_map(), candidates)?;
    if let Some(range) = range {
        plan.tokens
            .retain(|token| token_overlaps_range(*token, range));
        plan.packed = pack_tokens(&plan.tokens);
    }
    Ok(plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestedTokenRange {
    byte_span: ByteSpan,
}

impl RequestedTokenRange {
    fn new(source_map: &SourceMap, range: Range) -> Result<Option<Self>, TokenPlanError> {
        if (range.start.line, range.start.character) > (range.end.line, range.end.character) {
            return Err(TokenPlanError::ReversedRange { range });
        }

        let line_count = source_map.line_count();
        let start_line =
            validate_range_position(source_map, range, RangeEndpoint::Start, line_count)?;
        let end_line = validate_range_position(source_map, range, RangeEndpoint::End, line_count)?;
        if range.start == range.end {
            return Ok(None);
        }

        Ok(Some(Self {
            byte_span: ByteSpan {
                start: start_line.0,
                end: end_line.1,
            },
        }))
    }

    const fn overlaps(self, span: ByteSpan) -> bool {
        span.start < self.byte_span.end && span.end > self.byte_span.start
    }
}

#[derive(Debug, Clone, Copy)]
enum RangeEndpoint {
    Start,
    End,
}

fn validate_range_position(
    source_map: &SourceMap,
    range: Range,
    endpoint: RangeEndpoint,
    line_count: usize,
) -> Result<(usize, usize), TokenPlanError> {
    let position = match endpoint {
        RangeEndpoint::Start => range.start,
        RangeEndpoint::End => range.end,
    };
    let Some((line_start, line_end)) = source_map.line_bounds(position.line) else {
        let error = match endpoint {
            RangeEndpoint::Start => TokenPlanError::RangeStartLineOutOfBounds { range, line_count },
            RangeEndpoint::End => TokenPlanError::RangeEndLineOutOfBounds { range, line_count },
        };
        return Err(error);
    };

    let line_length_utf16 = source_map
        .utf16_position(line_end)
        .map_err(|_| TokenPlanError::InvalidSpan {
            span: ByteSpan {
                start: line_end,
                end: line_end,
            },
            source_len: source_map.source_len(),
        })?
        .character;
    if position.character > line_length_utf16 {
        let error = match endpoint {
            RangeEndpoint::Start => TokenPlanError::RangeStartCharacterOutOfBounds {
                range,
                line_length_utf16,
            },
            RangeEndpoint::End => TokenPlanError::RangeEndCharacterOutOfBounds {
                range,
                line_length_utf16,
            },
        };
        return Err(error);
    }

    let position = merman_analysis::Utf16Position {
        line: position.line,
        character: position.character,
    };
    if source_map
        .byte_offset_for_utf16_position(position)
        .is_none()
    {
        let error = match endpoint {
            RangeEndpoint::Start => TokenPlanError::RangeStartCharacterNotBoundary { range },
            RangeEndpoint::End => TokenPlanError::RangeEndCharacterNotBoundary { range },
        };
        return Err(error);
    }

    Ok((line_start, line_end))
}

fn token_overlaps_range(token: PlannedToken, range: Range) -> bool {
    let token_line = token.line as usize;
    if token_line < range.start.line || token_line > range.end.line {
        return false;
    }

    let token_start = token.start as usize;
    let token_end = token_start + token.length as usize;
    if range.start.line == range.end.line {
        return token_line == range.start.line
            && token_end > range.start.character
            && token_start < range.end.character;
    }
    if token_line == range.start.line {
        return token_end > range.start.character;
    }
    if token_line == range.end.line {
        return token_start < range.end.character;
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenCandidate {
    span: ByteSpan,
    kind: PlannedTokenKind,
    modifiers: Vec<PlannedTokenModifier>,
    origin: TokenOverlayKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidTokenCandidate {
    span: ByteSpan,
    kind: PlannedTokenKind,
    modifier_bits: u32,
    origin: TokenOverlayKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlannedByteToken {
    span: ByteSpan,
    kind: PlannedTokenKind,
    modifier_bits: u32,
}

fn collect_fence_candidates(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    requested: Option<RequestedTokenRange>,
    candidates: &mut Vec<TokenCandidate>,
) -> Result<(), TokenPlanError> {
    reject_upstream_lexeme_failure(fence.index(), fence.text_index().lexeme_failure())?;

    let mut previous_lexeme: Option<ByteSpan> = None;
    for lexeme in fence.text_index().lexemes() {
        let span = absolute_fence_span(snapshot, fence, lexeme.span)?;
        if requested.is_some_and(|requested| !requested.overlaps(span)) {
            continue;
        }
        if let Some(previous) = previous_lexeme {
            if (previous.start, previous.end) > (span.start, span.end) {
                return Err(TokenPlanError::UnsortedLexemes {
                    previous,
                    current: span,
                });
            }
            if previous.end > span.start {
                return Err(TokenPlanError::OverlappingLexemes {
                    left: previous,
                    right: span,
                });
            }
        }
        previous_lexeme = Some(span);
        candidates.push(TokenCandidate {
            span,
            kind: planned_kind_for_lexeme(lexeme.kind),
            modifiers: lexeme
                .modifiers
                .iter()
                .copied()
                .map(planned_modifier_for_lexeme)
                .collect(),
            origin: TokenOverlayKind::Lexeme,
        });
    }

    for item in fence.text_index().semantic_items() {
        let span = absolute_fence_span(snapshot, fence, item.selection)?;
        if requested.is_some_and(|requested| !requested.overlaps(span)) {
            continue;
        }
        candidates.push(TokenCandidate {
            span,
            kind: planned_kind_for_symbol(item.kind),
            modifiers: vec![planned_modifier_for_role(item.role)],
            origin: token_overlay_for_role(item.role),
        });
    }

    collect_fence_delimiters(snapshot, fence, requested, candidates)
}

fn reject_upstream_lexeme_failure(
    fence_index: usize,
    failure: Option<FenceLexemeFailure>,
) -> Result<(), TokenPlanError> {
    match failure {
        Some(failure) => Err(TokenPlanError::UpstreamLexemeFailure {
            fence_index,
            failure,
        }),
        None => Ok(()),
    }
}

fn absolute_fence_span(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    relative: ByteSpan,
) -> Result<ByteSpan, TokenPlanError> {
    let body = fence.text();
    if relative.start >= relative.end
        || relative.end > body.len()
        || !body.is_char_boundary(relative.start)
        || !body.is_char_boundary(relative.end)
    {
        return Err(TokenPlanError::InvalidSpan {
            span: relative,
            source_len: body.len(),
        });
    }
    let body_start = fence.body_range().start;
    let start = body_start
        .checked_add(relative.start)
        .ok_or(TokenPlanError::PositionOverflow {
            value: relative.start,
        })?;
    let end = body_start
        .checked_add(relative.end)
        .ok_or(TokenPlanError::PositionOverflow {
            value: relative.end,
        })?;
    let span = ByteSpan { start, end };
    validate_span(snapshot.source_map().source(), span)?;
    Ok(span)
}

fn collect_fence_delimiters(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    requested: Option<RequestedTokenRange>,
    candidates: &mut Vec<TokenCandidate>,
) -> Result<(), TokenPlanError> {
    let (delimiter, spans) = match (fence.fence_delimiter(), fence.fence_delimiter_spans()) {
        (None, None) => return Ok(()),
        (Some(delimiter), Some(spans)) => (delimiter, spans),
        _ => {
            return Err(TokenPlanError::InvalidFenceDelimiter {
                fence_index: fence.index(),
            });
        }
    };
    let document_range = fence.document_range();
    let body_range = fence.body_range();
    if spans.opening.start < document_range.start
        || spans.opening.end > body_range.start
        || spans.opening.end.checked_sub(spans.opening.start) != Some(delimiter.marker_len())
    {
        return Err(TokenPlanError::InvalidFenceDelimiter {
            fence_index: fence.index(),
        });
    }
    let opening = ByteSpan {
        start: spans.opening.start,
        end: spans.opening.end,
    };
    validate_span(snapshot.text(), opening)?;
    if requested.is_none_or(|requested| requested.overlaps(opening)) {
        candidates.push(TokenCandidate {
            span: opening,
            kind: PlannedTokenKind::Delimiter,
            modifiers: Vec::new(),
            origin: TokenOverlayKind::Lexeme,
        });
    }

    match &spans.closing {
        Some(closing)
            if closing.start >= body_range.end
                && closing.end <= document_range.end
                && closing
                    .end
                    .checked_sub(closing.start)
                    .is_some_and(|len| len >= delimiter.marker_len()) =>
        {
            let closing = ByteSpan {
                start: closing.start,
                end: closing.end,
            };
            validate_span(snapshot.text(), closing)?;
            if requested.is_none_or(|requested| requested.overlaps(closing)) {
                candidates.push(TokenCandidate {
                    span: closing,
                    kind: PlannedTokenKind::Delimiter,
                    modifiers: Vec::new(),
                    origin: TokenOverlayKind::Lexeme,
                });
            }
        }
        None if body_range.end == document_range.end => {}
        _ => {
            return Err(TokenPlanError::InvalidFenceDelimiter {
                fence_index: fence.index(),
            });
        }
    }
    Ok(())
}

fn plan_candidates(
    source_map: &SourceMap,
    candidates: Vec<TokenCandidate>,
) -> Result<SemanticTokenPlan, TokenPlanError> {
    let mut candidates = validate_candidates(source_map.source(), candidates)?;
    validate_lexical_non_overlap(&candidates)?;
    candidates.sort_by_key(|candidate| {
        (
            candidate.span.start,
            candidate.span.end,
            candidate.origin.precedence(),
            candidate.kind.code(),
            candidate.modifier_bits,
        )
    });
    let byte_tokens = resolve_candidates(candidates)?;
    let mut tokens = split_and_encode_tokens(source_map, &byte_tokens)?;
    tokens.sort_by_key(|token| {
        (
            token.line,
            token.start,
            token.length,
            token.kind.code(),
            token.modifier_bits,
        )
    });
    validate_planned_non_overlap(&tokens)?;
    let packed = pack_tokens(&tokens);
    Ok(SemanticTokenPlan { tokens, packed })
}

fn validate_candidates(
    source: &str,
    candidates: Vec<TokenCandidate>,
) -> Result<Vec<ValidTokenCandidate>, TokenPlanError> {
    candidates
        .into_iter()
        .map(|candidate| {
            validate_span(source, candidate.span)?;
            let mut modifier_bits = 0u32;
            for modifier in candidate.modifiers {
                let bit = modifier.bit();
                if modifier_bits & bit != 0 {
                    return Err(TokenPlanError::DuplicateModifier {
                        span: candidate.span,
                        modifier,
                    });
                }
                modifier_bits |= bit;
            }
            Ok(ValidTokenCandidate {
                span: candidate.span,
                kind: candidate.kind,
                modifier_bits,
                origin: candidate.origin,
            })
        })
        .collect()
}

fn validate_span(source: &str, span: ByteSpan) -> Result<(), TokenPlanError> {
    if span.start >= span.end
        || span.end > source.len()
        || !source.is_char_boundary(span.start)
        || !source.is_char_boundary(span.end)
    {
        return Err(TokenPlanError::InvalidSpan {
            span,
            source_len: source.len(),
        });
    }
    Ok(())
}

fn validate_lexical_non_overlap(candidates: &[ValidTokenCandidate]) -> Result<(), TokenPlanError> {
    let mut lexical = candidates
        .iter()
        .filter(|candidate| candidate.origin == TokenOverlayKind::Lexeme)
        .collect::<Vec<_>>();
    lexical.sort_by_key(|candidate| (candidate.span.start, candidate.span.end));
    for pair in lexical.windows(2) {
        if pair[0].span.end > pair[1].span.start {
            return Err(TokenPlanError::OverlappingLexemes {
                left: pair[0].span,
                right: pair[1].span,
            });
        }
    }
    Ok(())
}

fn resolve_candidates(
    candidates: Vec<ValidTokenCandidate>,
) -> Result<Vec<PlannedByteToken>, TokenPlanError> {
    let mut boundaries = candidates
        .iter()
        .flat_map(|candidate| [candidate.span.start, candidate.span.end])
        .collect::<Vec<_>>();
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut next_candidate = 0usize;
    let mut active: Vec<usize> = Vec::new();
    let mut resolved: Vec<PlannedByteToken> = Vec::new();
    for boundary_pair in boundaries.windows(2) {
        let start = boundary_pair[0];
        let end = boundary_pair[1];
        active.retain(|index| candidates[*index].span.end > start);
        while next_candidate < candidates.len() && candidates[next_candidate].span.start <= start {
            if candidates[next_candidate].span.end > start {
                active.push(next_candidate);
            }
            next_candidate += 1;
        }
        if active.is_empty() || start == end {
            continue;
        }

        let winner = choose_candidate(&candidates, &active)?;
        let token = PlannedByteToken {
            span: ByteSpan { start, end },
            kind: winner.0,
            modifier_bits: winner.1,
        };
        if let Some(previous) = resolved.last_mut()
            && previous.span.end == token.span.start
            && previous.kind == token.kind
            && previous.modifier_bits == token.modifier_bits
        {
            previous.span.end = token.span.end;
        } else {
            resolved.push(token);
        }
    }
    Ok(resolved)
}

fn choose_candidate(
    candidates: &[ValidTokenCandidate],
    active: &[usize],
) -> Result<(PlannedTokenKind, u32), TokenPlanError> {
    let top_precedence = active
        .iter()
        .map(|index| candidates[*index].origin.precedence())
        .max()
        .expect("non-empty active token set");
    let narrowest = active
        .iter()
        .map(|index| candidates[*index])
        .filter(|candidate| candidate.origin.precedence() == top_precedence)
        .map(|candidate| candidate.span.end - candidate.span.start)
        .min()
        .expect("top-precedence token set");
    let finalists = active
        .iter()
        .map(|index| candidates[*index])
        .filter(|candidate| {
            candidate.origin.precedence() == top_precedence
                && candidate.span.end - candidate.span.start == narrowest
        })
        .collect::<Vec<_>>();
    let first = finalists[0];
    if let Some(conflict) = finalists
        .iter()
        .copied()
        .find(|candidate| candidate.kind != first.kind)
    {
        return Err(TokenPlanError::UnresolvedOverlap {
            left: first.span,
            left_kind: first.kind,
            right: conflict.span,
            right_kind: conflict.kind,
        });
    }
    let modifier_bits = active
        .iter()
        .fold(0u32, |bits, index| bits | candidates[*index].modifier_bits);
    Ok((first.kind, modifier_bits))
}

fn split_and_encode_tokens(
    source_map: &SourceMap,
    tokens: &[PlannedByteToken],
) -> Result<Vec<PlannedToken>, TokenPlanError> {
    let mut planned = Vec::new();
    for token in tokens {
        let start = source_map.utf16_position(token.span.start).map_err(|_| {
            TokenPlanError::InvalidSpan {
                span: token.span,
                source_len: source_map.source_len(),
            }
        })?;
        let end =
            source_map
                .utf16_position(token.span.end)
                .map_err(|_| TokenPlanError::InvalidSpan {
                    span: token.span,
                    source_len: source_map.source_len(),
                })?;
        for line in start.line..=end.line {
            let Some((line_start, line_end)) = source_map.line_bounds(line) else {
                continue;
            };
            let segment_start = token.span.start.max(line_start);
            let segment_end = token.span.end.min(line_end);
            if segment_start >= segment_end {
                continue;
            }
            let segment_start = source_map.utf16_position(segment_start).map_err(|_| {
                TokenPlanError::InvalidSpan {
                    span: token.span,
                    source_len: source_map.source_len(),
                }
            })?;
            let segment_end = source_map.utf16_position(segment_end).map_err(|_| {
                TokenPlanError::InvalidSpan {
                    span: token.span,
                    source_len: source_map.source_len(),
                }
            })?;
            let encoded_start = u32::try_from(segment_start.character).map_err(|_| {
                TokenPlanError::PositionOverflow {
                    value: segment_start.character,
                }
            })?;
            let encoded_end = u32::try_from(segment_end.character).map_err(|_| {
                TokenPlanError::PositionOverflow {
                    value: segment_end.character,
                }
            })?;
            let length = encoded_end - encoded_start;
            if length == 0 {
                continue;
            }
            planned.push(PlannedToken {
                line: u32::try_from(segment_start.line).map_err(|_| {
                    TokenPlanError::PositionOverflow {
                        value: segment_start.line,
                    }
                })?,
                start: encoded_start,
                length,
                kind: token.kind,
                modifier_bits: token.modifier_bits,
            });
        }
    }
    Ok(planned)
}

fn validate_planned_non_overlap(tokens: &[PlannedToken]) -> Result<(), TokenPlanError> {
    for pair in tokens.windows(2) {
        let left_end = u64::from(pair[0].start) + u64::from(pair[0].length);
        if pair[0].line == pair[1].line && left_end > u64::from(pair[1].start) {
            return Err(TokenPlanError::UnresolvedOverlap {
                left: ByteSpan {
                    start: pair[0].start as usize,
                    end: left_end as usize,
                },
                left_kind: pair[0].kind,
                right: ByteSpan {
                    start: pair[1].start as usize,
                    end: (pair[1].start + pair[1].length) as usize,
                },
                right_kind: pair[1].kind,
            });
        }
    }
    Ok(())
}

fn pack_tokens(tokens: &[PlannedToken]) -> Vec<u32> {
    let mut packed = Vec::with_capacity(tokens.len() * 5);
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    for token in tokens {
        let delta_line = token.line - previous_line;
        let delta_start = if delta_line == 0 {
            token.start - previous_start
        } else {
            token.start
        };
        packed.extend_from_slice(&[
            delta_line,
            delta_start,
            token.length,
            token.kind.code(),
            token.modifier_bits,
        ]);
        previous_line = token.line;
        previous_start = token.start;
    }
    packed
}

const fn planned_kind_for_lexeme(kind: FenceLexemeKind) -> PlannedTokenKind {
    match kind {
        FenceLexemeKind::Keyword => PlannedTokenKind::Keyword,
        FenceLexemeKind::Comment => PlannedTokenKind::Comment,
        FenceLexemeKind::Operator => PlannedTokenKind::Operator,
        FenceLexemeKind::Delimiter => PlannedTokenKind::Delimiter,
        FenceLexemeKind::Identifier => PlannedTokenKind::Identifier,
        FenceLexemeKind::Number => PlannedTokenKind::Number,
        FenceLexemeKind::Date => PlannedTokenKind::Date,
        FenceLexemeKind::Duration => PlannedTokenKind::Duration,
        FenceLexemeKind::Boolean => PlannedTokenKind::Boolean,
        FenceLexemeKind::String => PlannedTokenKind::String,
        FenceLexemeKind::Style => PlannedTokenKind::Style,
        FenceLexemeKind::Color => PlannedTokenKind::Color,
        FenceLexemeKind::Literal => PlannedTokenKind::Literal,
        FenceLexemeKind::Frontmatter => PlannedTokenKind::Frontmatter,
        FenceLexemeKind::Directive => PlannedTokenKind::Directive,
    }
}

const fn planned_modifier_for_lexeme(modifier: FenceLexemeModifier) -> PlannedTokenModifier {
    match modifier {
        FenceLexemeModifier::Declaration => PlannedTokenModifier::Declaration,
        FenceLexemeModifier::Definition => PlannedTokenModifier::Definition,
        FenceLexemeModifier::Reference => PlannedTokenModifier::Reference,
        FenceLexemeModifier::Readonly => PlannedTokenModifier::Readonly,
        FenceLexemeModifier::Documentation => PlannedTokenModifier::Documentation,
        FenceLexemeModifier::DefaultLibrary => PlannedTokenModifier::DefaultLibrary,
    }
}

const fn planned_kind_for_symbol(kind: EditorSymbolKind) -> PlannedTokenKind {
    match kind {
        EditorSymbolKind::Class => PlannedTokenKind::Class,
        EditorSymbolKind::Event => PlannedTokenKind::Event,
        EditorSymbolKind::Function => PlannedTokenKind::Function,
        EditorSymbolKind::Module | EditorSymbolKind::Namespace | EditorSymbolKind::Package => {
            PlannedTokenKind::Namespace
        }
        EditorSymbolKind::Object | EditorSymbolKind::Variable => PlannedTokenKind::Variable,
        EditorSymbolKind::Property => PlannedTokenKind::Property,
        EditorSymbolKind::String => PlannedTokenKind::String,
        EditorSymbolKind::Struct => PlannedTokenKind::Struct,
    }
}

const fn planned_modifier_for_role(role: FenceSemanticRole) -> PlannedTokenModifier {
    match role {
        FenceSemanticRole::Entity => PlannedTokenModifier::Entity,
        FenceSemanticRole::Outline => PlannedTokenModifier::Outline,
        FenceSemanticRole::Payload => PlannedTokenModifier::Payload,
    }
}

const fn token_overlay_for_role(role: FenceSemanticRole) -> TokenOverlayKind {
    match role {
        FenceSemanticRole::Entity => TokenOverlayKind::SemanticEntity,
        FenceSemanticRole::Outline => TokenOverlayKind::SemanticOutline,
        FenceSemanticRole::Payload => TokenOverlayKind::SemanticPayload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn candidate(
        span: std::ops::Range<usize>,
        kind: PlannedTokenKind,
        modifiers: Vec<PlannedTokenModifier>,
        origin: TokenOverlayKind,
    ) -> TokenCandidate {
        TokenCandidate {
            span: ByteSpan {
                start: span.start,
                end: span.end,
            },
            kind,
            modifiers,
            origin,
        }
    }

    #[test]
    fn descriptor_codes_and_lexeme_mapping_are_exact() {
        let mappings = [
            (FenceLexemeKind::Keyword, PlannedTokenKind::Keyword, 0),
            (FenceLexemeKind::Comment, PlannedTokenKind::Comment, 1),
            (FenceLexemeKind::Operator, PlannedTokenKind::Operator, 2),
            (FenceLexemeKind::Delimiter, PlannedTokenKind::Delimiter, 3),
            (FenceLexemeKind::Identifier, PlannedTokenKind::Identifier, 4),
            (FenceLexemeKind::Number, PlannedTokenKind::Number, 5),
            (FenceLexemeKind::Date, PlannedTokenKind::Date, 6),
            (FenceLexemeKind::Duration, PlannedTokenKind::Duration, 7),
            (FenceLexemeKind::Boolean, PlannedTokenKind::Boolean, 8),
            (FenceLexemeKind::String, PlannedTokenKind::String, 9),
            (FenceLexemeKind::Style, PlannedTokenKind::Style, 10),
            (FenceLexemeKind::Color, PlannedTokenKind::Color, 11),
            (FenceLexemeKind::Literal, PlannedTokenKind::Literal, 12),
            (
                FenceLexemeKind::Frontmatter,
                PlannedTokenKind::Frontmatter,
                13,
            ),
            (FenceLexemeKind::Directive, PlannedTokenKind::Directive, 14),
        ];
        for (lexeme, planned, code) in mappings {
            assert_eq!(planned_kind_for_lexeme(lexeme), planned);
            assert_eq!(planned.code(), code);
        }
        let semantic_mappings = [
            (EditorSymbolKind::Namespace, PlannedTokenKind::Namespace, 15),
            (EditorSymbolKind::Class, PlannedTokenKind::Class, 16),
            (EditorSymbolKind::Struct, PlannedTokenKind::Struct, 17),
            (EditorSymbolKind::Variable, PlannedTokenKind::Variable, 18),
            (EditorSymbolKind::Property, PlannedTokenKind::Property, 19),
            (EditorSymbolKind::Event, PlannedTokenKind::Event, 20),
            (EditorSymbolKind::Function, PlannedTokenKind::Function, 21),
        ];
        for (symbol, planned, code) in semantic_mappings {
            assert_eq!(planned_kind_for_symbol(symbol), planned);
            assert_eq!(planned.code(), code);
        }
        for symbol in [
            EditorSymbolKind::Module,
            EditorSymbolKind::Namespace,
            EditorSymbolKind::Package,
        ] {
            assert_eq!(planned_kind_for_symbol(symbol), PlannedTokenKind::Namespace);
        }
        for symbol in [EditorSymbolKind::Object, EditorSymbolKind::Variable] {
            assert_eq!(planned_kind_for_symbol(symbol), PlannedTokenKind::Variable);
        }
        assert_eq!(
            planned_kind_for_symbol(EditorSymbolKind::String),
            PlannedTokenKind::String
        );
        for (index, modifier) in [
            PlannedTokenModifier::Declaration,
            PlannedTokenModifier::Definition,
            PlannedTokenModifier::Reference,
            PlannedTokenModifier::Readonly,
            PlannedTokenModifier::Documentation,
            PlannedTokenModifier::DefaultLibrary,
            PlannedTokenModifier::Entity,
            PlannedTokenModifier::Outline,
            PlannedTokenModifier::Payload,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(modifier.index(), index as u32);
        }
        for (lexeme, planned) in [
            (
                FenceLexemeModifier::Declaration,
                PlannedTokenModifier::Declaration,
            ),
            (
                FenceLexemeModifier::Definition,
                PlannedTokenModifier::Definition,
            ),
            (
                FenceLexemeModifier::Reference,
                PlannedTokenModifier::Reference,
            ),
            (
                FenceLexemeModifier::Readonly,
                PlannedTokenModifier::Readonly,
            ),
            (
                FenceLexemeModifier::Documentation,
                PlannedTokenModifier::Documentation,
            ),
            (
                FenceLexemeModifier::DefaultLibrary,
                PlannedTokenModifier::DefaultLibrary,
            ),
        ] {
            assert_eq!(planned_modifier_for_lexeme(lexeme), planned);
        }
    }

    #[test]
    fn upstream_lexeme_failure_is_monotonic_and_blocks_planning() {
        let failure = FenceLexemeFailure::InvalidProvenance;
        assert_eq!(
            reject_upstream_lexeme_failure(7, Some(failure)),
            Err(TokenPlanError::UpstreamLexemeFailure {
                fence_index: 7,
                failure,
            })
        );
    }

    #[test]
    fn semantic_overlay_precedence_replaces_kind_and_preserves_modifiers() {
        let source_map = SourceMap::new(Arc::<str>::from("alpha"));
        let plan = plan_candidates(
            &source_map,
            vec![
                candidate(
                    0..5,
                    PlannedTokenKind::Identifier,
                    vec![PlannedTokenModifier::Reference],
                    TokenOverlayKind::Lexeme,
                ),
                candidate(
                    0..5,
                    PlannedTokenKind::Variable,
                    vec![PlannedTokenModifier::Entity],
                    TokenOverlayKind::SemanticEntity,
                ),
            ],
        )
        .expect("semantic overlay plan");

        assert_eq!(plan.tokens.len(), 1);
        assert_eq!(plan.tokens[0].kind, PlannedTokenKind::Variable);
        assert!(plan.tokens[0].has_modifier(PlannedTokenModifier::Reference));
        assert!(plan.tokens[0].has_modifier(PlannedTokenModifier::Entity));
    }

    #[test]
    fn semantic_overlay_subtracts_from_quoted_lexeme_without_losing_sides() {
        let source_map = SourceMap::new(Arc::<str>::from("\"alpha\""));
        let plan = plan_candidates(
            &source_map,
            vec![
                candidate(
                    0..7,
                    PlannedTokenKind::String,
                    Vec::new(),
                    TokenOverlayKind::Lexeme,
                ),
                candidate(
                    1..6,
                    PlannedTokenKind::Variable,
                    vec![PlannedTokenModifier::Entity],
                    TokenOverlayKind::SemanticEntity,
                ),
            ],
        )
        .expect("quoted overlay plan");

        assert_eq!(
            plan.tokens
                .iter()
                .map(|token| (token.start, token.length, token.kind))
                .collect::<Vec<_>>(),
            vec![
                (0, 1, PlannedTokenKind::String),
                (1, 5, PlannedTokenKind::Variable),
                (6, 1, PlannedTokenKind::String),
            ]
        );
    }

    #[test]
    fn semantic_overlay_subtracts_inside_multiline_lexeme_before_line_splitting() {
        let source = "left\nmiddle\nright";
        let source_map = SourceMap::new(Arc::<str>::from(source));
        let middle_start = source.find("middle").expect("middle start");
        let plan = plan_candidates(
            &source_map,
            vec![
                candidate(
                    0..source.len(),
                    PlannedTokenKind::String,
                    Vec::new(),
                    TokenOverlayKind::Lexeme,
                ),
                candidate(
                    middle_start..middle_start + "middle".len(),
                    PlannedTokenKind::Property,
                    vec![PlannedTokenModifier::Payload],
                    TokenOverlayKind::SemanticPayload,
                ),
            ],
        )
        .expect("multiline overlay plan");

        assert_eq!(
            plan.tokens
                .iter()
                .map(|token| (token.line, token.start, token.length, token.kind))
                .collect::<Vec<_>>(),
            vec![
                (0, 0, 4, PlannedTokenKind::String),
                (1, 0, 6, PlannedTokenKind::Property),
                (2, 0, 5, PlannedTokenKind::String),
            ]
        );
    }

    #[test]
    fn two_disjoint_semantic_overlays_partition_one_lexeme() {
        let source_map = SourceMap::new(Arc::<str>::from("[alpha beta]"));
        let plan = plan_candidates(
            &source_map,
            vec![
                candidate(
                    0..12,
                    PlannedTokenKind::String,
                    Vec::new(),
                    TokenOverlayKind::Lexeme,
                ),
                candidate(
                    1..6,
                    PlannedTokenKind::Variable,
                    Vec::new(),
                    TokenOverlayKind::SemanticEntity,
                ),
                candidate(
                    7..11,
                    PlannedTokenKind::Variable,
                    Vec::new(),
                    TokenOverlayKind::SemanticEntity,
                ),
            ],
        )
        .expect("two-overlay plan");

        assert_eq!(
            plan.tokens
                .iter()
                .map(|token| (token.start, token.length, token.kind))
                .collect::<Vec<_>>(),
            vec![
                (0, 1, PlannedTokenKind::String),
                (1, 5, PlannedTokenKind::Variable),
                (6, 1, PlannedTokenKind::String),
                (7, 4, PlannedTokenKind::Variable),
                (11, 1, PlannedTokenKind::String),
            ]
        );
    }

    #[test]
    fn planner_splits_lines_counts_utf16_and_packs_deltas() {
        let source = "pre 🤓value\nnext";
        let source_map = SourceMap::new(Arc::<str>::from(source));
        let start = source.find('🤓').expect("emoji start");
        let plan = plan_candidates(
            &source_map,
            vec![candidate(
                start..source.len(),
                PlannedTokenKind::String,
                Vec::new(),
                TokenOverlayKind::Lexeme,
            )],
        )
        .expect("multiline token plan");

        assert_eq!(
            plan.tokens,
            vec![
                PlannedToken {
                    line: 0,
                    start: 4,
                    length: 7,
                    kind: PlannedTokenKind::String,
                    modifier_bits: 0,
                },
                PlannedToken {
                    line: 1,
                    start: 0,
                    length: 4,
                    kind: PlannedTokenKind::String,
                    modifier_bits: 0,
                },
            ]
        );
        assert_eq!(plan.packed, vec![0, 4, 7, 9, 0, 1, 0, 4, 9, 0]);
    }

    #[test]
    fn planner_rejects_overlaps_invalid_boundaries_and_duplicate_modifiers() {
        let ascii = SourceMap::new(Arc::<str>::from("abcdef"));
        let overlap = plan_candidates(
            &ascii,
            vec![
                candidate(
                    0..4,
                    PlannedTokenKind::Keyword,
                    Vec::new(),
                    TokenOverlayKind::Lexeme,
                ),
                candidate(
                    3..6,
                    PlannedTokenKind::Identifier,
                    Vec::new(),
                    TokenOverlayKind::Lexeme,
                ),
            ],
        );
        assert!(matches!(
            overlap,
            Err(TokenPlanError::OverlappingLexemes { .. })
        ));

        let unicode = SourceMap::new(Arc::<str>::from("🤓"));
        let invalid = plan_candidates(
            &unicode,
            vec![candidate(
                1..4,
                PlannedTokenKind::String,
                Vec::new(),
                TokenOverlayKind::Lexeme,
            )],
        );
        assert!(matches!(invalid, Err(TokenPlanError::InvalidSpan { .. })));

        let duplicate = plan_candidates(
            &ascii,
            vec![candidate(
                0..1,
                PlannedTokenKind::Identifier,
                vec![
                    PlannedTokenModifier::Reference,
                    PlannedTokenModifier::Reference,
                ],
                TokenOverlayKind::Lexeme,
            )],
        );
        assert!(matches!(
            duplicate,
            Err(TokenPlanError::DuplicateModifier { .. })
        ));
    }

    #[test]
    fn equal_precedence_semantic_conflicts_fail_closed() {
        let source_map = SourceMap::new(Arc::<str>::from("alpha"));
        let result = plan_candidates(
            &source_map,
            vec![
                candidate(
                    0..5,
                    PlannedTokenKind::Variable,
                    Vec::new(),
                    TokenOverlayKind::SemanticEntity,
                ),
                candidate(
                    0..5,
                    PlannedTokenKind::Class,
                    Vec::new(),
                    TokenOverlayKind::SemanticEntity,
                ),
            ],
        );
        assert!(matches!(
            result,
            Err(TokenPlanError::UnresolvedOverlap { .. })
        ));
    }
}
