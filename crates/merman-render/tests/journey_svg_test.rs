use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_journey(source: &str, config: serde_json::Value) -> family::RenderedFamilySvg {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Journey source")
        .expect("detect Journey source")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Journey layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Journey SVG")
}

#[test]
fn journey_numeric_actor_legend_preserves_identity_and_authored_task_people() {
    for (people, expected_legend, expected_legend_classes) in [
        ("10, 2, 10", ["2", "10"], ["actor-1", "actor-0"]),
        ("A10, A2, A10", ["A10", "A2"], ["actor-0", "actor-1"]),
    ] {
        let source = format!("journey\nPay: 4: {people}\n");
        let rendered = render_journey(&source, serde_json::json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let legend_text = document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("legend"))
            .map(|node| {
                node.descendants()
                    .find(|span| span.has_tag_name("tspan"))
                    .and_then(|span| span.text())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(legend_text, expected_legend, "{people}");

        let legend_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node
                        .attribute("class")
                        .is_some_and(|class| class.starts_with("actor-"))
                    && !node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            expected_legend_classes,
            "{people}"
        );
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("cy").unwrap())
                .collect::<Vec<_>>(),
            ["60", "80"]
        );

        let task_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            task_circles
                .iter()
                .map(|node| {
                    node.children()
                        .find(|child| child.has_tag_name("title"))
                        .and_then(|title| title.text())
                        .unwrap()
                })
                .collect::<Vec<_>>(),
            people.split(", ").collect::<Vec<_>>()
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            ["actor-0", "actor-1", "actor-0"]
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("fill").unwrap())
                .collect::<Vec<_>>(),
            ["#8FBC8F", "#7CFC00", "#8FBC8F"]
        );
    }
}

#[test]
fn journey_unicode_actor_order_matches_utf16_sort_and_preserves_source_order() {
    for (people, expected, task_classes, task_colors) in [
        (
            "😀, \u{e000}",
            ["😀", "\u{e000}"],
            ["actor-0", "actor-1"],
            ["#8FBC8F", "#7CFC00"],
        ),
        (
            "\u{e000}, 😀",
            ["😀", "\u{e000}"],
            ["actor-1", "actor-0"],
            ["#7CFC00", "#8FBC8F"],
        ),
        (
            "A, Z",
            ["A", "Z"],
            ["actor-0", "actor-1"],
            ["#8FBC8F", "#7CFC00"],
        ),
    ] {
        let source = format!("journey\nPay: 4: {people}\n");
        let rendered = render_journey(&source, serde_json::json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let legend = document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("legend"))
            .map(|node| {
                node.descendants()
                    .find(|span| span.has_tag_name("tspan"))
                    .and_then(|span| span.text())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(legend, expected, "{people}");

        let legend_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node
                        .attribute("class")
                        .is_some_and(|class| class.starts_with("actor-"))
                    && !node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            ["actor-0", "actor-1"]
        );
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("cy").unwrap())
                .collect::<Vec<_>>(),
            ["60", "80"]
        );

        let task_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            task_circles
                .iter()
                .map(|node| {
                    node.children()
                        .find(|child| child.has_tag_name("title"))
                        .and_then(|title| title.text())
                        .unwrap()
                })
                .collect::<Vec<_>>(),
            people.split(", ").collect::<Vec<_>>()
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            task_classes
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("fill").unwrap())
                .collect::<Vec<_>>(),
            task_colors
        );
    }
}

#[test]
fn journey_frontmatter_zero_cell_dimensions_reach_terminal_svg_geometry() {
    for (width, height, task_y, face_x, activity_y) in [
        (0, 0, "10", "150", "0"),
        (0, 50, "110", "150", "200"),
        (150, 0, "10", "225", "0"),
        (1, 1, "12", "150.5", "4"),
    ] {
        let source = format!(
            "---\nconfig:\n  journey:\n    width: {width}\n    height: {height}\n---\njourney\nsection Checkout\nPay: 4\n"
        );
        let rendered = render_journey(&source, serde_json::json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let task = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "task")
                    })
            })
            .unwrap();
        let section = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "journey-section")
                    })
            })
            .unwrap();
        let expected_width = width.to_string();
        let expected_height = height.to_string();
        for rect in [task, section] {
            assert_eq!(rect.attribute("width"), Some(expected_width.as_str()));
            assert_eq!(rect.attribute("height"), Some(expected_height.as_str()));
        }
        assert_eq!(task.attribute("y"), Some(task_y));
        assert_eq!(section.attribute("y"), Some("50"));
        let face = document
            .descendants()
            .find(|node| node.has_tag_name("circle") && node.attribute("class") == Some("face"))
            .unwrap();
        assert_eq!(face.attribute("cx"), Some(face_x));
        let task_line = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("class") == Some("task-line"))
            .unwrap();
        assert_eq!(task_line.attribute("x1"), Some(face_x));
        assert_eq!(task_line.attribute("x2"), Some(face_x));
        assert_eq!(task_line.attribute("y1"), Some(task_y));
        assert_eq!(task_line.attribute("y2"), Some("450"));
        let activity_line = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .unwrap();
        assert_eq!(activity_line.attribute("y1"), Some(activity_y));
        assert_eq!(activity_line.attribute("y2"), Some(activity_y));
    }
}

