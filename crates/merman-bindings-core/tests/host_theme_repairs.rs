#![cfg(all(
    feature = "all-diagrams",
    feature = "svg",
    feature = "layout-cytoscape"
))]

use merman_bindings_core::render_svg;
use serde_json::{Value, json};

const ID: &str = "issue-theme";
const ISSUE_C4: &str = r#"C4Context
title C4 boundary
Person(user, "User", "Uses the system")
Enterprise_Boundary(corp, "Company") {
    System(app, "App", "Main system")
    SystemDb_Ext(db, "Database", "Stores data")
}
Rel(user, app, "Uses")
Rel(app, db, "Reads")"#;
const ISSUE_GANTT: &str = "gantt\ntitle Task states\ndateFormat YYYY-MM-DD\naxisFormat %b %d\nsection Work\nDone task :done, t1, 2026-09-01, 5d\nActive task :active, t2, after t1, 5d\nNormal task :t3, after t2, 5d\nCritical task :crit, t4, after t3, 5d\nActive critical :crit, active, t5, after t4, 5d\nDone critical :crit, done, t6, after t5, 5d";
const ISSUE_WARDLEY: &str = "wardley-beta\ntitle Tea Shop\nanchor Business [0.95, 0.63]\nanchor Public [0.95, 0.78]\ncomponent Cup of Tea [0.79, 0.61]\ncomponent Water [0.38, 0.82]\nBusiness -> Cup of Tea\nCup of Tea -> Water";
const ISSUE_ZENUML: &str = "zenuml\ntitle Checkout\n@Actor User\n@Control Shop\n@Database Store\nUser->Shop.order() {\n    Store.save()\n}";
const ISSUE_QUADRANT: &str = "---\nconfig:\n  theme: base\n  themeVariables:\n    quadrant1Fill: '#e8f5e9'\n    quadrant2Fill: '#e3f2fd'\n    quadrant3Fill: '#f5f5f5'\n    quadrant4Fill: '#ffebee'\n---\nquadrantChart\ntitle Value vs. cost\nx-axis Low cost --> High cost\ny-axis Low value --> High value\nquadrant-1 Plan\nquadrant-2 Do now\nquadrant-3 Maybe\nquadrant-4 Drop\nAlpha: [0.25, 0.75]\nBeta: [0.7, 0.8]\nGamma: [0.3, 0.3]\nDelta: [0.75, 0.25]";
const ISSUE_ER: &str = "%%{init: {'theme':'forest'}}%%\nerDiagram\nCUSTOMER ||--o{ ORDER : places\nCUSTOMER {\n  string id PK\n  string name\n}\nORDER {\n  string id PK\n  string customer_id FK\n}";

fn preset_options(preset: Option<&str>, pipeline: &str) -> Value {
    let mut options = json!({"svg": {"diagram_id": ID, "pipeline": pipeline}});
    if let Some(preset) = preset {
        options["presentation"] = json!({"theme": {"preset": preset}});
    }
    options
}

fn render(source: &str, options: &Value) -> String {
    String::from_utf8(render_svg(source.as_bytes(), &serde_json::to_vec(options).unwrap()).unwrap())
        .unwrap()
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|item| item == class))
}

fn text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

fn text_labeled<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    label: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| node.has_tag_name("text") && text_content(*node) == label)
        .unwrap_or_else(|| panic!("missing visible text {label:?}"))
}

fn inline_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(|entry| entry.split_once(':'))
        .filter(|(key, _)| key.trim() == property)
        .map(|(_, value)| value.trim().trim_end_matches("!important").trim())
        .next_back()
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
fn pure_editor_presets_keep_c4_children_visible_and_color_relationship_terminals() {
    for (preset, text, line, background) in [
        (None, "#444444", "#444444", None),
        (Some("editor-light"), "#0f172a", "#64748b", Some("#f1f5f9")),
        (Some("editor-dark"), "#e5e7eb", "#94a3b8", Some("#1e293b")),
    ] {
        for pipeline in ["parity", "resvg-safe"] {
            let svg = render(ISSUE_C4, &preset_options(preset, pipeline));
            let document = roxmltree::Document::parse(&svg).unwrap();
            for label in ["Company", "Uses", "Reads"] {
                assert_eq!(
                    explicit_fill(text_labeled(&document, label)),
                    text,
                    "{preset:?} {pipeline}: {label}"
                );
            }
            let app = document
                .descendants()
                .find(|node| node.attribute("id") == Some("issue-theme-app"))
                .unwrap();
            assert_eq!(
                explicit_fill(text_labeled(&document, "App")),
                if preset.is_some() { text } else { "#FFFFFF" }
            );
            let backgrounds = document
                .descendants()
                .filter(|node| has_class(*node, "c4-boundary-background"))
                .collect::<Vec<_>>();
            if let Some(background) = background {
                assert_eq!(backgrounds.len(), 1);
                assert_eq!(backgrounds[0].attribute("fill"), Some(background));
                assert!(
                    backgrounds[0].range().start < app.range().start,
                    "the opaque boundary must paint before its children"
                );
                assert!(
                    app.range().start < text_labeled(&document, "Company").range().start,
                    "the boundary label must remain above its children"
                );
            } else {
                assert!(
                    backgrounds.is_empty(),
                    "plain C4 retains its transparent boundary"
                );
            }
            let relations = document
                .descendants()
                .filter(|node| node.attribute("marker-end").is_some())
                .collect::<Vec<_>>();
            assert_eq!(relations.len(), 2);
            for relation in relations {
                assert_eq!(relation.attribute("stroke"), Some(line));
                if preset.is_some() {
                    let marker_id = relation
                        .attribute("marker-end")
                        .unwrap()
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let marker = document
                        .descendants()
                        .find(|node| node.attribute("id") == Some(marker_id))
                        .unwrap();
                    let path = marker
                        .children()
                        .find(|node| node.has_tag_name("path"))
                        .unwrap();
                    assert_eq!(path.attribute("fill"), Some(line));
                    assert_eq!(path.attribute("stroke"), Some(line));
                }
            }
        }
    }
}

