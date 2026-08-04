use self::documents::SessionState;
use self::lifecycle::SessionLifecycle;
use crate::refresh_coordinator::RefreshCoordinator;
use crate::refresh_transport::RefreshClient;
use crate::server::MermanLanguageServer;
use crate::sync::lock_recovering_poison;
use futures::future::BoxFuture;
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::Notify;
use tower::Service;
use tower_lsp_server::jsonrpc::{Error, Id, Request, Response};
#[cfg(test)]
use tower_lsp_server::ls_types::Uri;
use tower_lsp_server::{ExitedError, LspService};

mod analysis;
mod analysis_cache;
#[cfg(test)]
mod analysis_tests;
mod cache;
mod client_effects;
mod documents;
mod lifecycle;

use client_effects::ClientEffectDispatcher;
pub(crate) use client_effects::ClientEffectKey;
#[cfg(test)]
pub(crate) use client_effects::LSP_CLIENT_EFFECT_QUEUE_LIMIT;

pub(crate) use documents::{
    DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS, DEFAULT_LSP_MAX_SOURCE_BYTES, DiagnosticContext,
    DocumentDiagnosticState, DocumentSyncError, SemanticTokensState, StoredDocument,
    analysis_options_with_lsp_resource_defaults, default_lsp_analysis_options,
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
}

impl LanguageSessionInner {
    fn terminate(&self) -> bool {
        if !self.lifecycle.terminate() {
            return false;
        }

        self.cancellation.cancel();
        self.analysis_executor.invalidate_all();
        self.admission_cancellations.cancel_all();
        self.client_effects.cancel();
        self.refresh_coordinator.cancel();
        true
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
        let service = Arc::downgrade(service);
        self.inner.lifecycle.register_termination_hook(move || {
            let Some(service) = service.upgrade() else {
                return;
            };

            // The exit layer updates its state and clears the pending-request registry inside
            // `Service::call`, so the returned ready future does not need a transport poll.
            std::mem::drop(lock_recovering_poison(&service).call(Request::build("exit").finish()));
        });
    }

    pub(crate) fn terminate(&self) -> bool {
        self.inner.terminate()
    }

