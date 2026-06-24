use merman_analysis::lsp::DiagnosticCodeActionData;
use tower_lsp::lsp_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Diagnostic, Position, Range, TextEdit, Url, WorkspaceEdit,
};

pub fn code_actions_for_params(params: &CodeActionParams) -> Option<CodeActionResponse> {
    if !allows_quickfix(&params.context) {
        return None;
    }

    let actions = params
        .context
        .diagnostics
        .iter()
        .flat_map(|diagnostic| code_actions_for_diagnostic(diagnostic, &params.text_document.uri))
        .collect::<Vec<_>>();

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

fn allows_quickfix(context: &CodeActionContext) -> bool {
    context
        .only
        .as_ref()
        .is_none_or(|only| only.iter().any(|kind| kind == &CodeActionKind::QUICKFIX))
}

fn code_actions_for_diagnostic(diagnostic: &Diagnostic, uri: &Url) -> Vec<CodeActionOrCommand> {
    if diagnostic.source.as_deref() != Some("merman") {
        return Vec::new();
    }
    let Some(data) = diagnostic.data.as_ref() else {
        return Vec::new();
    };
    let Ok(data) = serde_json::from_value::<DiagnosticCodeActionData>(data.clone()) else {
        return Vec::new();
    };

    let actions = data
        .fixes
        .into_iter()
        .filter_map(|fix| {
            let edit = workspace_edit_for_fix(&fix, uri)?;
            Some(tower_lsp::lsp_types::CodeAction {
                title: fix.title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(edit),
                command: None,
                is_preferred: fix.is_preferred.then_some(true),
                disabled: None,
                data: None,
            })
        })
        .map(tower_lsp::lsp_types::CodeActionOrCommand::CodeAction)
        .collect::<Vec<_>>();

    actions
}

fn workspace_edit_for_fix(
    fix: &merman_analysis::DiagnosticFix,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    let mut edits = fix
        .edits
        .iter()
        .map(|edit| {
            let range = Range {
                start: position_from_lsp(edit.span.lsp_range.start),
                end: position_from_lsp(edit.span.lsp_range.end),
            };
            TextEdit::new(range, edit.replacement.clone())
        })
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return None;
    }
    edits.sort_by(|left, right| {
        (
            left.range.start.line,
            left.range.start.character,
            left.range.end.line,
            left.range.end.character,
        )
            .cmp(&(
                right.range.start.line,
                right.range.start.character,
                right.range.end.line,
                right.range.end.character,
            ))
    });

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

fn position_from_lsp(value: merman_analysis::Utf16Position) -> Position {
    Position {
        line: value.line as u32,
        character: value.character as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::code_actions_for_params;
    use merman_analysis::{
        Analyzer, DiagnosticFix, DiagnosticFixEdit, DiagnosticSpan, Utf16Position,
        document::analyze_document,
        lsp::{DiagnosticCodeActionData, analysis_payload_to_diagnostics},
        markdown::markdown_source_descriptor,
    };
    use tower_lsp::lsp_types::{
        CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, Diagnostic,
        DiagnosticSeverity, NumberOrString, Position, Range, TextDocumentIdentifier, Url,
    };

    fn diagnostic_with_fix() -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 5),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("merman.test".to_string())),
            code_description: None,
            source: Some("merman".to_string()),
            message: "test".to_string(),
            related_information: None,
            tags: None,
            data: Some(
                serde_json::to_value(DiagnosticCodeActionData {
                    id: "merman.test".to_string(),
                    fixes: vec![DiagnosticFix {
                        title: "Replace text".to_string(),
                        edits: vec![DiagnosticFixEdit::new(
                            DiagnosticSpan::new(
                                0,
                                5,
                                1,
                                1,
                                1,
                                6,
                                Utf16Position {
                                    line: 0,
                                    character: 0,
                                },
                                Utf16Position {
                                    line: 0,
                                    character: 5,
                                },
                            ),
                            "fixed",
                        )],
                        is_preferred: true,
                    }],
                })
                .unwrap(),
            ),
        }
    }

    #[test]
    fn quickfixes_from_diagnostic_data_are_projected() {
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse("file:///tmp/example.mmd").unwrap(),
            },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 5),
            },
            context: CodeActionContext {
                diagnostics: vec![diagnostic_with_fix()],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = code_actions_for_params(&params).expect("expected quickfix actions");
        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        assert_eq!(action.title, "Replace text");
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        assert_eq!(action.is_preferred, Some(true));
        let changes = action.edit.as_ref().unwrap().changes.as_ref().unwrap();
        let edits = changes
            .get(&Url::parse("file:///tmp/example.mmd").unwrap())
            .unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "fixed");
    }

    #[test]
    fn diagnostics_without_fix_metadata_do_not_create_actions() {
        let diagnostic = Diagnostic {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 5),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("merman.test".to_string())),
            code_description: None,
            source: Some("merman".to_string()),
            message: "test".to_string(),
            related_information: None,
            tags: None,
            data: None,
        };
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse("file:///tmp/example.mmd").unwrap(),
            },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 5),
            },
            context: CodeActionContext {
                diagnostics: vec![diagnostic],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        assert!(code_actions_for_params(&params).is_none());
    }

    #[test]
    fn non_quickfix_requests_do_not_return_quickfix_actions() {
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse("file:///tmp/example.mmd").unwrap(),
            },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 5),
            },
            context: CodeActionContext {
                diagnostics: vec![diagnostic_with_fix()],
                only: Some(vec![CodeActionKind::REFACTOR]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        assert!(code_actions_for_params(&params).is_none());
    }

    #[test]
    fn analyzer_fix_metadata_produces_quickfix_action() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let analyzer = Analyzer::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let payload = analyzer.analyze(source);
        let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 12),
            },
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = code_actions_for_params(&params).expect("expected analyzer quickfix");
        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        assert_eq!(action.title, "Replace `initialize` with `init`");
        let changes = action.edit.as_ref().unwrap().changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "init");
        assert_eq!(edits[0].range.start, Position::new(0, 4));
        assert_eq!(edits[0].range.end, Position::new(0, 14));
    }

    #[test]
    fn markdown_analyzer_fix_metadata_uses_host_document_ranges() {
        let source = "before\n```mermaid\n%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n```\nafter\n";
        let analyzer = Analyzer::new();
        let uri = Url::parse("file:///tmp/example.md").unwrap();
        let payload = analyze_document(
            source,
            &analyzer,
            markdown_source_descriptor(Some(uri.as_str())),
        );
        let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(2, 0),
                end: Position::new(2, 20),
            },
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = code_actions_for_params(&params).expect("expected markdown quickfix");
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        let changes = action.edit.as_ref().unwrap().changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();

        assert_eq!(edits[0].new_text, "init");
        assert_eq!(edits[0].range.start, Position::new(2, 4));
        assert_eq!(edits[0].range.end, Position::new(2, 14));
    }
}
