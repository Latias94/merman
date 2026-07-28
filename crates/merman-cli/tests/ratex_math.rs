use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cli");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait cli")
}

#[test]
fn cli_renders_ratex_math_svg_to_stdout() {
    let output = run_with_stdin(
        &["render", "--math-renderer", "ratex", "--format", "svg", "-"],
        "flowchart LR\nA[\"$$x^2$$\"] --> B[Done]\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"width="0.97153em""#),
        "expected RaTeX inline SVG in CLI output:\n{stdout}"
    );
    assert!(
        stdout.contains("<path"),
        "expected RaTeX glyph paths in CLI output:\n{stdout}"
    );
    assert!(
        !stdout.contains("$$x^2$$"),
        "expected rendered output to replace math delimiters:\n{stdout}"
    );
}
