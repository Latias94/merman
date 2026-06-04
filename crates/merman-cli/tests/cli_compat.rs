use assert_cmd::prelude::*;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    run_with_stdin_in_dir(args, input, None)
}

fn run_with_stdin_in_dir(args: &[&str], input: &str, cwd: Option<&Path>) -> Output {
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

fn pdf_media_box(bytes: &[u8]) -> Option<String> {
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

fn serve_icon_json_once(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http server");
    let addr = listener.local_addr().expect("local addr");
    thread::spawn(move || {
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
    format!("http://{addr}/icons.json")
}

#[test]
fn cli_prints_help_successfully() {
    let exe = assert_cmd::cargo_bin!("merman-cli");

    for arg in ["--help", "-h"] {
        let output = Command::new(&exe).arg(arg).output().expect("run cli");

        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("Usage:"), "unexpected help:\n{stdout}");
        assert!(
            stdout.contains("-i, --input"),
            "help should include mmdc-compatible input flag:\n{stdout}"
        );
    }
}

#[test]
fn cli_prints_version_successfully() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .arg("--version")
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "unexpected version output:\n{stdout}"
    );
}

#[test]
fn cli_rejects_non_positive_numeric_options() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["-i", "-", "-o", "-", "--scale", "0"])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "expected --scale 0 to be rejected"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("expected a positive number"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn cli_rejects_non_positive_jobs() {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .args(["-i", "-", "-o", "-", "--jobs", "0"])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected --jobs 0 to be rejected");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("expected a positive integer"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn top_level_mmdc_flags_render_svg_file() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.svg");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "-i",
            fixture.to_string_lossy().as_ref(),
            "-o",
            out.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let svg = fs::read_to_string(&out).expect("read svg");
    assert!(svg.trim_start().starts_with("<svg"), "output is not SVG");
    assert!(svg.contains("flowchart"), "expected rendered flowchart SVG");
}

#[test]
fn top_level_missing_input_file_reports_path() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "missing.mmd", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected missing input failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Input file \"missing.mmd\" doesn't exist"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!tmp.path().join("out.svg").exists());
}

#[test]
fn top_level_output_dash_writes_to_stdout() {
    let output = run_with_stdin(
        &["-i", "-", "-o", "-"],
        "flowchart LR\nA[Start] --> B[Done]\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.trim_start().starts_with("<svg"),
        "expected SVG on stdout:\n{stdout}"
    );

    let dash_file = repo_root().join("-");
    assert!(
        !dash_file.exists(),
        "stdout output must not create a file named '-'"
    );
}

#[test]
fn top_level_infers_png_from_output_extension() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.png");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "-i",
            fixture.to_string_lossy().as_ref(),
            "-o",
            out.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let bytes = fs::read(&out).expect("read png");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "output is not a PNG"
    );
}

#[test]
fn top_level_rejects_unknown_output_extension() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "out.unknown"])
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "expected extension failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Output file must end"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!tmp.path().join("out.unknown").exists());
}

#[test]
fn top_level_output_format_does_not_bypass_extension_validation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "out.unknown", "-e", "svg"])
        .output()
        .expect("run cli");

    assert!(
        !output.status.success(),
        "explicit format should not bypass mmdc output extension validation"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Output file must end"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn top_level_rejects_unknown_output_format() {
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "-e", "gif"],
        "flowchart LR\nA-->B\n",
    );

    assert!(!output.status.success(), "expected output format failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("invalid value") && stderr.contains("gif"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn top_level_pdf_fit_controls_page_size() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(&exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "default.pdf", "-q"])
        .assert()
        .success();
    Command::new(&exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-o", "fit.pdf", "--pdfFit", "-q"])
        .assert()
        .success();

    let default_pdf = fs::read(tmp.path().join("default.pdf")).expect("read default pdf");
    let fit_pdf = fs::read(tmp.path().join("fit.pdf")).expect("read fit pdf");
    assert!(default_pdf.starts_with(b"%PDF-"));
    assert!(fit_pdf.starts_with(b"%PDF-"));

    let default_media_box = pdf_media_box(&default_pdf).expect("default media box");
    let fit_media_box = pdf_media_box(&fit_pdf).expect("fit media box");
    assert!(
        default_media_box.contains("612") && default_media_box.contains("792"),
        "default mmdc PDF should use a Letter-sized page, got {default_media_box}"
    );
    assert_ne!(
        default_media_box, fit_media_box,
        "--pdfFit should produce a chart-sized page distinct from default PDF output"
    );
}

#[test]
fn top_level_default_output_for_input_file_appends_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.mmd", "-q"])
        .assert()
        .success();

    let output = tmp.path().join("input.mmd.svg");
    assert!(
        output.exists(),
        "default mmdc output should append .svg to the input path"
    );
    let svg = fs::read_to_string(output).expect("read svg");
    assert!(svg.trim_start().starts_with("<svg"));
}

