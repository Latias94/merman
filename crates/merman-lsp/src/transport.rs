use crate::session::{LSP_HANDLER_CONCURRENCY, LSP_MAX_MESSAGE_BYTES, LSP_REQUEST_BYTE_BUDGET};
use futures::channel::mpsc;
use futures::future::{self, BoxFuture};
use futures::stream;
use futures::{FutureExt, Sink, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{Notify, Semaphore, watch};
use tokio::time::Instant;
use tower::{BoxError, Service};
use tower_lsp_server::Loopback;
use tower_lsp_server::jsonrpc::{Error, Id, Request, Response};

const MESSAGE_QUEUE_SIZE: usize = 100;
const CONTROL_HANDLER_RESERVE: usize = LSP_HANDLER_CONCURRENCY;
const MAIN_HANDLER_LIMIT: usize = MESSAGE_QUEUE_SIZE - CONTROL_HANDLER_RESERVE;
const MAX_HEADER_BYTES: usize = 8 * 1024;
// Cancellation carries only a JSON-RPC envelope and request id. Keep enough room for string ids
// without allowing method-name spoofing to reserve an ordinary source-sized request.
const MAX_CONTROL_MESSAGE_BYTES: usize = 4 * 1024;
const CONTROL_REQUEST_BYTE_BUDGET: usize = MAX_CONTROL_MESSAGE_BYTES * LSP_HANDLER_CONCURRENCY;
const INPUT_DISPATCH_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

const RUNNING: u8 = 0;
const SHUTDOWN_COMPLETED: u8 = 1;
const EXIT_WITHOUT_SHUTDOWN: u8 = 2;
const EXIT_AFTER_SHUTDOWN: u8 = 3;

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

#[derive(Debug, Default)]
struct ProtocolLifecycleState {
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

struct ProtocolLifecycleService<S> {
    inner: S,
    lifecycle: Arc<ProtocolLifecycleState>,
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

#[derive(Deserialize)]
#[serde(untagged)]
enum IncomingMessage {
    Response(Response),
    Request(Request),
}

#[derive(Serialize)]
#[serde(untagged)]
enum OutgoingMessage {
    Response(Response),
    Request(Request),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recovery {
    Continue,
    Stop,
}

#[derive(Debug)]
enum FrameError {
    HeaderTooLarge { limit: usize },
    BodyTooLarge { length: usize, limit: usize },
    InvalidHeader(String),
    InvalidContentLength,
    MissingContentLength,
    EmptyBody,
    InvalidContentType,
    InvalidUtf8(std::str::Utf8Error),
    InvalidJson(serde_json::Error),
}

impl FrameError {
    fn jsonrpc_error(&self) -> Error {
        match self {
            Self::InvalidJson(error) if error.is_data() => Error::invalid_request(),
            _ => Error::parse_error(),
        }
    }
}

impl Display for FrameError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooLarge { limit } => {
                write!(formatter, "LSP headers exceed the {limit}-byte limit")
            }
            Self::BodyTooLarge { length, limit } => write!(
                formatter,
                "LSP message body is {length} bytes, exceeding the {limit}-byte limit"
            ),
            Self::InvalidHeader(message) => write!(formatter, "invalid LSP header: {message}"),
            Self::InvalidContentLength => formatter.write_str("invalid Content-Length header"),
            Self::MissingContentLength => {
                formatter.write_str("missing required Content-Length header")
            }
            Self::EmptyBody => formatter.write_str("LSP message body is empty"),
            Self::InvalidContentType => formatter
                .write_str("Content-Type must declare the utf-8 or utf8 character encoding"),
            Self::InvalidUtf8(error) => write!(formatter, "message body is not UTF-8: {error}"),
            Self::InvalidJson(error) => write!(formatter, "invalid JSON-RPC message: {error}"),
        }
    }
}

enum FrameRead {
    Eof,
    Message {
        message: IncomingMessage,
        body_length: usize,
    },
    Error(FrameError, Recovery),
}

struct HeaderBlock {
    prefix: Vec<u8>,
    too_large: bool,
    boundary_complete: bool,
}

struct HeaderParseError {
    error: FrameError,
    content_length: Option<usize>,
}

struct LspFrameReader<I> {
    input: BufReader<I>,
    max_header_bytes: usize,
    max_message_bytes: usize,
}

impl<I> LspFrameReader<I>
where
    I: AsyncRead + Unpin,
{
    fn new(input: I) -> Self {
        Self::with_limits(input, MAX_HEADER_BYTES, LSP_MAX_MESSAGE_BYTES)
    }

    fn with_limits(input: I, max_header_bytes: usize, max_message_bytes: usize) -> Self {
        Self {
            input: BufReader::new(input),
            max_header_bytes,
            max_message_bytes,
        }
    }

    async fn read_frame(&mut self) -> io::Result<FrameRead> {
        let Some(headers) = self.read_header_block().await? else {
            return Ok(FrameRead::Eof);
        };

        if headers.too_large {
            let content_length = content_length_from_prefix(&headers.prefix);
            let recovery = if headers.boundary_complete {
                self.recover_body(content_length).await?
            } else {
                Recovery::Stop
            };
            return Ok(FrameRead::Error(
                FrameError::HeaderTooLarge {
                    limit: self.max_header_bytes,
                },
                recovery,
            ));
        }

        let content_length = match parse_headers(&headers.prefix) {
            Ok(content_length) => content_length,
            Err(error) => {
                let recovery = self.recover_body(error.content_length).await?;
                return Ok(FrameRead::Error(error.error, recovery));
            }
        };

        if content_length > self.max_message_bytes {
            let recovery = self.recover_body(Some(content_length)).await?;
            return Ok(FrameRead::Error(
                FrameError::BodyTooLarge {
                    length: content_length,
                    limit: self.max_message_bytes,
                },
                recovery,
            ));
        }

        if content_length == 0 {
            return Ok(FrameRead::Error(FrameError::EmptyBody, Recovery::Continue));
        }

        let mut body = vec![0; content_length];
        self.input.read_exact(&mut body).await?;
        let body = match std::str::from_utf8(&body) {
            Ok(body) => body,
            Err(error) => {
                return Ok(FrameRead::Error(
                    FrameError::InvalidUtf8(error),
                    Recovery::Continue,
                ));
            }
        };
        tracing::trace!("<- {}", body);
        match serde_json::from_str(body) {
            Ok(message) => Ok(FrameRead::Message {
                message,
                body_length: content_length,
            }),
            Err(error) => Ok(FrameRead::Error(
                FrameError::InvalidJson(error),
                Recovery::Continue,
            )),
        }
    }

    async fn read_header_block(&mut self) -> io::Result<Option<HeaderBlock>> {
        let mut prefix = Vec::with_capacity(self.max_header_bytes.min(256));
        let mut too_large = false;
        let mut bytes_seen = 0usize;
        let recoverable_limit = self.max_header_bytes.saturating_mul(2);
        let mut previous_3 = 0u8;
        let mut previous_2 = 0u8;
        let mut previous_1 = 0u8;

        loop {
            let available = self.input.fill_buf().await?;
            if available.is_empty() {
                if bytes_seen == 0 {
                    return Ok(None);
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "stdin closed inside an LSP header block",
                ));
            }

            let mut consumed = 0usize;
            let mut complete = false;
            for byte in available.iter().copied() {
                consumed += 1;
                bytes_seen += 1;
                if prefix.len() < self.max_header_bytes {
                    prefix.push(byte);
                } else {
                    too_large = true;
                }

                let crlf_terminated = bytes_seen >= 4
                    && previous_3 == b'\r'
                    && previous_2 == b'\n'
                    && previous_1 == b'\r'
                    && byte == b'\n';
                let lf_terminated = bytes_seen >= 2 && previous_1 == b'\n' && byte == b'\n';
                previous_3 = previous_2;
                previous_2 = previous_1;
                previous_1 = byte;

                if crlf_terminated || lf_terminated {
                    complete = true;
                    break;
                }
                if bytes_seen > recoverable_limit {
                    break;
                }
            }
            self.input.consume(consumed);

            if complete {
                return Ok(Some(HeaderBlock {
                    prefix,
                    too_large,
                    boundary_complete: true,
                }));
            }
            if bytes_seen > recoverable_limit {
                return Ok(Some(HeaderBlock {
                    prefix,
                    too_large: true,
                    boundary_complete: false,
                }));
            }
        }
    }

    async fn recover_body(&mut self, content_length: Option<usize>) -> io::Result<Recovery> {
        let Some(content_length) = content_length else {
            return Ok(Recovery::Stop);
        };
        let recoverable_limit = self.max_message_bytes.saturating_mul(2);
        if content_length > recoverable_limit {
            return Ok(Recovery::Stop);
        }

        let mut remaining = content_length;
        let mut buffer = [0u8; 8 * 1024];
        while remaining > 0 {
            let chunk_length = remaining.min(buffer.len());
            self.input.read_exact(&mut buffer[..chunk_length]).await?;
            remaining -= chunk_length;
        }
        Ok(Recovery::Continue)
    }
}

