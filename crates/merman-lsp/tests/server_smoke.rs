use merman_lsp::MermanLanguageServer;
use tower_lsp::lsp_types::InitializeParams;

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
