use crate::session::SessionEndpointGuard;
use crate::sync::lock_recovering_poison;
use futures::channel::mpsc;
use futures::task::AtomicWaker;
use futures::{Sink, SinkExt, Stream};
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::oneshot;
use tower_lsp_server::jsonrpc::{Error, Id, Request, Response, Result as JsonRpcResult};
use tower_lsp_server::{ClientSocket, ExitedError, Loopback};

const REFRESH_REQUEST_CHANNEL_CAPACITY: usize = 4;
const REFRESH_REQUEST_ID_PREFIX: &str = "merman-refresh-";

#[derive(Debug, Clone, Copy)]
pub(crate) enum RefreshKind {
    SemanticTokens,
    Diagnostics,
}

impl RefreshKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SemanticTokens => "semantic tokens",
            Self::Diagnostics => "diagnostic",
        }
    }

    fn method(self) -> &'static str {
        match self {
            Self::SemanticTokens => "workspace/semanticTokens/refresh",
            Self::Diagnostics => "workspace/diagnostic/refresh",
        }
    }
}

type PendingRefreshMap = HashMap<Id, oneshot::Sender<JsonRpcResult<()>>>;

#[derive(Clone, Default)]
struct PendingRefreshes(Arc<Mutex<PendingRefreshMap>>);

impl PendingRefreshes {
    fn insert(&self, id: Id, response: oneshot::Sender<JsonRpcResult<()>>) {
        lock_recovering_poison(&self.0).insert(id, response);
    }

    fn remove(&self, id: &Id) -> Option<oneshot::Sender<JsonRpcResult<()>>> {
        lock_recovering_poison(&self.0).remove(id)
    }

    fn cancel_all(&self) {
        lock_recovering_poison(&self.0).clear();
    }

    fn len(&self) -> usize {
        lock_recovering_poison(&self.0).len()
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        lock_recovering_poison(&self.0).is_empty()
    }
}

#[derive(Clone)]
pub(crate) struct RefreshClient {
    outgoing: mpsc::Sender<Request>,
    pending: PendingRefreshes,
    next_request_id: Arc<AtomicU64>,
}

impl fmt::Debug for RefreshClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RefreshClient")
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

impl RefreshClient {
    pub(crate) fn channel() -> (Self, mpsc::Receiver<Request>, RefreshResponseRouter) {
        let (outgoing, requests) = mpsc::channel(REFRESH_REQUEST_CHANNEL_CAPACITY);
        let pending = PendingRefreshes::default();
        (
            Self {
                outgoing,
                pending: pending.clone(),
                next_request_id: Arc::new(AtomicU64::new(0)),
            },
            requests,
            RefreshResponseRouter { pending },
        )
    }

    pub(crate) async fn request(&self, kind: RefreshKind) -> JsonRpcResult<()> {
        let id = Id::String(format!(
            "{REFRESH_REQUEST_ID_PREFIX}{}",
            self.next_request_id.fetch_add(1, Ordering::Relaxed)
        ));
        let request = Request::build(kind.method())
            .params(serde_json::Value::Null)
            .id(id.clone())
            .finish();
        let (response, receive_response) = oneshot::channel();
        self.pending.insert(id.clone(), response);
        let _guard = PendingRefreshGuard {
            id,
            pending: self.pending.clone(),
        };

        let mut outgoing = self.outgoing.clone();
        outgoing.send(request).await.map_err(|_| {
            internal_error("refresh request transport closed before the request was sent")
        })?;
        receive_response.await.map_err(|_| {
            internal_error("refresh request transport closed before the client response arrived")
        })?
    }

    pub(crate) fn cancel_all(&self) {
        self.pending.cancel_all();
    }
}

struct PendingRefreshGuard {
    id: Id,
    pending: PendingRefreshes,
}

impl Drop for PendingRefreshGuard {
    fn drop(&mut self) {
        self.pending.remove(&self.id);
    }
}

#[derive(Clone)]
pub(crate) struct RefreshResponseRouter {
    pending: PendingRefreshes,
}

impl fmt::Debug for RefreshResponseRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RefreshResponseRouter")
            .field("pending", &self.pending.len())
            .finish()
    }
}

