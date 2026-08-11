use super::LanguageSession;
use super::documents::{DiagnosticContext, SemanticTokensState, StoredDocument};
use crate::snapshot::{DocumentSnapshot, SnapshotContext};
use std::sync::Arc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Uri;

pub(in crate::session) mod acquisition;
pub(super) mod executor;
pub(super) mod request;

use acquisition::semantic_tokens_stale_error;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(in crate::session) struct AnalysisJobGeneration(pub(in crate::session) u64);

impl LanguageSession {
    pub(crate) async fn query_structure<T>(
        &self,
        uri: &Uri,
        compute: impl FnOnce(&DocumentSnapshot) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let Some(snapshot) = self.acquire_structure_snapshot(uri).await? else {
            return Ok(None);
        };
        let result = compute(&snapshot.snapshot);
        self.ensure_structure_snapshot_current(&snapshot).await?;
        result
    }

    pub(crate) async fn query_code_actions<T>(
        &self,
        uri: &Uri,
        compute: impl FnOnce(&SnapshotContext) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let Some(context) = self.acquire_code_action_context(uri).await? else {
            return Ok(None);
        };
        let result = compute(&context);
        self.ensure_code_action_context_current(&context).await?;
        result
    }

    pub(crate) async fn query_semantic_tokens<T>(
        &self,
        uri: &Uri,
        previous_result_id: Option<&str>,
        compute: impl FnOnce(
            &DocumentSnapshot,
            Option<Arc<SemanticTokensState>>,
        ) -> Result<Option<(T, Option<SemanticTokensState>)>>,
    ) -> Result<Option<T>> {
        let Some(snapshot) = self.acquire_semantic_token_snapshot(uri).await? else {
            return Ok(None);
        };
        let previous = match previous_result_id {
            Some(result_id) => self
                .inner
                .state
                .lock()
                .await
                .semantic_tokens_state_for_delta(uri, result_id),
            None => None,
        };
        let computed = compute(&snapshot.snapshot, previous);

        let mut state = self.inner.state.lock().await;
        self.commit_state_if_active(&mut state, |state| {
            if !state.is_acquired_snapshot_current(&snapshot) {
                return Err(semantic_tokens_stale_error());
            }
            match computed {
                Ok(Some((result, Some(next_state)))) => {
                    if state.set_semantic_tokens_state_if_snapshot_current(&snapshot, next_state) {
                        Ok(Some(result))
                    } else {
                        Err(semantic_tokens_stale_error())
                    }
                }
                Ok(Some((result, None))) => Ok(Some(result)),
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            }
        })
        .unwrap_or(Ok(None))
    }

    pub(crate) async fn query_push_diagnostics<T>(
        &self,
        context: &DiagnosticContext,
        compute: impl FnOnce(&StoredDocument, Option<&SnapshotContext>) -> T,
    ) -> Result<Option<T>> {
        let analysis_context = if context.document.is_analysis_unavailable() {
            None
        } else {
            self.acquire_push_diagnostic_context(&context.document.uri)
                .await?
        };
        if !context.document.is_analysis_unavailable() && analysis_context.is_none() {
            return Ok(None);
        }
        let result = compute(&context.document, analysis_context.as_ref());
        let state = self.inner.state.lock().await;
        let current = state.diagnostic_contexts_are_current(context, analysis_context.as_ref());
        Ok(current.then_some(result))
    }
}
