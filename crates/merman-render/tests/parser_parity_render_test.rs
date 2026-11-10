#![cfg(any(
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-class",
    feature = "diagram-gantt",
    feature = "diagram-sequence",
    feature = "diagram-state"
))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::Value;

fn prepare_and_render(source: &str) -> (Value, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse source-backed syntax")
        .expect("detect selected family")
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare parsed semantics");
    let projection = artifact.layout_json().unwrap().into_unmanaged_value();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render prepared semantics")
        .svg()
        .to_owned();
    (projection["semantic"].clone(), svg)
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn style_created_flowchart_vertices_keep_order_without_duplicating_subgraphs() {
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        let (model, svg) = prepare_and_render(&format!(
            concat!(
                "---\nconfig:\n  layout: {layout}\n  look: classic\n---\n",
                "flowchart TD\nstyle Ghost fill:#abcdef\nA-->B\n",
                "linkStyle 0 stroke:#eeaabb, fill:blue\n",
                "subgraph G\nC\nend\nstyle G stroke:#123456\nD\n",
            ),
            layout = layout,
        ));
        assert_eq!(
            model["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|node| node["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Ghost", "A", "B", "C", "G", "D"]
        );
        assert_eq!(
            model["edges"][0]["style"],
            serde_json::json!(["stroke:#eeaabb", " fill:blue", "fill:none"])
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let clusters = document
            .descendants()
            .filter(|node| {
                node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "cluster")
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(clusters.len(), 1, "{layout}");
        assert!(
            clusters[0]
                .descendants()
                .any(|node| node.is_text() && node.text() == Some("G"))
        );
        let nodes = document
            .descendants()
            .filter(|node| {
                node.attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "node"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            nodes.len(),
            5,
            "same-ID vertex must not duplicate its subgraph"
        );
        assert!(
            document.descendants().any(|node| node
                .attribute("style")
                .is_some_and(|style| style.contains("#abcdef"))),
            "style-created node must reach SVG"
        );
        assert!(
            document.descendants().any(|node| node
                .attribute("style")
                .is_some_and(|style| style.contains("#123456"))),
            "same-ID vertex CSS must reach its subgraph"
        );
        let edge = document
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .expect("rendered Flowchart edge");
        let style = edge.attribute("style").unwrap();
        assert!(
            style.contains("fill:blue") && style.contains("fill:none"),
            "{layout}: {style}"
        );
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_aggregation_and_sanitized_member_modifiers_reach_svg() {
    let (model, svg) = prepare_and_render(concat!(
        "classDiagram\nA --o`目标`\nclass A {\n",
        "+value$ <!--hint-->\n+static$\n}\n",
    ));
    assert_eq!(model["relations"][0]["id2"], "目标");
    assert_eq!(model["relations"][0]["relation"]["type2"], 0);
    assert_eq!(model["classes"]["A"]["members"][0]["id"], "value$");
    assert_eq!(model["classes"]["A"]["members"][0]["classifier"], "");
    assert_eq!(model["classes"]["A"]["members"][1]["classifier"], "$");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(document.descendants().any(|node| {
        node.attribute("marker-end")
            .is_some_and(|marker| marker.contains("aggregation"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.is_text() && node.text().is_some_and(|text| text.contains("+value$")))
    );
}

#[cfg(feature = "diagram-gantt")]
#[test]
fn literal_gantt_end_fields_preserve_zero_duration_in_prepared_output() {
    let (model, svg) = prepare_and_render(concat!(
        "gantt\ndateFormat YYYY-MM-DD\n",
        "Base:a,2026-01-01,1d\nLiteral:b,2026-01-02,\u{0085}until a\n",
        "Dependent:c,after b,1d\n",
    ));
    let tasks = model["tasks"].as_array().unwrap();
    assert_eq!(tasks[1]["startTime"], tasks[1]["endTime"]);
    assert_eq!(tasks[2]["startTime"], tasks[1]["endTime"]);
    assert_eq!(tasks[1]["manualEndTime"], false);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let task_id = format!("{}-b", document.root_element().attribute("id").unwrap());
    let task = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("id") == Some(task_id.as_str()))
        .expect("literal-end task SVG");
    assert_eq!(
        task.attribute("width").unwrap().parse::<f64>().unwrap(),
        0.0
    );
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_method_boundaries_reach_prepared_fields_and_visible_svg_text() {
    let (model, svg) = prepare_and_render(concat!(
        "classDiagram\nclass C {\n+old\u{2028}run(arg) Result\n",
        "+keep(\u{85}arg\u{85}) \u{85}Result\u{85}$\n+trim(\u{feff}arg\u{feff}) Result  $\n}\n",
    ));
    let methods = model["classes"]["C"]["methods"].as_array().unwrap();
    assert_eq!(methods[0]["id"], "run");
    assert_eq!(methods[0]["visibility"], "");
    assert_eq!(methods[1]["parameters"], "\u{85}arg\u{85}");
    assert_eq!(methods[1]["returnType"], "\u{85}Result\u{85}");
    assert_eq!(methods[2]["parameters"], "arg");
    assert_eq!(methods[2]["returnType"], "Result  ");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        !document
            .descendants()
            .any(|node| node.is_text() && node.text().is_some_and(|text| text.contains("+old")))
    );
    assert!(document.descendants().any(|node|node.is_text()&&node.text().is_some_and(|text|text.contains("run(arg)"))));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn comma_click_links_and_inline_edge_classes_reach_svg() {
    let (model, svg) = prepare_and_render(concat!(
        "flowchart TD\nclassDef hot stroke:#123abc\nA e@-->B\ne:::hot\n",
        "click A,B \"https://example.test/path\" \"tip\" _blank\n",
    ));
    assert_eq!(model["edges"][0]["classes"], serde_json::json!(["hot"]));
    for node in model["nodes"].as_array().unwrap() {
        assert_eq!(node["link"], "https://example.test/path");
    }
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("a")
                && node.attributes().any(
                    |attr| attr.name() == "href" && attr.value() == "https://example.test/path"
                ))
            .count(),
        2
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-edge") == Some("true")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains("#123abc"))
    }));
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn overflowing_sequence_numbers_keep_visible_js_number_text() {
    for (directive, expected) in [
        (
            format!("autonumber {}", "9".repeat(309)),
            vec!["Infinity", "Infinity"],
        ),
        (
            format!("autonumber 1 {}", "9".repeat(309)),
            vec!["1", "Infinity"],
        ),
    ] {
        let (_model, svg) = prepare_and_render(&format!(
            "sequenceDiagram\n{directive}\nA->>B: first\nB-->>A: second\n"
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let actual = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("sequenceNumber"))
            .map(|node| node.text().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn sequence_continuation_identity_and_wrapped_menu_reach_svg() {
    let (model, svg) = prepare_and_render(concat!(
        "sequenceDiagram\nparticipant api/v2 as Service\n",
        "links api/v2: wrap: {\"Docs\":\"https://example.test/docs\"}\n",
        "link api/v2: nowrap: Help @ https://example.test/help\n",
        "api/v2->>Other: request\n",
    ));
    assert_eq!(model["actors"]["api/v2"]["description"], "Service");
    assert_eq!(model["messages"][0]["from"], "api/v2");
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (label, url) in [
        ("Docs", "https://example.test/docs"),
        ("Help", "https://example.test/help"),
    ] {
        assert!(
            document.descendants().any(|node| {
                node.has_tag_name("a")
                    && node
                        .attributes()
                        .any(|attr| attr.name() == "href" && attr.value() == url)
                    && node
                        .descendants()
                        .any(|child| child.is_text() && child.text() == Some(label))
            }),
            "missing menu link {label}"
        );
    }
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_empty_backticks_and_style_token_whitespace_reach_svg() {
    let (model, svg) = prepare_and_render(concat!(
        "erDiagram\nE {``int ``id``PK}\n",
        "style E fill:\u{feff}#abcdef\n",
    ));
    assert_eq!(model["entities"]["E"]["attributes"][0]["name"], "id");
    assert_eq!(model["entities"]["E"]["attributes"][0]["type"], "int");
    assert_eq!(model["entities"]["E"]["cssStyles"][0], "fill:#abcdef");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(document.descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains("#abcdef"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.is_text() && node.text() == Some("id"))
    );
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_next_line_alias_block_and_complete_note_reach_prepared_output() {
    let (model, svg) = prepare_and_render(
        "stateDiagram-v2\nstate \"Container\" as Outer\n{\nA\n}\nOuter --> Done\n",
    );
    let nodes = model["nodes"].as_array().unwrap();
    assert!(
        nodes
            .iter()
            .any(|node| node["id"] == "Outer" && node["isGroup"] == true)
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["id"] == "A" && node["parentId"] == "Outer")
    );
    assert!(svg.contains("Container"));

    let expected = "first  \n    indented\n\n      deepest\n  last";
    let (model, svg) = prepare_and_render(concat!(
        "stateDiagram-v2\nA\nnote right of A\n",
        "  first  \n    indented\n\n      deepest\n  last  \nend note\nA --> B\n",
    ));
    assert!(
        model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| { node["shape"] == "note" && node["label"] == expected })
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for word in ["first", "indented", "deepest", "last"] {
        assert!(
            document.descendants().any(|node| {
                node.is_text() && node.text().is_some_and(|text| text.contains(word))
            })
        );
    }
}
