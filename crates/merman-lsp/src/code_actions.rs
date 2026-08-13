use crate::workspace_edit::{WorkspaceEditChange, WorkspaceEditEncoding, project_workspace_edit};
use merman_analysis::DiagnosticFix;
use merman_editor_core::{DocumentUri, code_action_from_fix};
use tower_lsp_server::ls_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, Diagnostic, Uri,
};

pub(crate) fn append_action_diagnostic(action: &mut CodeActionOrCommand, diagnostic: &Diagnostic) {
    let CodeActionOrCommand::CodeAction(action) = action else {
        return;
    };
    action
        .diagnostics
        .get_or_insert_default()
        .push(diagnostic.clone());
}

pub(crate) fn code_action_for_server_fix(
    fix: &DiagnosticFix,
    lsp_diagnostic: &Diagnostic,
    uri: &Uri,
    current_document_version: i32,
    workspace_edit_encoding: WorkspaceEditEncoding,
    is_preferred_support: bool,
) -> Option<CodeActionOrCommand> {
    let action = code_action_from_fix(fix)?;
    let changes = action.edits.into_iter().map(|edit| {
        WorkspaceEditChange::new(DocumentUri::from(uri.as_str()), edit.range, edit.new_text)
    });
    let edit = project_workspace_edit(
        changes,
        uri,
        current_document_version,
        workspace_edit_encoding,
    )?;
    Some(CodeActionOrCommand::CodeAction(
        tower_lsp_server::ls_types::CodeAction {
            title: action.title,
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![lsp_diagnostic.clone()]),
            edit: Some(edit),
            command: None,
            is_preferred: (is_preferred_support && action.is_preferred).then_some(true),
            disabled: None,
            data: None,
        },
    ))
}

pub(crate) fn allows_quickfix(context: &CodeActionContext) -> bool {
    context
        .only
        .as_ref()
        .is_none_or(|only| only.iter().any(|kind| kind == &CodeActionKind::QUICKFIX))
}
