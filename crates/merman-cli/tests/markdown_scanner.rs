mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use support::exit_code;

fn run(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run merman-cli")
}

fn svg_count(directory: &Path) -> usize {
    fs::read_dir(directory)
        .expect("read output directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "svg"))
        .count()
}

#[test]
fn native_and_mmdc_route_to_distinct_scanner_snapshots() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("dialects.md");
    let source = concat!(
        "# Dialects\n",
        "```mermaid\nflowchart LR\nA-->B\n```\n",
        "```Mermaid\nflowchart LR\nB-->C\n```\n",
        "~~~mermaid\nflowchart LR\nC-->D\n~~~\n",
        "```` mermaid\nflowchart LR\nD-->E\n````\n",
        ":::mermaid title=Native\nflowchart LR\nE-->F\n:::\n",
        "~~~mermaid\nflowchart LR\nF-->G\n",
    );
    fs::write(&input, source).expect("write Markdown");

    let strict = run(
        tmp.path(),
        &["mmdc", "-i", "dialects.md", "-o", "strict.md", "--quiet"],
    );
    assert_eq!(exit_code(strict.status), 0, "{:?}", strict.stderr);
    assert_eq!(svg_count(tmp.path()), 1);
    let strict_document = fs::read_to_string(tmp.path().join("strict.md")).expect("strict output");
    assert_eq!(strict_document.matches("![diagram]").count(), 1);
    assert!(strict_document.contains("```Mermaid"));
    assert!(strict_document.contains("~~~mermaid"));

    let native = run(
        tmp.path(),
        &["batch", "dialects.md", "--output-dir", "native", "--quiet"],
    );
    assert_eq!(exit_code(native.status), 0, "{:?}", native.stderr);
    assert_eq!(svg_count(&tmp.path().join("native")), 6);
    let native_document =
        fs::read_to_string(tmp.path().join("native/dialects.md")).expect("native output");
    assert_eq!(native_document.matches("![diagram]").count(), 6);
}

#[test]
fn strict_scanner_preserves_the_pinned_regex_quirks() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join("quirks.md"),
        concat!(
            "````text\n",
            "`::mermaid\n",
            "flowchart LR\n",
            "A-->B\n",
            ":`:\n",
            "````\n",
            ":::mermaid\n",
            "flowchart LR\n",
            "B-->C:::\n",
        ),
    )
    .expect("write strict fixture");

    let output = run(
        tmp.path(),
        &["mmdc", "-i", "quirks.md", "-o", "quirks.svg", "--quiet"],
    );

    assert_eq!(exit_code(output.status), 0, "{:?}", output.stderr);
    assert!(tmp.path().join("quirks-1.svg").exists());
    assert!(tmp.path().join("quirks-2.svg").exists());
    assert!(!tmp.path().join("quirks-3.svg").exists());
    assert!(!tmp.path().join("quirks.svg").exists());
}

#[test]
fn native_accepts_mdx_while_mmdc_keeps_case_sensitive_path_detection() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = "```mermaid\nflowchart LR\nA-->B\n```\n";
    fs::write(tmp.path().join("input.mdx"), source).expect("write MDX");
    fs::write(tmp.path().join("input.MD"), source).expect("write uppercase Markdown");

    let native = run(
        tmp.path(),
        &[
            "batch",
            "input.mdx",
            "--output-dir",
            "native-mdx",
            "--quiet",
        ],
    );
    assert_eq!(exit_code(native.status), 0, "{:?}", native.stderr);
    assert!(tmp.path().join("native-mdx/input-1.svg").exists());

    for (input, output) in [("input.mdx", "mdx.svg"), ("input.MD", "uppercase.svg")] {
        let strict = run(tmp.path(), &["mmdc", "-i", input, "-o", output, "--quiet"]);
        assert_eq!(exit_code(strict.status), 1, "{input}: {:?}", strict.stderr);
        assert!(!tmp.path().join(output).exists());
        assert!(!tmp.path().join(output.replace(".svg", "-1.svg")).exists());
    }
}

