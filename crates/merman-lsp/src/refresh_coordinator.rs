use crate::refresh_transport::{RefreshClient, RefreshKind};
use crate::sync::recover_poison;
use futures::future::{AbortHandle, Abortable};
use std::fmt;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::mpsc;

const REFRESH_TIMEOUT: Duration = Duration::from_secs(5);
/// Maximum number of response-bearing refresh requests that can be active at once.
#[cfg(feature = "stdio")]
pub(crate) const MAX_CONCURRENT_REFRESH_REQUESTS: usize = 2;

#[derive(Clone)]
pub(crate) struct RefreshCoordinator {
    inner: Arc<RefreshCoordinatorInner>,
}

struct RefreshCoordinatorInner {
    semantic_tokens: RefreshLane,
    diagnostics: RefreshLane,
    client: RefreshClient,
    cancelled: Arc<AtomicBool>,
}

struct RefreshLane {
    pending: Arc<AtomicBool>,
    wake: mpsc::Sender<()>,
    receiver: Mutex<Option<mpsc::Receiver<()>>>,
    worker: Mutex<Option<AbortHandle>>,
    idle: Notify,
    #[cfg(test)]
    requests: AtomicUsize,
}

impl RefreshLane {
    fn new() -> Self {
        let (wake, receiver) = mpsc::channel(1);
        Self {
            pending: Arc::new(AtomicBool::new(false)),
            wake,
            receiver: Mutex::new(Some(receiver)),
            worker: Mutex::new(None),
            idle: Notify::new(),
            #[cfg(test)]
            requests: AtomicUsize::new(0),
        }
    }
}

impl RefreshCoordinatorInner {
    fn lane(&self, kind: RefreshKind) -> &RefreshLane {
        match kind {
            RefreshKind::SemanticTokens => &self.semantic_tokens,
            RefreshKind::Diagnostics => &self.diagnostics,
        }
    }
}

