use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::{
    FenceCursorCompletionKind, FenceExpectedSyntaxKind, lsp::position_from_utf16,
};
use tower_lsp::lsp_types::{Position, Range, Url};

#[derive(Debug)]
pub struct CompletionContext<'a> {
    snapshot: &'a DocumentSnapshot,
    fence: &'a FenceSnapshot,
    prefix: String,
    prefix_start_offset: usize,
    cursor_offset: usize,
    directive_prefix: Option<&'static str>,
    comment_or_directive_line: bool,
    expected_syntax: Option<FenceExpectedSyntaxKind>,
    expected_syntax_span: Option<(usize, usize)>,
    completion_kinds: Vec<FenceCursorCompletionKind>,
}

impl<'a> CompletionContext<'a> {
    pub fn from_snapshot(snapshot: &'a DocumentSnapshot, position: Position) -> Option<Self> {
        let fence = snapshot.fence_at_position(position)?;
        let cursor_offset = snapshot.byte_offset_for_position(position)?;
        let relative_cursor = cursor_offset
            .saturating_sub(fence.body_start)
            .min(fence.text.len());
        let cursor_context = fence
            .text_index
            .cursor_context(&fence.text, relative_cursor);
        let prefix_start_offset = fence.body_start + cursor_context.prefix_start();
        let cursor_offset = fence.body_start + cursor_context.cursor();

        Some(Self {
            snapshot,
            fence,
            prefix: cursor_context.prefix().to_string(),
            prefix_start_offset,
            cursor_offset,
            directive_prefix: cursor_context.directive_prefix(),
            comment_or_directive_line: cursor_context.is_comment_or_directive_line(),
            expected_syntax: cursor_context.expected_syntax(),
            expected_syntax_span: cursor_context
                .expected_syntax_span()
                .map(|span| (fence.body_start + span.start, fence.body_start + span.end)),
            completion_kinds: cursor_context.completion_kinds().to_vec(),
        })
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn fence(&self) -> &FenceSnapshot {
        self.fence
    }

    pub fn uri(&self) -> &Url {
        &self.snapshot.uri
    }

    pub fn prefix_range(&self) -> Option<Range> {
        self.range_for_offsets(self.prefix_start_offset, self.cursor_offset)
    }

    pub fn operator_range(&self) -> Option<Range> {
        let suffix_start = operator_suffix_start(&self.prefix)?;
        self.range_for_offsets(self.prefix_start_offset + suffix_start, self.cursor_offset)
    }

    pub fn shape_value_range(&self) -> Option<Range> {
        self.shape_value_edit_parts().map(|(range, _, _)| range)
    }

    pub fn shape_value_edit(&self, value: &str) -> Option<(Range, String)> {
        let (range, has_separator_space, append_closing_brace) = self.shape_value_edit_parts()?;
        let replacement = if append_closing_brace {
            if has_separator_space {
                format!("{value} }}")
            } else {
                format!(" {value} }}")
            }
        } else {
            if has_separator_space {
                value.to_string()
            } else {
                format!(" {value}")
            }
        };

        Some((range, replacement))
    }

    pub fn shape_trigger_range(&self) -> Option<Range> {
        if matches!(
            self.expected_syntax,
            Some(FenceExpectedSyntaxKind::ShapeTrigger)
        ) {
            if let Some((start, end)) = self.expected_syntax_span {
                return self.range_for_offsets(start, end);
            }
        }

        let prefix = self.prefix.trim_end();
        let trigger_len = if prefix.ends_with("((")
            || prefix.ends_with("{{")
            || prefix.ends_with("[/")
            || prefix.ends_with("[\\")
        {
            2
        } else if prefix.ends_with('[') || prefix.ends_with('>') {
            1
        } else {
            return None;
        };

        self.range_for_offsets(
            self.prefix_start_offset + prefix.len().saturating_sub(trigger_len),
            self.cursor_offset,
        )
    }

    pub fn offer_diagram_headers(&self) -> bool {
        self.offers(FenceCursorCompletionKind::DiagramHeader)
    }

    pub fn offer_operator_items(&self) -> bool {
        self.offers(FenceCursorCompletionKind::Operator)
    }

    pub fn offer_directive_items(&self) -> bool {
        self.offers(FenceCursorCompletionKind::Directive)
    }

    pub fn offer_direction_items(&self) -> bool {
        if let Some(expected) = self.expected_syntax {
            return matches!(expected, FenceExpectedSyntaxKind::Direction);
        }

        self.offers(FenceCursorCompletionKind::Direction)
    }

    pub fn offer_shape_items(&self) -> bool {
        if let Some(expected) = self.expected_syntax {
            return matches!(
                expected,
                FenceExpectedSyntaxKind::Shape | FenceExpectedSyntaxKind::ShapeTrigger
            );
        }

        self.offers(FenceCursorCompletionKind::Shape)
    }

    pub fn offer_node_items(&self) -> bool {
        if let Some(expected) = self.expected_syntax {
            return matches!(
                expected,
                FenceExpectedSyntaxKind::NodeIdentifier | FenceExpectedSyntaxKind::IdList
            );
        }

        self.offers(FenceCursorCompletionKind::NodeIdentifier)
    }

    pub(crate) fn is_comment_or_directive_line(&self) -> bool {
        self.comment_or_directive_line
    }

    pub(crate) fn is_parser_controlled_payload(&self) -> bool {
        self.expected_syntax == Some(FenceExpectedSyntaxKind::Payload)
    }

    pub fn directive_prefix(&self) -> Option<&'static str> {
        self.directive_prefix
    }