#[test]
fn top_level_default_output_for_stdin_writes_out_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = run_with_stdin_in_dir(&["-q"], "flowchart LR\nA-->B\n", Some(tmp.path()));

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let out = tmp.path().join("out.svg");
    assert!(out.exists(), "stdin default output should be out.svg");
    let svg = fs::read_to_string(out).expect("read svg");
    assert!(svg.trim_start().starts_with("<svg"));
}

#[test]
fn config_file_theme_overrides_cli_theme() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("mermaid.json");
    fs::write(&config, r#"{"theme":"default"}"#).expect("write config");

    let diagram = "flowchart LR\nA-->B\n";
    let default_svg = run_with_stdin(&["-i", "-", "-o", "-", "-t", "default"], diagram);
    let dark_svg = run_with_stdin(&["-i", "-", "-o", "-", "-t", "dark"], diagram);
    let config_svg = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "-t",
            "dark",
            "-c",
            config.to_string_lossy().as_ref(),
        ],
        diagram,
    );

    assert!(
        default_svg.status.success(),
        "stderr: {:?}",
        default_svg.stderr
    );
    assert!(dark_svg.status.success(), "stderr: {:?}", dark_svg.stderr);
    assert!(
        config_svg.status.success(),
        "stderr: {:?}",
        config_svg.stderr
    );
    assert_ne!(
        default_svg.stdout, dark_svg.stdout,
        "dark theme should differ from default theme"
    );
    assert_eq!(
        default_svg.stdout, config_svg.stdout,
        "config theme should override CLI theme like official mmdc"
    );
}

#[test]
fn markdown_input_writes_numbered_svg_artefacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output = tmp.path().join("out.svg");
    fs::write(
        &input,
        "before\n```mermaid\nflowchart LR\nA-->B\n```\nafter\n:::mermaid\nsequenceDiagram\nA->>B: Hi\n:::\n",
    )
    .expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "-q",
        ])
        .assert()
        .success();

    let first = fs::read_to_string(tmp.path().join("out-1.svg")).expect("read first svg");
    let second = fs::read_to_string(tmp.path().join("out-2.svg")).expect("read second svg");
    assert!(first.trim_start().starts_with("<svg"));
    assert!(second.trim_start().starts_with("<svg"));
    assert!(
        !output.exists(),
        "template image output should not be written for Markdown input"
    );
}

#[test]
fn markdown_output_rewrites_mermaid_blocks_to_images() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output = tmp.path().join("out.md");
    fs::write(&input, "# Doc\n\n```mermaid\nflowchart LR\nA-->B\n```\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "-q",
        ])
        .assert()
        .success();

    let rewritten = fs::read_to_string(&output).expect("read rewritten markdown");
    assert!(
        rewritten.contains("![diagram](./out-1.svg)"),
        "unexpected rewritten markdown:\n{rewritten}"
    );
    assert!(!rewritten.contains("```mermaid"));
    assert!(tmp.path().join("out-1.svg").exists());
}

#[test]
fn markdown_artefacts_directory_controls_image_location() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let docs = tmp.path().join("docs");
    let assets = tmp.path().join("assets");
    fs::create_dir(&docs).expect("create docs dir");
    let input = docs.join("input.md");
    let output = docs.join("out.md");
    fs::write(&input, "```mermaid\nflowchart LR\nA-->B\n```\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "-a",
            assets.to_string_lossy().as_ref(),
            "-q",
        ])
        .assert()
        .success();

    assert!(assets.join("out-1.svg").exists());
    let rewritten = fs::read_to_string(&output).expect("read rewritten markdown");
    assert!(
        rewritten.contains("![diagram](./../assets/out-1.svg)"),
        "unexpected rewritten markdown:\n{rewritten}"
    );
}

#[test]
fn markdown_jobs_preserve_rewrite_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output = tmp.path().join("out.md");
    fs::write(
        &input,
        "one\n```mermaid\nflowchart LR\nA1-->B1\n```\ntwo\n```mermaid\nflowchart LR\nA2-->B2\n```\nthree\n```mermaid\nflowchart LR\nA3-->B3\n```\n",
    )
    .expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "-i",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "--jobs",
            "2",
            "-q",
        ])
        .assert()
        .success();

    for index in 1..=3 {
        assert!(
            tmp.path().join(format!("out-{index}.svg")).exists(),
            "missing numbered artefact {index}"
        );
    }

    let rewritten = fs::read_to_string(&output).expect("read rewritten markdown");
    let first = rewritten.find("./out-1.svg").expect("first image");
    let second = rewritten.find("./out-2.svg").expect("second image");
    let third = rewritten.find("./out-3.svg").expect("third image");
    assert!(
        first < second && second < third,
        "Markdown image order must follow source order:\n{rewritten}"
    );
}

