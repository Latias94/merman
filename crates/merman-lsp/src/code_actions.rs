use crate::protocol::WorkspaceEditEncoding;
use merman_editor_core::{
    DiagnosticCodeActionData, EditorCodeActionEdit, Position as EditorPosition,
    code_actions_from_fixes,
};
use serde::Deserialize;
use tower_lsp::lsp_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Diagnostic, DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, TextDocumentEdit,
    TextEdit, Url, WorkspaceEdit,
};

#[cfg(test)]
pub fn code_actions_for_params(
    params: &CodeActionParams,
    current_document_version: Option<i32>,
) -> Option<CodeActionResponse> {
    code_actions_for_params_with_encoding(
        params,
        current_document_version,
        WorkspaceEditEncoding::DocumentChanges,
    )
}

pub fn code_actions_for_params_with_encoding(
    params: &CodeActionParams,
    current_document_version: Option<i32>,
    workspace_edit_encoding: WorkspaceEditEncoding,
) -> Option<CodeActionResponse> {
    if !allows_quickfix(&params.context) {
        return None;
    }
    let current_document_version = current_document_version?;

    let actions = params
        .context
        .diagnostics
        .iter()
        .flat_map(|diagnostic| {
            code_actions_for_diagnostic(
                diagnostic,
                &params.text_document.uri,
                current_document_version,
                workspace_edit_encoding,
            )
        })
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

fn code_actions_for_diagnostic(
    diagnostic: &Diagnostic,
    uri: &Url,
    current_document_version: i32,
    workspace_edit_encoding: WorkspaceEditEncoding,
) -> Vec<CodeActionOrCommand> {
    if diagnostic.source.as_deref() != Some("merman") {
        return Vec::new();
    }
    let Some(data) = diagnostic.data.as_ref() else {
        return Vec::new();
    };
    let Ok(data) = serde_json::from_value::<LspDiagnosticCodeActionData>(data.clone()) else {
        return Vec::new();
    };
    if data.document_version != Some(current_document_version) {
        return Vec::new();
    }

    code_actions_from_fixes(&data.inner.fixes)
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
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(edit),
                command: None,
                is_preferred: action.is_preferred.then_some(true),
                disabled: None,
                data: None,
            })
        })
        .map(tower_lsp::lsp_types::CodeActionOrCommand::CodeAction)
        .collect::<Vec<_>>()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspDiagnosticCodeActionData {
    #[serde(flatten)]
    inner: DiagnosticCodeActionData,
    document_version: Option<i32>,
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
    use super::{code_actions_for_params, code_actions_for_params_with_encoding};
    use crate::diagnostics::analysis_payload_to_versioned_diagnostics;
    use crate::protocol::WorkspaceEditEncoding;
    use merman_analysis::{
        AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, Analyzer, DiagnosticCategory,
        DiagnosticFix, DiagnosticFixEdit, DiagnosticSpan, SourceMap, Utf16Position,
        document::analyze_document,
    };
    use merman_editor_core::DiagnosticCodeActionData;
    use serde_json::{Value, json};
    use tower_lsp::lsp_types::{
        CodeAction, CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
        Diagnostic, DiagnosticSeverity, DocumentChanges, NumberOrString, OneOf, Position, Range,
        TextDocumentIdentifier, TextEdit, Url,
    };

    const DOC_VERSION: i32 = 1;

    fn diagnostic_with_fix() -> Diagnostic {
        diagnostic_with_fix_for_version(Some(DOC_VERSION))
    }

    fn unversioned_diagnostic_with_fix() -> Diagnostic {
        diagnostic_with_fix_for_version(None)
    }

    fn diagnostic_with_fix_for_version(document_version: Option<i32>) -> Diagnostic {
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
            data: Some(diagnostic_data_value(
                DiagnosticCodeActionData {
                    id: "merman.test".to_string(),
                    code: None,
                    code_name: None,
                    category: DiagnosticCategory::Semantic,
                    diagram_type: None,
                    help: None,
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
                },
                document_version,
            )),
        }
    }

    fn diagnostic_data_value(
        data: DiagnosticCodeActionData,
        document_version: Option<i32>,
    ) -> Value {
        let mut value = serde_json::to_value(data).unwrap();
        if let Some(document_version) = document_version {
            value
                .as_object_mut()
                .unwrap()
                .insert("documentVersion".to_string(), json!(document_version));
        }
        value
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

        let actions =
            code_actions_for_params(&params, Some(DOC_VERSION)).expect("expected quickfix actions");
        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        assert_eq!(action.title, "Replace text");
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        assert_eq!(action.is_preferred, Some(true));
        let edits = text_edits_for_action(
            action,
            &Url::parse("file:///tmp/example.mmd").unwrap(),
            DOC_VERSION,
        );
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "fixed");
    }

    #[test]
    fn quickfixes_can_fall_back_to_workspace_edit_changes() {
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
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

        let actions = code_actions_for_params_with_encoding(
            &params,
            Some(DOC_VERSION),
            WorkspaceEditEncoding::Changes,
        )
        .expect("expected quickfix actions");
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        let edit = action.edit.as_ref().expect("workspace edit");

        assert!(edit.document_changes.is_none());
        let changes = edit.changes.as_ref().expect("plain changes");
        assert_eq!(changes[&uri].len(), 1);
        assert_eq!(changes[&uri][0].new_text, "fixed");
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

        assert!(code_actions_for_params(&params, Some(DOC_VERSION)).is_none());
    }

    #[test]
    fn quickfixes_without_document_version_are_rejected() {
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse("file:///tmp/example.mmd").unwrap(),
            },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 5),
            },
            context: CodeActionContext {
                diagnostics: vec![unversioned_diagnostic_with_fix()],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        assert!(code_actions_for_params(&params, Some(DOC_VERSION)).is_none());
    }

    #[test]
    fn overlapping_fix_edits_do_not_create_actions() {
        let mut diagnostic = diagnostic_with_fix();
        let data = DiagnosticCodeActionData {
            id: "merman.test".to_string(),
            code: None,
            code_name: None,
            category: DiagnosticCategory::Semantic,
            diagram_type: None,
            help: None,
            fixes: vec![DiagnosticFix {
                title: "Overlapping replacement".to_string(),
                edits: vec![
                    DiagnosticFixEdit::new(
                        DiagnosticSpan::new(
                            0,
                            4,
                            1,
                            1,
                            1,
                            5,
                            Utf16Position {
                                line: 0,
                                character: 0,
                            },
                            Utf16Position {
                                line: 0,
                                character: 4,
                            },
                        ),
                        "left",
                    ),
                    DiagnosticFixEdit::new(
                        DiagnosticSpan::new(
                            2,
                            5,
                            1,
                            3,
                            1,
                            6,
                            Utf16Position {
                                line: 0,
                                character: 2,
                            },
                            Utf16Position {
                                line: 0,
                                character: 5,
                            },
                        ),
                        "right",
                    ),
                ],
                is_preferred: true,
            }],
        };
        diagnostic.data = Some(diagnostic_data_value(data, Some(DOC_VERSION)));
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

        assert!(code_actions_for_params(&params, Some(DOC_VERSION)).is_none());
    }

    #[test]
    fn non_overlapping_fix_edits_are_sorted() {
        let mut diagnostic = diagnostic_with_fix();
        let map = SourceMap::new("0123456789");
        let data = DiagnosticCodeActionData {
            id: "merman.test".to_string(),
            code: None,
            code_name: None,
            category: DiagnosticCategory::Semantic,
            diagram_type: None,
            help: None,
            fixes: vec![DiagnosticFix::new(
                "Sorted replacement",
                vec![
                    DiagnosticFixEdit::new(map.span(5, 6).unwrap(), "late"),
                    DiagnosticFixEdit::new(map.span(1, 2).unwrap(), "early"),
                ],
            )],
        };
        diagnostic.data = Some(diagnostic_data_value(data, Some(DOC_VERSION)));
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 9),
            },
            context: CodeActionContext {
                diagnostics: vec![diagnostic],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions =
            code_actions_for_params(&params, Some(DOC_VERSION)).expect("expected quickfix action");
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        let edits = text_edits_for_action(action, &uri, DOC_VERSION);
        assert_eq!(edits[0].range.start, Position::new(0, 1));
        assert_eq!(edits[0].new_text, "early");
        assert_eq!(edits[1].range.start, Position::new(0, 5));
        assert_eq!(edits[1].new_text, "late");
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

        assert!(code_actions_for_params(&params, Some(DOC_VERSION)).is_none());
    }

    #[test]
    fn analyzer_fix_metadata_produces_quickfix_action() {
        let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let analyzer = alias_analyzer();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let payload = analyzer.analyze(source);
        let diagnostics = analysis_payload_to_versioned_diagnostics(&payload, &uri, DOC_VERSION);

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

        let actions = code_actions_for_params(&params, Some(DOC_VERSION))
            .expect("expected analyzer quickfix");
        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        assert_eq!(action.title, "Replace `initialize` with `init`");
        let edits = text_edits_for_action(action, &uri, DOC_VERSION);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "init");
        assert_eq!(edits[0].range.start, Position::new(0, 4));
        assert_eq!(edits[0].range.end, Position::new(0, 14));
    }

    #[test]
    fn frontmatter_config_migration_fix_produces_quickfix_action() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let analyzer = authoring_analyzer();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let payload = analyzer.analyze(source);
        let diagnostics = analysis_payload_to_versioned_diagnostics(&payload, &uri, DOC_VERSION);

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 8),
            },
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = code_actions_for_params(&params, Some(DOC_VERSION))
            .expect("expected frontmatter quickfix");
        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        assert_eq!(action.title, "Move init directive config into frontmatter");
        assert_eq!(action.is_preferred, Some(true));
        let edits = text_edits_for_action(action, &uri, DOC_VERSION);
        assert_eq!(edits.len(), 1);
        assert!(edits[0].new_text.starts_with("---\nconfig:\n"));
        assert!(edits[0].new_text.contains("theme: dark\n"));
        assert_eq!(edits[0].range.start, Position::new(0, 0));
        assert_eq!(edits[0].range.end, Position::new(1, 0));
    }

    #[test]
    fn deprecated_flowchart_html_labels_diagnostic_has_no_quickfix_action() {
        let source = "%%{init: { \"flowchart\": { \"htmlLabels\": false, \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n";
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let payload = Analyzer::new().analyze(source);
        let diagnostics = analysis_payload_to_versioned_diagnostics(&payload, &uri, DOC_VERSION);

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 80),
            },
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        assert!(code_actions_for_params(&params, Some(DOC_VERSION)).is_none());
    }

    #[test]
    fn flowchart_missing_direction_fix_produces_quickfix_action() {
        let source = "flowchart\nA-->B\n";
        let analyzer = authoring_analyzer();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let payload = analyzer.analyze(source);
        let diagnostics = analysis_payload_to_versioned_diagnostics(&payload, &uri, DOC_VERSION);

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 9),
            },
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = code_actions_for_params(&params, Some(DOC_VERSION))
            .expect("expected flowchart quickfix");
        let action = actions
            .iter()
            .filter_map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => Some(action),
                CodeActionOrCommand::Command(_) => None,
            })
            .find(|action| action.title == "Insert `TB` into the flowchart header")
            .expect("missing flowchart direction quickfix");

        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        assert_eq!(action.is_preferred, Some(true));
        let edits = text_edits_for_action(action, &uri, DOC_VERSION);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, " TB");
        assert_eq!(edits[0].range.start, Position::new(0, 9));
        assert_eq!(edits[0].range.end, Position::new(0, 9));
    }

    #[test]
    fn markdown_analyzer_fix_metadata_uses_host_document_ranges() {
        let source = "before\n```mermaid\n%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n```\nafter\n";
        let analyzer = alias_analyzer();
        let uri = Url::parse("file:///tmp/example.md").unwrap();
        let payload = analyze_document(
            source,
            &analyzer,
            merman_analysis::source_descriptor_for_uri(uri.as_str()),
        );
        let diagnostics = analysis_payload_to_versioned_diagnostics(&payload, &uri, DOC_VERSION);
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

        let actions = code_actions_for_params(&params, Some(DOC_VERSION))
            .expect("expected markdown quickfix");
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        let edits = text_edits_for_action(action, &uri, DOC_VERSION);

        assert_eq!(edits[0].new_text, "init");
        assert_eq!(edits[0].range.start, Position::new(2, 4));
        assert_eq!(edits[0].range.end, Position::new(2, 14));
    }

    #[test]
    fn markdown_frontmatter_config_migration_fix_uses_host_document_ranges() {
        let source = "before\n```mermaid\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n```\nafter\n";
        let analyzer = authoring_analyzer();
        let uri = Url::parse("file:///tmp/example.md").unwrap();
        let payload = analyze_document(
            source,
            &analyzer,
            merman_analysis::source_descriptor_for_uri(uri.as_str()),
        );
        let diagnostics = analysis_payload_to_versioned_diagnostics(&payload, &uri, DOC_VERSION);
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(2, 0),
                end: Position::new(2, 8),
            },
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = code_actions_for_params(&params, Some(DOC_VERSION))
            .expect("expected markdown quickfix");
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action")
        };
        let edits = text_edits_for_action(action, &uri, DOC_VERSION);

        assert_eq!(action.title, "Move init directive config into frontmatter");
        assert_eq!(edits.len(), 1);
        assert!(edits[0].new_text.starts_with("---\nconfig:\n"));
        assert_eq!(edits[0].range.start, Position::new(2, 0));
        assert_eq!(edits[0].range.end, Position::new(3, 0));
    }

    #[test]
    fn markdown_flowchart_missing_direction_fix_uses_host_document_ranges() {
        let source = "before\n```mermaid\nflowchart\nA-->B\n```\nafter\n";
        let analyzer = authoring_analyzer();
        let uri = Url::parse("file:///tmp/example.md").unwrap();
        let payload = analyze_document(
            source,
            &analyzer,
            merman_analysis::source_descriptor_for_uri(uri.as_str()),
        );
        let diagnostics = analysis_payload_to_versioned_diagnostics(&payload, &uri, DOC_VERSION);

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(2, 0),
                end: Position::new(2, 9),
            },
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = code_actions_for_params(&params, Some(DOC_VERSION))
            .expect("expected markdown quickfix");
        let action = actions
            .iter()
            .filter_map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => Some(action),
                CodeActionOrCommand::Command(_) => None,
            })
            .find(|action| action.title == "Insert `TB` into the flowchart header")
            .expect("missing flowchart direction quickfix");
        let edits = text_edits_for_action(action, &uri, DOC_VERSION);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, " TB");
        assert_eq!(edits[0].range.start, Position::new(2, 9));
        assert_eq!(edits[0].range.end, Position::new(2, 9));
    }

    fn text_edits_for_action(
        action: &CodeAction,
        uri: &Url,
        document_version: i32,
    ) -> Vec<TextEdit> {
        let Some(DocumentChanges::Edits(document_edits)) =
            action.edit.as_ref().unwrap().document_changes.as_ref()
        else {
            panic!("expected versioned document changes");
        };
        assert_eq!(document_edits.len(), 1);
        assert_eq!(document_edits[0].text_document.uri, *uri);
        assert_eq!(
            document_edits[0].text_document.version,
            Some(document_version)
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

    fn authoring_analyzer() -> Analyzer {
        Analyzer::with_options(AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
        ))
    }

    fn alias_analyzer() -> Analyzer {
        Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_profile(AnalysisRuleProfile::Recommended)
                    .with_rule_disabled("merman.authoring.config.prefer_frontmatter_config"),
            ),
        )
    }
}
