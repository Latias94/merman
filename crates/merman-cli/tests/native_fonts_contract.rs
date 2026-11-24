#![cfg(all(feature = "native-fonts", feature = "diagram-flowchart"))]

mod support;

use support::{repo_root, run_with_stdin};

#[cfg(any(
    feature = "png",
    feature = "jpeg",
    feature = "pdf",
    all(target_os = "linux", feature = "platform-system-font-query")
))]
const RAW_FONT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text x="10" y="40" font-family="Cousine">Native font</text></svg>"#;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const RAW_PATH_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><path d="M10 10H40V40H10Z"/></svg>"#;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const RAW_NESTED_FONT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><svg x="5" y="5" width="190" height="70"><text x="10" y="40" font-family="Cousine">Native font</text></svg></svg>"#;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const RAW_TREF_FONT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><defs><g id="label">Native font</g></defs><text x="10" y="40" font-family="Cousine"><tref href="#label"/></text></svg>"##;

fn font() -> String {
    repo_root()
        .join("crates/merman-export/tests/fonts/Cousine-Regular.ttf")
        .to_string_lossy()
        .into_owned()
}

fn svg(args: &[&str], source: &str) -> String {
    let output = run_with_stdin(args, source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn measurement_notices(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("native measurement faces: "))
        .collect()
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn raw_conversion(format: &str, source: &str, options: &[&str]) -> std::process::Output {
    let mut args = vec!["render", "-", "--input-kind=svg", "--format", format];
    args.extend_from_slice(options);
    run_with_stdin(&args, source)
}

#[test]
fn native_measurement_diagnostics_describe_the_executing_svg() {
    let font = font();
    let context = merman::svg::export::NativeFontOptions::default()
        .with_font_file(&font)
        .load(&merman::OperationControl::new())
        .unwrap();
    let expected = format!(
        "native measurement faces: Cousine-Regular#0; context {}",
        context.identity()
    );
    let source = "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[Native measurement]\n";
    for command in ["render", "mmdc"] {
        let temp = tempfile::tempdir().unwrap();
        let output_path = temp.path().join("native-fonts.svg");
        let output_path = output_path.to_str().unwrap();
        let input = if command == "render" {
            "-"
        } else {
            "--input=-"
        };
        let mut args = vec![
            command,
            input,
            "--output",
            output_path,
            "--font-file",
            &font,
            "--font-system=off",
        ];
        let output = run_with_stdin(&args, source);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{command}: {stderr}");
        assert!(
            std::fs::read_to_string(output_path)
                .unwrap()
                .contains("<svg")
        );
        assert_eq!(
            measurement_notices(&stderr),
            [expected.as_str()],
            "{command}"
        );

        args.push("--quiet");
        let output = run_with_stdin(&args, source);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert!(
            std::fs::read_to_string(output_path)
                .unwrap()
                .contains("<svg")
        );
    }
}

#[test]
fn cumulative_font_byte_limit_preserves_output_and_gives_a_working_retry() {
    assert_eq!(
        merman::svg::export::NativeFontLimits::default().max_bytes,
        256 * 1024 * 1024
    );
    let regular = font();
    let bold = std::path::Path::new(&regular).with_file_name("Cousine-Bold.ttf");
    let total =
        std::fs::metadata(&regular).unwrap().len() + std::fs::metadata(&bold).unwrap().len();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("diagram.svg");
    let destination = path.to_str().unwrap();
    let previous = b"previous diagram";
    std::fs::write(&path, previous).unwrap();

    let render = |output: &str, size: &str| {
        run_with_stdin(
            &[
                "render",
                "-",
                "--output",
                output,
                "--font-file",
                &regular,
                "--font-file",
                bold.to_str().unwrap(),
                "--font-system=off",
                "--font-max-bytes",
                size,
            ],
            "flowchart LR\nA[Font budget]\n",
        )
    };
    let too_small = (total - 1).to_string();
    let file_failure = render(destination, &too_small);
    let stdout_failure = render("-", &too_small);
    for failure in [&file_failure, &stdout_failure] {
        let stderr = String::from_utf8_lossy(&failure.stderr);
        assert_eq!(failure.status.code(), Some(3), "{stderr}");
        assert!(failure.stdout.is_empty(), "{stderr}");
        assert!(stderr.contains(&total.to_string()), "{stderr}");
        assert!(stderr.contains(&too_small), "{stderr}");
    }
    assert_eq!(std::fs::read(&path).unwrap(), previous);

    let stderr = String::from_utf8_lossy(&file_failure.stderr);
    let suggested = stderr
        .split_once("--font-max-bytes ")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("missing --font-max-bytes retry value: {stderr}"));
    let retry = render(destination, suggested);
    assert!(
        retry.status.success(),
        "--font-max-bytes {suggested}: {}",
        String::from_utf8_lossy(&retry.stderr)
    );
    let artifact = std::fs::read_to_string(&path).unwrap();
    let document = roxmltree::Document::parse(&artifact).unwrap();
    assert!(document.root_element().has_tag_name("svg"));

    let exact = total.to_string();
    for size in [exact.as_str(), "1MiB"] {
        let output = render("-", size);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{size}: {stderr}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("<svg"));
    }
}

#[test]
fn invalid_font_byte_limits_fail_before_rendering() {
    for size in ["0", "1.5MiB"] {
        let invalid = support::run_before_stdin_close_in_dir(
            &["mmdc", "--input=-", "--output=-", "--font-max-bytes", size],
            None,
            std::time::Duration::from_secs(10),
        );
        let stderr = String::from_utf8_lossy(&invalid.stderr);
        assert_eq!(invalid.status.code(), Some(2), "{size}: {stderr}");
        assert!(invalid.stdout.is_empty(), "{size}: {stderr}");
    }
}

#[test]
fn exhausted_measurement_budget_explains_how_to_retry() {
    let font = font();
    let source = "flowchart LR\nA[Measure this label] --> B[Destination]\n";
    for command in ["render", "mmdc"] {
        let input = if command == "render" {
            "-"
        } else {
            "--input=-"
        };
        let args = [
            command,
            input,
            "--output=-",
            "--font-file",
            &font,
            "--font-system=off",
            "--font-max-measurements=1",
        ];
        for profile in ["trusted-native", "unbounded-for-trusted-input"] {
            let mut limited = args.to_vec();
            limited.extend(["--resource-profile", profile]);
            let output = run_with_stdin(&limited, source);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{stderr}");
            assert!(output.stdout.is_empty(), "no partial SVG may be published");
            assert!(stderr.contains("native_font_measurements"), "{stderr}");
            assert!(stderr.contains("actual=2 maximum=1"), "{stderr}");
            assert!(stderr.contains("--font-max-measurements 16384"), "{stderr}");
            assert!(!stderr.contains("--font-max-bytes"), "{stderr}");
            assert!(
                stderr.contains("resource profiles do not change"),
                "{stderr}"
            );
            assert!(
                !stderr.contains("NativeFontLimits"),
                "CLI help must use CLI options"
            );
        }
        let mut retry = args;
        retry[6] = "--font-max-measurements=16384";
        let output = svg(&retry, source);
        assert!(output.contains("<svg"));
    }
}

#[test]
fn measurement_budget_requires_native_mermaid_measurement() {
    let font = font();
    for extra in [
        vec![],
        vec!["--font-measurement=deterministic"],
        vec!["--font-file", &font, "--font-measurement=deterministic"],
    ] {
        let mut args = vec!["render", "-", "--output=-", "--font-max-measurements=1"];
        args.extend(extra);
        let output = run_with_stdin(&args, "flowchart LR\nA --> B\n");
        assert_eq!(output.status.code(), Some(2), "{:?}", output);
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--font-max-measurements"), "{stderr}");
        assert!(stderr.contains("native"), "{stderr}");
    }
}

#[cfg(feature = "png")]
#[test]
fn raw_svg_rejects_native_measurement_budget() {
    let font = font();
    let output = run_with_stdin(
        &[
            "render",
            "-",
            "--input-kind=svg",
            "--format=png",
            "--output=-",
            "--font-file",
            &font,
            "--font-max-measurements=1",
        ],
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("raw SVG is not relaid out"), "{stderr}");
}

#[cfg(feature = "png")]
#[test]
fn raw_svg_font_byte_limit_applies_to_native_painting() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("existing.png");
    std::fs::write(&destination, b"previous output").unwrap();
    let font = font();
    let convert = |flag| {
        raw_conversion(
            "png",
            RAW_FONT_SVG,
            &[
                "--output",
                destination.to_str().unwrap(),
                "--font-file",
                &font,
                flag,
            ],
        )
    };
    for (flag, status, detail) in [
        ("--font-max-bytes=1", 3, "--font-max-bytes "),
        ("--operation-timeout-ms=0", 1, "deadline_exceeded"),
    ] {
        let failed = convert(flag);
        let stderr = String::from_utf8_lossy(&failed.stderr);
        assert_eq!(failed.status.code(), Some(status), "{stderr}");
        assert!(failed.stdout.is_empty());
        assert!(stderr.contains(detail), "{stderr}");
        assert_eq!(std::fs::read(&destination).unwrap(), b"previous output");
    }
    let painted = convert("--font-max-bytes=1MiB");
    assert!(
        painted.status.success(),
        "{}",
        String::from_utf8_lossy(&painted.stderr)
    );
    assert!(
        std::fs::read(&destination)
            .unwrap()
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );

    let path_only = raw_conversion("png", RAW_PATH_SVG, &["--font-max-bytes=1"]);
    assert!(path_only.status.success(), "{path_only:?}");
    assert!(path_only.stdout.starts_with(b"\x89PNG\r\n\x1a\n"));

    let explicit_path = raw_conversion(
        "png",
        RAW_PATH_SVG,
        &["--font-file", &font, "--font-max-bytes=1"],
    );
    let stderr = String::from_utf8_lossy(&explicit_path.stderr);
    assert_eq!(explicit_path.status.code(), Some(3), "{stderr}");
    assert!(explicit_path.stdout.is_empty());
    assert!(stderr.contains("--font-max-bytes "), "{stderr}");
}

