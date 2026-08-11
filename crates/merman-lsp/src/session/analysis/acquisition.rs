use super::executor::{AnalysisExecutionError, AnalysisExecutor};
use super::request::DiagnosticReprojectionRequest;
use crate::session::LanguageSession;
use crate::session::analysis_cache::AnalysisCacheStamp;
use crate::session::documents::{DocumentDiagnosticState, StoredDocument};
use crate::snapshot::{DocumentSnapshot, SnapshotContext};
use std::sync::Arc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Uri;

const MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS: usize = 3;

#[derive(Debug, Clone)]
pub(in crate::session) struct AcquiredSnapshot {
    pub(in crate::session) snapshot: Arc<DocumentSnapshot>,
    pub(in crate::session) stamp: AnalysisCacheStamp,
}

impl AcquiredSnapshot {
    pub(in crate::session) fn new(
        snapshot: Arc<DocumentSnapshot>,
        stamp: AnalysisCacheStamp,
    ) -> Self {
        Self { snapshot, stamp }
    }
}

pub(in crate::session) enum ProjectionDecision {
    Ready(SnapshotContext),
    Project(DiagnosticReprojectionRequest),
}

enum AcquisitionAttempt<T> {
    Ready(T),
    Stale,
    Unavailable,
    Missing,
}

#[derive(Clone, Copy)]
enum DocumentAnalysisAvailability {
    Available,
    Unavailable,
    Missing,
}

impl LanguageSession {
    pub(super) async fn acquire_structure_snapshot(
        &self,
        uri: &Uri,
    ) -> Result<Option<AcquiredSnapshot>> {
        interactive_result(self.try_acquire_snapshot(uri).await?, structure_stale_error)
    }

    pub(super) async fn acquire_semantic_token_snapshot(
        &self,
        uri: &Uri,
    ) -> Result<Option<AcquiredSnapshot>> {
        interactive_result(
            self.try_acquire_snapshot(uri).await?,
            semantic_tokens_stale_error,
        )
    }

    pub(super) async fn acquire_code_action_context(
        &self,
        uri: &Uri,
    ) -> Result<Option<SnapshotContext>> {
        interactive_result(
            self.try_acquire_analysis(uri).await?,
            code_actions_stale_error,
        )
    }

    pub(super) async fn acquire_push_diagnostic_context(
        &self,
        uri: &Uri,
    ) -> Result<Option<SnapshotContext>> {
        match self.try_acquire_analysis(uri).await? {
            AcquisitionAttempt::Ready(context) => Ok(Some(context)),
            AcquisitionAttempt::Stale
            | AcquisitionAttempt::Unavailable
            | AcquisitionAttempt::Missing => Ok(None),
        }
    }

    pub(super) async fn ensure_structure_snapshot_current(
        &self,
        snapshot: &AcquiredSnapshot,
    ) -> Result<()> {
        if self
            .inner
            .state
            .lock()
            .await
            .is_acquired_snapshot_current(snapshot)
        {
            Ok(())
        } else {
            Err(structure_stale_error())
        }
    }

    pub(super) async fn ensure_code_action_context_current(
        &self,
        context: &SnapshotContext,
    ) -> Result<()> {
        if self
            .inner
            .state
            .lock()
            .await
            .is_analysis_context_current(context)
        {
            Ok(())
        } else {
            Err(code_actions_stale_error())
        }
    }

    pub(crate) async fn pull_diagnostics(
        &self,
        uri: &Uri,
        compute: impl Fn(&StoredDocument, Option<&SnapshotContext>) -> DocumentDiagnosticState,
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
                match self.try_acquire_analysis(uri).await? {
                    AcquisitionAttempt::Ready(context) => Some(context),
                    AcquisitionAttempt::Stale | AcquisitionAttempt::Unavailable => continue,
                    AcquisitionAttempt::Missing => return Ok(None),
                }
            };

            let state_value = compute(&diagnostic_context.document, analysis_context.as_ref());
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

