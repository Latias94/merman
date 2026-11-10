#![cfg(feature = "diagram-flowchart")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::model::FlowchartLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::json;

const BACKENDS: &[&str] = if cfg!(feature = "layout-elk") {
    &["dagre", "elk"]
} else {
    &["dagre"]
};

fn source(title: &str) -> String {
    format!("flowchart TD\nsubgraph G[\"`{title}`\"]\nA[A] --> B[B]\nend\n")
}

fn render(source: &str, backend: &str, html_labels: bool) -> (FlowchartLayout, String) {
    if backend == "elk" {
        assert!(merman_render::layout_elk_available());
    }
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "layout": backend,
                "look": "classic",
                "fontFamily": "Arial",
                "themeVariables": {"fontFamily": "Arial", "fontSize": "14px"},
                "htmlLabels": html_labels,
                "flowchart": {"wrappingWidth": 80, "diagramPadding": 0}
            })),
        )
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Flowchart cluster")
        .expect("detect Flowchart")
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare Flowchart cluster");
    let layout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone())
            .expect("Flowchart layout");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Flowchart cluster");
    (layout, rendered.svg().to_owned())
}

fn cluster<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    id: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-et") == Some("cluster")
                && node.attribute("data-id") == Some(id)
        })
        .expect("the semantic subgraph's actual cluster")
}

fn number(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
    node.attribute(attribute)
        .expect("geometry attribute")
        .parse()
        .expect("numeric geometry")
}

fn property<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    node.attribute("style")?
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim())
        .next_back()
}

#[cfg(feature = "layout-elk")]
fn translation(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let coordinates = node
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split([',', ' '])
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(coordinates.len(), 2);
    (coordinates[0], coordinates[1])
}