#[cfg(feature = "markdown")]
#[test]
fn batch_measurement_budget_applies_to_each_diagram() {
    let temp = tempfile::tempdir().unwrap();
    let source = (0..10)
        .map(|i| format!("```mermaid\nflowchart LR\nA[Diagram {i}] --> B[Destination {i}]\n```\n"))
        .collect::<String>();
    std::fs::write(temp.path().join("input.md"), source).unwrap();
    let font = font();
    let mut args = vec![
        "batch",
        "input.md",
        "--output-dir=generated",
        "--font-file",
        &font,
        "--font-system=off",
        "--font-max-measurements=8",
    ];
    #[cfg(feature = "parallel-markdown")]
    args.extend(["--jobs", "2"]);
    let output = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{}", stderr);
    let notices = measurement_notices(&stderr);
    assert_eq!(notices.len(), 10, "{stderr}");
    assert!(
        notices
            .iter()
            .all(|notice| notice.contains("Cousine-Regular#0; context ")),
        "{stderr}"
    );
    for i in 1..=10 {
        assert!(
            temp.path()
                .join(format!("generated/input-{i}.svg"))
                .is_file()
        );
    }
    args[2] = "--output-dir=failed";
    args[6] = "--font-max-measurements=1";
    let output = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("--font-max-measurements"), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(
        std::fs::read_dir(temp.path().join("failed"))
            .unwrap()
            .all(|entry| entry.unwrap().file_name() == ".merman.lock"),
        "no incomplete batch may be published"
    );
}

fn title<'a>(group: &'a usvg::Group, wanted: &str) -> Option<&'a usvg::Text> {
    for node in group.children() {
        match node {
            usvg::Node::Text(text)
                if text.chunks().iter().map(|c| c.text()).collect::<String>() == wanted =>
            {
                return Some(text);
            }
            usvg::Node::Group(group) => {
                if let Some(text) = title(group, wanted) {
                    return Some(text);
                }
            }
            _ => {}
        }
    }
    None
}

fn title_overflow(svg: &str, text: &str) -> f64 {
    let context = merman::svg::export::NativeFontOptions::default()
        .with_font_file(font())
        .load(&merman::OperationControl::new())
        .unwrap();
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let tree = usvg::Tree::from_str(svg, &options).unwrap();
    let text = title(tree.root(), text).expect("rendered title");
    let metric = text.bounding_box().transform(text.abs_transform()).unwrap();
    let ink = text
        .flattened()
        .bounding_box()
        .expect("rendered title ink bounds")
        .transform(text.abs_transform())
        .unwrap();
    f64::from(
        (-metric.x().min(ink.x())).max(0.0)
            + (metric.right().max(ink.right()) - tree.size().width()).max(0.0),
    )
}

#[test]
fn native_cli_title_contains_selected_font_metric_and_ink_where_default_overflows() {
    let label = "iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii";
    let source = format!(
        "---\ntitle: {label}\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA-->B\n"
    );
    let legacy = svg(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "native-title",
            "--font-measurement",
            "deterministic",
        ],
        &source,
    );
    let native = svg(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "native-title",
            "--font-file",
            &font(),
            "--font-system",
            "off",
        ],
        &source,
    );
    assert!(
        title_overflow(&legacy, label) > 10.0,
        "fixture must reproduce native-font clipping"
    );
    assert!(
        title_overflow(&native, label) < 0.05,
        "selected metric and ink must fit native canvas"
    );
}

#[test]
fn native_cli_sizes_node_text_without_a_title() {
    let label = "iiiiiiiiiiiiiiiiiiiiiiii";
    let source = format!(
        "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n  flowchart:\n    minNodeWidth: 0\n    wrappingWidth: 1000\n---\nflowchart LR\nA[{label}]\n"
    );
    let legacy = svg(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "native-node",
            "--font-measurement",
            "deterministic",
        ],
        &source,
    );
    let native = svg(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "native-node",
            "--font-file",
            &font(),
            "--font-system",
            "off",
        ],
        &source,
    );
    let node_width = |svg: &str| {
        let document = roxmltree::Document::parse(svg).unwrap();
        let node = document
            .descendants()
            .find(|node| {
                node.attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "node"))
            })
            .expect("flowchart node");
        node.descendants()
            .find(|element| {
                element.has_tag_name("rect")
                    && element.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "label-container")
                    })
            })
            .expect("rectangular node shape")
            .attribute("width")
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!(
        node_width(&native) > node_width(&legacy) + 30.0,
        "monospaced Cousine must resize the node around its narrow-looking glyphs"
    );
    let context = merman::svg::export::NativeFontOptions::default()
        .with_font_file(font())
        .load(&merman::OperationControl::new())
        .unwrap();
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let tree = usvg::Tree::from_str(&native, &options).unwrap();
    let text = title(tree.root(), label).expect("rendered node text");
    assert!(node_width(&native) >= f64::from(text.bounding_box().width()));
}

#[test]
fn compiled_native_feature_does_not_change_default_rendering() {
    let source = "---\nconfig:\n  fontFamily: Cousine\n---\nflowchart LR\nA-->B\n";
    let args = ["mmdc", "-i", "-", "-o", "-", "--svgId", "unchanged"];
    let implicit = svg(&args, source);
    for options in [
        ["--font-measurement", "deterministic"],
        ["--font-max-bytes", "1"],
    ] {
        let mut explicit = args.to_vec();
        explicit.extend(options);
        assert_eq!(implicit, svg(&explicit, source), "{options:?}");
    }
}

#[test]
fn default_measurement_preserves_post_layout_css_compatibility() {
    let css = tempfile::NamedTempFile::new().unwrap();
    let rule = ".nodeLabel { letter-spacing: 2px; }";
    std::fs::write(css.path(), rule).unwrap();
    let path = css.path().to_string_lossy();
    let source = "flowchart LR\nA-->B\n";
    let mut args = vec![
        "mmdc",
        "-i",
        "-",
        "-o",
        "-",
        "--svgId",
        "default-css",
        "--cssFile",
        &path,
    ];
    let automatic = svg(&args, source);
    assert!(automatic.contains(rule));
    args.extend(["--font-measurement", "deterministic"]);
    assert_eq!(automatic, svg(&args, source));
}

#[cfg(feature = "system-fonts")]
#[test]
fn system_fonts_require_explicit_native_measurement_or_system_opt_in() {
    let source = "---\nconfig:\n  fontFamily: monospace\n  htmlLabels: false\n  flowchart:\n    minNodeWidth: 0\n    wrappingWidth: 1000\n---\nflowchart LR\nA[iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii]\n";
    let args = ["mmdc", "-i", "-", "-o", "-", "--svgId", "system-opt-in"];
    let implicit = svg(&args, source);
    let mut deterministic = args.to_vec();
    deterministic.extend(["--font-measurement", "deterministic"]);
    assert_eq!(implicit, svg(&deterministic, source));
    let mut native = args.to_vec();
    native.extend(["--font-measurement", "native"]);
    let native_svg = svg(&native, source);
    assert_ne!(implicit, native_svg);
    let mut system = args.to_vec();
    system.extend(["--font-system", "on"]);
    assert_eq!(native_svg, svg(&system, source));
    let mut off = args.to_vec();
    off.extend(["--font-system", "off"]);
    assert_eq!(implicit, svg(&off, source));
}

#[cfg(feature = "system-fonts")]
#[test]
fn explicit_font_sources_remain_closed_without_system_opt_in() {
    let font = font();
    let source = "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[Font measurement]-->B\n";
    let mut args = vec![
        "mmdc",
        "-i",
        "-",
        "-o",
        "-",
        "--svgId",
        "closed-fonts",
        "--font-file",
        &font,
    ];
    let automatic = svg(&args, source);
    args.extend(["--font-system", "off"]);
    assert_eq!(automatic, svg(&args, source));
}

#[test]
fn font_sources_enable_the_same_measurement_as_explicit_native_mode() {
    let font = font();
    let source = "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[Font measurement]-->B\n";
    let mut args = vec![
        "mmdc",
        "-i",
        "-",
        "-o",
        "-",
        "--svgId",
        "font-mode",
        "--font-file",
        &font,
    ];
    let implicit = svg(&args, source);
    args.extend(["--font-measurement", "native"]);
    assert_eq!(implicit, svg(&args, source));
}

