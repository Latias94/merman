use crate::server::MermanLanguageServer;
use crate::sync::lock_recovering_poison;
use futures::FutureExt;
use futures::future::{AbortHandle, Abortable, BoxFuture};
use std::collections::{BTreeSet, VecDeque};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::Notify;
use tower::Service;
use tower_lsp_server::jsonrpc::{Request, Response};
use tower_lsp_server::{ExitedError, LspService};

tokio::task_local! {
    static ACTIVE_MUTATION: MutationCompletion;
}

/// The ordered JSON-RPC session exposed to embedded and stdio hosts.
///
/// Admission happens synchronously in [`Service::call`], before transport futures may be polled
/// concurrently. Reads therefore observe every earlier state mutation after it commits or aborts.
#[derive(Debug)]
pub struct MermanLspService {
    inner: LspService<MermanLanguageServer>,
    input_order: Arc<InputOrder>,
}

impl MermanLspService {
    pub(crate) fn new(inner: LspService<MermanLanguageServer>) -> Self {
        Self {
            inner,
            input_order: Arc::new(InputOrder::default()),
        }
    }

    #[cfg(test)]
    pub(crate) fn inner(&self) -> &MermanLanguageServer {
        self.inner.inner()
    }
}

impl Service<Request> for MermanLspService {
    type Response = Option<Response>;
    type Error = ExitedError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let admission = self.input_order.admit(request.method());
        let future = self.inner.call(request);

        match admission {
            Admission::Immediate => Box::pin(future),
            Admission::Read { order, target } => Box::pin(async move {
                order.wait_until_completed(target).await;
                future.await
            }),
            Admission::Mutation {
                order,
                predecessor,
                completion,
            } => Box::pin(async move {
                order.wait_until_completed(predecessor).await;
                ACTIVE_MUTATION.scope(completion, future).await
            }),
        }
    }
}

pub(crate) fn commit_active_mutation() {
    let _ = ACTIVE_MUTATION.try_with(MutationCompletion::complete);
}

type ClientEffect = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub(crate) const LSP_CLIENT_EFFECT_QUEUE_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClientEffectKey {
    Document(String),
    AllDiagnostics,
}

struct QueuedClientEffect {
    key: ClientEffectKey,
    effect: ClientEffect,
}

/// Serializes client-visible side effects with bounded, latest-intent admission.
#[derive(Clone)]
pub(crate) struct ClientEffectDispatcher {
    inner: Arc<ClientEffectDispatcherInner>,
}

struct ClientEffectDispatcherInner {
    state: Mutex<ClientEffectState>,
    capacity: Notify,
    idle: Notify,
}

#[derive(Default)]
struct ClientEffectState {
    queue: VecDeque<QueuedClientEffect>,
    running: bool,
    cancelled: bool,
    worker_abort: Option<AbortHandle>,
}

impl std::fmt::Debug for ClientEffectDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = lock_recovering_poison(&self.inner.state);
        formatter
            .debug_struct("ClientEffectDispatcher")
            .field("queued", &state.queue.len())
            .field("running", &state.running)
            .field("cancelled", &state.cancelled)
            .finish()
    }
}