#[test]
fn flowchart_html_cluster_keeps_wrapping_after_long_word_expands_min_content() {
    let title = format!("prefix {} suffix", "W".repeat(36));
    for backend in BACKENDS {
        let (layout, svg) = render(&source(&title), backend, true);
        let uses_elk = *backend == "elk";
        let allocated = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "G")
            .unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let cluster = cluster(&document, "G");
        let label = cluster
            .children()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .unwrap();
        let foreign_object = label
            .children()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let cap = if uses_elk { allocated.width } else { 200.0 };
        assert!(
            number(foreign_object, "width") > cap,
            "{backend}: min-content exceeds the original constraint"
        );
        assert_eq!(property(div, "display"), Some("table"), "{backend}");
        assert_eq!(
            property(div, "white-space"),
            Some("break-spaces"),
            "{backend}"
        );
        for name in ["width", "max-width"] {
            let actual: f64 = property(div, name)
                .unwrap()
                .strip_suffix("px")
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                (actual - cap).abs() < 1e-6,
                "{backend}: keep the original wrapping constraint"
            );
        }
        let text = div
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(text, title);
        if !uses_elk {
            let frame = cluster
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            assert_eq!(
                number(frame, "width"),
                allocated.width,
                "Dagre frame remains layout-owned"
            );
            assert_eq!(number(frame, "height"), allocated.height);
            assert_eq!(number(frame, "x"), allocated.x - allocated.width / 2.0);
            assert_eq!(number(frame, "y"), allocated.y - allocated.height / 2.0);
            assert_eq!(number(foreign_object, "width"), allocated.title_label.width);
            assert_eq!(
                number(foreign_object, "height"),
                allocated.title_label.height
            );
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_cluster_expands_painted_frame_and_viewport_around_long_html_title() {
    let title = format!("prefix {} suffix", "W".repeat(36));
    let (layout, svg) = render(&source(&title), "elk", true);
    let allocated = layout
        .clusters
        .iter()
        .find(|cluster| cluster.id == "G")
        .unwrap();
    let document = roxmltree::Document::parse(&svg).unwrap();
    let cluster = cluster(&document, "G");
    let frame = cluster
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    let label = cluster
        .children()
        .find(|node| node.attribute("class") == Some("cluster-label"))
        .unwrap();
    let foreign_object = label
        .children()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    let label_width = number(foreign_object, "width");
    let frame_width = number(frame, "width");
    assert!(label_width > allocated.width);
    assert!((frame_width - label_width - allocated.padding).abs() < 1e-6);
    let (label_x, label_y) = translation(label);
    let left_margin = label_x - number(frame, "x");
    let right_margin = number(frame, "x") + frame_width - label_x - label_width;
    assert!((left_margin - allocated.padding / 2.0).abs() < 1e-6);
    assert!((right_margin - left_margin).abs() < 1e-6);
    assert_eq!(number(frame, "height"), allocated.height);
    assert_eq!(number(frame, "y"), allocated.y - allocated.height / 2.0);

    let viewbox = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    assert!(viewbox[0] <= number(frame, "x") + 1e-6);
    assert!(viewbox[0] + viewbox[2] >= number(frame, "x") + frame_width - 1e-6);
    assert!(viewbox[1] <= label_y + 1e-6);
    assert!(viewbox[1] + viewbox[3] >= label_y + number(foreign_object, "height") - 1e-6);
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_side_edges_attach_to_the_expanded_html_cluster_frame() {
    use base64::Engine as _;

    let title = format!("prefix {} suffix", "W".repeat(36));
    for (connection, incoming) in [("Outside --> G", true), ("G --> Outside", false)] {
        // Keep the outside node beyond the expanded title without enlarging the
        // single-child provider frame through an internal inter-layer gap.
        let source = format!(
            "---\nconfig:\n  flowchart:\n    rankSpacing: 400\n---\nflowchart LR\nsubgraph G[\"`{title}`\"]\nA[A]\nend\n{connection}\n"
        );
        let (layout, svg) = render(&source, "elk", true);
        let allocated = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "G")
            .unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let frame = cluster(&document, "G")
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert!(number(frame, "width") > allocated.width, "{connection}");
        let outside = layout
            .nodes
            .iter()
            .find(|node| node.id == "Outside")
            .unwrap();
        assert!(
            if incoming {
                outside.x + outside.width / 2.0 < number(frame, "x")
            } else {
                outside.x - outside.width / 2.0 > number(frame, "x") + number(frame, "width")
            },
            "{connection}: the outside node must remain clear of the expanded frame"
        );

        let edge = layout
            .edges
            .iter()
            .find(|edge| {
                if incoming {
                    edge.from == "Outside" && edge.to == "G"
                } else {
                    edge.from == "G" && edge.to == "Outside"
                }
            })
            .unwrap();
        let path = document
            .descendants()
            .find(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-edge") == Some("true")
                    && node.attribute("data-id") == Some(edge.id.as_str())
            })
            .unwrap();
        assert!(path.attribute("marker-end").is_some(), "{connection}");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap();
        let points: Vec<merman_render::model::LayoutPoint> =
            serde_json::from_slice(&decoded).unwrap();
        assert!(points.len() >= 2, "{connection}: {points:?}");
        // data-points retains the clipped endpoint before the visible path is
        // shortened for its arrow marker, so no marker-specific offset is needed.
        let endpoint = if incoming {
            points.last().unwrap()
        } else {
            points.first().unwrap()
        };
        let expected_x = number(frame, "x")
            + if incoming {
                0.0
            } else {
                number(frame, "width")
            };
        assert!(
            (endpoint.x - expected_x).abs() < 1e-6,
            "{connection}: endpoint {endpoint:?} must reach final frame side {expected_x}"
        );
        assert!(endpoint.y >= number(frame, "y") - 1e-6, "{connection}");
        assert!(
            endpoint.y <= number(frame, "y") + number(frame, "height") + 1e-6,
            "{connection}"
        );
    }
}