#[test]
fn legacy_editor_presets_preserve_gantt_state_palettes() {
    for (preset, normal, active, done, critical, critical_border, text) in [
        (
            None,
            "#8a90dd",
            "#bfc7ff",
            "lightgrey",
            "red",
            "#ff8888",
            "#333",
        ),
        (
            Some("editor-light"),
            "#f8fafc",
            "#f1f5f9",
            "#e2e8f0",
            "#e2e8f0",
            "#dc2626",
            "#0f172a",
        ),
        (
            Some("editor-dark"),
            "#111827",
            "#334155",
            "#1f2937",
            "#1f2937",
            "#f87171",
            "#e5e7eb",
        ),
    ] {
        for pipeline in ["parity", "resvg-safe"] {
            let svg = render(ISSUE_GANTT, &preset_options(preset, pipeline));
            let document = roxmltree::Document::parse(&svg).unwrap();
            for (id, class, fill) in [
                ("t1", "done0", done),
                ("t2", "active0", active),
                ("t3", "task0", normal),
                ("t4", "crit0", critical),
                ("t5", "activeCrit0", active),
                ("t6", "doneCrit0", done),
            ] {
                let terminal_id = format!("{ID}-{id}");
                let task = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect")
                            && node.attribute("id") == Some(terminal_id.as_str())
                    })
                    .unwrap();
                assert!(has_class(task, class), "{preset:?} {pipeline}: {id}");
                assert_eq!(
                    css_property(&document, &format!("#{ID} .{class}"), "fill"),
                    fill,
                    "{preset:?} {pipeline}: {id}"
                );
                if ["t4", "t5", "t6"].contains(&id) {
                    assert_eq!(
                        css_property(&document, &format!("#{ID} .{class}"), "stroke"),
                        critical_border
                    );
                }
                let terminal_id = format!("{ID}-{id}-text");
                let label = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("text")
                            && node.attribute("id") == Some(terminal_id.as_str())
                    })
                    .unwrap();
                assert!(!text_content(label).is_empty());
            }
            if preset.is_some() {
                for class in [
                    "taskText0",
                    "activeText0",
                    "doneText0",
                    "activeCritText0",
                    "doneCritText0",
                ] {
                    assert_eq!(
                        css_property(&document, &format!("#{ID} .{class}"), "fill")
                            .trim_end_matches("!important"),
                        text
                    );
                }
            }
            assert_ne!(normal, active);
            assert_ne!(active, done);
        }
    }
}

