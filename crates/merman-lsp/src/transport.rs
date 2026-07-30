mod framing;
mod lifecycle;
#[cfg(test)]
mod tests;

use self::framing::{
    FrameRead, IncomingMessage, LspFrameReader, OutgoingMessage, Recovery, write_message,
};
use self::lifecycle::{ProtocolLifecycleService, ProtocolLifecycleState};
use crate::session::{LSP_HANDLER_CONCURRENCY, LSP_REQUEST_BYTE_BUDGET};
use futures::channel::mpsc;
use futures::future;
use futures::stream;
use futures::{FutureExt, Sink, SinkExt, StreamExt};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Semaphore, watch};
use tokio::time::Instant;
use tower::{BoxError, Service};
use tower_lsp_server::Loopback;
use tower_lsp_server::jsonrpc::{Id, Request, Response};

const MESSAGE_QUEUE_SIZE: usize = 100;
const CONTROL_HANDLER_RESERVE: usize = LSP_HANDLER_CONCURRENCY;
const MAIN_HANDLER_LIMIT: usize = MESSAGE_QUEUE_SIZE - CONTROL_HANDLER_RESERVE;
// Cancellation carries only a JSON-RPC envelope and request id. Keep enough room for string ids
// without allowing method-name spoofing to reserve an ordinary source-sized request.
const MAX_CONTROL_MESSAGE_BYTES: usize = 4 * 1024;
const CONTROL_REQUEST_BYTE_BUDGET: usize = MAX_CONTROL_MESSAGE_BYTES * LSP_HANDLER_CONCURRENCY;
const INPUT_DISPATCH_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

/// Describes why a stdio language-server session stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioTermination {
    /// The client closed the input stream without sending `exit`.
    InputClosed,
    /// The client closed the output stream before the session could end normally.
    OutputClosed,
    /// The client sent `exit` after a successful `shutdown` response.
    ExitAfterShutdown,
    /// The client sent `exit` without a preceding `shutdown` request.
    ExitWithoutShutdown,
}

async fn output_drain_deadline(
    drain_deadline: &mut watch::Receiver<Option<Instant>>,
) -> Option<Instant> {
    drain_deadline
        .wait_for(Option::is_some)
        .await
        .ok()
        .and_then(|deadline| *deadline)
}

async fn write_message_bounded<O>(
    output: &mut O,
    message: &OutgoingMessage,
    drain_deadline: &mut watch::Receiver<Option<Instant>>,
) -> Result<io::Result<()>, tokio::time::error::Elapsed>
where
    O: AsyncWrite + Unpin,
{
    let write_deadline = Instant::now() + OUTPUT_WRITE_TIMEOUT;
    let mut write = std::pin::pin!(write_message(output, message));

    let active_drain_deadline = *drain_deadline.borrow();
    if let Some(drain_deadline) = active_drain_deadline {
        return tokio::time::timeout_at(write_deadline.min(drain_deadline), &mut write).await;
    }

    tokio::select! {
        result = tokio::time::timeout_at(write_deadline, &mut write) => result,
        drain_deadline = output_drain_deadline(drain_deadline) => {
            let deadline = drain_deadline.map_or(write_deadline, |deadline| {
                write_deadline.min(deadline)
            });
            tokio::time::timeout_at(deadline, &mut write).await
        }
    }
}

async fn shutdown_output_bounded<O>(
    output: &mut O,
    drain_deadline: &watch::Receiver<Option<Instant>>,
) -> Result<io::Result<()>, tokio::time::error::Elapsed>
where
    O: AsyncWrite + Unpin,
{
    let shutdown_deadline = Instant::now() + OUTPUT_SHUTDOWN_TIMEOUT;
    let deadline = (*drain_deadline.borrow()).map_or(shutdown_deadline, |deadline| {
        shutdown_deadline.min(deadline)
    });
    tokio::time::timeout_at(deadline, output.shutdown()).await
}

