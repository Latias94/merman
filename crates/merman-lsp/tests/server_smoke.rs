use futures::StreamExt;
use merman_lsp::MermanLanguageServer;
use serde_json::from_value;
use tokio::time::{Duration, timeout};
use tower::{Service, ServiceExt};
use tower_lsp::jsonrpc::Request;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    InitializeParams, PublishDiagnosticsParams, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, VersionedTextDocumentIdentifier,
};

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_initialize() {
    let (service, _socket) = tower_lsp::LspService::new(MermanLanguageServer::new);

    let response =
        tower_lsp::LanguageServer::initialize(service.inner(), InitializeParams::default())
            .await
            .unwrap();

    assert!(response.capabilities.completion_provider.is_some());
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
