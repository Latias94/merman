#![cfg(feature = "svg")]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
#[cfg(feature = "parallel-markdown")]
use std::time::{Duration, Instant};

const SOURCE: &str = "flowchart TD\nA[Start] --> B{Ready?}\nB -->|Yes| C[Ship]\n";

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for CLI")
}

#[test]
fn cli_renders_svg_from_stdin_to_stdout() {
    let output = run_with_stdin(&["render", "--format", "svg", "-"], SOURCE);

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = String::from_utf8(output.stdout).expect("SVG should be UTF-8");
    assert!(svg.trim_start().starts_with("<svg"), "{svg}");
    assert!(svg.contains("<foreignObject"), "{svg}");
    let doc = roxmltree::Document::parse(&svg).expect("valid SVG");
    assert!(
        !doc.root_element()
            .attribute("style")
            .unwrap_or_default()
            .contains("background"),
        "native SVG rendering should leave the canvas unpainted"
    );
}

#[test]
fn mmdc_svg_canvas_defaults_to_white_and_accepts_overrides() {
    for color in [None, Some("transparent"), Some("#112233")] {
        let mut args = vec!["mmdc", "-i", "-", "-o", "-"];
        if let Some(color) = color {
            args.extend(["-b", color]);
        }
        let output = run_with_stdin(&args, SOURCE);
        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let svg = String::from_utf8(output.stdout).expect("UTF-8 SVG");
        let doc = roxmltree::Document::parse(&svg).expect("valid SVG");
        assert!(
            doc.root_element()
                .attribute("style")
                .unwrap()
                .contains(&format!("background-color: {};", color.unwrap_or("white"))),
            "{svg}"
        );
    }
}

#[test]
fn native_svg_canvas_accepts_explicit_host_colors() {
    for color in ["transparent", "white", "#112233"] {
        let output = run_with_stdin(
            &["render", "--format", "svg", "--background", color, "-"],
            SOURCE,
        );
        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let svg = String::from_utf8(output.stdout).expect("UTF-8 SVG");
        let doc = roxmltree::Document::parse(&svg).expect("valid SVG");
        assert!(
            doc.root_element()
                .attribute("style")
                .unwrap()
                .contains(&format!("background-color: {color};")),
            "{svg}"
        );
    }
}

#[test]
fn cli_renders_named_input_to_a_sibling_svg_by_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("diagram.mmd");
    fs::write(&input, SOURCE).expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(repo_root())
        .args(["render", input.to_string_lossy().as_ref()])
        .output()
        .expect("run CLI");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = fs::read_to_string(input.with_extension("svg")).expect("read sibling SVG");
    assert!(svg.trim_start().starts_with("<svg"), "{svg}");
}

#[test]
fn cli_selects_the_resvg_safe_svg_pipeline() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "svg",
            "--svg-pipeline",
            "resvg-safe",
            "-",
        ],
        SOURCE,
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = String::from_utf8(output.stdout).expect("SVG should be UTF-8");
    assert!(!svg.contains("<foreignObject"), "{svg}");
    assert!(
        svg.contains(r#"data-merman-foreignobject="fallback""#),
        "{svg}"
    );
}

#[test]
fn svg_only_help_does_not_offer_raw_svg_conversion() {
    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    {
        let exe = assert_cmd::cargo_bin!("merman-cli");
        let output = Command::new(exe)
            .args(["render", "--help"])
            .output()
            .expect("run CLI help");

        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let help = String::from_utf8(output.stdout).expect("help should be UTF-8");
        assert!(!help.contains("--input-kind"), "{help}");
    }
}

#[test]
fn working_set_is_rejected_before_the_svg_backend_runs() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "svg",
            "--resource-limit",
            "max_scheduling_weight_bytes=1",
            "-",
        ],
        "this is not Mermaid",
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("max_scheduling_weight_bytes"), "{stderr}");
    assert!(!stderr.contains("UnknownDiagram"), "{stderr}");
}

#[cfg(not(feature = "math"))]
#[test]
fn svg_only_build_does_not_advertise_ratex() {
    let output = run_with_stdin(
        &["render", "--math-renderer", "ratex", "--format", "svg", "-"],
        SOURCE,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("invalid value 'ratex'"), "{stderr}");
    assert!(stderr.contains("possible values: none"), "{stderr}");
}

