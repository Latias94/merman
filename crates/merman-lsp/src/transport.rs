use futures::channel::mpsc;
use futures::future::{self, BoxFuture};
use futures::stream;
use futures::{Sink, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::Notify;
use tower::{BoxError, Service};
use tower_lsp_server::Loopback;
use tower_lsp_server::jsonrpc::{Error, Id, Request, Response};

/// Maximum number of client requests the stdio transport may process concurrently.
///
/// Keep workspace-wide requests from monopolizing the handler loop while document epochs and
/// snapshot generations guard response freshness.
pub const LSP_HANDLER_CONCURRENCY: usize = 4;

const MESSAGE_QUEUE_SIZE: usize = 100;
const MAX_HEADER_BYTES: usize = 8 * 1024;
// The source limit is 4 MiB. This leaves room for JSON string escaping and protocol metadata.
const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;
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
struct LifecycleState {
    state: AtomicU8,
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
    Message(IncomingMessage),
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
        Self::with_limits(input, MAX_HEADER_BYTES, MAX_MESSAGE_BYTES)
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
            Ok(message) => Ok(FrameRead::Message(message)),
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

        let (server_tasks_abort, server_tasks_registration) = future::AbortHandle::new_pair();
        let process_server_tasks = future::Abortable::new(
            async move {
                let mut tasks = server_tasks_rx.buffer_unordered(max_concurrency);
                while let Some(response) = tasks.next().await {
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
                        FrameRead::Message(IncomingMessage::Response(response)) => {
                            if let Err(error) = client_responses.send(response).await {
                                tracing::error!(
                                    error = %display_sources(&error),
                                    "failed to route LSP client response"
                                );
                                break TransportStop::InputClosed;
                            }
                        }
                        FrameRead::Message(IncomingMessage::Request(request)) => {
                            if let Err(error) = future::poll_fn(|cx| service.poll_ready(cx)).await {
                                let error: BoxError = error.into();
                                tracing::error!(
                                    error = %display_sources(error.as_ref()),
                                    "LSP service was not ready"
                                );
                                break TransportStop::InputClosed;
                            }

                            let will_exit = request.method() == "exit" && request.id().is_none();
                            let handler = service.call(request);
                            let task = async move {
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
                            };
                            if server_tasks_tx.send(task).await.is_err() {
                                break TransportStop::InputClosed;
                            }
                            if will_exit {
                                break TransportStop::ExitNotification;
                            }
                        }
                        FrameRead::Error(error, recovery) => {
                            tracing::error!(%error, "failed to decode LSP message");
                            let response = Response::from_error(Id::Null, error.jsonrpc_error());
                            if responses_tx
                                .send(OutgoingMessage::Response(response))
                                .await
                                .is_err()
                            {
                                break TransportStop::InputClosed;
                            }
                            if recovery == Recovery::Stop {
                                break TransportStop::InputClosed;
                            }
                        }
                    }
                };

                server_tasks_tx.disconnect();
                responses_tx.disconnect();
                client_abort.abort();
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
                if let Err(error) = write_message(&mut stdout, &message).await {
                    tracing::error!(%error, "failed to write LSP message");
                    output_failed = true;
                    break;
                }
            }
            input_abort.abort();
            server_tasks_abort.abort();
            output_client_abort.abort();

            if !output_failed {
                match tokio::time::timeout(OUTPUT_SHUTDOWN_TIMEOUT, stdout.shutdown()).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::error!(%error, "failed to close stdio output");
                    }
                    Err(_) => {
                        tracing::error!("timed out while closing stdio output");
                    }
                }
            }
        };

        let (_, _, stop) = future::join3(process_server_tasks, print_output, read_input).await;
        stop.unwrap_or(TransportStop::OutputClosed)
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
    let lifecycle = Arc::new(LifecycleState::default());
    let service = LifecycleService {
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
    use std::io::Cursor;

    fn frame(body: &[u8]) -> Vec<u8> {
        let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        frame.extend_from_slice(body);
        frame
    }

    fn request_method(frame: FrameRead) -> String {
        match frame {
            FrameRead::Message(IncomingMessage::Request(request)) => request.method().to_owned(),
            _ => panic!("expected JSON-RPC request"),
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
}