fn display_sources(error: &dyn std::error::Error) -> String {
    error.source().map_or_else(
        || error.to_string(),
        |source| format!("{}: {}", error, display_sources(source)),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportStop {
    InputClosed,
    OutputClosed,
    ExitNotification,
}

/// Merman's bounded, frame-preserving stdio transport.
#[derive(Debug)]
pub struct StdioServer<I, O, L> {
    stdin: I,
    stdout: O,
    loopback: L,
    max_concurrency: usize,
    request_byte_budget: usize,
    max_control_message_bytes: usize,
    control_request_byte_budget: usize,
}

impl<I, O, L> StdioServer<I, O, L>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
{
    /// Sets the maximum number of incoming handlers polled concurrently.
    #[must_use]
    pub const fn concurrency_level(mut self, max: usize) -> Self {
        self.max_concurrency = max;
        self
    }

    #[cfg(test)]
    fn request_byte_budget(mut self, max_bytes: usize) -> Self {
        assert!(max_bytes > 0, "request byte budget must be positive");
        self.request_byte_budget = max_bytes;
        self
    }

    #[cfg(test)]
    fn control_request_byte_budget(
        mut self,
        max_message_bytes: usize,
        max_total_bytes: usize,
    ) -> Self {
        assert!(
            max_message_bytes > 0,
            "control message byte limit must be positive"
        );
        assert!(
            max_total_bytes > 0,
            "control request byte budget must be positive"
        );
        self.max_control_message_bytes = max_message_bytes;
        self.control_request_byte_budget = max_total_bytes;
        self
    }

    /// Serves messages until an input/output stream closes or a valid `exit` is dispatched.
    pub async fn serve<S>(self, service: S)
    where
        S: Service<Request, Response = Option<Response>> + Send + 'static,
        S::Error: Into<BoxError>,
        S::Future: Send,
    {
        let _ = self.serve_inner(service).await;
    }

    async fn serve_inner<S>(self, mut service: S) -> TransportStop
    where
        S: Service<Request, Response = Option<Response>> + Send + 'static,
        S::Error: Into<BoxError>,
        S::Future: Send,
    {
        let (client_requests, mut client_responses) = self.loopback.split();
        let (client_requests, client_abort) = stream::abortable(client_requests);
        let (mut responses_tx, responses_rx) = mpsc::channel::<OutgoingMessage>(0);
        let (mut server_tasks_tx, server_tasks_rx) = mpsc::channel(MESSAGE_QUEUE_SIZE);
        let mut task_responses_tx = responses_tx.clone();
        let max_concurrency = self.max_concurrency.max(1);
        let request_byte_budget = self.request_byte_budget;
        let request_budget = Arc::new(Semaphore::new(request_byte_budget));
        let max_control_message_bytes = self.max_control_message_bytes;
        let control_request_byte_budget = self.control_request_byte_budget;
        let control_request_budget = Arc::new(Semaphore::new(control_request_byte_budget));
        let handler_budget = Arc::new(Semaphore::new(MESSAGE_QUEUE_SIZE));
        let main_handler_budget = Arc::new(Semaphore::new(MAIN_HANDLER_LIMIT));
        let (output_drain_tx, mut output_drain_rx) = watch::channel(None::<Instant>);
        let mut task_drain_rx = output_drain_rx.clone();

        let (server_tasks_abort, server_tasks_registration) = future::AbortHandle::new_pair();
        let input_server_tasks_abort = server_tasks_abort.clone();
        let process_server_tasks = future::Abortable::new(
            async move {
                let mut tasks = server_tasks_rx.buffer_unordered(max_concurrency);
                let mut drain_deadline = None;
                loop {
                    let response = match drain_deadline {
                        Some(deadline) => {
                            match tokio::time::timeout_at(deadline, tasks.next()).await {
                                Ok(response) => response,
                                Err(_) => break,
                            }
                        }
                        None => {
                            tokio::select! {
                                response = tasks.next() => response,
                                deadline = output_drain_deadline(&mut task_drain_rx) => {
                                    let Some(deadline) = deadline else {
                                        break;
                                    };
                                    drain_deadline = Some(deadline);
                                    continue;
                                }
                            }
                        }
                    };
                    let Some(response) = response else {
                        break;
                    };
                    let Some(response) = response else {
                        continue;
                    };
                    if task_responses_tx
                        .send(OutgoingMessage::Response(response))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            },
            server_tasks_registration,
        );

        let output_client_abort = client_abort.clone();
        let (input_abort, input_registration) = future::AbortHandle::new_pair();
        let read_input = future::Abortable::new(
            async move {
                let mut reader = LspFrameReader::new(self.stdin);
                let stop = loop {
                    let frame = match reader.read_frame().await {
                        Ok(frame) => frame,
                        Err(error) => {
                            tracing::error!(%error, "failed to read LSP message");
                            break TransportStop::InputClosed;
                        }
                    };
                    match frame {
                        FrameRead::Eof => break TransportStop::InputClosed,
                        FrameRead::Message {
                            message: IncomingMessage::Response(response),
                            ..
                        } => {
                            // A handler may be waiting for this response, so request admission
                            // must never consume or wait for budget on the response path.
                            match tokio::time::timeout(
                                INPUT_DISPATCH_TIMEOUT,
                                client_responses.send(response),
                            )
                            .await
                            {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => {
                                    tracing::error!(
                                        error = %display_sources(&error),
                                        "failed to route LSP client response"
                                    );
                                    break TransportStop::InputClosed;
                                }
                                Err(_) => {
                                    tracing::error!(
                                        timeout_seconds = INPUT_DISPATCH_TIMEOUT.as_secs(),
                                        "timed out while routing an LSP client response"
                                    );
                                    break TransportStop::InputClosed;
                                }
                            }
                        }
                        FrameRead::Message {
                            message: IncomingMessage::Request(request),
                            body_length,
                        } => {
                            let will_exit = request.method() == "exit" && request.id().is_none();
                            let bounded_exit =
                                will_exit && body_length <= max_control_message_bytes;
                            let bounded_cancel = request.method() == "$/cancelRequest"
                                && body_length <= max_control_message_bytes;
                            let request_budget_permit = if bounded_exit {
                                // Protocol lifecycle routing must observe exit even when active
                                // handlers consume both request budgets. The byte cap prevents a
                                // forged oversized control frame from bypassing both.
                                None
                            } else {
                                let permits = u32::try_from(body_length)
                                    .expect("validated LSP body length fits in u32");
                                let (budget, budget_limit, budget_name) = if bounded_cancel {
                                    (
                                        Arc::clone(&control_request_budget),
                                        control_request_byte_budget,
                                        "control",
                                    )
                                } else {
                                    (Arc::clone(&request_budget), request_byte_budget, "request")
                                };
                                match budget.try_acquire_many_owned(permits) {
                                    Ok(permit) => Some(permit),
                                    Err(error) => {
                                        tracing::error!(
                                            %error,
                                            body_length,
                                            budget_limit,
                                            budget_name,
                                            "LSP message byte budget exhausted"
                                        );
                                        input_server_tasks_abort.abort();
                                        break TransportStop::InputClosed;
                                    }
                                }
                            };

                            let main_handler_permit = if bounded_cancel || bounded_exit {
                                None
                            } else {
                                match Arc::clone(&main_handler_budget).try_acquire_owned() {
                                    Ok(permit) => Some(permit),
                                    Err(error) => {
                                        tracing::error!(
                                            %error,
                                            handler_limit = MAIN_HANDLER_LIMIT,
                                            "LSP main handler admission exhausted"
                                        );
                                        break TransportStop::InputClosed;
                                    }
                                }
                            };
                            let handler_permit = if bounded_exit {
                                None
                            } else {
                                match Arc::clone(&handler_budget).try_acquire_owned() {
                                    Ok(permit) => Some(permit),
                                    Err(error) => {
                                        tracing::error!(
                                            %error,
                                            handler_limit = MESSAGE_QUEUE_SIZE,
                                            "LSP handler admission exhausted"
                                        );
                                        break TransportStop::InputClosed;
                                    }
                                }
                            };

                            match tokio::time::timeout(
                                INPUT_DISPATCH_TIMEOUT,
                                future::poll_fn(|cx| service.poll_ready(cx)),
                            )
                            .await
                            {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => {
                                    let error: BoxError = error.into();
                                    tracing::error!(
                                        error = %display_sources(error.as_ref()),
                                        "LSP service was not ready"
                                    );
                                    break TransportStop::InputClosed;
                                }
                                Err(_) => {
                                    tracing::error!(
                                        timeout_seconds = INPUT_DISPATCH_TIMEOUT.as_secs(),
                                        "timed out while waiting for LSP service readiness"
                                    );
                                    break TransportStop::InputClosed;
                                }
                            }

                            let request_id = request.id().cloned();
                            let handler = service.call(request);
                            if will_exit {
                                // `Service::call` performs synchronous protocol/session routing.
                                // Poll once so a generic service can observe dispatch in its
                                // future, then drop Pending work because exit has no response.
                                let _ = handler.now_or_never();
                                drop(request_budget_permit);
                                break TransportStop::ExitNotification;
                            }
                            let task = async move {
                                let _handler_permit = handler_permit;
                                let _main_handler_permit = main_handler_permit;
                                let response = match handler.await {
                                    Ok(response) => response,
                                    Err(error) => {
                                        let error: BoxError = error.into();
                                        tracing::error!(
                                            error = %display_sources(error.as_ref()),
                                            "LSP request handler failed"
                                        );
                                        None
                                    }
                                };
                                drop(request_budget_permit);
                                response
                            };
                            match server_tasks_tx.try_send(task) {
                                Ok(()) => {}
                                Err(error) => {
                                    // Handler permits are acquired before `Service::call`, so a
                                    // full queue is an internal invariant failure rather than an
                                    // overload path that may discard an admitted mutation.
                                    tracing::error!(
                                        %error,
                                        request_id = ?request_id,
                                        "LSP handler queue rejected a reserved task"
                                    );
                                    drop(error);
                                    break TransportStop::InputClosed;
                                }
                            }
                        }
                        FrameRead::Error(error, recovery) => {
                            tracing::error!(%error, "failed to decode LSP message");
                            let response = Response::from_error(Id::Null, error.jsonrpc_error());
                            match responses_tx.try_send(OutgoingMessage::Response(response)) {
                                Ok(()) => {}
                                Err(error) if error.is_full() => {
                                    // Output backpressure must not prevent the reader from
                                    // reaching a later cancellation, exit, or EOF marker.
                                    tracing::warn!(
                                        "dropping protocol error response because output is full"
                                    );
                                }
                                Err(error) => {
                                    tracing::error!(%error, "failed to queue protocol error response");
                                    break TransportStop::InputClosed;
                                }
                            }
                            if recovery == Recovery::Stop {
                                break TransportStop::InputClosed;
                            }
                        }
                    }
                };

                output_drain_tx.send_replace(Some(Instant::now() + OUTPUT_WRITE_TIMEOUT));
                server_tasks_tx.disconnect();
                responses_tx.disconnect();
                client_abort.abort();
                if stop == TransportStop::InputClosed {
                    input_server_tasks_abort.abort();
                }
                stop
            },
            input_registration,
        );

        let print_output = async move {
            let mut stdout = std::pin::pin!(self.stdout);
            let mut messages =
                stream::select(responses_rx, client_requests.map(OutgoingMessage::Request));
            let mut output_failed = false;
            while let Some(message) = messages.next().await {
                match write_message_bounded(&mut stdout, &message, &mut output_drain_rx).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::error!(%error, "failed to write LSP message");
                        output_failed = true;
                        break;
                    }
                    Err(_) => {
                        tracing::error!(
                            timeout_seconds = OUTPUT_WRITE_TIMEOUT.as_secs(),
                            "timed out while writing an LSP message"
                        );
                        output_failed = true;
                        break;
                    }
                }
            }

            if !output_failed {
                match shutdown_output_bounded(&mut stdout, &output_drain_rx).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::error!(%error, "failed to close stdio output");
                        output_failed = true;
                    }
                    Err(_) => {
                        tracing::error!("timed out while closing stdio output");
                        output_failed = true;
                    }
                }
            }
            input_abort.abort();
            server_tasks_abort.abort();
            output_client_abort.abort();
            output_failed
        };

        let (_, output_failed, stop) =
            future::join3(process_server_tasks, print_output, read_input).await;
        if output_failed {
            TransportStop::OutputClosed
        } else {
            stop.unwrap_or(TransportStop::OutputClosed)
        }
    }
}

