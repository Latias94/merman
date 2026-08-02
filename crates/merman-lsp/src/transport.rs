mod framing;
mod lifecycle;
#[cfg(test)]
mod tests;

use self::framing::{
    FrameRead, IncomingMessage, LspFrameReader, OutgoingMessage, Recovery, write_message,
};
use self::lifecycle::{ProtocolLifecycleService, ProtocolLifecycleState};
use crate::refresh_coordinator::MAX_CONCURRENT_REFRESH_REQUESTS;
use crate::session::{
    LSP_ORDINARY_HANDLER_CONCURRENCY, LSP_REQUEST_BYTE_BUDGET, MermanLspService,
    ProtocolMessageShape, exact_cancellation_request_id,
};
use futures::channel::mpsc;
use futures::future;
use futures::stream;
use futures::{Sink, SinkExt, StreamExt};
use std::borrow::Cow;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot, watch};
use tokio::time::Instant;
use tower::{BoxError, Service};
use tower_lsp_server::Loopback;
use tower_lsp_server::jsonrpc::{Error, ErrorCode, Id, Request, Response};

// futures::mpsc reserves one slot for its sole sender in addition to this shared buffer. One
// response may therefore be held by the sink while every response-bearing refresh lane still has
// a bounded place to wait. Adding another concurrent client-request lane must update the owner
// constant rather than silently turning a valid response burst into transport failure.
const CLIENT_RESPONSE_QUEUE_SIZE: usize = MAX_CONCURRENT_REFRESH_REQUESTS - 1;
const ORDINARY_HANDLER_CAPACITY: usize = 96;
const MAX_CONTROL_MESSAGE_BYTES: usize = 4 * 1024;
const OVERLOAD_RESPONSE_CAPACITY: usize = 4;
const OVERLOAD_RESPONSE_BYTE_BUDGET: usize = 64 * 1024;
const OVERLOAD_ERROR_CODE: i64 = -32099;
const OVERLOAD_ERROR_MESSAGE: &str = "Server overloaded";
const CLIENT_RESPONSE_DISPATCH_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

type HandlerTask = future::BoxFuture<'static, Option<Response>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdmissionClass {
    ImmediateControl,
    Deferred,
}

enum ServiceAdmission<F> {
    Immediate(F),
    Exit(F),
    Deferred(F),
}

fn admission_class(
    request: &Request,
    body_length: usize,
    max_control_message_bytes: usize,
) -> AdmissionClass {
    if body_length <= max_control_message_bytes
        && (exact_cancellation_request_id(request).is_some()
            || matches!(
                ProtocolMessageShape::classify(request),
                ProtocolMessageShape::ExitNotification
            ))
    {
        AdmissionClass::ImmediateControl
    } else {
        AdmissionClass::Deferred
    }
}

trait StdioAdmissionService {
    type Error;
    type Future: std::future::Future<Output = Result<Option<Response>, Self::Error>>;

    fn admit(&mut self, request: Request, class: AdmissionClass) -> ServiceAdmission<Self::Future>;
}

impl StdioAdmissionService for MermanLspService {
    type Error = <Self as Service<Request>>::Error;
    type Future = <Self as Service<Request>>::Future;

    fn admit(&mut self, request: Request, class: AdmissionClass) -> ServiceAdmission<Self::Future> {
        let exits = matches!(
            ProtocolMessageShape::classify(&request),
            ProtocolMessageShape::ExitNotification
        );
        let future = Service::call(self, request);
        if exits {
            ServiceAdmission::Exit(future)
        } else if class == AdmissionClass::ImmediateControl {
            ServiceAdmission::Immediate(future)
        } else {
            ServiceAdmission::Deferred(future)
        }
    }
}

