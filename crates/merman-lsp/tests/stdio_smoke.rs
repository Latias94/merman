use std::{
    convert::Infallible,
    future::Future,
    io::{self, Read, Write},
    pin::Pin,
    process::{Child, Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    thread,
    time::Instant,
};

use futures::{StreamExt, channel::mpsc, sink, stream};
use merman_lsp::{LSP_HANDLER_CONCURRENCY, StdioTermination, serve_stdio, stdio_server};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};
use tower::Service;
use tower_lsp_server::Loopback;
use tower_lsp_server::jsonrpc::{Id, Request, Response};

fn frame(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
}

fn spawn_lsp_binary() -> Child {
    Command::new(env!("CARGO_BIN_EXE_merman-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn merman-lsp")
}

fn write_initialize(stdin: &mut impl Write) {
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#,
        ))
        .expect("write initialize request");
}

fn write_initialized(stdin: &mut impl Write) {
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
        ))
        .expect("write initialized notification");
}

fn initialize_lsp_binary(stdin: &mut impl Write) {
    write_initialize(stdin);
    write_initialized(stdin);
}

fn initialize_lsp_binary_and_wait(stdin: &mut impl Write, stdout: &mut impl Read) {
    write_initialize(stdin);
    let initialize_response = read_lsp_response_sync(stdout, 1);
    assert_eq!(initialize_response["id"], json!(1));
    write_initialized(stdin);
}

