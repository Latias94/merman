use crate::completion::completion_for_snapshot;
use crate::document_store::DocumentStore;
use crate::snapshot::DocumentSnapshot;
use crate::structure::{
    document_symbols as structure_document_symbols, goto_definition as structure_goto_definition,
    hover as structure_hover, prepare_rename as structure_prepare_rename,
    references as structure_references, rename as structure_rename,
};
use merman_analysis::{
    Analyzer,
    document::analyze_document,
    lsp::{analysis_payload_to_diagnostics, uri_is_markdown},
    markdown::markdown_source_descriptor,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, MessageType,
    OneOf, PrepareRenameResponse, ReferenceParams, RenameParams, ServerCapabilities,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, WorkspaceEdit,
};
use tower_lsp::{Client, LanguageServer};

#[derive(Debug)]
pub struct MermanLanguageServer {
    client: Client,
    store: Arc<Mutex<DocumentStore>>,
    analyzer: Analyzer,
}

impl MermanLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            store: Arc::new(Mutex::new(DocumentStore::new())),
            analyzer: Analyzer::new(),
        }
    }

    pub fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions::default()),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        }
    }

    async fn snapshot_for_uri(&self, uri: &tower_lsp::lsp_types::Url) -> Option<DocumentSnapshot> {
        let store = self.store.lock().await;
        store.get(uri).cloned()
    }

    async fn publish_for_uri(&self, uri: &tower_lsp::lsp_types::Url, version: Option<i32>) {
        let snapshot = self.snapshot_for_uri(uri).await;

        let Some(snapshot) = snapshot else {
            return;
        };

        let source = if uri_is_markdown(&snapshot.uri) {
            markdown_source_descriptor(Some(snapshot.uri.as_str()))
        } else {
            merman_analysis::SourceDescriptor::diagram().with_path(snapshot.uri.as_str())
        };
        let payload = analyze_document(&snapshot.text, &self.analyzer, source);

        let diagnostics = analysis_payload_to_diagnostics(&payload, uri);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, version)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for MermanLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: Self::capabilities(),
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: tower_lsp::lsp_types::InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "merman-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.store
            .lock()
            .await
            .upsert(doc.uri.clone(), doc.version, doc.text);
        self.publish_for_uri(&doc.uri, Some(doc.version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let doc = params.text_document;
        let mut store = self.store.lock().await;
        let Some(current) = store.get(&doc.uri).cloned() else {
            return;
        };

        let mut text = current.text;
        for change in params.content_changes {
            text = change.text;
        }
        store.upsert(doc.uri.clone(), doc.version, text);
        drop(store);
        self.publish_for_uri(&doc.uri, Some(doc.version)).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = {
            let store = self.store.lock().await;
            store.get(&uri).map(|doc| doc.version)
        };
        self.publish_for_uri(&uri, version).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.store.lock().await.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot
            .map(|snapshot| CompletionResponse::List(completion_for_snapshot(&snapshot, position))))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| structure_hover(&snapshot, position)))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.map(|snapshot| structure_document_symbols(&snapshot)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| structure_goto_definition(&snapshot, position)))
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| {
            structure_references(&snapshot, position, params.context.include_declaration)
        }))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| structure_prepare_rename(&snapshot, position)))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let snapshot = self.snapshot_for_uri(&uri).await;

        match snapshot {
            Some(snapshot) => structure_rename(&snapshot, params),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MermanLanguageServer;
    use crate::document_store::DocumentStore;
    use crate::structure::{
        document_symbols, goto_definition, hover, prepare_rename, references, rename,
    };
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::{
        DocumentSymbolResponse, GotoDefinitionResponse, HoverContents, HoverParams, Position,
        RenameParams, TextDocumentIdentifier, TextDocumentPositionParams,
        TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    };
    use tower_lsp::lsp_types::{HoverProviderCapability, OneOf};

    #[test]
    fn capabilities_advertise_completion_and_full_sync() {
        let capabilities = MermanLanguageServer::capabilities();

        assert!(matches!(
            capabilities.text_document_sync,
            Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
        ));
        assert!(matches!(
            capabilities.hover_provider,
            Some(HoverProviderCapability::Simple(true))
        ));
        assert!(matches!(
            capabilities.document_symbol_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.definition_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.references_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.rename_provider,
            Some(OneOf::Left(true))
        ));
        assert!(capabilities.completion_provider.is_some());
    }

    #[test]
    fn structure_helpers_produce_hover_and_nested_symbols() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri.clone(),
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        );

        let hover = hover(&snapshot, Position::new(1, 0)).unwrap();
        let text = match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            other => panic!("unexpected hover contents: {other:?}"),
        };
        assert!(text.contains("group") || text.contains("diagram element"));

        let symbols = match document_symbols(&snapshot) {
            DocumentSymbolResponse::Nested(symbols) => symbols,
            other => panic!("unexpected symbol response: {other:?}"),
        };
        assert_eq!(symbols.len(), 1);
        assert!(
            symbols[0]
                .children
                .as_ref()
                .unwrap()
                .iter()
                .any(|symbol| symbol.name == "group")
        );
    }

    #[test]
    fn structure_helpers_cover_navigation_surface() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\nA-->C\n".to_string());
        let position = Position::new(1, 0);

        assert!(matches!(
            goto_definition(&snapshot, position),
            Some(GotoDefinitionResponse::Scalar(_))
        ));
        assert_eq!(references(&snapshot, position, true).unwrap().len(), 2);
        assert!(prepare_rename(&snapshot, position).is_some());
        let rename = rename(
            &snapshot,
            RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri },
                    position,
                ),
                new_name: "X".to_string(),
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();
        assert_eq!(
            rename
                .unwrap()
                .changes
                .unwrap()
                .values()
                .next()
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_handlers_return_hover_and_symbols() {
        let (service, _socket) = tower_lsp::LspService::new(MermanLanguageServer::new);
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        {
            let mut store = server.store.lock().await;
            store.upsert(
                uri.clone(),
                1,
                "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
            );
        }

        let hover = server
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
            })
            .await
            .unwrap();
        assert!(hover.is_some());

        let document_symbols = server
            .document_symbol(tower_lsp::lsp_types::DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();
        assert!(matches!(
            document_symbols,
            Some(DocumentSymbolResponse::Nested(_))
        ));
    }
}