async fn complete_immediate_control<F, E>(future: F) -> bool
where
    F: std::future::Future<Output = Result<Option<Response>, E>>,
    E: Into<BoxError>,
{
    match future.await {
        Ok(None) => true,
        Ok(Some(response)) => {
            tracing::error!(
                response_id = ?response.id(),
                "response-less immediate LSP control produced a response"
            );
            false
        }
        Err(error) => {
            let error: BoxError = error.into();
            tracing::error!(
                error = %display_sources(error.as_ref()),
                "immediate LSP control failed"
            );
            false
        }
    }
}

/// Describes why a stdio language-server session stopped.
///
/// `OutputClosed` is dominant when output failure races another stop reason because required
/// responses can no longer be delivered.
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
    /// The transport could not retain an ordinary message without losing input integrity.
    ///
    /// This includes an overloaded notification, or a request whose complete overload response
    /// cannot enter the bounded output lane.
    InputOverloaded,
}

struct OverloadResponseBudget {
    _count: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}

struct QueuedOutput {
    message: OutgoingMessage,
    _overload_budget: Option<OverloadResponseBudget>,
}

impl QueuedOutput {
    fn normal(message: OutgoingMessage) -> Self {
        Self {
            message,
            _overload_budget: None,
        }
    }

    fn overload(message: OutgoingMessage, budget: OverloadResponseBudget) -> Self {
        Self {
            message,
            _overload_budget: Some(budget),
        }
    }
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

async fn process_handler_tasks(
    tasks: mpsc::UnboundedReceiver<HandlerTask>,
    concurrency: usize,
    mut responses: mpsc::Sender<QueuedOutput>,
    mut drain_deadline: watch::Receiver<Option<Instant>>,
) {
    let mut tasks = tasks.buffer_unordered(concurrency.max(1));
    let mut deadline = None;
    loop {
        let response = match deadline {
            Some(deadline) => match tokio::time::timeout_at(deadline, tasks.next()).await {
                Ok(response) => response,
                Err(_) => break,
            },
            None => {
                tokio::select! {
                    response = tasks.next() => response,
                    observed_deadline = output_drain_deadline(&mut drain_deadline) => {
                        let Some(next_deadline) = observed_deadline else {
                            break;
                        };
                        deadline = Some(next_deadline);
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
        if responses
            .send(QueuedOutput::normal(OutgoingMessage::Response(response)))
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn route_client_responses<S>(
    mut responses: mpsc::Receiver<Response>,
    mut sink: S,
    routing_failed: oneshot::Sender<()>,
    mut drain_deadline: watch::Receiver<Option<Instant>>,
) where
    S: Sink<Response> + Unpin,
    S::Error: std::error::Error,
{
    while let Some(response) = responses.next().await {
        let dispatch_deadline = Instant::now() + CLIENT_RESPONSE_DISPATCH_TIMEOUT;
        let mut send = std::pin::pin!(sink.send(response));
        let active_drain_deadline = *drain_deadline.borrow();
        let result = if let Some(active_drain_deadline) = active_drain_deadline {
            tokio::time::timeout_at(dispatch_deadline.min(active_drain_deadline), &mut send).await
        } else {
            tokio::select! {
                result = tokio::time::timeout_at(dispatch_deadline, &mut send) => result,
                observed_deadline = output_drain_deadline(&mut drain_deadline) => {
                    let deadline = observed_deadline.map_or(dispatch_deadline, |drain_deadline| {
                        dispatch_deadline.min(drain_deadline)
                    });
                    tokio::time::timeout_at(deadline, &mut send).await
                }
            }
        };
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::error!(
                    error = %display_sources(&error),
                    "failed to route LSP client response"
                );
                let _ = routing_failed.send(());
                return;
            }
            Err(_) => {
                tracing::error!(
                    timeout_seconds = CLIENT_RESPONSE_DISPATCH_TIMEOUT.as_secs(),
                    "timed out while routing an LSP client response"
                );
                let _ = routing_failed.send(());
                return;
            }
        }
    }
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

fn framed_message_len(message: &OutgoingMessage) -> usize {
    let body_len = serde_json::to_vec(message)
        .expect("JSON-RPC responses always serialize")
        .len();
    let digits = body_len
        .checked_ilog10()
        .map_or(1, |value| value as usize + 1);
    "Content-Length: ".len() + digits + "\r\n\r\n".len() + body_len
}

fn queue_overload_response(
    request_id: Id,
    count_budget: &Arc<Semaphore>,
    byte_budget: &Arc<Semaphore>,
    byte_budget_limit: usize,
    responses: &mpsc::UnboundedSender<QueuedOutput>,
) -> Result<(), ()> {
    let error = Error {
        code: ErrorCode::ServerError(OVERLOAD_ERROR_CODE),
        message: Cow::Borrowed(OVERLOAD_ERROR_MESSAGE),
        data: None,
    };
    let message = OutgoingMessage::Response(Response::from_error(request_id, error));
    let retained_bytes = framed_message_len(&message);
    if retained_bytes > byte_budget_limit {
        return Err(());
    }
    let count = Arc::clone(count_budget)
        .try_acquire_owned()
        .map_err(|_| ())?;
    let bytes = Arc::clone(byte_budget)
        .try_acquire_many_owned(
            u32::try_from(retained_bytes).expect("overload response budget fits in u32"),
        )
        .map_err(|_| ())?;
    responses
        .unbounded_send(QueuedOutput::overload(
            message,
            OverloadResponseBudget {
                _count: count,
                _bytes: bytes,
            },
        ))
        .map_err(|_| ())
}

fn reject_overloaded_message(
    request_id: Option<Id>,
    count_budget: &Arc<Semaphore>,
    byte_budget: &Arc<Semaphore>,
    byte_budget_limit: usize,
    responses: &mpsc::UnboundedSender<QueuedOutput>,
) -> Result<(), TransportStop> {
    let Some(request_id) = request_id else {
        return Err(TransportStop::InputOverloaded);
    };
    queue_overload_response(
        request_id,
        count_budget,
        byte_budget,
        byte_budget_limit,
        responses,
    )
    .map_err(|()| TransportStop::InputOverloaded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportStop {
    InputClosed,
    InputOverloaded,
    OutputClosed,
    ExitNotification,
}

/// Merman's bounded, frame-preserving stdio transport.
///
/// The transport privately executes exact, valid, ID-less cancel and exit notifications inline
/// when their encoded frame is small enough. Ordinary messages share one retained admission
/// budget and a separate consumer concurrency limit. Request overload is reported with a bounded
/// JSON-RPC error when possible; notification overload terminates input integrity.
#[derive(Debug)]
pub struct StdioServer<I, O, L> {
    stdin: I,
    stdout: O,
    loopback: L,
    ordinary_concurrency: usize,
    retained_deferred_capacity: usize,
    request_byte_budget: usize,
    max_control_message_bytes: usize,
    overload_response_capacity: usize,
    overload_response_byte_budget: usize,
}

impl<I, O, L> StdioServer<I, O, L>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
{
    /// Sets the maximum number of ordinary data-plane handlers polled concurrently.
    #[must_use]
    pub const fn ordinary_concurrency_level(mut self, max: usize) -> Self {
        self.ordinary_concurrency = max;
        self
    }

    #[cfg(test)]
    fn request_byte_budget(mut self, max_bytes: usize) -> Self {
        assert!(max_bytes > 0, "request byte budget must be positive");
        self.request_byte_budget = max_bytes;
        self
    }

    #[cfg(test)]
    fn retained_deferred_capacity(mut self, capacity: usize) -> Self {
        assert!(capacity > 0, "retained deferred capacity must be positive");
        self.retained_deferred_capacity = capacity;
        self
    }

    #[cfg(test)]
    fn control_message_byte_limit(mut self, max_message_bytes: usize) -> Self {
        assert!(
            max_message_bytes > 0,
            "control message byte limit must be positive"
        );
        self.max_control_message_bytes = max_message_bytes;
        self
    }

    #[cfg(test)]
    fn overload_response_budget(mut self, max_responses: usize, max_total_bytes: usize) -> Self {
        assert!(
            max_responses > 0,
            "overload response capacity must be positive"
        );
        assert!(
            max_total_bytes > 0,
            "overload response byte budget must be positive"
        );
        self.overload_response_capacity = max_responses;
        self.overload_response_byte_budget = max_total_bytes;
        self
    }

    /// Serves messages until the session terminates, discarding the detailed stop reason.
    ///
    /// Use [`serve_stdio`] when the host needs to distinguish clean input close, protocol exit,
    /// overload, and output failure.
    pub async fn serve(self, service: MermanLspService) {
        let _ = self.serve_inner(service).await;
    }

    async fn serve_inner<S>(self, mut service: S) -> TransportStop
    where
        S: StdioAdmissionService + Send + 'static,
        S::Error: Into<BoxError>,
        S::Future: Send + 'static,
    {
        let (client_requests, client_responses) = self.loopback.split();
        let (client_requests, client_abort) = stream::abortable(client_requests);
        let (mut client_response_tasks_tx, client_response_tasks_rx) =
            mpsc::channel::<Response>(CLIENT_RESPONSE_QUEUE_SIZE);
        let (client_response_routing_failed_tx, mut client_response_routing_failed_rx) =
            oneshot::channel();
        let (mut responses_tx, responses_rx) = mpsc::channel::<QueuedOutput>(0);
        let (mut server_tasks_tx, server_tasks_rx) = mpsc::unbounded::<HandlerTask>();
        let (mut overload_responses_tx, overload_responses_rx) = mpsc::unbounded::<QueuedOutput>();
        let ordinary_concurrency = self.ordinary_concurrency.max(1);
        let retained_deferred_capacity = self.retained_deferred_capacity;
        let retained_deferred_budget = Arc::new(Semaphore::new(retained_deferred_capacity));
        let request_byte_budget = self.request_byte_budget;
        let request_budget = Arc::new(Semaphore::new(request_byte_budget));
        let max_control_message_bytes = self.max_control_message_bytes;
        let overload_response_byte_budget = self.overload_response_byte_budget;
        let overload_count_budget = Arc::new(Semaphore::new(self.overload_response_capacity));
        let overload_byte_budget = Arc::new(Semaphore::new(overload_response_byte_budget));
        let (output_drain_tx, mut output_drain_rx) = watch::channel(None::<Instant>);
        let task_drain_rx = output_drain_rx.clone();
        let client_response_drain_rx = output_drain_rx.clone();

        let (server_tasks_abort, server_tasks_registration) = future::AbortHandle::new_pair();
        let input_server_tasks_abort = server_tasks_abort.clone();
        let process_server_tasks = future::Abortable::new(
            process_handler_tasks(
                server_tasks_rx,
                ordinary_concurrency,
                responses_tx.clone(),
                task_drain_rx,
            ),
            server_tasks_registration,
        );

        let (client_response_tasks_abort, client_response_tasks_registration) =
            future::AbortHandle::new_pair();
        let input_client_response_tasks_abort = client_response_tasks_abort.clone();
        let output_client_response_tasks_abort = client_response_tasks_abort.clone();
        let process_client_responses = future::Abortable::new(
            route_client_responses(
                client_response_tasks_rx,
                client_responses,
                client_response_routing_failed_tx,
                client_response_drain_rx,
            ),
            client_response_tasks_registration,
        );

        let output_client_abort = client_abort.clone();
        let (input_abort, input_registration) = future::AbortHandle::new_pair();
        let read_input = future::Abortable::new(
            async move {
                let mut reader = LspFrameReader::new(self.stdin);
                let stop = loop {
                    let frame = tokio::select! {
                        biased;
                        _ = &mut client_response_routing_failed_rx => {
                            break TransportStop::InputClosed;
                        }
                        result = reader.read_frame() => match result {
                            Ok(frame) => frame,
                            Err(error) => {
                                tracing::error!(%error, "failed to read LSP message");
                                break TransportStop::InputClosed;
                            }
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
                            match client_response_tasks_tx.try_send(response) {
                                Ok(()) => {}
                                Err(error) => {
                                    tracing::error!(%error, "LSP client response lane rejected a response");
                                    break TransportStop::InputClosed;
                                }
                            }
                        }
                        FrameRead::Message {
                            message: IncomingMessage::Request(request),
                            body_length,
                        } => {
                            let request_id = request.id().cloned();
                            let class =
                                admission_class(&request, body_length, max_control_message_bytes);
                            if class == AdmissionClass::ImmediateControl {
                                match service.admit(request, AdmissionClass::ImmediateControl) {
                                    ServiceAdmission::Immediate(handler) => {
                                        if complete_immediate_control(handler).await {
                                            continue;
                                        }
                                        break TransportStop::InputClosed;
                                    }
                                    ServiceAdmission::Exit(handler) => {
                                        if !complete_immediate_control(handler).await {
                                            break TransportStop::InputClosed;
                                        }
                                        break TransportStop::ExitNotification;
                                    }
                                    ServiceAdmission::Deferred(handler) => {
                                        drop(handler);
                                        tracing::error!(
                                            "immediate LSP control admission returned deferred work"
                                        );
                                        break TransportStop::InputClosed;
                                    }
                                }
                            }

                            let retained_deferred_permit =
                                match Arc::clone(&retained_deferred_budget).try_acquire_owned() {
                                    Ok(permit) => permit,
                                    Err(error) => {
                                        tracing::warn!(
                                            %error,
                                            retained_deferred_capacity,
                                            request_id = ?request_id,
                                            "LSP retained deferred admission exhausted"
                                        );
                                        if let Err(stop) = reject_overloaded_message(
                                            request_id,
                                            &overload_count_budget,
                                            &overload_byte_budget,
                                            overload_response_byte_budget,
                                            &overload_responses_tx,
                                        ) {
                                            input_server_tasks_abort.abort();
                                            break stop;
                                        }
                                        continue;
                                    }
                                };
                            let request_byte_permit = match Arc::clone(&request_budget)
                                .try_acquire_many_owned(
                                    u32::try_from(body_length)
                                        .expect("validated LSP body length fits in u32"),
                                ) {
                                Ok(permit) => permit,
                                Err(error) => {
                                    drop(retained_deferred_permit);
                                    tracing::warn!(
                                        %error,
                                        body_length,
                                        request_byte_budget,
                                        request_id = ?request_id,
                                        "LSP ordinary message byte budget exhausted"
                                    );
                                    if let Err(stop) = reject_overloaded_message(
                                        request_id,
                                        &overload_count_budget,
                                        &overload_byte_budget,
                                        overload_response_byte_budget,
                                        &overload_responses_tx,
                                    ) {
                                        input_server_tasks_abort.abort();
                                        break stop;
                                    }
                                    continue;
                                }
                            };

                            let handler = match service.admit(request, AdmissionClass::Deferred) {
                                ServiceAdmission::Deferred(handler) => handler,
                                ServiceAdmission::Immediate(handler) => {
                                    drop(request_byte_permit);
                                    drop(retained_deferred_permit);
                                    if !complete_immediate_control(handler).await {
                                        break TransportStop::InputClosed;
                                    }
                                    continue;
                                }
                                ServiceAdmission::Exit(handler) => {
                                    drop(request_byte_permit);
                                    drop(retained_deferred_permit);
                                    if !complete_immediate_control(handler).await {
                                        break TransportStop::InputClosed;
                                    }
                                    break TransportStop::ExitNotification;
                                }
                            };
                            let task: HandlerTask = Box::pin(async move {
                                let _retained_deferred_permit = retained_deferred_permit;
                                let _request_byte_permit = request_byte_permit;
                                match handler.await {
                                    Ok(response) => response,
                                    Err(error) => {
                                        let error: BoxError = error.into();
                                        tracing::error!(
                                            error = %display_sources(error.as_ref()),
                                            "LSP request handler failed"
                                        );
                                        None
                                    }
                                }
                            });
                            if let Err(error) = server_tasks_tx.unbounded_send(task) {
                                tracing::error!(
                                    request_id = ?request_id,
                                    "LSP task handoff closed after service admission"
                                );
                                drop(error);
                                break TransportStop::InputClosed;
                            }
                        }
                        FrameRead::Error(error, recovery) => {
                            tracing::error!(%error, "failed to decode LSP message");
                            let response = Response::from_error(Id::Null, error.jsonrpc_error());
                            match responses_tx
                                .try_send(QueuedOutput::normal(OutgoingMessage::Response(response)))
                            {
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
                client_response_tasks_tx.disconnect();
                server_tasks_tx.disconnect();
                overload_responses_tx.disconnect();
                responses_tx.disconnect();
                if stop == TransportStop::ExitNotification {
                    // Exit must not wait for an unrelated response sink that is still Pending.
                    input_client_response_tasks_abort.abort();
                }
                client_abort.abort();
                if matches!(
                    stop,
                    TransportStop::InputClosed | TransportStop::InputOverloaded
                ) {
                    input_server_tasks_abort.abort();
                }
                stop
            },
            input_registration,
        );

        let print_output = async move {
            let mut stdout = std::pin::pin!(self.stdout);
            let responses = stream::select(responses_rx, overload_responses_rx);
            let client_requests = client_requests
                .map(OutgoingMessage::Request)
                .map(QueuedOutput::normal);
            let mut messages = stream::select(responses, client_requests);
            let mut output_failed = false;
            while let Some(message) = messages.next().await {
                match write_message_bounded(&mut stdout, &message.message, &mut output_drain_rx)
                    .await
                {
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
            if output_failed {
                // A dead stdout terminates the entire loopback; no response can make progress.
                output_client_response_tasks_abort.abort();
            }
            server_tasks_abort.abort();
            output_client_abort.abort();
            output_failed
        };

        let (_, _, output_failed, stop) = future::join4(
            process_server_tasks,
            process_client_responses,
            print_output,
            read_input,
        )
        .await;
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
        ordinary_concurrency: LSP_ORDINARY_HANDLER_CONCURRENCY,
        retained_deferred_capacity: ORDINARY_HANDLER_CAPACITY,
        request_byte_budget: LSP_REQUEST_BYTE_BUDGET,
        max_control_message_bytes: MAX_CONTROL_MESSAGE_BYTES,
        overload_response_capacity: OVERLOAD_RESPONSE_CAPACITY,
        overload_response_byte_budget: OVERLOAD_RESPONSE_BYTE_BUDGET,
    }
}

/// Serves an LSP session until input closes, output fails, overload loses input integrity, or the
/// client sends a valid `exit` notification.
///
/// Framing is owned here so a rejected `exit` request cannot make an upstream buffered reader
/// discard later frames from the same client write. A valid notification cancels in-flight handlers
/// through the lifecycle wrapper before the transport drains its task queue. Exact small controls
/// bypass ordinary capacity only while input remains synchronized. `OutputClosed` dominates a
/// concurrent protocol or overload stop.
pub async fn serve_stdio<I, O, L>(
    stdin: I,
    stdout: O,
    socket: L,
    service: MermanLspService,
) -> StdioTermination
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
{
    serve_stdio_inner(stdin, stdout, socket, service).await
}

async fn serve_stdio_inner<I, O, L, S>(
    stdin: I,
    stdout: O,
    socket: L,
    service: S,
) -> StdioTermination
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
    S: StdioAdmissionService + Send + 'static,
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
        TransportStop::InputOverloaded => StdioTermination::InputOverloaded,
        TransportStop::InputClosed | TransportStop::ExitNotification => lifecycle.termination(),
    }
}
