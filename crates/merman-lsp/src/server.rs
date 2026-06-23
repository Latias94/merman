use crate::completion::completion_for_snapshot;
use crate::diagnostics::analysis_payload_to_diagnostics;
use crate::document_store::DocumentStore;
use merman_analysis::{
    Analyzer,
    markdown::{analyze_markdown_source, markdown_source_descriptor},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    InitializeParams, InitializeResult, MessageType, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
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

    async fn publish_for_uri(&self, uri: &tower_lsp::lsp_types::Url, version: Option<i32>) {
        let snapshot = {
            let store = self.store.lock().await;
            store.get(uri).cloned()
        };

        let Some(snapshot) = snapshot else {
            return;
        };

        let payload = if snapshot.is_markdown_document() {
            analyze_markdown_source(
                &snapshot.text,
                &self.analyzer,
                markdown_source_descriptor(Some(snapshot.uri.as_str())),
            )
        } else {
            self.analyzer.analyze(&snapshot.text)
        };

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
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions::default()),
                ..ServerCapabilities::default()
            },
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
        let snapshot = {
            let store = self.store.lock().await;
            store.get(&uri).cloned()
        };

        Ok(snapshot
            .map(|snapshot| CompletionResponse::List(completion_for_snapshot(&snapshot, position))))
    }
}
