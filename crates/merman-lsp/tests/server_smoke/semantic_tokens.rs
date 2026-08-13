use std::str::FromStr;

use super::prelude::*;

fn semantic_tokens_initialize_params() -> serde_json::Value {
    serde_json::json!({
        "capabilities": {
            "textDocument": {
                "semanticTokens": {
                    "requests": { "range": true, "full": { "delta": true } },
                    "tokenTypes": [
                        "namespace", "type", "class", "enum", "interface", "struct",
                        "typeParameter", "parameter", "variable", "property", "enumMember",
                        "event", "function", "method", "macro", "keyword", "comment",
                        "string", "number", "regexp", "operator", "decorator", "label"
                    ],
                    "tokenModifiers": [
                        "declaration", "definition", "readonly", "static", "deprecated",
                        "abstract", "async", "modification", "documentation", "defaultLibrary"
                    ],
                    "formats": ["relative"]
                }
            }
        }
    })
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_semantic_tokens_range() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp_server::ls_types::Uri::from_str("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(semantic_tokens_initialize_params())
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
                    end: Position::new(2, 0),
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
async fn lsp_service_rejects_invalid_semantic_token_ranges() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp_server::ls_types::Uri::from_str("file:///tmp/invalid-range.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(semantic_tokens_initialize_params())
        .id(1)
        .finish();
    service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap()
        .expect("initialize response");

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
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("semantic token range response");
    let error = response.error().expect("invalid range error");

    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(
        error
            .message
            .contains("semantic token range end line 10 is outside")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_filters_tokens_outside_the_negotiated_type_subset() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp_server::ls_types::Uri::from_str("file:///tmp/subset.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "semanticTokens": {
                        "requests": { "full": true },
                        "tokenTypes": ["string"],
                        "tokenModifiers": ["mermanPayload"],
                        "formats": ["relative"]
                    }
                }
            }
        }))
        .id(1)
        .finish();
    service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap()
        .expect("initialize response");

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: concat!(
                        "gantt\n",
                        "title Roadmap\n",
                        "section Demo\n",
                        "Task 1: id1,2014-01-01,1d\n",
                    )
                    .to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let request = Request::build("textDocument/semanticTokens/full")
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
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("semantic tokens response");
    let result: SemanticTokensResult =
        serde_json::from_value(response.result().cloned().expect("semantic tokens result"))
            .unwrap();
    let SemanticTokensResult::Tokens(tokens) = result else {
        panic!("expected full semantic tokens")
    };

    assert!(
        tokens.result_id.is_none(),
        "full-only clients do not need a delta baseline identity"
    );
    assert!(!tokens.data.is_empty());
    assert!(tokens.data.iter().all(|token| token.token_type == 0));
    assert!(
        tokens
            .data
            .iter()
            .all(|token| token.token_modifiers_bitset <= 1)
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_modifiers_bitset == 1)
    );

    let delta_request = Request::build("textDocument/semanticTokens/full/delta")
        .params(
            serde_json::to_value(SemanticTokensDeltaParams {
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                text_document: TextDocumentIdentifier { uri },
                previous_result_id: "not-negotiated".to_string(),
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
        .unwrap()
        .expect("delta response");
    assert_eq!(delta_response.result(), Some(&serde_json::Value::Null));
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_semantic_tokens_delta() {
    let (mut service, socket) = MermanLanguageServer::service();
    let (mut socket, _responses) = socket.split();
    let uri = tower_lsp_server::ls_types::Uri::from_str("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(semantic_tokens_initialize_params())
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

    let forged_delta_request = Request::build("textDocument/semanticTokens/full/delta")
        .params(
            serde_json::to_value(SemanticTokensDeltaParams {
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                previous_result_id: "forged".to_string(),
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let forged_delta_response = service
        .ready()
        .await
        .unwrap()
        .call(forged_delta_request)
        .await
        .unwrap();
    let forged_delta_value = forged_delta_response
        .as_ref()
        .and_then(|response| response.result().cloned())
        .expect("expected full fallback for forged semantic token result id");
    let forged_delta: SemanticTokensFullDeltaResult =
        serde_json::from_value(forged_delta_value).unwrap();
    let SemanticTokensFullDeltaResult::Tokens(forged_tokens) = forged_delta else {
        panic!("forged result id should fall back to full semantic tokens");
    };
    assert!(forged_tokens.result_id.is_some());

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
        .id(4)
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
    let (mut service, socket) = MermanLanguageServer::service();
    let (mut socket, _responses) = socket.split();
    let uri = tower_lsp_server::ls_types::Uri::from_str("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(semantic_tokens_initialize_params())
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
                    "site_config": { "theme": "dark" }
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