impl RefreshResponseRouter {
    pub(crate) fn route(&self, response: Response) -> Option<Response> {
        if !is_managed_refresh_id(response.id()) {
            return Some(response);
        }

        let (id, result) = response.into_parts();
        let waiter = self.pending.remove(&id);
        match waiter {
            Some(waiter) => {
                let _ = waiter.send(result.map(|_| ()));
            }
            None => {
                tracing::debug!(request_id = %id, "ignoring late refresh response");
            }
        }
        None
    }

    fn cancel_all(&self) {
        self.pending.cancel_all();
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

/// Error returned by the client-response half of a Merman loopback socket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MermanClientSocketError {
    /// The shared language session has already terminated.
    SessionClosed,
}

impl fmt::Display for MermanClientSocketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Merman language session is closed")
    }
}

impl std::error::Error for MermanClientSocketError {}

impl From<ExitedError> for MermanClientSocketError {
    fn from(_: ExitedError) -> Self {
        Self::SessionClosed
    }
}

struct SocketResources {
    endpoint: SessionEndpointGuard,
    refresh_requests: mpsc::Receiver<Request>,
}

struct SocketLifecycle {
    resources: Mutex<Option<SocketResources>>,
    request_waker: AtomicWaker,
    refresh_responses: RefreshResponseRouter,
}

impl fmt::Debug for SocketLifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SocketLifecycle")
            .field("closed", &self.is_closed())
            .field("refresh_responses", &self.refresh_responses)
            .finish_non_exhaustive()
    }
}

impl SocketLifecycle {
    fn new(
        endpoint: SessionEndpointGuard,
        refresh_requests: mpsc::Receiver<Request>,
        refresh_responses: RefreshResponseRouter,
    ) -> Self {
        Self {
            resources: Mutex::new(Some(SocketResources {
                endpoint,
                refresh_requests,
            })),
            request_waker: AtomicWaker::new(),
            refresh_responses,
        }
    }

    fn is_closed(&self) -> bool {
        lock_recovering_poison(&self.resources)
            .as_ref()
            .is_none_or(|resources| resources.endpoint.is_terminated())
    }

    fn ensure_open(&self) -> bool {
        if self.is_closed() {
            self.close();
            return false;
        }
        true
    }

    fn close(&self) {
        let resources = lock_recovering_poison(&self.resources).take();
        if let Some(SocketResources {
            endpoint,
            refresh_requests,
        }) = resources
        {
            drop(refresh_requests);
            self.refresh_responses.cancel_all();
            drop(endpoint);
        }

        self.request_waker.wake();
    }

    fn poll_refresh_request(&self, cx: &mut Context<'_>) -> Poll<Option<Request>> {
        let mut resources = lock_recovering_poison(&self.resources);
        let Some(resources) = resources.as_mut() else {
            return Poll::Ready(None);
        };
        Pin::new(&mut resources.refresh_requests).poll_next(cx)
    }

    fn clear_request_waker(&self) {
        drop(self.request_waker.take());
    }

    fn map_transport_result<T>(
        &self,
        result: Result<T, ExitedError>,
    ) -> Result<T, MermanClientSocketError> {
        result.map_err(|error| {
            self.close();
            error.into()
        })
    }
}

impl Drop for SocketLifecycle {
    fn drop(&mut self) {
        self.close();
    }
}

type InnerRequestStream = <ClientSocket as Loopback>::RequestStream;
type InnerResponseSink = <ClientSocket as Loopback>::ResponseSink;

/// Unsplit loopback socket for one Merman language session.
///
/// Call [`Self::split`] before driving the transport. The container deliberately does not
/// implement [`Stream`] or [`Sink`], so ownership of both directions is explicit.
pub struct MermanClientSocket {
    lifecycle: Arc<SocketLifecycle>,
    inner: ClientSocket,
}

impl fmt::Debug for MermanClientSocket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MermanClientSocket")
            .field("lifecycle", &self.lifecycle)
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl MermanClientSocket {
    pub(crate) fn new(
        inner: ClientSocket,
        refresh_requests: mpsc::Receiver<Request>,
        refresh_responses: RefreshResponseRouter,
        endpoint: SessionEndpointGuard,
    ) -> Self {
        Self {
            lifecycle: Arc::new(SocketLifecycle::new(
                endpoint,
                refresh_requests,
                refresh_responses,
            )),
            inner,
        }
    }

    /// Splits the socket into independently drivable request and response halves.
    ///
    /// Dropping either half terminates the shared session. A pending request half is woken when the
    /// response half closes. Request EOF, a successful response-sink close, or a response-sink
    /// error has the same effect.
    pub fn split(self) -> (MermanRequestStream, MermanResponseSink) {
        let Self { lifecycle, inner } = self;
        let (requests, responses) = ClientSocket::split(inner);
        (
            MermanRequestStream {
                lifecycle: Arc::clone(&lifecycle),
                inner: requests,
                pending_inner_request: None,
                prefer_refresh: true,
            },
            MermanResponseSink {
                lifecycle,
                inner: responses,
            },
        )
    }
}

