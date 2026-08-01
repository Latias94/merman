mod support;

use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use support::{
    exit_code, pdf_media_box, repo_root, run_with_stdin, run_with_stdin_in_dir,
    serve_icon_json_once,
};

#[test]
fn mmdc_gantt_fixed_today_is_carried_through_compatibility_args() {
    let diagram = r#"gantt
dateFormat YYYY-MM-DD
section Demo
Anchor: id1,2026-01-01,1d
Missing ref: id2,after missing,1d
"#;
    let first = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "fixed-gantt",
            "--fixed-today",
            "2026-02-15",
            "--fixed-local-offset-minutes",
            "0",
        ],
        diagram,
    );
    let second = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "fixed-gantt",
            "--fixed-today",
            "2026-03-15",
            "--fixed-local-offset-minutes",
            "0",
        ],
        diagram,
    );

    assert!(first.status.success(), "stderr: {:?}", first.stderr);
    assert!(second.status.success(), "stderr: {:?}", second.stderr);
    assert!(
        String::from_utf8_lossy(&first.stdout)
            .trim_start()
            .starts_with("<svg")
    );
    assert!(
        String::from_utf8_lossy(&second.stdout)
            .trim_start()
            .starts_with("<svg")
    );
    assert_ne!(
        first.stdout, second.stdout,
        "mmdc must carry fixed Gantt time options through compatibility arguments"
    );
}

