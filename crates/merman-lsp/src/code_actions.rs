use crate::client_profile::ClientProtocolProfile;
use crate::protocol::{DiagnosticIdentityData, WorkspaceEditEncoding, range_to_lsp};
use crate::snapshot::DocumentSnapshot;
use merman_editor_core::{
    EditorCodeActionEdit, EditorDiagnostic, Position as EditorPosition, code_actions_from_fixes,
};
use tower_lsp::lsp_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Diagnostic, DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, TextDocumentEdit,
    TextEdit, Url, WorkspaceEdit,
};

/// Builds quick fixes from server-owned diagnostics paired with the checked snapshot.
pub(crate) fn code_actions_for_snapshot_diagnostics_with_profile(
    snapshot: &DocumentSnapshot,
    diagnostics: &[EditorDiagnostic],
    params: &CodeActionParams,
    profile: &ClientProtocolProfile,
) -> Option<CodeActionResponse> {
    let projection = profile.code_actions.as_ref()?;
    code_actions_for_snapshot_with_encoding_and_preferred_support(
        snapshot,
        diagnostics,
        params,
        profile.workspace_edit_encoding,
        projection.is_preferred,
    )
}

fn code_actions_for_snapshot_with_encoding_and_preferred_support(
    snapshot: &DocumentSnapshot,
    diagnostics: &[EditorDiagnostic],
    params: &CodeActionParams,
    workspace_edit_encoding: WorkspaceEditEncoding,
    is_preferred_support: bool,
) -> Option<CodeActionResponse> {
    if !allows_quickfix(&params.context) {
        return None;
    }

    let actions = params
        .context
        .diagnostics
        .iter()
        .filter_map(|diagnostic| {
            Some((
                diagnostic,
                matching_snapshot_diagnostic(snapshot, diagnostics, diagnostic)?,
            ))
        })
        .flat_map(|(lsp_diagnostic, editor_diagnostic)| {
            code_actions_for_editor_diagnostic(
                editor_diagnostic,
                lsp_diagnostic,
                &params.text_document.uri,
                snapshot.version,
                workspace_edit_encoding,
                is_preferred_support,
            )
        })
        .collect::<Vec<_>>();

    (!actions.is_empty()).then_some(actions)
}

#[cfg(test)]
fn code_actions_for_snapshot_with_encoding(
    snapshot: &DocumentSnapshot,
    diagnostics: &[EditorDiagnostic],
    params: &CodeActionParams,
    workspace_edit_encoding: WorkspaceEditEncoding,
) -> Option<CodeActionResponse> {
    code_actions_for_snapshot_with_encoding_and_preferred_support(
        snapshot,
        diagnostics,
        params,
        workspace_edit_encoding,
        true,
    )
}

fn matching_snapshot_diagnostic<'a>(
    snapshot: &'a DocumentSnapshot,
    diagnostics: &'a [EditorDiagnostic],
    diagnostic: &Diagnostic,
) -> Option<&'a EditorDiagnostic> {
    if diagnostic.source.as_deref() != Some("merman") {
        return None;
    }
    let identity =
        serde_json::from_value::<DiagnosticIdentityData>(diagnostic.data.as_ref()?.clone()).ok()?;
    if identity.document_version != Some(snapshot.version) {
        return None;
    }

    diagnostics.iter().find(|candidate| {
        candidate.message == diagnostic.message
            && range_to_lsp(candidate.range) == diagnostic.range
            && matches!(
                diagnostic.code.as_ref(),
                Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == &candidate.code
            )
            && candidate
                .data
                .as_ref()
                .is_some_and(|data| data.id == identity.id)
    })
}

