#![cfg(all(
    feature = "all-diagrams",
    feature = "svg",
    feature = "layout-cytoscape"
))]

use merman_bindings_core::render_svg;
use serde_json::{Value, json};

const ID: &str = "issue-theme";
const C4: &str = "C4Context\nPerson(customer, \"Customer\", \"A customer\")\nSystem(bank, \"Banking System\", \"Does banking\")\nRel(customer, bank, \"Uses\")";
const ZENUML: &str = "zenuml\nAlice->Bob: Hello\nBob-->Alice: Reply";
const WARDLEY: &str = "wardley-beta\ntitle Tea Shop\nanchor Business [0.95, 0.63]\ncomponent Tea [0.79, 0.61] (build)\ncomponent Water [0.38, 0.82] (market)\nBusiness -> Tea\nTea -> Water\nannotations [0.1, 0.2]\nannotation 1,[0.78, 0.82] \"Notes\"\naccelerator \"New\" [0.2, 0.85]";
const GANTT: &str =
    "gantt\ntitle A Gantt Diagram\ndateFormat YYYY-MM-DD\nsection A\nTask1 :a1, 2024-01-01, 1d";
const XY: &str = "xychart\ntitle Chart\nx-axis [One, Two, Three]\nline [10, 30, 20]";
const MINDMAP: &str =
    "mindmap\nroot\n  child[Square]\n    plain\n    with icon\n    ::icon(mdi mdi-fire)";

fn options(dark: bool, pipeline: &str) -> Value {
    json!({
        "presentation": {"theme": {
            "appearance": if dark {"dark"} else {"light"},
            "font_family": "Inter, sans-serif",
            "roles": {
                "canvas": if dark {"#1e1f22"} else {"#ffffff"},
                "surface": if dark {"#2b2d30"} else {"#f4f4f5"},
                "text": if dark {"#dfe1e5"} else {"#1f2937"},
                "border": if dark {"#6f737a"} else {"#64748b"},
                "line": if dark {"#a8adbd"} else {"#334155"}
            },
            "series_palette": ["#1e3a5f", "#4a3410", "#183d2a", "#4a1d3a"]
        }},
        "svg": {"diagram_id": ID, "pipeline": pipeline}
    })
}

fn render(source: &str, options: &Value) -> String {
    String::from_utf8(render_svg(source.as_bytes(), &serde_json::to_vec(options).unwrap()).unwrap())
        .unwrap()
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|item| item == class))
}

fn fallback_content<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    fallback: roxmltree::Node<'a, 'input>,
) -> roxmltree::Node<'a, 'input> {
    let target = fallback
        .descendants()
        .find(|node| node.has_tag_name("use"))
        .and_then(|node| node.attribute("href"))
        .and_then(|href| href.strip_prefix('#'));
    target.map_or(fallback, |target| {
        document
            .descendants()
            .find(|node| node.attribute("id") == Some(target))
            .expect("fallback use must resolve")
    })
}

fn css_property(document: &roxmltree::Document<'_>, selector: &str, property: &str) -> String {
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| css.split('}'))
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(selectors, _)| selectors.split(',').any(|item| item.trim() == selector))
        .flat_map(|(_, declarations)| declarations.split(';'))
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(key, _)| key.trim() == property)
        .map(|(_, value)| value.trim().to_owned())
        .next_back()
        .unwrap_or_else(|| panic!("missing {selector} {{ {property} }}"))
}

fn explicit_fill(node: roxmltree::Node<'_, '_>) -> String {
    for node in node.ancestors() {
        if let Some(value) = node.attribute("style").and_then(|style| {
            style
                .split(';')
                .filter_map(|entry| entry.split_once(':'))
                .find(|(key, _)| key.trim() == "fill")
                .map(|(_, value)| value.trim().trim_end_matches("!important").trim())
        }) {
            return value.to_owned();
        }
        if let Some(value) = node.attribute("fill") {
            return value.to_owned();
        }
    }
    panic!("missing explicit fill");
}