#[test]
fn journey_scores_select_fractional_faces_and_preserve_integer_and_nan_controls() {
    for (literal, expected_y, mouth_tag, mouth_y) in [
        ("3.5", "345", "path", Some("347")),
        ("3.01", "359.7", "path", Some("361.7")),
        ("2.75", "367.5", "path", Some("374.5")),
        ("3", "360", "line", None),
        ("4", "330", "path", Some("332")),
        ("0x4", "330", "path", Some("332")),
        ("0b11", "360", "line", None),
        ("0o5", "300", "path", Some("302")),
        ("\u{feff}3.5\u{feff}", "345", "path", Some("347")),
        ("\u{0085}3.5\u{0085}", "NaN", "line", None),
        ("bad", "NaN", "line", None),
        ("NaN", "NaN", "line", None),
        ("Infinity", "-Infinity", "path", Some("-Infinity")),
        ("-Infinity", "Infinity", "path", Some("Infinity")),
    ] {
        let source = format!("journey\nPay: {literal}: Customer\n");
        let rendered = render_journey(&source, serde_json::json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let face = document
            .descendants()
            .find(|node| node.has_tag_name("circle") && node.attribute("class") == Some("face"))
            .unwrap();
        assert_eq!(face.attribute("cy"), Some(expected_y), "{literal}");
        let mouth = document
            .descendants()
            .find(|node| node.attribute("class") == Some("mouth"))
            .unwrap();
        assert_eq!(mouth.tag_name().name(), mouth_tag, "{literal}");
        let transform =
            mouth_y.map(|y| format!("translate({},{y})", face.attribute("cx").unwrap()));
        assert_eq!(
            mouth.attribute("transform"),
            transform.as_deref(),
            "{literal}"
        );
        if expected_y == "NaN" {
            assert_eq!(mouth.attribute("y1"), Some("NaN"));
            assert_eq!(mouth.attribute("y2"), Some("NaN"));
        }
    }
}

#[test]
fn journey_tspan_breaks_keep_the_opening_tag_rule() {
    for (separator, expected) in [
        ("<BR />", vec!["Pay", "now"]),
        ("<br\u{00a0}/>", vec!["Pay", "now"]),
        ("</br>", vec!["Pay</br>now"]),
    ] {
        let source = format!("journey\nPay{separator}now: 4\n");
        let rendered = render_journey(
            &source,
            serde_json::json!({"journey": {"textPlacement": "tspan"}}),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rows: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("task"))
            .map(|node| {
                node.descendants()
                    .find(|child| child.is_text())
                    .unwrap()
                    .text()
                    .unwrap()
            })
            .collect();
        assert_eq!(rows, expected, "{separator:?}");
    }
}

#[test]
fn journey_text_placement_owns_label_dom_fonts() {
    let source = "journey\nsection Order<br/>review\nPay<br>now: 4\n";
    for placement in [None, Some("fo"), Some("tspan"), Some("old")] {
        let mut config = serde_json::json!({
            "journey": {"taskFontSize": 20, "taskFontFamily": "Task Choice"}
        });
        if let Some(placement) = placement {
            config["journey"]["textPlacement"] = serde_json::json!(placement);
        }
        let rendered = render_journey(source, config);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let uses_fo = placement.is_none() || placement == Some("fo");
        for tag in ["switch", "foreignObject"] {
            assert_eq!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name(tag))
                    .count(),
                if uses_fo { 2 } else { 0 },
                "placement {placement:?}, element {tag}"
            );
        }
        let labels = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_whitespace()
                            .any(|token| token == "task" || token == "journey-section")
                    })
            })
            .collect::<Vec<_>>();
        if placement == Some("old") {
            assert_eq!(labels.len(), 2);
            assert_eq!(labels[0].text(), Some("Order<br/>review"));
            assert_eq!(labels[1].text(), Some("Pay<br>now"));
            assert_eq!(labels[0].attribute("y"), Some("80"));
            assert_eq!(labels[1].attribute("y"), Some("140"));
            assert!(labels.iter().all(|node| {
                node.children().all(|child| !child.has_tag_name("tspan"))
                    && node.attribute("style") == Some("text-anchor: middle;")
            }));
        } else {
            assert_eq!(labels.len(), 4);
            for (index, (node, text)) in labels
                .iter()
                .zip(["Order", "review", "Pay", "now"])
                .enumerate()
            {
                assert_eq!(node.attribute("x"), Some("225"));
                assert_eq!(
                    node.attribute("y"),
                    Some(if index < 2 { "75" } else { "135" })
                );
                assert_eq!(node.attribute("dominant-baseline"), Some("central"));
                let style = node.attribute("style").unwrap();
                assert!(style.contains("font-size: 20px;"), "{style}");
                assert!(style.contains("font-family: Task Choice;"), "{style}");
                let tspan = node
                    .children()
                    .find(|child| child.has_tag_name("tspan"))
                    .unwrap();
                assert_eq!(
                    tspan.attribute("dy"),
                    Some(if index % 2 == 0 { "-10" } else { "10" })
                );
                assert_eq!(tspan.text(), Some(text));
                if placement == Some("tspan") {
                    assert_eq!(node.attribute("fill"), Some("#fff"));
                    assert!(!style.contains("fill:"));
                }
            }
        }
    }
}