fn code_actions_for_editor_diagnostic(
    editor_diagnostic: &EditorDiagnostic,
    lsp_diagnostic: &Diagnostic,
    uri: &Url,
    current_document_version: i32,
    workspace_edit_encoding: WorkspaceEditEncoding,
    is_preferred_support: bool,
) -> Vec<CodeActionOrCommand> {
    let Some(data) = editor_diagnostic.data.as_ref() else {
        return Vec::new();
    };
    code_actions_from_fixes(&data.fixes)
        .into_iter()
        .filter_map(|action| {
            let edit = workspace_edit_for_edits(
                &action.edits,
                uri,
                current_document_version,
                workspace_edit_encoding,
            )?;
            Some(tower_lsp::lsp_types::CodeAction {
                title: action.title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![lsp_diagnostic.clone()]),
                edit: Some(edit),
                command: None,
                is_preferred: (is_preferred_support && action.is_preferred).then_some(true),
                disabled: None,
                data: None,
            })
        })
        .map(tower_lsp::lsp_types::CodeActionOrCommand::CodeAction)
        .collect()
}

fn allows_quickfix(context: &CodeActionContext) -> bool {
    context
        .only
        .as_ref()
        .is_none_or(|only| only.iter().any(|kind| kind == &CodeActionKind::QUICKFIX))
}

fn workspace_edit_for_edits(
    planned_edits: &[EditorCodeActionEdit],
    uri: &Url,
    current_document_version: i32,
    workspace_edit_encoding: WorkspaceEditEncoding,
) -> Option<WorkspaceEdit> {
    let text_edits = planned_edits
        .iter()
        .map(|edit| {
            let range = tower_lsp::lsp_types::Range::new(
                editor_position_to_lsp(edit.range.start),
                editor_position_to_lsp(edit.range.end),
            );
            TextEdit::new(range, edit.new_text.clone())
        })
        .collect::<Vec<_>>();
    if text_edits.is_empty() {
        return None;
    }

    match workspace_edit_encoding {
        WorkspaceEditEncoding::DocumentChanges => Some(WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: Some(current_document_version),
                },
                edits: text_edits.into_iter().map(OneOf::Left).collect(),
            }])),
            change_annotations: None,
        }),
        WorkspaceEditEncoding::Changes => Some(WorkspaceEdit {
            changes: Some([(uri.clone(), text_edits)].into_iter().collect()),
            document_changes: None,
            change_annotations: None,
        }),
    }
}

fn editor_position_to_lsp(position: EditorPosition) -> tower_lsp::lsp_types::Position {
    tower_lsp::lsp_types::Position::new(position.line as u32, position.character as u32)
}

#[cfg(test)]
mod tests {
    use super::code_actions_for_snapshot_with_encoding;
    use crate::diagnostics::editor_diagnostics_to_versioned_diagnostics;
    use crate::document_store::DocumentStore;
    use crate::protocol::WorkspaceEditEncoding;
    use merman_analysis::{AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, Analyzer};
    use merman_editor_core::{EditorDiagnostic, analysis_payload_to_diagnostics};
    use serde_json::json;
    use tower_lsp::lsp_types::{
        CodeAction, CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
        DocumentChanges, OneOf, Position, Range, TextDocumentIdentifier, TextEdit, Url,
    };

    const DOCUMENT_VERSION: i32 = 7;