fn wait_with_output(mut child: Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().expect("poll merman-lsp child") {
            Some(_) => return child.wait_with_output().expect("collect merman-lsp output"),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                child.kill().expect("terminate timed-out merman-lsp child");
                let output = child
                    .wait_with_output()
                    .expect("collect timed-out merman-lsp output");
                panic!(
                    "merman-lsp did not exit within {timeout:?}; stderr:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}

fn wait_with_taken_stdout(
    child: Child,
    mut stdout: std::process::ChildStdout,
    timeout: Duration,
) -> Output {
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .read_to_end(&mut bytes)
            .expect("drain merman-lsp stdout");
        bytes
    });
    let mut output = wait_with_output(child, timeout);
    output.stdout = stdout_reader
        .join()
        .expect("stdout reader should not panic");
    output
}

fn decode_lsp_frames(stdout: &[u8]) -> Vec<serde_json::Value> {
    let mut offset = 0usize;
    let mut frames = Vec::new();

    while offset < stdout.len() {
        let rest = &stdout[offset..];
        assert!(
            rest.starts_with(b"Content-Length: "),
            "stdout contains non-LSP data at byte {offset}: {}",
            String::from_utf8_lossy(rest)
        );
        let header_end = rest
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("LSP frame header terminator");
        let header =
            std::str::from_utf8(&rest[..header_end]).expect("LSP frame header is valid UTF-8");
        let content_length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .expect("Content-Length header")
            .trim()
            .parse::<usize>()
            .expect("numeric Content-Length");
        let body_start = offset + header_end + 4;
        let body_end = body_start + content_length;
        assert!(
            body_end <= stdout.len(),
            "LSP frame body exceeds stdout length"
        );
        let frame = serde_json::from_slice::<serde_json::Value>(&stdout[body_start..body_end])
            .expect("LSP frame body is JSON");
        assert_eq!(
            frame.get("jsonrpc"),
            Some(&json!("2.0")),
            "LSP frame body must declare JSON-RPC 2.0"
        );
        frames.push(frame);
        offset = body_end;
    }

    frames
}

#[test]
#[should_panic(expected = "stdout contains non-LSP data")]
fn stdout_frame_decoder_rejects_trailing_data() {
    let mut stdout = frame(r#"{"jsonrpc":"2.0","id":1,"result":null}"#);
    stdout.extend_from_slice(b"trailing output");
    let _ = decode_lsp_frames(&stdout);
}

#[test]
fn stdout_frame_decoder_accepts_multiple_lsp_frames() {
    let mut stdout = frame(r#"{"jsonrpc":"2.0","id":1,"result":null}"#);
    stdout.extend_from_slice(&frame(
        r#"{"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"ready"}}"#,
    ));

    assert_eq!(decode_lsp_frames(&stdout).len(), 2);
}

fn read_lsp_frame_sync(reader: &mut impl Read) -> serde_json::Value {
    let mut header = Vec::new();
    let mut byte = [0u8; 1];
    while !header.ends_with(b"\r\n\r\n") {
        reader.read_exact(&mut byte).expect("read LSP frame header");
        header.push(byte[0]);
    }

    let header =
        std::str::from_utf8(&header[..header.len() - 4]).expect("LSP frame header is valid UTF-8");
    let content_length = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .expect("Content-Length header")
        .trim()
        .parse::<usize>()
        .expect("numeric Content-Length");
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).expect("read LSP frame body");
    serde_json::from_slice(&body).expect("LSP frame body is JSON")
}

fn read_lsp_response_sync(reader: &mut impl Read, expected_id: i64) -> serde_json::Value {
    loop {
        let frame = read_lsp_frame_sync(reader);
        if frame["id"] == json!(expected_id) {
            return frame;
        }
    }
}

fn read_lsp_responses_sync(reader: &mut impl Read, expected_ids: &[i64]) -> Vec<serde_json::Value> {
    let mut responses = Vec::with_capacity(expected_ids.len());
    while responses.len() < expected_ids.len() {
        let frame = read_lsp_frame_sync(reader);
        if expected_ids
            .iter()
            .any(|expected_id| frame["id"] == json!(expected_id))
        {
            responses.push(frame);
        }
    }
    responses
}

async fn read_lsp_frame(reader: &mut (impl AsyncRead + Unpin)) -> serde_json::Value {
    let mut header = Vec::new();
    let mut byte = [0u8; 1];
    while !header.ends_with(b"\r\n\r\n") {
        reader
            .read_exact(&mut byte)
            .await
            .expect("read LSP frame header");
        header.push(byte[0]);
    }

    let header =
        std::str::from_utf8(&header[..header.len() - 4]).expect("LSP frame header is valid UTF-8");
    let content_length = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .expect("Content-Length header")
        .trim()
        .parse::<usize>()
        .expect("numeric Content-Length");
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .await
        .expect("read LSP frame body");
    serde_json::from_slice(&body).expect("LSP frame body is JSON")
}

struct EmptyLoopback;

impl Loopback for EmptyLoopback {
    type RequestStream = stream::Empty<Request>;
    type ResponseSink = sink::Drain<Response>;

    fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
        (stream::empty(), sink::drain())
    }
}

struct TestLoopback {
    requests: Vec<Request>,
    responses: mpsc::UnboundedSender<Response>,
}

impl Loopback for TestLoopback {
    type RequestStream = stream::Iter<std::vec::IntoIter<Request>>;
    type ResponseSink = mpsc::UnboundedSender<Response>;

    fn split(self) -> (Self::RequestStream, Self::ResponseSink) {
        (stream::iter(self.requests), self.responses)
    }
}

struct LoopbackOnlyService;

impl Service<Request> for LoopbackOnlyService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        panic!("loopback-only service received request: {request}")
    }
}

struct WriteFailureWithPendingShutdown {
    shutdown_polled: Arc<AtomicBool>,
}

struct PendingWrite;

struct PendingShutdownWrite;

struct EofNotifyingRead<R> {
    inner: R,
    eof_observed: Option<oneshot::Sender<()>>,
}

impl<R: AsyncRead + Unpin> AsyncRead for EofNotifyingRead<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let filled_before = buffer.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Ok(())))
            && buffer.filled().len() == filled_before
            && let Some(eof_observed) = self.eof_observed.take()
        {
            let _ = eof_observed.send(());
        }
        result
    }
}

