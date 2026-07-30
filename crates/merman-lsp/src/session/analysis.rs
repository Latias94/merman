use self::executor::DiagnosticReprojectionLease;
use super::LanguageSession;
use super::documents::{
    DiagnosticContext, DocumentDiagnosticState, SemanticTokensState, StoredDocument,
};
use crate::snapshot::{DocumentSnapshot, SnapshotContext};
use merman_analysis::AnalysisPayload;
use std::sync::Arc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Uri;

pub(super) mod executor;
pub(super) mod request;

const MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotPurpose {
    CodeActions,
    Diagnostics,
    SemanticTokens,
    Structure,
}

impl SnapshotPurpose {
    fn requires_analysis_payload(self) -> bool {
        matches!(self, Self::CodeActions | Self::Diagnostics)
    }

    fn stale_error(self) -> tower_lsp_server::jsonrpc::Error {
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

impl LanguageSession {
    pub(crate) async fn query_structure<T>(
        &self,
        uri: &Uri,
        compute: impl FnOnce(&DocumentSnapshot) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let Some(context) = self
            .acquire_snapshot_context(uri, SnapshotPurpose::Structure)
            .await?
        else {
            return Ok(None);
        };
        let result = compute(&context.snapshot);
        self.ensure_current(&context, SnapshotPurpose::Structure)
            .await?;
        result
    }

    #[cfg(test)]
    pub(crate) async fn structure_snapshot(&self, uri: &Uri) -> Option<Arc<DocumentSnapshot>> {
        self.acquire_snapshot_context(uri, SnapshotPurpose::Structure)
            .await
            .ok()
            .flatten()
            .map(|context| context.snapshot)
    }

    pub(crate) async fn query_code_actions<T>(
        &self,
        uri: &Uri,
        compute: impl FnOnce(&SnapshotContext) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let Some(context) = self
            .acquire_snapshot_context(uri, SnapshotPurpose::CodeActions)
            .await?
        else {
            return Ok(None);
        };
        let result = compute(&context);
        self.ensure_current(&context, SnapshotPurpose::CodeActions)
            .await?;
        result
    }

    pub(crate) async fn query_semantic_tokens<T>(
        &self,
        uri: &Uri,
        previous_result_id: Option<&str>,
        compute: impl FnOnce(
            &DocumentSnapshot,
            Option<SemanticTokensState>,
        ) -> Result<Option<(T, Option<SemanticTokensState>)>>,
    ) -> Result<Option<T>> {
        let Some(context) = self
            .acquire_snapshot_context(uri, SnapshotPurpose::SemanticTokens)
            .await?
        else {
            return Ok(None);
        };
        let previous = match previous_result_id {
            Some(result_id) => self
                .state
                .lock()
                .await
                .semantic_tokens_state_for_delta(uri, result_id),
            None => None,
        };
        let Some((result, next_state)) = compute(&context.snapshot, previous)? else {
            return Ok(None);
        };

        let mut state = self.state.lock().await;
        let current = match next_state {
            Some(next_state) => state.set_semantic_tokens_state_if_current(&context, next_state),
            None => state.is_snapshot_context_current(&context),
        };
        if current {
            Ok(Some(result))
        } else {
            Err(SnapshotPurpose::SemanticTokens.stale_error())
        }
    }

    pub(crate) async fn pull_diagnostics(
        &self,
        uri: &Uri,
        compute: impl Fn(&StoredDocument, Option<&AnalysisPayload>) -> DocumentDiagnosticState,
    ) -> Result<Option<DocumentDiagnosticState>> {
        for _ in 0..MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS {
            let (diagnostic_context, cached) = {
                let state = self.state.lock().await;
                (state.diagnostic_context(uri), state.diagnostic_state(uri))
            };
            if let Some(cached) = cached {
                return Ok(Some(cached));
            }
            let Some(diagnostic_context) = diagnostic_context else {
                return Ok(None);
            };

            let analysis_context = if diagnostic_context.document.has_unavailable_source() {
                None
            } else {
                self.acquire_snapshot_context(uri, SnapshotPurpose::Diagnostics)
                    .await?
            };
            if !diagnostic_context.document.has_unavailable_source() && analysis_context.is_none() {
                continue;
            }

            let state_value = compute(
                &diagnostic_context.document,
                analysis_context
                    .as_ref()
                    .map(SnapshotContext::analysis_payload),
            );
            let mut state = self.state.lock().await;
            let current = state.is_diagnostic_context_current(&diagnostic_context)
                && analysis_context
                    .as_ref()
                    .is_none_or(|context| state.is_analysis_context_current(context));
            if current
                && state.set_diagnostic_state_if_current(&diagnostic_context, state_value.clone())
            {
                return Ok(Some(state_value));
            }
        }

        Err(stale_diagnostic_error())
    }

    pub(crate) async fn query_push_diagnostics<T>(
        &self,
        context: &DiagnosticContext,
        compute: impl FnOnce(&StoredDocument, Option<&AnalysisPayload>) -> T,
    ) -> Result<Option<T>> {
        let analysis_context = if context.document.has_unavailable_source() {
            None
        } else {
            self.acquire_snapshot_context(&context.document.uri, SnapshotPurpose::Diagnostics)
                .await?
        };
        if !context.document.has_unavailable_source() && analysis_context.is_none() {
            return Ok(None);
        }
        let result = compute(
            &context.document,
            analysis_context
                .as_ref()
                .map(SnapshotContext::analysis_payload),
        );
        let state = self.state.lock().await;
        let current = state.is_diagnostic_context_current(context)
            && analysis_context
                .as_ref()
                .is_none_or(|context| state.is_analysis_context_current(context));
        Ok(current.then_some(result))
    }

    async fn acquire_snapshot_context(
        &self,
        uri: &Uri,
        purpose: SnapshotPurpose,
    ) -> Result<Option<SnapshotContext>> {
        let (reprojection, request, executor) = {
            let mut state = self.state.lock().await;
            let cache_ready = if purpose.requires_analysis_payload() {
                state.has_analysis_payload(uri)
            } else {
                state.has_snapshot(uri)
            };
            if cache_ready {
                return Ok(state.cached_snapshot_context_for_uri(uri));
            }
            let reprojection = purpose
                .requires_analysis_payload()
                .then(|| state.diagnostic_reprojection_request(uri))
                .flatten();
            let request = if reprojection.is_none() {
                state.snapshot_build_request(uri)
            } else {
                None
            };
            (reprojection, request, state.analysis_executor())
        };

        if let Some(reprojection) = reprojection {
            let projection = match executor
                .execute_diagnostic_reprojection(&reprojection)
                .await
            {
                Ok(projection) => projection,
                Err(error) if error.is_stale() => {
                    return self.stale_or_missing(uri, purpose).await;
                }
                Err(error) => return Err(analysis_execution_error(error)),
            };
            let result = self
                .commit_diagnostic_reprojection_context(&projection, purpose)
                .await;
            drop(projection);
            return result;
        }

        let Some(request) = request else {
            return Ok(None);
        };
        let analysis = match executor.execute(&request).await {
            Ok(analysis) => analysis,
            Err(error) if error.is_stale() => {
                return self.stale_or_missing(uri, purpose).await;
            }
            Err(error) => return Err(analysis_execution_error(error)),
        };
        let context = Arc::clone(analysis.context());
        let mut state = self.state.lock().await;
        match state.insert_built_analysis(&request, context) {
            Some(context) => Ok(Some(context)),
            None if state.get(request.uri()).is_some() => match purpose {
                SnapshotPurpose::Diagnostics => Ok(None),
                _ => Err(purpose.stale_error()),
            },
            None => Ok(None),
        }
    }

    async fn commit_diagnostic_reprojection_context(
        &self,
        projection: &DiagnosticReprojectionLease,
        purpose: SnapshotPurpose,
    ) -> Result<Option<SnapshotContext>> {
        let uri = projection.uri();
        let mut state = self.state.lock().await;
        if let Some(context) = state.commit_diagnostic_reprojection_context(projection) {
            return Ok(Some(context));
        }
        if state.get(uri).is_none() || purpose == SnapshotPurpose::Diagnostics {
            Ok(None)
        } else {
            Err(purpose.stale_error())
        }
    }

    async fn stale_or_missing(
        &self,
        uri: &Uri,
        purpose: SnapshotPurpose,
    ) -> Result<Option<SnapshotContext>> {
        let document_exists = self.state.lock().await.get(uri).is_some();
        if !document_exists || purpose == SnapshotPurpose::Diagnostics {
            Ok(None)
        } else {
            Err(purpose.stale_error())
        }
    }

    async fn ensure_current(
        &self,
        context: &SnapshotContext,
        purpose: SnapshotPurpose,
    ) -> Result<()> {
        let state = self.state.lock().await;
        let current = if purpose.requires_analysis_payload() {
            state.is_analysis_context_current(context)
        } else {
            state.is_snapshot_context_current(context)
        };
        if current {
            Ok(())
        } else {
            Err(purpose.stale_error())
        }
    }
}

fn analysis_execution_error(
    error: executor::AnalysisExecutionError,
) -> tower_lsp_server::jsonrpc::Error {
    let mut response = if error.is_stale() {
        tower_lsp_server::jsonrpc::Error::content_modified()
    } else {
        tower_lsp_server::jsonrpc::Error::internal_error()
    };
    response.message = error.to_string().into();
    response
}

fn stale_diagnostic_error() -> tower_lsp_server::jsonrpc::Error {
    let mut error = tower_lsp_server::jsonrpc::Error::content_modified();
    error.message = "diagnostic document changed repeatedly while computing".into();
    error
}