    pub fn node_text_edit_range(&self) -> Option<Range> {
        if self.offer_operator_items() {
            None
        } else {
            self.prefix_range()
        }
    }

    fn range_for_offsets(&self, start: usize, end: usize) -> Option<Range> {
        let span = self.snapshot.source_map.span(start, end).ok()?;
        Some(Range {
            start: position_from_utf16(span.lsp_range.start),
            end: position_from_utf16(span.lsp_range.end),
        })
    }

    fn shape_value_edit_parts(&self) -> Option<(Range, bool, bool)> {
        let prefix = self.prefix.as_str();
        if let Some((range, has_separator_space)) = self.shape_value_edit_parts_from_prefix(prefix)
        {
            return Some((range, has_separator_space, true));
        }

        if self.expected_syntax == Some(FenceExpectedSyntaxKind::Shape) {
            return self.shape_value_edit_parts_from_expected_span();
        }

        None
    }

    fn shape_value_edit_parts_from_prefix(&self, prefix: &str) -> Option<(Range, bool)> {
        let marker = prefix.rfind("@{ shape:")?;
        let after_colon = marker + "@{ shape:".len();
        let suffix = &prefix[after_colon..];
        let whitespace = suffix
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(|ch| ch.len_utf8())
            .sum::<usize>();
        let has_separator_space = whitespace > 0;
        let range = self.range_for_offsets(
            self.prefix_start_offset + after_colon + whitespace,
            self.cursor_offset,
        )?;

        Some((range, has_separator_space))
    }

    fn shape_value_edit_parts_from_expected_span(&self) -> Option<(Range, bool, bool)> {
        let (start, end) = self.expected_syntax_span?;
        let range = self.range_for_offsets(start, end)?;
        let has_separator_space = self.snapshot.text[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace());
        let append_closing_brace = self.snapshot.text[end..]
            .chars()
            .all(|ch| ch.is_whitespace());

        Some((range, has_separator_space, append_closing_brace))
    }

    fn offers(&self, kind: FenceCursorCompletionKind) -> bool {
        self.completion_kinds.contains(&kind)
    }
}

fn operator_suffix_start(prefix: &str) -> Option<usize> {
    let mut start = prefix.len();
    let mut seen_operator = false;

    for (idx, ch) in prefix.char_indices().rev() {
        if matches!(ch, '-' | '>' | '.' | '=') {
            start = idx;
            seen_operator = true;
        } else {
            break;
        }
    }

    seen_operator.then_some(start)
}

#[cfg(test)]
mod tests {
    use super::CompletionContext;
    use crate::document_store::DocumentStore;
    use merman_analysis::FenceExpectedSyntaxKind;
    use tower_lsp::lsp_types::{Position, Url};

    #[test]
    fn context_captures_prefix_and_fence_metadata() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\n".to_string());
        let context = CompletionContext::from_snapshot(&snapshot, Position::new(0, 9)).unwrap();

