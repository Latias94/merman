use std::{
    convert::Infallible,
    future::Future,
    io::{Read, Write},
    pin::Pin,
    process::{Child, Command, Output, Stdio},
    task::{Context, Poll},
    thread,
    time::Instant,
};

use futures::{sink, stream};
use merman_lsp::{LSP_HANDLER_CONCURRENCY, StdioTermination, serve_stdio, stdio_server};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};
use tower::Service;
use tower_lsp::Loopback;
use tower_lsp::jsonrpc::{Request, Response};

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
    exit_seen: Option<oneshot::Sender<()>>,
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
                Box::pin(async move {
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

    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"test/block","params":null}"#,
        ))
        .await
        .expect("write blocking request");
    client_stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":2,"method":"test/ping","params":null}"#,
        ))
        .await
        .expect("write ping request");

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
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":9,"method":"exit","params":null}"#,
        ))
        .expect("write invalid exit request");
    let rejection = read_lsp_response_sync(&mut stdout, 9);
    assert_eq!(rejection["id"], json!(9));
    assert_eq!(rejection["error"]["code"], json!(-32600));
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