struct SplitFrameWrite {
    bytes: Arc<Mutex<Vec<u8>>>,
    header_written: Option<oneshot::Sender<()>>,
    body_release: oneshot::Receiver<()>,
    body_released: bool,
    writes: usize,
}

impl AsyncWrite for SplitFrameWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.writes > 0 && !self.body_released {
            match Pin::new(&mut self.body_release).poll(cx) {
                Poll::Ready(_) => self.body_released = true,
                Poll::Pending => return Poll::Pending,
            }
        }
        self.bytes.lock().unwrap().extend_from_slice(buffer);
        self.writes += 1;
        if let Some(header_written) = self.header_written.take() {
            let _ = header_written.send(());
        }
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for PendingWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Pending
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

impl AsyncWrite for PendingShutdownWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

impl AsyncWrite for WriteFailureWithPendingShutdown {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "synthetic output failure",
        )))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.shutdown_polled.store(true, Ordering::Release);
        Poll::Pending
    }
}

struct OverlapService {
    unblock: Option<oneshot::Receiver<()>>,
}

impl Service<Request> for OverlapService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let (method, id, _) = request.into_parts();
        let id = id.expect("test request id");
        match method.as_ref() {
            "test/block" => {
                let unblock = self.unblock.take().expect("single blocking request");
                Box::pin(async move {
                    let _ = unblock.await;
                    Ok(Some(Response::from_ok(id, json!("blocked"))))
                })
            }
            "test/ping" => Box::pin(async move { Ok(Some(Response::from_ok(id, json!("pong")))) }),
            other => panic!("unexpected test request: {other}"),
        }
    }
}

struct DelayedShutdownService {
    shutdown_started: Option<oneshot::Sender<()>>,
    unblock_shutdown: Option<oneshot::Receiver<()>>,
    exit_seen: Option<oneshot::Sender<()>>,
}

struct PendingNotificationService {
    notification_started: Option<oneshot::Sender<()>>,
    notification_cancelled: Option<oneshot::Sender<()>>,
    exit_seen: Option<oneshot::Sender<()>>,
}

struct DropNotifier(Option<oneshot::Sender<()>>);

impl Drop for DropNotifier {
    fn drop(&mut self) {
        if let Some(notifier) = self.0.take() {
            let _ = notifier.send(());
        }
    }
}

impl Service<Request> for PendingNotificationService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let (method, id, _) = request.into_parts();
        match method.as_ref() {
            "test/block-notification" => {
                assert!(id.is_none(), "blocking message must be a notification");
                let started = self
                    .notification_started
                    .take()
                    .expect("single blocking notification");
                let cancelled = self.notification_cancelled.take();
                Box::pin(async move {
                    let _cancelled = DropNotifier(cancelled);
                    let _ = started.send(());
                    std::future::pending::<()>().await;
                    Ok(None)
                })
            }
            "exit" => {
                assert!(id.is_none(), "exit must be a notification");
                let exit_seen = self.exit_seen.take().expect("single exit notification");
                Box::pin(async move {
                    let _ = exit_seen.send(());
                    Ok(None)
                })
            }
            other => panic!("unexpected lifecycle request: {other}"),
        }
    }
}

