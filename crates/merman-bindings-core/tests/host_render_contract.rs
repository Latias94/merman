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
const ISSUE_QUADRANT: &str = "---\nconfig:\n  theme: base\n  themeVariables:\n    quadrant1Fill: '#e8f5e9'\n    quadrant2Fill: '#e3f2fd'\n    quadrant3Fill: '#f5f5f5'\n    quadrant4Fill: '#ffebee'\n---\nquadrantChart\ntitle Value vs. cost\nx-axis Low cost --> High cost\ny-axis Low value --> High value\nquadrant-1 Plan\nquadrant-2 Do now\nquadrant-3 Maybe\nquadrant-4 Drop\nAlpha: [0.25, 0.75]\nBeta: [0.7, 0.8]\nGamma: [0.3, 0.3]\nDelta: [0.75, 0.25]";
const ISSUE_ER: &str = "%%{init: {'theme':'forest'}}%%\nerDiagram\nCUSTOMER ||--o{ ORDER : places\nCUSTOMER {\n  string id PK\n  string name\n}\nORDER {\n  string id PK\n  string customer_id FK\n}";

fn preset_options(preset: Option<&str>, pipeline: &str) -> Value {
    let mut options = json!({"svg": {"diagram_id": ID, "pipeline": pipeline}});
    if let Some(preset) = preset {
        options["theme"] = json!({"preset": preset, "precedence": "source"});
    }
    options
}