/// Stream of server-to-client requests and notifications for one Merman session.
pub struct MermanRequestStream {
    lifecycle: Arc<SocketLifecycle>,
    inner: InnerRequestStream,
    pending_inner_request: Option<Request>,
    prefer_refresh: bool,
}

impl fmt::Debug for MermanRequestStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MermanRequestStream")
            .field("lifecycle", &self.lifecycle)
            .field("inner", &self.inner)
            .field(
                "has_pending_inner_request",
                &self.pending_inner_request.is_some(),
            )
            .field("prefer_refresh", &self.prefer_refresh)
            .finish()
    }
}

impl Drop for MermanRequestStream {
    fn drop(&mut self) {
        self.lifecycle.close();
    }
}

impl Stream for MermanRequestStream {
    type Item = Request;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.lifecycle.request_waker.register(cx.waker());
        if !this.lifecycle.ensure_open() {
            this.pending_inner_request = None;
            this.lifecycle.clear_request_waker();
            return Poll::Ready(None);
        }
        if this.pending_inner_request.is_none() {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Ready(Some(request)) => this.pending_inner_request = Some(request),
                Poll::Ready(None) => {
                    this.lifecycle.close();
                    return Poll::Ready(None);
                }
                Poll::Pending => {}
            }
        }

        if this.prefer_refresh
            && let Poll::Ready(Some(request)) = this.lifecycle.poll_refresh_request(cx)
        {
            this.prefer_refresh = false;
            this.lifecycle.clear_request_waker();
            return Poll::Ready(Some(request));
        }

        if let Some(request) = this.pending_inner_request.take() {
            this.prefer_refresh = true;
            this.lifecycle.clear_request_waker();
            return Poll::Ready(Some(request));
        }

        match this.lifecycle.poll_refresh_request(cx) {
            Poll::Ready(Some(request)) => {
                this.prefer_refresh = false;
                this.lifecycle.clear_request_waker();
                Poll::Ready(Some(request))
            }
            Poll::Ready(None) | Poll::Pending => Poll::Pending,
        }
    }
}

/// Sink for client responses to server-initiated requests for one Merman session.
pub struct MermanResponseSink {
    lifecycle: Arc<SocketLifecycle>,
    inner: InnerResponseSink,
}

impl fmt::Debug for MermanResponseSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MermanResponseSink")
            .field("lifecycle", &self.lifecycle)
            .field("inner", &self.inner)
            .finish()
    }
}

impl Drop for MermanResponseSink {
    fn drop(&mut self) {
        self.lifecycle.close();
    }
}