#[test]
fn flowchart_explicit_cluster_size_preserves_provider_frame_and_expands_viewport() {
    let source = format!("{}style G width:640px,height:720px\n", source("Ready"));
    for backend in BACKENDS {
        for html_labels in [false, true] {
            let (layout, svg) = render(&source, backend, html_labels);
            let allocated = layout
                .clusters
                .iter()
                .find(|cluster| cluster.id == "G")
                .unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let cluster = cluster(&document, "G");
            let frame = cluster
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            assert_eq!(number(frame, "width"), allocated.width);
            assert_eq!(number(frame, "height"), allocated.height);
            assert_eq!(property(frame, "width"), Some("640px !important"));
            assert_eq!(property(frame, "height"), Some("720px !important"));
            let viewbox = document
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            assert!(viewbox[0] <= number(frame, "x") + 1e-6);
            assert!(viewbox[1] <= number(frame, "y") + 1e-6);
            assert!(viewbox[0] + viewbox[2] >= number(frame, "x") + 640.0 - 1e-6);
            assert!(viewbox[1] + viewbox[3] >= number(frame, "y") + 720.0 - 1e-6);
            if html_labels {
                let foreign_object = cluster
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .unwrap();
                let div = foreign_object
                    .children()
                    .find(|node| node.has_tag_name("div"))
                    .unwrap();
                assert!(number(foreign_object, "width") + allocated.padding < allocated.width);
                assert_eq!(property(div, "white-space"), Some("nowrap"));
                assert_eq!(property(div, "display"), Some("table-cell"));
                assert_eq!(property(div, "width"), None);
            }
        }
    }
}

#[test]
fn flowchart_svg_cluster_titles_keep_their_existing_frame_path() {
    let title = format!("prefix {} suffix", "W".repeat(36));
    for backend in BACKENDS {
        let (layout, svg) = render(&source(&title), backend, false);
        let allocated = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "G")
            .unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let cluster = cluster(&document, "G");
        let frame = cluster
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        let label = cluster
            .children()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .unwrap();
        assert_eq!(number(frame, "width"), allocated.width, "{backend}");
        assert_eq!(number(frame, "height"), allocated.height, "{backend}");
        assert_eq!(number(frame, "x"), allocated.x - allocated.width / 2.0);
        assert_eq!(number(frame, "y"), allocated.y - allocated.height / 2.0);
        assert!(
            label.descendants().any(|node| node.has_tag_name("text")),
            "{backend}"
        );
        assert!(
            !label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            "{backend}"
        );
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_nested_long_title_keeps_parent_reservation_and_expands_root_bounds() {
    let title = format!("prefix {} suffix", "W".repeat(36));
    for parent_title in ["Outer", "`Outer`"] {
        let source = format!(
            "flowchart TD\nsubgraph Outer[\"{parent_title}\"]\nsubgraph G[\"`{title}`\"]\nA[A] --> B[B]\nend\nend\n"
        );
        let (layout, svg) = render(&source, "elk", true);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let parent = cluster(&document, "Outer");
        let child = cluster(&document, "G");
        let parent_frame = parent
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        let child_frame = child
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        let allocated_parent = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "Outer")
            .unwrap();
        let allocated_child = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "G")
            .unwrap();
        // Mermaid's final clusters.js title expansion does not re-run the
        // provider's parent reservation. Preserve that source-backed limitation.
        assert_eq!(number(parent_frame, "width"), allocated_parent.width);
        assert_eq!(
            number(parent_frame, "x"),
            allocated_parent.x - allocated_parent.width / 2.0
        );
        assert!(number(child_frame, "width") > allocated_child.width);
        let label = child
            .children()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .unwrap();
        let foreign_object = label
            .children()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert!(
            (number(child_frame, "width")
                - number(foreign_object, "width")
                - allocated_child.padding)
                .abs()
                < 1e-6
        );
        let viewbox = document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        for frame in [parent_frame, child_frame] {
            assert!(viewbox[0] <= number(frame, "x") + 1e-6);
            assert!(viewbox[0] + viewbox[2] >= number(frame, "x") + number(frame, "width") - 1e-6);
        }
    }
}
