use crate::types::{Position, Range};
use merman_analysis::{DiagnosticFix, DiagnosticFixEdit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCodeAction {
    pub title: String,
    pub edits: Vec<EditorCodeActionEdit>,
    pub is_preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCodeActionEdit {
    pub range: Range,
    pub new_text: String,
}

pub fn code_actions_from_fixes<'a>(
    fixes: impl IntoIterator<Item = &'a DiagnosticFix>,
) -> Vec<EditorCodeAction> {
    fixes.into_iter().filter_map(code_action_from_fix).collect()
}

pub fn code_action_from_fix(fix: &DiagnosticFix) -> Option<EditorCodeAction> {
    let mut edits = fix
        .edits
        .iter()
        .map(code_action_edit_from_fix_edit)
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return None;
    }

    edits.sort_by_key(|edit| range_sort_key(edit.range));
    if has_overlapping_edits(&edits) {
        return None;
    }

    Some(EditorCodeAction {
        title: fix.title.clone(),
        edits,
        is_preferred: fix.is_preferred,
    })
}

fn code_action_edit_from_fix_edit(edit: &DiagnosticFixEdit) -> EditorCodeActionEdit {
    EditorCodeActionEdit {
        range: Range::new(
            Position::new(
                edit.span.lsp_range.start.line,
                edit.span.lsp_range.start.character,
            ),
            Position::new(
                edit.span.lsp_range.end.line,
                edit.span.lsp_range.end.character,
            ),
        ),
        new_text: edit.replacement.clone(),
    }
}

fn has_overlapping_edits(edits: &[EditorCodeActionEdit]) -> bool {
    edits.windows(2).any(|window| {
        let [left, right] = window else {
            return false;
        };
        position_key(left.range.end) > position_key(right.range.start)
    })
}

fn range_sort_key(range: Range) -> (usize, usize, usize, usize) {
    (
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character,
    )
}

fn position_key(position: Position) -> (usize, usize) {
    (position.line, position.character)
}
