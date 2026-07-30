use super::StdioTermination;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::task::{Context, Poll};
use tokio::sync::Notify;
use tower::Service;
use tower_lsp_server::jsonrpc::{Error, Request, Response};

const RUNNING: u8 = 0;
const SHUTDOWN_COMPLETED: u8 = 1;
const EXIT_WITHOUT_SHUTDOWN: u8 = 2;
const EXIT_AFTER_SHUTDOWN: u8 = 3;

#[derive(Debug, Default)]
pub(super) struct ProtocolLifecycleState {
    state: AtomicU8,
    exit_signal: Notify,
}

impl ProtocolLifecycleState {
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
        self.exit_signal.notify_waiters();
    }

    fn has_exited(&self) -> bool {
        matches!(
            self.state.load(Ordering::Acquire),
            EXIT_WITHOUT_SHUTDOWN | EXIT_AFTER_SHUTDOWN
        )
    }

    pub(super) fn termination(&self) -> StdioTermination {
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

pub(super) struct ProtocolLifecycleService<S> {
    pub(super) inner: S,
    pub(super) lifecycle: Arc<ProtocolLifecycleState>,
}

impl<S> Service<Request> for ProtocolLifecycleService<S>
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
