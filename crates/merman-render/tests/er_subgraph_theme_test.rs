use merman_core::{MermaidEngine, ParseOptions};

use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::{Value, json};

const LAYOUTS: &[&str] = &[
    "dagre",
    #[cfg(feature = "layout-elk")]
    "elk",
    "swimlane",
];

fn render_classic_cluster(config: Value, source_style: &str) -> merman_render::Result<String> {
    let source = format!(
        "---\nconfig: {config}\n---\nerDiagram\nsubgraph Orders [\"**Order** Domain\"]\n CUSTOMER\nend\nstyle CUSTOMER color:#556677\n{source_style}\n"
    );
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({"secure": []})),
        )
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ER cluster")
        .expect("detect ER cluster")
        .into_presentation();
    assert_eq!(
        parsed
            .metadata()
            .effective_config
            .as_value()
            .pointer("/themeVariables/titleColor"),
        config.pointer("/themeVariables/titleColor"),
        "the fixture must retain its explicit titleColor owner or fallback"
    );
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin ER cluster session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare ER cluster")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn dagre_duplicate_subgraphs_keep_first_box_style_and_class_without_svg_source_text_color() {
    let source = r##"---
config:
  layout: dagre
  htmlLabels: false
---
erDiagram
classDef first fill:#112233,stroke:#445566,color:#778899
classDef last fill:#aabbcc,stroke:#ddeeff,color:#abcdef
subgraph Shared ["First title"]
 FIRST
end
class Shared first
style Shared stroke-width:2px
subgraph Shared ["Last title"]
 LAST
end
class Shared last
style Shared stroke-width:7px
"##;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse duplicate ER clusters")
        .expect("detect ER diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin ER session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Dagre duplicate clusters")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render duplicate ER clusters");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid SVG");
    let clusters: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("cluster first"))
        .collect();
    assert!(
        !clusters.is_empty(),
        "the writer must select the first subgraph class"
    );
    for cluster in clusters {
        let rect = cluster
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("cluster rectangle");
        let style = rect.attribute("style").expect("prepared source style");
        assert!(style.contains("fill:#112233 !important"), "{style}");
        assert!(style.contains("stroke:#445566 !important"), "{style}");
        assert!(style.contains("stroke-width:2px !important"), "{style}");
        assert!(!style.contains("#aabbcc"), "{style}");
        let label = cluster
            .descendants()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .expect("cluster label");
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .expect("SVG title");
        // The shared upstream cluster painter applies class color only to HTML spans.
        assert_eq!(label.attribute("style"), Some(""));
        assert_eq!(text.attribute("style"), Some(""));
        let title: String = text
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        assert!(title.contains("First"), "{title}");
        assert!(!title.contains("Last"), "{title}");
    }
    assert!(
        !document
            .descendants()
            .any(|node| node.attribute("class") == Some("cluster last"))
    );
}

#[test]
fn cluster_source_label_styles_stay_on_html_spans_and_do_not_leak_to_svg_or_member_labels() {
    let cases = [
        (
            "style Orders color:#ff0000,color:#0000ff",
            "color:#0000ff !important",
        ),
        (
            "style Orders color:#0000ff,color:#ff0000",
            "color:#ff0000 !important",
        ),
        ("style Orders color:#c0ffee", "color:#c0ffee !important"),
        (
            "style Orders font-family:SourceFace",
            "font-family:SourceFace !important",
        ),
        ("style Orders font-size:30px", "font-size:30px !important"),
        ("style Orders font-weight:900", "font-weight:900 !important"),
        ("", ""),
    ];
    for layout in LAYOUTS {
        for html_labels in [false, true] {
            for (source_style, html_style) in cases {
                let svg = render_classic_cluster(
                    json!({
                        "layout": layout,
                        "look": "classic",
                        "htmlLabels": html_labels,
                        "flowchart": {"htmlLabels": html_labels},
                        "themeVariables": {"titleColor": "#112233"}
                    }),
                    source_style,
                )
                .expect("render ER subgraph source-label-style control");
                let document = roxmltree::Document::parse(&svg).expect("valid ER SVG");
                let cluster = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("merman-Orders"))
                    .expect("authored Orders cluster");
                let label = cluster
                    .descendants()
                    .find(|node| {
                        node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "cluster-label")
                        })
                    })
                    .expect("cluster title label");
                let context = format!("{layout}, htmlLabels={html_labels}, {source_style:?}");
                assert_eq!(
                    label
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html_labels,
                    "{context}"
                );
                if html_labels {
                    let span = label
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("span")
                                && node.attribute("class") == Some("nodeLabel")
                        })
                        .expect("HTML cluster title span");
                    assert_eq!(
                        span.attribute("style").unwrap_or(""),
                        html_style,
                        "{context}"
                    );
                } else {
                    let text = label
                        .descendants()
                        .find(|node| node.has_tag_name("text"))
                        .expect("SVG cluster title text");
                    assert_eq!(label.attribute("style").unwrap_or(""), "", "{context}");
                    assert_eq!(text.attribute("style").unwrap_or(""), "", "{context}");
                    for terminal in label.descendants().filter(|node| node.is_element()) {
                        assert_eq!(terminal.attribute("font-family"), None, "{context}");
                        assert_eq!(terminal.attribute("font-size"), None, "{context}");
                        let style = terminal.attribute("style").unwrap_or("");
                        assert!(!style.contains("font-family"), "{context}: {style}");
                        assert!(!style.contains("font-size"), "{context}: {style}");
                        assert!(!style.contains("font-weight"), "{context}: {style}");
                    }
                    assert!(svg.contains("fill:#112233"), "{context}: title theme role");
                }
                let entity = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .attribute("id")
                                .is_some_and(|id| id.starts_with("merman-entity-CUSTOMER-"))
                    })
                    .expect("independently styled member entity");
                let entity_label = entity
                    .descendants()
                    .find(|node| node.attribute("class") == Some("label"))
                    .expect("member entity label");
                let entity_style = if html_labels {
                    "color:#556677 !important"
                } else {
                    "fill:#556677 !important"
                };
                assert_eq!(
                    entity_label.attribute("style"),
                    Some(entity_style),
                    "{context}"
                );
            }
        }
    }
}
