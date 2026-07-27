#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

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