impl ClientEffectDispatcher {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(ClientEffectDispatcherInner {
                state: Mutex::new(ClientEffectState::default()),
                capacity: Notify::new(),
                idle: Notify::new(),
            }),
        }
    }

    pub(crate) async fn enqueue_latest(
        &self,
        key: ClientEffectKey,
        effect: impl Future<Output = ()> + Send + 'static,
    ) {
        let mut effect = Some(Box::pin(effect) as ClientEffect);
        let registration = loop {
            let capacity = self.inner.capacity.notified();
            let admission = {
                let mut state = lock_recovering_poison(&self.inner.state);
                if state.cancelled {
                    return;
                }
                if let Some(index) = state.queue.iter().position(|queued| queued.key == key) {
                    state.queue.remove(index);
                }
                if state.queue.len() >= LSP_CLIENT_EFFECT_QUEUE_LIMIT {
                    None
                } else {
                    state.queue.push_back(QueuedClientEffect {
                        key: key.clone(),
                        effect: effect
                            .take()
                            .expect("a client effect is admitted at most once"),
                    });
                    let registration = if state.running {
                        None
                    } else {
                        state.running = true;
                        let (abort, registration) = AbortHandle::new_pair();
                        state.worker_abort = Some(abort);
                        Some(registration)
                    };
                    Some(registration)
                }
            };
            if let Some(registration) = admission {
                break registration;
            }
            capacity.await;
        };

        if let Some(registration) = registration {
            let dispatcher = self.clone();
            tokio::spawn(async move {
                let _ = Abortable::new(dispatcher.drain(), registration).await;
            });
        }
    }

    pub(crate) fn cancel(&self) {
        let worker_abort = {
            let mut state = lock_recovering_poison(&self.inner.state);
            state.cancelled = true;
            state.running = false;
            state.queue.clear();
            state.worker_abort.take()
        };
        if let Some(worker_abort) = worker_abort {
            worker_abort.abort();
        }
        self.inner.capacity.notify_waiters();
        self.inner.idle.notify_waiters();
    }

    #[cfg(test)]
    pub(crate) async fn wait_idle(&self) {
        loop {
            let idle = self.inner.idle.notified();
            let is_idle = {
                let state = lock_recovering_poison(&self.inner.state);
                !state.running && state.queue.is_empty()
            };
            if is_idle {
                return;
            }
            idle.await;
        }
    }

    async fn drain(&self) {
        loop {
            let queued = {
                let mut state = lock_recovering_poison(&self.inner.state);
                if state.cancelled {
                    return;
                }
                let queued = state.queue.pop_front();
                if queued.is_none() {
                    state.running = false;
                    state.worker_abort = None;
                }
                queued
            };
            let Some(queued) = queued else {
                self.inner.idle.notify_waiters();
                return;
            };
            self.inner.capacity.notify_waiters();
            if AssertUnwindSafe(queued.effect)
                .catch_unwind()
                .await
                .is_err()
            {
                tracing::error!("LSP client effect panicked");
            }
        }
    }
}

#[derive(Debug)]
enum Admission {
    Immediate,
    Read {
        order: Arc<InputOrder>,
        target: u64,
    },
    Mutation {
        order: Arc<InputOrder>,
        predecessor: u64,
        completion: MutationCompletion,
    },
}

#[derive(Debug, Default)]
struct InputOrder {
    state: Mutex<InputOrderState>,
    changed: Notify,
}

#[derive(Debug, Default)]
struct InputOrderState {
    latest_mutation: u64,
    completed_mutation: u64,
    completed_out_of_order: BTreeSet<u64>,
}

impl InputOrder {
    fn admit(self: &Arc<Self>, method: &str) -> Admission {
        match method_class(method) {
            MethodClass::Control => Admission::Immediate,
            MethodClass::Read => Admission::Read {
                order: Arc::clone(self),
                target: lock_recovering_poison(&self.state).latest_mutation,
            },
            MethodClass::Mutation => {
                let sequence = {
                    let mut state = lock_recovering_poison(&self.state);
                    state.latest_mutation = state
                        .latest_mutation
                        .checked_add(1)
                        .expect("LSP mutation sequence exhausted");
                    state.latest_mutation
                };
                Admission::Mutation {
                    order: Arc::clone(self),
                    predecessor: sequence - 1,
                    completion: MutationCompletion::new(Arc::clone(self), sequence),
                }
            }
        }
    }

    async fn wait_until_completed(&self, target: u64) {
        loop {
            let changed = self.changed.notified();
            if lock_recovering_poison(&self.state).completed_mutation >= target {
                return;
            }
            changed.await;
        }
    }

    fn complete(&self, sequence: u64) {
        let mut state = lock_recovering_poison(&self.state);
        if sequence <= state.completed_mutation {
            return;
        }
        state.completed_out_of_order.insert(sequence);
        loop {
            let next = state.completed_mutation + 1;
            if !state.completed_out_of_order.remove(&next) {
                break;
            }
            state.completed_mutation += 1;
        }
        drop(state);
        self.changed.notify_waiters();
    }
}

#[derive(Debug, Clone)]
struct MutationCompletion(Arc<MutationCompletionInner>);

impl MutationCompletion {
    fn new(order: Arc<InputOrder>, sequence: u64) -> Self {
        Self(Arc::new(MutationCompletionInner {
            order,
            sequence,
            completed: AtomicBool::new(false),
        }))
    }

    fn complete(&self) {
        self.0.complete();
    }
}

#[derive(Debug)]
struct MutationCompletionInner {
    order: Arc<InputOrder>,
    sequence: u64,
    completed: AtomicBool,
}

