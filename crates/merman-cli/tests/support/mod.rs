#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

pub fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

pub fn run_with_stdin(args: &[&str], input: &str) -> Output {
    run_with_stdin_in_dir(args, input, None)
}

pub fn run_with_stdin_bytes(args: &[&str], input: &[u8]) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn cli");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait cli")
}

pub fn run_with_stdin_in_dir(args: &[&str], input: &str, cwd: Option<&Path>) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let mut child = command.spawn().expect("spawn cli");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait cli")
}

pub fn run_before_stdin_close_in_dir(
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let mut child = command.spawn().expect("spawn cli");
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().expect("poll cli").is_some() {
            return child.wait_with_output().expect("collect cli output");
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill blocked cli");
            let output = child
                .wait_with_output()
                .expect("collect blocked cli output");
            panic!(
                "CLI waited for stdin instead of rejecting {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn run_with_closed_stdout(args: &[&str], input: Option<&[u8]>) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }

    let mut child = command.spawn().expect("spawn cli");
    drop(child.stdout.take().expect("stdout pipe"));
    if let Some(input) = input {
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(input)
            .expect("write stdin");
        drop(child.stdin.take());
    }

    child.wait_with_output().expect("wait cli")
}

pub fn pdf_media_box(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let marker = text.find("/MediaBox")?;
    let after_marker = &text[marker..];
    let start = after_marker.find('[')?;
    let end = after_marker[start..].find(']')? + start;
    Some(
        after_marker[start + 1..end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn serve_icon_json_once(body: &'static str) -> String {
    use std::io::Read;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http server");
    let address = listener.local_addr().expect("local addr");
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    });
    format!("http://{address}/icons.json")
}

#[cfg(unix)]
pub fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(-1)
}

#[cfg(windows)]
pub fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}