#[cfg(feature = "markdown")]
#[test]
fn native_font_budget_failure_aborts_the_entire_batch_publication() {
    fn snapshot(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
        let mut files = std::collections::BTreeMap::new();
        let mut directories = vec![root.to_path_buf()];
        while let Some(directory) = directories.pop() {
            if !directory.exists() {
                continue;
            }
            for entry in std::fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    directories.push(entry.path());
                } else {
                    files.insert(
                        entry.path().strip_prefix(root).unwrap().to_path_buf(),
                        std::fs::read(entry.path()).unwrap(),
                    );
                }
            }
        }
        files
    }

    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.md");
    let source = "```mermaid\nflowchart LR\nA[Normal]-->B[Diagram]\n```\n";
    std::fs::write(&input, source.repeat(2)).unwrap();
    let font = font();
    let mut args = vec![
        "batch",
        "input.md",
        "--output-dir",
        "generated",
        "--font-file",
        &font,
        "--font-system",
        "off",
        "--quiet",
    ];
    #[cfg(feature = "parallel-markdown")]
    args.extend(["--jobs", "2"]);
    let baseline = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
    assert!(
        baseline.status.success(),
        "{}",
        String::from_utf8_lossy(&baseline.stderr)
    );
    let output_root = temp.path().join("generated");
    let previous = snapshot(&output_root);
    assert!(
        previous
            .keys()
            .any(|path| path.extension().is_some_and(|ext| ext == "svg"))
    );

    let oversized_label = "i".repeat(64 * 1024 + 1);
    std::fs::write(
        &input,
        format!("{source}```mermaid\nflowchart LR\nC[{oversized_label}]\n```\n"),
    )
    .unwrap();
    #[cfg(feature = "parallel-markdown")]
    let job_counts = ["1", "2"];
    #[cfg(not(feature = "parallel-markdown"))]
    let job_counts = ["1"];
    for jobs in job_counts {
        #[cfg(feature = "parallel-markdown")]
        {
            let last = args.len() - 1;
            args[last] = jobs;
        }
        #[cfg(not(feature = "parallel-markdown"))]
        let _ = jobs;
        for output_directory in ["generated", "fresh"] {
            args[3] = output_directory;
            let output = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
            assert!(
                !output.status.success(),
                "native label budget must fail the batch"
            );
            assert!(
                output.stdout.is_empty(),
                "failed batch must not publish stdout artifacts"
            );
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(
                diagnostic.contains("native_font_label_bytes")
                    || diagnostic.contains("label-bytes"),
                "expected the native font label budget, not another failure: {diagnostic}"
            );
            assert!(!diagnostic.contains("--font-max-bytes"), "{diagnostic}");
            assert!(
                diagnostic.contains("Markdown chart 2 at line 5, column 1"),
                "expected the second fence location: {diagnostic}"
            );
            assert!(
                !temp
                    .path()
                    .join(output_directory)
                    .join(".merman.transaction")
                    .exists()
            );
            if output_directory == "generated" {
                assert_eq!(snapshot(&output_root), previous);
            } else {
                assert_eq!(
                    snapshot(&temp.path().join("fresh")),
                    std::collections::BTreeMap::from([(
                        std::path::PathBuf::from(".merman.lock"),
                        Vec::new(),
                    )]),
                    "aborted publication must leave only the stable empty transaction lock"
                );
            }
        }
    }
}

#[test]
fn explicit_font_files_are_protected_from_output_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let original = std::fs::read(font()).unwrap();
    let font_path = temp.path().join("font");
    std::fs::write(&font_path, &original).unwrap();
    std::fs::hard_link(&font_path, temp.path().join("font-alias")).unwrap();
    let source = "flowchart LR\nA-->B\n";
    let mut args = [
        "render",
        "-",
        "--font-file",
        "font",
        "--output",
        "-",
        "--format",
        "svg",
    ];
    let baseline = support::run_with_stdin_in_dir(&args, source, Some(temp.path()));
    assert!(
        baseline.status.success(),
        "extensionless font must be valid: {}",
        String::from_utf8_lossy(&baseline.stderr)
    );
    assert!(String::from_utf8_lossy(&baseline.stdout).contains("<svg"));

    for output_name in ["font", "font-alias"] {
        args[5] = output_name;
        let output = support::run_before_stdin_close_in_dir(
            &args,
            Some(temp.path()),
            std::time::Duration::from_secs(2),
        );
        assert!(
            !output.status.success(),
            "font output alias must be rejected"
        );
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("aliases protected font input") && diagnostic.contains(output_name),
            "expected a protected font alias error: {diagnostic}"
        );
        assert_eq!(std::fs::read(&font_path).unwrap(), original);
        assert_eq!(
            std::fs::read(temp.path().join("font-alias")).unwrap(),
            original
        );
    }
}

#[test]
fn explicit_font_directories_are_protected_without_restricting_mermaid_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let fonts = temp.path().join("fonts");
    std::fs::create_dir_all(fonts.join("nested")).unwrap();
    let original = std::fs::read(font()).unwrap();
    let font_path = fonts.join("selectedfont.ttf");
    std::fs::write(&font_path, &original).unwrap();
    let source = "flowchart LR\nA-->B\n";
    std::fs::write(fonts.join("diagram.mmd"), source).unwrap();
    std::fs::write(fonts.join("existing.svg"), "previous output").unwrap();
    std::fs::create_dir(temp.path().join("fonts-output")).unwrap();

    for command in [
        ["render", "fonts/diagram.mmd"],
        ["mmdc", "--input=fonts/diagram.mmd"],
    ] {
        let mut args = vec![
            command[0],
            command[1],
            "--font-dir",
            "./fonts",
            "--font-system",
            "off",
            "--format",
            "svg",
            "--output",
            "-",
        ];
        for output_name in ["-", "fonts-output/result.svg"] {
            args[9] = output_name;
            let output = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
            assert!(
                output.status.success(),
                "Mermaid input in a font directory and output outside it must work: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            std::fs::read_to_string(temp.path().join("fonts-output/result.svg"))
                .unwrap()
                .contains("<svg")
        );
        let output_names = [
            "fonts/existing.svg",
            "./fonts/nested/../existing.svg",
            "fonts/nested/new.svg",
            "fonts/new.svg",
        ];
        #[cfg(windows)]
        let output_names = output_names
            .into_iter()
            .chain(["FONTS/existing.svg", "FONTS/new.svg"]);
        for output_name in output_names {
            args[9] = output_name;
            let output = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
            assert!(
                !output.status.success(),
                "font directory output must be rejected"
            );
            assert!(output.stdout.is_empty());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(
                diagnostic.contains("aliases protected font directory input"),
                "expected directory protection before rendering: {diagnostic}"
            );
            assert_eq!(std::fs::read(&font_path).unwrap(), original);
            assert_eq!(
                std::fs::read_to_string(fonts.join("diagram.mmd")).unwrap(),
                source
            );
            assert_eq!(
                std::fs::read_to_string(fonts.join("existing.svg")).unwrap(),
                "previous output"
            );
            assert!(!fonts.join("new.svg").exists());
            assert!(!fonts.join("nested/new.svg").exists());
        }
    }
}

#[cfg(any(unix, windows))]
#[test]
fn explicit_relative_font_directory_symlinks_render_like_real_directories() {
    let temp = tempfile::tempdir().unwrap();
    let fonts = temp.path().join("fonts");
    std::fs::create_dir(&fonts).unwrap();
    let original = std::fs::read(font()).unwrap();
    let font_path = fonts.join("selectedfont.ttf");
    std::fs::write(&font_path, &original).unwrap();
    let alias = temp.path().join("font-view");
    #[cfg(unix)]
    std::os::unix::fs::symlink("fonts", &alias).unwrap();
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_dir("fonts", &alias) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("create directory alias: {error}");
    }
    let output_root = temp.path().join("fonts-output");
    std::fs::create_dir(&output_root).unwrap();
    let source = "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[Native font rendering: iii WWW mmm]\n";
    let formats = [
        "svg",
        #[cfg(feature = "png")]
        "png",
    ];
    for (command, input, svg_id) in [
        ("render", "-", "--svg-id"),
        ("mmdc", "--input=-", "--svgId"),
    ] {
        for format in formats {
            let mut artifacts = Vec::new();
            for (font_directory, name) in [("fonts", "real"), ("./font-view", "linked")] {
                let output_path = output_root.join(format!("{command}-{name}.{format}"));
                let output = support::run_with_stdin_in_dir(
                    &[
                        command,
                        input,
                        "--font-dir",
                        font_directory,
                        "--font-system=off",
                        "--format",
                        format,
                        "--output",
                        output_path.to_str().unwrap(),
                        svg_id,
                        "linked-font-root",
                    ],
                    source,
                    Some(temp.path()),
                );
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    output.status.success(),
                    "{command}/{format}/{font_directory}: {stderr}"
                );
                assert!(output.stdout.is_empty());
                let notices = measurement_notices(&stderr);
                assert_eq!(notices.len(), 1, "{stderr}");
                assert!(
                    notices[0].starts_with("native measurement faces: Cousine-Regular#0; context "),
                    "{stderr}"
                );
                assert!(
                    !stderr.contains("native export font resolution is incomplete"),
                    "{stderr}"
                );
                let artifact = std::fs::read(output_path).unwrap();
                if format == "png" {
                    assert!(artifact.starts_with(b"\x89PNG\r\n\x1a\n"));
                } else {
                    assert!(String::from_utf8_lossy(&artifact).contains("<svg"));
                }
                artifacts.push(artifact);
            }
            assert_eq!(
                artifacts[0], artifacts[1],
                "{command}/{format}: the directory alias must use the same font bytes"
            );
        }
    }
    assert_eq!(std::fs::read(&font_path).unwrap(), original);
    assert_eq!(
        std::fs::read(alias.join("selectedfont.ttf")).unwrap(),
        original
    );
}

#[cfg(any(unix, windows))]
#[test]
fn explicit_font_directory_protection_resolves_output_parent_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let fonts = temp.path().join("fonts");
    std::fs::create_dir(&fonts).unwrap();
    let original = std::fs::read(font()).unwrap();
    let font_path = fonts.join("selectedfont.ttf");
    std::fs::write(&font_path, &original).unwrap();
    std::fs::write(fonts.join("existing.svg"), "previous output").unwrap();
    let alias = temp.path().join("font-view");
    #[cfg(unix)]
    std::os::unix::fs::symlink("fonts", &alias).unwrap();
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_dir("fonts", &alias) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("create directory alias: {error}");
    }

    for font_directory in ["fonts", "./font-view"] {
        for output_name in [
            "fonts/existing.svg",
            "fonts/new.svg",
            "font-view/existing.svg",
            "font-view/new.svg",
        ] {
            let output = support::run_before_stdin_close_in_dir(
                &[
                    "render",
                    "-",
                    "--font-dir",
                    font_directory,
                    "--font-system=off",
                    "--format",
                    "svg",
                    "--output",
                    output_name,
                ],
                Some(temp.path()),
                std::time::Duration::from_secs(2),
            );
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(
                diagnostic.contains("aliases protected font directory input"),
                "{font_directory}/{output_name}: expected canonical directory protection: {diagnostic}"
            );
            assert_eq!(std::fs::read(&font_path).unwrap(), original);
            assert_eq!(
                std::fs::read_to_string(fonts.join("existing.svg")).unwrap(),
                "previous output"
            );
            assert!(!fonts.join("new.svg").exists());
        }
    }
}

