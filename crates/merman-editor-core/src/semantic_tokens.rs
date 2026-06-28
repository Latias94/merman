use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::{
    ByteSpan, EditorSymbolKind, FenceSemanticItem, FenceSemanticRole, SourceMap,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticToken {
    pub line: u32,
    pub start: u32,
    pub length: u32,
    pub kind: SemanticTokenKind,
    pub modifier: SemanticTokenModifier,
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
    let mut tokens = Vec::new();

    for fence in &snapshot.fences {
        for item in fence.text_index.semantic_items() {
            tokens.extend(tokens_for_item(&snapshot.source_map, fence, item));
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

fn tokens_for_item(
    source_map: &SourceMap,
    fence: &FenceSnapshot,
    item: &FenceSemanticItem,
) -> Vec<SemanticToken> {
    let span = ByteSpan {
        start: fence.body_start + item.selection.start,
        end: fence.body_start + item.selection.end,
    };
    if span.start >= span.end {
        return Vec::new();
    }

    token_pieces_for_span(source_map, span)
        .into_iter()
        .map(|piece| SemanticToken {
            line: piece.line,
            start: piece.start,
            length: piece.length,
            kind: token_kind_for_symbol(item.kind),
            modifier: token_modifier_for_role(item.role),
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
