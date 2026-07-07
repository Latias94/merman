#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_semantic_tokens_range() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities": {}}))
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
                    text: "flowchart TD\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let request = Request::build("textDocument/semanticTokens/range")
        .params(
            serde_json::to_value(SemanticTokensRangeParams {
                text_document: TextDocumentIdentifier { uri },
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(10, 0),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let response = service.ready().await.unwrap().call(request).await.unwrap();
    let value = response
        .as_ref()
        .and_then(|response| response.result().cloned())
        .expect("expected semantic tokens range result");
    let _: SemanticTokensRangeResult = serde_json::from_value(value.clone()).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_semantic_tokens_delta() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities": {}}))
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
                    text: "flowchart TD\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );
    let first_diagnostics = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics after open");
    assert_eq!(
        first_diagnostics.method(),
        "textDocument/publishDiagnostics"
    );

    let full_request = Request::build("textDocument/semanticTokens/full")
        .params(
            serde_json::to_value(SemanticTokensParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let full_response = service
        .ready()
        .await
        .unwrap()
        .call(full_request)
        .await
        .unwrap();
    let full_value = full_response
        .as_ref()
        .and_then(|response| response.result().cloned())
        .expect("expected semantic tokens full result");
    let full_result: SemanticTokensResult = serde_json::from_value(full_value.clone()).unwrap();
    let previous_result_id = match full_result {
        SemanticTokensResult::Tokens(tokens) => tokens
            .result_id
            .expect("expected semantic tokens result id"),
        other => panic!("unexpected semantic tokens full result: {other:?}"),
    };

    let change = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    text: "flowchart TD\nAlpha-->B\n".to_string(),
                    range: None,
                    range_length: None,
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );
    let second_diagnostics = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics after change");
    assert_eq!(
        second_diagnostics.method(),
        "textDocument/publishDiagnostics"
    );

    let delta_request = Request::build("textDocument/semanticTokens/full/delta")
        .params(
            serde_json::to_value(SemanticTokensDeltaParams {
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                text_document: TextDocumentIdentifier { uri },
                previous_result_id,
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let delta_response = service
        .ready()
        .await
        .unwrap()
        .call(delta_request)
        .await
        .unwrap();
    let delta_value = delta_response
        .as_ref()
        .and_then(|response| response.result().cloned())
        .expect("expected semantic tokens delta result");
    let delta_result: SemanticTokensFullDeltaResult =
        serde_json::from_value(delta_value.clone()).unwrap();
    match delta_result {
        SemanticTokensFullDeltaResult::TokensDelta(delta) => {
            assert!(delta.result_id.is_some());
            assert!(!delta.edits.is_empty());
        }
        other => panic!("unexpected semantic tokens delta result: {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn lsp_service_semantic_tokens_delta_falls_back_to_full_after_snapshot_configuration_change()
{
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities": {}}))
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
                    text: "flowchart TD\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );
    let first_diagnostics = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics after open");
    assert_eq!(
        first_diagnostics.method(),
        "textDocument/publishDiagnostics"
    );

    let full_request = Request::build("textDocument/semanticTokens/full")
        .params(
            serde_json::to_value(SemanticTokensParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let full_response = service
        .ready()
        .await
        .unwrap()
        .call(full_request)
        .await
        .unwrap();
    let full_value = full_response
        .as_ref()
        .and_then(|response| response.result().cloned())
        .expect("expected semantic tokens full result");
    let full_result: SemanticTokensResult = serde_json::from_value(full_value.clone()).unwrap();
    let previous_result_id = match full_result {
        SemanticTokensResult::Tokens(tokens) => tokens
            .result_id
            .expect("expected semantic tokens result id"),
        other => panic!("unexpected semantic tokens full result: {other:?}"),
    };

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "parse": {
                        "suppress_errors": true
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );
    let refreshed_diagnostics = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics after configuration change");
    assert_eq!(
        refreshed_diagnostics.method(),
        "textDocument/publishDiagnostics"
    );

    let delta_request = Request::build("textDocument/semanticTokens/full/delta")
        .params(
            serde_json::to_value(SemanticTokensDeltaParams {
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                text_document: TextDocumentIdentifier { uri },
                previous_result_id,
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let delta_response = service
        .ready()
        .await
        .unwrap()
        .call(delta_request)
        .await
        .unwrap();
    let delta_value = delta_response
        .as_ref()
        .and_then(|response| response.result().cloned())
        .expect("expected semantic tokens delta result");
    let delta_result: SemanticTokensFullDeltaResult =
        serde_json::from_value(delta_value.clone()).unwrap();
    match delta_result {
        SemanticTokensFullDeltaResult::Tokens(tokens) => {
            assert!(tokens.result_id.is_some());
            assert!(!tokens.data.is_empty());
        }
        other => panic!("unexpected semantic tokens delta result: {other:?}"),
    }
}
