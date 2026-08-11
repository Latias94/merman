use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{Position, Range};
use merman_analysis::FenceTextIndexSource;
use merman_core::{EditorExpectedSyntax, EditorExpectedSyntaxKind};

#[derive(Debug)]
pub(crate) struct CompletionQuery<'a> {
    pub(crate) snapshot: &'a DocumentSnapshot,
    pub(crate) fence: &'a FenceSnapshot,
    pub(crate) prefix: &'a str,
    pub(crate) prefix_start_offset: usize,
    pub(crate) cursor_offset: usize,
    pub(crate) source: FenceTextIndexSource,
    pub(crate) source_start: bool,
    pub(crate) in_directive: bool,
    pub(crate) expected_syntax: Option<EditorExpectedSyntax>,
}

impl<'a> CompletionQuery<'a> {
    pub(crate) fn from_snapshot(
        snapshot: &'a DocumentSnapshot,
        position: Position,
    ) -> Option<Self> {
        let fence = snapshot.fence_at_position(position)?;
        let cursor_offset = snapshot.byte_offset_for_position(position)?;
        let body_range = fence.body_range();
        let document_range = fence.document_range();
        if cursor_offset < body_range.start
            || cursor_offset > body_range.end
            || (cursor_offset == body_range.end && document_range.end > body_range.end)
        {
            return None;
        }
        let relative_cursor = cursor_offset
            .saturating_sub(body_range.start)
            .min(fence.text().len());
        let cursor = fence
            .text()
            .floor_char_boundary(relative_cursor.min(fence.text().len()));
        let (prefix_start, prefix) = current_line_prefix(fence.text(), cursor);
        let prefix_start_offset = body_range.start + prefix_start;
        let cursor_offset = body_range.start + cursor;
        let (expected_syntax, in_directive) = completion_evidence_at_cursor(
            fence.text_index().expected_syntax(),
            fence.text(),
            cursor,
        );

        Some(Self {
            snapshot,
            fence,
            prefix,
            prefix_start_offset,
            cursor_offset,
            source: fence.text_index().source(),
            source_start: is_source_start_context(fence.text(), prefix_start),
            in_directive,
            expected_syntax,
        })
    }

    pub(crate) fn document_uri(&self) -> &str {
        self.snapshot.uri().as_str()
    }

    pub(crate) fn prefix_range(&self) -> Option<Range> {
        self.range_for_offsets(self.prefix_start_offset, self.cursor_offset)
    }

    pub(crate) fn direction_value_range(&self) -> Option<Range> {
        self.expected_range(EditorExpectedSyntaxKind::DirectionValue)
    }

    pub(crate) fn operator_range(&self) -> Option<Range> {
        self.expected_range(EditorExpectedSyntaxKind::Operator)
    }

    pub(crate) fn shape_value_edit(&self, value: &str) -> Option<CompletionTextEditParts> {
        let (range, has_separator_space, append_closing_brace) = self.shape_value_edit_parts()?;
        let replacement = if append_closing_brace {
            if has_separator_space {
                format!("{value} }}")
            } else {
                format!(" {value} }}")
            }
        } else if has_separator_space {
            value.to_string()
        } else {
            format!(" {value}")
        };

        Some(CompletionTextEditParts { range, replacement })
    }

    pub(crate) fn shape_trigger_range(&self) -> Option<Range> {
        self.expected_range(EditorExpectedSyntaxKind::ShapeTrigger)
    }

    pub(crate) fn expected_node_range(&self) -> Option<Range> {
        matches!(
            self.expected_syntax.map(|expected| expected.kind),
            Some(EditorExpectedSyntaxKind::NodeIdentifier | EditorExpectedSyntaxKind::IdList)
        )
        .then(|| self.expected_syntax_range())
        .flatten()
    }

    pub(crate) fn class_name_range(&self) -> Option<Range> {
        self.expected_range(EditorExpectedSyntaxKind::ClassName)
    }

    pub(crate) fn style_range(&self) -> Option<Range> {
        self.expected_range(EditorExpectedSyntaxKind::StyleValue)
    }

    pub(crate) fn interaction_range(&self) -> Option<Range> {
        self.expected_range(EditorExpectedSyntaxKind::InteractionAction)
    }

    fn expected_range(&self, kind: EditorExpectedSyntaxKind) -> Option<Range> {
        (self.expected_syntax.map(|expected| expected.kind) == Some(kind))
            .then(|| self.expected_syntax_range())
            .flatten()
    }