impl Sink<Response> for MermanResponseSink {
    type Error = MermanClientSocketError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        if !this.lifecycle.ensure_open() {
            return Poll::Ready(Err(MermanClientSocketError::SessionClosed));
        }
        match Pin::new(&mut this.inner).poll_ready(cx) {
            Poll::Ready(result) => Poll::Ready(this.lifecycle.map_transport_result(result)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn start_send(self: Pin<&mut Self>, response: Response) -> Result<(), Self::Error> {
        let this = self.get_mut();
        if !this.lifecycle.ensure_open() {
            return Err(MermanClientSocketError::SessionClosed);
        }
        if let Some(response) = this.lifecycle.refresh_responses.route(response) {
            let result = Pin::new(&mut this.inner).start_send(response);
            this.lifecycle.map_transport_result(result)
        } else {
            Ok(())
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        if !this.lifecycle.ensure_open() {
            return Poll::Ready(Err(MermanClientSocketError::SessionClosed));
        }
        match Pin::new(&mut this.inner).poll_flush(cx) {
            Poll::Ready(result) => Poll::Ready(this.lifecycle.map_transport_result(result)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        if !this.lifecycle.ensure_open() {
            return Poll::Ready(Err(MermanClientSocketError::SessionClosed));
        }
        match Pin::new(&mut this.inner).poll_close(cx) {
            Poll::Ready(result) => {
                this.lifecycle.close();
                Poll::Ready(result.map_err(Into::into))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Loopback for MermanClientSocket {
    type RequestStream = MermanRequestStream;
    type ResponseSink = MermanResponseSink;

    fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
        MermanClientSocket::split(self)
    }
}

fn is_managed_refresh_id(id: &Id) -> bool {
    matches!(id, Id::String(id) if id.starts_with(REFRESH_REQUEST_ID_PREFIX))
}

fn internal_error(message: &'static str) -> Error {
    let mut error = Error::internal_error();
    error.message = message.into();
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::test_support::TestService;
    use futures::task::{ArcWake, waker};
    use futures::{SinkExt, StreamExt};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::time::Duration;
    use tower::Service;
    use tower_lsp_server::ls_types::MessageType;

    #[derive(Debug, Default)]
    struct WakeCounter(AtomicUsize);

    impl ArcWake for WakeCounter {
        fn wake_by_ref(counter: &Arc<Self>) {
            counter.0.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    #[tokio::test]
    async fn cancelled_request_removes_waiter_and_ignores_late_response() {
        let (client, mut requests, responses) = RefreshClient::channel();
        let request_task =
            tokio::spawn(async move { client.request(RefreshKind::SemanticTokens).await });
        let request = requests.next().await.expect("expected refresh request");
        assert_eq!(responses.pending_count(), 1);

        request_task.abort();
        let _ = request_task.await;
        tokio::time::timeout(Duration::from_secs(1), async {
            while responses.pending_count() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled refresh waiter was not removed");

        assert!(
            responses
                .route(Response::from_ok(
                    request.id().cloned().expect("refresh request id"),
                    serde_json::Value::Null,
                ))
                .is_none()
        );
        assert_eq!(responses.pending_count(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn splitting_the_socket_does_not_terminate_the_session() {
        let TestService {
            service: _service,
            socket,
            session,
            ..
        } = crate::server::test_support::service();

        let (_requests, _responses) = socket.split();

        assert_eq!(session.termination_count(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropping_the_request_half_closes_responses_and_terminates_once() {
        let TestService {
            service: _service,
            socket,
            session,
            ..
        } = crate::server::test_support::service();
        let (requests, mut responses) = socket.split();

        drop(requests);

        assert_eq!(session.termination_count(), 1);
        assert_eq!(
            responses
                .send(Response::from_ok(Id::Number(1), serde_json::Value::Null))
                .await,
            Err(MermanClientSocketError::SessionClosed)
        );

        drop(responses);
        assert_eq!(session.termination_count(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropping_the_response_half_wakes_a_pending_request_and_terminates_once() {
        let TestService {
            service: _service,
            socket,
            session,
            ..
        } = crate::server::test_support::service();
        let (mut requests, responses) = socket.split();
        let wakes = Arc::new(WakeCounter::default());
        let request_waker = waker(Arc::clone(&wakes));
        let mut context = Context::from_waker(&request_waker);
        assert!(matches!(
            Pin::new(&mut requests).poll_next(&mut context),
            Poll::Pending
        ));

        drop(responses);

        assert!(wakes.0.load(AtomicOrdering::Relaxed) > 0);
        assert!(matches!(
            Pin::new(&mut requests).poll_next(&mut context),
            Poll::Ready(None)
        ));
        assert_eq!(session.termination_count(), 1);

        drop(requests);
        assert_eq!(session.termination_count(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_eof_closes_both_socket_halves_once() {
        let TestService {
            mut service,
            socket,
            session,
            ..
        } = crate::server::test_support::service();
        let (mut requests, mut responses) = socket.split();

        assert!(
            service
                .call(Request::build("exit").finish())
                .await
                .expect("exit notification should be handled")
                .is_none()
        );
        assert!(requests.next().await.is_none());
        assert_eq!(session.termination_count(), 1);
        assert_eq!(
            responses
                .send(Response::from_ok(Id::Number(1), serde_json::Value::Null))
                .await,
            Err(MermanClientSocketError::SessionClosed)
        );

        drop(requests);
        drop(responses);
        assert_eq!(session.termination_count(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closing_the_response_half_wakes_requests_and_terminates_once() {
        let TestService {
            service: _service,
            socket,
            session,
            ..
        } = crate::server::test_support::service();
        let (mut requests, mut responses) = socket.split();
        let wakes = Arc::new(WakeCounter::default());
        let request_waker = waker(Arc::clone(&wakes));
        let mut context = Context::from_waker(&request_waker);
        assert!(matches!(
            Pin::new(&mut requests).poll_next(&mut context),
            Poll::Pending
        ));

        responses.close().await.expect("response sink close");

        assert!(wakes.0.load(AtomicOrdering::Relaxed) > 0);
        assert!(matches!(
            Pin::new(&mut requests).poll_next(&mut context),
            Poll::Ready(None)
        ));
        assert_eq!(session.termination_count(), 1);

        drop(requests);
        drop(responses);
        assert_eq!(session.termination_count(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropping_a_socket_half_cancels_pending_refresh_waiters() {
        let TestService {
            service: _service,
            socket,
            session,
            refresh_client,
            ..
        } = crate::server::test_support::service();
        let (mut requests, responses) = socket.split();
        let request_task = tokio::spawn({
            let refresh_client = refresh_client.clone();
            async move { refresh_client.request(RefreshKind::SemanticTokens).await }
        });

        tokio::time::timeout(Duration::from_secs(1), async {
            while refresh_client.pending.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("refresh request should register its response waiter");

        drop(responses);

        assert!(
            tokio::time::timeout(Duration::from_secs(1), request_task)
                .await
                .expect("pending refresh waiter was not cancelled")
                .expect("refresh task panicked")
                .is_err()
        );
        assert!(requests.next().await.is_none());
        assert_eq!(session.termination_count(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn terminated_session_discards_a_buffered_refresh_before_socket_delivery() {
        let TestService {
            service,
            socket,
            client: retained_client,
            refresh_client,
            ..
        } = crate::server::test_support::service();
        let (mut requests, _responses) = socket.split();
        let request_task = tokio::spawn({
            let refresh_client = refresh_client.clone();
            async move { refresh_client.request(RefreshKind::SemanticTokens).await }
        });

        tokio::time::timeout(Duration::from_secs(1), async {
            while refresh_client.pending.is_empty() {
                tokio::task::yield_now().await;
            }
            tokio::task::yield_now().await;
        })
        .await
        .expect("refresh request should enter the bounded socket queue");

        drop(service);
        let mut next = Box::pin(requests.next());
        assert!(matches!(futures::poll!(&mut next), Poll::Ready(None)));
        assert!(request_task.await.unwrap().is_err());

        drop(retained_client);
    }

    #[tokio::test]
    async fn refresh_and_inner_requests_take_fair_turns() {
        let TestService {
            service: _service,
            socket,
            client,
            refresh_client,
            ..
        } = crate::server::test_support::service();
        let (mut requests, mut responses) = socket.split();
        client
            .log_message(MessageType::INFO, "ordinary client message")
            .await;

        let first_refresh_client = refresh_client.clone();
        let first_refresh = tokio::spawn(async move {
            first_refresh_client
                .request(RefreshKind::SemanticTokens)
                .await
        });
        tokio::task::yield_now().await;
        let second_refresh =
            tokio::spawn(async move { refresh_client.request(RefreshKind::Diagnostics).await });
        tokio::task::yield_now().await;

        let first = requests.next().await.expect("first socket request");
        assert_eq!(first.method(), "workspace/semanticTokens/refresh");
        let ordinary = requests.next().await.expect("ordinary socket request");
        assert_eq!(ordinary.method(), "window/logMessage");
        let second = requests.next().await.expect("second socket request");
        assert_eq!(second.method(), "workspace/diagnostic/refresh");

        for request in [&first, &second] {
            responses
                .send(Response::from_ok(
                    request.id().cloned().expect("refresh request id"),
                    serde_json::Value::Null,
                ))
                .await
                .expect("route refresh response");
        }
        first_refresh.await.unwrap().unwrap();
        second_refresh.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn exited_inner_socket_discards_queued_refresh_requests() {
        let TestService {
            mut service,
            socket,
            refresh_client,
            ..
        } = crate::server::test_support::service();
        let (mut requests, responses) = socket.split();
        let refresh =
            tokio::spawn(async move { refresh_client.request(RefreshKind::Diagnostics).await });
        tokio::task::yield_now().await;
        assert!(
            service
                .call(Request::build("exit").finish())
                .await
                .expect("exit notification should be handled")
                .is_none()
        );

        assert!(
            requests.next().await.is_none(),
            "the Tower client socket owns protocol termination"
        );
        drop(requests);
        drop(responses);
        assert!(refresh.await.unwrap().is_err());
    }
}
