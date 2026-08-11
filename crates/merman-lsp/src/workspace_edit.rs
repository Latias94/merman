use crate::protocol::{document_uri_to_lsp, range_to_lsp};
use merman_editor_core::{DocumentUri, Range as CoreRange};
use std::collections::BTreeMap;
use tower_lsp_server::ls_types::{
    DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, TextDocumentEdit, TextEdit,
    Uri, WorkspaceEdit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceEditEncoding {
    DocumentChanges,
    Changes,
}

impl WorkspaceEditEncoding {
    pub(crate) const fn from_document_changes_support(supported: bool) -> Self {
        if supported {
            Self::DocumentChanges
        } else {
            Self::Changes
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceEditChange {
    uri: DocumentUri,
    range: CoreRange,
    new_text: String,
}

impl WorkspaceEditChange {
    pub(crate) fn new(uri: DocumentUri, range: CoreRange, new_text: String) -> Self {
        Self {
            uri,
            range,
            new_text,
        }
    }
}

pub(crate) fn project_workspace_edit(
    changes: impl IntoIterator<Item = WorkspaceEditChange>,
    fallback_uri: &Uri,
    current_document_version: i32,
    encoding: WorkspaceEditEncoding,
) -> Option<WorkspaceEdit> {
    let mut buckets = BTreeMap::<String, (Uri, Vec<TextEdit>)>::new();
    for change in changes {
        let uri = document_uri_to_lsp(&change.uri, fallback_uri);
        let key = uri.as_str().to_string();
        buckets
            .entry(key)
            .or_insert_with(|| (uri.clone(), Vec::new()))
            .1
            .push(TextEdit::new(range_to_lsp(change.range), change.new_text));
    }

    if buckets.is_empty() {
        return None;
    }

    match encoding {
        WorkspaceEditEncoding::DocumentChanges => Some(WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Edits(
                buckets
                    .into_values()
                    .map(|(uri, edits)| TextDocumentEdit {
                        text_document: OptionalVersionedTextDocumentIdentifier {
                            version: (uri == *fallback_uri).then_some(current_document_version),
                            uri,
                        },
                        edits: edits.into_iter().map(OneOf::Left).collect(),
                    })
                    .collect(),
            )),
            change_annotations: None,
        }),
        WorkspaceEditEncoding::Changes => Some(WorkspaceEdit {
            changes: Some(buckets.into_values().collect()),
            document_changes: None,
            change_annotations: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tower_lsp_server::ls_types::Position;

    fn change(uri: &str, start: u32, text: &str) -> WorkspaceEditChange {
        WorkspaceEditChange::new(
            DocumentUri::from(uri),
            CoreRange::new(
                merman_editor_core::Position::new(0, start as usize),
                merman_editor_core::Position::new(0, start as usize + 1),
            ),
            text.to_string(),
        )
    }

    #[test]
    fn both_encodings_preserve_grouping_order_and_current_version() {
        let current = Uri::from_str("file:///tmp/current.mmd").unwrap();
        let changes = vec![
            change(current.as_str(), 0, "a"),
            change("file:///tmp/other.mmd", 1, "b"),
            change(current.as_str(), 2, "c"),
        ];
        let document_changes = project_workspace_edit(
            changes.clone(),
            &current,
            7,
            WorkspaceEditEncoding::DocumentChanges,
        )
        .expect("document changes");
        let DocumentChanges::Edits(document_edits) = document_changes.document_changes.unwrap()
        else {
            panic!("expected text document edits");
        };
        assert_eq!(document_edits.len(), 2);
        assert_eq!(document_edits[0].text_document.uri, current);
        assert_eq!(document_edits[0].text_document.version, Some(7));
        assert_eq!(document_edits[1].text_document.version, None);
        assert_eq!(document_edits[0].edits.len(), 2);
        let OneOf::Left(first) = &document_edits[0].edits[0] else {
            panic!("expected text edit");
        };
        assert_eq!(first.range.start, Position::new(0, 0));
        assert_eq!(first.new_text, "a");

        let changes_encoding =
            project_workspace_edit(changes, &current, 7, WorkspaceEditEncoding::Changes)
                .expect("changes encoding");
        assert!(changes_encoding.document_changes.is_none());
        let changes_map = changes_encoding.changes.unwrap();
        assert_eq!(changes_map[&current].len(), 2);
        assert_eq!(
            changes_map[&Uri::from_str("file:///tmp/other.mmd").unwrap()].len(),
            1
        );
    }

    #[test]
    fn empty_input_and_colliding_fallback_uris_are_safe() {
        let fallback = Uri::from_str("file:///tmp/current.mmd").unwrap();
        assert!(
            project_workspace_edit(
                Vec::new(),
                &fallback,
                1,
                WorkspaceEditEncoding::DocumentChanges,
            )
            .is_none()
        );

        let edits = project_workspace_edit(
            vec![change("not a URI", 0, "a"), change("not a URI", 1, "b")],
            &fallback,
            1,
            WorkspaceEditEncoding::DocumentChanges,
        )
        .expect("fallback bucket");
        let DocumentChanges::Edits(edits) = edits.document_changes.unwrap() else {
            panic!("expected text document edits");
        };
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text_document.uri, fallback);
        assert_eq!(edits[0].edits.len(), 2);
    }
}
