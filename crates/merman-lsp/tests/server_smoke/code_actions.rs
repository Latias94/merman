use super::prelude::*;

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_reports_deprecated_flowchart_html_labels_without_quickfix() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "codeAction": {
                        "codeActionLiteralSupport": {
                            "codeActionKind": { "valueSet": ["quickfix"] }
                        }
                    },
                    "publishDiagnostics": { "dataSupport": true }
                },
                "workspace": {
                    "workspaceEdit": {
                        "documentChanges": true
                    }
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "%%{init: { \"flowchart\": { \"htmlLabels\": false, \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let publish = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics publish");
    assert_eq!(publish.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams =
        from_value(publish.params().cloned().expect("publish params")).unwrap();
    assert_eq!(params.diagnostics.len(), 1);
    let diagnostic = params.diagnostics[0].clone();
    assert_eq!(
        diagnostic.severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING)
    );
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String(
            "merman.compatibility.config.deprecated_flowchart_html_labels".to_string()
        ))
    );
    let request = Request::build("textDocument/codeAction")
        .params(
            serde_json::to_value(CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 80),
                },
                context: CodeActionContext {
                    diagnostics: vec![diagnostic],
                    only: Some(vec![CodeActionKind::QUICKFIX]),
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("code action response");
    let result = response.result().expect("code action result");
    assert!(result.is_null());
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_returns_negotiated_quickfix_with_versioned_edit() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/quickfix.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "codeAction": {
                        "codeActionLiteralSupport": {
                            "codeActionKind": { "valueSet": ["quickfix"] }
                        },
                        "isPreferredSupport": true
                    },
                    "publishDiagnostics": { "dataSupport": true }
                },
                "workspace": {
                    "workspaceEdit": { "documentChanges": true }
                }
            },
            "initializationOptions": {
                "lint": { "profile": "recommended" }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 7,
                    text: "flowchart\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let publish = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics publish");
    let params: PublishDiagnosticsParams =
        from_value(publish.params().cloned().expect("publish params")).unwrap();
    let diagnostic = params
        .diagnostics
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.authoring.flowchart.explicit_direction".to_string(),
                ))
        })
        .expect("expected missing flowchart direction diagnostic");
    assert!(diagnostic.data.is_some());

    let request = Request::build("textDocument/codeAction")
        .params(
            serde_json::to_value(CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: diagnostic.range,
                context: CodeActionContext {
                    diagnostics: vec![diagnostic],
                    only: Some(vec![CodeActionKind::QUICKFIX]),
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("code action response");
    let actions: Vec<CodeActionOrCommand> =
        from_value(response.result().cloned().expect("code action result")).unwrap();
    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Insert `TB` into the flowchart header" =>
            {
                Some(action)
            }
            _ => None,
        })
        .expect("expected flowchart direction quickfix");

    assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
    assert_eq!(action.is_preferred, Some(true));
    let edit = action.edit.as_ref().expect("quickfix workspace edit");
    assert!(edit.changes.is_none());
    let DocumentChanges::Edits(document_edits) =
        edit.document_changes.as_ref().expect("document changes")
    else {
        panic!("expected versioned text document edits")
    };
    assert_eq!(document_edits.len(), 1);
    assert_eq!(document_edits[0].text_document.uri, uri);
    assert_eq!(document_edits[0].text_document.version, Some(7));
    assert_eq!(document_edits[0].edits.len(), 1);
}