impl fmt::Debug for RefreshCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RefreshCoordinator")
            .field(
                "semantic_tokens_started",
                &recover_poison(self.inner.semantic_tokens.worker.lock()).is_some(),
            )
            .field(
                "semantic_tokens_pending",
                &self.inner.semantic_tokens.pending.load(Ordering::Acquire),
            )
            .field(
                "diagnostics_started",
                &recover_poison(self.inner.diagnostics.worker.lock()).is_some(),
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
        Self {
            inner: Arc::new(RefreshCoordinatorInner {
                semantic_tokens: RefreshLane::new(),
                diagnostics: RefreshLane::new(),
                client,
                cancelled: Arc::new(AtomicBool::new(false)),
            }),
        }
    }

    pub(crate) fn request_semantic_tokens_refresh(&self) {
        self.request_lane(RefreshKind::SemanticTokens);
    }

    pub(crate) fn request_diagnostic_refresh(&self) {
        self.request_lane(RefreshKind::Diagnostics);
    }

    fn request_lane(&self, kind: RefreshKind) {
        if self.inner.cancelled.load(Ordering::Acquire) {
            return;
        }
        let lane = self.inner.lane(kind);
        lane.pending.store(true, Ordering::Release);
        if self.inner.cancelled.load(Ordering::Acquire) {
            lane.pending.store(false, Ordering::Release);
            return;
        }
        #[cfg(test)]
        lane.requests.fetch_add(1, Ordering::Relaxed);
        self.ensure_worker(kind);
        let _ = lane.wake.try_send(());
    }

    fn ensure_worker(&self, kind: RefreshKind) {
        if self.inner.cancelled.load(Ordering::Acquire) {
            return;
        }
        let lane = self.inner.lane(kind);
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!("refresh requested outside a Tokio runtime");
            return;
        };
        let mut worker = recover_poison(lane.worker.lock());
        if worker.is_some() || self.inner.cancelled.load(Ordering::Acquire) {
            return;
        }

        let receiver = recover_poison(lane.receiver.lock()).take();
        let Some(receiver) = receiver else {
            tracing::warn!(
                refresh_kind = kind.label(),
                "refresh worker receiver was unavailable"
            );
            return;
        };
        let pending = Arc::clone(&lane.pending);
        let client = self.inner.client.clone();
        let cancelled = Arc::clone(&self.inner.cancelled);
        let inner = Arc::downgrade(&self.inner);
        let (abort, registration) = AbortHandle::new_pair();
        *worker = Some(abort);
        drop(worker);
        runtime.spawn(async move {
            let _ = Abortable::new(
                run_worker(receiver, pending, client, cancelled, kind),
                registration,
            )
            .await;
            worker_finished(inner, kind);
        });
    }

    pub(crate) fn cancel(&self) {
        if self.inner.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }

        self.inner
            .semantic_tokens
            .pending
            .store(false, Ordering::Release);
        self.inner
            .diagnostics
            .pending
            .store(false, Ordering::Release);
        self.inner.client.cancel_all();
        for lane in [&self.inner.semantic_tokens, &self.inner.diagnostics] {
            if let Some(abort) = recover_poison(lane.worker.lock()).as_ref() {
                abort.abort();
            } else {
                lane.idle.notify_waiters();
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn request_counts(&self) -> (usize, usize) {
        (
            self.inner.semantic_tokens.requests.load(Ordering::Relaxed),
            self.inner.diagnostics.requests.load(Ordering::Relaxed),
        )
    }

    #[cfg(test)]
    pub(crate) async fn wait_stopped(&self) {
        for lane in [&self.inner.semantic_tokens, &self.inner.diagnostics] {
            loop {
                let idle = lane.idle.notified();
                tokio::pin!(idle);
                idle.as_mut().enable();
                if recover_poison(lane.worker.lock()).is_none() {
                    break;
                }
                idle.as_mut().await;
            }
        }
    }
}

async fn run_worker(
    mut receiver: mpsc::Receiver<()>,
    pending: Arc<AtomicBool>,
    client: RefreshClient,
    cancelled: Arc<AtomicBool>,
    kind: RefreshKind,
) {
    while receiver.recv().await.is_some() {
        loop {
            if cancelled.load(Ordering::Acquire) {
                return;
            }
            if !pending.swap(false, Ordering::AcqRel) {
                break;
            }

            supervise_refresh(kind.label(), client.request(kind)).await;

            if !pending.load(Ordering::Acquire) {
                break;
            }
        }
    }
}

fn worker_finished(inner: Weak<RefreshCoordinatorInner>, kind: RefreshKind) {
    let Some(inner) = inner.upgrade() else {
        return;
    };
    let lane = inner.lane(kind);
    recover_poison(lane.worker.lock()).take();
    lane.idle.notify_waiters();
}

async fn supervise_refresh<F>(kind: &str, refresh: F)
where
    F: std::future::Future<Output = tower_lsp_server::jsonrpc::Result<()>>,
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
    use std::task::Poll;
    use tower_lsp_server::jsonrpc::Response;

    #[tokio::test(flavor = "current_thread")]
    async fn pending_refresh_requests_coalesce_to_one_follow_up() {
        let (client, mut requests, responses) = RefreshClient::channel();
        let coordinator = RefreshCoordinator::new(client);

        coordinator.request_semantic_tokens_refresh();
        let first = requests
            .next()
            .await
            .expect("expected first refresh request");
        assert_eq!(responses.pending_count(), 1);

        coordinator.request_semantic_tokens_refresh();
        coordinator.request_semantic_tokens_refresh();
        assert_eq!(coordinator.request_counts(), (3, 0));
        assert!(
            coordinator
                .inner
                .semantic_tokens
                .pending
                .load(Ordering::Acquire)
        );

        let mut next_request = Box::pin(requests.next());
        assert!(matches!(
            futures::poll!(next_request.as_mut()),
            Poll::Pending
        ));

        assert!(
            responses
                .route(Response::from_ok(
                    first.id().cloned().expect("first refresh id"),
                    serde_json::Value::Null,
                ))
                .is_none()
        );
        let follow_up = next_request
            .await
            .expect("expected one coalesced follow-up refresh");
        assert_ne!(first.id(), follow_up.id());
        assert_eq!(responses.pending_count(), 1);
        assert!(
            !coordinator
                .inner
                .semantic_tokens
                .pending
                .load(Ordering::Acquire)
        );

        assert!(
            responses
                .route(Response::from_ok(
                    follow_up.id().cloned().expect("follow-up refresh id"),
                    serde_json::Value::Null,
                ))
                .is_none()
        );
        tokio::task::yield_now().await;
        assert_eq!(responses.pending_count(), 0);
        let mut extra_request = Box::pin(requests.next());
        assert!(matches!(
            futures::poll!(extra_request.as_mut()),
            Poll::Pending
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn semantic_token_and_diagnostic_refreshes_use_independent_lanes() {
        let (client, mut requests, responses) = RefreshClient::channel();
        let coordinator = RefreshCoordinator::new(client);

        coordinator.request_semantic_tokens_refresh();
        coordinator.request_diagnostic_refresh();

        let first = requests
            .next()
            .await
            .expect("expected first refresh request");
        let second = requests
            .next()
            .await
            .expect("expected second refresh request");
        let methods = [first.method(), second.method()];
        assert!(methods.contains(&"workspace/semanticTokens/refresh"));
        assert!(methods.contains(&"workspace/diagnostic/refresh"));
        assert_eq!(responses.pending_count(), 2);

        for request in [first, second] {
            assert!(
                responses
                    .route(Response::from_ok(
                        request.id().cloned().expect("refresh request id"),
                        serde_json::Value::Null,
                    ))
                    .is_none()
            );
        }
        tokio::task::yield_now().await;
        assert_eq!(responses.pending_count(), 0);

        coordinator.cancel();
        coordinator.wait_stopped().await;
    }

    #[tokio::test(start_paused = true)]
    async fn timed_out_refresh_releases_waiter_and_allows_a_follow_up() {
        let (client, mut requests, responses) = RefreshClient::channel();
        let coordinator = RefreshCoordinator::new(client);

        coordinator.request_semantic_tokens_refresh();
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

        coordinator.request_semantic_tokens_refresh();
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
            "late managed responses must not reach tower-lsp-server"
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

    #[tokio::test(flavor = "current_thread")]
    async fn cancellation_aborts_refresh_workers_and_response_waiters() {
        let (client, mut requests, responses) = RefreshClient::channel();
        let coordinator = RefreshCoordinator::new(client);

        coordinator.request_semantic_tokens_refresh();
        requests
            .next()
            .await
            .expect("expected an in-flight refresh request");
        assert_eq!(responses.pending_count(), 1);

        coordinator.cancel();
        coordinator.wait_stopped().await;

        assert_eq!(responses.pending_count(), 0);
        coordinator.request_semantic_tokens_refresh();
        coordinator.request_diagnostic_refresh();
        assert_eq!(responses.pending_count(), 0);
    }
}