#[test]
fn explicit_font_directories_reject_external_hard_link_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let fonts = temp.path().join("fonts");
    std::fs::create_dir_all(fonts.join("nested")).unwrap();
    let original = std::fs::read(font()).unwrap();
    let font_path = fonts.join("nested/Cousine.ttf");
    std::fs::write(&font_path, &original).unwrap();
    std::fs::hard_link(&font_path, temp.path().join("font-alias.svg")).unwrap();
    std::fs::write(temp.path().join("ordinary.svg"), "previous output").unwrap();
    std::fs::write(temp.path().join("unrelated.svg"), "unrelated contents").unwrap();
    std::fs::hard_link(
        temp.path().join("unrelated.svg"),
        temp.path().join("unrelated-alias.svg"),
    )
    .unwrap();
    let source = "flowchart LR\nA-->B\n";
    for command in [["render", "-"], ["mmdc", "--input=-"]] {
        for output_name in [
            "ordinary.svg",
            "new.svg",
            "font-alias.svg",
            "unrelated-alias.svg",
        ] {
            let args = [
                command[0],
                command[1],
                "--font-dir",
                "fonts",
                "--format",
                "svg",
                "--output",
                output_name,
            ];
            if matches!(output_name, "ordinary.svg" | "new.svg") {
                let output = support::run_with_stdin_in_dir(&args, source, Some(temp.path()));
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                continue;
            }
            let output = support::run_before_stdin_close_in_dir(
                &args,
                Some(temp.path()),
                std::time::Duration::from_secs(2),
            );
            assert!(
                !output.status.success(),
                "hard-linked output must be rejected"
            );
            assert!(output.stdout.is_empty());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(
                diagnostic.contains("multiple hard links")
                    && diagnostic.contains("font directory input"),
                "expected bounded hard-link protection: {diagnostic}"
            );
            assert_eq!(std::fs::read(&font_path).unwrap(), original);
            assert_eq!(
                std::fs::read(temp.path().join("font-alias.svg")).unwrap(),
                original
            );
            assert!(
                same_file::is_same_file(&font_path, temp.path().join("font-alias.svg")).unwrap()
            );
            assert_eq!(
                std::fs::read_to_string(temp.path().join("unrelated-alias.svg")).unwrap(),
                "unrelated contents"
            );
        }
    }
}

#[test]
fn contradictory_or_unavailable_font_modes_fail_without_output() {
    let font = font();
    let mut failures = vec![
        vec!["--font-file", &font, "--font-measurement", "deterministic"],
        vec!["--font-measurement", "native", "--font-system", "off"],
        vec!["--font-measurement", "deterministic", "--font-system", "on"],
        vec!["--font-file", "missing-native-font.ttf"],
    ];
    if !cfg!(feature = "system-fonts") {
        failures.extend([
            vec!["--font-measurement", "native"],
            vec!["--font-system", "on"],
        ]);
    }
    if cfg!(feature = "platform-system-font-query") {
        failures.extend([
            vec![
                "--font-measurement",
                "deterministic",
                "--font-system",
                "platform",
            ],
            vec![
                "--font-measurement",
                "deterministic",
                "--font-system",
                "bounded",
            ],
        ]);
    } else {
        failures.push(vec!["--font-system", "platform"]);
    }
    for extra in failures {
        let mut args = vec!["mmdc", "-i", "-", "-o", "-"];
        args.extend(extra);
        let output = run_with_stdin(&args, "flowchart LR\nA-->B\n");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[cfg(feature = "platform-system-font-query")]
mod platform_fonts {
    use merman::svg::export::{NativeFontContext, NativeFontOptions};

    pub(super) const FONTCONFIG_UNAVAILABLE: &str =
        "platform system font service Fontconfig is unavailable";

    pub(super) fn context_with_cousine() -> Option<NativeFontContext> {
        // An explicit face keeps an empty system catalog usable for this availability check.
        let options = NativeFontOptions::default()
            .with_font_file(super::font())
            .with_system_fonts(true);
        match options.load_platform(&merman::OperationControl::new()) {
            Ok(context) => Some(context),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            Err(merman::svg::export::NativeFontError::PlatformDiscoveryUnavailable {
                service: "Fontconfig",
            }) => None,
            Err(error) => panic!("loading the platform font context with Cousine failed: {error}"),
        }
    }

    #[track_caller]
    pub(super) fn assert_error(output: &std::process::Output, mode: &str, expected: &str) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(3), "{mode}: {stderr}");
        assert!(output.stdout.is_empty(), "{mode} wrote to stdout");
        let error = stderr.lines().next_back().unwrap_or_default();
        assert!(
            error.contains(expected),
            "{mode}: expected {expected:?}, got {stderr}"
        );
    }
}

#[cfg(feature = "platform-system-font-query")]
#[test]
fn explicit_platform_query_uses_shared_context_and_respects_explicit_fonts() {
    let font = font();
    let source = "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[Native measurement]\n";
    let render = |system| {
        run_with_stdin(
            &[
                "render",
                "-",
                "--output=-",
                "--svg-id=platform-font-test",
                "--font-file",
                &font,
                "--font-system",
                system,
            ],
            source,
        )
    };
    let assert_cousine = |output: &std::process::Output, identity: &str| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        assert!(!output.stdout.is_empty());
        let expected = format!("native measurement faces: Cousine-Regular#0; context {identity}");
        assert_eq!(measurement_notices(&stderr), [expected.as_str()]);
    };
    let closed_context = merman::svg::export::NativeFontOptions::default()
        .with_font_file(&font)
        .load(&merman::OperationControl::new())
        .unwrap();
    let closed = render("off");
    assert_cousine(&closed, closed_context.identity());

    let platform = render("platform");
    let Some(platform_context) = platform_fonts::context_with_cousine() else {
        platform_fonts::assert_error(
            &platform,
            "platform",
            platform_fonts::FONTCONFIG_UNAVAILABLE,
        );
        return;
    };
    assert_cousine(&platform, platform_context.identity());
    assert_ne!(platform_context.identity(), closed_context.identity());
    assert_eq!(platform.stdout, closed.stdout);
}

#[cfg(all(
    feature = "png",
    any(
        all(
            windows,
            any(
                feature = "platform-system-font-default",
                feature = "windows-platform-font-default"
            )
        ),
        all(target_os = "linux", feature = "platform-system-font-query")
    )
))]
mod large_font_inventory {
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    use write_fonts::read::FontRef;
    use write_fonts::tables::cmap::{
        Cmap, CmapSubtable, EncodingRecord, PlatformId, SequentialMapGroup,
    };
    use write_fonts::tables::name::{Name, NameRecord};
    use write_fonts::types::{NameId, Tag};
    use write_fonts::{FontBuilder, dump_table};

    fn populate_inventory(directory: &Path) {
        let seed = fs::read(
            super::repo_root().join("crates/merman-export/tests/fonts/MermanProbeBase.ttf"),
        )
        .unwrap();
        let seed = FontRef::new(&seed).unwrap();
        // Distinct faces avoid catalog deduplication. Each cmap fits the per-face limit,
        // and the sum exceeds the 16 MiB discovery limit.
        let groups = (0x20000..0x48000)
            .step_by(2)
            .map(|codepoint| SequentialMapGroup::new(codepoint, codepoint, 1))
            .collect();
        let cmap = dump_table(&Cmap::new(vec![EncodingRecord::new(
            PlatformId::Windows,
            10,
            CmapSubtable::format_12(0, groups),
        )]))
        .unwrap();
        for index in 0..18 {
            let family = format!("MermanInventory{index:02}");
            let name = Name::new(
                [
                    NameId::FAMILY_NAME,
                    NameId::FULL_NAME,
                    NameId::POSTSCRIPT_NAME,
                ]
                .into_iter()
                .map(|id| NameRecord::new(3, 1, 0x409, id, family.clone().into()))
                .collect(),
            );
            let mut builder = FontBuilder::new();
            builder.add_raw(Tag::new(b"cmap"), cmap.as_slice());
            builder.add_table(&name).unwrap();
            builder.copy_missing_tables(seed.clone());
            fs::write(directory.join(format!("{family}.ttf")), builder.build()).unwrap();
        }
    }

