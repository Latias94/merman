use futures::Sink;
use futures::future::BoxFuture;
use futures::task::AtomicWaker;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::Notify;
use tower::Service;
use tower_lsp::jsonrpc::{Error, Request, Response};
use tower_lsp::{Loopback, Server};

/// Maximum number of client requests the stdio transport may process concurrently.
///
/// Keep workspace-wide requests from monopolizing the handler loop while document epochs and
/// snapshot generations guard response freshness.
pub const LSP_HANDLER_CONCURRENCY: usize = 4;

const RUNNING: u8 = 0;
const SHUTDOWN_COMPLETED: u8 = 1;
const EXIT_WITHOUT_SHUTDOWN: u8 = 2;
const EXIT_AFTER_SHUTDOWN: u8 = 3;

/// Describes why a stdio language-server session stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioTermination {
    /// The client closed the input stream without sending `exit`.
    InputClosed,
    /// The client sent `exit` after a successful `shutdown` response.
    ExitAfterShutdown,
    /// The client sent `exit` without a preceding `shutdown` request.
    ExitWithoutShutdown,
}

#[derive(Debug, Default)]
struct LifecycleState {
    state: AtomicU8,
    input_waker: AtomicWaker,
    exit_signal: Notify,
}

impl LifecycleState {
    fn observe_shutdown(&self) {
        let _ = self.state.compare_exchange(
            RUNNING,
            SHUTDOWN_COMPLETED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    fn observe_exit(&self) {
        let next = if self.state.load(Ordering::Acquire) == SHUTDOWN_COMPLETED {
            EXIT_AFTER_SHUTDOWN
        } else {
            EXIT_WITHOUT_SHUTDOWN
        };
        self.state.store(next, Ordering::Release);
        self.input_waker.wake();
        self.exit_signal.notify_waiters();
    }

    fn has_exited(&self) -> bool {
        matches!(
            self.state.load(Ordering::Acquire),
            EXIT_WITHOUT_SHUTDOWN | EXIT_AFTER_SHUTDOWN
        )
    }

    fn termination(&self) -> StdioTermination {
        match self.state.load(Ordering::Acquire) {
            EXIT_AFTER_SHUTDOWN => StdioTermination::ExitAfterShutdown,
            EXIT_WITHOUT_SHUTDOWN => StdioTermination::ExitWithoutShutdown,
            _ => StdioTermination::InputClosed,
        }
    }

    async fn exited(&self) {
        loop {
            let notified = self.exit_signal.notified();
            if self.has_exited() {
                return;
            }
            notified.await;
        }
    }
}

struct LifecycleReader<I> {
    inner: I,
    lifecycle: Arc<LifecycleState>,
}

impl<I> AsyncRead for LifecycleReader<I>
where
    I: AsyncRead + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if this.lifecycle.has_exited() {
            return Poll::Ready(Ok(()));
        }

        this.lifecycle.input_waker.register(cx.waker());
        if this.lifecycle.has_exited() {
            return Poll::Ready(Ok(()));
        }

        Pin::new(&mut this.inner).poll_read(cx, buffer)
    }
}

struct LifecycleService<S> {
    inner: S,
    lifecycle: Arc<LifecycleState>,
}

impl<S> Service<Request> for LifecycleService<S>
where
    S: Service<Request, Response = Option<Response>>,
    S::Future: Send + 'static,
{
    type Response = Option<Response>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Option<Response>, S::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        match (request.method(), request.id().cloned()) {
            ("shutdown", None) => {
                tracing::warn!("ignoring shutdown notification; shutdown must be a request");
                Box::pin(async { Ok(None) })
            }
            ("exit", Some(id)) => {
                tracing::warn!("rejecting exit request; exit must be a notification");
                Box::pin(
                    async move { Ok(Some(Response::from_error(id, Error::invalid_request()))) },
                )
            }
            ("shutdown", Some(_)) => {
                let future = self.inner.call(request);
                let lifecycle = Arc::clone(&self.lifecycle);
                Box::pin(async move {
                    let response = tokio::select! {
                        biased;
                        result = future => result?,
                        () = lifecycle.exited() => return Ok(None),
                    };
                    let successful = response
                        .as_ref()
                        .is_some_and(|response| response.error().is_none());
                    if successful {
                        lifecycle.observe_shutdown();
                    }
                    Ok(response)
                })
            }
            ("exit", None) => {
                let future = self.inner.call(request);
                self.lifecycle.observe_exit();
                Box::pin(future)
            }
            _ => {
                let future = self.inner.call(request);
                let lifecycle = Arc::clone(&self.lifecycle);
                Box::pin(async move {
                    tokio::select! {
                        biased;
                        result = future => result,
                        () = lifecycle.exited() => Ok(None),
                    }
                })
            }
        }
    }
}

/// Builds the stdio LSP transport with Merman's production concurrency policy.
pub fn stdio_server<I, O, L>(stdin: I, stdout: O, socket: L) -> Server<I, O, L>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
{
    Server::new(stdin, stdout, socket).concurrency_level(LSP_HANDLER_CONCURRENCY)
}

/// Serves an LSP session until stdin closes or the client sends `exit`.
///
/// `tower-lsp` closes its client socket after `exit`, but its transport continues waiting for
/// stdin. The lifecycle-aware reader converts the processed `exit` notification into EOF, while
/// the service wrapper cancels in-flight handlers so tower-lsp cannot wait forever while draining.
pub async fn serve_stdio<I, O, L, S>(stdin: I, stdout: O, socket: L, service: S) -> StdioTermination
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
    S: Service<Request, Response = Option<Response>> + Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
    S::Future: Send,
{
    let lifecycle = Arc::new(LifecycleState::default());
    let reader = LifecycleReader {
        inner: stdin,
        lifecycle: Arc::clone(&lifecycle),
    };
    let service = LifecycleService {
        inner: service,
        lifecycle: Arc::clone(&lifecycle),
    };

    stdio_server(reader, stdout, socket).serve(service).await;
    lifecycle.termination()
}