impl MutationCompletionInner {
    fn complete(&self) {
        if !self.completed.swap(true, Ordering::AcqRel) {
            self.order.complete(self.sequence);
        }
    }
}

impl Drop for MutationCompletionInner {
    fn drop(&mut self) {
        self.complete();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MethodClass {
    Control,
    Mutation,
    Read,
}

fn method_class(method: &str) -> MethodClass {
    match method {
        "$/cancelRequest" | "exit" => MethodClass::Control,
        "initialize"
        | "textDocument/didOpen"
        | "textDocument/didChange"
        | "textDocument/didSave"
        | "textDocument/didClose"
        | "workspace/didChangeConfiguration" => MethodClass::Mutation,
        _ => MethodClass::Read,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_methods_have_explicit_ordering_classes() {
        assert_eq!(method_class("$/cancelRequest"), MethodClass::Control);
        assert_eq!(method_class("exit"), MethodClass::Control);
        assert_eq!(method_class("initialize"), MethodClass::Mutation);
        assert_eq!(
            method_class("textDocument/didChange"),
            MethodClass::Mutation
        );
        assert_eq!(method_class("textDocument/completion"), MethodClass::Read);
        assert_eq!(method_class("textDocument/didSave"), MethodClass::Mutation);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropped_mutations_do_not_leave_admission_gaps() {
        let order = Arc::new(InputOrder::default());
        let first = order.admit("textDocument/didOpen");
        let second = order.admit("textDocument/didChange");
        let read = order.admit("textDocument/completion");

        drop(second);
        drop(first);

        let Admission::Read { order, target } = read else {
            panic!("completion should use a read admission");
        };
        order.wait_until_completed(target).await;
        assert_eq!(target, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn client_effects_finish_in_enqueue_order() {
        let dispatcher = ClientEffectDispatcher::new();
        let (release_first_tx, release_first_rx) = tokio::sync::oneshot::channel();
        let (first_done_tx, first_done_rx) = tokio::sync::oneshot::channel();
        let (second_done_tx, mut second_done_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(ClientEffectKey::Document("first".to_string()), async move {
                let _ = release_first_rx.await;
                let _ = first_done_tx.send(());
            })
            .await;
        dispatcher
            .enqueue_latest(
                ClientEffectKey::Document("second".to_string()),
                async move {
                    let _ = second_done_tx.send(());
                },
            )
            .await;

        assert!(matches!(
            futures::poll!(&mut second_done_rx),
            std::task::Poll::Pending
        ));
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
            .enqueue_latest(
                ClientEffectKey::Document("blocker".to_string()),
                async move {
                    let _ = started_tx.send(());
                    let _ = release_rx.await;
                },
            )
            .await;
        started_rx.await.unwrap();

        let superseded_marker = Arc::clone(&superseded_ran);
        dispatcher
            .enqueue_latest(ClientEffectKey::Document("same".to_string()), async move {
                superseded_marker.store(true, Ordering::Release);
            })
            .await;
        let latest_marker = Arc::clone(&latest_ran);
        dispatcher
            .enqueue_latest(ClientEffectKey::Document("same".to_string()), async move {
                latest_marker.store(true, Ordering::Release);
            })
            .await;

        release_tx.send(()).unwrap();
        dispatcher.wait_idle().await;

        assert!(!superseded_ran.load(Ordering::Acquire));
        assert!(latest_ran.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn distinct_client_effects_apply_backpressure_at_the_queue_limit() {
        let dispatcher = ClientEffectDispatcher::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        dispatcher
            .enqueue_latest(
                ClientEffectKey::Document("blocker".to_string()),
                async move {
                    let _ = started_tx.send(());
                    let _ = release_rx.await;
                },
            )
            .await;
        started_rx.await.unwrap();

        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::Document(format!("queued-{index}")),
                    async {},
                )
                .await;
        }

        let overflow_dispatcher = dispatcher.clone();
        let (admitted_tx, mut admitted_rx) = tokio::sync::oneshot::channel();
        let overflow = tokio::spawn(async move {
            overflow_dispatcher
                .enqueue_latest(ClientEffectKey::Document("overflow".to_string()), async {})
                .await;
            let _ = admitted_tx.send(());
        });
        assert!(matches!(
            futures::poll!(&mut admitted_rx),
            std::task::Poll::Pending
        ));

        release_tx.send(()).unwrap();
        admitted_rx.await.unwrap();
        overflow.await.unwrap();
        dispatcher.wait_idle().await;
    }
}
