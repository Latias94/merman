use std::{
    io::Write,
    process::{Command, Stdio},
};

fn frame(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
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
