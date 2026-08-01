use crate::sync::lock_recovering_poison;
use futures::FutureExt;
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use tower_lsp_server::ls_types::Uri;

type ClientEffect = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub(crate) const LSP_CLIENT_EFFECT_QUEUE_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ClientEffectKey {
    Document(Uri),
    AllDiagnostics,
    LogMessage,
}

impl ClientEffectKey {
    #[cfg(test)]
    pub(crate) fn document_for_test(name: impl AsRef<str>) -> Self {
        Self::Document(
            format!("file:///client-effects/{}", name.as_ref())
                .parse()
                .expect("client-effect test URI must be valid"),
        )
    }
}

struct QueuedClientEffect {
    key: ClientEffectKey,
    intent: u64,
    effect: ClientEffect,
}

enum ClientEffectAdmission {
    Cancelled(QueuedClientEffect),
    Superseded(QueuedClientEffect),
    Pending(QueuedClientEffect),
    Admitted {
        replaced: Option<QueuedClientEffect>,
    },
}

enum ClientEffectClaim {
    Cancelled,
    Empty,
    Current(QueuedClientEffect),
    Superseded(QueuedClientEffect),
}

/// Serializes client-visible side effects with bounded, latest-intent admission.
#[derive(Clone)]
pub(super) struct ClientEffectDispatcher {
    inner: Arc<ClientEffectDispatcherInner>,
}

struct ClientEffectDispatcherInner {
    state: Mutex<ClientEffectState>,
    changed: Notify,
    idle: Notify,
}

#[derive(Default)]
struct ClientEffectState {
    queue: VecDeque<QueuedClientEffect>,
    latest_intents: HashMap<ClientEffectKey, u64>,
    cancelled: bool,
    next_intent_id: u64,
    next_worker_id: u64,
    active_worker: Option<ActiveClientEffectWorker>,
    #[cfg(test)]
    admitted_count: usize,
}

impl ClientEffectState {
    fn register_intent(&mut self, key: ClientEffectKey) -> Option<u64> {
        if self.cancelled {
            return None;
        }
        self.next_intent_id = self
            .next_intent_id
            .checked_add(1)
            .expect("client effect intent sequence exhausted");
        let intent = self.next_intent_id;
        self.latest_intents.insert(key, intent);
        Some(intent)
    }

    fn remove_intent_if_current(&mut self, key: &ClientEffectKey, intent: u64) -> bool {
        if self.latest_intents.get(key) != Some(&intent) {
            return false;
        }
        self.latest_intents.remove(key);
        true
    }

    fn try_admit(&mut self, queued: QueuedClientEffect) -> ClientEffectAdmission {
        if self.cancelled {
            return ClientEffectAdmission::Cancelled(queued);
        }
        if self.latest_intents.get(&queued.key) != Some(&queued.intent) {
            return ClientEffectAdmission::Superseded(queued);
        }

        let replacement = self
            .queue
            .iter()
            .position(|candidate| candidate.key == queued.key);
        if replacement.is_none() && self.queue.len() >= LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            return ClientEffectAdmission::Pending(queued);
        }

        let replaced = replacement.and_then(|index| self.queue.remove(index));
        self.queue.push_back(queued);
        #[cfg(test)]
        {
            self.admitted_count += 1;
        }
        ClientEffectAdmission::Admitted { replaced }
    }

    fn claim_next(&mut self) -> ClientEffectClaim {
        if self.cancelled {
            return ClientEffectClaim::Cancelled;
        }
        let Some(queued) = self.queue.pop_front() else {
            return ClientEffectClaim::Empty;
        };
        if self.latest_intents.get(&queued.key) != Some(&queued.intent) {
            return ClientEffectClaim::Superseded(queued);
        }

        self.latest_intents.remove(&queued.key);
        ClientEffectClaim::Current(queued)
    }
}

struct ActiveClientEffectWorker {
    id: u64,
    abort: AbortHandle,
}

