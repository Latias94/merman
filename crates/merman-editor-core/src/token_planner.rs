use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::{
    ByteSpan, EditorSymbolKind, FenceLexemeFailure, FenceLexemeKind, FenceLexemeModifier,
    FenceSemanticRole, SourceMap,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlannedTokenKind {
    Keyword,
    Comment,
    Operator,
    Delimiter,
    Identifier,
    Number,
    Date,
    Duration,
    Boolean,
    String,
    Style,
    Color,
    Literal,
    Frontmatter,
    Directive,
    Namespace,
    Class,
    Struct,
    Variable,
    Property,
    Event,
    Function,
}

impl PlannedTokenKind {
    pub const fn code(self) -> u32 {
        match self {
            Self::Keyword => 0,
            Self::Comment => 1,
            Self::Operator => 2,
            Self::Delimiter => 3,
            Self::Identifier => 4,
            Self::Number => 5,
            Self::Date => 6,
            Self::Duration => 7,
            Self::Boolean => 8,
            Self::String => 9,
            Self::Style => 10,
            Self::Color => 11,
            Self::Literal => 12,
            Self::Frontmatter => 13,
            Self::Directive => 14,
            Self::Namespace => 15,
            Self::Class => 16,
            Self::Struct => 17,
            Self::Variable => 18,
            Self::Property => 19,
            Self::Event => 20,
            Self::Function => 21,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlannedTokenModifier {
    Declaration,
    Definition,
    Reference,
    Readonly,
    Documentation,
    DefaultLibrary,
    Entity,
    Outline,
    Payload,
}

impl PlannedTokenModifier {
    pub const fn index(self) -> u32 {
        match self {
            Self::Declaration => 0,
            Self::Definition => 1,
            Self::Reference => 2,
            Self::Readonly => 3,
            Self::Documentation => 4,
            Self::DefaultLibrary => 5,
            Self::Entity => 6,
            Self::Outline => 7,
            Self::Payload => 8,
        }
    }

    pub const fn bit(self) -> u32 {
        1 << self.index()
    }
}

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

pub const fn planned_token_type_index(kind: PlannedTokenKind) -> u32 {
    kind.code()
}

pub const fn planned_token_modifier_index(modifier: PlannedTokenModifier) -> u32 {
    modifier.index()
}

pub fn plan_semantic_tokens_for_snapshot(
    snapshot: &DocumentSnapshot,
) -> Result<SemanticTokenPlan, TokenPlanError> {
    let mut candidates = Vec::new();
    for fence in &snapshot.fences {
        collect_fence_candidates(snapshot, fence, &mut candidates)?;
    }
    plan_candidates(&snapshot.source_map, candidates)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateOrigin {
    Lexeme,
    Semantic(FenceSemanticRole),
}

impl CandidateOrigin {
    const fn precedence(self) -> u8 {
        match self {
            Self::Lexeme => 0,
            Self::Semantic(FenceSemanticRole::Payload) => 1,
            Self::Semantic(FenceSemanticRole::Outline) => 2,
            Self::Semantic(FenceSemanticRole::Entity) => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenCandidate {
    span: ByteSpan,
    kind: PlannedTokenKind,
    modifiers: Vec<PlannedTokenModifier>,
    origin: CandidateOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidTokenCandidate {
    span: ByteSpan,
    kind: PlannedTokenKind,
    modifier_bits: u32,
    origin: CandidateOrigin,
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
    candidates: &mut Vec<TokenCandidate>,
) -> Result<(), TokenPlanError> {
    reject_upstream_lexeme_failure(fence.index, fence.text_index.lexeme_failure())?;

    validate_lexeme_order(fence)?;
    for lexeme in fence.text_index.lexemes() {
        candidates.push(TokenCandidate {
            span: absolute_fence_span(snapshot, fence, lexeme.span)?,
            kind: planned_kind_for_lexeme(lexeme.kind),
            modifiers: lexeme
                .modifiers
                .iter()
                .copied()
                .map(planned_modifier_for_lexeme)
                .collect(),
            origin: CandidateOrigin::Lexeme,
        });
    }

    for item in fence.text_index.semantic_items() {
        candidates.push(TokenCandidate {
            span: absolute_fence_span(snapshot, fence, item.selection)?,
            kind: planned_kind_for_symbol(item.kind),
            modifiers: vec![planned_modifier_for_role(item.role)],
            origin: CandidateOrigin::Semantic(item.role),
        });
    }

    collect_fence_delimiters(snapshot, fence, candidates)
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

fn validate_lexeme_order(fence: &FenceSnapshot) -> Result<(), TokenPlanError> {
    let mut previous: Option<ByteSpan> = None;
    for lexeme in fence.text_index.lexemes() {
        if let Some(previous_span) = previous {
            if (previous_span.start, previous_span.end) > (lexeme.span.start, lexeme.span.end) {
                return Err(TokenPlanError::UnsortedLexemes {
                    previous: previous_span,
                    current: lexeme.span,
                });
            }
            if previous_span.end > lexeme.span.start {
                return Err(TokenPlanError::OverlappingLexemes {
                    left: previous_span,
                    right: lexeme.span,
                });
            }
        }
        previous = Some(lexeme.span);
    }
    Ok(())
}

fn absolute_fence_span(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    relative: ByteSpan,
) -> Result<ByteSpan, TokenPlanError> {
    let body = fence.text.as_str();
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
    let start =
        fence
            .body_start
            .checked_add(relative.start)
            .ok_or(TokenPlanError::PositionOverflow {
                value: relative.start,
            })?;
    let end =
        fence
            .body_start
            .checked_add(relative.end)
            .ok_or(TokenPlanError::PositionOverflow {
                value: relative.end,
            })?;
    let span = ByteSpan { start, end };
    validate_span(snapshot.source_map.source(), span)?;
    Ok(span)
}

fn collect_fence_delimiters(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    candidates: &mut Vec<TokenCandidate>,
) -> Result<(), TokenPlanError> {
    let (delimiter, spans) = match (fence.fence_delimiter, &fence.fence_delimiter_spans) {
        (None, None) => return Ok(()),
        (Some(delimiter), Some(spans)) => (delimiter, spans),
        _ => {
            return Err(TokenPlanError::InvalidFenceDelimiter {
                fence_index: fence.index,
            });
        }
    };
    if spans.opening.start < fence.start
        || spans.opening.end > fence.body_start
        || spans.opening.end.checked_sub(spans.opening.start) != Some(delimiter.marker_len())
    {
        return Err(TokenPlanError::InvalidFenceDelimiter {
            fence_index: fence.index,
        });
    }
    let opening = ByteSpan {
        start: spans.opening.start,
        end: spans.opening.end,
    };
    validate_span(snapshot.text.as_ref(), opening)?;
    candidates.push(TokenCandidate {
        span: opening,
        kind: PlannedTokenKind::Delimiter,
        modifiers: Vec::new(),
        origin: CandidateOrigin::Lexeme,
    });

    match &spans.closing {
        Some(closing)
            if closing.start >= fence.body_end
                && closing.end <= fence.end
                && closing
                    .end
                    .checked_sub(closing.start)
                    .is_some_and(|len| len >= delimiter.marker_len()) =>
        {
            let closing = ByteSpan {
                start: closing.start,
                end: closing.end,
            };
            validate_span(snapshot.text.as_ref(), closing)?;
            candidates.push(TokenCandidate {
                span: closing,
                kind: PlannedTokenKind::Delimiter,
                modifiers: Vec::new(),
                origin: CandidateOrigin::Lexeme,
            });
        }
        None if fence.body_end == fence.end => {}
        _ => {
            return Err(TokenPlanError::InvalidFenceDelimiter {
                fence_index: fence.index,
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
        .filter(|candidate| candidate.origin == CandidateOrigin::Lexeme)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn candidate(
        span: std::ops::Range<usize>,
        kind: PlannedTokenKind,
        modifiers: Vec<PlannedTokenModifier>,
        origin: CandidateOrigin,
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
                    CandidateOrigin::Lexeme,
                ),
                candidate(
                    0..5,
                    PlannedTokenKind::Variable,
                    vec![PlannedTokenModifier::Entity],
                    CandidateOrigin::Semantic(FenceSemanticRole::Entity),
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
                    CandidateOrigin::Lexeme,
                ),
                candidate(
                    1..6,
                    PlannedTokenKind::Variable,
                    vec![PlannedTokenModifier::Entity],
                    CandidateOrigin::Semantic(FenceSemanticRole::Entity),
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
                    CandidateOrigin::Lexeme,
                ),
                candidate(
                    middle_start..middle_start + "middle".len(),
                    PlannedTokenKind::Property,
                    vec![PlannedTokenModifier::Payload],
                    CandidateOrigin::Semantic(FenceSemanticRole::Payload),
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
                    CandidateOrigin::Lexeme,
                ),
                candidate(
                    1..6,
                    PlannedTokenKind::Variable,
                    Vec::new(),
                    CandidateOrigin::Semantic(FenceSemanticRole::Entity),
                ),
                candidate(
                    7..11,
                    PlannedTokenKind::Variable,
                    Vec::new(),
                    CandidateOrigin::Semantic(FenceSemanticRole::Entity),
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
                CandidateOrigin::Lexeme,
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
                    CandidateOrigin::Lexeme,
                ),
                candidate(
                    3..6,
                    PlannedTokenKind::Identifier,
                    Vec::new(),
                    CandidateOrigin::Lexeme,
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
                CandidateOrigin::Lexeme,
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
                CandidateOrigin::Lexeme,
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
                    CandidateOrigin::Semantic(FenceSemanticRole::Entity),
                ),
                candidate(
                    0..5,
                    PlannedTokenKind::Class,
                    Vec::new(),
                    CandidateOrigin::Semantic(FenceSemanticRole::Entity),
                ),
            ],
        );
        assert!(matches!(
            result,
            Err(TokenPlanError::UnresolvedOverlap { .. })
        ));
    }
}
