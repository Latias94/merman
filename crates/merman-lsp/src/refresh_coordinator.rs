use crate::refresh_transport::{RefreshClient, RefreshKind};
use crate::sync::recover_poison;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tower_lsp::Client;

const REFRESH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub(crate) struct RefreshCoordinator {
    inner: Arc<RefreshCoordinatorInner>,
}

struct RefreshCoordinatorInner {
    semantic_tokens: RefreshLane,
    diagnostics: RefreshLane,
    transport: RefreshTransport,
}

#[derive(Clone)]
enum RefreshTransport {
    Managed(RefreshClient),
    Tower(Client),
}

struct RefreshLane {
    pending: Arc<AtomicBool>,
    wake: mpsc::Sender<()>,
    receiver: Mutex<Option<mpsc::Receiver<()>>>,
    started: AtomicBool,
}

impl RefreshLane {
    fn new() -> Self {
        let (wake, receiver) = mpsc::channel(1);
        Self {
            pending: Arc::new(AtomicBool::new(false)),
            wake,
            receiver: Mutex::new(Some(receiver)),
            started: AtomicBool::new(false),
        }
    }
}

impl fmt::Debug for RefreshCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RefreshCoordinator")
            .field(
                "semantic_tokens_started",
                &self.inner.semantic_tokens.started.load(Ordering::Acquire),
            )
            .field(
                "semantic_tokens_pending",
                &self.inner.semantic_tokens.pending.load(Ordering::Acquire),
            )
            .field(
                "diagnostics_started",
                &self.inner.diagnostics.started.load(Ordering::Acquire),
            )
            .field(
                "diagnostics_pending",
                &self.inner.diagnostics.pending.load(Ordering::Acquire),
            )
            .finish_non_exhaustive()
    }
}

impl RefreshCoordinator {
    pub(crate) fn new(client: RefreshClient) -> Self {
        Self::with_transport(RefreshTransport::Managed(client))
    }

    pub(crate) fn from_tower_client(client: Client) -> Self {
        Self::with_transport(RefreshTransport::Tower(client))
    }

    fn with_transport(transport: RefreshTransport) -> Self {
        Self {
            inner: Arc::new(RefreshCoordinatorInner {
                semantic_tokens: RefreshLane::new(),
                diagnostics: RefreshLane::new(),
                transport,
            }),
        }
    }

    pub(crate) fn request(&self, semantic_tokens: bool, diagnostics: bool) {
        if semantic_tokens {
            self.request_lane(RefreshKind::SemanticTokens);
        }
        if diagnostics {
            self.request_lane(RefreshKind::Diagnostics);
        }
    }

    fn request_lane(&self, kind: RefreshKind) {
        let lane = self.lane(kind);
        lane.pending.store(true, Ordering::Release);
        self.ensure_worker(kind);
        let _ = lane.wake.try_send(());
    }

    fn lane(&self, kind: RefreshKind) -> &RefreshLane {
        match kind {
            RefreshKind::SemanticTokens => &self.inner.semantic_tokens,
            RefreshKind::Diagnostics => &self.inner.diagnostics,
        }
    }

    fn ensure_worker(&self, kind: RefreshKind) {
        let lane = self.lane(kind);
        if lane.started.load(Ordering::Acquire) {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!("refresh requested outside a Tokio runtime");
            return;
        };
        if lane
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let receiver = recover_poison(lane.receiver.lock()).take();
        let Some(receiver) = receiver else {
            lane.started.store(false, Ordering::Release);
            tracing::warn!(
                refresh_kind = kind.label(),
                "refresh worker receiver was unavailable"
            );
            return;
        };
        let pending = Arc::clone(&lane.pending);
        let transport = self.inner.transport.clone();
        runtime.spawn(run_worker(receiver, pending, transport, kind));
    }
}

async fn run_worker(
    mut receiver: mpsc::Receiver<()>,
    pending: Arc<AtomicBool>,
    transport: RefreshTransport,
    kind: RefreshKind,
) {
    while receiver.recv().await.is_some() {
        loop {
            if !pending.swap(false, Ordering::AcqRel) {
                break;
            }

            transport.request(kind).await;

            if !pending.load(Ordering::Acquire) {
                break;
            }
        }
    }
}

impl RefreshTransport {
    async fn request(&self, kind: RefreshKind) {
        match self {
            Self::Managed(client) => {
                supervise_refresh(kind.label(), client.request(kind)).await;
            }
            Self::Tower(client) => {
                let result = match kind {
                    RefreshKind::SemanticTokens => client.semantic_tokens_refresh().await,
                    RefreshKind::Diagnostics => client.workspace_diagnostic_refresh().await,
                };
                if let Err(error) = result {
                    tracing::warn!(%error, refresh_kind = kind.label(), "client refresh failed");
                }
            }
        }
    }
}

async fn supervise_refresh<F>(kind: &str, refresh: F)
where
    F: std::future::Future<Output = tower_lsp::jsonrpc::Result<()>>,
{
    let result = match tokio::time::timeout(REFRESH_TIMEOUT, refresh).await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!(refresh_kind = kind, "client refresh response timed out");
            return;
        }
    };
    if let Err(error) = result {
        tracing::warn!(%error, refresh_kind = kind, "client refresh failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refresh_transport::RefreshClient;
    use futures::StreamExt;
    use tower_lsp::jsonrpc::Response;

    #[tokio::test(start_paused = true)]
    async fn timed_out_refresh_releases_waiter_and_allows_a_follow_up() {
        let (client, mut requests, responses) = RefreshClient::channel();
        let coordinator = RefreshCoordinator::new(client);

        coordinator.request(true, false);
        let first = requests
            .next()
            .await
            .expect("expected first refresh request");
        assert_eq!(responses.pending_count(), 1);

        tokio::time::advance(REFRESH_TIMEOUT + Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            responses.pending_count(),
            0,
            "timed-out refresh must remove its response waiter"
        );

        coordinator.request(true, false);
        let second = requests
            .next()
            .await
            .expect("same refresh lane should accept a follow-up request");
        assert_ne!(first.id(), second.id());
        assert_eq!(responses.pending_count(), 1);

        assert!(
            responses
                .route(Response::from_ok(
                    first.id().cloned().expect("first refresh id"),
                    serde_json::Value::Null,
                ))
                .is_none(),
            "late managed responses must not reach tower-lsp"
        );
        assert_eq!(responses.pending_count(), 1);

        assert!(
            responses
                .route(Response::from_ok(
                    second.id().cloned().expect("second refresh id"),
                    serde_json::Value::Null,
                ))
                .is_none()
        );
        tokio::task::yield_now().await;
        assert_eq!(responses.pending_count(), 0);
    }
}
