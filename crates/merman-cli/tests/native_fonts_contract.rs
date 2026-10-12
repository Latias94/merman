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
        let output = support::run_with_stdin_in_dir(&args, source, Some(temp.path()));
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
    std::os::unix::fs::symlink(&fonts, &alias).unwrap();
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_dir(&fonts, &alias) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("create directory alias: {error}");
    }

    for output_name in ["font-view/existing.svg", "font-view/new.svg"] {
        let output = support::run_with_stdin_in_dir(
            &[
                "render",
                "-",
                "--font-dir",
                "fonts",
                "--format",
                "svg",
                "--output",
                output_name,
            ],
            "flowchart LR\nA-->B\n",
            Some(temp.path()),
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("aliases protected font directory input"),
            "expected canonical directory protection: {diagnostic}"
        );
        assert_eq!(std::fs::read(&font_path).unwrap(), original);
        assert_eq!(
            std::fs::read_to_string(fonts.join("existing.svg")).unwrap(),
            "previous output"
        );
        assert!(!fonts.join("new.svg").exists());
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
            let output = support::run_with_stdin_in_dir(
                &[
                    command[0],
                    command[1],
                    "--font-dir",
                    "fonts",
                    "--format",
                    "svg",
                    "--output",
                    output_name,
                ],
                source,
                Some(temp.path()),
            );
            if matches!(output_name, "ordinary.svg" | "new.svg") {
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                continue;
            }
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
    assert_eq!(native["default_measurement"], "deterministic");
    assert_eq!(native["default_font_source"], "none");
    assert_eq!(native["default_system_discovery"], "disabled");
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