fn content_length_from_prefix(headers: &[u8]) -> Option<usize> {
    let headers = std::str::from_utf8(headers).ok()?;
    let mut content_length = None;
    for line in headers.split_inclusive('\n') {
        if !line.ends_with('\n') {
            continue;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.eq_ignore_ascii_case("Content-Length") {
            continue;
        }
        if content_length.is_some() {
            return None;
        }
        content_length = parse_content_length(value);
        content_length?;
    }
    content_length
}

fn parse_headers(headers: &[u8]) -> Result<usize, HeaderParseError> {
    let headers = std::str::from_utf8(headers).map_err(|error| HeaderParseError {
        error: FrameError::InvalidHeader(error.to_string()),
        content_length: None,
    })?;
    let mut content_length = None;
    let mut saw_content_length = false;
    let mut first_error = None;

    for line in headers.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            first_error.get_or_insert_with(|| {
                FrameError::InvalidHeader("header line lacks ':' separator".to_owned())
            });
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("Content-Length") {
            if saw_content_length {
                return Err(HeaderParseError {
                    error: FrameError::InvalidHeader(
                        "duplicate Content-Length headers are not allowed".to_owned(),
                    ),
                    content_length: None,
                });
            }
            saw_content_length = true;
            match parse_content_length(value) {
                Some(length) => content_length = Some(length),
                None => {
                    first_error.get_or_insert(FrameError::InvalidContentLength);
                }
            };
        } else if name.eq_ignore_ascii_case("Content-Type") {
            if !valid_content_type(value) {
                first_error.get_or_insert(FrameError::InvalidContentType);
            }
        } else {
            tracing::warn!(header = name, "ignoring unsupported LSP header");
        }
    }

    if let Some(error) = first_error {
        return Err(HeaderParseError {
            error,
            content_length,
        });
    }
    content_length.ok_or(HeaderParseError {
        error: FrameError::MissingContentLength,
        content_length: None,
    })
}

