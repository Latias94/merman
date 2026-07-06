use std::{
    convert::Infallible,
    future::Future,
    io::Write,
    pin::Pin,
    process::{Command, Stdio},
    task::{Context, Poll},
};

use futures::{sink, stream};
use merman_lsp::{LSP_HANDLER_CONCURRENCY, stdio_server};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};
use tower::Service;
use tower_lsp::Loopback;
use tower_lsp::jsonrpc::{Request, Response};

fn frame(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", json.as_bytes().len(), json).into_bytes()
}

fn assert_lsp_frames_only(stdout: &[u8]) {
    let mut offset = 0usize;
    let mut frames = 0usize;

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
        serde_json::from_slice::<serde_json::Value>(&stdout[body_start..body_end])
            .expect("LSP frame body is JSON");
        offset = body_end;
        frames += 1;
    }

    assert!(frames >= 2, "expected initialize and shutdown responses");
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

#[tokio::test(flavor = "current_thread")]
async fn stdio_server_processes_overlapping_requests() {
    assert!(
        LSP_HANDLER_CONCURRENCY > 1,
        "stdio handler concurrency must allow overlapping requests"
    );

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

#[test]
fn stdio_binary_writes_only_lsp_frames_to_stdout() {
    let exe = env!("CARGO_BIN_EXE_merman-lsp");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn merman-lsp");

    let mut stdin = child.stdin.take().expect("child stdin");
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#,
        ))
        .expect("write initialize request");
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
        ))
        .expect("write initialized notification");
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#,
        ))
        .expect("write shutdown request");
    stdin
        .write_all(&frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#))
        .expect("write exit notification");
    drop(stdin);

    let output = child.wait_with_output().expect("wait for merman-lsp");
    assert!(
        output.status.success(),
        "merman-lsp exited with {:?}; stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_lsp_frames_only(&output.stdout);
}
