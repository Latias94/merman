use crate::session::SessionEndpointGuard;
use crate::sync::lock_recovering_poison;
use futures::channel::mpsc;
use futures::{Sink, SinkExt, Stream, StreamExt};
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

/// Loopback socket that adds cancellation-safe server-to-client refresh requests.
pub struct MermanClientSocket {
    endpoint: SessionEndpointGuard,
    inner: ClientSocket,
    refresh_requests: mpsc::Receiver<Request>,
    refresh_responses: RefreshResponseRouter,
    pending_inner_request: Option<Request>,
    prefer_refresh: bool,
}

impl fmt::Debug for MermanClientSocket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MermanClientSocket")
            .field("inner", &self.inner)
            .field("refresh_responses", &self.refresh_responses)
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
            endpoint,
            inner,
            refresh_requests,
            refresh_responses,
            pending_inner_request: None,
            prefer_refresh: true,
        }
    }

    fn close_refresh_transport(&mut self) {
        self.endpoint.terminate();
        self.refresh_requests.close();
        while self.refresh_requests.try_recv().is_ok() {}
        self.pending_inner_request = None;
        self.refresh_responses.cancel_all();
    }
}

impl Drop for MermanClientSocket {
    fn drop(&mut self) {
        self.close_refresh_transport();
    }
}

impl Stream for MermanClientSocket {
    type Item = Request;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.endpoint.is_terminated() {
            this.close_refresh_transport();
            return Poll::Ready(None);
        }
        if this.pending_inner_request.is_none() {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Ready(Some(request)) => this.pending_inner_request = Some(request),
                Poll::Ready(None) => {
                    this.close_refresh_transport();
                    return Poll::Ready(None);
                }
                Poll::Pending => {}
            }
        }

        if this.prefer_refresh
            && let Poll::Ready(Some(request)) = Pin::new(&mut this.refresh_requests).poll_next(cx)
        {
            this.prefer_refresh = false;
            return Poll::Ready(Some(request));
        }

        if let Some(request) = this.pending_inner_request.take() {
            this.prefer_refresh = true;
            return Poll::Ready(Some(request));
        }

        match Pin::new(&mut this.refresh_requests).poll_next(cx) {
            Poll::Ready(Some(request)) => {
                this.prefer_refresh = false;
                Poll::Ready(Some(request))
            }
            Poll::Ready(None) | Poll::Pending => Poll::Pending,
        }
    }
}

impl Sink<Response> for MermanClientSocket {
    type Error = ExitedError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, response: Response) -> Result<(), Self::Error> {
        let this = self.get_mut();
        if let Some(response) = this.refresh_responses.route(response) {
            Pin::new(&mut this.inner).start_send(response)
        } else {
            Ok(())
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_close(cx)
    }
}

impl Loopback for MermanClientSocket {
    type RequestStream = futures::stream::SplitStream<Self>;
    type ResponseSink = futures::stream::SplitSink<Self, Response>;

    fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
        let (responses, requests) = StreamExt::split(self);
        (requests, responses)
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
    use crate::MermanLanguageServer;
    use futures::SinkExt;
    use std::time::Duration;
    use tower::Service;
    use tower_lsp_server::ls_types::MessageType;

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
    async fn terminated_session_discards_a_buffered_refresh_before_socket_delivery() {
        let (service, mut socket, refresh_client) =
            MermanLanguageServer::service_with_refresh_client();
        let retained_client = service.inner().client_for_tests();
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
        let mut next = Box::pin(socket.next());
        assert!(matches!(futures::poll!(&mut next), Poll::Ready(None)));
        assert!(request_task.await.unwrap().is_err());

        drop(retained_client);
    }

    #[tokio::test]
    async fn refresh_and_inner_requests_take_fair_turns() {
        let (service, mut socket, refresh_client) =
            MermanLanguageServer::service_with_refresh_client();
        let client = service.inner().client_for_tests();
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

        let first = socket.next().await.expect("first socket request");
        assert_eq!(first.method(), "workspace/semanticTokens/refresh");
        let ordinary = socket.next().await.expect("ordinary socket request");
        assert_eq!(ordinary.method(), "window/logMessage");
        let second = socket.next().await.expect("second socket request");
        assert_eq!(second.method(), "workspace/diagnostic/refresh");

        for request in [&first, &second] {
            socket
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
        let (mut service, mut socket, refresh_client) =
            MermanLanguageServer::service_with_refresh_client();
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
            socket.next().await.is_none(),
            "the Tower client socket owns protocol termination"
        );
        drop(socket);
        assert!(refresh.await.unwrap().is_err());
    }
}