    #[test]
    fn platform_png_succeeds_when_bounded_discovery_exhausts_cmap_budget() {
        let temp = tempfile::tempdir().unwrap();
        let fonts = temp
            .path()
            .join(if cfg!(windows) { "Fonts" } else { "fonts" });
        fs::create_dir(&fonts).unwrap();
        populate_inventory(&fonts);
        let cjk_font =
            super::repo_root().join("crates/merman-export/tests/fonts/NotoSansSC-subset.ttf");

        let config_contents = if cfg!(windows) {
            r#"{"look":"classic"}"#
        } else {
            r#"{"look":"classic","fontFamily":"Cousine","htmlLabels":false}"#
        };
        let config = temp.path().join("classic.json");
        fs::write(&config, config_contents).unwrap();

        let fontconfig = temp.path().join("fonts.conf");
        if cfg!(target_os = "linux") {
            // Give Fontconfig access to the same large inventory that bounded discovery scans.
            fs::copy(super::font(), fonts.join("Cousine-Regular.ttf")).unwrap();
            fs::copy(&cjk_font, fonts.join("NotoSansSC-subset.ttf")).unwrap();
            fs::write(
                &fontconfig,
                r#"<?xml version="1.0"?>
<fontconfig>
  <dir prefix="xdg">fonts</dir>
  <cachedir prefix="xdg">fontconfig</cachedir>
  <alias><family>sans-serif</family><prefer><family>Cousine</family></prefer></alias>
</fontconfig>"#,
            )
            .unwrap();
        }

        let binary = std::env::var_os("MERMAN_TEST_FONT_INVENTORY_CLI_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| assert_cmd::cargo_bin!("merman-cli").to_path_buf());
        let source = if cfg!(target_os = "linux") {
            "flowchart LR\nA[Draft 中] --> B[Review]\n"
        } else {
            "flowchart LR\nA[Draft] --> B[Review]\n"
        };
        let command_for_catalog = || {
            let mut command = assert_cmd::Command::new(&binary);
            if cfg!(windows) {
                // DirectWrite queries the system catalog independently of the scan roots.
                command
                    .env("WINDIR", temp.path())
                    .env("LOCALAPPDATA", temp.path());
            } else {
                // Newer Fontconfig also reads XDG_DATA_DIRS for a dir with prefix="xdg".
                command
                    .env("HOME", temp.path())
                    .env("XDG_DATA_HOME", temp.path())
                    .env("XDG_DATA_DIRS", temp.path())
                    .env("XDG_CACHE_HOME", temp.path())
                    .env("FONTCONFIG_FILE", &fontconfig);
            }
            command
        };
        let render = |file: &str, extra: &[&str]| {
            command_for_catalog()
                // mmdc suppresses export warnings when the destination is stdout.
                .args(["-i", "-", "-o"])
                .arg(temp.path().join(file))
                .args(["-e", "png", "-t", "default"])
                .args(["-b", "#FFFFFF", "-s", "2", "-c"])
                .arg(&config)
                .args(extra)
                .write_stdin(source)
                .output()
                .unwrap()
        };

        let bounded = render("bounded.png", &["--font-system=bounded"]);
        let stderr = String::from_utf8_lossy(&bounded.stderr);
        assert_eq!(bounded.status.code(), Some(3), "{stderr}");
        assert!(
            stderr.contains("native font limit system-cmap-bytes exceeded"),
            "{stderr}"
        );
        assert!(!stderr.contains("--font-max-bytes"), "{stderr}");
        assert!(bounded.stdout.is_empty());
        assert!(!temp.path().join("bounded.png").exists());

        let missing_glyph_warning = "native export font resolution is incomplete";
        let render_complete_png = |file: &str, extra: &[&str]| {
            let output = render(file, extra);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{file}: {stderr}");
            assert!(!stderr.contains(missing_glyph_warning), "{file}: {stderr}");
            fs::read(temp.path().join(file)).unwrap()
        };
        let platform = render_complete_png("platform.png", &["--font-system=platform"]);
        assert!(platform.starts_with(b"\x89PNG\r\n\x1a\n"));
        if cfg!(target_os = "linux") {
            let latin = super::font();
            let explicit_fonts = render_complete_png(
                "explicit.png",
                &[
                    "--font-file",
                    &latin,
                    "--font-file",
                    cjk_font.to_str().unwrap(),
                ],
            );
            assert_eq!(platform, explicit_fonts);

            let incomplete = render("latin-only.png", &["--font-file", &latin]);
            let stderr = String::from_utf8_lossy(&incomplete.stderr);
            assert!(incomplete.status.success(), "{stderr}");
            assert!(stderr.contains(missing_glyph_warning), "{stderr}");
            let latin_only = fs::read(temp.path().join("latin-only.png")).unwrap();
            assert!(latin_only.starts_with(b"\x89PNG\r\n\x1a\n"));
            assert!(
                latin_only != platform,
                "the missing Han glyph must affect the PNG"
            );
        }
        let platform_default = cfg!(any(windows, feature = "platform-system-font-default"));
        if platform_default {
            let default = render_complete_png("default.png", &[]);
            assert_eq!(platform, default);
        }

        let raw = |source: &str, extra: &[&str]| {
            command_for_catalog()
                .args(["render", "-", "--input-kind=svg", "--format=png"])
                .args(extra)
                .write_stdin(source)
                .output()
                .unwrap()
        };
        let paths = raw(super::RAW_PATH_SVG, &["--font-system=bounded"]);
        assert!(paths.status.success(), "{paths:?}");
        assert!(paths.stdout.starts_with(b"\x89PNG\r\n\x1a\n"));
        let automatic_paths = raw(super::RAW_PATH_SVG, &[]);
        assert!(automatic_paths.status.success(), "{automatic_paths:?}");
        assert_eq!(automatic_paths.stdout, paths.stdout);

        if cfg!(target_os = "linux") {
            let bounded = raw(super::RAW_FONT_SVG, &["--font-system=bounded"]);
            super::platform_fonts::assert_error(
                &bounded,
                "raw SVG with bounded discovery",
                "native font limit system-cmap-bytes exceeded",
            );
            let platform = raw(super::RAW_FONT_SVG, &["--font-system=platform"]);
            assert!(platform.status.success(), "{platform:?}");
            let explicit = raw(super::RAW_FONT_SVG, &["--font-file", &super::font()]);
            assert!(explicit.status.success(), "{explicit:?}");
            assert_eq!(platform.stdout, explicit.stdout);

            let budget = raw(super::RAW_FONT_SVG, &["--font-max-bytes=1"]);
            assert!(!budget.status.success(), "{budget:?}");
            assert!(budget.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&budget.stderr);
            if platform_default {
                let automatic = raw(super::RAW_FONT_SVG, &[]);
                assert!(automatic.status.success(), "{automatic:?}");
                assert_eq!(automatic.stdout, explicit.stdout);
                assert!(stderr.contains("--font-max-bytes "), "{stderr}");
            } else {
                super::platform_fonts::assert_error(
                    &raw(super::RAW_FONT_SVG, &[]),
                    "raw SVG with automatic bounded discovery",
                    "native font limit system-cmap-bytes exceeded",
                );
                assert!(
                    stderr.contains("native font limit system-cmap-bytes exceeded"),
                    "{stderr}"
                );
            }
        }
    }
}

#[cfg(all(target_os = "linux", feature = "platform-system-font-query"))]
fn check_fontconfig_fallback(render_option: &str, raw_svg: bool) {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("empty")).unwrap();
    let config = temp.path().join("fonts.conf");
    std::fs::write(
        &config,
        r#"<?xml version="1.0"?>
<fontconfig>
  <dir prefix="xdg">empty</dir>
  <cachedir prefix="xdg">fontconfig</cachedir>
</fontconfig>"#,
    )
    .unwrap();
    // Bounded discovery can find Cousine even on a host with no installed fonts.
    let fonts = temp.path().join("fonts");
    std::fs::create_dir(&fonts).unwrap();
    std::fs::copy(font(), fonts.join("Cousine-Regular.ttf")).unwrap();
    let (filename, source) = if raw_svg {
        ("input.svg", RAW_FONT_SVG)
    } else {
        (
            "input.mmd",
            "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[Empty font policy]\n",
        )
    };
    let input = temp.path().join(filename);
    std::fs::write(&input, source).unwrap();
    // Give only the child process an unloadable Fontconfig, keeping its other library paths.
    std::fs::write(
        temp.path().join("libfontconfig.so.1"),
        b"unloadable test library",
    )
    .unwrap();
    let mut library_dirs = vec![temp.path().to_owned()];
    if let Some(paths) = std::env::var_os("LD_LIBRARY_PATH") {
        library_dirs.extend(std::env::split_paths(&paths));
    }
    let unavailable_path = std::env::join_paths(library_dirs).unwrap();
    let render = |mode: Option<&str>, library_path: Option<&std::ffi::OsStr>| {
        let mut command = std::process::Command::new(assert_cmd::cargo_bin!("merman-cli"));
        command
            .arg("render")
            .arg(&input)
            .args(["--output=-", render_option]);
        if !raw_svg {
            command.arg("--svg-id=fontconfig-fallback");
        }
        if let Some(mode) = mode {
            command.args(["--font-system", mode]);
        }
        if let Some(path) = library_path {
            command.env("LD_LIBRARY_PATH", path);
        }
        command
            .env("FONTCONFIG_FILE", &config)
            .env("HOME", temp.path())
            .env("XDG_DATA_HOME", temp.path())
            .env("XDG_DATA_DIRS", temp.path())
            .env("XDG_CACHE_HOME", temp.path())
            .output()
            .unwrap()
    };
    let bounded = render(Some("bounded"), None);
    let bounded_stderr = String::from_utf8_lossy(&bounded.stderr);
    assert!(bounded.status.success(), "{bounded_stderr}");
    assert!(!bounded.stdout.is_empty());
    assert_eq!(
        measurement_notices(&bounded_stderr).is_empty(),
        raw_svg,
        "{bounded_stderr}"
    );
    for (library_path, available) in [
        (None, platform_fonts::context_with_cousine().is_some()),
        (Some(unavailable_path.as_os_str()), false),
    ] {
        let platform_error = if available {
            "no usable font faces"
        } else {
            platform_fonts::FONTCONFIG_UNAVAILABLE
        };
        platform_fonts::assert_error(
            &render(Some("platform"), library_path),
            "platform",
            platform_error,
        );
        for mode in [None, Some("on")] {
            let output = render(mode, library_path);
            let name = mode.unwrap_or("default");
            if available && cfg!(feature = "platform-system-font-default") {
                platform_fonts::assert_error(&output, name, platform_error);
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    output.status.success(),
                    "{name}, {platform_error}: {stderr}"
                );
                assert_eq!(output.stdout, bounded.stdout, "{name}, {platform_error}");
            }
        }
    }
}

#[cfg(all(target_os = "linux", feature = "platform-system-font-query"))]
#[test]
fn fontconfig_availability_controls_automatic_fallback() {
    let option = if cfg!(feature = "png") {
        "--format=png"
    } else {
        "--font-measurement=native"
    };
    check_fontconfig_fallback(option, false);
}

#[cfg(all(
    target_os = "linux",
    feature = "platform-system-font-query",
    feature = "png"
))]
#[test]
fn empty_fontconfig_policy_does_not_trigger_bounded_fallback() {
    check_fontconfig_fallback("--font-measurement=native", false);
}

#[cfg(all(
    target_os = "linux",
    feature = "platform-system-font-query",
    feature = "png"
))]
#[test]
fn fontconfig_availability_controls_raw_svg_automatic_fallback() {
    check_fontconfig_fallback("--format=png", true);
}

