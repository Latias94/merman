use crate::client_profile::ClientProtocolProfile;
use crate::protocol::{DiagnosticIdentityData, WorkspaceEditEncoding, range_to_lsp};
use crate::snapshot::DocumentSnapshot;
use merman_editor_core::{
    EditorCodeActionEdit, EditorDiagnostic, Position as EditorPosition, code_actions_from_fixes,
};
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashMap;
use tower_lsp_server::ls_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Diagnostic, DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, TextDocumentEdit,
    TextEdit, Uri, WorkspaceEdit,
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

    let diagnostic_index = SnapshotDiagnosticIndex::new(diagnostics);
    let actions = params
        .context
        .diagnostics
        .iter()
        .filter_map(|diagnostic| {
            Some((
                diagnostic,
                matching_snapshot_diagnostic(snapshot, &diagnostic_index, diagnostic)?,
            ))
        })
        .flat_map(|(lsp_diagnostic, editor_diagnostic)| {
            code_actions_for_editor_diagnostic(
                editor_diagnostic,
                lsp_diagnostic,
                &params.text_document.uri,
                snapshot.version(),
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DiagnosticLookupKey {
    id: String,
    code: String,
    message: String,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
}

impl DiagnosticLookupKey {
    fn from_editor(diagnostic: &EditorDiagnostic) -> Option<Self> {
        let data = diagnostic.data.as_ref()?;
        let range = range_to_lsp(diagnostic.range);
        Some(Self {
            id: data.id.clone(),
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            start_line: range.start.line,
            start_character: range.start.character,
            end_line: range.end.line,
            end_character: range.end.character,
        })
    }

    fn from_lsp(diagnostic: &Diagnostic, identity: DiagnosticIdentityData) -> Option<Self> {
        let tower_lsp_server::ls_types::NumberOrString::String(code) = diagnostic.code.as_ref()?
        else {
            return None;
        };
        Some(Self {
            id: identity.id,
            code: code.clone(),
            message: diagnostic.message.clone(),
            start_line: diagnostic.range.start.line,
            start_character: diagnostic.range.start.character,
            end_line: diagnostic.range.end.line,
            end_character: diagnostic.range.end.character,
        })
    }
}

struct SnapshotDiagnosticIndex<'a> {
    by_identity: HashMap<DiagnosticLookupKey, &'a EditorDiagnostic>,
}

#[cfg(test)]
thread_local! {
    static SNAPSHOT_DIAGNOSTIC_INDEX_PROBE: Cell<(usize, usize)> =
        const { Cell::new((0, 0)) };
}

#[cfg(test)]
fn reset_snapshot_diagnostic_index_probe() {
    SNAPSHOT_DIAGNOSTIC_INDEX_PROBE.set((0, 0));
}

#[cfg(test)]
fn snapshot_diagnostic_index_probe() -> (usize, usize) {
    SNAPSHOT_DIAGNOSTIC_INDEX_PROBE.get()
}

impl<'a> SnapshotDiagnosticIndex<'a> {
    fn new(diagnostics: &'a [EditorDiagnostic]) -> Self {
        #[cfg(test)]
        SNAPSHOT_DIAGNOSTIC_INDEX_PROBE.with(|probe| {
            let (builds, lookups) = probe.get();
            probe.set((builds.saturating_add(1), lookups));
        });

        let mut by_identity = HashMap::with_capacity(diagnostics.len());
        for diagnostic in diagnostics {
            let Some(key) = DiagnosticLookupKey::from_editor(diagnostic) else {
                continue;
            };
            by_identity.entry(key).or_insert(diagnostic);
        }
        Self { by_identity }
    }

    fn get(&self, key: &DiagnosticLookupKey) -> Option<&'a EditorDiagnostic> {
        #[cfg(test)]
        SNAPSHOT_DIAGNOSTIC_INDEX_PROBE.with(|probe| {
            let (builds, lookups) = probe.get();
            probe.set((builds, lookups.saturating_add(1)));
        });

        self.by_identity.get(key).copied()
    }
}

fn matching_snapshot_diagnostic<'a>(
    snapshot: &DocumentSnapshot,
    diagnostics: &SnapshotDiagnosticIndex<'a>,
    diagnostic: &Diagnostic,
) -> Option<&'a EditorDiagnostic> {
    if diagnostic.source.as_deref() != Some("merman") {
        return None;
    }
    let identity =
        serde_json::from_value::<DiagnosticIdentityData>(diagnostic.data.as_ref()?.clone()).ok()?;
    if identity.document_version != Some(snapshot.version()) {
        return None;
    }

    diagnostics.get(&DiagnosticLookupKey::from_lsp(diagnostic, identity)?)
}