#[test]
fn pure_editor_presets_color_wardley_anchors_and_zenuml_foregrounds() {
    for (preset, text, actor, line, lifeline, canvas) in [
        (None, "#222", "#fff", "#000", "#666", "#fff"),
        (
            Some("editor-light"),
            "#0f172a",
            "#f8fafc",
            "#64748b",
            "#94a3b8",
            "#ffffff",
        ),
        (
            Some("editor-dark"),
            "#e5e7eb",
            "#1f2937",
            "#94a3b8",
            "#475569",
            "#0f172a",
        ),
    ] {
        for pipeline in ["parity", "resvg-safe"] {
            let mut opts = preset_options(preset, pipeline);
            let svg = render(ISSUE_WARDLEY, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for label in ["Business", "Public"] {
                let node = text_labeled(&document, label);
                assert!(has_class(node, "wardley-node-label"));
                assert_eq!(
                    explicit_fill(node),
                    if preset.is_some() { text } else { "#000" },
                    "{preset:?} {pipeline}: {label}"
                );
            }
            if preset.is_some() {
                opts["presentation"]["theme"]["font_family"] = json!("Inter, sans-serif");
                opts["presentation"]["theme"]["font_size"] = json!("20px");
            }
            let svg = render(ISSUE_ZENUML, &opts);
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                text_content(text_labeled(&document, "Checkout")),
                "Checkout"
            );
            if preset.is_some() {
                for (class, property, expected) in [
                    ("participant-label", "font-family", "Inter,sans-serif"),
                    ("participant-label", "font-size", "20px"),
                    ("message-label", "font-size", "17.5px"),
                ] {
                    assert_eq!(
                        css_property(&document, &format!("#{ID} .{class}"), property),
                        expected
                    );
                }
            }
            for (class, property, expected) in [
                ("frame-border-inner", "fill", canvas),
                ("frame-title", "fill", text),
                ("participant-box", "fill", actor),
                ("participant-label", "fill", text),
                ("message-label", "fill", text),
                ("message-line", "stroke", line),
                ("lifeline", "stroke", lifeline),
            ] {
                assert!(
                    document
                        .descendants()
                        .any(|node| node.is_element() && has_class(node, class)),
                    "the original ZenUML input must reach {class}"
                );
                assert_eq!(
                    css_property(&document, &format!("#{ID} .{class}"), property),
                    expected,
                    "{preset:?} {pipeline}: {class}"
                );
            }
        }
    }
}

#[test]
fn source_selected_palettes_retain_quadrant_and_er_paint_under_pure_editor_presets() {
    for pipeline in ["parity", "resvg-safe"] {
        let opts = preset_options(None, pipeline);
        let mut empty = opts.clone();
        empty["presentation"] = json!({});
        for source in [
            ISSUE_C4,
            ISSUE_GANTT,
            ISSUE_WARDLEY,
            ISSUE_ZENUML,
            ISSUE_QUADRANT,
            ISSUE_ER,
        ] {
            assert_eq!(render(source, &opts), render(source, &empty));
        }
    }
    for preset in [None, Some("editor-light"), Some("editor-dark")] {
        for pipeline in ["parity", "resvg-safe"] {
            let opts = preset_options(preset, pipeline);
            let engine = merman_bindings_core::BindingEngine::from_options(
                &serde_json::to_vec(&opts).unwrap(),
            )
            .unwrap();
            let plain_body = "quadrantChart\nAlpha: [0.25, 0.75]";
            let before = engine.render_svg(plain_body.as_bytes()).unwrap();
            let svg =
                String::from_utf8(engine.render_svg(ISSUE_QUADRANT.as_bytes()).unwrap()).unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            for label in ["Alpha", "Beta", "Gamma", "Delta"] {
                assert_eq!(
                    explicit_fill(text_labeled(&document, label)),
                    "#333",
                    "{preset:?} {pipeline}: {label}"
                );
            }
            for fill in ["#e8f5e9", "#e3f2fd", "#f5f5f5", "#ffebee"] {
                assert!(
                    document
                        .descendants()
                        .any(|node| node.has_tag_name("rect")
                            && node.attribute("fill") == Some(fill)),
                    "the authored quadrant must retain {fill}"
                );
            }
            let svg = String::from_utf8(engine.render_svg(ISSUE_ER.as_bytes()).unwrap()).unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                css_property(&document, &format!("#{ID} .entityBox"), "fill"),
                "#cde498"
            );
            assert_eq!(
                css_property(&document, &format!("#{ID} .label"), "color"),
                "#000000"
            );
            let attributes = document
                .descendants()
                .filter(|node| has_class(*node, "attribute-name"))
                .collect::<Vec<_>>();
            assert_eq!(attributes.len(), 4);
            for attribute in attributes {
                for property in ["fill", "color"] {
                    assert_eq!(
                        attribute
                            .attribute("style")
                            .and_then(|style| inline_property(style, property)),
                        None,
                        "a typed host foreground must not override the forest label"
                    );
                }
                if pipeline == "resvg-safe" {
                    let fallback = attribute
                        .descendants()
                        .find(|node| {
                            node.attribute("data-merman-foreignobject") == Some("fallback")
                        })
                        .unwrap();
                    let content = fallback_content(&document, fallback);
                    let label = content
                        .descendants()
                        .find(|node| node.has_tag_name("text"))
                        .unwrap();
                    assert!(!text_content(label).is_empty());
                    assert_eq!(explicit_fill(label), "#000000");
                } else {
                    assert!(
                        attribute
                            .descendants()
                            .any(|node| node.has_tag_name("foreignObject"))
                    );
                    assert!(!text_content(attribute).is_empty());
                }
            }
            assert_eq!(
                engine.render_svg(plain_body.as_bytes()).unwrap(),
                before,
                "source palette authority must not leak into a later request"
            );
        }
    }
}
