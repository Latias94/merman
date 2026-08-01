use self::executor::{AnalysisExecutor, DiagnosticProjectionOrigin};
use super::LanguageSession;
use super::documents::{
    DiagnosticContext, DiagnosticProjectionPreparation, DocumentDiagnosticState,
    SemanticTokensState, SnapshotLease, StoredDocument,
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
        let Some(snapshot) = self
            .acquire_snapshot(uri, SnapshotPurpose::Structure)
            .await?
        else {
            return Ok(None);
        };
        let result = compute(&snapshot.snapshot);
        self.ensure_snapshot_current(&snapshot, SnapshotPurpose::Structure)
            .await?;
        result
    }

    #[cfg(test)]
    pub(crate) async fn structure_snapshot(&self, uri: &Uri) -> Option<Arc<DocumentSnapshot>> {
        self.acquire_snapshot(uri, SnapshotPurpose::Structure)
            .await
            .ok()
            .flatten()
            .map(|snapshot| snapshot.snapshot)
    }

    pub(crate) async fn query_code_actions<T>(
        &self,
        uri: &Uri,
        compute: impl FnOnce(&SnapshotContext) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        let Some(context) = self
            .acquire_analysis_context(uri, SnapshotPurpose::CodeActions)
            .await?
        else {
            return Ok(None);
        };
        let result = compute(&context);
        self.ensure_analysis_current(&context, SnapshotPurpose::CodeActions)
            .await?;
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
        let Some(snapshot) = self
            .acquire_snapshot(uri, SnapshotPurpose::SemanticTokens)
            .await?
        else {
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
            if !state.is_snapshot_lease_current(&snapshot) {
                return Err(SnapshotPurpose::SemanticTokens.stale_error());
            }
            match computed {
                Ok(Some((result, Some(next_state)))) => {
                    if state.set_semantic_tokens_state_if_snapshot_current(&snapshot, next_state) {
                        Ok(Some(result))
                    } else {
                        Err(SnapshotPurpose::SemanticTokens.stale_error())
                    }
                }
                Ok(Some((result, None))) => Ok(Some(result)),
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            }
        })
        .unwrap_or(Ok(None))
    }

    pub(crate) async fn pull_diagnostics(
        &self,
        uri: &Uri,
        compute: impl Fn(&StoredDocument, Option<&AnalysisPayload>) -> DocumentDiagnosticState,
    ) -> Result<Option<DocumentDiagnosticState>> {
        for _ in 0..MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS {
            let (diagnostic_context, cached) = {
                let state = self.inner.state.lock().await;
                (state.diagnostic_context(uri), state.diagnostic_state(uri))
            };
            if let Some(cached) = cached {
                return Ok(Some(cached));
            }
            let Some(diagnostic_context) = diagnostic_context else {
                return Ok(None);
            };

            let analysis_context = if diagnostic_context.document.is_analysis_unavailable() {
                None
            } else {
                self.acquire_analysis_context(uri, SnapshotPurpose::Diagnostics)
                    .await?
            };
            if !diagnostic_context.document.is_analysis_unavailable() && analysis_context.is_none()
            {
                continue;
            }

            let state_value = compute(
                &diagnostic_context.document,
                analysis_context
                    .as_ref()
                    .map(SnapshotContext::analysis_payload),
            );
            let mut state = self.inner.state.lock().await;
            let Some(committed) = self.commit_state_if_active(&mut state, |state| {
                let current = state.diagnostic_contexts_are_current(
                    &diagnostic_context,
                    analysis_context.as_ref(),
                );
                current
                    && state
                        .set_diagnostic_state_if_current(&diagnostic_context, state_value.clone())
            }) else {
                return Ok(None);
            };
            if committed {
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
        let analysis_context = if context.document.is_analysis_unavailable() {
            None
        } else {
            self.acquire_analysis_context(&context.document.uri, SnapshotPurpose::Diagnostics)
                .await?
        };
        if !context.document.is_analysis_unavailable() && analysis_context.is_none() {
            return Ok(None);
        }
        let result = compute(
            &context.document,
            analysis_context
                .as_ref()
                .map(SnapshotContext::analysis_payload),
        );
        let state = self.inner.state.lock().await;
        let current = state.diagnostic_contexts_are_current(context, analysis_context.as_ref());
        Ok(current.then_some(result))
    }

    async fn acquire_snapshot(
        &self,
        uri: &Uri,
        purpose: SnapshotPurpose,
    ) -> Result<Option<SnapshotLease>> {
        let (snapshot, request, executor) = {
            let mut state = self.inner.state.lock().await;
            let Some(captured) = self.commit_state_if_active(&mut state, |state| {
                let snapshot = state.snapshot_lease_for_uri(uri);
                let request = snapshot
                    .is_none()
                    .then(|| state.snapshot_build_request(uri))
                    .flatten();
                (snapshot, request, state.analysis_executor())
            }) else {
                return Ok(None);
            };
            captured
        };

        if let Some(snapshot) = snapshot {
            return Ok(Some(snapshot));
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
        let snapshot = Arc::clone(analysis.snapshot());
        let committed = {
            let mut state = self.inner.state.lock().await;
            let Some((snapshot, document_exists)) =
                self.commit_state_if_active(&mut state, |state| {
                    let snapshot = state.commit_built_snapshot(&request, snapshot);
                    let document_exists = state.get(request.uri()).is_some();
                    (snapshot, document_exists)
                })
            else {
                return Ok(None);
            };
            let Some(snapshot) = snapshot else {
                return stale_or_missing_result(purpose, document_exists);
            };
            snapshot
        };
        drop(analysis);
        Ok(Some(committed))
    }

    async fn acquire_analysis_context(
        &self,
        uri: &Uri,
        purpose: SnapshotPurpose,
    ) -> Result<Option<SnapshotContext>> {
        let (cached, preparation, request, executor) = {
            let mut state = self.inner.state.lock().await;
            let Some(captured) = self.commit_state_if_active(&mut state, |state| {
                let cached = state
                    .has_analysis_payload(uri)
                    .then(|| state.cached_snapshot_context_for_uri(uri))
                    .flatten();
                let preparation = cached.is_none().then(|| {
                    state
                        .diagnostic_reprojection_request(uri)
                        .map(DiagnosticProjectionPreparation::Project)
                        .or_else(|| {
                            let snapshot = state.snapshot_lease_for_uri(uri)?;
                            state.prepare_diagnostic_projection_for_snapshot(
                                &snapshot,
                                DiagnosticProjectionOrigin::FreshBuild,
                            )
                        })
                });
                let preparation = preparation.flatten();
                let request = (cached.is_none() && preparation.is_none())
                    .then(|| state.snapshot_build_request(uri))
                    .flatten();
                (cached, preparation, request, state.analysis_executor())
            }) else {
                return Ok(None);
            };
            captured
        };

        if cached.is_some() {
            return Ok(cached);
        }

        if let Some(preparation) = preparation {
            return self
                .resolve_diagnostic_projection(&executor, preparation, purpose)
                .await;
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
        let preparation = {
            let mut state = self.inner.state.lock().await;
            let Some((preparation, document_exists)) =
                self.commit_state_if_active(&mut state, |state| {
                    let preparation = state
                        .commit_built_snapshot(&request, Arc::clone(analysis.snapshot()))
                        .and_then(|snapshot| {
                            state.prepare_diagnostic_projection_for_snapshot(
                                &snapshot,
                                DiagnosticProjectionOrigin::FreshBuild,
                            )
                        });
                    let document_exists = state.get(request.uri()).is_some();
                    (preparation, document_exists)
                })
            else {
                return Ok(None);
            };
            let Some(preparation) = preparation else {
                return stale_or_missing_result(purpose, document_exists);
            };
            preparation
        };
        let result = self
            .resolve_diagnostic_projection(&executor, preparation, purpose)
            .await;
        drop(analysis);
        result
    }

    async fn resolve_diagnostic_projection(
        &self,
        executor: &AnalysisExecutor,
        mut preparation: DiagnosticProjectionPreparation,
        purpose: SnapshotPurpose,
    ) -> Result<Option<SnapshotContext>> {
        for _ in 0..MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS {
            let request = match preparation {
                DiagnosticProjectionPreparation::Ready(context) => return Ok(Some(context)),
                DiagnosticProjectionPreparation::Project(request) => request,
            };
            let projection = match executor.execute_diagnostic_reprojection(&request).await {
                Ok(projection) => projection,
                Err(error) if error.is_stale() => {
                    let state = self.inner.state.lock().await;
                    let Some(next) = state.retry_diagnostic_reprojection_request(&request) else {
                        return stale_or_missing_result(
                            purpose,
                            state.get(request.uri()).is_some(),
                        );
                    };
                    preparation = next;
                    continue;
                }
                Err(error) => return Err(analysis_execution_error(error)),
            };
            let mut state = self.inner.state.lock().await;
            let Some((context, next, document_exists)) =
                self.commit_state_if_active(&mut state, |state| {
                    let context = state.commit_diagnostic_reprojection_context(&projection);
                    let next = context
                        .is_none()
                        .then(|| state.retry_diagnostic_reprojection_request(&request))
                        .flatten();
                    let document_exists = state.get(request.uri()).is_some();
                    (context, next, document_exists)
                })
            else {
                return Ok(None);
            };
            if let Some(context) = context {
                return Ok(Some(context));
            }
            let Some(next) = next else {
                return stale_or_missing_result(purpose, document_exists);
            };
            drop(state);
            drop(projection);
            preparation = next;
        }
        let uri = match &preparation {
            DiagnosticProjectionPreparation::Ready(context) => context.snapshot.uri(),
            DiagnosticProjectionPreparation::Project(request) => request.uri(),
        };
        self.stale_or_missing(uri, purpose).await
    }

    async fn stale_or_missing<T>(&self, uri: &Uri, purpose: SnapshotPurpose) -> Result<Option<T>> {
        let document_exists = self.inner.state.lock().await.get(uri).is_some();
        stale_or_missing_result(purpose, document_exists)
    }

    async fn ensure_snapshot_current(
        &self,
        snapshot: &SnapshotLease,
        purpose: SnapshotPurpose,
    ) -> Result<()> {
        if self
            .inner
            .state
            .lock()
            .await
            .is_snapshot_lease_current(snapshot)
        {
            Ok(())
        } else {
            Err(purpose.stale_error())
        }
    }

    async fn ensure_analysis_current(
        &self,
        context: &SnapshotContext,
        purpose: SnapshotPurpose,
    ) -> Result<()> {
        let state = self.inner.state.lock().await;
        if state.is_analysis_context_current(context) {
            Ok(())
        } else {
            Err(purpose.stale_error())
        }
    }
}

fn stale_or_missing_result<T>(
    purpose: SnapshotPurpose,
    document_exists: bool,
) -> Result<Option<T>> {
    if !document_exists || purpose == SnapshotPurpose::Diagnostics {
        Ok(None)
    } else {
        Err(purpose.stale_error())
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