impl Service<Request> for DelayedShutdownService {
    type Response = Option<Response>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let (method, id, _) = request.into_parts();
        match method.as_ref() {
            "shutdown" => {
                let id = id.expect("shutdown request id");
                let shutdown_started = self
                    .shutdown_started
                    .take()
                    .expect("single shutdown request");
                let unblock_shutdown = self
                    .unblock_shutdown
                    .take()
                    .expect("single shutdown request");
                Box::pin(async move {
                    let _ = shutdown_started.send(());
                    let _ = unblock_shutdown.await;
                    Ok(Some(Response::from_ok(id, json!(null))))
                })
            }
            "exit" => {
                assert!(id.is_none(), "exit must be a notification");
                let exit_seen = self.exit_seen.take().expect("single exit notification");
                Box::pin(async move {
                    let _ = exit_seen.send(());
                    Ok(None)
                })
            }
            other => panic!("unexpected lifecycle request: {other}"),
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_server_processes_overlapping_requests() {
    const {
        assert!(
            LSP_HANDLER_CONCURRENCY > 1,
            "stdio handler concurrency must allow overlapping requests"
        );
    }

    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (server_stdout, mut client_stdout) = tokio::io::duplex(4096);
    let (unblock_tx, unblock_rx) = oneshot::channel();
    let mut unblock_tx = Some(unblock_tx);

    let server_task = tokio::spawn(async move {
        stdio_server(server_stdin, server_stdout, EmptyLoopback)
            .serve(OverlapService {
                unblock: Some(unblock_rx),
            })
            .await;
    });

    let blocking = frame(r#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#);
    let ping = frame(r#"{"jsonrpc":"2.0","id":2,"method":"test/ping","params":null}"#);
    client_stdin
        .write_all(&blocking[..10])
        .await
        .expect("write partial Content-Length header");
    tokio::task::yield_now().await;
    client_stdin
        .write_all(&blocking[10..blocking.len() - 5])
        .await
        .expect("write header and partial body");
    tokio::task::yield_now().await;
    let mut final_batch = blocking[blocking.len() - 5..].to_vec();
    final_batch.extend(ping);
    client_stdin
        .write_all(&final_batch)
        .await
        .expect("write final body fragment and pipelined request");

    let first_response = match timeout(Duration::from_secs(2), read_lsp_frame(&mut client_stdout))
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let _ = unblock_tx.take().expect("unblock sender").send(());
            panic!("lightweight request did not complete while first request was blocked: {err}");
        }
    };
    assert_eq!(first_response["id"], json!(2));
    assert_eq!(first_response["result"], json!("pong"));

    unblock_tx
        .take()
        .expect("unblock sender")
        .send(())
        .expect("blocking request receiver is alive");
    let second_response = timeout(Duration::from_secs(2), read_lsp_frame(&mut client_stdout))
        .await
        .expect("blocking request should complete after unblock");
    assert_eq!(second_response["id"], json!(1));
    assert_eq!(second_response["result"], json!("blocked"));

    drop(client_stdin);
    timeout(Duration::from_secs(2), server_task)
        .await
        .expect("stdio server should stop after stdin closes")
        .expect("stdio server task should not panic");
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_server_routes_loopback_requests_and_responses() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (server_stdout, mut client_stdout) = tokio::io::duplex(4096);
    let (responses_tx, mut responses_rx) = mpsc::unbounded();
    let client_request = Request::build("client/test").id(42).finish();

    let server_task = tokio::spawn(async move {
        stdio_server(
            server_stdin,
            server_stdout,
            TestLoopback {
                requests: vec![client_request],
                responses: responses_tx,
            },
        )
        .serve(LoopbackOnlyService)
        .await;
    });

    let request = timeout(Duration::from_secs(2), read_lsp_frame(&mut client_stdout))
        .await
        .expect("loopback request should reach stdout");
    assert_eq!(request["id"], json!(42));
    assert_eq!(request["method"], json!("client/test"));

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":42,"result":{"received":true}}"#,
        ))
        .await
        .expect("write loopback response");
    let response = timeout(Duration::from_secs(2), responses_rx.next())
        .await
        .expect("loopback response should be routed")
        .expect("loopback response stream should remain open");
    assert_eq!(response.id(), &Id::Number(42));
    assert_eq!(response.result(), Some(&json!({ "received": true })));