    async fn try_acquire_snapshot(
        &self,
        uri: &Uri,
    ) -> Result<AcquisitionAttempt<AcquiredSnapshot>> {
        let (availability, snapshot, request, executor) = {
            let mut state = self.inner.state.lock().await;
            let Some(captured) = self.commit_state_if_active(&mut state, |state| {
                let availability = match state.get(uri) {
                    Some(document) if document.is_analysis_unavailable() => {
                        DocumentAnalysisAvailability::Unavailable
                    }
                    Some(_) => DocumentAnalysisAvailability::Available,
                    None => DocumentAnalysisAvailability::Missing,
                };
                let (snapshot, request) =
                    if matches!(availability, DocumentAnalysisAvailability::Available) {
                        let snapshot = state.acquired_snapshot_for_uri(uri);
                        let request = snapshot
                            .is_none()
                            .then(|| state.snapshot_build_request_after_cache_miss(uri))
                            .flatten();
                        (snapshot, request)
                    } else {
                        (None, None)
                    };
                (availability, snapshot, request, state.analysis_executor())
            }) else {
                return Ok(AcquisitionAttempt::Missing);
            };
            captured
        };

        match availability {
            DocumentAnalysisAvailability::Missing => return Ok(AcquisitionAttempt::Missing),
            DocumentAnalysisAvailability::Unavailable => {
                return Ok(AcquisitionAttempt::Unavailable);
            }
            DocumentAnalysisAvailability::Available => {}
        }
        if let Some(snapshot) = snapshot {
            return Ok(AcquisitionAttempt::Ready(snapshot));
        }
        let Some(request) = request else {
            return Ok(AcquisitionAttempt::Stale);
        };
        let analysis = match executor.execute(&request).await {
            Ok(analysis) => analysis,
            Err(error) if error.is_stale() => return self.stale_or_missing(uri).await,
            Err(error) => return Err(analysis_execution_error(error)),
        };
        let committed = {
            let mut state = self.inner.state.lock().await;
            let Some(snapshot) = self.commit_state_if_active(&mut state, |state| {
                state.commit_built_snapshot(&request, &analysis)
            }) else {
                return Ok(AcquisitionAttempt::Missing);
            };
            snapshot
        };
        drop(analysis);
        match committed {
            Some(snapshot) => Ok(AcquisitionAttempt::Ready(snapshot)),
            None => self.stale_or_missing(uri).await,
        }
    }

    async fn try_acquire_analysis(&self, uri: &Uri) -> Result<AcquisitionAttempt<SnapshotContext>> {
        let snapshot = match self.try_acquire_snapshot(uri).await? {
            AcquisitionAttempt::Ready(snapshot) => snapshot,
            AcquisitionAttempt::Stale => return Ok(AcquisitionAttempt::Stale),
            AcquisitionAttempt::Unavailable => return Ok(AcquisitionAttempt::Unavailable),
            AcquisitionAttempt::Missing => return Ok(AcquisitionAttempt::Missing),
        };
        let (decision, executor) = {
            let mut state = self.inner.state.lock().await;
            let Some(captured) = self.commit_state_if_active(&mut state, |state| {
                (
                    state.projection_decision_for_snapshot(&snapshot),
                    state.analysis_executor(),
                )
            }) else {
                return Ok(AcquisitionAttempt::Missing);
            };
            captured
        };

        let Some(decision) = decision else {
            return self.stale_or_missing(uri).await;
        };
        match decision {
            ProjectionDecision::Ready(context) => Ok(AcquisitionAttempt::Ready(context)),
            ProjectionDecision::Project(request) => {
                self.execute_projection_once(&executor, request).await
            }
        }
    }

    async fn execute_projection_once(
        &self,
        executor: &AnalysisExecutor,
        request: DiagnosticReprojectionRequest,
    ) -> Result<AcquisitionAttempt<SnapshotContext>> {
        let uri = request.uri().clone();
        let projection = match executor.execute_diagnostic_reprojection(&request).await {
            Ok(projection) => projection,
            Err(error) if error.is_stale() => return self.stale_or_missing(&uri).await,
            Err(error) => return Err(analysis_execution_error(error)),
        };
        let committed = {
            let mut state = self.inner.state.lock().await;
            let Some(context) = self.commit_state_if_active(&mut state, |state| {
                state.commit_diagnostic_reprojection(&request, &projection)
            }) else {
                return Ok(AcquisitionAttempt::Missing);
            };
            context
        };
        drop(projection);
        match committed {
            Some(context) => Ok(AcquisitionAttempt::Ready(context)),
            None => self.stale_or_missing(&uri).await,
        }
    }

    async fn stale_or_missing<T>(&self, uri: &Uri) -> Result<AcquisitionAttempt<T>> {
        if self.inner.state.lock().await.get(uri).is_some() {
            Ok(AcquisitionAttempt::Stale)
        } else {
            Ok(AcquisitionAttempt::Missing)
        }
    }
}

fn interactive_result<T>(
    attempt: AcquisitionAttempt<T>,
    stale_error: fn() -> tower_lsp_server::jsonrpc::Error,
) -> Result<Option<T>> {
    match attempt {
        AcquisitionAttempt::Ready(value) => Ok(Some(value)),
        AcquisitionAttempt::Unavailable | AcquisitionAttempt::Missing => Ok(None),
        AcquisitionAttempt::Stale => Err(stale_error()),
    }
}

pub(super) fn semantic_tokens_stale_error() -> tower_lsp_server::jsonrpc::Error {
    stale_error("semantic tokens document changed while computing")
}

fn structure_stale_error() -> tower_lsp_server::jsonrpc::Error {
    stale_error("structure document changed while computing")
}

fn code_actions_stale_error() -> tower_lsp_server::jsonrpc::Error {
    stale_error("code action document changed while computing")
}

fn stale_error(message: &'static str) -> tower_lsp_server::jsonrpc::Error {
    let mut error = tower_lsp_server::jsonrpc::Error::content_modified();
    error.message = message.into();
    error
}

fn analysis_execution_error(error: AnalysisExecutionError) -> tower_lsp_server::jsonrpc::Error {
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