#[test]
fn bindings_apply_light_and_dark_roles_to_the_reported_paint_surfaces() {
    for dark in [false, true] {
        let text = if dark { "#dfe1e5" } else { "#1f2937" };
        let line = if dark { "#a8adbd" } else { "#334155" };
        let surface = if dark { "#2b2d30" } else { "#f4f4f5" };
        let canvas = if dark { "#1e1f22" } else { "#ffffff" };
        for pipeline in ["parity", "resvg-safe"] {
            let opts = options(dark, pipeline);
            let svg = render(C4, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for label in ["Customer", "Banking System", "Uses"] {
                let node = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("text")
                            && node
                                .descendants()
                                .filter_map(|part| part.text().filter(|_| part.is_text()))
                                .collect::<String>()
                                .contains(label)
                    })
                    .unwrap();
                assert_eq!(explicit_fill(node), text, "C4 {label}: {pipeline}");
            }

            let svg = render(ZENUML, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for (class, property, expected) in [
                ("frame-border-inner", "fill", canvas),
                ("participant-box", "fill", surface),
                ("participant-label", "fill", text),
                ("message-line", "stroke", line),
                ("message-label", "fill", text),
            ] {
                assert_eq!(
                    css_property(&document, &format!("#{ID} .{class}"), property),
                    expected,
                    "ZenUML {class}: {pipeline}"
                );
            }
            assert_eq!(
                css_property(&document, &format!("#{ID} .message-label"), "font-family"),
                "Inter,sans-serif"
            );

            let svg = render(WARDLEY, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let anchor = document
                .descendants()
                .find(|node| has_class(*node, "wardley-node--anchor"))
                .unwrap();
            let label = anchor
                .descendants()
                .find(|node| has_class(*node, "wardley-node-label"))
                .unwrap();
            assert_eq!(label.attribute("fill"), Some(text));
            let stages = document
                .descendants()
                .find(|node| has_class(*node, "wardley-stages"))
                .unwrap();
            assert!(
                stages
                    .children()
                    .filter(|node| node.has_tag_name("line"))
                    .all(|node| node.attribute("stroke") == Some(line))
            );
            let annotation = document
                .descendants()
                .find(|node| has_class(*node, "wardley-annotations-box"))
                .unwrap();
            assert_eq!(
                annotation
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap()
                    .attribute("fill"),
                Some(surface)
            );
            let map = document
                .descendants()
                .find(|node| has_class(*node, "wardley-map"))
                .unwrap();
            assert_eq!(map.attribute("font-family"), Some("Inter,sans-serif"));

            let svg = render(GANTT, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                css_property(&document, &format!("#{ID} .grid"), "color"),
                line
            );
            assert!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name("line")
                        && node.ancestors().any(|parent| has_class(parent, "grid")))
                    .all(|node| node.attribute("stroke") == Some("currentColor"))
            );

            let svg = render(XY, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                css_property(&document, &format!("#{ID}"), "font-family"),
                "Inter,sans-serif"
            );
            assert_eq!(css_property(&document, &format!("#{ID}"), "fill"), text);
            assert_eq!(
                css_property(&document, &format!("#{ID} .marker"), "stroke"),
                line
            );
            for group in ["bottom-axis", "left-axis"] {
                let axis = document
                    .descendants()
                    .find(|node| has_class(*node, group))
                    .unwrap();
                assert!(
                    axis.descendants()
                        .filter(|node| node.has_tag_name("path"))
                        .all(|node| node.attribute("stroke") == Some(line))
                );
            }

            let svg = render(MINDMAP, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                css_property(&document, &format!("#{ID} .labelBkg"), "background-color"),
                canvas
            );
            if pipeline == "resvg-safe" {
                let fallback = document
                    .descendants()
                    .filter(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
                    .map(|node| fallback_content(&document, node))
                    .find(|node| {
                        node.descendants()
                            .filter_map(|part| part.text().filter(|_| part.is_text()))
                            .collect::<String>()
                            .contains("with icon")
                    })
                    .unwrap();
                let background = fallback
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect") && node.attribute("fill") != Some("none")
                    })
                    .unwrap();
                assert_eq!(background.attribute("fill"), Some(canvas));
                let label = fallback
                    .descendants()
                    .find(|node| node.has_tag_name("text"))
                    .unwrap();
                assert_eq!(explicit_fill(label), text);
                assert!(!svg.contains("rgba(232, 232, 232, 0.5)"));
            }
        }
    }
}

#[test]
fn source_and_site_overrides_still_win_over_presentation_defaults() {
    let mut opts = options(true, "resvg-safe");
    opts["site_config"] = json!({"themeVariables": {"wardley": {"anchorLabelColor": "#abcdef", "fontFamily":"Georgia, serif"}, "mindmap": {"labelBackgroundColor": "#123456"}, "gantt": {"axisColor": "#c0ffee"}}});
    let svg = render(WARDLEY, &opts);
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        document
            .descendants()
            .find(|node| has_class(*node, "wardley-map"))
            .unwrap()
            .attribute("font-family"),
        Some("Georgia,serif")
    );
    let anchor = document
        .descendants()
        .find(|node| has_class(*node, "wardley-node--anchor"))
        .unwrap();
    assert_eq!(
        anchor
            .descendants()
            .find(|node| has_class(*node, "wardley-node-label"))
            .unwrap()
            .attribute("fill"),
        Some("#abcdef")
    );
    let svg = render(GANTT, &opts);
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        css_property(&document, &format!("#{ID} .grid"), "color"),
        "#c0ffee"
    );
    let source = format!(
        "---\nconfig:\n  themeVariables:\n    mindmap:\n      labelBackgroundColor: '#aabbcc'\n---\n{MINDMAP}"
    );
    let svg = render(&source, &opts);
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        css_property(&document, &format!("#{ID} .labelBkg"), "background-color"),
        "#aabbcc"
    );
}

#[test]
fn empty_presentation_preserves_plain_output_for_affected_families() {
    for source in [C4, ZENUML, WARDLEY, GANTT, XY, MINDMAP] {
        let opts = json!({"svg":{"diagram_id": ID}});
        let mut empty = opts.clone();
        empty["presentation"] = json!({});
        assert_eq!(render(source, &opts), render(source, &empty));
    }
}

#[test]
fn wardley_annotations_use_readable_text_on_their_own_surface() {
    let opts = json!({
        "presentation":{"theme":{"roles":{"canvas":"#ffffff", "surface":"#111111"}}},
        "svg":{"diagram_id":ID, "pipeline":"resvg-safe"}
    });
    let svg = render(WARDLEY, &opts);
    let document = roxmltree::Document::parse(&svg).unwrap();
    for group in ["wardley-annotation", "wardley-annotations-box"] {
        let group = document
            .descendants()
            .find(|node| has_class(*node, group))
            .unwrap();
        let background = group
            .children()
            .find(|node| node.has_tag_name("rect") || node.has_tag_name("circle"))
            .unwrap();
        assert_eq!(background.attribute("fill"), Some("#111111"));
        let label = group
            .children()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(label.attribute("fill"), Some("#ffffff"));
    }
}