fn parse_content_length(value: &str) -> Option<usize> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn valid_content_type(value: &str) -> bool {
    value.split(';').skip(1).any(|parameter| {
        let Some((name, value)) = parameter.trim().split_once('=') else {
            return false;
        };
        name.trim().eq_ignore_ascii_case("charset")
            && matches!(value.trim().to_ascii_lowercase().as_str(), "utf-8" | "utf8")
    })
}

async fn write_message<O>(output: &mut O, message: &OutgoingMessage) -> io::Result<()>
where
    O: AsyncWrite + Unpin,
{
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    tracing::trace!("-> {}", String::from_utf8_lossy(&body));
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    output.write_all(header.as_bytes()).await?;
    output.write_all(&body).await?;
    output.flush().await
}

async fn output_drain_deadline(
    drain_deadline: &mut watch::Receiver<Option<Instant>>,
) -> Option<Instant> {
    loop {
        let deadline = *drain_deadline.borrow_and_update();
        if deadline.is_some() {
            return deadline;
        }
        if drain_deadline.changed().await.is_err() {
            return None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        convert::Infallible, future::Future, io::Cursor, pin::Pin, sync::atomic::AtomicUsize,
    };
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    fn frame(body: &[u8]) -> Vec<u8> {
        let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        frame.extend_from_slice(body);
        frame
    }

    fn request_method(frame: FrameRead) -> String {
        match frame {
            FrameRead::Message {
                message: IncomingMessage::Request(request),
                ..
            } => request.method().to_owned(),
            _ => panic!("expected JSON-RPC request"),
        }
    }

    struct ResponseLoopback {
        responses: mpsc::UnboundedSender<Response>,
    }

    struct PendingResponseLoopback;

    struct PendingResponseSink;

    impl Loopback for ResponseLoopback {
        type RequestStream = stream::Empty<Request>;
        type ResponseSink = mpsc::UnboundedSender<Response>;

        fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
            (stream::empty(), self.responses)
        }
    }

    impl Loopback for PendingResponseLoopback {
        type RequestStream = stream::Empty<Request>;
        type ResponseSink = PendingResponseSink;

        fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
            (stream::empty(), PendingResponseSink)
        }
    }

    impl Sink<Response> for PendingResponseSink {
        type Error = Infallible;

        fn poll_ready(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Pending
        }

        fn start_send(self: Pin<&mut Self>, _item: Response) -> Result<(), Self::Error> {
            Ok(())
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Pending
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    struct BudgetedService {
        calls: Arc<AtomicUsize>,
        first_started: Option<oneshot::Sender<()>>,
        release_first: Option<oneshot::Receiver<()>>,
        second_called: Option<oneshot::Sender<()>>,
    }

    struct ExitReleasesBudgetedService {
        calls: Arc<AtomicUsize>,
        main_release: Option<oneshot::Receiver<()>>,
        control_release: Option<oneshot::Receiver<()>>,
        release_main: Option<oneshot::Sender<()>>,
        release_control: Option<oneshot::Sender<()>>,
        exit_seen: Option<oneshot::Sender<()>>,
    }

    impl Service<Request> for ExitReleasesBudgetedService {
        type Response = Option<Response>;
        type Error = Infallible;
        type Future =
            Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match request.method() {
                "test/block" => {
                    let release = self.main_release.take().expect("single main request");
                    Box::pin(async move {
                        let _ = release.await;
                        Ok(None)
                    })
                }
                "$/cancelRequest" => {
                    let release = self.control_release.take().expect("single control request");
                    Box::pin(async move {
                        let _ = release.await;
                        Ok(None)
                    })
                }
                "exit" => {
                    let _ = self
                        .release_main
                        .take()
                        .expect("single exit notification")
                        .send(());
                    let _ = self
                        .release_control
                        .take()
                        .expect("single exit notification")
                        .send(());
                    let _ = self
                        .exit_seen
                        .take()
                        .expect("single exit notification")
                        .send(());
                    Box::pin(async { Ok(None) })
                }
                method => panic!("unexpected test method: {method}"),
            }
        }
    }

    struct PendingService {
        calls: Arc<AtomicUsize>,
    }

    struct NeverReadyService {
        calls: Arc<AtomicUsize>,
    }

    struct ReservedControlService {
        calls: Arc<AtomicUsize>,
        cancel_seen: Option<oneshot::Sender<()>>,
        exit_seen: Option<oneshot::Sender<()>>,
    }

    struct SharedDeadlineWrite {
        first_flush_release: oneshot::Receiver<()>,
        first_flush_released: bool,
        flush_started: mpsc::UnboundedSender<usize>,
        active_flush: Option<usize>,
        completed_flushes: usize,
    }

    impl AsyncWrite for SharedDeadlineWrite {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(buffer.len()))
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            let flush = self.completed_flushes + 1;
            if self.active_flush != Some(flush) {
                self.active_flush = Some(flush);
                let _ = self.flush_started.unbounded_send(flush);
            }

            if flush == 1 && !self.first_flush_released {
                match Pin::new(&mut self.first_flush_release).poll(cx) {
                    Poll::Ready(_) => self.first_flush_released = true,
                    Poll::Pending => return Poll::Pending,
                }
            }
            if flush > 1 {
                return Poll::Pending;
            }

            self.completed_flushes = flush;
            self.active_flush = None;
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Service<Request> for PendingService {
        type Response = Option<Response>;
        type Error = Infallible;
        type Future =
            Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _request: Request) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(std::future::pending())
        }
    }

    impl Service<Request> for NeverReadyService {
        type Response = Option<Response>;
        type Error = Infallible;
        type Future =
            Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Pending
        }

        fn call(&mut self, _request: Request) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(None) })
        }
    }

    impl Service<Request> for ReservedControlService {
        type Response = Option<Response>;
        type Error = Infallible;
        type Future =
            Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match request.method() {
                "$/cancelRequest" => {
                    let _ = self
                        .cancel_seen
                        .take()
                        .expect("single cancel notification")
                        .send(());
                }
                "exit" => {
                    let _ = self
                        .exit_seen
                        .take()
                        .expect("single exit notification")
                        .send(());
                }
                _ => {}
            }
            Box::pin(std::future::pending())
        }
    }

    impl Service<Request> for BudgetedService {
        type Response = Option<Response>;
        type Error = Infallible;
        type Future =
            Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match request.id() {
                Some(Id::Number(1)) => {
                    let first_started = self
                        .first_started
                        .take()
                        .expect("first request is called once");
                    let release_first = self
                        .release_first
                        .take()
                        .expect("first request is called once");
                    Box::pin(async move {
                        let _ = first_started.send(());
                        let _ = release_first.await;
                        Ok(None)
                    })
                }
                Some(Id::Number(2)) => {
                    let second_called = self
                        .second_called
                        .take()
                        .expect("second request is called once");
                    let _ = second_called.send(());
                    Box::pin(async { Ok(None) })
                }
                id => panic!("unexpected test request id: {id:?}"),
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn frame_reader_preserves_pipelined_messages() {
        let mut bytes = frame(br#"{"jsonrpc":"2.0","id":9,"method":"exit"}"#);
        bytes.extend(frame(br#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#));
        let mut reader = LspFrameReader::new(Cursor::new(bytes));

        assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
        assert_eq!(
            request_method(reader.read_frame().await.unwrap()),
            "shutdown"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn frame_reader_recovers_after_utf8_and_json_errors() {
        let mut bytes = frame(&[0xff]);
        bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"#));
        bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
        let mut reader = LspFrameReader::new(Cursor::new(bytes));

        assert!(matches!(
            reader.read_frame().await.unwrap(),
            FrameRead::Error(FrameError::InvalidUtf8(_), Recovery::Continue)
        ));
        assert!(matches!(
            reader.read_frame().await.unwrap(),
            FrameRead::Error(FrameError::InvalidJson(_), Recovery::Continue)
        ));
        assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn frame_reader_reports_an_empty_body_and_recovers() {
        let mut bytes = b"Content-Length: 0\r\n\r\n".to_vec();
        bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
        let mut reader = LspFrameReader::new(Cursor::new(bytes));

        let error = reader.read_frame().await.unwrap();
        assert!(matches!(
            &error,
            FrameRead::Error(FrameError::EmptyBody, Recovery::Continue)
        ));
        let FrameRead::Error(error, _) = error else {
            unreachable!("empty frame must produce an error")
        };
        assert_eq!(
            error.jsonrpc_error().code,
            tower_lsp_server::jsonrpc::ErrorCode::ParseError
        );
        assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn frame_reader_discards_a_bounded_oversized_body() {
        let mut bytes = frame(&[b'x'; 65]);
        bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
        let mut reader = LspFrameReader::with_limits(Cursor::new(bytes), 64, 64);

        assert!(matches!(
            reader.read_frame().await.unwrap(),
            FrameRead::Error(
                FrameError::BodyTooLarge {
                    length: 65,
                    limit: 64
                },
                Recovery::Continue
            )
        ));
        assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn frame_reader_recovers_from_an_oversized_header_with_known_length() {
        let oversized = format!("Content-Length: 0\r\nX-Padding: {}\r\n\r\n", "x".repeat(80));
        let mut bytes = oversized.into_bytes();
        bytes.extend(frame(br#"{"jsonrpc":"2.0","method":"exit"}"#));
        let mut reader = LspFrameReader::with_limits(Cursor::new(bytes), 64, 64);

        assert!(matches!(
            reader.read_frame().await.unwrap(),
            FrameRead::Error(FrameError::HeaderTooLarge { limit: 64 }, Recovery::Continue)
        ));
        assert_eq!(request_method(reader.read_frame().await.unwrap()), "exit");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn frame_reader_stops_scanning_an_unbounded_header() {
        let input = format!("Content-Length: 0\r\nX-Padding: {}", "x".repeat(200));
        let mut reader = LspFrameReader::with_limits(Cursor::new(input.into_bytes()), 64, 64);

        assert!(matches!(
            reader.read_frame().await.unwrap(),
            FrameRead::Error(FrameError::HeaderTooLarge { limit: 64 }, Recovery::Stop)
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn frame_reader_distinguishes_clean_and_partial_eof() {
        let mut empty = LspFrameReader::new(Cursor::new(Vec::new()));
        assert!(matches!(empty.read_frame().await.unwrap(), FrameRead::Eof));

        let mut partial = LspFrameReader::new(Cursor::new(b"Content-Length: 5\r\n".to_vec()));
        let error = partial
            .read_frame()
            .await
            .err()
            .expect("partial frame should fail");
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn pending_client_response_routing_is_bounded() {
        let response = br#"{"jsonrpc":"2.0","id":99,"result":null}"#;
        let exit = br#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
        let mut input = frame(response);
        input.extend(frame(exit));
        let calls = Arc::new(AtomicUsize::new(0));

        let stop = timeout(
            INPUT_DISPATCH_TIMEOUT + Duration::from_secs(1),
            stdio_server(
                Cursor::new(input),
                Vec::<u8>::new(),
                PendingResponseLoopback,
            )
            .serve_inner(PendingService {
                calls: Arc::clone(&calls),
            }),
        )
        .await
        .expect("pending response routing must hit the input dispatch bound");

        assert_eq!(stop, TransportStop::InputClosed);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn pending_service_readiness_is_bounded_before_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let (responses_tx, _responses_rx) = mpsc::unbounded();

        let stop = timeout(
            INPUT_DISPATCH_TIMEOUT + Duration::from_secs(1),
            stdio_server(
                Cursor::new(frame(br#"{"jsonrpc":"2.0","method":"exit","params":null}"#)),
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
            )
            .serve_inner(NeverReadyService {
                calls: Arc::clone(&calls),
            }),
        )
        .await
        .expect("pending service readiness must hit the input dispatch bound");

        assert_eq!(stop, TransportStop::InputClosed);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_byte_budget_fails_closed_without_blocking_prior_client_responses() {
        let first = br#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#;
        let response = br#"{"jsonrpc":"2.0","id":99,"result":null}"#;
        let second = br#"{"jsonrpc":"2.0","id":2,"method":"test/block","params":null}"#;
        assert_eq!(first.len(), second.len());

        let mut input = frame(first);
        input.extend(frame(response));
        input.extend(frame(second));

        let calls = Arc::new(AtomicUsize::new(0));
        let (first_started_tx, _first_started_rx) = oneshot::channel();
        let (_release_first_tx, release_first_rx) = oneshot::channel();
        let (second_called_tx, second_called_rx) = oneshot::channel();
        let (responses_tx, mut responses_rx) = mpsc::unbounded();

        let server = stdio_server(
            Cursor::new(input),
            Vec::<u8>::new(),
            ResponseLoopback {
                responses: responses_tx,
            },
        )
        .request_byte_budget(first.len());
        let server_task = tokio::spawn({
            let calls = Arc::clone(&calls);
            async move {
                server
                    .serve(BudgetedService {
                        calls,
                        first_started: Some(first_started_tx),
                        release_first: Some(release_first_rx),
                        second_called: Some(second_called_tx),
                    })
                    .await;
            }
        });

        let routed_response = timeout(Duration::from_secs(2), responses_rx.next())
            .await
            .expect("client response should bypass the request byte budget")
            .expect("client response sink should remain open");
        assert_eq!(routed_response.id(), &Id::Number(99));
        assert!(
            timeout(Duration::from_secs(2), second_called_rx)
                .await
                .expect("server should close the second request signal")
                .is_err(),
            "a request beyond the byte budget must not call the service"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server should stop after exhausting the request byte budget")
            .expect("server task should not panic");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn bounded_cancel_and_exit_bypass_a_saturated_main_request_budget() {
        let main = br#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#;
        let cancel = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#;
        let exit = br#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
        assert!(exit.len() <= cancel.len());

        let mut input = frame(main);
        input.extend(frame(cancel));
        input.extend(frame(exit));

        let calls = Arc::new(AtomicUsize::new(0));
        let (release_main, main_release) = oneshot::channel();
        let (release_control, control_release) = oneshot::channel();
        let (exit_seen, exit_observed) = oneshot::channel();
        let (responses_tx, _responses_rx) = mpsc::unbounded();

        let stop = timeout(
            Duration::from_secs(2),
            stdio_server(
                Cursor::new(input),
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
            )
            .request_byte_budget(main.len())
            .control_request_byte_budget(cancel.len(), cancel.len())
            .serve_inner(ExitReleasesBudgetedService {
                calls: Arc::clone(&calls),
                main_release: Some(main_release),
                control_release: Some(control_release),
                release_main: Some(release_main),
                release_control: Some(release_control),
                exit_seen: Some(exit_seen),
            }),
        )
        .await
        .expect("bounded control messages should remain reachable");

        assert_eq!(stop, TransportStop::ExitNotification);
        exit_observed
            .await
            .expect("the exit notification should reach the service");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn control_messages_use_reserved_handler_capacity() {
        let mut input = Vec::new();
        for id in 0..MAIN_HANDLER_LIMIT {
            input.extend(frame(
                format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"test/block"}}"#).as_bytes(),
            ));
        }
        input.extend(frame(
            br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":0}}"#,
        ));
        input.extend(frame(br#"{"jsonrpc":"2.0","method":"exit","params":null}"#));

        let calls = Arc::new(AtomicUsize::new(0));
        let (cancel_seen_tx, cancel_seen_rx) = oneshot::channel();
        let (exit_seen_tx, exit_seen_rx) = oneshot::channel();
        let (responses_tx, _responses_rx) = mpsc::unbounded();
        let server = tokio::spawn({
            let calls = Arc::clone(&calls);
            async move {
                serve_stdio(
                    Cursor::new(input),
                    Vec::<u8>::new(),
                    ResponseLoopback {
                        responses: responses_tx,
                    },
                    ReservedControlService {
                        calls,
                        cancel_seen: Some(cancel_seen_tx),
                        exit_seen: Some(exit_seen_tx),
                    },
                )
                .await
            }
        });

        timeout(Duration::from_secs(1), cancel_seen_rx)
            .await
            .expect("cancel should use the reserved handler capacity")
            .expect("cancel service call should signal synchronously");
        timeout(Duration::from_secs(1), exit_seen_rx)
            .await
            .expect("exit should bypass all handler capacity")
            .expect("exit service call should signal synchronously");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            MAIN_HANDLER_LIMIT + 2,
            "ordinary work must leave the reserved control capacity intact"
        );

        timeout(OUTPUT_WRITE_TIMEOUT + Duration::from_secs(1), server)
            .await
            .expect("pending handlers should share the absolute drain deadline")
            .expect("stdio task should not panic");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn handler_saturation_fails_before_service_admission() {
        let mut input = Vec::new();
        for id in 0..=MAIN_HANDLER_LIMIT {
            input.extend(frame(
                format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"test/block"}}"#).as_bytes(),
            ));
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let (responses_tx, _responses_rx) = mpsc::unbounded();

        let termination = timeout(
            Duration::from_secs(1),
            serve_stdio(
                Cursor::new(input),
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
                PendingService {
                    calls: Arc::clone(&calls),
                },
            ),
        )
        .await
        .expect("handler saturation should fail closed without waiting for pending work");

        assert_eq!(termination, StdioTermination::InputClosed);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            MAIN_HANDLER_LIMIT,
            "the rejected message must not reach Service::call or ordered admission"
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn exit_polls_once_without_waiting_for_a_pending_handler() {
        let calls = Arc::new(AtomicUsize::new(0));
        let (responses_tx, _responses_rx) = mpsc::unbounded();

        let termination = timeout(
            Duration::from_secs(1),
            serve_stdio(
                Cursor::new(frame(br#"{"jsonrpc":"2.0","method":"exit","params":null}"#)),
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
                PendingService {
                    calls: Arc::clone(&calls),
                },
            ),
        )
        .await
        .expect("a pending exit handler must not hold transport shutdown open");

        assert_eq!(termination, StdioTermination::ExitWithoutShutdown);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn control_request_byte_budget_is_bounded() {
        let first = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#;
        let second = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":2}}"#;
        assert_eq!(first.len(), second.len());

        let mut input = frame(first);
        input.extend(frame(second));
        let calls = Arc::new(AtomicUsize::new(0));
        let (responses_tx, _responses_rx) = mpsc::unbounded();

        let stop = timeout(
            Duration::from_secs(2),
            stdio_server(
                Cursor::new(input),
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
            )
            .control_request_byte_budget(first.len(), first.len())
            .serve_inner(PendingService {
                calls: Arc::clone(&calls),
            }),
        )
        .await
        .expect("exhausting the control budget should fail closed");

        assert_eq!(stop, TransportStop::InputClosed);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn oversized_cancel_cannot_bypass_the_main_request_budget() {
        let main = br#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#;
        let normal_cancel = br#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":1}}"#;
        let oversized_cancel = format!(
            r#"{{"jsonrpc":"2.0","method":"$/cancelRequest","params":{{"id":1,"padding":"{}"}}}}"#,
            "x".repeat(normal_cancel.len())
        );
        assert!(oversized_cancel.len() > normal_cancel.len());

        let mut input = frame(main);
        input.extend(frame(oversized_cancel.as_bytes()));
        let calls = Arc::new(AtomicUsize::new(0));
        let (responses_tx, _responses_rx) = mpsc::unbounded();

        let stop = timeout(
            Duration::from_secs(2),
            stdio_server(
                Cursor::new(input),
                Vec::<u8>::new(),
                ResponseLoopback {
                    responses: responses_tx,
                },
            )
            .request_byte_budget(main.len())
            .control_request_byte_budget(normal_cancel.len(), normal_cancel.len())
            .serve_inner(PendingService {
                calls: Arc::clone(&calls),
            }),
        )
        .await
        .expect("an oversized pseudo-control message should fail closed");

        assert_eq!(stop, TransportStop::InputClosed);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn queued_messages_share_one_absolute_output_drain_deadline() {
        let (_drain_tx, mut drain_rx) = watch::channel(Some(Instant::now() + OUTPUT_WRITE_TIMEOUT));
        let (release_first_flush, first_flush_release) = oneshot::channel();
        let (flush_started_tx, mut flush_started_rx) = mpsc::unbounded();
        let writer = SharedDeadlineWrite {
            first_flush_release,
            first_flush_released: false,
            flush_started: flush_started_tx,
            active_flush: None,
            completed_flushes: 0,
        };

        let output = tokio::spawn(async move {
            let mut writer = writer;
            let first = OutgoingMessage::Response(Response::from_ok(
                Id::Number(1),
                serde_json::Value::Null,
            ));
            let second = OutgoingMessage::Response(Response::from_ok(
                Id::Number(2),
                serde_json::Value::Null,
            ));
            let first_result = write_message_bounded(&mut writer, &first, &mut drain_rx).await;
            let second_result = write_message_bounded(&mut writer, &second, &mut drain_rx).await;
            (first_result, second_result)
        });

        assert_eq!(
            flush_started_rx
                .next()
                .await
                .expect("the first message should begin flushing"),
            1
        );
        tokio::time::advance(Duration::from_secs(20)).await;
        release_first_flush
            .send(())
            .expect("the first message should still be draining");
        assert_eq!(
            flush_started_rx
                .next()
                .await
                .expect("the second message should begin flushing"),
            2
        );

        let (first_result, second_result) = timeout(Duration::from_secs(11), output)
            .await
            .expect("the second message must use the remaining shared deadline")
            .expect("output task should not panic");
        assert!(matches!(first_result, Ok(Ok(()))));
        assert!(second_result.is_err());
    }
}
