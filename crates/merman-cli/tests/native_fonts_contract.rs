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

fn measurement_notices(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("native measurement faces: "))
        .collect()
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
            assert!(stderr.contains("--font-max-measurements"), "{stderr}");
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
        retry[6] = "--font-max-measurements=8192";
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
        let output = support::run_before_stdin_close_in_dir(
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
            Some(temp.path()),
            std::time::Duration::from_secs(2),
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
    assert_eq!(native["default_policy_scope"], "mermaid-svg-input");
    let expected_encoded = if cfg!(feature = "experimental-system-font-query") {
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
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("raw SVG uses the selected font context for painting"),
            "{format}: {stderr}"
        );
        assert!(
            measurement_notices(&stderr).is_empty(),
            "{format}: {stderr}"
        );
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
    let directional = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(60 20) rotate(90)" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##;
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
