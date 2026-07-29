use crate::server::MermanLanguageServer;
use crate::sync::lock_recovering_poison;
use futures::FutureExt;
use futures::future::{AbortHandle, Abortable, BoxFuture};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::Notify;
use tower::Service;
use tower_lsp_server::jsonrpc::{Error, Id, Request, Response};
use tower_lsp_server::ls_types::CancelParams;
use tower_lsp_server::{ExitedError, LspService};

pub(crate) mod cache;

tokio::task_local! {
    static ACTIVE_MUTATION: MutationCompletion;
}

/// Maximum number of handler futures an embedded or stdio transport should poll concurrently.
pub const LSP_HANDLER_CONCURRENCY: usize = 4;

/// Maximum encoded size of one LSP message accepted by the bundled transport.
pub const LSP_MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

/// Maximum aggregate encoded bytes retained by queued and running requests.
pub const LSP_REQUEST_BYTE_BUDGET: usize = LSP_MAX_MESSAGE_BYTES * LSP_HANDLER_CONCURRENCY;

/// The ordered JSON-RPC session exposed to embedded and stdio hosts.
///
/// Admission happens synchronously in [`Service::call`], before transport futures may be polled
/// concurrently. Reads therefore observe every earlier state mutation after it commits or aborts.
/// The transport owns its request queue and must apply [`LSP_HANDLER_CONCURRENCY`],
/// [`LSP_MAX_MESSAGE_BYTES`], and [`LSP_REQUEST_BYTE_BUDGET`] before retaining request futures.
#[derive(Debug)]
pub struct MermanLspService {
    inner: Arc<Mutex<LspService<MermanLanguageServer>>>,
    _backend: MermanLanguageServer,
    input_order: Arc<InputOrder>,
    admission_cancellations: Arc<AdmissionCancellationRegistry>,
}

impl MermanLspService {
    pub(crate) fn new(
        inner: LspService<MermanLanguageServer>,
        backend: MermanLanguageServer,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            _backend: backend,
            input_order: Arc::new(InputOrder::default()),
            admission_cancellations: Arc::new(AdmissionCancellationRegistry::default()),
        }
    }

    #[cfg(test)]
    pub(crate) fn inner(&self) -> &MermanLanguageServer {
        &self._backend
    }

    fn route(
        &self,
        request: Request,
        admission: Admission,
        cancellation: Option<AdmissionCancellation>,
    ) -> BoxFuture<'static, Result<Option<Response>, ExitedError>> {
        let inner = Arc::clone(&self.inner);
        match admission {
            Admission::Immediate => Box::pin(route_after_admission(inner, request, cancellation)),
            Admission::Read { order, target } => Box::pin(async move {
                order.wait_until_completed(target).await;
                route_after_admission(inner, request, cancellation).await
            }),
            Admission::Mutation {
                order,
                predecessor,
                completion,
            } => Box::pin(async move {
                order.wait_until_completed(predecessor).await;
                ACTIVE_MUTATION
                    .scope(
                        completion,
                        route_after_admission(inner, request, cancellation),
                    )
                    .await
            }),
        }
    }
}

async fn route_after_admission(
    inner: Arc<Mutex<LspService<MermanLanguageServer>>>,
    request: Request,
    cancellation: Option<AdmissionCancellation>,
) -> Result<Option<Response>, ExitedError> {
    // `LspService::call` performs lifecycle routing synchronously. Keep that call behind ordered
    // admission, but release the mutex before polling the handler so unrelated reads can overlap.
    let future = match cancellation {
        Some(cancellation) => match cancellation.route(&inner, request) {
            AdmissionRoute::Cancelled(id) => {
                return Ok(Some(Response::from_error(id, Error::request_cancelled())));
            }
            AdmissionRoute::Routed(future) => future,
        },
        None => lock_recovering_poison(&inner).call(request),
    };
    future.await
}

