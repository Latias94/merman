use futures::StreamExt;
use merman_lsp::MermanLanguageServer;
use serde_json::from_value;
use tokio::time::{Duration, timeout};
use tower::{Service, ServiceExt};
use tower_lsp::jsonrpc::Request;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentSymbolParams, GotoDefinitionParams, HoverContents, HoverParams, InitializeParams,
    Position, PrepareRenameResponse, PublishDiagnosticsParams, ReferenceContext, ReferenceParams,
    RenameParams, TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, VersionedTextDocumentIdentifier,
};

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_initialize() {
    let (service, _socket) = tower_lsp::LspService::new(MermanLanguageServer::new);

    let response =
        tower_lsp::LanguageServer::initialize(service.inner(), InitializeParams::default())
            .await
            .unwrap();

    assert!(response.capabilities.completion_provider.is_some());
    assert!(response.capabilities.code_action_provider.is_some());
    assert!(response.capabilities.semantic_tokens_provider.is_some());
    assert!(matches!(
        MermanLanguageServer::capabilities().text_document_sync,
        Some(tower_lsp::lsp_types::TextDocumentSyncCapability::Kind(
            tower_lsp::lsp_types::TextDocumentSyncKind::FULL
        ))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_publishes_current_diagnostics_version() {
    let (mut service, mut socket) = tower_lsp::LspService::new(MermanLanguageServer::new);
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
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
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let first = socket.next().await.expect("expected diagnostics publish");
    let first_params: PublishDiagnosticsParams =
        from_value(first.params().cloned().expect("publish params")).unwrap();
    assert_eq!(first.method(), "textDocument/publishDiagnostics");
    assert_eq!(first_params.version, Some(1));
    assert!(!first_params.diagnostics.is_empty());

    let change = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "flowchart TD\nA[Hello] --> B[World]\n".to_string(),
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );

    let second = socket.next().await.expect("expected updated diagnostics");
    let second_params: PublishDiagnosticsParams =
        from_value(second.params().cloned().expect("publish params")).unwrap();
    assert_eq!(second.method(), "textDocument/publishDiagnostics");
    assert_eq!(second_params.version, Some(2));
    assert!(second_params.diagnostics.is_empty());

    let save = Request::build("textDocument/didSave")
        .params(
            serde_json::to_value(DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                text: None,
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(save).await.unwrap(),
        None
    );

    let third = socket.next().await.expect("expected save diagnostics");
    let third_params: PublishDiagnosticsParams =
        from_value(third.params().cloned().expect("publish params")).unwrap();
    assert_eq!(third.method(), "textDocument/publishDiagnostics");
    assert_eq!(third_params.version, Some(2));
    assert!(third_params.diagnostics.is_empty());

    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err(),
        "unexpected extra publishDiagnostics message"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_hover_and_document_symbols() {
    let (mut service, _socket) = tower_lsp::LspService::new(MermanLanguageServer::new);
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
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
                    text: "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let hover = Request::build("textDocument/hover")
        .params(
            serde_json::to_value(HoverParams {
                text_document_position_params: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let hover_response = service.ready().await.unwrap().call(hover).await.unwrap();
    let hover_value = hover_response
        .and_then(|response| response.result().cloned())
        .expect("expected hover result");
    let hover: tower_lsp::lsp_types::Hover = serde_json::from_value(hover_value).unwrap();
    let hover_text = match hover.contents {
        HoverContents::Markup(markup) => markup.value,
        other => panic!("unexpected hover contents: {other:?}"),
    };
    assert!(hover_text.contains("group"));

    let document_symbol = Request::build("textDocument/documentSymbol")
        .params(
            serde_json::to_value(DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let document_symbol_response = service
        .ready()
        .await
        .unwrap()
        .call(document_symbol)
        .await
        .unwrap();
    let document_symbol_value = document_symbol_response
        .and_then(|response| response.result().cloned())
        .expect("expected document symbol result");
    let symbols: tower_lsp::lsp_types::DocumentSymbolResponse =
        serde_json::from_value(document_symbol_value).unwrap();
    assert!(matches!(
        symbols,
        tower_lsp::lsp_types::DocumentSymbolResponse::Nested(_)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_navigation_requests() {
    let (mut service, _socket) = tower_lsp::LspService::new(MermanLanguageServer::new);
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
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
                    text: "flowchart TD\nA-->B\nA-->C\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let definition = Request::build("textDocument/definition")
        .params(
            serde_json::to_value(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let definition_response = service
        .ready()
        .await
        .unwrap()
        .call(definition)
        .await
        .unwrap();
    let definition_value = definition_response
        .and_then(|response| response.result().cloned())
        .expect("expected definition result");
    let definition: tower_lsp::lsp_types::GotoDefinitionResponse =
        serde_json::from_value(definition_value).unwrap();
    assert!(matches!(
        definition,
        tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(_)
    ));

    let references = Request::build("textDocument/references")
        .params(
            serde_json::to_value(ReferenceParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: ReferenceContext {
                    include_declaration: true,
                },
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let references_response = service
        .ready()
        .await
        .unwrap()
        .call(references)
        .await
        .unwrap();
    let references_value = references_response
        .and_then(|response| response.result().cloned())
        .expect("expected references result");
    let locations: Vec<tower_lsp::lsp_types::Location> =
        serde_json::from_value(references_value).unwrap();
    assert_eq!(locations.len(), 2);

    let prepare = Request::build("textDocument/prepareRename")
        .params(
            serde_json::to_value(TextDocumentPositionParams::new(
                TextDocumentIdentifier { uri: uri.clone() },
                Position::new(1, 0),
            ))
            .unwrap(),
        )
        .id(4)
        .finish();
    let prepare_response = service.ready().await.unwrap().call(prepare).await.unwrap();
    let prepare_value = prepare_response
        .and_then(|response| response.result().cloned())
        .expect("expected prepare rename result");
    let prepare: PrepareRenameResponse = serde_json::from_value(prepare_value).unwrap();
    assert!(matches!(
        prepare,
        PrepareRenameResponse::RangeWithPlaceholder { .. }
    ));

    let rename = Request::build("textDocument/rename")
        .params(
            serde_json::to_value(RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri },
                    Position::new(1, 0),
                ),
                new_name: "X".to_string(),
                work_done_progress_params: Default::default(),
            })
            .unwrap(),
        )
        .id(5)
        .finish();
    let rename_response = service.ready().await.unwrap().call(rename).await.unwrap();
    let rename_value = rename_response
        .and_then(|response| response.result().cloned())
        .expect("expected rename result");
    let edit: tower_lsp::lsp_types::WorkspaceEdit = serde_json::from_value(rename_value).unwrap();
    assert_eq!(edit.changes.unwrap().values().next().unwrap().len(), 2);
}