fn code_actions_for_editor_diagnostic(
    editor_diagnostic: &EditorDiagnostic,
    lsp_diagnostic: &Diagnostic,
    uri: &Uri,
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
            Some(tower_lsp_server::ls_types::CodeAction {
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
        .map(tower_lsp_server::ls_types::CodeActionOrCommand::CodeAction)
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
    uri: &Uri,
    current_document_version: i32,
    workspace_edit_encoding: WorkspaceEditEncoding,
) -> Option<WorkspaceEdit> {
    let text_edits = planned_edits
        .iter()
        .map(|edit| {
            let range = tower_lsp_server::ls_types::Range::new(
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

fn editor_position_to_lsp(position: EditorPosition) -> tower_lsp_server::ls_types::Position {
    tower_lsp_server::ls_types::Position::new(position.line as u32, position.character as u32)
}

#[cfg(test)]
mod tests {
    use super::{
        code_actions_for_snapshot_with_encoding, reset_snapshot_diagnostic_index_probe,
        snapshot_diagnostic_index_probe,
    };
    use crate::diagnostics::editor_diagnostics_to_versioned_diagnostics;
    use crate::protocol::WorkspaceEditEncoding;
    use crate::snapshot::snapshot_for_test;
    use merman_analysis::{AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, Analyzer};
    use merman_editor_core::{EditorDiagnostic, analysis_payload_to_diagnostics};
    use serde_json::json;
    use std::str::FromStr;
    use tower_lsp_server::ls_types::{
        CodeAction, CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
        DocumentChanges, OneOf, Position, Range, TextDocumentIdentifier, TextEdit, Uri,
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
        assert_eq!(versioned_edits(action, snapshot.uri())[0].new_text, " TB");
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

    #[test]
    fn snapshot_code_action_entry_indexes_many_full_identity_keys_once() {
        let (snapshot, diagnostics, mut params, _) = snapshot_with_direction_fix();
        let prototype = diagnostics
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .expect("diagnostic identity");
        let diagnostics = (0..4_096)
            .map(|index| {
                let mut diagnostic = prototype.clone();
                diagnostic.code = format!("code-{index}");
                diagnostic.message = format!("message-{index}");
                diagnostic.data.as_mut().expect("diagnostic data").id = format!("identity-{index}");
                diagnostic
            })
            .collect::<Vec<_>>();
        params.context.diagnostics = editor_diagnostics_to_versioned_diagnostics(
            &diagnostics,
            snapshot.uri(),
            snapshot.version(),
        );

        reset_snapshot_diagnostic_index_probe();
        let actions = code_actions_for_snapshot_with_encoding(
            &snapshot,
            &diagnostics,
            &params,
            WorkspaceEditEncoding::DocumentChanges,
        )
        .expect("snapshot-owned quick fixes");

        assert_eq!(actions.len(), diagnostics.len());
        assert_eq!(
            snapshot_diagnostic_index_probe(),
            (1, diagnostics.len()),
            "the production entry must build one index and perform one lookup per requested diagnostic"
        );
    }

    fn snapshot_with_direction_fix() -> (
        std::sync::Arc<crate::snapshot::DocumentSnapshot>,
        Vec<EditorDiagnostic>,
        CodeActionParams,
        Uri,
    ) {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let options = AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
        );
        let source = "flowchart\nA-->B\n".to_string();
        let snapshot = snapshot_for_test(uri.clone(), DOCUMENT_VERSION, source.clone());
        let diagnostics =
            analysis_payload_to_diagnostics(&Analyzer::with_options(options).analyze(&source));
        let diagnostic =
            editor_diagnostics_to_versioned_diagnostics(&diagnostics, &uri, DOCUMENT_VERSION)
                .into_iter()
                .find(|diagnostic| {
                    diagnostic.code.as_ref().is_some_and(|code| {
                        code == &tower_lsp_server::ls_types::NumberOrString::String(
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

    fn versioned_edits(action: &CodeAction, uri: &Uri) -> Vec<TextEdit> {
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
