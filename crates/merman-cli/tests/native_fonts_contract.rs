#![cfg(all(feature = "native-fonts", feature = "diagram-flowchart"))]

mod support;

use support::{repo_root, run_with_stdin};

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
#[cfg(not(feature = "system-fonts"))]
fn compiled_native_feature_does_not_change_default_rendering() {
    let source = "---\nconfig:\n  fontFamily: Cousine\n---\nflowchart LR\nA-->B\n";
    let implicit = svg(
        &["mmdc", "-i", "-", "-o", "-", "--svgId", "unchanged"],
        source,
    );
    let explicit = svg(
        &[
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--svgId",
            "unchanged",
            "--font-measurement",
            "deterministic",
        ],
        source,
    );
    assert_eq!(implicit, explicit);
}

#[cfg(feature = "system-fonts")]
#[test]
fn system_fonts_default_to_native_measurement_and_allow_compatibility_opt_out() {
    let source = "---\nconfig:\n  fontFamily: monospace\n  htmlLabels: false\n  flowchart:\n    minNodeWidth: 0\n    wrappingWidth: 1000\n---\nflowchart LR\nA[iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii]\n";
    let args = ["mmdc", "-i", "-", "-o", "-", "--svgId", "system-default"];
    let implicit = svg(&args, source);
    let mut explicit = args.to_vec();
    explicit.extend(["--font-measurement", "native"]);
    assert_eq!(implicit, svg(&explicit, source));
    let mut deterministic = args.to_vec();
    deterministic.extend(["--font-measurement", "deterministic"]);
    assert_ne!(implicit, svg(&deterministic, source));
    deterministic.extend(["--font-system", "off"]);
    let off = svg(&deterministic, source);
    assert_eq!(off, svg(&deterministic[..deterministic.len() - 2], source));
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
            diagnostic.contains("native_font_label_bytes") || diagnostic.contains("label-bytes"),
            "expected the native font label budget, not another failure: {diagnostic}"
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
    for extra in failures {
        let mut args = vec!["mmdc", "-i", "-", "-o", "-"];
        args.extend(extra);
        let output = run_with_stdin(&args, "flowchart LR\nA-->B\n");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
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
    assert_eq!(
        native["default_measurement"],
        if cfg!(feature = "system-fonts") {
            "native"
        } else {
            "deterministic"
        }
    );
    assert_eq!(
        native["default_font_source"],
        if cfg!(feature = "experimental-system-font-query") {
            "platform-managed-system-query"
        } else if cfg!(feature = "system-fonts") {
            "bounded-system-discovery"
        } else {
            "none"
        }
    );
    assert_eq!(native["selected_font_bytes_bounded"], true);
    assert_eq!(
        native["system_resources_bounded"],
        cfg!(feature = "system-fonts") && !cfg!(feature = "experimental-system-font-query")
    );
    assert_eq!(
        native["explicit_font_sources"],
        "closed-set-unless-font-system-on"
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
fn raw_svg_fonts_are_conversion_only_and_each_compiled_codec_works() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text x="10" y="40" font-family="Cousine">Native font</text></svg>"#;
    let font = font();
    let mut formats = Vec::new();
    #[cfg(feature = "png")]
    formats.push(("png", b"\x89PNG\r\n\x1a\n".as_slice()));
    #[cfg(feature = "jpeg")]
    formats.push(("jpeg", b"\xff\xd8".as_slice()));
    #[cfg(feature = "pdf")]
    formats.push(("pdf", b"%PDF".as_slice()));
    for (format, magic) in formats {
        let output = run_with_stdin(
            &[
                "render",
                "--input-kind",
                "svg",
                "--format",
                format,
                "--font-file",
                &font,
            ],
            input,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.starts_with(magic));
        let rejected = run_with_stdin(
            &[
                "render",
                "--input-kind",
                "svg",
                "--format",
                format,
                "--font-file",
                &font,
                "--font-measurement",
                "native",
            ],
            input,
        );
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
    }
}