fn options(dark: bool, pipeline: &str) -> Value {
    let canvas = if dark { "#1e1f22" } else { "#ffffff" };
    let surface = if dark { "#2b2d30" } else { "#f4f4f5" };
    let text = if dark { "#dfe1e5" } else { "#1f2937" };
    let border = if dark { "#6f737a" } else { "#64748b" };
    let line = if dark { "#a8adbd" } else { "#334155" };
    json!({
        "site_config": {
            "theme": "base",
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
                "fontFamily": "Inter, sans-serif",
                "textColor": text,
                "primaryColor": surface,
                "primaryTextColor": text,
                "nodeTextColor": text,
                "lineColor": line,
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

#[cfg(feature = "ascii")]
#[test]
fn gantt_ascii_hosts_preserve_source_config_records_and_the_selected_projection() {
    let body = concat!(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery work\n",
        "Build phase: done, build, 2026-01-01, 2d\n",
        "Review phase: crit, active, review, after build, 1d\n",
    );
    for (compact_mode, width) in [(false, 60_u64), (false, 59), (true, 59)] {
        let gantt_config = if compact_mode {
            "  gantt:\n    displayMode: compact\n"
        } else {
            ""
        };
        let source = format!(
            "---\ntitle: Host release plan\nconfig:\n  theme: dark\n{gantt_config}---\n{body}"
        );
        let options = serde_json::to_vec(&json!({
            "ascii": {"colorMode": "plain", "overflow": "fallback", "maxWidth": width},
        }))
        .unwrap();
        let engine = merman_bindings_core::BindingEngine::from_options(&options).unwrap();
        let cached = engine.render_ascii_result(source.as_bytes()).unwrap();
        let once = merman_bindings_core::render_ascii_result(source.as_bytes(), &options).unwrap();
        assert_eq!(
            cached.data(),
            once.data(),
            "compact {compact_mode}, width {width}"
        );
        let cached_metadata: Value = serde_json::from_slice(cached.metadata_json()).unwrap();
        let once_metadata: Value = serde_json::from_slice(once.metadata_json()).unwrap();
        assert_eq!(cached_metadata["output_plan"], once_metadata["output_plan"]);
        let plan = &cached_metadata["output_plan"];
        let fallback = !compact_mode && width < 60;
        assert_eq!(plan["kind"], "ascii");
        assert_eq!(plan["schema_version"], 4);
        assert_eq!(plan["family"], "gantt");
        assert_eq!(plan["layout_profile"], "canonical");
        assert_eq!(plan["compact_attempted"], false);
        assert_eq!(plan["fallback_attempted"], fallback);
        assert_eq!(
            plan["outcome"],
            if fallback { "fallback" } else { "primary" }
        );
        assert_eq!(
            plan["projection"],
            if compact_mode || fallback {
                "structured_text"
            } else {
                "diagrammatic"
            }
        );
        assert_eq!(plan["emitted_overflowed"], false);
        assert!(plan["emitted_width"].as_u64().unwrap() <= width);
        let text = std::str::from_utf8(cached.data()).unwrap();
        assert!(!text.contains('\u{1b}'));
        let records: String = text
            .lines()
            .map(|line| line.strip_prefix("  ").unwrap_or(line))
            .collect();
        for expected in [
            "sourceTitle:\"Host release plan\"",
            "label=\"Build phase\"",
            "label=\"Review phase\"",
            "section=\"Delivery work\"",
            "T1 interval (UTC): 2026-01-01 -> 2026-01-03",
            "T2 interval (UTC): 2026-01-03 -> 2026-01-04",
        ] {
            assert!(records.contains(expected), "missing {expected:?}: {text}");
        }
        let compact: String = text.split_whitespace().collect();
        for expected in ["ganttdetails:{", "sourceConfig:{", "\"theme\":\"dark\""] {
            assert!(compact.contains(expected), "missing {expected:?}: {text}");
        }
        if compact_mode {
            assert!(compact.contains("\"gantt\":{\"displayMode\":\"compact\"}"));
        }
    }
}

#[cfg(all(feature = "ascii", feature = "diagram-git-graph"))]
#[test]
fn gitgraph_ascii_hosts_keep_track_records_and_the_actual_output_plan() {
    let body = "gitGraph LR:\ncommit id:\"A\"\nbranch feature\ncommit id:\"F\"\ncheckout main\ncommit id:\"B\"\nmerge feature id:\"M\" tag:\"v1\"\n";
    for (parallel, width) in [(false, 80), (false, 20), (true, 80)] {
        let source = format!(
            "---\ntitle: Host history\nconfig:\n  theme: dark\n  gitGraph:\n    parallelCommits: {parallel}\n---\n{body}"
        );
        let options = serde_json::to_vec(&json!({ "ascii": { "colorMode": "plain", "overflow": "fallback", "maxWidth": width } })).unwrap();
        let engine = merman_bindings_core::BindingEngine::from_options(&options).unwrap();
        let cached = engine.render_ascii_result(source.as_bytes()).unwrap();
        let once = merman_bindings_core::render_ascii_result(source.as_bytes(), &options).unwrap();
        assert_eq!(cached.data(), once.data());
        assert_eq!(cached.metadata_json(), once.metadata_json());
        let Some(merman_bindings_core::BindingOutputPlan::Ascii(plan)) =
            cached.metadata().output_plan()
        else {
            panic!("missing ASCII plan")
        };
        assert_eq!(plan.family(), "gitGraph");
        assert_eq!(
            plan.projection(),
            if !parallel && width == 80 {
                "diagrammatic"
            } else {
                "structured_text"
            }
        );
        assert_eq!(
            plan.outcome(),
            if width == 20 { "fallback" } else { "primary" }
        );
        assert_eq!(plan.fallback_attempted(), width == 20);
        assert!(plan.emitted_width() <= width);
        let records: String = std::str::from_utf8(cached.data())
            .unwrap()
            .split_once("GitGraph records")
            .unwrap()
            .1
            .split_whitespace()
            .collect();
        assert_eq!(records.matches("[commit]").count(), 4);
        assert!(records.contains("parents=[\"B\",\"F\"]"));
        assert!(records.contains("sourceTitle:\"Hosthistory\""));
        assert!(records.contains("\"theme\":\"dark\""));
    }
}

#[cfg(feature = "ascii")]
#[test]
fn zenuml_ascii_hosts_keep_source_records_and_the_complete_width_fallback() {
    let source = format!("---\ntitle: Host ZenUML\nconfig:\n  theme: dark\n---\n{ZENUML}");
    for width in [None, Some(10)] {
        let mut options = json!({"ascii": {"colorMode": "plain", "overflow": "fallback"}});
        if let Some(width) = width {
            options["ascii"]["maxWidth"] = json!(width);
        }
        let options = serde_json::to_vec(&options).unwrap();
        let engine = merman_bindings_core::BindingEngine::from_options(&options).unwrap();
        let cached = engine.render_ascii_result(source.as_bytes()).unwrap();
        let once = merman_bindings_core::render_ascii_result(source.as_bytes(), &options).unwrap();
        assert_eq!(cached.data(), once.data(), "width {width:?}");
        let cached_metadata: Value = serde_json::from_slice(cached.metadata_json()).unwrap();
        let once_metadata: Value = serde_json::from_slice(once.metadata_json()).unwrap();
        assert_eq!(cached_metadata["output_plan"], once_metadata["output_plan"]);
        let plan = &cached_metadata["output_plan"];
        assert_eq!(plan["kind"], "ascii");
        assert_eq!(plan["schema_version"], 4);
        assert_eq!(plan["family"], "zenuml");
        assert_eq!(plan["encoding"], "plain");
        assert_eq!(plan["layout_profile"], "canonical");
        assert_eq!(plan["requested_graph_presentation"], "strict");
        assert_eq!(plan["compact_attempted"], false);
        assert_eq!(plan["fallback_capability"], "available");
        assert_eq!(plan["fallback_attempted"], width.is_some());
        assert_eq!(plan["primary_overflowed"], width.is_some());
        assert_eq!(plan["emitted_overflowed"], false);
        assert_eq!(
            plan["projection"],
            if width.is_some() {
                "structured_text"
            } else {
                "diagrammatic"
            }
        );
        assert_eq!(
            plan["outcome"],
            if width.is_some() {
                "fallback"
            } else {
                "primary"
            }
        );
        if let Some(width) = width {
            assert!(plan["emitted_width"].as_u64().unwrap() <= width);
        }
        let text = std::str::from_utf8(cached.data()).unwrap();
        assert!(!text.contains('\u{1b}'));
        let compact: String = text.split_whitespace().collect();
        for expected in [
            "zenumldetails:{",
            "sourceTitle:\"HostZenUML\"",
            "sourceConfig:{",
            "\"theme\":\"dark\"",
            "Alice",
            "Bob",
            "Hello",
            "Reply",
            "zenuml-statement-0",
            "zenuml-statement-1",
        ] {
            assert!(compact.contains(expected), "missing {expected:?}: {text}");
        }
    }
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

#[test]
fn source_precedence_preserves_quadrant_and_er_paint() {
    for preset in [None, Some("studio"), Some("tokyo")] {
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
