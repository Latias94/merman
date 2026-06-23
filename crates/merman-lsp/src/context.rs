use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use tower_lsp::lsp_types::{Position, Url};

#[derive(Debug)]
pub struct CompletionContext<'a> {
    snapshot: &'a DocumentSnapshot,
    fence: &'a FenceSnapshot,
    prefix: String,
}

impl<'a> CompletionContext<'a> {
    pub fn from_snapshot(snapshot: &'a DocumentSnapshot, position: Position) -> Option<Self> {
        let fence = snapshot.fence_at_position(position)?;
        let prefix = completion_prefix(snapshot, fence, position);

        Some(Self {
            snapshot,
            fence,
            prefix,
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

    pub fn offer_diagram_headers(&self) -> bool {
        let prefix = self.prefix.trim_end();

        prefix.is_empty()
            || prefix == "flowchart"
            || prefix == "sequenceDiagram"
            || prefix == "stateDiagram"
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
    }

    #[test]
    fn context_classifies_header_operator_and_directive_prefixes() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        let header = store.upsert(uri.clone(), 1, "flowchart".to_string());
        let header_context =
            CompletionContext::from_snapshot(&header, Position::new(0, 9)).unwrap();
        assert!(header_context.offer_diagram_headers());

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
    }
}
