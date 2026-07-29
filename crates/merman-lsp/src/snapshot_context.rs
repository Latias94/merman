use crate::analysis_request::AnalysisBuildRequest;
use crate::document_store::{DiagnosticReprojectionBatch, DocumentStore};
use crate::snapshot::SnapshotContext;
use crate::snapshot::{DocumentAnalysisContext, DocumentSnapshot};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Uri;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotContextKind {
    CodeActions,
    Diagnostics,
    SemanticTokens,
    Structure,
}

impl SnapshotContextKind {
    fn requires_analysis_payload(self) -> bool {
        matches!(self, Self::CodeActions | Self::Diagnostics)
    }

    pub(crate) fn stale_error(self) -> tower_lsp_server::jsonrpc::Error {
        let mut error = tower_lsp_server::jsonrpc::Error::content_modified();
        error.message = match self {
            Self::CodeActions => "code action document changed while computing",
            Self::Diagnostics => "diagnostic document changed while computing",
            Self::SemanticTokens => "semantic tokens document changed while computing",
            Self::Structure => "structure document changed while computing",
        }
        .into();
        error
    }
}

pub(crate) async fn snapshot_context_for_uri(
    store: &Arc<Mutex<DocumentStore>>,
    uri: &Uri,
    kind: SnapshotContextKind,
) -> Result<Option<SnapshotContext>> {
    let (reprojection, request, executor) = {
        let mut store = store.lock().await;
        let cache_ready = if kind.requires_analysis_payload() {
            store.has_analysis_payload(uri)
        } else {
            store.has_snapshot(uri)
        };
        if cache_ready {
            return Ok(store.snapshot_context(uri));
        }
        let reprojection = kind
            .requires_analysis_payload()
            .then(|| store.diagnostic_reprojection_request(uri))
            .flatten();
        let request = if reprojection.is_none() {
            store.snapshot_build_request(uri)
        } else {
            None
        };
        (reprojection, request, store.analysis_executor())
    };
    if let Some(reprojection) = reprojection {
        let projection = tokio::task::spawn_blocking(move || reprojection.project())
            .await
            .map_err(diagnostic_reprojection_execution_error)?;
        let batch = match projection {
            Ok(batch) => batch,
            Err(_) => {
                let store = store.lock().await;
                return stale_or_retry(kind, store.get(uri).is_some());
            }
        };
        return commit_diagnostic_reprojection_context(store, uri, batch, kind).await;
    }
    let Some(request) = request else {
        return Ok(None);
    };

    let analysis = executor
        .execute(&request)
        .await
        .map_err(analysis_execution_error)?;
    commit_snapshot_context(store, &request, analysis, kind).await
}

pub(crate) async fn commit_diagnostic_reprojection_context(
    store: &Arc<Mutex<DocumentStore>>,
    uri: &Uri,
    batch: DiagnosticReprojectionBatch,
    kind: SnapshotContextKind,
) -> Result<Option<SnapshotContext>> {
    let mut store = store.lock().await;
    store.commit_diagnostic_reprojection(batch);
    if store.has_analysis_payload(uri) {
        return Ok(store.snapshot_context(uri));
    }
    stale_or_retry(kind, store.get(uri).is_some())
}

fn stale_or_retry(
    kind: SnapshotContextKind,
    document_exists: bool,
) -> Result<Option<SnapshotContext>> {
    if !document_exists || kind == SnapshotContextKind::Diagnostics {
        Ok(None)
    } else {
        Err(kind.stale_error())
    }
}

fn diagnostic_reprojection_execution_error(
    error: tokio::task::JoinError,
) -> tower_lsp_server::jsonrpc::Error {
    let mut response = tower_lsp_server::jsonrpc::Error::internal_error();
    response.message = format!("diagnostic reprojection worker failed: {error}").into();
    response
}

pub(crate) fn analysis_execution_error(
    error: crate::analysis_executor::AnalysisExecutionError,
) -> tower_lsp_server::jsonrpc::Error {
    let mut response = if error.is_stale() {
        tower_lsp_server::jsonrpc::Error::content_modified()
    } else {
        tower_lsp_server::jsonrpc::Error::internal_error()
    };
    response.message = error.to_string().into();
    response
}

pub(crate) async fn commit_snapshot_context(
    store: &Arc<Mutex<DocumentStore>>,
    request: &AnalysisBuildRequest,
    analysis: Arc<DocumentAnalysisContext>,
    kind: SnapshotContextKind,
) -> Result<Option<SnapshotContext>> {
    let mut store = store.lock().await;
    match store.insert_built_analysis(request, analysis) {
        Some(context) => Ok(Some(context)),
        None if store.get(request.uri()).is_some() => Err(kind.stale_error()),
        None => Ok(None),
    }
}

pub(crate) async fn snapshot_result<T>(
    store: &Arc<Mutex<DocumentStore>>,
    uri: &Uri,
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
    let is_current = if kind.requires_analysis_payload() {
        store.is_analysis_context_current(context)
    } else {
        store.is_snapshot_context_current(context)
    };
    if is_current {
        Ok(())
    } else {
        Err(kind.stale_error())
    }
}
