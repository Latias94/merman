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
    let canvas = if dark { "#1e1f22" } else { "#ffffff" };
    let surface = if dark { "#2b2d30" } else { "#f4f4f5" };
    let text = if dark { "#dfe1e5" } else { "#1f2937" };
    let border = if dark { "#6f737a" } else { "#64748b" };
    let line = if dark { "#a8adbd" } else { "#334155" };
    json!({
        "theme": {"spec": {"mermaid": {
            "theme": "base",
            "variables": {
                "fontFamily": "Inter, sans-serif",
                "textColor": text,
                "primaryColor": surface,
                "primaryTextColor": text,
                "nodeTextColor": text,
                "lineColor": line
            }
        }}},
        "site_config": {
            "c4": {
                "person_bg_color": surface,
                "person_border_color": border,
                "person_font_color": text,
                "personFontFamily": "Inter, sans-serif",
                "system_bg_color": surface,
                "system_border_color": border,
                "system_font_color": text,
                "systemFontFamily": "Inter, sans-serif",
                "relationship_text_color": text,
                "relationship_line_color": line,
                "messageFontFamily": "Inter, sans-serif"
            },
            "themeVariables": {
                "xyChart": {
                    "backgroundColor": canvas,
                    "titleColor": text,
                    "legendTextColor": text,
                    "xAxisTitleColor": text,
                    "xAxisLabelColor": text,
                    "yAxisTitleColor": text,
                    "yAxisLabelColor": text,
                    "xAxisTickColor": line,
                    "xAxisLineColor": line,
                    "yAxisTickColor": line,
                    "yAxisLineColor": line
                },
                "wardley": {
                    "backgroundColor": canvas,
                    "axisColor": line,
                    "axisTextColor": text,
                    "componentFill": surface,
                    "componentStroke": border,
                    "componentLabelColor": text,
                    "anchorLabelColor": text,
                    "stageDividerColor": line,
                    "annotationBackgroundColor": surface,
                    "annotationLabelColor": text,
                    "fontFamily": "Inter, sans-serif"
                },
                "gantt": {"axisColor": line},
                "mindmap": {
                    "labelBackgroundColor": canvas,
                    "labelTextColor": text
                },
                "zenuml": {
                    "backgroundColor": canvas,
                    "surfaceColor": surface,
                    "textColor": text,
                    "lineColor": line,
                    "fontFamily": "Inter, sans-serif"
                }
            }
        },
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
fn bindings_apply_explicit_light_and_dark_configuration_to_reported_paint_surfaces() {
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
fn source_and_site_overrides_retain_their_own_configuration_owners() {
    let mut opts = options(true, "resvg-safe");
    opts["site_config"]["themeVariables"]["wardley"]["anchorLabelColor"] = json!("#abcdef");
    opts["site_config"]["themeVariables"]["wardley"]["fontFamily"] = json!("Georgia, serif");
    opts["site_config"]["themeVariables"]["mindmap"]["labelBackgroundColor"] = json!("#123456");
    opts["site_config"]["themeVariables"]["gantt"]["axisColor"] = json!("#c0ffee");
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
fn empty_typed_theme_preserves_plain_output_for_affected_families() {
    for source in [C4, ZENUML, WARDLEY, GANTT, XY, MINDMAP] {
        let opts = json!({"svg":{"diagram_id": ID}});
        let mut empty = opts.clone();
        empty["theme"] = json!({"spec": {}});
        assert_eq!(render(source, &opts), render(source, &empty));
    }
}

#[test]
fn wardley_annotation_text_and_background_have_independent_paint_owners() {
    let opts = json!({
        "site_config":{"themeVariables":{"wardley":{
            "backgroundColor":"#ffffff",
            "annotationBackgroundColor":"#111111",
            "annotationLabelColor":"#ffffff"
        }}},
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

#[test]
fn usecase_source_fonts_override_only_their_matching_host_shape_default() {
    let source = "---\nconfig:\n  usecase:\n    actorFontFamily: Georgia\n---\nusecase-beta\nactor A(Reader)\nB(Open)\nA-->B";
    let opts = json!({
        "site_config": {
            "htmlLabels": false,
            "usecase": {
                "actorFontFamily": "Inter, sans-serif",
                "usecaseFontFamily": "Inter, sans-serif",
                "actorFontSize": 18,
                "usecaseFontSize": 18
            }
        },
        "svg": {"diagram_id": ID}
    });
    let svg = render(source, &opts);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let root_style = document.root_element().attribute("style").unwrap();
    for expected in [
        "--mermaid-usecase-actor-font-family: Georgia;",
        "--mermaid-usecase-font-family: Inter, sans-serif;",
        "--mermaid-usecase-actor-font-size: 18px;",
        "--mermaid-usecase-font-size: 18px;",
    ] {
        assert!(root_style.contains(expected), "{root_style}");
    }
}
