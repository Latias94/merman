use serde_json::json;
use std::{
    io::{Read, Write},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

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
        frames.push(
            serde_json::from_slice::<serde_json::Value>(&stdout[body_start..body_end])
                .expect("LSP frame body is JSON"),
        );
        offset = body_end;
    }

    frames
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
    assert_eq!(rejection["error"]["code"], json!(-32600));
    let shutdown_response = responses
        .iter()
        .find(|response| response["id"] == json!(2))
        .expect("pipelined shutdown response");
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