#[cfg(feature = "parallel-markdown")]
#[test]
fn parallel_markdown_progresses_with_capacity_for_one_backend() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output_dir = tmp.path().join("rendered");
    fs::write(
        &input,
        "```mermaid\nflowchart LR\nA-->B\n```\n\n```mermaid\nflowchart LR\nB-->C\n```\n",
    )
    .expect("write Markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut command = Command::new(exe);
    command.args([
        "batch",
        input.to_string_lossy().as_ref(),
        "--output-dir",
        output_dir.to_string_lossy().as_ref(),
        "--jobs",
        "2",
        "--resource-profile",
        "constrained",
        "--resource-limit",
        // The constrained SVG estimate fits once, but not twice, in 48 MiB.
        "max_scheduling_weight_bytes=50331648",
    ]);
    let output = run_with_timeout(command, Duration::from_secs(10));

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg_count = fs::read_dir(&output_dir)
        .expect("read output directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "svg"))
        .count();
    assert_eq!(svg_count, 2);
}

#[cfg(feature = "parallel-markdown")]
fn run_with_timeout(mut command: Command, timeout: Duration) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn CLI");
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().expect("poll CLI").is_some() {
            return child.wait_with_output().expect("collect CLI output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().expect("collect timed-out CLI");
            panic!("CLI did not finish within {timeout:?}: {:?}", output.stderr);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(all(feature = "diagram-class", feature = "layout-elk"))]
#[test]
fn native_defaults_render_nested_class_in_every_graphical_format() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("nested.mmd");
    fs::write(
        &input,
        include_str!("../../merman-rustdoc/tests/fixtures/class_nested_namespaces.mmd"),
    )
    .expect("write Class diagram");
    let formats = [
        "svg",
        #[cfg(feature = "png")]
        "png",
        #[cfg(feature = "jpeg")]
        "jpg",
        #[cfg(feature = "pdf")]
        "pdf",
    ];
    for format in formats {
        let output = tmp.path().join(format!("nested.{format}"));
        let mut command = Command::new(assert_cmd::cargo_bin!("merman-cli"));
        command
            .args([
                "render",
                "--format",
                format,
                "--operation-timeout-ms",
                "30000",
                "--output",
            ])
            .arg(&output)
            .arg(&input);
        if matches!(format, "png" | "jpg") {
            command.args(["--raster-fit-width", "256", "--raster-fit-height", "256"]);
        } else if format == "pdf" {
            // Exercise native admission/output without high-resolution blur work in debug tests.
            command.args(["--pdf-filter-scale", "0.1"]);
        }
        let result = command.output().expect("render Class diagram");
        assert!(
            result.status.success(),
            "{format}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = fs::read(output).expect("read diagram");
        match format {
            "svg" => assert!(String::from_utf8(bytes).unwrap().contains("C19")),
            "png" => assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n")),
            "jpg" => assert!(bytes.starts_with(b"\xff\xd8")),
            "pdf" => assert!(bytes.starts_with(b"%PDF-")),
            _ => unreachable!(),
        }
    }
}

#[cfg(all(
    feature = "diagram-class",
    feature = "layout-elk",
    feature = "parallel-markdown"
))]
#[test]
fn native_batch_completes_multiple_nested_class_diagrams() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.md");
    let output_dir = tmp.path().join("rendered");
    let source = include_str!("../../merman-rustdoc/tests/fixtures/class_nested_namespaces.mmd");
    fs::write(
        &input,
        format!("```mermaid\n{source}\n```\n\n```mermaid\n{source}\n```\n"),
    )
    .expect("write Markdown");
    let mut command = Command::new(assert_cmd::cargo_bin!("merman-cli"));
    command
        .args(["batch", "--jobs", "2", "--output-dir"])
        .arg(&output_dir)
        .arg(&input);
    let result = run_with_timeout(command, Duration::from_secs(30));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let svgs = fs::read_dir(output_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "svg"))
        .collect::<Vec<_>>();
    assert_eq!(svgs.len(), 2);
    for svg in svgs {
        assert!(fs::read_to_string(svg.path()).unwrap().contains("C19"));
    }
}