impl Service<Request> for MermanLspService {
    type Response = Option<Response>;
    type Error = ExitedError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // The wrapper has no request queue: the host owns and bounds retained futures. Keeping
        // admission ready also lets cancel and exit notifications reach queued handler futures.
        // Lifecycle errors are evaluated after earlier mutations complete.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        if let Some(id) = cancellation_request_id(&request) {
            self.admission_cancellations.cancel(&id);
        }
        let cancellation = request
            .id()
            .cloned()
            .and_then(|id| self.admission_cancellations.register(id));
        let admission = self
            .input_order
            .admit(request.method(), request.id().is_none());
        self.route(request, admission, cancellation)
    }
}

fn cancellation_request_id(request: &Request) -> Option<Id> {
    if request.method() != "$/cancelRequest" {
        return None;
    }
    serde_json::from_value::<CancelParams>(request.params()?.clone())
        .ok()
        .map(|params| params.id.into())
}

#[derive(Debug, Default)]
struct AdmissionCancellationRegistry {
    pending: Mutex<HashMap<Id, Arc<AtomicBool>>>,
}

impl AdmissionCancellationRegistry {
    fn register(self: &Arc<Self>, id: Id) -> Option<AdmissionCancellation> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut pending = lock_recovering_poison(&self.pending);
        if pending.contains_key(&id) {
            return None;
        }
        pending.insert(id.clone(), Arc::clone(&cancellation));
        Some(AdmissionCancellation {
            registry: Arc::clone(self),
            id,
            cancellation,
            registered: true,
        })
    }

    fn cancel(&self, id: &Id) {
        if let Some(cancellation) = lock_recovering_poison(&self.pending).get(id) {
            cancellation.store(true, Ordering::Release);
        }
    }
}

#[derive(Debug)]
struct AdmissionCancellation {
    registry: Arc<AdmissionCancellationRegistry>,
    id: Id,
    cancellation: Arc<AtomicBool>,
    registered: bool,
}

enum AdmissionRoute {
    Cancelled(Id),
    Routed(BoxFuture<'static, Result<Option<Response>, ExitedError>>),
}

impl AdmissionCancellation {
    fn route(
        mut self,
        inner: &Mutex<LspService<MermanLanguageServer>>,
        request: Request,
    ) -> AdmissionRoute {
        let mut pending = lock_recovering_poison(&self.registry.pending);
        let route = if self.cancellation.load(Ordering::Acquire) {
            AdmissionRoute::Cancelled(self.id.clone())
        } else {
            // The registry lock closes the gap between wrapper admission and tower-lsp-server's
            // own synchronous pending-request registration.
            AdmissionRoute::Routed(lock_recovering_poison(inner).call(request))
        };
        if pending
            .get(&self.id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.cancellation))
        {
            pending.remove(&self.id);
        }
        self.registered = false;
        route
    }
}

impl Drop for AdmissionCancellation {
    fn drop(&mut self) {
        if !self.registered {
            return;
        }
        let mut pending = lock_recovering_poison(&self.registry.pending);
        if pending
            .get(&self.id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.cancellation))
        {
            pending.remove(&self.id);
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
    fn admit(self: &Arc<Self>, method: &str, is_notification: bool) -> Admission {
        match method_class(method, is_notification) {
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

fn method_class(method: &str, is_notification: bool) -> MethodClass {
    match method {
        "$/cancelRequest" | "exit" => MethodClass::Control,
        "initialize" | "shutdown" => MethodClass::Mutation,
        _ if is_notification => MethodClass::Mutation,
        _ => MethodClass::Read,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_methods_have_explicit_ordering_classes() {
        assert_eq!(method_class("$/cancelRequest", true), MethodClass::Control);
        assert_eq!(method_class("exit", true), MethodClass::Control);
        assert_eq!(method_class("initialize", false), MethodClass::Mutation);
        assert_eq!(method_class("shutdown", false), MethodClass::Mutation);
        assert_eq!(
            method_class("textDocument/didChange", true),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class("future/notification", true),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class("textDocument/completion", false),
            MethodClass::Read
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropped_mutations_do_not_leave_admission_gaps() {
        let order = Arc::new(InputOrder::default());
        let first = order.admit("textDocument/didOpen", true);
        let second = order.admit("textDocument/didChange", true);
        let read = order.admit("textDocument/completion", false);

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
