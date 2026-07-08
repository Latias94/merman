use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::{
    ByteSpan, EditorSymbolKind, FenceSemanticItem, FenceSemanticRole, FenceTextIndexSource,
    SourceMap,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticToken {
    pub line: u32,
    pub start: u32,
    pub length: u32,
    pub kind: SemanticTokenKind,
    pub modifier: SemanticTokenModifier,
    pub fact_source: FenceTextIndexSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenKind {
    Namespace,
    Class,
    Struct,
    Variable,
    Property,
    Event,
    Function,
    String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenModifier {
    Entity,
    Outline,
    Payload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTokenLegend {
    pub token_types: Vec<SemanticTokenKind>,
    pub token_modifiers: Vec<SemanticTokenModifier>,
}

pub fn semantic_token_legend() -> SemanticTokenLegend {
    SemanticTokenLegend {
        token_types: vec![
            SemanticTokenKind::Namespace,
            SemanticTokenKind::Class,
            SemanticTokenKind::Struct,
            SemanticTokenKind::Variable,
            SemanticTokenKind::Property,
            SemanticTokenKind::Event,
            SemanticTokenKind::Function,
            SemanticTokenKind::String,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::Entity,
            SemanticTokenModifier::Outline,
            SemanticTokenModifier::Payload,
        ],
    }
}

pub fn semantic_tokens_for_snapshot(snapshot: &DocumentSnapshot) -> Vec<SemanticToken> {
    semantic_tokens_for_fences(&snapshot.source_map, &snapshot.fences, None)
}

pub fn semantic_tokens_for_snapshot_range(
    snapshot: &DocumentSnapshot,
    start_line: u32,
    end_line: u32,
) -> Vec<SemanticToken> {
    let Some(line_range) = RequestedLineRange::new(&snapshot.source_map, start_line, end_line)
    else {
        return Vec::new();
    };
    semantic_tokens_for_fences(&snapshot.source_map, &snapshot.fences, Some(line_range))
}

fn semantic_tokens_for_fences<'a>(
    source_map: &SourceMap,
    fences: impl IntoIterator<Item = &'a FenceSnapshot>,
    line_range: Option<RequestedLineRange>,
) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();

    for fence in fences {
        if line_range.is_some_and(|range| !fence_overlaps_line_range(source_map, fence, range)) {
            continue;
        }
        for item in fence.text_index.semantic_items() {
            let Some(span) = absolute_span_for_item(fence, item) else {
                continue;
            };
            if line_range.is_some_and(|range| !range.overlaps_byte_span(span)) {
                continue;
            }
            tokens.extend(tokens_for_item(source_map, fence, item, span, line_range));
        }
    }

    tokens.sort_by(|left, right| {
        (
            left.line,
            left.start,
            left.length,
            token_type_index(left.kind),
            token_modifier_index(left.modifier),
        )
            .cmp(&(
                right.line,
                right.start,
                right.length,
                token_type_index(right.kind),
                token_modifier_index(right.modifier),
            ))
    });
    tokens.dedup();
    tokens
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestedLineRange {
    start_line: u32,
    end_line: u32,
    byte_span: ByteSpan,
}

impl RequestedLineRange {
    fn new(source_map: &SourceMap, start_line: u32, end_line: u32) -> Option<Self> {
        let requested_start_line = start_line.min(end_line);
        let requested_end_line = start_line.max(end_line);
        let line_count = source_map.line_starts().len();
        if line_count == 0 || requested_start_line as usize >= line_count {
            return None;
        }

        let end_line = requested_end_line.min((line_count - 1) as u32);
        let (byte_start, _) = source_map.line_bounds(requested_start_line as usize)?;
        let (_, byte_end) = source_map.line_bounds(end_line as usize)?;
        Some(Self {
            start_line: requested_start_line,
            end_line,
            byte_span: ByteSpan {
                start: byte_start,
                end: byte_end,
            },
        })
    }

    fn contains_line(self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    fn overlaps_byte_span(self, span: ByteSpan) -> bool {
        span.start < self.byte_span.end && span.end > self.byte_span.start
    }
}

fn fence_overlaps_line_range(
    source_map: &SourceMap,
    fence: &FenceSnapshot,
    line_range: RequestedLineRange,
) -> bool {
    let Ok(fence_start) = source_map.utf16_position(fence.body_start) else {
        return true;
    };
    let Ok(fence_end) = source_map.utf16_position(fence.body_end) else {
        return true;
    };

    (fence_start.line as u32) <= line_range.end_line
        && (fence_end.line as u32) >= line_range.start_line
}

fn absolute_span_for_item(fence: &FenceSnapshot, item: &FenceSemanticItem) -> Option<ByteSpan> {
    let span = ByteSpan {
        start: fence.body_start + item.selection.start,
        end: fence.body_start + item.selection.end,
    };
    if span.start >= span.end {
        None
    } else {
        Some(span)
    }
}

fn tokens_for_item(
    source_map: &SourceMap,
    fence: &FenceSnapshot,
    item: &FenceSemanticItem,
    span: ByteSpan,
    line_range: Option<RequestedLineRange>,
) -> Vec<SemanticToken> {
    token_pieces_for_span(source_map, span)
        .into_iter()
        .filter(|piece| line_range.is_none_or(|range| range.contains_line(piece.line)))
        .map(|piece| SemanticToken {
            line: piece.line,
            start: piece.start,
            length: piece.length,
            kind: token_kind_for_symbol(item.kind),
            modifier: token_modifier_for_role(item.role),
            fact_source: fence.text_index.source(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TokenPiece {
    line: u32,
    start: u32,
    length: u32,
}

fn token_pieces_for_span(source_map: &SourceMap, span: ByteSpan) -> Vec<TokenPiece> {
    let Ok(start) = source_map.utf16_position(span.start) else {
        return Vec::new();
    };
    let Ok(end) = source_map.utf16_position(span.end) else {
        return Vec::new();
    };

    let mut pieces = Vec::new();
    for line in start.line..=end.line {
        let Some((line_start, line_end)) = source_map.line_bounds(line) else {
            continue;
        };
        let segment_start = span.start.max(line_start);
        let segment_end = span.end.min(line_end);
        if segment_start >= segment_end {
            continue;
        }

        let Ok(segment_start_pos) = source_map.utf16_position(segment_start) else {
            continue;
        };
        let Ok(segment_end_pos) = source_map.utf16_position(segment_end) else {
            continue;
        };
        if segment_start_pos.line != segment_end_pos.line
            || segment_end_pos.character <= segment_start_pos.character
        {
            continue;
        }

        pieces.push(TokenPiece {
            line: segment_start_pos.line as u32,
            start: segment_start_pos.character as u32,
            length: (segment_end_pos.character - segment_start_pos.character) as u32,
        });
    }

    pieces
}

fn token_kind_for_symbol(kind: EditorSymbolKind) -> SemanticTokenKind {
    match kind {
        EditorSymbolKind::Class => SemanticTokenKind::Class,
        EditorSymbolKind::Event => SemanticTokenKind::Event,
        EditorSymbolKind::Function => SemanticTokenKind::Function,
        EditorSymbolKind::Module | EditorSymbolKind::Namespace | EditorSymbolKind::Package => {
            SemanticTokenKind::Namespace
        }
        EditorSymbolKind::Object | EditorSymbolKind::Variable => SemanticTokenKind::Variable,
        EditorSymbolKind::Property => SemanticTokenKind::Property,
        EditorSymbolKind::String => SemanticTokenKind::String,
        EditorSymbolKind::Struct => SemanticTokenKind::Struct,
    }
}

fn token_modifier_for_role(role: FenceSemanticRole) -> SemanticTokenModifier {
    match role {
        FenceSemanticRole::Entity => SemanticTokenModifier::Entity,
        FenceSemanticRole::Outline => SemanticTokenModifier::Outline,
        FenceSemanticRole::Payload => SemanticTokenModifier::Payload,
    }
}

pub fn token_type_index(kind: SemanticTokenKind) -> u32 {
    match kind {
        SemanticTokenKind::Namespace => 0,
        SemanticTokenKind::Class => 1,
        SemanticTokenKind::Struct => 2,
        SemanticTokenKind::Variable => 3,
        SemanticTokenKind::Property => 4,
        SemanticTokenKind::Event => 5,
        SemanticTokenKind::Function => 6,
        SemanticTokenKind::String => 7,
    }
}

pub fn token_modifier_index(modifier: SemanticTokenModifier) -> u32 {
    match modifier {
        SemanticTokenModifier::Entity => 0,
        SemanticTokenModifier::Outline => 1,
        SemanticTokenModifier::Payload => 2,
    }
}