    drop(client_stdin);
    timeout(Duration::from_secs(2), server_task)
        .await
        .expect("stdio server should stop after stdin closes")
        .expect("stdio server task should not panic");
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_server_stops_when_output_closes_while_input_remains_open() {
    let (client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (server_stdout, client_stdout) = tokio::io::duplex(4096);
    drop(client_stdout);
    let (responses_tx, _responses_rx) = mpsc::unbounded();
    let client_request = Request::build("client/test").id(42).finish();

    let server_task = tokio::spawn(async move {
        serve_stdio(
            server_stdin,
            server_stdout,
            TestLoopback {
                requests: vec![client_request],
                responses: responses_tx,
            },
            LoopbackOnlyService,
        )
        .await
    });

    let termination = timeout(Duration::from_secs(2), server_task)
        .await
        .expect("closed output should cancel the blocked input reader")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::OutputClosed);
    drop(client_stdin);
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_write_failure_cancels_input_without_waiting_for_shutdown() {
    let (client_stdin, server_stdin) = tokio::io::duplex(4096);
    let shutdown_polled = Arc::new(AtomicBool::new(false));
    let stdout = WriteFailureWithPendingShutdown {
        shutdown_polled: Arc::clone(&shutdown_polled),
    };
    let (responses_tx, _responses_rx) = mpsc::unbounded();
    let client_request = Request::build("client/test").id(42).finish();

    let termination = timeout(
        Duration::from_secs(2),
        serve_stdio(
            server_stdin,
            stdout,
            TestLoopback {
                requests: vec![client_request],
                responses: responses_tx,
            },
            LoopbackOnlyService,
        ),
    )
    .await
    .expect("write failure should cancel the blocked input reader");

    assert_eq!(termination, StdioTermination::OutputClosed);
    assert!(
        !shutdown_polled.load(Ordering::Acquire),
        "a failed writer must not block session cancellation in shutdown"
    );
    drop(client_stdin);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stdio_pending_write_times_out_and_cancels_input() {
    let (_client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (responses_tx, _responses_rx) = mpsc::unbounded();
    let client_request = Request::build("client/test").id(42).finish();

    let termination = timeout(
        Duration::from_secs(31),
        serve_stdio(
            server_stdin,
            PendingWrite,
            TestLoopback {
                requests: vec![client_request],
                responses: responses_tx,
            },
            LoopbackOnlyService,
        ),
    )
    .await
    .expect("pending output write should be bounded");

    assert_eq!(termination, StdioTermination::OutputClosed);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stdio_exit_remains_reachable_when_protocol_error_output_is_saturated() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (notification_started_tx, notification_started_rx) = oneshot::channel();
    let (notification_cancelled_tx, notification_cancelled_rx) = oneshot::channel();
    let (exit_seen_tx, exit_seen_rx) = oneshot::channel();

    let server_task = tokio::spawn(async move {
        serve_stdio(
            server_stdin,
            PendingWrite,
            EmptyLoopback,
            PendingNotificationService {
                notification_started: Some(notification_started_tx),
                notification_cancelled: Some(notification_cancelled_tx),
                exit_seen: Some(exit_seen_tx),
            },
        )
        .await
    });

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","method":"test/block-notification","params":null}"#,
        ))
        .await
        .expect("write blocking notification");
    notification_started_rx
        .await
        .expect("blocking notification should start");

    let invalid = frame(r#"{"jsonrpc":"2.0","method":"#);
    let mut input = invalid.clone();
    input.extend(invalid);
    input.extend(frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#));
    client_stdin
        .write_all(&input)
        .await
        .expect("write malformed messages followed by exit");

    timeout(Duration::from_secs(1), exit_seen_rx)
        .await
        .expect("output backpressure must not block the exit notification")
        .expect("exit service should signal receipt");
    drop(client_stdin);

    let termination = timeout(Duration::from_secs(31), server_task)
        .await
        .expect("stalled protocol-error output should hit the drain deadline")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::OutputClosed);
    notification_cancelled_rx
        .await
        .expect("exit should cancel the pending handler despite stalled output");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stdio_eof_with_stalled_output_cancels_an_in_flight_notification() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (notification_started_tx, notification_started_rx) = oneshot::channel();
    let (notification_cancelled_tx, notification_cancelled_rx) = oneshot::channel();
    let (exit_seen_tx, _exit_seen_rx) = oneshot::channel();

    let server_task = tokio::spawn(async move {
        serve_stdio(
            server_stdin,
            PendingWrite,
            EmptyLoopback,
            PendingNotificationService {
                notification_started: Some(notification_started_tx),
                notification_cancelled: Some(notification_cancelled_tx),
                exit_seen: Some(exit_seen_tx),
            },
        )
        .await
    });

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","method":"test/block-notification","params":null}"#,
        ))
        .await
        .expect("write blocking notification");
    notification_started_rx
        .await
        .expect("blocking notification should start");
    client_stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"#))
        .await
        .expect("write malformed message for stalled output");
    drop(client_stdin);

    let termination = timeout(Duration::from_secs(31), server_task)
        .await
        .expect("EOF with stalled output should hit the bounded drain deadline")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::OutputClosed);
    notification_cancelled_rx
        .await
        .expect("EOF should cancel the pending handler despite stalled output");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stdio_output_shutdown_remains_bounded_after_eof() {
    let termination = timeout(
        Duration::from_secs(2),
        serve_stdio(
            tokio::io::empty(),
            PendingShutdownWrite,
            EmptyLoopback,
            LoopbackOnlyService,
        ),
    )
    .await
    .expect("a pending output shutdown should hit its own bound");

    assert_eq!(termination, StdioTermination::OutputClosed);
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_eof_finishes_an_output_frame_after_its_header_is_written() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (eof_observed_tx, eof_observed_rx) = oneshot::channel();
    let (header_written_tx, header_written_rx) = oneshot::channel();
    let (body_release_tx, body_release_rx) = oneshot::channel();
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer = SplitFrameWrite {
        bytes: Arc::clone(&output),
        header_written: Some(header_written_tx),
        body_release: body_release_rx,
        body_released: false,
        writes: 0,
    };
    let (unblock_tx, unblock_rx) = oneshot::channel();
    drop(unblock_tx);

    let server_task = tokio::spawn(async move {
        serve_stdio(
            EofNotifyingRead {
                inner: server_stdin,
                eof_observed: Some(eof_observed_tx),
            },
            writer,
            EmptyLoopback,
            OverlapService {
                unblock: Some(unblock_rx),
            },
        )
        .await
    });

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"test/ping","params":null}"#,
        ))
        .await
        .expect("write ping request");
    timeout(Duration::from_secs(2), header_written_rx)
        .await
        .expect("response header should be written")
        .expect("writer should signal the response header");
    drop(client_stdin);
    timeout(Duration::from_secs(2), eof_observed_rx)
        .await
        .expect("transport should observe EOF")
        .expect("EOF observer should remain alive");
    body_release_tx
        .send(())
        .expect("output must remain alive until the frame body is written");

    let termination = timeout(Duration::from_secs(2), server_task)
        .await
        .expect("stdio server should finish after draining the active frame")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::InputClosed);
    let frames = decode_lsp_frames(&output.lock().unwrap());
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["id"], json!(1));
    assert_eq!(frames[0]["result"], json!("pong"));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stdio_eof_does_not_hide_a_later_output_frame_timeout() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (eof_observed_tx, eof_observed_rx) = oneshot::channel();
    let (header_written_tx, header_written_rx) = oneshot::channel();
    let (_body_release_tx, body_release_rx) = oneshot::channel::<()>();
    let writer = SplitFrameWrite {
        bytes: Arc::new(Mutex::new(Vec::new())),
        header_written: Some(header_written_tx),
        body_release: body_release_rx,
        body_released: false,
        writes: 0,
    };
    let (unblock_tx, unblock_rx) = oneshot::channel();
    drop(unblock_tx);

    let server_task = tokio::spawn(async move {
        serve_stdio(
            EofNotifyingRead {
                inner: server_stdin,
                eof_observed: Some(eof_observed_tx),
            },
            writer,
            EmptyLoopback,
            OverlapService {
                unblock: Some(unblock_rx),
            },
        )
        .await
    });

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"test/ping","params":null}"#,
        ))
        .await
        .expect("write ping request");
    header_written_rx
        .await
        .expect("response header should be written");
    drop(client_stdin);
    eof_observed_rx.await.expect("transport should observe EOF");

    let termination = timeout(Duration::from_secs(31), server_task)
        .await
        .expect("pending response body should hit the output timeout")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::OutputClosed);
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_eof_cancels_an_in_flight_notification() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (server_stdout, _client_stdout) = tokio::io::duplex(4096);
    let (notification_started_tx, notification_started_rx) = oneshot::channel();
    let (exit_seen_tx, _exit_seen_rx) = oneshot::channel();

    let server_task = tokio::spawn(async move {
        serve_stdio(
            server_stdin,
            server_stdout,
            EmptyLoopback,
            PendingNotificationService {
                notification_started: Some(notification_started_tx),
                notification_cancelled: None,
                exit_seen: Some(exit_seen_tx),
            },
        )
        .await
    });

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","method":"test/block-notification","params":null}"#,
        ))
        .await
        .expect("write blocking notification");
    timeout(Duration::from_secs(2), notification_started_rx)
        .await
        .expect("blocking notification should start")
        .expect("notification service should signal start");
    drop(client_stdin);

    let termination = timeout(Duration::from_secs(2), server_task)
        .await
        .expect("EOF should cancel the blocked notification")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::InputClosed);
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_exit_before_shutdown_response_is_not_a_clean_exit() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (server_stdout, _client_stdout) = tokio::io::duplex(4096);
    let (shutdown_started_tx, shutdown_started_rx) = oneshot::channel();
    let (unblock_shutdown_tx, unblock_shutdown_rx) = oneshot::channel();
    let (exit_seen_tx, exit_seen_rx) = oneshot::channel();

    let server_task = tokio::spawn(async move {
        serve_stdio(
            server_stdin,
            server_stdout,
            EmptyLoopback,
            DelayedShutdownService {
                shutdown_started: Some(shutdown_started_tx),
                unblock_shutdown: Some(unblock_shutdown_rx),
                exit_seen: Some(exit_seen_tx),
            },
        )
        .await
    });

    client_stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#))
        .await
        .expect("write shutdown request");
    timeout(Duration::from_secs(2), shutdown_started_rx)
        .await
        .expect("shutdown request should start")
        .expect("shutdown service should signal start");

    client_stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .await
        .expect("write exit notification");
    timeout(Duration::from_secs(2), exit_seen_rx)
        .await
        .expect("exit notification should be processed")
        .expect("exit service should signal receipt");

    let termination = timeout(Duration::from_secs(2), server_task)
        .await
        .expect("stdio server should stop after exit")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::ExitWithoutShutdown);
    assert!(
        unblock_shutdown_tx.send(()).is_err(),
        "exit must cancel the in-flight shutdown handler"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_exit_cancels_an_in_flight_notification() {
    let (mut client_stdin, server_stdin) = tokio::io::duplex(4096);
    let (server_stdout, _client_stdout) = tokio::io::duplex(4096);
    let (notification_started_tx, notification_started_rx) = oneshot::channel();
    let (exit_seen_tx, exit_seen_rx) = oneshot::channel();

    let server_task = tokio::spawn(async move {
        serve_stdio(
            server_stdin,
            server_stdout,
            EmptyLoopback,
            PendingNotificationService {
                notification_started: Some(notification_started_tx),
                notification_cancelled: None,
                exit_seen: Some(exit_seen_tx),
            },
        )
        .await
    });

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","method":"test/block-notification","params":null}"#,
        ))
        .await
        .expect("write blocking notification");
    timeout(Duration::from_secs(2), notification_started_rx)
        .await
        .expect("blocking notification should start")
        .expect("notification service should signal start");

    client_stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .await
        .expect("write exit notification");
    timeout(Duration::from_secs(2), exit_seen_rx)
        .await
        .expect("exit notification should be processed")
        .expect("exit service should signal receipt");

    let termination = timeout(Duration::from_secs(2), server_task)
        .await
        .expect("stdio server should not drain the blocked notification")
        .expect("stdio server task should not panic");
    assert_eq!(termination, StdioTermination::ExitWithoutShutdown);
}

