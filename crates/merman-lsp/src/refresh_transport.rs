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
use tower_lsp::jsonrpc::{Error, Id, Request, Response, Result as JsonRpcResult};
use tower_lsp::{ClientSocket, ExitedError, Loopback};

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

type PendingRefreshes = Arc<Mutex<HashMap<Id, oneshot::Sender<JsonRpcResult<()>>>>>;

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
            .field("pending", &lock_recovering_poison(&self.pending).len())
            .finish_non_exhaustive()
    }
}

impl RefreshClient {
    pub(crate) fn channel() -> (Self, mpsc::Receiver<Request>, RefreshResponseRouter) {
        let (outgoing, requests) = mpsc::channel(REFRESH_REQUEST_CHANNEL_CAPACITY);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        (
            Self {
                outgoing,
                pending: Arc::clone(&pending),
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
        lock_recovering_poison(&self.pending).insert(id.clone(), response);
        let _guard = PendingRefreshGuard {
            id,
            pending: Arc::clone(&self.pending),
        };

        let mut outgoing = self.outgoing.clone();
        outgoing.send(request).await.map_err(|_| {
            internal_error("refresh request transport closed before the request was sent")
        })?;
        receive_response.await.map_err(|_| {
            internal_error("refresh request transport closed before the client response arrived")
        })?
    }
}

struct PendingRefreshGuard {
    id: Id,
    pending: PendingRefreshes,
}

impl Drop for PendingRefreshGuard {
    fn drop(&mut self) {
        lock_recovering_poison(&self.pending).remove(&self.id);
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
            .field("pending", &lock_recovering_poison(&self.pending).len())
            .finish()
    }
}

impl RefreshResponseRouter {
    pub(crate) fn route(&self, response: Response) -> Option<Response> {
        if !is_managed_refresh_id(response.id()) {
            return Some(response);
        }

        let (id, result) = response.into_parts();
        let waiter = lock_recovering_poison(&self.pending).remove(&id);
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

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        lock_recovering_poison(&self.pending).len()
    }
}

/// Loopback socket that adds cancellation-safe server-to-client refresh requests.
pub struct MermanClientSocket {
    inner: ClientSocket,
    refresh_requests: mpsc::Receiver<Request>,
    refresh_responses: RefreshResponseRouter,
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
    ) -> Self {
        Self {
            inner,
            refresh_requests,
            refresh_responses,
        }
    }
}

impl Stream for MermanClientSocket {
    type Item = Request;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(request)) => Poll::Ready(Some(request)),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => match Pin::new(&mut this.refresh_requests).poll_next(cx) {
                Poll::Ready(Some(request)) => Poll::Ready(Some(request)),
                Poll::Ready(None) | Poll::Pending => Poll::Pending,
            },
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
    use std::time::Duration;

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
}