    fn commit_state_if_active<T>(
        &self,
        state: &mut SessionState,
        mutation: impl FnOnce(&mut SessionState) -> T,
    ) -> Option<T> {
        self.inner.lifecycle.commit_if_active(|| mutation(state))
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
    pub(crate) fn analysis_waiter_count(&self, uri: &Uri) -> usize {
        self.inner.analysis_executor.waiter_count_for_uri(uri)
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
        self.inner.analysis_executor.wait_idle().await;
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

/// Default maximum number of ordinary data-plane handler futures polled concurrently.
pub const LSP_ORDINARY_HANDLER_CONCURRENCY: usize = 4;

/// Maximum encoded JSON body size accepted by the bundled transport.
///
/// This is the `Content-Length` value and does not include LSP framing headers.
pub const LSP_MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

/// Maximum aggregate encoded JSON body bytes retained by queued and running ordinary requests.
pub const LSP_REQUEST_BYTE_BUDGET: usize = LSP_MAX_MESSAGE_BYTES * LSP_ORDINARY_HANDLER_CONCURRENCY;

#[derive(Debug)]
pub(crate) struct ClassifiedRequest {
    request: Request,
    shape: ProtocolMessageShape,
}

impl ClassifiedRequest {
    pub(crate) fn new(request: Request) -> Self {
        let shape = ProtocolMessageShape::classify(&request);
        Self { request, shape }
    }

    #[cfg(feature = "stdio")]
    pub(crate) fn request(&self) -> &Request {
        &self.request
    }

    #[cfg(feature = "stdio")]
    pub(crate) fn shape(&self) -> ProtocolMessageShape {
        self.shape
    }

    #[cfg(feature = "stdio")]
    pub(crate) fn into_request(self) -> Request {
        self.request
    }

    fn into_parts(self) -> (Request, ProtocolMessageShape) {
        (self.request, self.shape)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtocolMessageShape {
    Ordinary,
    CancellationNotification,
    ShutdownRequest,
    ShutdownNotification,
    ExitRequest,
    InvalidExitNotification,
    ExitNotification,
}

impl ProtocolMessageShape {
    pub(crate) fn classify(request: &Request) -> Self {
        match (request.method(), request.id().is_some()) {
            ("$/cancelRequest", false)
                if request
                    .params()
                    .and_then(borrowed_cancellation_request_id)
                    .is_some() =>
            {
                Self::CancellationNotification
            }
            ("shutdown", true) => Self::ShutdownRequest,
            ("shutdown", false) => Self::ShutdownNotification,
            ("exit", true) => Self::ExitRequest,
            ("exit", false) if request.params().is_none_or(|params| params.is_null()) => {
                Self::ExitNotification
            }
            ("exit", false) => Self::InvalidExitNotification,
            _ => Self::Ordinary,
        }
    }

    pub(crate) fn is_exact_control_notification(&self) -> bool {
        matches!(
            self,
            Self::CancellationNotification | Self::ExitNotification
        )
    }

    #[cfg(feature = "stdio")]
    pub(crate) fn is_exit_notification(&self) -> bool {
        matches!(self, Self::ExitNotification)
    }
}

#[derive(Debug, Clone, Copy)]
enum BorrowedCancellationRequestId<'a> {
    Number(i32),
    String(&'a str),
}

impl BorrowedCancellationRequestId<'_> {
    fn into_owned(self) -> Id {
        match self {
            Self::Number(id) => Id::Number(i64::from(id)),
            Self::String(id) => Id::String(id.to_owned()),
        }
    }
}

fn borrowed_cancellation_request_id(
    params: &serde_json::Value,
) -> Option<BorrowedCancellationRequestId<'_>> {
    match params.as_object()?.get("id")? {
        serde_json::Value::Number(id) => {
            let id = i32::try_from(id.as_i64()?).ok()?;
            Some(BorrowedCancellationRequestId::Number(id))
        }
        serde_json::Value::String(id) => Some(BorrowedCancellationRequestId::String(id.as_str())),
        _ => None,
    }
}

/// The ordered JSON-RPC session exposed to embedded and stdio hosts.
///
/// Admission happens synchronously in [`Service::call`], before transport futures may be polled
/// concurrently. Reads therefore observe every earlier state mutation after it commits or aborts.
/// The transport owns its request queues and must apply [`LSP_ORDINARY_HANDLER_CONCURRENCY`],
/// [`LSP_MAX_MESSAGE_BYTES`], and [`LSP_REQUEST_BYTE_BUDGET`] before retaining request futures.
#[derive(Debug)]
pub struct MermanLspService {
    _endpoint: SessionEndpointGuard,
    inner: Arc<Mutex<LspService<MermanLanguageServer>>>,
    session: LanguageSession,
}

impl MermanLspService {
    pub(crate) fn new(inner: LspService<MermanLanguageServer>, session: LanguageSession) -> Self {
        let inner = Arc::new(Mutex::new(inner));
        session.attach_protocol_service(&inner);
        Self {
            _endpoint: session.endpoint_guard(),
            inner,
            session,
        }
    }

    fn route(
        inner: Arc<Mutex<LspService<MermanLanguageServer>>>,
        session: LanguageSession,
        request: Request,
        admission: Admission,
        cancellation: Option<AdmissionCancellation>,
    ) -> BoxFuture<'static, Result<Option<Response>, ExitedError>> {
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

    fn call_request(
        inner: Arc<Mutex<LspService<MermanLanguageServer>>>,
        session: LanguageSession,
        request: ClassifiedRequest,
    ) -> BoxFuture<'static, Result<Option<Response>, ExitedError>> {
        let (request, shape) = request.into_parts();
        let method_class = MethodClass::classify(&request, shape);
        let cancellation_request_id = match shape {
            ProtocolMessageShape::ShutdownNotification => {
                tracing::warn!("ignoring shutdown notification; shutdown must be a request");
                return Box::pin(async { Ok(None) });
            }
            ProtocolMessageShape::ExitRequest => {
                let (_, id, _) = request.into_parts();
                let id = id.expect("classified exit request must carry an ID");
                tracing::warn!("rejecting exit request; exit must be a notification");
                return Box::pin(async move {
                    Ok(Some(Response::from_error(id, Error::invalid_request())))
                });
            }
            ProtocolMessageShape::InvalidExitNotification => {
                tracing::warn!("ignoring exit notification with unexpected parameters");
                return Box::pin(async { Ok(None) });
            }
            ProtocolMessageShape::ExitNotification => {
                // The first session terminator owns the one raw `exit` handoff. It synchronously
                // clears tower-lsp-server's pending registry before the remaining workers are
                // cancelled, so this wrapper must not route the same message a second time.
                session.terminate();
                return Box::pin(async { Ok(None) });
            }
            ProtocolMessageShape::CancellationNotification => Some(
                borrowed_cancellation_request_id(
                    request
                        .params()
                        .expect("classified cancellation notification must carry parameters"),
                )
                .expect("classified cancellation notification must carry a valid target ID")
                .into_owned(),
            ),
            ProtocolMessageShape::Ordinary | ProtocolMessageShape::ShutdownRequest => None,
        };
        if session.is_terminated() {
            return Box::pin(async { Ok(None) });
        }
        if let Some(id) = cancellation_request_id {
            session.inner.admission_cancellations.cancel(&id);
        }
        let cancellation = request
            .id()
            .cloned()
            .and_then(|id| session.inner.admission_cancellations.register(id));
        let admission = session.inner.input_order.admit(method_class);
        Self::route(inner, session, request, admission, cancellation)
    }

    pub(crate) fn call_classified(
        &self,
        request: ClassifiedRequest,
    ) -> BoxFuture<'static, Result<Option<Response>, ExitedError>> {
        Self::call_request(Arc::clone(&self.inner), self.session.clone(), request)
    }

    #[cfg(feature = "stdio")]
    pub(crate) fn call_deferred_control(
        &self,
        request: ClassifiedRequest,
    ) -> BoxFuture<'static, Result<Option<Response>, ExitedError>> {
        debug_assert!(request.shape().is_exact_control_notification());
        let inner = Arc::clone(&self.inner);
        let session = self.session.clone();
        Box::pin(async move { Self::call_request(inner, session, request).await })
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
        self.call_classified(ClassifiedRequest::new(request))
    }
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
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.cancelled.load(Ordering::Acquire) {
                return;
            }
            changed.as_mut().await;
        }
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
    fn admit(self: &Arc<Self>, class: MethodClass) -> Admission {
        match class {
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
            tokio::pin!(changed);
            changed.as_mut().enable();
            if lock_recovering_poison(&self.state).completed_mutation >= target {
                return;
            }
            changed.as_mut().await;
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

impl MethodClass {
    fn classify(request: &Request, shape: ProtocolMessageShape) -> Self {
        if shape.is_exact_control_notification() {
            return Self::Control;
        }

        match request.method() {
            "initialize" | "shutdown" => Self::Mutation,
            _ if request.id().is_none() => Self::Mutation,
            _ => Self::Read,
        }
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

    fn id_bearing_cancel_request(request_id: i64, target_id: i64) -> Request {
        Request::build("$/cancelRequest")
            .params(serde_json::json!({ "id": target_id }))
            .id(request_id)
            .finish()
    }

    fn method_class(request: &Request) -> MethodClass {
        let shape = ProtocolMessageShape::classify(request);
        MethodClass::classify(request, shape)
    }

    #[test]
    fn protocol_methods_have_explicit_ordering_classes() {
        assert_eq!(method_class(&cancel_request(1)), MethodClass::Control);
        assert_eq!(
            method_class(
                &Request::build("$/cancelRequest")
                    .params(serde_json::json!({
                        "id": "request-id",
                        "padding": "ignored without cloning",
                    }))
                    .finish()
            ),
            MethodClass::Control
        );
        assert_eq!(
            method_class(
                &Request::build("$/cancelRequest")
                    .params(serde_json::json!({ "id": i64::from(i32::MAX) + 1 }))
                    .finish()
            ),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class(&Request::build("exit").finish()),
            MethodClass::Control
        );
        assert_eq!(
            method_class(&id_bearing_cancel_request(9, 1)),
            MethodClass::Read
        );
        assert_eq!(
            method_class(
                &Request::build("$/cancelRequest")
                    .params(serde_json::json!({ "unexpected": true }))
                    .finish()
            ),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class(
                &Request::build("exit")
                    .params(serde_json::json!({ "unexpected": true }))
                    .finish()
            ),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class(&Request::build("initialize").id(1).finish()),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class(&Request::build("shutdown").id(2).finish()),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class(&Request::build("textDocument/didChange").finish()),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class(&Request::build("future/notification").finish()),
            MethodClass::Mutation
        );
        assert_eq!(
            method_class(&Request::build("textDocument/completion").id(3).finish()),
            MethodClass::Read
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropped_mutations_do_not_leave_admission_gaps() {
        let order = Arc::new(InputOrder::default());
        let first = order.admit(MethodClass::Mutation);
        let second = order.admit(MethodClass::Mutation);
        let read = order.admit(MethodClass::Read);

        drop(second);
        drop(first);

        let Admission::Read { order, target } = read else {
            panic!("completion should use a read admission");
        };
        order.wait_until_completed(target).await;
        assert_eq!(target, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unpolled_cancellation_releases_a_waiting_read() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let uri = "file:///tmp/cancel-before-first-poll.mmd";

        let open = service.call(did_open_request(uri));
        let hover = service.call(hover_request(uri, 2));

        drop(service.call(cancel_request(2)));
        let response = hover.await.unwrap().expect("cancelled hover response");
        assert_eq!(
            response.error().expect("request cancellation error").code,
            tower_lsp_server::jsonrpc::ErrorCode::RequestCancelled
        );

        assert!(open.await.unwrap().is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executed_cancellation_aborts_a_tower_pending_request() {
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
    async fn id_bearing_cancel_does_not_cancel_a_request_before_first_poll() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let uri = "file:///tmp/id-bearing-cancel-before-first-poll.mmd";

        let open = service.call(did_open_request(uri));
        let hover = service.call(hover_request(uri, 2));
        let rejection = service.call(id_bearing_cancel_request(99, 2));
        tokio::pin!(rejection);

        assert!(
            futures::poll!(&mut rejection).is_pending(),
            "pseudo-control requests must follow ordinary read ordering"
        );
        assert!(open.await.unwrap().is_none());

        let rejection = rejection
            .await
            .unwrap()
            .expect("ID-bearing cancel rejection");
        assert_eq!(
            rejection.error().expect("invalid request error").code,
            tower_lsp_server::jsonrpc::ErrorCode::InvalidRequest
        );

        let response = hover.await.unwrap().expect("hover response");
        assert!(response.is_ok(), "pseudo-cancel must not cancel the hover");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn id_bearing_cancel_does_not_cancel_a_tower_pending_request() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let uri = "file:///tmp/id-bearing-cancel-after-handoff.mmd";
        assert!(service.call(did_open_request(uri)).await.unwrap().is_none());

        let state = Arc::clone(&service.session.inner.state);
        let state = state.lock().await;
        let completion = service.call(completion_request(uri, 2));
        tokio::pin!(completion);
        assert!(futures::poll!(&mut completion).is_pending());

        let rejection = service
            .call(id_bearing_cancel_request(99, 2))
            .await
            .unwrap()
            .expect("ID-bearing cancel rejection");
        assert_eq!(
            rejection.error().expect("invalid request error").code,
            tower_lsp_server::jsonrpc::ErrorCode::InvalidRequest
        );

        drop(state);
        let response = completion.await.unwrap().expect("completion response");
        assert!(
            response.is_ok(),
            "pseudo-cancel must not cancel the Tower pending request"
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
    async fn exit_notification_uses_the_session_owned_protocol_exit_once() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let session = service.session.clone();
        let inner = Arc::clone(&service.inner);

        assert!(
            service
                .call(Request::build("exit").finish())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(session.termination_count(), 1);
        assert!(!session.terminate());
        assert_eq!(session.termination_count(), 1);

        let request = lock_recovering_poison(&inner)
            .call(Request::build("textDocument/hover").id(2).finish());
        assert!(request.await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn exit_notification_with_params_does_not_terminate_the_session() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let session = service.session.clone();

        assert!(
            service
                .call(
                    Request::build("exit")
                        .params(serde_json::json!({ "unexpected": true }))
                        .finish()
                )
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(session.termination_count(), 0);
        assert!(!session.is_terminated());

        let shutdown = service
            .call(Request::build("shutdown").id(3).finish())
            .await
            .unwrap()
            .expect("shutdown response after malformed exit notification");
        assert!(shutdown.error().is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn exit_request_is_rejected_without_terminating_the_session() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let session = service.session.clone();

        let response = service
            .call(Request::build("exit").id(2).finish())
            .await
            .unwrap()
            .expect("exit request rejection");
        assert_eq!(
            response.error().expect("invalid request error").code,
            tower_lsp_server::jsonrpc::ErrorCode::InvalidRequest
        );
        assert_eq!(session.termination_count(), 0);
        assert!(!session.is_terminated());

        let shutdown = service
            .call(Request::build("shutdown").id(3).finish())
            .await
            .unwrap()
            .expect("shutdown response after rejected exit request");
        assert!(shutdown.error().is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shutdown_notification_is_ignored_without_changing_protocol_state() {
        let (mut service, _socket) = MermanLanguageServer::service();
        initialize_service(&mut service).await;
        let session = service.session.clone();

        assert!(
            service
                .call(Request::build("shutdown").finish())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(session.termination_count(), 0);
        assert!(!session.is_terminated());

        let shutdown = service
            .call(Request::build("shutdown").id(2).finish())
            .await
            .unwrap()
            .expect("shutdown response after ignored notification");
        assert!(shutdown.error().is_none());
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
}