#[test]
fn markdown_input_rejects_stdout_output() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    fs::write(&input, "```mermaid\nflowchart LR\nA-->B\n```\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", input.to_string_lossy().as_ref(), "-o", "-"])
        .assert()
        .failure();
}

#[test]
fn markdown_without_charts_logs_and_writes_no_artefacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    fs::write(&input, "# No diagrams\n\nPlain text.\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["-i", "input.md", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("No mermaid charts found in Markdown input"),
        "unexpected stdout:\n{stdout}"
    );
    assert!(!tmp.path().join("out.svg").exists());
    assert!(!tmp.path().join("out-1.svg").exists());
}

#[test]
fn missing_puppeteer_config_file_fails_before_rendering() {
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "-p", "missing-puppeteer-config.json"],
        "flowchart LR\nA-->B\n",
    );

    assert!(!output.status.success(), "expected missing file failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Puppeteer configuration file")
            && stderr.contains("missing-puppeteer-config.json"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn invalid_puppeteer_config_file_fails_before_rendering() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("puppeteer.json");
    fs::write(&config, "{not json").expect("write invalid config");

    let output = run_with_stdin(
        &[
            "-i",
            "-",
            "-o",
            "-",
            "-p",
            config.to_string_lossy().as_ref(),
        ],
        "flowchart LR\nA-->B\n",
    );

    assert!(!output.status.success(), "expected JSON failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("JSON error"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn dynamic_icon_pack_url_file_renders_flowchart_icon() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let icons = tmp.path().join("icons.json");
    fs::write(
        &icons,
        r#"{
            "prefix": "test",
            "width": 16,
            "height": 16,
            "icons": {
                "rocket": {
                    "body": "<path data-icon=\"rocket\" fill=\"currentColor\" d=\"M1 1H15V15H1z\"/>"
                }
            }
        }"#,
    )
    .expect("write icons");

    let icon_arg = format!("test#{}", icons.display());
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "--iconPacksNamesAndUrls", &icon_arg],
        "flowchart TD\nA@{ icon: \"test:rocket\", label: \"Rocket\" }\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"data-icon="rocket""#),
        "expected custom icon body in SVG:\n{stdout}"
    );
    assert!(
        !stdout.contains(r#"<tspan x="0" y="0">?</tspan>"#),
        "custom icon should replace the placeholder SVG:\n{stdout}"
    );
}

#[test]
fn dynamic_icon_pack_http_url_renders_flowchart_icon() {
    let url = serve_icon_json_once(
        r#"{
            "prefix": "remote",
            "width": 16,
            "height": 16,
            "icons": {
                "cloud": {
                    "body": "<path data-icon=\"cloud\" fill=\"currentColor\" d=\"M1 8H15V14H1z\"/>"
                }
            }
        }"#,
    );

    let icon_arg = format!("remote#{url}");
    let output = run_with_stdin(
        &["-i", "-", "-o", "-", "--iconPacksNamesAndUrls", &icon_arg],
        "flowchart TD\nA@{ icon: \"remote:cloud\", label: \"Cloud\" }\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"data-icon="cloud""#),
        "expected HTTP icon body in SVG:\n{stdout}"
    );
}

#[test]
fn dynamic_icon_pack_package_renders_local_node_modules_icon() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let package = tmp
        .path()
        .join("node_modules")
        .join("@iconify-json")
        .join("test");
    fs::create_dir_all(&package).expect("create icon package");
    fs::write(
        package.join("icons.json"),
        r#"{
            "prefix": "ignored",
            "width": 20,
            "height": 20,
            "icons": {
                "box": {
                    "body": "<path data-icon=\"box\" fill=\"currentColor\" d=\"M2 2H18V18H2z\"/>"
                }
            }
        }"#,
    )
    .expect("write icons");

    let output = run_with_stdin_in_dir(
        &["-i", "-", "-o", "-", "--iconPacks", "@iconify-json/test"],
        "flowchart TD\nA@{ icon: \"test:box\", label: \"Box\" }\n",
        Some(tmp.path()),
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"data-icon="box""#),
        "expected local package icon body in SVG:\n{stdout}"
    );
}

#[test]
fn developer_subcommands_still_work() {
    let output = run_with_stdin(&["detect", "-"], "sequenceDiagram\nA->>B: Hello\n");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert_eq!(stdout.trim(), "sequence");
}
