#![cfg(feature = "diagram-flowchart")]

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

#[cfg(feature = "png")]
mod rendering {
    use super::run_with_stdin;
    use std::io::Cursor;
    use std::process::Command;

    #[derive(Clone, Copy)]
    enum Primitive {
        Outline,
        Rule,
        Stroke,
    }

    fn rasterize(svg: &str) -> (u32, u32, Vec<u8>) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let input = tmp.path().join("math.svg");
        std::fs::write(&input, svg).expect("write raw SVG");
        let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
            .args([
                "render",
                "--format",
                "png",
                "--background",
                "white",
                "--output",
                "-",
            ])
            .arg(&input)
            .output()
            .expect("rasterize raw SVG");
        assert!(
            output.status.success(),
            "raw SVG rasterization failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        decode_png(output.stdout)
    }

    fn decode_png(bytes: Vec<u8>) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(Cursor::new(bytes))
            .read_info()
            .expect("PNG info");
        let mut pixels = vec![0; reader.output_buffer_size().expect("PNG buffer size")];
        let info = reader.next_frame(&mut pixels).expect("PNG frame");
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        pixels.truncate(info.buffer_size());
        (info.width, info.height, pixels)
    }

    fn replace_children(svg: &str, node: roxmltree::Node<'_, '_>, body: &str) -> String {
        let first = node.children().next().expect("nonempty SVG wrapper");
        let last = node.children().next_back().expect("nonempty SVG wrapper");
        format!(
            "{}{body}{}",
            &svg[node.range().start..first.range().start],
            &svg[last.range().end..node.range().end],
        )
    }

    fn native_scene(svg: &str, math: roxmltree::Node<'_, '_>, body: &str) -> String {
        let root = math.document().root_element();
        let defs = math
            .ancestors()
            .find(|ancestor| ancestor.has_tag_name("defs") && ancestor.parent() == Some(root))
            .expect("native formula must be defined in root defs");
        assert!(
            !math.ancestors().any(|ancestor| {
                ancestor.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| matches!(class, "node" | "hostile"))
                })
            }),
            "native formula definitions must be outside node and hostile ancestors"
        );
        let target = math
            .ancestors()
            .find(|ancestor| ancestor.parent() == Some(defs))
            .expect("root defs formula wrapper");
        let href = format!("#{}", target.attribute("id").expect("definition id"));
        let instances: Vec<_> = root
            .descendants()
            .filter(|node| {
                node.has_tag_name("use")
                    && node.attribute("href") == Some(href.as_str())
                    && !node
                        .ancestors()
                        .any(|ancestor| ancestor.has_tag_name("defs"))
            })
            .collect();
        assert_eq!(instances.len(), 1, "one effective native formula instance");
        // Keep the emitted definition and instance placement, including every wrapper attribute.
        let definition = format!(
            "{}{}{}",
            &svg[defs.range().start..math.range().start],
            replace_children(svg, math, body),
            &svg[math.range().end..defs.range().end],
        );
        let instance = instances[0];
        let mut placed = svg[instance.range()].to_owned();
        for ancestor in instance
            .ancestors()
            .skip(1)
            .take_while(|node| *node != root)
        {
            placed = replace_children(svg, ancestor, &placed);
        }
        let resources: String = root
            .children()
            .filter(|node| node.has_tag_name("defs") && *node != defs)
            .map(|node| &svg[node.range()])
            .collect();
        format!("{resources}{definition}{placed}")
    }

    fn assert_math_paint(formula: &str, primitive: Primitive) {
        let html_labels = true;
        for pipeline in ["parity", "resvg-safe"] {
            for styled in [false, true] {
                // Chromium owns browser !important cascade checks; usvg is not its oracle.
                if pipeline == "parity" && styled {
                    continue;
                }
                let source = format!(
                    "%%{{init: {{\"htmlLabels\": {html_labels}, \"flowchart\": {{\"htmlLabels\": {html_labels}}}}}}}%%\n\
                     flowchart LR\nA[\"$${formula}$$\"]{}\n\
                     classDef hostile fill:#ffffff,stroke:#00ff00,stroke-width:3px,color:#0055cc;\n",
                    if styled { ":::hostile" } else { "" },
                );
                let case =
                    format!("{formula}, htmlLabels={html_labels}, {pipeline}, classDef={styled}");
                let output = run_with_stdin(
                    &[
                        "render",
                        "--math-renderer",
                        "ratex",
                        "--format",
                        "svg",
                        "--svg-pipeline",
                        pipeline,
                        "-",
                    ],
                    &source,
                );
                assert!(
                    output.status.success(),
                    "{case}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let svg = String::from_utf8(output.stdout).expect("UTF-8 SVG");
                let doc = roxmltree::Document::parse(&svg).expect("parse CLI SVG");
                let root = doc.root_element();
                let css: String = root
                    .descendants()
                    .filter(|node| node.has_tag_name("style"))
                    .filter_map(|node| node.text())
                    .collect();
                let filters: String = root
                    .children()
                    .filter(|node| {
                        node.has_tag_name("defs")
                            && node.descendants().any(|child| child.has_tag_name("filter"))
                    })
                    .map(|node| &svg[node.range()])
                    .collect();
                if styled {
                    assert!(
                        css.split('}')
                            .any(|rule| rule.contains(".hostile") && rule.contains("!important")),
                        "{case}: expected source classDef important CSS"
                    );
                }
                let node = root
                    .descendants()
                    .find(|node| {
                        node.attribute("class").is_some_and(|class| {
                            class.split_ascii_whitespace().any(|c| c == "node")
                        })
                    })
                    .expect("actual flowchart node");
                let node_classes = node.attribute("class").expect("node classes");
                assert_eq!(
                    node_classes
                        .split_ascii_whitespace()
                        .any(|class| class == "hostile"),
                    styled,
                    "{case}: source node class"
                );
                let context = format!(
                    "<style>{css}</style><g class=\"{}\" data-look=\"{}\"><g class=\"label nodeLabel\">",
                    node_classes,
                    node.attribute("data-look").unwrap_or(""),
                );
                let math_svgs: Vec<_> = root
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("svg")
                            && *node != root
                            && node.children().any(|child| child.has_tag_name("path"))
                            && (pipeline != "parity"
                                || !node.ancestors().any(|ancestor| {
                                    ancestor.has_tag_name("template")
                                        || ancestor.has_tag_name("defs")
                                }))
                    })
                    .collect();
                assert!(!math_svgs.is_empty(), "{case}: no actual formula SVG");
                for (index, math) in math_svgs.into_iter().enumerate() {
                    let view_box: Vec<f64> = math
                        .attribute("viewBox")
                        .expect("formula viewBox")
                        .split_ascii_whitespace()
                        .map(|value| value.parse().expect("viewBox number"))
                        .collect();
                    assert_eq!(view_box.len(), 4, "{case}");
                    assert!(
                        view_box.iter().all(|value| value.is_finite())
                            && view_box[2] > 0.0
                            && view_box[3] > 0.0,
                        "{case}: invalid formula viewport"
                    );
                    let canvas = format!(
                        "<svg xmlns=\"http://www.w3.org/2000/svg\" id=\"{}\" \
                         width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">",
                        root.attribute("id").expect("diagram id"),
                        ((view_box[2] + 0.5) * 48.0).ceil(),
                        ((view_box[3] + 0.5) * 48.0).ceil(),
                        view_box[0] - 0.25,
                        view_box[1] - 0.25,
                        view_box[2] + 0.5,
                        view_box[3] + 0.5,
                    );
                    // Copy exact XML ranges: glyph paint and geometry must survive unchanged.
                    let body: String = math
                        .children()
                        .filter(|child| child.is_element())
                        .map(|child| &svg[child.range()])
                        .collect();
                    let required = math
                        .children()
                        .find(|child| match primitive {
                            Primitive::Outline => child.has_tag_name("path"),
                            Primitive::Rule => child.has_tag_name("rect"),
                            Primitive::Stroke => {
                                child.has_tag_name("path")
                                    && child.attribute("fill") == Some("none")
                                    && child
                                        .attribute("stroke")
                                        .is_some_and(|stroke| stroke != "none")
                            }
                        })
                        .unwrap_or_else(|| panic!("{case}: required formula primitive missing"));
                    // A rule/stroke must paint on its own, even if the other glyphs survive.
                    for (part, body) in [
                        ("formula", body.as_str()),
                        ("primitive", &svg[required.range()]),
                    ] {
                        let (reference_svg, actual_svg) = if pipeline == "resvg-safe" {
                            let scene = native_scene(&svg, math, body);
                            (
                                replace_children(&svg, root, &scene),
                                replace_children(
                                    &svg,
                                    root,
                                    &format!("<style>{css}</style>{scene}"),
                                ),
                            )
                        } else {
                            (
                                format!("{canvas}{filters}{body}</svg>"),
                                format!("{canvas}{filters}{context}{body}</g></g></svg>"),
                            )
                        };
                        let reference = rasterize(&reference_svg);
                        let actual = rasterize(&actual_svg);
                        assert_eq!(
                            (reference.0, reference.1),
                            (actual.0, actual.1),
                            "{case}, SVG {index}, {part}: canvas dimensions"
                        );
                        assert!(
                            reference
                                .2
                                .chunks_exact(4)
                                .any(|pixel| { pixel[3] == 255 && pixel[..3] != [255, 255, 255] }),
                            "{case}, SVG {index}, {part}: reference has no ink"
                        );
                        if formula.contains(r"\color{red}") {
                            assert!(
                                reference.2.chunks_exact(4).any(|pixel| {
                                    pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80
                                }),
                                "{case}, SVG {index}, {part}: explicit red lost"
                            );
                        }
                        let different = reference
                            .2
                            .chunks_exact(4)
                            .zip(actual.2.chunks_exact(4))
                            .filter(|(expected, actual)| expected != actual)
                            .count();
                        assert_eq!(
                            different, 0,
                            "{case}, SVG {index}, {part}: node CSS changed formula pixels"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cli_plain_svg_labels_follow_the_literal_math_branch() {
        let output = run_with_stdin(
            &["render", "--math-renderer", "ratex", "--format", "svg", "-"],
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA[\"$$x^2$$\"]\n",
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = String::from_utf8(output.stdout).unwrap();
        assert!(
            !svg.contains("<foreignObject"),
            "plain SVG labels must use their own branch"
        );
        assert!(
            svg.contains("$$x^2$$"),
            "plain SVG math retains the existing literal semantics"
        );
    }

    #[test]
    fn cli_ratex_outline_pixels_survive_node_css() {
        assert_math_paint("E=mc^2", Primitive::Outline);
    }

    #[test]
    fn cli_ratex_fraction_rule_pixels_survive_node_css() {
        assert_math_paint(r"\frac{a}{b}", Primitive::Rule);
    }

    #[test]
    fn cli_ratex_radical_stroke_pixels_survive_node_css() {
        // RaTeX outlines the surd; cancel adds a real unfilled, stroked path.
        assert_math_paint(r"\sqrt{\cancel{x}}", Primitive::Stroke);
    }

    #[test]
    fn cli_ratex_explicit_color_pixels_survive_node_css() {
        assert_math_paint(r"\color{red}{E=mc^2}", Primitive::Outline);
    }

    #[cfg(feature = "native-fonts")]
    #[test]
    fn cli_mermaid_png_keeps_math_ink_in_label_center() {
        let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../merman-export/tests/fonts/Cousine-Regular.ttf");
        let font = font.to_str().expect("fixture font path");
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                "png",
                "--output",
                "-",
                "--math-renderer",
                "ratex",
                "--font-measurement",
                "native",
                "--font-file",
                font,
                "--font-system",
                "off",
                "--background",
                "white",
                "-",
            ],
            "---\nconfig:\n  fontFamily: Cousine\n---\nflowchart LR\nA[\"$$\\color{red}{E=mc^2}$$\"]\n",
        );
        assert!(
            output.status.success(),
            "Mermaid PNG export failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let (width, height, pixels) = decode_png(output.stdout);
        assert!(width >= 4 && height >= 4, "nonempty diagram viewport");
        // A single node centers its label; the middle half excludes the node border.
        let has_math_ink = (height / 4..height * 3 / 4).any(|y| {
            (width / 4..width * 3 / 4).any(|x| {
                let offset = (y as usize * width as usize + x as usize) * 4;
                let pixel = &pixels[offset..offset + 4];
                pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80 && pixel[3] > 128
            })
        });
        assert!(
            has_math_ink,
            "Mermaid PNG lost red formula ink in label center"
        );
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod native_exports {
    use super::run_with_stdin;

    const MATH: &str = "flowchart LR\nA[\"$$\\frac{x}{y}$$\"]\n";
    const CSS: &str = "path { fill: #ff0000 !important; stroke: #00ff00 !important; stroke-width: 3px !important; }";
    const FORMATS: &[(&str, &[u8])] = &[
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n"),
        #[cfg(feature = "jpeg")]
        ("jpeg", b"\xff\xd8\xff"),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF-"),
    ];

    #[test]
    fn cli_native_math_with_scoped_css_is_rejected() {
        let css = tempfile::NamedTempFile::new().expect("CSS file");
        std::fs::write(css.path(), CSS).unwrap();
        let css_path = css.path().to_str().expect("CSS path");
        for &(format, _) in FORMATS {
            let output = run_with_stdin(
                &[
                    "render",
                    "--format",
                    format,
                    "--output",
                    "-",
                    "--math-renderer",
                    "ratex",
                    "--css-file",
                    css_path,
                    "--font-measurement",
                    "deterministic",
                    "-",
                ],
                MATH,
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !output.status.success(),
                "{format}: caller CSS must not change formula paint"
            );
            assert!(output.stdout.is_empty(), "{format}: no partial artifact");
            assert!(
                stderr.contains("prepared-math evidence was invalidated"),
                "{format}: {stderr}"
            );
        }
    }

    #[test]
    fn cli_native_exports_allow_math_without_css_and_css_without_math() {
        let css = tempfile::NamedTempFile::new().expect("CSS file");
        std::fs::write(css.path(), CSS).unwrap();
        let css_path = css.path().to_str().expect("CSS path");
        for &(format, magic) in FORMATS {
            for (source, styled) in [
                (MATH, false),
                ("flowchart LR\nA[Ordinary label]\n", true),
                (
                    "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA[\"$$x^2$$\"]\n",
                    true,
                ),
            ] {
                let mut args = vec![
                    "render",
                    "--format",
                    format,
                    "--output",
                    "-",
                    "--math-renderer",
                    "ratex",
                ];
                if styled {
                    args.extend([
                        "--css-file",
                        css_path,
                        "--font-measurement",
                        "deterministic",
                    ]);
                }
                args.push("-");
                let output = run_with_stdin(&args, source);
                assert!(
                    output.status.success(),
                    "{format}, styled={styled}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(
                    output.stdout.starts_with(magic),
                    "{format}: encoded artifact"
                );
            }
        }
    }
}