        assert_eq!(context.prefix(), "flowchart");
        assert_eq!(context.uri().as_str(), "file:///tmp/example.mmd");
        assert_eq!(context.fence().text_index.node_ids().count(), 2);
        assert!(context.prefix_range().is_some());
    }

    #[test]
    fn context_classifies_header_operator_and_directive_prefixes() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        let header = store.upsert(uri.clone(), 1, "flowchart".to_string());
        let header_context =
            CompletionContext::from_snapshot(&header, Position::new(0, 9)).unwrap();
        assert!(header_context.offer_diagram_headers());
        assert!(!header_context.offer_node_items());

        let operator = store.upsert(uri.clone(), 2, "flowchart TD\nA-->B".to_string());
        let operator_context =
            CompletionContext::from_snapshot(&operator, Position::new(1, 3)).unwrap();
        assert!(operator_context.offer_operator_items());

        let directive = store.upsert(uri.clone(), 3, "classDef foo fill:#f00".to_string());
        let directive_context =
            CompletionContext::from_snapshot(&directive, Position::new(0, 21)).unwrap();
        assert!(directive_context.offer_directive_items());

        let click = store.upsert(
            uri.clone(),
            4,
            "click User href \"https://example.com\" \"Open user\" _blank".to_string(),
        );
        let click_context = CompletionContext::from_snapshot(&click, Position::new(0, 18)).unwrap();
        assert!(click_context.is_comment_or_directive_line());
        assert!(!click_context.offer_node_items());

        let direction = store.upsert(uri.clone(), 5, "direction".to_string());
        let direction_context =
            CompletionContext::from_snapshot(&direction, Position::new(0, 9)).unwrap();
        assert!(direction_context.offer_direction_items());

        let shape = store.upsert(uri.clone(), 6, "A@{ shape: ".to_string());
        let shape_context = CompletionContext::from_snapshot(&shape, Position::new(0, 11)).unwrap();
        assert!(shape_context.offer_shape_items());
        assert!(shape_context.shape_value_range().is_some());
        let (shape_range, shape_replacement) = shape_context
            .shape_value_edit("circle")
            .expect("shape edit");
        assert_eq!(shape_range.start.character, 11);
        assert_eq!(shape_replacement, "circle }");

        let classic_shape = store.upsert(uri.clone(), 7, "A((".to_string());
        let classic_shape_context =
            CompletionContext::from_snapshot(&classic_shape, Position::new(0, 3)).unwrap();
        assert!(classic_shape_context.offer_shape_items());
        assert!(classic_shape_context.shape_trigger_range().is_some());

        let parsed_classic_shape = store.upsert(uri.clone(), 8, "flowchart TD\nA((\n".to_string());
        let parsed_classic_shape_context =
            CompletionContext::from_snapshot(&parsed_classic_shape, Position::new(1, 3)).unwrap();
        assert!(parsed_classic_shape_context.offer_shape_items());
        assert!(parsed_classic_shape_context.shape_trigger_range().is_some());

        let parser_shape = store.upsert(
            uri.clone(),
            9,
            "flowchart TD\nA@{\n  shape: rou\n}\n".to_string(),
        );
        let parser_shape_context =
            CompletionContext::from_snapshot(&parser_shape, Position::new(2, 11)).unwrap();
        assert!(parser_shape_context.offer_shape_items());
        let (shape_range, shape_replacement) = parser_shape_context
            .shape_value_edit("circle")
            .expect("parser-backed shape edit");
        assert_eq!(shape_range.start.line, 2);
        assert_eq!(shape_range.start.character, 9);
        assert_eq!(shape_replacement, "circle");

        let node = store.upsert(
            Url::parse("file:///tmp/example.mmd").unwrap(),
            9,
            "node_1".to_string(),
        );
        let node_context = CompletionContext::from_snapshot(&node, Position::new(0, 6)).unwrap();
        assert!(node_context.offer_node_items());

        let gantt = store.upsert(
            uri,
            10,
            "gantt\nsection Demo\nTask 1: id1,after id2,1d\n".to_string(),
        );
        let gantt_context = CompletionContext::from_snapshot(&gantt, Position::new(2, 18)).unwrap();
        assert_eq!(
            gantt_context.expected_syntax,
            Some(FenceExpectedSyntaxKind::NodeIdentifier)
        );
        assert!(gantt_context.offer_node_items());
    }
}
