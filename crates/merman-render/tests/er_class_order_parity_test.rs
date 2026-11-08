#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const PROVIDERS: &[&str] = &[
    "dagre",
    #[cfg(feature = "layout-elk")]
    "elk",
    "swimlane",
];
const CLASS_DEFINITIONS: &str = concat!(
    "classDef red fill:red,color:crimson,font-size:18px\n",
    "classDef blue fill:blue,stroke:navy,color:teal,font-size:22px\n",
);

fn render(body: &str, provider: &str, html_labels: bool) -> (Value, String) {
    let source = format!(
        "---\nconfig:\n  layout: {provider}\n  look: classic\n  htmlLabels: {html_labels}\n  flowchart:\n    htmlLabels: {html_labels}\n  themeVariables:\n    titleColor: '#112233'\n    fontSize: 16px\n---\nerDiagram\n{CLASS_DEFINITIONS}{body}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ER class-order fixture")
        .expect("detect ER diagram")
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin deterministic ER session"),
    )
    .expect("prepare ER class-order fixture");
    let layout = json!(
        artifact
            .layout_json()
            .expect("serialize ER class-order layout")
    );
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("er-class-order".into()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render ER class-order fixture")
        .svg()
        .to_owned();
    (layout["semantic"].clone(), svg)
}

fn has_class(node: Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_whitespace().any(|value| value == class))
}

fn assert_style_property(node: Node<'_, '_>, property: &str, value: &str) {
    let style = node.attribute("style").expect("source-owned inline style");
    let mut values = style.split(';').filter_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name.trim() == property).then(|| value.trim())
    });
    assert_eq!(
        values.next(),
        Some(format!("{value} !important").as_str()),
        "{property} on {node:?}"
    );
    assert!(
        values.next().is_none(),
        "duplicate {property} declarations: {style}"
    );
}

fn assert_entity_and_subgraph_styles(
    svg: &str,
    html_labels: bool,
    fill: &str,
    color: &str,
    font_size: &str,
) {
    let document = Document::parse(svg).expect("well-formed ER class-order SVG");
    for (id, label_class, title) in [
        ("er-class-order-entity-A-0", "label", "A"),
        ("er-class-order-Group", "cluster-label", "Group"),
    ] {
        let group = document
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
            .unwrap_or_else(|| panic!("missing rendered ER target {id}"));
        let rectangles: Vec<_> = group
            .children()
            .filter(|node| node.has_tag_name("rect"))
            .collect();
        assert_eq!(
            rectangles.len(),
            if has_class(group, "swimlane") { 2 } else { 1 },
            "{id} must retain its entity, cluster, or lane rectangles"
        );
        for rectangle in rectangles {
            assert_style_property(rectangle, "fill", fill);
            assert_style_property(rectangle, "stroke", "navy");
        }
        let label = group
            .children()
            .find(|node| node.has_tag_name("g") && has_class(*node, label_class))
            .expect("rendered ER target label");
        assert_eq!(
            label
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<String>(),
            title
        );
        assert_eq!(
            label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            html_labels
        );
        if label_class == "label" || html_labels {
            let style_owner = if label_class == "label" {
                label
            } else {
                label
                    .descendants()
                    .find(|node| node.has_tag_name("span") && has_class(*node, "nodeLabel"))
                    .expect("HTML cluster title style owner")
            };
            assert_style_property(
                style_owner,
                if html_labels { "color" } else { "fill" },
                color,
            );
            assert_style_property(style_owner, "font-size", font_size);
        } else {
            let text = label
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .expect("SVG cluster title text");
            assert_eq!(label.attribute("style").unwrap_or_default(), "");
            assert_eq!(text.attribute("style").unwrap_or_default(), "");
            assert!(document.descendants().any(|node| {
                node.has_tag_name("style")
                    && node.text().is_some_and(|css| {
                        css.contains("#er-class-order .cluster-label text{fill:#112233;}")
                    })
            }));
            for node in label.descendants().filter(Node::is_element) {
                assert!(node.attribute("font-size").is_none(), "{node:?}");
                assert!(node.attribute("font-family").is_none(), "{node:?}");
                let style = node.attribute("style").unwrap_or_default();
                assert!(!style.contains("font-size:"), "{style}");
                assert!(!style.contains("font-family:"), "{style}");
            }
        }
    }
}

#[test]
fn er_classes_follow_last_assignment_order_for_entities_and_subgraphs() {
    for provider in PROVIDERS {
        for html_labels in [false, true] {
            for (classes, mutation, expected_classes, fill, color, font_size) in [
                ("red,blue,red", "", "red blue red", "red", "crimson", "18px"),
                (
                    "red,blue",
                    "class A red\nclass Group red\n",
                    "red blue red",
                    "red",
                    "crimson",
                    "18px",
                ),
                ("red,blue", "", "red blue", "blue", "teal", "22px"),
            ] {
                let body = format!(
                    "subgraph Group\nA:::{classes}\nB\nend\nclass Group {classes}\n{mutation}A ||--|| B : relates\n"
                );
                let (semantic, svg) = render(&body, provider, html_labels);
                assert_eq!(
                    semantic["entities"]["A"]["cssClasses"],
                    format!("default {expected_classes}"),
                    "{provider}, HTML={html_labels}, {classes}, {mutation}"
                );
                assert_eq!(
                    semantic["subgraphs"][0]["classes"],
                    json!(expected_classes.split_whitespace().collect::<Vec<_>>())
                );
                assert_entity_and_subgraph_styles(&svg, html_labels, fill, color, font_size);
            }
        }
    }
}

#[test]
fn er_inline_source_styles_outrank_reapplied_classes_in_both_statement_orders() {
    let mutation = "class A red\nclass Group red\n";
    let inline_styles = concat!(
        "style A fill:green,color:lime,font-size:26px\n",
        "style Group fill:green,color:lime,font-size:26px\n",
    );
    for provider in PROVIDERS {
        for html_labels in [false, true] {
            for suffix in [
                format!("{inline_styles}{mutation}"),
                format!("{mutation}{inline_styles}"),
            ] {
                let body = format!(
                    "subgraph Group\nA:::red,blue\nB\nend\nclass Group red,blue\n{suffix}A ||--|| B : relates\n"
                );
                let (semantic, svg) = render(&body, provider, html_labels);
                assert_eq!(
                    semantic["entities"]["A"]["cssClasses"],
                    "default red blue red"
                );
                assert_eq!(
                    semantic["subgraphs"][0]["classes"],
                    json!(["red", "blue", "red"])
                );
                for styles in [
                    &semantic["entities"]["A"]["cssStyles"],
                    &semantic["subgraphs"][0]["cssStyles"],
                ] {
                    assert_eq!(
                        styles,
                        &json!(["fill:green", "color:lime", "font-size:26px"])
                    );
                }
                assert_entity_and_subgraph_styles(&svg, html_labels, "green", "lime", "26px");
            }
        }
    }
}
