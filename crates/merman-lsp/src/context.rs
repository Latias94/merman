use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::lsp::position_from_utf16;
use tower_lsp::lsp_types::{Position, Range, Url};

#[derive(Debug)]
pub struct CompletionContext<'a> {
    snapshot: &'a DocumentSnapshot,
    fence: &'a FenceSnapshot,
    prefix: String,
    prefix_start_offset: usize,
    cursor_offset: usize,
}

impl<'a> CompletionContext<'a> {
    pub fn from_snapshot(snapshot: &'a DocumentSnapshot, position: Position) -> Option<Self> {
        let fence = snapshot.fence_at_position(position)?;
        let cursor_offset = snapshot.byte_offset_for_position(position)?;
        let prefix = completion_prefix(snapshot, fence, position);
        let prefix_start_offset = cursor_offset.saturating_sub(prefix.len());

        Some(Self {
            snapshot,
            fence,
            prefix,
            prefix_start_offset,
            cursor_offset,
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
        self.shape_value_edit_parts().map(|(range, _)| range)
    }

    pub fn shape_value_edit(&self, value: &str) -> Option<(Range, String)> {
        let (range, has_separator_space) = self.shape_value_edit_parts()?;
        let replacement = if has_separator_space {
            format!("{value} }}")
        } else {
            format!(" {value} }}")
        };

        Some((range, replacement))
    }

    pub fn shape_trigger_range(&self) -> Option<Range> {
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
        let prefix = self.prefix.trim_end();

        prefix.is_empty() || diagram_header_prefix_matches(prefix)
    }

    pub fn offer_operator_items(&self) -> bool {
        let prefix = self.prefix.trim_end();

        prefix.ends_with('-') || prefix.ends_with("--") || prefix.ends_with("->")
    }

    pub fn offer_directive_items(&self) -> bool {
        let prefix = self.prefix.trim_end();

        prefix.contains("class") || prefix.contains(":::")
    }

    pub fn offer_direction_items(&self) -> bool {
        self.prefix.trim_end() == "direction"
    }

    pub fn offer_shape_items(&self) -> bool {
        let prefix = self.prefix.trim_end();

        prefix.contains("@{ shape:")
            || prefix.ends_with("((")
            || prefix.ends_with("{{")
            || prefix.ends_with('[')
            || prefix.ends_with("[/")
            || prefix.ends_with("[\\")
            || prefix.ends_with('>')
    }

    pub fn offer_node_items(&self) -> bool {
        let prefix = self.prefix.trim_end();

        !diagram_header_prefix_matches(prefix)
            && !self.offer_direction_items()
            && !self.offer_directive_items()
            && !self.offer_operator_items()
            && !self.offer_shape_items()
    }

    pub fn node_text_edit_range(&self) -> Option<Range> {
        if self.operator_range().is_some() {
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

    fn shape_value_edit_parts(&self) -> Option<(Range, bool)> {
        let prefix = self.prefix.as_str();
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
}

fn completion_prefix(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    position: Position,
) -> String {
    let Some(offset) = snapshot.byte_offset_for_position(position) else {
        return String::new();
    };

    let rel = offset.saturating_sub(fence.body_start);
    fence.text[..rel.min(fence.text.len())]
        .rsplit_once('\n')
        .map(|(_, tail)| tail.trim_start().to_string())
        .unwrap_or_else(|| {
            fence.text[..rel.min(fence.text.len())]
                .trim_start()
                .to_string()
        })
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

fn diagram_header_prefix_matches(prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }

    [
        "flowchart TD",
        "sequenceDiagram",
        "stateDiagram-v2",
        "mindmap",
    ]
    .iter()
    .any(|candidate| candidate.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::CompletionContext;
    use crate::document_store::DocumentStore;
    use tower_lsp::lsp_types::{Position, Url};

    #[test]
    fn context_captures_prefix_and_fence_metadata() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\n".to_string());
        let context = CompletionContext::from_snapshot(&snapshot, Position::new(0, 9)).unwrap();

        assert_eq!(context.prefix(), "flowchart");
        assert_eq!(context.uri().as_str(), "file:///tmp/example.mmd");
        assert_eq!(context.fence().completion.node_ids().count(), 2);
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

        let direction = store.upsert(uri.clone(), 4, "direction".to_string());
        let direction_context =
            CompletionContext::from_snapshot(&direction, Position::new(0, 9)).unwrap();
        assert!(direction_context.offer_direction_items());

        let shape = store.upsert(uri.clone(), 5, "A@{ shape: ".to_string());
        let shape_context = CompletionContext::from_snapshot(&shape, Position::new(0, 11)).unwrap();
        assert!(shape_context.offer_shape_items());
        assert!(shape_context.shape_value_range().is_some());
        let (shape_range, shape_replacement) = shape_context
            .shape_value_edit("circle")
            .expect("shape edit");
        assert_eq!(shape_range.start.character, 11);
        assert_eq!(shape_replacement, "circle }");

        let classic_shape = store.upsert(uri, 6, "A((".to_string());
        let classic_shape_context =
            CompletionContext::from_snapshot(&classic_shape, Position::new(0, 3)).unwrap();
        assert!(classic_shape_context.offer_shape_items());
        assert!(classic_shape_context.shape_trigger_range().is_some());

        let node = store.upsert(
            Url::parse("file:///tmp/example.mmd").unwrap(),
            7,
            "node_1".to_string(),
        );
        let node_context = CompletionContext::from_snapshot(&node, Position::new(0, 6)).unwrap();
        assert!(node_context.offer_node_items());
    }
}