#[test]
fn default_mmdc_profile_renders_architecture_fixture() {
    let root = repo_root();
    let fixture = root
        .join("fixtures")
        .join("architecture")
        .join("upstream_docs_architecture_example_002.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("architecture.svg");
    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "mmdc",
            "-i",
            fixture.to_string_lossy().as_ref(),
            "-o",
            out.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let svg = fs::read_to_string(&out).expect("read SVG");
    assert!(svg.trim_start().starts_with("<svg"), "output is not SVG");
}

#[test]
fn mmdc_missing_output_directory_uses_output_exit_code() {
    let output = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "missing-dir/out.svg"],
        "flowchart LR\nA-->B\n",
    );

    assert!(
        !output.status.success(),
        "expected missing output directory failure"
    );
    assert_eq!(
        exit_code(output.status),
        2,
        "invalid output path should use usage/output exit code"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("Output directory") && stderr.contains("missing-dir"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn mmdc_output_dash_writes_to_stdout() {
    let output = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "-"],
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
fn mmdc_svg_pipeline_resvg_safe_outputs_export_safe_svg() {
    let diagram = "flowchart TD
A[Start] --> B{Is it working?}
B -->|Yes| C[Ship it]
B -->|No| D[Debug]
";
    let parity = run_with_stdin(&["mmdc", "-i", "-", "-o", "-"], diagram);
    let resvg_safe = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "-", "--svg-pipeline", "resvg-safe"],
        diagram,
    );

    assert!(parity.status.success(), "stderr: {:?}", parity.stderr);
    assert!(
        resvg_safe.status.success(),
        "stderr: {:?}",
        resvg_safe.stderr
    );

    let parity_svg = String::from_utf8(parity.stdout).expect("parity stdout should be utf8");
    let safe_svg = String::from_utf8(resvg_safe.stdout).expect("resvg-safe stdout should be utf8");
    assert!(
        parity_svg.contains("<foreignObject"),
        "default SVG output should preserve parity HTML labels:\n{parity_svg}"
    );
    assert!(
        !safe_svg.contains("<foreignObject"),
        "resvg-safe SVG output should not rely on foreignObject:\n{safe_svg}"
    );
    assert!(
        safe_svg.contains(r#"data-merman-foreignobject="fallback""#),
        "resvg-safe SVG output should keep generated text fallbacks:\n{safe_svg}"
    );
}

#[test]
fn mmdc_quadrant_pipeline_preserves_raw_token_and_materializes_resvg_fallback() {
    let diagram = "quadrantChart\n  title Reach and engagement\n  x-axis Low --> High\n  y-axis Low --> High\n  Campaign A: [0.3, 0.6]\n";
    let parity = run_with_stdin(&["mmdc", "-i", "-", "-o", "-"], diagram);
    let resvg_safe = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "-", "--svg-pipeline", "resvg-safe"],
        diagram,
    );

    assert!(parity.status.success(), "stderr: {:?}", parity.stderr);
    assert!(
        resvg_safe.status.success(),
        "stderr: {:?}",
        resvg_safe.stderr
    );

    let parity_svg = String::from_utf8(parity.stdout).expect("parity stdout should be utf8");
    let safe_svg = String::from_utf8(resvg_safe.stdout).expect("resvg-safe stdout should be utf8");
    assert!(
        parity_svg.contains(r#"fill="hsl(240, 100%, NaN%)""#),
        "raw/source output must preserve the pinned Mermaid token:\n{parity_svg}"
    );
    assert!(
        safe_svg.contains(r##"fill="#000000" stroke="none""##),
        "resvg-safe output must explicitly materialize the browser fallback:\n{safe_svg}"
    );
    assert!(!safe_svg.contains("NaN"), "resvg-safe output:\n{safe_svg}");
}

#[test]
fn mmdc_infers_png_from_output_extension() {
    let root = repo_root();
    let fixture = root.join("fixtures").join("flowchart").join("basic.mmd");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("out.png");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(&root)
        .args([
            "mmdc",
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
fn mmdc_rejects_unknown_output_extension() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["mmdc", "-i", "input.mmd", "-o", "out.unknown"])
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
fn mmdc_output_format_does_not_bypass_extension_validation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(&input, "flowchart LR\nA-->B\n").expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    let output = Command::new(exe)
        .current_dir(tmp.path())
        .args(["mmdc", "-i", "input.mmd", "-o", "out.unknown", "-e", "svg"])
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
fn mmdc_rejects_unknown_output_format() {
    let output = run_with_stdin(
        &["mmdc", "-i", "-", "-o", "-", "-e", "gif"],
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
fn mmdc_pdf_fit_controls_page_size() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    fs::write(
        &input,
        r#"---
config:
  xyChart:
    width: 9000
    height: 9000
---
xychart-beta
  x-axis [a, b]
  y-axis 0 --> 10
  line [1, 9]
"#,
    )
    .expect("write input");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["mmdc", "-i", "input.mmd", "-o", "default.pdf", "-q"])
        .assert()
        .success();
    Command::new(exe)
        .current_dir(tmp.path())
        .args(["mmdc", "-i", "input.mmd", "-o", "fit.pdf", "--pdfFit", "-q"])
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
    assert_eq!(
        fit_media_box, "0 0 600 600",
        "--pdfFit should match mmdc's 800 CSS pixel viewport converted to PDF points"
    );
}

#[test]
fn mmdc_default_output_for_stdin_writes_out_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = run_with_stdin_in_dir(&["mmdc", "-q"], "flowchart LR\nA-->B\n", Some(tmp.path()));

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
    let default_svg = run_with_stdin(&["mmdc", "-i", "-", "-o", "-", "-t", "default"], diagram);
    let dark_svg = run_with_stdin(&["mmdc", "-i", "-", "-o", "-", "-t", "dark"], diagram);
    let config_svg = run_with_stdin(
        &[
            "mmdc",
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
fn config_file_theme_variables_and_theme_css_affect_svg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("mermaid.json");
    fs::write(
        &config,
        r##"{
  "theme": "base",
  "themeVariables": {
    "mainBkg": "#111827",
    "nodeTextColor": "#f8fafc",
    "nodeBorder": "#38bdf8"
  },
  "themeCSS": ".node rect { filter: drop-shadow(1px 1px 1px #000); }"
}"##,
    )
    .expect("write config");

    let output = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "-I",
            "cli-theme-config",
            "-c",
            config.to_string_lossy().as_ref(),
        ],
        "flowchart TD\nA[Plain source]\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let svg = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(svg.contains("#111827"), "unexpected SVG:\n{svg}");
    assert!(svg.contains("#f8fafc"), "unexpected SVG:\n{svg}");
    assert!(svg.contains("#38bdf8"), "unexpected SVG:\n{svg}");
    assert!(
        svg.contains("#cli-theme-config .node rect { filter: drop-shadow(1px 1px 1px #000); }"),
        "unexpected SVG:\n{svg}"
    );
    assert_eq!(
        svg.matches("#cli-theme-config .node rect { filter: drop-shadow(1px 1px 1px #000); }")
            .count(),
        1,
        "theme CSS should be scoped and merged exactly once:\n{svg}"
    );
}

#[test]
fn non_object_config_file_fails_before_rendering() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("mermaid.json");
    fs::write(&config, r#""dark""#).expect("write config");

    let output = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "-c",
            config.to_string_lossy().as_ref(),
        ],
        "flowchart TD\nA[Plain source]\n",
    );

    assert!(!output.status.success(), "expected config file failure");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("configuration file") && stderr.contains("JSON object"),
        "unexpected stderr:\n{stderr}"
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
            "mmdc",
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
            "mmdc",
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
    let assets = docs.join("assets");
    fs::create_dir(&docs).expect("create docs dir");
    let input = docs.join("input.md");
    let output = docs.join("out.md");
    fs::write(&input, "```mermaid\nflowchart LR\nA-->B\n```\n").expect("write markdown");

    let exe = assert_cmd::cargo_bin!("merman-cli");
    Command::new(exe)
        .current_dir(tmp.path())
        .args([
            "mmdc",
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
        rewritten.contains("![diagram](./assets/out-1.svg)"),
        "unexpected rewritten markdown:\n{rewritten}"
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
        .args(["mmdc", "-i", input.to_string_lossy().as_ref(), "-o", "-"])
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
        .args(["mmdc", "-i", "input.md", "-o", "out.svg"])
        .output()
        .expect("run cli");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stdout.is_empty(),
        "Markdown diagnostics must not be written to stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("No mermaid charts found in Markdown input"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(!tmp.path().join("out.svg").exists());
    assert!(!tmp.path().join("out-1.svg").exists());
}

#[test]
fn missing_puppeteer_config_file_fails_before_rendering() {
    let output = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "-p",
            "missing-puppeteer-config.json",
        ],
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
            "mmdc",
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
    assert_eq!(
        exit_code(output.status),
        2,
        "invalid config should be usage/config error"
    );
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
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--iconPacksNamesAndUrls",
            &icon_arg,
        ],
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
fn dynamic_icon_pack_url_file_renders_tree_view_icon() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let icons = tmp.path().join("icons.json");
    fs::write(
        &icons,
        r#"{
            "prefix": "test",
            "left": 2,
            "top": 3,
            "width": 32,
            "height": 18,
            "icons": {
                "rocket": {
                    "body": "<path data-icon=\"tree-view-cli\" fill=\"currentColor\" d=\"M2 3H34V21H2z\"/>"
                }
            }
        }"#,
    )
    .expect("write icons");

    let icon_arg = format!("test#{}", icons.display());
    let output = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--iconPacksNamesAndUrls",
            &icon_arg,
        ],
        "treeView-beta\nRoot icon(test:rocket)\n",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="2 3 32 18"><path data-icon="tree-view-cli""#
        ),
        "expected a non-empty 14px TreeView icon symbol in SVG:\n{stdout}"
    );
    assert!(
        !stdout.contains(r#"<tspan x="0" y="0">?</tspan>"#),
        "custom TreeView icon should replace the unknown icon:\n{stdout}"
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
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--allow-network",
            "--allow-private-network",
            "--iconPacksNamesAndUrls",
            &icon_arg,
        ],
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
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--iconPacks",
            "@iconify-json/test",
        ],
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