    #[test]
    fn quickfixes_are_owned_by_the_current_snapshot() {
        let (snapshot, diagnostics, params, uri) = snapshot_with_direction_fix();

        let actions = code_actions_for_snapshot_with_encoding(
            &snapshot,
            &diagnostics,
            &params,
            WorkspaceEditEncoding::DocumentChanges,
        )
        .expect("snapshot-owned quick fix");
        let action = only_code_action(&actions);

        assert_eq!(action.title, "Insert `TB` into the flowchart header");
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        assert_eq!(action.is_preferred, Some(true));
        let edits = versioned_edits(action, &uri);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, " TB");
        assert_eq!(edits[0].range.start, Position::new(0, 9));
    }

    #[test]
    fn client_fix_payload_is_never_trusted() {
        let (snapshot, diagnostics, mut params, _) = snapshot_with_direction_fix();
        params.context.diagnostics[0]
            .data
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .expect("diagnostic identity")
            .insert(
                "fixes".to_string(),
                json!([{
                    "title": "Replace the entire document",
                    "edits": [{
                        "span": { "lspRange": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } } },
                        "replacement": "forged"
                    }]
                }]),
            );

        let actions = code_actions_for_snapshot_with_encoding(
            &snapshot,
            &diagnostics,
            &params,
            WorkspaceEditEncoding::DocumentChanges,
        )
        .expect("snapshot-owned quick fix");
        let action = only_code_action(&actions);
        assert_eq!(action.title, "Insert `TB` into the flowchart header");
        assert_eq!(versioned_edits(action, &snapshot.uri)[0].new_text, " TB");
    }

    #[test]
    fn stale_or_non_quickfix_requests_are_rejected() {
        let (snapshot, diagnostics, mut params, _) = snapshot_with_direction_fix();
        params.context.diagnostics[0]
            .data
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .expect("diagnostic identity")
            .insert("documentVersion".to_string(), json!(DOCUMENT_VERSION - 1));
        assert!(
            code_actions_for_snapshot_with_encoding(
                &snapshot,
                &diagnostics,
                &params,
                WorkspaceEditEncoding::DocumentChanges,
            )
            .is_none()
        );

        let (_, diagnostics, mut params, _) = snapshot_with_direction_fix();
        params.context.only = Some(vec![CodeActionKind::REFACTOR]);
        assert!(
            code_actions_for_snapshot_with_encoding(
                &snapshot,
                &diagnostics,
                &params,
                WorkspaceEditEncoding::DocumentChanges,
            )
            .is_none()
        );
    }

    #[test]
    fn snapshot_actions_support_plain_workspace_changes() {
        let (snapshot, diagnostics, params, uri) = snapshot_with_direction_fix();
        let actions = code_actions_for_snapshot_with_encoding(
            &snapshot,
            &diagnostics,
            &params,
            WorkspaceEditEncoding::Changes,
        )
        .expect("snapshot-owned quick fix");
        let action = only_code_action(&actions);
        let edit = action.edit.as_ref().expect("workspace edit");

        assert!(edit.document_changes.is_none());
        assert_eq!(
            edit.changes.as_ref().expect("plain changes")[&uri][0].new_text,
            " TB"
        );
    }

    fn snapshot_with_direction_fix() -> (
        std::sync::Arc<crate::snapshot::DocumentSnapshot>,
        Vec<EditorDiagnostic>,
        CodeActionParams,
        Url,
    ) {
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let mut store = DocumentStore::new();
        let options = AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
        );
        store.apply_analyzer_options(options.clone());
        let source = "flowchart\nA-->B\n".to_string();
        let snapshot = store.upsert(uri.clone(), DOCUMENT_VERSION, source.clone());
        let diagnostics =
            analysis_payload_to_diagnostics(&Analyzer::with_options(options).analyze(&source));
        let diagnostic =
            editor_diagnostics_to_versioned_diagnostics(&diagnostics, &uri, DOCUMENT_VERSION)
                .into_iter()
                .find(|diagnostic| {
                    diagnostic.code.as_ref().is_some_and(|code| {
                        code == &tower_lsp::lsp_types::NumberOrString::String(
                            "merman.authoring.flowchart.explicit_direction".to_string(),
                        )
                    })
                })
                .expect("flowchart direction diagnostic");
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(0, 9)),
            context: CodeActionContext {
                diagnostics: vec![diagnostic],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        (snapshot, diagnostics, params, uri)
    }

    fn only_code_action(actions: &[CodeActionOrCommand]) -> &CodeAction {
        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action");
        };
        action
    }

    fn versioned_edits(action: &CodeAction, uri: &Url) -> Vec<TextEdit> {
        let Some(DocumentChanges::Edits(document_edits)) =
            action.edit.as_ref().unwrap().document_changes.as_ref()
        else {
            panic!("expected versioned document changes");
        };
        assert_eq!(document_edits.len(), 1);
        assert_eq!(document_edits[0].text_document.uri, *uri);
        assert_eq!(
            document_edits[0].text_document.version,
            Some(DOCUMENT_VERSION)
        );
        document_edits[0]
            .edits
            .iter()
            .map(|edit| match edit {
                OneOf::Left(edit) => edit.clone(),
                OneOf::Right(_) => panic!("expected plain text edit"),
            })
            .collect()
    }
}
