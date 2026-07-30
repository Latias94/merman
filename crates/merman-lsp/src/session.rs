use self::documents::SessionState;
use self::lifecycle::SessionLifecycle;
use crate::refresh_coordinator::RefreshCoordinator;
use crate::refresh_transport::RefreshClient;
use crate::server::MermanLanguageServer;
use crate::sync::lock_recovering_poison;
use futures::FutureExt;
use futures::future::{AbortHandle, AbortRegistration, Abortable, BoxFuture};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::Notify;
use tower::Service;
use tower_lsp_server::jsonrpc::{Error, Id, Request, Response};
use tower_lsp_server::ls_types::CancelParams;
use tower_lsp_server::{ExitedError, LspService};

mod analysis;
#[cfg(test)]
mod analysis_tests;
mod cache;
mod documents;
mod lifecycle;

pub(crate) use documents::{
    DEFAULT_LSP_MAX_SOURCE_BYTES, DiagnosticContext, DocumentDiagnosticState, DocumentSyncError,
    SemanticTokensState, StoredDocument, analysis_options_with_lsp_resource_defaults,
    default_lsp_analysis_options,
};
#[cfg(test)]
pub(crate) use documents::{DocumentDiscardedSource, DocumentResourceLimit};

/// Owns all mutable language state and the workers derived from that state.
#[derive(Debug, Clone)]
pub(crate) struct LanguageSession {
    inner: Arc<LanguageSessionInner>,
}

#[derive(Debug)]
struct LanguageSessionInner {
    state: Arc<AsyncMutex<SessionState>>,
    cancellation: merman_analysis::AnalysisCancellationToken,
    analysis_executor: analysis::executor::AnalysisExecutor,
    client_effects: ClientEffectDispatcher,
    refresh_coordinator: RefreshCoordinator,
    lifecycle: SessionLifecycle,
    input_order: Arc<InputOrder>,
    admission_cancellations: Arc<AdmissionCancellationRegistry>,
    protocol_service: Mutex<Option<Weak<Mutex<LspService<MermanLanguageServer>>>>>,
}

impl LanguageSessionInner {
    fn terminate(&self) -> bool {
        if !self.lifecycle.terminate() {
            return false;
        }

        self.exit_protocol_service();
        self.cancellation.cancel();
        self.analysis_executor.invalidate_all();
        self.admission_cancellations.cancel_all();
        self.client_effects.cancel();
        self.refresh_coordinator.cancel();
        true
    }

    fn exit_protocol_service(&self) {
        let Some(service) = lock_recovering_poison(&self.protocol_service)
            .as_ref()
            .and_then(Weak::upgrade)
        else {
            return;
        };

        // tower-lsp-server clears its private pending-request registry synchronously on exit.
        std::mem::drop(lock_recovering_poison(&service).call(Request::build("exit").finish()));
    }
}

impl Drop for LanguageSessionInner {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[derive(Debug)]
pub(crate) struct SessionEndpointGuard {
    session: Weak<LanguageSessionInner>,
}

impl SessionEndpointGuard {
    pub(crate) fn terminate(&self) {
        if let Some(session) = self.session.upgrade() {
            session.terminate();
        }
    }

    pub(crate) fn is_terminated(&self) -> bool {
        self.session
            .upgrade()
            .is_none_or(|session| session.lifecycle.is_terminated())
    }
}

impl Drop for SessionEndpointGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

impl LanguageSession {
    #[cfg(test)]
    pub(crate) fn with_cancellation(
        cancellation: merman_analysis::AnalysisCancellationToken,
    ) -> Self {
        let state = SessionState::with_session_cancellation(cancellation.clone());
        let (refresh_client, _, _) = RefreshClient::channel();
        Self::from_state(state, cancellation, refresh_client)
    }

    pub(crate) fn with_refresh_client(refresh_client: RefreshClient) -> Self {
        let cancellation = merman_analysis::AnalysisCancellationToken::new();
        let state = SessionState::with_session_cancellation(cancellation.clone());
        Self::from_state(state, cancellation, refresh_client)
    }