/// Builds the stdio LSP transport with Merman's production concurrency policy.
pub fn stdio_server<I, O, L>(stdin: I, stdout: O, socket: L) -> StdioServer<I, O, L>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
{
    StdioServer {
        stdin,
        stdout,
        loopback: socket,
        max_concurrency: LSP_HANDLER_CONCURRENCY,
        request_byte_budget: LSP_REQUEST_BYTE_BUDGET,
        max_control_message_bytes: MAX_CONTROL_MESSAGE_BYTES,
        control_request_byte_budget: CONTROL_REQUEST_BYTE_BUDGET,
    }
}

/// Serves an LSP session until an input/output stream closes or the client sends `exit`.
///
/// Framing is owned here so a rejected `exit` request cannot make an upstream buffered reader
/// discard later frames from the same client write. A valid notification cancels in-flight handlers
/// through the lifecycle wrapper before the transport drains its task queue.
pub async fn serve_stdio<I, O, L, S>(stdin: I, stdout: O, socket: L, service: S) -> StdioTermination
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
    S: Service<Request, Response = Option<Response>> + Send + 'static,
    S::Error: Into<BoxError> + Send + 'static,
    S::Future: Send + 'static,
{
    let lifecycle = Arc::new(ProtocolLifecycleState::default());
    let service = ProtocolLifecycleService {
        inner: service,
        lifecycle: Arc::clone(&lifecycle),
    };
    let stop = stdio_server(stdin, stdout, socket)
        .serve_inner(service)
        .await;
    match stop {
        TransportStop::OutputClosed => StdioTermination::OutputClosed,
        TransportStop::InputClosed | TransportStop::ExitNotification => lifecycle.termination(),
    }
}