#[test]
fn strict_zero_chart_and_native_line_endings_remain_observable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plain = "# No diagrams\r\n\r\nPlain text.\r\n";
    fs::write(tmp.path().join("plain.md"), plain).expect("write plain Markdown");

    let image = run(
        tmp.path(),
        &["mmdc", "-i", "plain.md", "-o", "plain.svg", "--quiet"],
    );
    assert_eq!(exit_code(image.status), 0, "{:?}", image.stderr);
    assert!(!tmp.path().join("plain.svg").exists());
    assert!(!tmp.path().join("plain-1.svg").exists());

    let document = run(
        tmp.path(),
        &["mmdc", "-i", "plain.md", "-o", "plain.out.md", "--quiet"],
    );
    assert_eq!(exit_code(document.status), 0, "{:?}", document.stderr);
    assert_eq!(
        fs::read(tmp.path().join("plain.out.md")).expect("read passthrough"),
        plain.as_bytes()
    );

    fs::write(
        tmp.path().join("crlf.md"),
        "```Mermaid\r\nflowchart LR\r\nA-->B\r\n```\r\nafter\r\n",
    )
    .expect("write CRLF Markdown");
    let native = run(
        tmp.path(),
        &["batch", "crlf.md", "--output-dir", "native-crlf", "--quiet"],
    );
    assert_eq!(exit_code(native.status), 0, "{:?}", native.stderr);
    let rewritten = fs::read(tmp.path().join("native-crlf/crlf.md")).expect("rewritten");
    assert!(
        rewritten
            .windows(b"\r\nafter\r\n".len())
            .any(|window| window == b"\r\nafter\r\n"),
        "native rewrite must retain the closing CRLF: {:?}",
        String::from_utf8_lossy(&rewritten)
    );
}

#[test]
fn chart_limit_errors_name_the_rejected_fence_location() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join("limit.md"),
        concat!(
            "```mermaid\nflowchart LR\nA-->B\n```\n",
            "标题\n",
            "  ```Mermaid\nflowchart LR\nB-->C\n```\n",
        ),
    )
    .expect("write limit fixture");

    let output = run(
        tmp.path(),
        &[
            "batch",
            "limit.md",
            "--output-dir",
            "limited",
            "--resource-limit",
            "max_markdown_charts=1",
        ],
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr UTF-8");

    assert_eq!(exit_code(output.status), 1, "{stderr}");
    assert!(stderr.contains("Markdown chart 2"), "{stderr}");
    assert!(stderr.contains("line 6, column 3"), "{stderr}");
    assert!(!tmp.path().join("limited/limit-1.svg").exists());

    fs::write(
        tmp.path().join("strict-limit.md"),
        concat!(
            "```mermaid\nflowchart LR\nA-->B\n```\n",
            "\u{2028}\t```mermaid\nflowchart LR\nB-->C\n```\n",
        ),
    )
    .expect("write strict limit fixture");
    let strict = run(
        tmp.path(),
        &[
            "mmdc",
            "-i",
            "strict-limit.md",
            "-o",
            "strict-limit.svg",
            "--resource-limit",
            "max_markdown_charts=1",
        ],
    );
    let strict_stderr = String::from_utf8(strict.stderr).expect("strict stderr UTF-8");

    assert_eq!(exit_code(strict.status), 1, "{strict_stderr}");
    assert!(
        strict_stderr.contains("Markdown chart 2"),
        "{strict_stderr}"
    );
    assert!(
        strict_stderr.contains("line 6, column 2"),
        "{strict_stderr}"
    );
    assert!(!tmp.path().join("strict-limit-1.svg").exists());
}

#[test]
fn render_errors_name_the_chart_and_native_fence_location() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join("render-error.md"),
        "# Document\n\n ```Mermaid\nflowchart LR\nA-->B\n```\n",
    )
    .expect("write render error fixture");

    let output = run(
        tmp.path(),
        &[
            "batch",
            "render-error.md",
            "--output-dir",
            "render-error",
            "--resource-limit",
            "max_staged_bytes=1",
        ],
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr UTF-8");

    assert_eq!(exit_code(output.status), 1, "{stderr}");
    assert!(stderr.contains("Markdown chart 1"), "{stderr}");
    assert!(stderr.contains("line 3, column 2"), "{stderr}");
    assert!(!tmp.path().join("render-error/render-error-1.svg").exists());
}
