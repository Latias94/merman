use crate::document_store::{DocumentStore, SnapshotBuildRequest, SnapshotContext};
use crate::snapshot::DocumentSnapshot;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotContextKind {
    SemanticTokens,
    Structure,
    WorkspaceSymbols,
}

impl SnapshotContextKind {
    pub(crate) fn stale_error(self) -> tower_lsp::jsonrpc::Error {
        let mut error = tower_lsp::jsonrpc::Error::content_modified();
        error.message = match self {
            Self::SemanticTokens => "semantic tokens document changed while computing",
            Self::Structure => "structure document changed while computing",
            Self::WorkspaceSymbols => "workspace symbol documents changed while computing",
        }
        .into();
        error
    }
}

pub(crate) async fn snapshot_context_for_uri(
    store: &Arc<Mutex<DocumentStore>>,
    uri: &Url,
    kind: SnapshotContextKind,
) -> Result<Option<SnapshotContext>> {
    let request = {
        let mut store = store.lock().await;
        if store.has_snapshot(uri) {
            return Ok(store.snapshot_context(uri));
        }
        store.snapshot_build_request(uri)
    };
    let Some(request) = request else {
        return Ok(None);
    };

    let snapshot = request.build();
    commit_snapshot_context(store, &request, snapshot, kind).await
}

pub(crate) async fn commit_snapshot_context(
    store: &Arc<Mutex<DocumentStore>>,
    request: &SnapshotBuildRequest,
    snapshot: Arc<DocumentSnapshot>,
    kind: SnapshotContextKind,
) -> Result<Option<SnapshotContext>> {
    let mut store = store.lock().await;
    match store.insert_built_snapshot(request, snapshot) {
        Some(context) => Ok(Some(context)),
        None if store.get(request.uri()).is_some() => Err(kind.stale_error()),
        None => Ok(None),
    }
}

pub(crate) async fn snapshot_result<T>(
    store: &Arc<Mutex<DocumentStore>>,
    uri: &Url,
    kind: SnapshotContextKind,
    compute: impl FnOnce(&DocumentSnapshot) -> Result<Option<T>>,
) -> Result<Option<T>> {
    let Some(context) = snapshot_context_for_uri(store, uri, kind).await? else {
        return Ok(None);
    };

    let result = compute(&context.snapshot);
    ensure_snapshot_current(store, &context, kind).await?;
    result
}

pub(crate) async fn ensure_snapshot_current(
    store: &Arc<Mutex<DocumentStore>>,
    context: &SnapshotContext,
    kind: SnapshotContextKind,
) -> Result<()> {
    let store = store.lock().await;
    if store.is_snapshot_context_current(context) {
        Ok(())
    } else {
        Err(kind.stale_error())
    }
}