#[cfg(all(
    target_os = "linux",
    feature = "platform-system-font-query",
    feature = "png"
))]
#[test]
fn raw_svg_platform_font_selection_uses_inherited_and_overridden_language() {
    if platform_fonts::context_with_cousine().is_none() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let fonts = root.path().join("fonts");
    std::fs::create_dir(&fonts).unwrap();
    let config = root.path().join("fonts.conf");
    std::fs::write(
        &config,
        r#"<fontconfig>
          <dir prefix="xdg">fonts</dir><cachedir prefix="xdg">cache</cachedir>
          <alias binding="strong"><family>sans-serif</family><prefer><family>Cousine</family></prefer></alias>
          <match target="pattern"><test name="lang" compare="contains"><string>ja</string></test>
          <edit name="family" mode="prepend_first" binding="strong"><string>Noto Sans</string></edit></match>
        </fontconfig>"#,
    )
    .unwrap();
    let latin = font();
    let noto = repo_root().join("crates/merman-export/tests/fonts/NotoSans-Latin-subset.ttf");
    std::fs::copy(&latin, fonts.join("Cousine-Regular.ttf")).unwrap();
    std::fs::copy(&noto, fonts.join("NotoSans-Latin-subset.ttf")).unwrap();

    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="180" height="80" xml:lang="en"><text x="10" y="40" font-size="24" font-family="sans-serif">AW<tspan x="90" xml:lang="ja">AW</tspan></text></svg>"#;
    let platform = assert_cmd::Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .args([
            "render",
            "-",
            "--input-kind=svg",
            "--format=png",
            "--font-system=platform",
        ])
        .env("FONTCONFIG_FILE", &config)
        .env("XDG_DATA_HOME", root.path())
        .env("XDG_DATA_DIRS", root.path())
        .env("XDG_CACHE_HOME", root.path())
        .env("FC_LANG", "en")
        .write_stdin(source)
        .output()
        .unwrap();
    let explicit_source = source.replace("sans-serif", "Cousine").replace(
        r#"xml:lang="ja""#,
        r#"xml:lang="ja" font-family="Noto Sans""#,
    );
    let explicit = raw_conversion(
        "png",
        &explicit_source,
        &["--font-file", &latin, "--font-file", noto.to_str().unwrap()],
    );
    assert!(platform.status.success(), "{platform:?}");
    assert!(explicit.status.success(), "{explicit:?}");
    assert_eq!(platform.stdout, explicit.stdout);
}

#[cfg(all(
    target_os = "linux",
    feature = "platform-system-font-query",
    feature = "png"
))]
#[test]
fn fontconfig_keeps_preferred_aliases_and_usable_coverage_alternatives() {
    use std::{fs, process::Command};

    let temp = tempfile::tempdir().unwrap();
    let fonts = temp.path().join("fonts");
    fs::create_dir(&fonts).unwrap();
    let fixtures = repo_root().join("crates/merman-export/tests/fonts");
    for file in [
        "Cousine-Regular.ttf",
        "MermanPictorialTest.ttf",
        "NotoSans-Latin-subset.ttf",
    ] {
        fs::copy(fixtures.join(file), fonts.join(file)).unwrap();
    }
    let config = temp.path().join("fonts.conf");
    fs::write(
        &config,
        r#"<?xml version="1.0"?>
<fontconfig>
  <dir prefix="xdg">fonts</dir>
  <cachedir prefix="xdg">cache</cachedir>
  <alias binding="strong">
    <family>Office Body</family>
    <prefer><family>Cousine</family></prefer>
  </alias>
  <alias binding="strong">
    <family>sans-serif</family>
    <prefer><family>Merman Pictorial Test</family><family>Cousine</family></prefer>
  </alias>
  <alias binding="strong">
    <family>serif</family>
    <prefer><family>Merman Pictorial Test</family><family>Cousine</family></prefer>
  </alias>
  <alias>
    <family>Missing Body</family>
    <default><family>Cousine</family></default>
  </alias>
  <alias binding="weak">
    <family>Weak Body</family>
    <prefer><family>Cousine</family></prefer>
  </alias>
  <match target="scan">
    <test name="family"><string>Cousine</string></test>
    <edit name="family" mode="prepend"><string>Office Scan</string></edit>
    <edit name="postscriptname" mode="assign"><string>Cousine-Regular</string></edit>
  </match>
  <match target="scan">
    <test name="family" compare="not_eq"><string>Merman Pictorial Test</string></test>
    <edit name="charset" mode="assign"><charset><int>65</int></charset></edit>
  </match>
</fontconfig>"#,
    )
    .unwrap();

    let input = temp.path().join("input.svg");
    let run = |family: &str| {
        fs::write(
            &input,
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="50"><text x="5" y="35" font-size="30" font-family="{family}">A</text></svg>"#
            ),
        )
        .unwrap();
        Command::new(assert_cmd::cargo_bin!("merman-cli"))
            .arg("render")
            .arg(&input)
            .args([
                "--input-kind=svg",
                "--format=png",
                "--output=-",
                "--font-system=platform",
            ])
            .env("XDG_DATA_HOME", temp.path())
            .env("XDG_DATA_DIRS", temp.path())
            .env("XDG_CACHE_HOME", temp.path())
            .env("FONTCONFIG_FILE", &config)
            .output()
            .unwrap()
    };
    let render = |family| {
        let result = run(family);
        assert!(
            result.status.success(),
            "{family}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.starts_with(b"\x89PNG\r\n\x1a\n"));
        result.stdout
    };

    let cousine = render("Cousine");
    let noto = render("Noto Sans");
    assert_ne!(noto, cousine);
    assert_eq!(render("Office Scan, Noto Sans"), cousine);
    assert!(
        render("Office Body, Noto Sans") == cousine,
        "the preferred alias must win"
    );
    assert!(
        render("Missing Body, Noto Sans") == noto,
        "a weak default must not shadow the next authored family"
    );
    assert!(
        render("Weak Body, Noto Sans") == noto,
        "native matching keeps a weak preference below a strong authored name"
    );
    assert!(
        render("sans-serif") == cousine,
        "the pictorial face must leave Cousine available"
    );

    // The scan rule retains the original PostScript name even if Fontconfig rescans the file.
    fs::copy(
        fixtures.join("Cousine-Bold.ttf"),
        fonts.join("Cousine-Regular.ttf"),
    )
    .unwrap();
    let replaced = run("Office Scan");
    assert!(!replaced.status.success());
    assert!(replaced.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&replaced.stderr);
    assert!(
        diagnostic.contains("font properties changed after platform lookup"),
        "{diagnostic}"
    );
}