#[test]
fn journey_frontmatter_tspan_retains_inherited_legend_typography() {
    let source = concat!(
        "---\nconfig:\n  journey:\n",
        "    textPlacement: tspan\n",
        "    taskFontSize: 18\n",
        "    taskFontFamily: Task Specific\n",
        "  themeVariables:\n    fontFamily: JourneySource\n    fontSize: 21px\n",
        "---\njourney\nsection Checkout\nPay: 3.5: A\n",
    );
    let rendered = render_journey(source, serde_json::json!({}));
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("switch"))
    );
    let task = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("task"))
        .unwrap();
    assert!(
        task.attribute("style")
            .unwrap()
            .contains("font-family: Task Specific;")
    );
    assert!(
        task.attribute("style")
            .unwrap()
            .contains("font-size: 18px;")
    );
    let legend = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("legend"))
        .unwrap();
    assert_eq!(legend.attribute("style"), None);
    assert!(rendered.svg().contains("font-family:JourneySource"));
    assert!(rendered.svg().contains("font-size:21px"));
}

#[test]
fn journey_tspan_retains_section_colours_separately_from_inherited_text_paint() {
    let source = concat!(
        "journey\n",
        "Before section: 3.5\n",
        "section First\n",
        "Pay: 4\n",
        "section Second\n",
        "Confirm: 3\n",
        "section Third\n",
        "Complete: 4\n",
    );
    let rendered = render_journey(
        source,
        serde_json::json!({
            "journey": {
                "textPlacement": "tspan",
                "sectionColours": ["#123456", "#654321"]
            },
            "themeVariables": {"textColor": "#abcdef"}
        }),
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let labels = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("task"))
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 4);
    // Mermaid's config merge retains the default colour before appending authored colours.
    for (label, colour) in labels.iter().zip(["black", "#fff", "#123456", "#654321"]) {
        assert_eq!(label.attribute("fill"), Some(colour));
        assert!(!label.attribute("style").unwrap().contains("fill:"));
    }
    assert!(rendered.svg().contains("line{stroke:#abcdef;}"));
}

#[test]
fn journey_accessibility_dom_links_complete_punctuation_payloads() {
    for (title, description) in [
        ("Checkout 1", "Confirm then pay"),
        (
            "Checkout #1 ready; confirm & pay",
            "Confirm; then pay #ready",
        ),
    ] {
        let source =
            format!("journey\naccTitle: {title}\naccDescr: {description}\nPay: 4: Customer\n");
        let rendered = render_journey(&source, serde_json::json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let root = document.root_element();
        for (aria, tag, expected) in [
            ("aria-labelledby", "title", title),
            ("aria-describedby", "desc", description),
        ] {
            let target = root.attribute(aria).unwrap();
            let node = root
                .children()
                .find(|node| node.attribute("id") == Some(target))
                .unwrap();
            assert!(node.has_tag_name(tag));
            assert_eq!(node.text(), Some(expected));
        }
    }
}

#[test]
fn journey_unicode_structural_whitespace_reaches_accessible_and_visible_svg_text() {
    let same_line = concat!(
        "journey\r\n",
        "\u{feff}title\u{feff}Checkout\r\n",
        "\u{feff}accTitle\u{feff}:\u{feff}\u{0085}Accessible\u{0085}\u{feff}\r\n",
        "\u{feff}accDescr\u{feff}{\u{feff}Help\u{feff}}\u{feff}Pay: 4: Customer\r\n",
        "\u{feff}section\u{feff}Delivery\r\n",
        "\u{feff}Ship: 5: Customer\r\n",
    );
    let cross_line = concat!(
        "journey\r\n",
        "\u{feff}title\r\nCheckout\r\n",
        "accTitle\r\n\u{feff}:\r\n\u{feff}\u{0085}Accessible\u{0085}\u{feff}\r\n",
        "accDescr\r\n\u{feff}{\r\n\u{feff}Help\r\n}\u{feff}Pay: 4: Customer\r\n",
        "section\r\nDelivery\r\n",
        "\u{feff}Ship: 5: Customer\r\n",
    );
    for source in [same_line, cross_line] {
        let rendered = render_journey(source, serde_json::json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let root = document.root_element();
        for (aria, tag, expected) in [
            ("aria-labelledby", "title", "\u{0085}Accessible\u{0085}"),
            ("aria-describedby", "desc", "Help"),
        ] {
            let id = root.attribute(aria).unwrap();
            let target = root
                .children()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap();
            assert!(target.has_tag_name(tag));
            assert_eq!(target.text(), Some(expected));
        }
        for text in ["Checkout", "Delivery", "Pay", "Ship"] {
            assert!(
                document
                    .descendants()
                    .any(|node| node.is_text() && node.text() == Some(text)),
                "{text}"
            );
        }
    }
}