#[test]
fn stdio_binary_writes_only_lsp_frames_to_stdout() {
    let mut child = spawn_lsp_binary();

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");
    initialize_lsp_binary_and_wait(&mut stdin, &mut stdout);
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#))
        .expect("write shutdown request");
    let shutdown_response = read_lsp_response_sync(&mut stdout, 2);
    assert_eq!(shutdown_response["id"], json!(2));
    assert_eq!(shutdown_response["result"], json!(null));
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .expect("write exit notification");

    let output = wait_with_taken_stdout(child, stdout, Duration::from_secs(5));
    drop(stdin);
    assert!(
        output.status.success(),
        "merman-lsp exited with {:?}; stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = decode_lsp_frames(&output.stdout);
}

#[test]
fn stdio_binary_exits_with_error_when_exit_precedes_shutdown() {
    let mut child = spawn_lsp_binary();

    let mut stdin = child.stdin.take().expect("child stdin");
    initialize_lsp_binary(&mut stdin);
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .expect("write exit notification");

    let output = wait_with_output(child, Duration::from_secs(5));
    drop(stdin);
    assert_eq!(
        output.status.code(),
        Some(1),
        "merman-lsp should reject exit-before-shutdown; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdio_binary_rejected_shutdown_does_not_authorize_exit() {
    let mut child = spawn_lsp_binary();

    let mut stdin = child.stdin.take().expect("child stdin");
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#))
        .expect("write shutdown request before initialize");
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .expect("write exit notification");

    let output = wait_with_output(child, Duration::from_secs(5));
    drop(stdin);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a rejected shutdown request must not authorize a clean exit; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let shutdown_response = decode_lsp_frames(&output.stdout)
        .into_iter()
        .find(|frame| frame["id"] == json!(1))
        .expect("expected rejected shutdown response");
    assert!(shutdown_response.get("error").is_some());
}

#[test]
fn stdio_binary_does_not_accept_shutdown_notifications() {
    let mut child = spawn_lsp_binary();

    let mut stdin = child.stdin.take().expect("child stdin");
    initialize_lsp_binary(&mut stdin);
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"shutdown"}"#))
        .expect("write invalid shutdown notification");
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .expect("write exit notification");

    let output = wait_with_output(child, Duration::from_secs(5));
    drop(stdin);
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn stdio_binary_rejects_exit_requests_without_terminating() {
    let mut child = spawn_lsp_binary();

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");
    initialize_lsp_binary_and_wait(&mut stdin, &mut stdout);
    let mut pipelined = frame(r#"{"jsonrpc":"2.0","id":9,"method":"exit","params":null}"#);
    pipelined.extend(frame(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#));
    stdin
        .write_all(&pipelined)
        .expect("write pipelined invalid exit and shutdown requests");
    let responses = read_lsp_responses_sync(&mut stdout, &[9, 2]);
    let rejection = responses
        .iter()
        .find(|response| response["id"] == json!(9))
        .expect("invalid exit response");
    assert_eq!(rejection["id"], json!(9));
    assert_eq!(rejection["error"]["code"], json!(-32600));
    let shutdown_response = responses
        .iter()
        .find(|response| response["id"] == json!(2))
        .expect("pipelined shutdown response");
    assert_eq!(shutdown_response["id"], json!(2));
    assert_eq!(shutdown_response["result"], json!(null));
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .expect("write exit notification");

    let output = wait_with_taken_stdout(child, stdout, Duration::from_secs(5));
    drop(stdin);
    assert!(
        output.status.success(),
        "merman-lsp exited with {:?}; stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = decode_lsp_frames(&output.stdout);
}
