use super::LanguageSession;
use super::documents::DiagnosticContext;
use crate::snapshot::{DocumentSnapshot, SnapshotContext};
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Uri;

pub(in crate::session) mod acquisition;
pub(super) mod executor;
pub(super) mod request;

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

    pub(crate) async fn query_push_diagnostics<T>(
        &self,
        context: &DiagnosticContext,
        compute: impl FnOnce(&DiagnosticContext, Option<&SnapshotContext>) -> T,
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
        let result = compute(context, analysis_context.as_ref());
        let state = self.inner.state.lock().await;
        let current = state.diagnostic_contexts_are_current(context, analysis_context.as_ref());
        Ok(current.then_some(result))
    }
}