    fn expected_syntax_range(&self) -> Option<Range> {
        let expected = self.expected_syntax?;
        let body_start = self.fence.body_range().start;
        self.range_for_offsets(
            body_start + expected.span.start,
            body_start + expected.span.end,
        )
    }

    fn range_for_offsets(&self, start: usize, end: usize) -> Option<Range> {
        let span = self.snapshot.source_map().span(start, end).ok()?;
        Some(Range {
            start: Position {
                line: span.lsp_range.start.line,
                character: span.lsp_range.start.character,
            },
            end: Position {
                line: span.lsp_range.end.line,
                character: span.lsp_range.end.character,
            },
        })
    }

    fn shape_value_edit_parts(&self) -> Option<(Range, bool, bool)> {
        (self.expected_syntax.map(|expected| expected.kind)
            == Some(EditorExpectedSyntaxKind::ShapeValue))
        .then_some(())
        .and_then(|()| self.shape_value_edit_parts_from_expected_span())
    }

    fn shape_value_edit_parts_from_expected_span(&self) -> Option<(Range, bool, bool)> {
        let expected = self.expected_syntax?;
        let body_start = self.fence.body_range().start;
        let start = body_start + expected.span.start;
        let end = body_start + expected.span.end;
        let range = self.range_for_offsets(start, end)?;
        let has_separator_space = self.snapshot.text()[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace());
        let append_closing_brace = self.should_append_shape_closing_brace(end);

        Some((range, has_separator_space, append_closing_brace))
    }

    fn should_append_shape_closing_brace(&self, offset: usize) -> bool {
        let Some(suffix) = self
            .snapshot
            .text()
            .get(offset..self.fence.body_range().end)
        else {
            return false;
        };
        !matches!(
            suffix.chars().find(|ch| !ch.is_whitespace()),
            Some('}' | ',')
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionTextEditParts {
    pub(crate) range: Range,
    pub(crate) replacement: String,
}

fn current_line_prefix(text: &str, cursor: usize) -> (usize, &str) {
    let before = &text[..cursor];
    let line_start = before
        .as_bytes()
        .iter()
        .rposition(|byte| matches!(byte, b'\n' | b'\r'))
        .map(|index| index + 1)
        .unwrap_or(0);
    let raw_prefix = &before[line_start..];
    let trimmed = raw_prefix.trim_start();
    let prefix_start = line_start + raw_prefix.len().saturating_sub(trimmed.len());

    (prefix_start, trimmed)
}

fn is_source_start_context(text: &str, prefix_start: usize) -> bool {
    text[..prefix_start].trim().is_empty()
}

fn completion_evidence_at_cursor(
    expected_syntax: &[EditorExpectedSyntax],
    text: &str,
    cursor: usize,
) -> (Option<EditorExpectedSyntax>, bool) {
    let bytes = text.as_bytes();
    let insertion = match bytes.get(cursor).copied() {
        Some(b'\r') if bytes.get(cursor + 1) == Some(&b'\n') => cursor.checked_add(2),
        Some(b'\r' | b'\n') => cursor.checked_add(1),
        _ => None,
    };
    let mut primary = None;
    let mut insertion_best = None;
    let mut in_directive = false;

    for expected in expected_syntax.iter().copied() {
        if expected.span.start <= cursor && cursor <= expected.span.end {
            in_directive |= expected.kind == EditorExpectedSyntaxKind::Directive;
            retain_narrower_expected(&mut primary, expected);
        }
        if insertion
            .is_some_and(|offset| expected.span.start <= offset && offset <= expected.span.end)
        {
            retain_narrower_expected(&mut insertion_best, expected);
        }
    }

    let fallback = insertion_best.filter(|expected| {
        insertion.is_some_and(|offset| expected.span.start == offset && expected.span.end == offset)
    });
    if !in_directive {
        in_directive =
            fallback.is_some_and(|expected| expected.kind == EditorExpectedSyntaxKind::Directive);
    }

    (primary.or(fallback), in_directive)
}

fn retain_narrower_expected(
    selected: &mut Option<EditorExpectedSyntax>,
    candidate: EditorExpectedSyntax,
) {
    let candidate_key = (
        candidate.span.end.saturating_sub(candidate.span.start),
        candidate.span.start,
        candidate.span.end,
    );
    if selected.is_none_or(|current| {
        candidate_key
            < (
                current.span.end.saturating_sub(current.span.start),
                current.span.start,
                current.span.end,
            )
    }) {
        *selected = Some(candidate);
    }
}