    fn from_state(
        state: SessionState,
        cancellation: merman_analysis::AnalysisCancellationToken,
        refresh_client: RefreshClient,
    ) -> Self {
        let analysis_executor = state.analysis_executor();
        Self {
            inner: Arc::new(LanguageSessionInner {
                state: Arc::new(AsyncMutex::new(state)),
                cancellation,
                analysis_executor,
                client_effects: ClientEffectDispatcher::new(),
                refresh_coordinator: RefreshCoordinator::new(refresh_client),
                lifecycle: SessionLifecycle::default(),
                input_order: Arc::new(InputOrder::default()),
                admission_cancellations: Arc::new(AdmissionCancellationRegistry::default()),
                protocol_service: Mutex::new(None),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_analysis_cache_budget(analysis_cache_budget: usize) -> Self {
        let cancellation = merman_analysis::AnalysisCancellationToken::new();
        let state = SessionState::with_session_cancellation_and_cache_budget(
            cancellation.clone(),
            analysis_cache_budget,
        );
        let (refresh_client, _, _) = RefreshClient::channel();
        Self::from_state(state, cancellation, refresh_client)
    }

    #[cfg(test)]
    pub(crate) fn with_analyzer_for_tests(analyzer: merman_analysis::Analyzer) -> Self {
        let cancellation = merman_analysis::AnalysisCancellationToken::new();
        let state = SessionState::with_analyzer_and_cache_budget(
            analyzer,
            cancellation.clone(),
            documents::DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES,
        );
        let (refresh_client, _, _) = RefreshClient::channel();
        Self::from_state(state, cancellation, refresh_client)
    }

    pub(crate) fn endpoint_guard(&self) -> SessionEndpointGuard {
        SessionEndpointGuard {
            session: Arc::downgrade(&self.inner),
        }
    }

    fn attach_protocol_service(&self, service: &Arc<Mutex<LspService<MermanLanguageServer>>>) {
        let mut protocol_service = lock_recovering_poison(&self.inner.protocol_service);
        debug_assert!(protocol_service.is_none());
        *protocol_service = Some(Arc::downgrade(service));
    }

    pub(crate) fn terminate(&self) -> bool {
        self.inner.terminate()
    }

    fn is_terminated(&self) -> bool {
        self.inner.lifecycle.is_terminated()
    }

    async fn terminated(&self) {
        self.inner.lifecycle.terminated().await;
    }

    pub(crate) async fn enqueue_latest_client_effect(
        &self,
        key: ClientEffectKey,
        effect: impl Future<Output = ()> + Send + 'static,
    ) {
        self.inner.client_effects.enqueue_latest(key, effect).await;
    }

    pub(crate) fn request_refresh(&self, semantic_tokens: bool, diagnostics: bool) {
        self.inner
            .refresh_coordinator
            .request(semantic_tokens, diagnostics);
    }

    #[cfg(test)]
    pub(crate) fn client_effect_admission_count(&self) -> usize {
        self.inner.client_effects.admission_count()
    }

    #[cfg(test)]
    pub(crate) fn refresh_request_counts(&self) -> (usize, usize) {
        self.inner.refresh_coordinator.request_counts()
    }

    #[cfg(test)]
    pub(crate) fn analysis_execution_count(&self) -> usize {
        self.inner.analysis_executor.execution_count()
    }

    #[cfg(test)]
    pub(crate) fn diagnostic_reprojection_count(&self) -> usize {
        self.inner.analysis_executor.reprojection_count()
    }

    #[cfg(test)]
    pub(crate) fn analysis_registry_state(&self) -> (usize, usize, usize) {
        self.inner.analysis_executor.registry_state()
    }

    #[cfg(test)]
    pub(crate) fn probe(&self) -> SessionProbe {
        SessionProbe {
            state: Arc::clone(&self.inner.state),
        }
    }

    #[cfg(test)]
    pub(crate) async fn wait_client_effects_idle(&self) {
        self.inner.client_effects.wait_idle().await;
    }

    #[cfg(test)]
    pub(crate) fn termination_count(&self) -> usize {
        self.inner.lifecycle.termination_count()
    }

    #[cfg(test)]
    pub(crate) fn analysis_is_cancelled(&self) -> bool {
        self.inner.cancellation.is_cancelled()
    }

    #[cfg(test)]
    pub(crate) async fn wait_stopped(&self) {
        self.inner.client_effects.wait_idle().await;
        self.inner.refresh_coordinator.wait_stopped().await;
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct SessionProbe {
    state: Arc<AsyncMutex<SessionState>>,
}

#[cfg(test)]
impl SessionProbe {
    pub(crate) async fn document(
        &self,
        uri: &tower_lsp_server::ls_types::Uri,
    ) -> Option<documents::StoredDocument> {
        self.state.lock().await.get(uri).cloned()
    }

    pub(crate) async fn cache_state(&self, uri: &tower_lsp_server::ls_types::Uri) -> (bool, bool) {
        let state = self.state.lock().await;
        (state.has_snapshot(uri), state.has_analysis_payload(uri))
    }

    pub(crate) async fn cached_snapshot(
        &self,
        uri: &tower_lsp_server::ls_types::Uri,
    ) -> Option<Arc<crate::snapshot::DocumentSnapshot>> {
        self.state.lock().await.cached_snapshot_for_probe(uri)
    }
}

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
    _endpoint: SessionEndpointGuard,
    inner: Arc<Mutex<LspService<MermanLanguageServer>>>,
    session: LanguageSession,
    #[cfg(test)]
    backend: Option<MermanLanguageServer>,
}

impl MermanLspService {
    pub(crate) fn new(inner: LspService<MermanLanguageServer>, session: LanguageSession) -> Self {
        let inner = Arc::new(Mutex::new(inner));
        session.attach_protocol_service(&inner);
        Self {
            _endpoint: session.endpoint_guard(),
            inner,
            session,
            #[cfg(test)]
            backend: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_backend_for_tests(
        inner: LspService<MermanLanguageServer>,
        session: LanguageSession,
        backend: MermanLanguageServer,
    ) -> Self {
        let mut service = Self::new(inner, session);
        service.backend = Some(backend);
        service
    }

    #[cfg(test)]
    pub(crate) fn inner(&self) -> &MermanLanguageServer {
        self.backend
            .as_ref()
            .expect("test services retain their backend probe")
    }

    fn route(
        &self,
        request: Request,
        admission: Admission,
        cancellation: Option<AdmissionCancellation>,
    ) -> BoxFuture<'static, Result<Option<Response>, ExitedError>> {
        let inner = Arc::clone(&self.inner);
        let session = self.session.clone();
        let preserve_ready_response = request.method() == "shutdown" && request.id().is_some();
        match admission {
            Admission::Immediate => route_admitted(
                inner,
                session,
                request,
                cancellation,
                preserve_ready_response,
            ),
            Admission::Read { order, target } => Box::pin(async move {
                match wait_for_admission(&order, target, &session, cancellation.as_ref()).await {
                    AdmissionWait::Ready => {}
                    AdmissionWait::RequestCancelled => {
                        let cancellation = cancellation
                            .expect("only registered requests can cancel ordered admission");
                        return Ok(Some(cancellation.into_response()));
                    }
                    AdmissionWait::SessionTerminated => return Ok(None),
                }
                route_admitted(
                    inner,
                    session,
                    request,
                    cancellation,
                    preserve_ready_response,
                )
                .await
            }),
            Admission::Mutation {
                order,
                predecessor,
                completion,
            } => {
                if preserve_ready_response && order.is_completed(predecessor) {
                    let future = route_admitted(
                        inner,
                        session,
                        request,
                        cancellation,
                        preserve_ready_response,
                    );
                    return Box::pin(ACTIVE_MUTATION.scope(completion, future));
                }
                Box::pin(async move {
                    match wait_for_admission(&order, predecessor, &session, cancellation.as_ref())
                        .await
                    {
                        AdmissionWait::Ready => {}
                        AdmissionWait::RequestCancelled => {
                            let cancellation = cancellation
                                .expect("only registered requests can cancel ordered admission");
                            return Ok(Some(cancellation.into_response()));
                        }
                        AdmissionWait::SessionTerminated => return Ok(None),
                    }
                    let future = route_admitted(
                        inner,
                        session,
                        request,
                        cancellation,
                        preserve_ready_response,
                    );
                    ACTIVE_MUTATION.scope(completion, future).await
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdmissionWait {
    Ready,
    RequestCancelled,
    SessionTerminated,
}

async fn wait_for_admission(
    order: &InputOrder,
    target: u64,
    session: &LanguageSession,
    cancellation: Option<&AdmissionCancellation>,
) -> AdmissionWait {
    let request_cancelled = async {
        match cancellation {
            Some(cancellation) => cancellation.cancelled().await,
            None => std::future::pending().await,
        }
    };
    tokio::select! {
        biased;
        () = session.terminated() => AdmissionWait::SessionTerminated,
        () = request_cancelled => AdmissionWait::RequestCancelled,
        () = order.wait_until_completed(target) => AdmissionWait::Ready,
    }
}

fn route_admitted(
    inner: Arc<Mutex<LspService<MermanLanguageServer>>>,
    session: LanguageSession,
    request: Request,
    cancellation: Option<AdmissionCancellation>,
    preserve_ready_response: bool,
) -> BoxFuture<'static, Result<Option<Response>, ExitedError>> {
    if session.is_terminated() {
        return Box::pin(async { Ok(None) });
    }

    // `LspService::call` performs lifecycle routing synchronously. Keep that call behind ordered
    // admission, but release the mutex before polling the handler so unrelated reads can overlap.
    let future = match cancellation {
        Some(cancellation) => match cancellation.route(&inner, request) {
            AdmissionRoute::Cancelled(id) => {
                return Box::pin(async move {
                    Ok(Some(Response::from_error(id, Error::request_cancelled())))
                });
            }
            AdmissionRoute::Routed(future) => future,
        },
        None => lock_recovering_poison(&inner).call(request),
    };
    if preserve_ready_response {
        Box::pin(async move {
            tokio::select! {
                biased;
                result = future => result,
                () = session.terminated() => Ok(None),
            }
        })
    } else {
        Box::pin(async move {
            tokio::select! {
                biased;
                () = session.terminated() => Ok(None),
                result = future => result,
            }
        })
    }
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
        if request.method() == "exit" && request.id().is_none() {
            let future = lock_recovering_poison(&self.inner).call(request);
            self.session.terminate();
            return future;
        }
        if self.session.is_terminated() {
            return Box::pin(async { Ok(None) });
        }
        if let Some(id) = cancellation_request_id(&request) {
            self.session.inner.admission_cancellations.cancel(&id);
        }
        let cancellation = request
            .id()
            .cloned()
            .and_then(|id| self.session.inner.admission_cancellations.register(id));
        let admission = self
            .session
            .inner
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
    pending: Mutex<HashMap<Id, Arc<AdmissionCancellationState>>>,
}

impl AdmissionCancellationRegistry {
    fn register(self: &Arc<Self>, id: Id) -> Option<AdmissionCancellation> {
        let state = Arc::new(AdmissionCancellationState::default());
        let mut pending = lock_recovering_poison(&self.pending);
        if pending.contains_key(&id) {
            return None;
        }
        pending.insert(id.clone(), Arc::clone(&state));
        Some(AdmissionCancellation {
            registry: Arc::clone(self),
            id,
            state,
            registered: true,
        })
    }

    fn cancel(&self, id: &Id) {
        if let Some(state) = lock_recovering_poison(&self.pending).get(id) {
            state.cancel();
        }
    }

    fn cancel_all(&self) {
        let mut pending = lock_recovering_poison(&self.pending);
        for state in pending.values() {
            state.cancel();
        }
        pending.clear();
    }
}

#[derive(Debug, Default)]
struct AdmissionCancellationState {
    cancelled: AtomicBool,
    changed: Notify,
}

impl AdmissionCancellationState {
    fn cancel(&self) {
        if !self.cancelled.swap(true, Ordering::AcqRel) {
            self.changed.notify_one();
        }
    }

    async fn cancelled(&self) {
        if self.cancelled.load(Ordering::Acquire) {
            return;
        }
        self.changed.notified().await;
    }
}

#[derive(Debug)]
struct AdmissionCancellation {
    registry: Arc<AdmissionCancellationRegistry>,
    id: Id,
    state: Arc<AdmissionCancellationState>,
    registered: bool,
}

enum AdmissionRoute {
    Cancelled(Id),
    Routed(BoxFuture<'static, Result<Option<Response>, ExitedError>>),
}

impl AdmissionCancellation {
    async fn cancelled(&self) {
        self.state.cancelled().await;
    }

    fn into_response(self) -> Response {
        Response::from_error(self.id.clone(), Error::request_cancelled())
    }

    fn route(
        mut self,
        inner: &Mutex<LspService<MermanLanguageServer>>,
        request: Request,
    ) -> AdmissionRoute {
        let mut pending = lock_recovering_poison(&self.registry.pending);
        let route = if self.state.cancelled.load(Ordering::Acquire) {
            AdmissionRoute::Cancelled(self.id.clone())
        } else {
            // The registry lock closes the gap between wrapper admission and tower-lsp-server's
            // own synchronous pending-request registration.
            AdmissionRoute::Routed(lock_recovering_poison(inner).call(request))
        };
        if pending
            .get(&self.id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.state))
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
            .is_some_and(|current| Arc::ptr_eq(current, &self.state))
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ClientEffectKey {
    Document(String),
    AllDiagnostics,
    LogMessage,
}

struct QueuedClientEffect {
    key: ClientEffectKey,
    intent: u64,
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
    latest_intents: HashMap<ClientEffectKey, u64>,
    cancelled: bool,
    next_intent_id: u64,
    next_worker_id: u64,
    active_worker: Option<ActiveClientEffectWorker>,
    #[cfg(test)]
    admitted_count: usize,
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
        let intent = {
            let mut state = lock_recovering_poison(&inner.state);
            if state.cancelled {
                return None;
            }
            state.next_intent_id = state
                .next_intent_id
                .checked_add(1)
                .expect("client effect intent sequence exhausted");
            let intent = state.next_intent_id;
            state.latest_intents.insert(key.clone(), intent);
            intent
        };
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
        let mut state = lock_recovering_poison(&self.inner.state);
        if state.latest_intents.get(&self.key) == Some(&self.intent) {
            state.latest_intents.remove(&self.key);
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
        let Some(mut intent) =
            ClientEffectIntentGuard::register(Arc::clone(&self.inner), key.clone())
        else {
            return;
        };
        let intent_id = intent.intent();
        let mut effect = Some(Box::pin(effect) as ClientEffect);
        let registration = loop {
            let capacity = self.inner.capacity.notified();
            let admission = {
                let mut state = lock_recovering_poison(&self.inner.state);
                if state.cancelled || state.latest_intents.get(&key) != Some(&intent_id) {
                    return;
                }
                if let Some(index) = state.queue.iter().position(|queued| queued.key == key) {
                    debug_assert!(state.queue[index].intent < intent_id);
                    state.queue.remove(index);
                }
                if state.queue.len() >= LSP_CLIENT_EFFECT_QUEUE_LIMIT {
                    None
                } else {
                    state.queue.push_back(QueuedClientEffect {
                        key: key.clone(),
                        intent: intent_id,
                        effect: effect
                            .take()
                            .expect("a client effect is admitted at most once"),
                    });
                    #[cfg(test)]
                    {
                        state.admitted_count += 1;
                    }
                    let registration = if state.active_worker.is_some() {
                        None
                    } else {
                        Some(Self::register_worker(&mut state))
                    };
                    Some(registration)
                }
            };
            if let Some(registration) = admission {
                intent.disarm();
                break registration;
            }
            capacity.await;
        };

        if let Some((worker_id, registration)) = registration {
            self.spawn_worker(worker_id, registration);
        }
    }

    pub(crate) fn cancel(&self) {
        let worker_abort = {
            let mut state = lock_recovering_poison(&self.inner.state);
            state.cancelled = true;
            state.queue.clear();
            state.latest_intents.clear();
            state
                .active_worker
                .as_ref()
                .map(|worker| worker.abort.clone())
        };
        if let Some(worker_abort) = worker_abort {
            worker_abort.abort();
        }
        self.inner.capacity.notify_waiters();
        self.inner.idle.notify_waiters();
    }

    #[cfg(test)]
    fn admission_count(&self) -> usize {
        lock_recovering_poison(&self.inner.state).admitted_count
    }

    #[cfg(test)]
    fn latest_intent_count(&self) -> usize {
        lock_recovering_poison(&self.inner.state)
            .latest_intents
            .len()
    }

    #[cfg(test)]
    pub(crate) async fn wait_idle(&self) {
        loop {
            let idle = self.inner.idle.notified();
            let is_idle = {
                let state = lock_recovering_poison(&self.inner.state);
                state.active_worker.is_none() && state.queue.is_empty()
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
                if let Some(queued) = queued.as_ref()
                    && state.latest_intents.get(&queued.key) == Some(&queued.intent)
                {
                    state.latest_intents.remove(&queued.key);
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

        self.inner.capacity.notify_waiters();
        if let Some((worker_id, registration)) = replacement {
            self.spawn_worker(worker_id, registration);
        } else {
            self.inner.idle.notify_waiters();
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

    fn is_completed(&self, target: u64) -> bool {
        lock_recovering_poison(&self.state).completed_mutation >= target
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
    use tower::{Service, ServiceExt};

    async fn initialize_service(service: &mut MermanLspService) {
        let response = service
            .ready()
            .await
            .expect("service should accept initialization")
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({ "capabilities": {} }))
                    .id(1)
                    .finish(),
            )
            .await
            .expect("initialization should succeed")
            .expect("initialization should respond");
        assert!(response.is_ok());
    }

    fn did_open_request(uri: &str) -> Request {
        Request::build("textDocument/didOpen")
            .params(serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "mermaid",
                    "version": 1,
                    "text": "flowchart TD\\nA-->B\\n",
                },
            }))
            .finish()
    }

    fn hover_request(uri: &str, id: i64) -> Request {
        Request::build("textDocument/hover")
            .params(serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 0 },
            }))
            .id(id)
            .finish()
    }

    fn completion_request(uri: &str, id: i64) -> Request {
        Request::build("textDocument/completion")
            .params(serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 0 },
            }))
            .id(id)
            .finish()
    }

    fn cancel_request(id: i64) -> Request {
        Request::build("$/cancelRequest")
            .params(serde_json::json!({ "id": id }))
            .finish()
    }

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
    async fn cancellation_before_first_poll_releases_a_waiting_read() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let uri = "file:///tmp/cancel-before-first-poll.mmd";

        let open = service.call(did_open_request(uri));
        let hover = service.call(hover_request(uri, 2));

        assert!(service.call(cancel_request(2)).await.unwrap().is_none());
        let response = hover.await.unwrap().expect("cancelled hover response");
        assert_eq!(
            response.error().expect("request cancellation error").code,
            tower_lsp_server::jsonrpc::ErrorCode::RequestCancelled
        );

        assert!(open.await.unwrap().is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancellation_after_tower_handoff_aborts_the_registered_request() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let uri = "file:///tmp/cancel-after-handoff.mmd";
        assert!(service.call(did_open_request(uri)).await.unwrap().is_none());

        let state = Arc::clone(&service.session.inner.state);
        let state = state.lock().await;
        let completion = service.call(completion_request(uri, 2));
        tokio::pin!(completion);
        assert!(futures::poll!(&mut completion).is_pending());

        assert!(service.call(cancel_request(2)).await.unwrap().is_none());
        drop(state);

        let response = completion
            .await
            .unwrap()
            .expect("cancelled completion response");
        assert_eq!(
            response.error().expect("request cancellation error").code,
            tower_lsp_server::jsonrpc::ErrorCode::RequestCancelled
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn session_termination_exits_the_wrapped_tower_service() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let inner = Arc::clone(&service.inner);

        assert!(service.session.terminate());
        let request = lock_recovering_poison(&inner)
            .call(Request::build("textDocument/hover").id(2).finish());
        assert!(
            request.await.is_err(),
            "session termination must stop tower-lsp-server before the wrapper is dropped"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_waiting_mutation_completes_its_sequence_slot() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let uri = "file:///tmp/cancelled-mutation-slot.mmd";

        let open = service.call(did_open_request(uri));
        let shutdown = service.call(Request::build("shutdown").id(2).finish());
        tokio::pin!(shutdown);
        assert!(futures::poll!(&mut shutdown).is_pending());

        assert!(service.call(cancel_request(2)).await.unwrap().is_none());
        let response = shutdown
            .await
            .unwrap()
            .expect("cancelled shutdown response");
        assert_eq!(
            response.error().expect("request cancellation error").code,
            tower_lsp_server::jsonrpc::ErrorCode::RequestCancelled
        );

        let hover = service.call(hover_request(uri, 3));
        tokio::pin!(hover);
        assert!(futures::poll!(&mut hover).is_pending());

        assert!(open.await.unwrap().is_none());
        let response = hover
            .await
            .unwrap()
            .expect("hover response after the admitted open");
        assert!(response.is_ok());
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
    async fn saturated_effect_queue_keeps_the_newest_same_key_intent() {
        let dispatcher = ClientEffectDispatcher::new();
        let (active_started_tx, active_started_rx) = tokio::sync::oneshot::channel();
        let (release_active_tx, release_active_rx) = tokio::sync::oneshot::channel();
        let (queued_started_tx, queued_started_rx) = tokio::sync::oneshot::channel();
        let (release_queued_tx, release_queued_rx) = tokio::sync::oneshot::channel();

        dispatcher
            .enqueue_latest(
                ClientEffectKey::Document("active".to_string()),
                async move {
                    let _ = active_started_tx.send(());
                    let _ = release_active_rx.await;
                },
            )
            .await;
        active_started_rx.await.unwrap();
        dispatcher
            .enqueue_latest(
                ClientEffectKey::Document("queued-blocker".to_string()),
                async move {
                    let _ = queued_started_tx.send(());
                    let _ = release_queued_rx.await;
                },
            )
            .await;
        for index in 0..LSP_CLIENT_EFFECT_QUEUE_LIMIT - 1 {
            dispatcher
                .enqueue_latest(
                    ClientEffectKey::Document(format!("queued-{index}")),
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

        release_active_tx.send(()).unwrap();
        queued_started_rx.await.unwrap();

        assert!(futures::poll!(&mut newer).is_ready());
        assert!(futures::poll!(&mut older).is_ready());

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
            .enqueue_latest(
                ClientEffectKey::Document("active".to_string()),
                async move {
                    let _ = started_tx.send(());
                    std::future::pending::<()>().await;
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

        let baseline = dispatcher.latest_intent_count();
        for index in 0..4 {
            let mut waiting = Box::pin(dispatcher.enqueue_latest(
                ClientEffectKey::Document(format!("dropped-{index}")),
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
