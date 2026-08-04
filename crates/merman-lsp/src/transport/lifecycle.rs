use super::{StdioAdmissionService, StdioTermination};
use crate::session::{ClassifiedRequest, ProtocolMessageShape};
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Notify;
use tower_lsp_server::jsonrpc::{Error, Response};

const RUNNING: u8 = 0;
const SHUTDOWN_COMPLETED: u8 = 1;
const EXIT_RECEIVED_WITHOUT_SHUTDOWN: u8 = 2;
const EXIT_RECEIVED_AFTER_SHUTDOWN: u8 = 3;
const EXIT_OBSERVED_WITHOUT_SHUTDOWN: u8 = 4;
const EXIT_OBSERVED_AFTER_SHUTDOWN: u8 = 5;

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

    fn record_exit_receipt(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let next = match current {
                RUNNING => EXIT_RECEIVED_WITHOUT_SHUTDOWN,
                SHUTDOWN_COMPLETED => EXIT_RECEIVED_AFTER_SHUTDOWN,
                EXIT_RECEIVED_WITHOUT_SHUTDOWN
                | EXIT_RECEIVED_AFTER_SHUTDOWN
                | EXIT_OBSERVED_WITHOUT_SHUTDOWN
                | EXIT_OBSERVED_AFTER_SHUTDOWN => return,
                _ => unreachable!("invalid protocol lifecycle state"),
            };
            match self.state.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }

    fn observe_exit(&self) {
        self.record_exit_receipt();
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let next = match current {
                EXIT_RECEIVED_WITHOUT_SHUTDOWN => EXIT_OBSERVED_WITHOUT_SHUTDOWN,
                EXIT_RECEIVED_AFTER_SHUTDOWN => EXIT_OBSERVED_AFTER_SHUTDOWN,
                EXIT_OBSERVED_WITHOUT_SHUTDOWN | EXIT_OBSERVED_AFTER_SHUTDOWN => return,
                RUNNING | SHUTDOWN_COMPLETED => {
                    self.record_exit_receipt();
                    current = self.state.load(Ordering::Acquire);
                    continue;
                }
                _ => unreachable!("invalid protocol lifecycle state"),
            };
            match self.state.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.exit_signal.notify_waiters();
                    return;
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn has_exited(&self) -> bool {
        matches!(
            self.state.load(Ordering::Acquire),
            EXIT_OBSERVED_WITHOUT_SHUTDOWN | EXIT_OBSERVED_AFTER_SHUTDOWN
        )
    }

    pub(super) fn termination(&self) -> StdioTermination {
        match self.state.load(Ordering::Acquire) {
            EXIT_RECEIVED_AFTER_SHUTDOWN | EXIT_OBSERVED_AFTER_SHUTDOWN => {
                StdioTermination::ExitAfterShutdown
            }
            EXIT_RECEIVED_WITHOUT_SHUTDOWN | EXIT_OBSERVED_WITHOUT_SHUTDOWN => {
                StdioTermination::ExitWithoutShutdown
            }
            _ => StdioTermination::InputClosed,
        }
    }

    async fn exited(&self) {
        loop {
            let notified = self.exit_signal.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.has_exited() {
                return;
            }
            notified.as_mut().await;
        }
    }
}

pub(super) struct ProtocolLifecycleService<S> {
    pub(super) inner: S,
    pub(super) lifecycle: Arc<ProtocolLifecycleState>,
}

impl<S> ProtocolLifecycleService<S>
where
    S: StdioAdmissionService,
    S::Future: Send + 'static,
{
    fn admit_deferred_with_lifecycle(
        &mut self,
        request: ClassifiedRequest,
    ) -> BoxFuture<'static, Result<Option<Response>, S::Error>> {
        match request.shape() {
            ProtocolMessageShape::ShutdownNotification => {
                tracing::warn!("ignoring shutdown notification; shutdown must be a request");
                Box::pin(async { Ok(None) })
            }
            ProtocolMessageShape::ExitRequest => {
                let (_, id, _) = request.into_request().into_parts();
                let id = id.expect("classified exit request must carry an ID");
                tracing::warn!("rejecting exit request; exit must be a notification");
                Box::pin(
                    async move { Ok(Some(Response::from_error(id, Error::invalid_request()))) },
                )
            }
            ProtocolMessageShape::InvalidExitNotification => {
                tracing::warn!("ignoring exit notification with unexpected parameters");
                Box::pin(async { Ok(None) })
            }
            ProtocolMessageShape::ShutdownRequest => {
                let future = self.inner.admit_deferred(request);
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
            ProtocolMessageShape::ExitNotification => {
                let future = self.inner.admit_deferred(request);
                let lifecycle = Arc::clone(&self.lifecycle);
                Box::pin(async move {
                    lifecycle.observe_exit();
                    future.await
                })
            }
            ProtocolMessageShape::CancellationNotification | ProtocolMessageShape::Ordinary => {
                let future = self.inner.admit_deferred(request);
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

impl<S> StdioAdmissionService for ProtocolLifecycleService<S>
where
    S: StdioAdmissionService,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Option<Response>, S::Error>>;

    fn admit_immediate_control(&mut self, request: ClassifiedRequest) -> Self::Future {
        debug_assert!(request.shape().is_exact_control_notification());
        let exits = request.shape().is_exit_notification();
        let future = self.inner.admit_immediate_control(request);
        if exits {
            self.lifecycle.record_exit_receipt();
            self.lifecycle.observe_exit();
        }
        Box::pin(future)
    }

    fn admit_deferred(&mut self, request: ClassifiedRequest) -> Self::Future {
        self.admit_deferred_with_lifecycle(request)
    }

    fn deferred_published(&mut self, shape: ProtocolMessageShape) {
        if shape.is_exit_notification() {
            self.lifecycle.record_exit_receipt();
        }
        self.inner.deferred_published(shape);
    }
}