#[test]
fn capabilities_disclose_shared_coverage_and_cli_discovery_policy() {
    let output = run_with_stdin(&["capabilities", "--json"], "");
    assert!(output.status.success());
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let native = &document["native_fonts"];
    assert_eq!(
        native["measurement_operations"],
        serde_json::json!(
            merman::svg::TextMeasurementOperation::ALL
                .into_iter()
                .map(merman::svg::TextMeasurementOperation::external_name)
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(
        native["measurement_phases"],
        serde_json::json!(["layout", "wrap", "svg-bbox", "computed-length"])
    );
    assert_eq!(native["unsupported_operations"], "deterministic-fallback");
    assert_eq!(native["missing_glyphs"], "deterministic-fallback");
    assert_eq!(native["browser_pixel_parity"], false);
    assert_eq!(
        native["system_discovery_runtime_available"],
        cfg!(feature = "system-fonts")
    );
    assert_eq!(native["default_measurement"], "deterministic");
    assert_eq!(native["default_font_source"], "none");
    assert_eq!(native["default_system_discovery"], "disabled");
    assert_eq!(native["default_policy_scope"], "mermaid-svg-input");
    let platform_default = cfg!(any(
        feature = "platform-system-font-default",
        all(windows, feature = "windows-platform-font-default")
    ));
    let expected_encoded = if platform_default {
        serde_json::json!({
            "measurement": "native",
            "font_source": "platform-managed-system-query",
            "system_discovery": "platform-initialization-then-on-demand-families",
        })
    } else if cfg!(feature = "system-fonts") {
        serde_json::json!({
            "measurement": "native",
            "font_source": "bounded-system-discovery",
            "system_discovery": "before-layout",
        })
    } else {
        serde_json::json!({
            "measurement": "deterministic",
            "font_source": "none",
            "system_discovery": "disabled",
        })
    };
    assert_eq!(native["encoded_output_defaults"], expected_encoded);
    assert_eq!(
        native["legacy_export_paths"],
        if cfg!(feature = "system-fonts") {
            "deterministic-mermaid-uses-output-environment-contract"
        } else {
            "raw-svg-without-font-options-and-deterministic-mermaid-use-output-environment-contract"
        }
    );
    if cfg!(feature = "platform-system-font-query") {
        assert_eq!(
            native["platform_system_query"],
            serde_json::json!({
                "cli_option": "--font-system platform",
                "system_discovery": "platform-initialization-then-on-demand-families",
                "metadata_resources_bounded": false,
                "context_identity": "policy-and-initial-font-bytes",
                "system_fallback_values": ["on", "bounded", "platform"],
            })
        );
    } else {
        assert!(native.get("platform_system_query").is_none());
    }
    assert_eq!(native["selected_font_bytes_bounded"], true);
    assert_eq!(
        native["system_resources_bounded"],
        cfg!(feature = "system-fonts") && !platform_default
    );
    assert_eq!(
        native["explicit_font_sources"],
        if cfg!(feature = "platform-system-font-query") {
            "closed-set-unless-system-fallback-requested"
        } else {
            "closed-set-unless-font-system-on"
        }
    );
    assert_eq!(native["embeds_fonts_in_svg"], false);
    assert_eq!(
        native["system_discovery_compiled"],
        merman::svg::export::system_font_discovery_available()
    );
}

#[test]
fn native_measurement_rejects_post_layout_css_without_output() {
    let css = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(css.path(), ".flowchartTitleText { font-size: 200px; }").unwrap();
    let path = css.path().to_string_lossy();
    let output = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--font-file",
            &font(),
            "--cssFile",
            &path,
        ],
        "flowchart LR\nA-->B\n",
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not support --css-file"));
    let config = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        config.path(),
        r#"{"themeCSS":".flowchartTitleText { font-size: 200px; }"}"#,
    )
    .unwrap();
    let config_path = config.path().to_string_lossy();
    let output = run_with_stdin(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--font-file",
            &font(),
            "--configFile",
            &config_path,
        ],
        "flowchart LR\nA-->B\n",
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not support themeCSS"));
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn default_encoded_output_uses_native_layout_and_retains_explicit_deterministic() {
    let source = "---\nconfig:\n  fontFamily: monospace\n  htmlLabels: false\n  flowchart:\n    minNodeWidth: 0\n    wrappingWidth: 1000\n---\nflowchart LR\nA[iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii]\n";
    let formats = [
        #[cfg(feature = "png")]
        "png",
        #[cfg(feature = "jpeg")]
        "jpeg",
        #[cfg(feature = "pdf")]
        "pdf",
    ];
    for command in ["render", "mmdc"] {
        for format in formats {
            if command == "mmdc" && format == "jpeg" {
                continue;
            }
            let args = if command == "render" {
                vec!["render", "-", "-f", format, "-o", "-"]
            } else {
                vec!["mmdc", "-i", "-", "-e", format, "-o", "-"]
            };
            let implicit = run_with_stdin(&args, source);
            let mut native_args = args.clone();
            native_args.extend(["--font-measurement", "native"]);
            let native = run_with_stdin(&native_args, source);
            let mut deterministic_args = args;
            deterministic_args.extend(["--font-measurement", "deterministic"]);
            let deterministic = run_with_stdin(&deterministic_args, source);
            for output in [&implicit, &native, &deterministic] {
                assert!(
                    output.status.success(),
                    "{command}/{format}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(!output.stdout.is_empty());
            }
            assert_eq!(implicit.stdout, native.stdout, "{command}/{format}");
            assert_ne!(implicit.stdout, deterministic.stdout, "{command}/{format}");
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn default_encoded_native_measurement_enforces_its_budget_and_font_policy() {
    let format = if cfg!(feature = "png") {
        "png"
    } else if cfg!(feature = "jpeg") {
        "jpeg"
    } else {
        "pdf"
    };
    let source = "flowchart LR\nA[Measure this label] --> B[Destination]\n";
    let args = ["render", "-", "-f", format, "-o", "-"];
    let mut limited = args.to_vec();
    limited.push("--font-max-measurements=1");
    let output = run_with_stdin(&limited, source);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("native_font_measurements"), "{stderr}");

    let mut no_fonts = args.to_vec();
    no_fonts.push("--font-system=off");
    let output = run_with_stdin(&no_fonts, source);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("leaves no fonts"));
}

#[cfg(feature = "png")]
#[test]
fn default_encoded_native_measurement_requires_explicit_compatibility_for_css() {
    let css = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(css.path(), ".nodeLabel { letter-spacing: 2px; }").unwrap();
    let path = css.path().to_str().unwrap();
    let mut args = vec!["render", "-", "-f", "png", "-o", "-", "--css-file", path];
    let source = "flowchart LR\nA[Styled label]\n";
    let rejected = run_with_stdin(&args, source);
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert_eq!(rejected.status.code(), Some(2), "{stderr}");
    assert!(rejected.stdout.is_empty());
    assert!(
        stderr.contains("--font-measurement deterministic"),
        "{stderr}"
    );
    args.extend(["--font-measurement", "deterministic"]);
    let compatible = run_with_stdin(&args, source);
    assert!(
        compatible.status.success(),
        "{}",
        String::from_utf8_lossy(&compatible.stderr)
    );
    assert!(compatible.stdout.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[cfg(all(feature = "png", feature = "markdown"))]
#[test]
fn batch_default_encoded_output_matches_explicit_native_measurement() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("input.md"),
        "```mermaid\nflowchart LR\nA[First label] --> B[Second label]\n```\n",
    )
    .unwrap();
    for native in [false, true] {
        let destination = if native { "native" } else { "implicit" };
        let mut args = vec![
            "batch",
            "input.md",
            "--format=png",
            "--output-dir",
            destination,
        ];
        if native {
            args.push("--font-measurement=native");
        }
        let output = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let images = |directory: &str| {
        let mut entries = std::fs::read_dir(temp.path().join(directory))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
            .map(|path| {
                (
                    path.file_name().unwrap().to_owned(),
                    std::fs::read(&path).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    };
    let implicit = images("implicit");
    assert_eq!(implicit.len(), 1);
    assert_eq!(implicit, images("native"));
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn native_mermaid_binary_exports_paint_with_the_font_used_for_layout() {
    let source = "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[Native font painting: iii WWW mmm]\n";
    let font = font();
    let context = merman::svg::export::NativeFontOptions::default()
        .with_font_file(&font)
        .load(&merman::OperationControl::new())
        .unwrap();
    let output = merman::Renderer::new()
        .render(merman::RenderRequest::document(
            source,
            merman::OperationControl::new(),
            merman::SvgRequest {
                environment: merman::SvgEnvironment::deterministic()
                    .with_native_font_context(context.clone())
                    .unwrap(),
                options: merman::svg::SvgRenderOptions {
                    diagram_id: Some("native-export".to_owned()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .unwrap();
    let merman::RenderOutput::Document(Some(document)) = output else {
        panic!("expected a completed native-font document");
    };
    let formats = [
        #[cfg(feature = "png")]
        "png",
        #[cfg(feature = "jpeg")]
        "jpeg",
        #[cfg(feature = "pdf")]
        "pdf",
    ];
    for format in formats {
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                format,
                "--svg-id",
                "native-export",
                "--font-file",
                &font,
                "--font-system",
                "off",
            ],
            source,
        );
        assert!(
            output.status.success(),
            "{format}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected_notice = format!(
            "native measurement faces: Cousine-Regular#0; context {}",
            context.identity()
        );
        assert_eq!(
            measurement_notices(&stderr),
            [expected_notice.as_str()],
            "{format}: {stderr}"
        );
        let (expected, fonts) = match format {
            #[cfg(feature = "png")]
            "png" => {
                let options = merman::svg::export::RasterOptions::default()
                    .with_font_context(context.clone());
                let (bytes, report) =
                    merman::svg::export::svg_to_png_with_report(document.sealed_svg(), &options)
                        .unwrap();
                (bytes, report.fonts())
            }
            #[cfg(feature = "jpeg")]
            "jpeg" => {
                let options = merman::svg::export::RasterOptions::default()
                    .with_font_context(context.clone());
                let (bytes, report) =
                    merman::svg::export::svg_to_jpeg_with_report(document.sealed_svg(), &options)
                        .unwrap();
                (bytes, report.fonts())
            }
            #[cfg(feature = "pdf")]
            "pdf" => {
                let options =
                    merman::svg::export::PdfOptions::default().with_font_context(context.clone());
                let (bytes, report) =
                    merman::svg::export::svg_to_pdf_with_report(document.sealed_svg(), &options)
                        .unwrap();
                (bytes, report.fonts())
            }
            _ => unreachable!("only compiled codecs are tested"),
        };
        assert_eq!(
            fonts.source_mode(),
            merman::svg::export::ExportFontMode::NativeContext
        );
        assert!(
            !fonts.unresolved_font_request(),
            "fixture must use the selected font"
        );
        assert_eq!(
            output.stdout, expected,
            "{format} must paint the same sealed scene with the selected font"
        );
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn raw_svg_fonts_are_conversion_only_and_each_compiled_codec_works() {
    let font = font();
    let blank_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"/>"#;
    let formats = [
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n".as_slice()),
        #[cfg(feature = "jpeg")]
        ("jpeg", b"\xff\xd8".as_slice()),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF".as_slice()),
    ];
    for (format, magic) in formats {
        let paths = raw_conversion(format, RAW_PATH_SVG, &["--font-system=off"]);
        assert!(paths.status.success(), "{format}: {paths:?}");
        assert!(paths.stdout.starts_with(magic), "{format}");
        let blank = raw_conversion(format, blank_svg, &["--font-system=off"]);
        assert!(blank.status.success(), "{format}: {blank:?}");
        assert!(blank.stdout.starts_with(magic), "{format}");

        for input in [RAW_FONT_SVG, RAW_NESTED_FONT_SVG, RAW_TREF_FONT_SVG] {
            let no_fonts = raw_conversion(format, input, &["--font-system=off"]);
            let stderr = String::from_utf8_lossy(&no_fonts.stderr);
            assert_eq!(no_fonts.status.code(), Some(2), "{format}: {stderr}");
            assert!(no_fonts.stdout.is_empty());
            assert!(stderr.contains("--font-system off"), "{format}: {stderr}");
            assert!(
                stderr.contains("--font-file or --font-dir"),
                "{format}: {stderr}"
            );

            let automatic = raw_conversion(format, input, &["--font-file", &font]);
            let closed =
                raw_conversion(format, input, &["--font-file", &font, "--font-system=off"]);
            for output in [&automatic, &closed] {
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(output.status.success(), "{format}: {stderr}");
                assert!(output.stdout.starts_with(magic), "{format}");
                assert!(
                    stderr.contains("raw SVG uses the selected font context for painting"),
                    "{format}: {stderr}"
                );
                assert!(
                    !stderr.contains("font resolution is incomplete"),
                    "{format}: {stderr}"
                );
                assert!(
                    measurement_notices(&stderr).is_empty(),
                    "{format}: {stderr}"
                );
            }
            assert_eq!(automatic.stdout, closed.stdout, "{format}");
            assert_ne!(closed.stdout, blank.stdout, "{format}: text must paint");
            if input == RAW_TREF_FONT_SVG {
                let direct = raw_conversion(
                    format,
                    RAW_FONT_SVG,
                    &["--font-file", &font, "--font-system=off"],
                );
                assert!(direct.status.success(), "{format}: {direct:?}");
                assert!(
                    direct.stdout == closed.stdout,
                    "{format}: href text must paint identically"
                );
            }
        }

        #[cfg(all(
            windows,
            any(
                feature = "platform-system-font-default",
                feature = "windows-platform-font-default"
            )
        ))]
        {
            let direct = RAW_FONT_SVG.replace("Cousine", "sans-serif");
            let direct = raw_conversion(format, &direct, &[]);
            assert!(direct.status.success(), "{format}: {direct:?}");
            for input in [RAW_NESTED_FONT_SVG, RAW_TREF_FONT_SVG] {
                let system_svg = input.replace("Cousine", "sans-serif");
                let default = raw_conversion(format, &system_svg, &[]);
                let platform = raw_conversion(format, &system_svg, &["--font-system=platform"]);
                for output in [&default, &platform] {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    assert!(output.status.success(), "{format}: {stderr}");
                    assert!(output.stdout.starts_with(magic), "{format}");
                    assert!(
                        measurement_notices(&stderr).is_empty(),
                        "{format}: {stderr}"
                    );
                }
                assert_eq!(default.stdout, platform.stdout, "{format}");
                assert_ne!(default.stdout, blank.stdout, "{format}: text must paint");
                if input == RAW_TREF_FONT_SVG {
                    assert!(
                        default.stdout == direct.stdout,
                        "{format}: href text must paint identically"
                    );
                }
            }
        }

        let rejected = raw_conversion(
            format,
            RAW_FONT_SVG,
            &["--font-file", &font, "--font-measurement", "native"],
        );
        assert_eq!(rejected.status.code(), Some(2));
        assert!(rejected.stdout.is_empty());
    }
}

#[cfg(all(
    windows,
    feature = "system-fonts",
    feature = "png",
    not(any(
        feature = "platform-system-font-default",
        feature = "windows-platform-font-default"
    ))
))]
#[test]
fn raw_svg_default_uses_bounded_system_font_context() {
    let root = tempfile::tempdir().unwrap();
    let fonts = root.path().join("Fonts");
    std::fs::create_dir(&fonts).unwrap();
    std::fs::copy(font(), fonts.join("Cousine-Regular.ttf")).unwrap();

    let convert = |font_root: &std::path::Path, source: &str, args: &[&str]| {
        assert_cmd::Command::new(assert_cmd::cargo_bin!("merman-cli"))
            .args(["render", "-", "--input-kind=svg", "--format=png"])
            .args(args)
            .env("WINDIR", font_root)
            .env("LOCALAPPDATA", font_root)
            .write_stdin(source)
            .output()
            .unwrap()
    };
    let bounded = convert(root.path(), RAW_NESTED_FONT_SVG, &["--font-system=on"]);
    let automatic = convert(root.path(), RAW_NESTED_FONT_SVG, &[]);
    for output in [&bounded, &automatic] {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        assert!(output.stdout.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
    assert_eq!(automatic.stdout, bounded.stdout);
    let empty = root.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let paths = convert(&empty, RAW_PATH_SVG, &[]);
    assert!(paths.status.success(), "{paths:?}");
    assert_ne!(automatic.stdout, paths.stdout);
    let text = convert(&empty, RAW_NESTED_FONT_SVG, &[]);
    assert_eq!(text.status.code(), Some(3), "{text:?}");
    assert!(String::from_utf8_lossy(&text.stderr).contains("no usable font faces"));
    assert!(text.stdout.is_empty());
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_font_notice_cases(label: &str, quiet: bool) -> Vec<std::process::Output> {
    let font = font();
    let formats = [
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n".as_slice()),
        #[cfg(feature = "jpeg")]
        ("jpeg", b"\xff\xd8".as_slice()),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF".as_slice()),
    ];
    let mermaid = format!(
        "---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\nA[\"{label}\"]\n"
    );
    let raw_svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="80"><text x="10" y="40" font-family="Cousine" font-size="16">{label}</text></svg>"#
    );
    let mut results = Vec::new();
    for (format, magic) in formats {
        for (input_kind, source) in [("mermaid", mermaid.as_str()), ("svg", raw_svg.as_str())] {
            let mut args = vec![
                "render",
                "--format",
                format,
                "--font-file",
                &font,
                "--font-system",
                "off",
            ];
            if input_kind == "svg" {
                args.extend(["--input-kind", "svg"]);
            }
            if quiet {
                args.push("--quiet");
            }
            let output = run_with_stdin(&args, source);
            assert!(
                output.status.success(),
                "{input_kind}/{format}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stdout.starts_with(magic),
                "{input_kind}/{format} must still publish a valid best-effort artifact"
            );
            results.push(output);
        }
    }
    results
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn missing_glyph_exports_warn_with_real_font_resolution_details() {
    for output in export_font_notice_cases("Latin 汉🙂", false) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let notice = stderr
            .lines()
            .find(|line| line.starts_with("warning: native export font resolution is incomplete:"))
            .unwrap_or_else(|| {
                panic!("missing glyphs must be reported after successful export: {stderr}")
            });
        assert!(
            notice.contains("unresolved glyph fallback=true"),
            "{notice}"
        );
        assert!(stderr.contains("some text may be missing"), "{stderr}");
        assert!(stderr.contains("--font-file PATH"), "{stderr}");
        let notdef = notice
            .split(".notdef glyphs=")
            .nth(1)
            .expect("report must state the actual unresolved glyph count")
            .split(';')
            .next()
            .unwrap()
            .parse::<usize>()
            .expect(".notdef glyph count is an integer");
        assert!(
            notdef > 0,
            "the closed Cousine set lacks the Han and emoji glyphs: {notice}"
        );
        assert!(
            !notice.contains("U+"),
            "the export report does not provide character codepoints"
        );
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn covered_latin_exports_do_not_warn_about_font_resolution() {
    for output in export_font_notice_cases("Plain Latin WWW iii", false) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("warning: native export font resolution"),
            "{stderr}"
        );
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn quiet_missing_glyph_exports_suppress_font_resolution_notices() {
    for output in export_font_notice_cases("Latin 汉🙂", true) {
        assert!(
            output.stderr.is_empty(),
            "quiet best-effort export must suppress diagnostics: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn best_effort_native_quality_residuals_warn_without_disabling_output_and_respect_quiet() {
    let directional = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(60 20) skewX(30)" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##;
    let fallback_css = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><style>tspan{font-size:64px!important}</style><foreignObject x="10" y="10" width="180" height="60"><div xmlns="http://www.w3.org/1999/xhtml"><p>Alpha <b>Beta</b></p></div></foreignObject></svg>"#;
    let formats = [
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n".as_slice()),
        #[cfg(feature = "jpeg")]
        ("jpeg", b"\xff\xd8".as_slice()),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF".as_slice()),
    ];
    for (format, magic) in formats {
        let mut cases = vec![
            (directional, "native_directional_filter_transform_residual"),
            (fallback_css, "fallback_css_residual"),
        ];
        if format == "pdf" {
            cases.push((r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><rect x="-10" y="10" width="20" height="20" fill="red"/></svg>"#, "pdf_implicit_viewport_clipping_residual"));
        }
        for (source, reason) in cases {
            for quiet in [false, true] {
                let mut args = vec!["render", "--input-kind", "svg", "--format", format];
                if quiet {
                    args.push("--quiet");
                }
                let output = run_with_stdin(&args, source);
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(output.status.success(), "{format}/{reason}: {stderr}");
                assert!(
                    output.stdout.starts_with(magic),
                    "best-effort still publishes {format}"
                );
                if quiet {
                    assert!(
                        stderr.is_empty(),
                        "--quiet must suppress quality notices: {stderr}"
                    );
                } else {
                    assert!(
                        stderr.contains(&format!("warning: {reason}:")),
                        "the real export report must disclose {reason}: {stderr}"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "markdown")]
#[test]
fn font_directory_protection_precedes_batch_publication() {
    let temp = tempfile::tempdir().unwrap();
    let fonts = temp.path().join("fonts");
    std::fs::create_dir(&fonts).unwrap();
    let original = std::fs::read(font()).unwrap();
    let font_path = fonts.join("Cousine.ttf");
    std::fs::write(&font_path, &original).unwrap();
    let markdown = "```mermaid\nflowchart LR\nA-->B\n```\n";
    std::fs::write(temp.path().join("input.md"), markdown).unwrap();

    for args in [
        vec![
            "batch",
            "input.md",
            "--font-dir",
            "fonts",
            "--output-dir",
            "fonts/missing/output",
        ],
        vec![
            "mmdc",
            "--input",
            "input.md",
            "--font-dir",
            "fonts",
            "--output",
            "output.md",
            "--artefacts",
            "fonts/missing/assets",
        ],
    ] {
        let output = support::run_with_stdin_in_dir(&args, "", Some(temp.path()));
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("font directory input"));
        assert!(!fonts.join("missing").exists());
        assert!(!temp.path().join("output.md").exists());
    }

    for entry in [".merman-manifest.json", "input-1.svg"] {
        let output_root = temp.path().join(entry.replace('.', "_") + "-out");
        std::fs::create_dir(&output_root).unwrap();
        std::fs::hard_link(&font_path, output_root.join(entry)).unwrap();
        let output = support::run_with_stdin_in_dir(
            &[
                "batch",
                "input.md",
                "--font-dir",
                "fonts",
                "--output-dir",
                output_root.to_str().unwrap(),
            ],
            "",
            Some(temp.path()),
        );
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("multiple hard links"));
        assert_eq!(std::fs::read(output_root.join(entry)).unwrap(), original);
        assert!(!output_root.join("input.md").exists());
        assert!(!output_root.join(".merman.lock").exists());
    }
    assert_eq!(std::fs::read(&font_path).unwrap(), original);
}