struct ClientEffectIntentGuard {
    inner: Arc<ClientEffectDispatcherInner>,
    key: ClientEffectKey,
    intent: u64,
    armed: bool,
}

impl ClientEffectIntentGuard {
    fn register(inner: Arc<ClientEffectDispatcherInner>, key: ClientEffectKey) -> Option<Self> {
        let intent = lock_recovering_poison(&inner.state).register_intent(key.clone())?;
        inner.changed.notify_waiters();
        Some(Self {
            inner,
            key,
            intent,
            armed: true,
        })
    }

    fn intent(&self) -> u64 {
        self.intent
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ClientEffectIntentGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let removed = lock_recovering_poison(&self.inner.state)
            .remove_intent_if_current(&self.key, self.intent);
        if removed {
            self.inner.changed.notify_waiters();
        }
    }
}

impl std::fmt::Debug for ClientEffectDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = lock_recovering_poison(&self.inner.state);
        formatter
            .debug_struct("ClientEffectDispatcher")
            .field("queued", &state.queue.len())
            .field("running", &state.active_worker.is_some())
            .field("cancelled", &state.cancelled)
            .finish()
    }
}

impl ClientEffectDispatcher {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(ClientEffectDispatcherInner {
                state: Mutex::new(ClientEffectState::default()),
                changed: Notify::new(),
                idle: Notify::new(),
            }),
        }
    }

    pub(super) async fn enqueue_latest(
        &self,
        key: ClientEffectKey,
        effect: impl Future<Output = ()> + Send + 'static,
    ) {
        let Some(mut intent) =
            ClientEffectIntentGuard::register(Arc::clone(&self.inner), key.clone())
        else {
            return;
        };
        let mut queued = Some(QueuedClientEffect {
            key,
            intent: intent.intent(),
            effect: Box::pin(effect),
        });

        let registration = loop {
            let changed = self.inner.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let (admission, registration) = {
                let mut state = lock_recovering_poison(&self.inner.state);
                let admission = state.try_admit(
                    queued
                        .take()
                        .expect("a client effect admission always returns ownership"),
                );
                let registration = if matches!(&admission, ClientEffectAdmission::Admitted { .. })
                    && state.active_worker.is_none()
                {
                    Some(Self::register_worker(&mut state))
                } else {
                    None
                };
                (admission, registration)
            };

            match admission {
                ClientEffectAdmission::Cancelled(cancelled)
                | ClientEffectAdmission::Superseded(cancelled) => {
                    drop(cancelled);
                    return;
                }
                ClientEffectAdmission::Pending(pending) => {
                    queued = Some(pending);
                    changed.as_mut().await;
                }
                ClientEffectAdmission::Admitted { replaced } => {
                    intent.disarm();
                    drop(replaced);
                    break registration;
                }
            }
        };

        self.inner.changed.notify_waiters();
        if let Some((worker_id, registration)) = registration {
            self.spawn_worker(worker_id, registration);
        }
    }

    pub(super) fn cancel(&self) {
        let (worker_abort, queued) = {
            let mut state = lock_recovering_poison(&self.inner.state);
            state.cancelled = true;
            let queued = std::mem::take(&mut state.queue);
            state.latest_intents.clear();
            let worker_abort = state
                .active_worker
                .as_ref()
                .map(|worker| worker.abort.clone());
            (worker_abort, queued)
        };

        drop(queued);
        if let Some(worker_abort) = worker_abort {
            worker_abort.abort();
        }
        self.inner.changed.notify_waiters();
        self.inner.idle.notify_waiters();
    }

    #[cfg(test)]
    pub(super) fn admission_count(&self) -> usize {
        lock_recovering_poison(&self.inner.state).admitted_count
    }

    #[cfg(test)]
    pub(super) fn latest_intent_count(&self) -> usize {
        lock_recovering_poison(&self.inner.state)
            .latest_intents
            .len()
    }

    #[cfg(test)]
    pub(super) async fn wait_idle(&self) {
        loop {
            let idle = self.inner.idle.notified();
            tokio::pin!(idle);
            idle.as_mut().enable();
            let is_idle = {
                let state = lock_recovering_poison(&self.inner.state);
                state.active_worker.is_none() && state.queue.is_empty()
            };
            if is_idle {
                return;
            }
            idle.as_mut().await;
        }
    }

    async fn drain(&self) {
        loop {
            let claim = lock_recovering_poison(&self.inner.state).claim_next();
            match claim {
                ClientEffectClaim::Cancelled | ClientEffectClaim::Empty => return,
                ClientEffectClaim::Superseded(superseded) => {
                    self.inner.changed.notify_waiters();
                    drop(superseded);
                }
                ClientEffectClaim::Current(current) => {
                    self.inner.changed.notify_waiters();
                    if AssertUnwindSafe(current.effect)
                        .catch_unwind()
                        .await
                        .is_err()
                    {
                        tracing::error!("LSP client effect panicked");
                    }
                }
            }
        }
    }

    fn register_worker(state: &mut ClientEffectState) -> (u64, AbortRegistration) {
        state.next_worker_id = state
            .next_worker_id
            .checked_add(1)
            .expect("client effect worker sequence exhausted");
        let worker_id = state.next_worker_id;
        let (abort, registration) = AbortHandle::new_pair();
        state.active_worker = Some(ActiveClientEffectWorker {
            id: worker_id,
            abort,
        });
        (worker_id, registration)
    }

    fn spawn_worker(&self, worker_id: u64, registration: AbortRegistration) {
        let dispatcher = self.clone();
        tokio::spawn(async move {
            let _ = Abortable::new(dispatcher.drain(), registration).await;
            dispatcher.worker_finished(worker_id);
        });
    }

    fn worker_finished(&self, worker_id: u64) {
        let replacement = {
            let mut state = lock_recovering_poison(&self.inner.state);
            if state
                .active_worker
                .as_ref()
                .is_none_or(|worker| worker.id != worker_id)
            {
                return;
            }
            state.active_worker = None;
            if !state.cancelled && !state.queue.is_empty() {
                Some(Self::register_worker(&mut state))
            } else {
                None
            }
        };

        self.inner.changed.notify_waiters();
        if let Some((worker_id, registration)) = replacement {
            self.spawn_worker(worker_id, registration);
        } else {
            self.inner.idle.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::task::{Context, Poll, Wake, Waker};

    #[test]
    fn registering_new_intent_before_claim_supersedes_queued_effect() {
        let key = ClientEffectKey::document_for_test("same");
        let mut state = ClientEffectState::default();
        let older = state.register_intent(key.clone()).unwrap();
        let admission = state.try_admit(QueuedClientEffect {
            key: key.clone(),
            intent: older,
            effect: Box::pin(async {}),
        });
        assert!(matches!(
            admission,
            ClientEffectAdmission::Admitted { replaced: None }
        ));

        let _newer = state.register_intent(key).unwrap();
        assert!(matches!(
            state.claim_next(),
            ClientEffectClaim::Superseded(_)
        ));
    }

    #[test]
    fn dropping_new_intent_does_not_resurrect_queued_old_effect() {
        let key = ClientEffectKey::document_for_test("same");
        let mut state = ClientEffectState::default();
        let older = state.register_intent(key.clone()).unwrap();
        let admission = state.try_admit(QueuedClientEffect {
            key: key.clone(),
            intent: older,
            effect: Box::pin(async {}),
        });
        assert!(matches!(
            admission,
            ClientEffectAdmission::Admitted { replaced: None }
        ));

        let newer = state.register_intent(key.clone()).unwrap();
        assert!(state.remove_intent_if_current(&key, newer));
        assert!(matches!(
            state.claim_next(),
            ClientEffectClaim::Superseded(_)
        ));
    }

    #[test]
    fn registering_superseding_intent_wakes_existing_waiter() {
        #[derive(Default)]
        struct WakeCount(AtomicUsize);

        impl Wake for WakeCount {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }

            fn wake_by_ref(self: &Arc<Self>) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        let dispatcher = ClientEffectDispatcher::new();
        let key = ClientEffectKey::document_for_test("same");
        let _older =
            ClientEffectIntentGuard::register(Arc::clone(&dispatcher.inner), key.clone()).unwrap();
        let mut changed = Box::pin(dispatcher.inner.changed.notified());
        let wake_count = Arc::new(WakeCount::default());
        let waker = Waker::from(Arc::clone(&wake_count));
        let mut context = Context::from_waker(&waker);
        assert!(matches!(changed.as_mut().poll(&mut context), Poll::Pending));

        let _newer = ClientEffectIntentGuard::register(Arc::clone(&dispatcher.inner), key).unwrap();

        assert_eq!(wake_count.0.load(Ordering::Relaxed), 1);
        assert!(matches!(
            changed.as_mut().poll(&mut context),
            Poll::Ready(())
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn client_effects_finish_in_enqueue_order() {
        let dispatcher = ClientEffectDispatcher::new();
        let (release_first_tx, release_first_rx) = tokio::sync::oneshot::channel();
        let (first_done_tx, first_done_rx) = tokio::sync::oneshot::channel();
        let (second_done_tx, mut second_done_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("first"), async move {
                let _ = release_first_rx.await;
                let _ = first_done_tx.send(());
            })
            .await;
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("second"), async move {
                let _ = second_done_tx.send(());
            })
            .await;

        assert!(matches!(futures::poll!(&mut second_done_rx), Poll::Pending));
        release_first_tx.send(()).unwrap();
        first_done_rx.await.unwrap();
        second_done_rx.await.unwrap();
        dispatcher.wait_idle().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pending_document_effects_keep_only_the_latest_intent() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let superseded_ran = Arc::new(AtomicBool::new(false));
        let latest_ran = Arc::new(AtomicBool::new(false));

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("blocker"), async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
            })
            .await;
        started_rx.await.unwrap();

        let superseded_marker = Arc::clone(&superseded_ran);
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("same"), async move {
                superseded_marker.store(true, Ordering::Release);
            })
            .await;
        let latest_marker = Arc::clone(&latest_ran);
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("same"), async move {
                latest_marker.store(true, Ordering::Release);
            })
            .await;

        release_tx.send(()).unwrap();
        dispatcher.wait_idle().await;

        assert!(!superseded_ran.load(Ordering::Acquire));
        assert!(latest_ran.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn saturated_effect_queue_keeps_the_newest_same_key_intent() {
        let dispatcher = ClientEffectDispatcher::new();
        let (active_started_tx, active_started_rx) = tokio::sync::oneshot::channel();
        let (release_active_tx, release_active_rx) = tokio::sync::oneshot::channel();
        let (queued_started_tx, queued_started_rx) = tokio::sync::oneshot::channel();
        let (release_queued_tx, release_queued_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("active"), async move {
                let _ = active_started_tx.send(());
                let _ = release_active_rx.await;
            })
            .await;
        active_started_rx.await.unwrap();
        dispatcher
            .enqueue_latest(
                ClientEffectKey::document_for_test("queued-blocker"),
                async move {
                    let _ = queued_started_tx.send(());
                    let _ = release_queued_rx.await;
                },
            )
            .await;
        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT - 1 {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let older_ran = Arc::new(AtomicBool::new(false));
        let older_marker = Arc::clone(&older_ran);
        let older = dispatcher.enqueue_latest(ClientEffectKey::LogMessage, async move {
            older_marker.store(true, Ordering::Release);
        });
        tokio::pin!(older);
        assert!(futures::poll!(&mut older).is_pending());

        let newer_ran = Arc::new(AtomicBool::new(false));
        let newer_marker = Arc::clone(&newer_ran);
        let newer = dispatcher.enqueue_latest(ClientEffectKey::LogMessage, async move {
            newer_marker.store(true, Ordering::Release);
        });
        tokio::pin!(newer);
        assert!(futures::poll!(&mut newer).is_pending());
        assert!(futures::poll!(&mut older).is_ready());

        release_active_tx.send(()).unwrap();
        queued_started_rx.await.unwrap();

        assert!(futures::poll!(&mut newer).is_ready());

        release_queued_tx.send(()).unwrap();
        dispatcher.wait_idle().await;

        assert!(!older_ran.load(Ordering::Acquire));
        assert!(newer_ran.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropped_waiting_effects_release_their_latest_intents() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("active"), async move {
                let _ = started_tx.send(());
                std::future::pending::<()>().await;
            })
            .await;
        started_rx.await.unwrap();
        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let baseline = dispatcher.latest_intent_count();
        for index in 0..4 {
            let mut waiting = Box::pin(dispatcher.enqueue_latest(
                ClientEffectKey::document_for_test(format!("dropped-{index}")),
                async {},
            ));
            assert!(futures::poll!(&mut waiting).is_pending());
            assert_eq!(dispatcher.latest_intent_count(), baseline + 1);
            drop(waiting);
            assert_eq!(dispatcher.latest_intent_count(), baseline);
        }

        dispatcher.cancel();
        dispatcher.wait_idle().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelling_client_effects_waits_for_the_active_future_to_drop() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (effect_alive_tx, effect_dropped_rx) = tokio::sync::oneshot::channel::<()>();

        dispatcher
            .enqueue_latest(ClientEffectKey::AllDiagnostics, async move {
                let _effect_alive = effect_alive_tx;
                let _ = started_tx.send(());
                std::future::pending::<()>().await;
            })
            .await;
        started_rx.await.expect("client effect should start");

        dispatcher.cancel();
        dispatcher.wait_idle().await;

        assert!(
            effect_dropped_rx.await.is_err(),
            "idle must mean the aborted effect future has actually been dropped"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn distinct_client_effects_apply_backpressure_at_the_queue_limit() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        dispatcher
            .enqueue_latest(ClientEffectKey::document_for_test("blocker"), async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
            })
            .await;
        started_rx.await.unwrap();

        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::document_for_test(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let overflow_dispatcher = dispatcher.clone();
        let (admitted_tx, mut admitted_rx) = tokio::sync::oneshot::channel();
        let overflow = tokio::spawn(async move {
            overflow_dispatcher
                .enqueue_latest(ClientEffectKey::document_for_test("overflow"), async {})
                .await;
            let _ = admitted_tx.send(());
        });
        assert!(matches!(futures::poll!(&mut admitted_rx), Poll::Pending));

        release_tx.send(()).unwrap();
        admitted_rx.await.unwrap();
        overflow.await.unwrap();
        dispatcher.wait_idle().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn active_effect_is_not_preempted_by_new_same_key_intent() {
        let dispatcher = ClientEffectDispatcher::new();
        let key = ClientEffectKey::document_for_test("same");
        let (active_started_tx, active_started_rx) = tokio::sync::oneshot::channel();
        let (release_active_tx, release_active_rx) = tokio::sync::oneshot::channel();
        let (active_done_tx, active_done_rx) = tokio::sync::oneshot::channel();
        let (queued_done_tx, mut queued_done_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(key.clone(), async move {
                let _ = active_started_tx.send(());
                let _ = release_active_rx.await;
                let _ = active_done_tx.send(());
            })
            .await;
        active_started_rx.await.unwrap();

        dispatcher
            .enqueue_latest(key, async move {
                let _ = queued_done_tx.send(());
            })
            .await;
        assert!(matches!(futures::poll!(&mut queued_done_rx), Poll::Pending));

        release_active_tx.send(()).unwrap();
        active_done_rx.await.unwrap();
        queued_done_rx.await.unwrap();
        dispatcher.wait_idle().await;
    }
}
