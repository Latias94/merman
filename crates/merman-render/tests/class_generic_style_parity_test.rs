use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::ClassDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

const LAYOUTS: &[&str] = if cfg!(feature = "layout-elk") {
    &["dagre", "elk"]
} else {
    &["dagre"]
};

fn render_class(body: &str, layout: &str, html_labels: bool) -> (ClassDiagramLayout, String) {
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: classic\n  htmlLabels: {html_labels}\n---\nclassDiagram\n{body}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Class parity regression")
        .expect("Class diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let prepared = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Class parity regression");
    let projection = prepared.layout_json().expect("Class layout projection");
    let layout = serde_json::from_value(projection["layout"]["ClassDiagramV2"].clone())
        .expect("Class layout");
    let rendered = prepared
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Class parity regression");
    (layout, rendered.svg().to_owned())
}

fn class_node<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    id: &str,
) -> roxmltree::Node<'a, 'input> {
    let prefix = format!("merman-classId-{id}-");
    document
        .descendants()
        .filter(|node| node.has_tag_name("g"))
        .find(|node| {
            node.attribute("id")
                .is_some_and(|id| id.starts_with(&prefix))
        })
        .expect("named Class node")
}

fn outer_fill_path<'a, 'input>(node: roxmltree::Node<'a, 'input>) -> roxmltree::Node<'a, 'input> {
    node.children()
        .find(|child| child.attribute("class") == Some("basic label-container outer-path"))
        .expect("Class outer container")
        .children()
        .find(|child| child.has_tag_name("path") && child.attribute("stroke") == Some("none"))
        .expect("Class outer fill path")
}

#[test]
fn class_group_generic_matches_same_text_control_in_leaves_and_namespaces() {
    for layout in LAYOUTS {
        for html_labels in [false, true] {
            for namespaced in [false, true] {
                let render = |body: &str| {
                    let body = if namespaced {
                        format!("namespace group {{\n{body}}}\n")
                    } else {
                        body.to_owned()
                    };
                    render_class(&body, layout, html_labels)
                };
                let (probe_layout, probe_svg) = render("class A~group~\n");
                let (control_layout, control_svg) = render("class A[\"A&lt;group&gt;\"]\n");
                let probe = probe_layout
                    .nodes
                    .iter()
                    .find(|node| node.id == "A")
                    .expect("generic class A layout node");
                let control = control_layout
                    .nodes
                    .iter()
                    .find(|node| node.id == "A")
                    .expect("same-text control A layout node");
                let context = format!("{layout}, htmlLabels={html_labels}, namespace={namespaced}");

                assert!(!probe.is_cluster, "{context}");
                assert_eq!(probe.width, control.width, "{context}");
                assert_eq!(probe.height, control.height, "{context}");
                assert!(
                    control.width < 500.0,
                    "short control must be content-sized: {context}"
                );

                let probe_document = roxmltree::Document::parse(&probe_svg).unwrap();
                let control_document = roxmltree::Document::parse(&control_svg).unwrap();
                let probe_node = class_node(&probe_document, "A");
                let control_node = class_node(&control_document, "A");
                assert_eq!(
                    outer_fill_path(probe_node).attribute("d"),
                    outer_fill_path(control_node).attribute("d"),
                    "painted class bounds must follow the matching layout: {context}"
                );
                let title_text = |node: roxmltree::Node<'_, '_>| {
                    let title = node
                        .descendants()
                        .find(|child| child.attribute("class") == Some("label-group text"))
                        .expect("Class title group");
                    assert_eq!(
                        title
                            .descendants()
                            .any(|child| child.has_tag_name("foreignObject")),
                        html_labels,
                        "{context}"
                    );
                    title
                        .descendants()
                        .filter(|child| child.is_text())
                        .filter_map(|child| child.text())
                        .collect::<String>()
                };
                assert_eq!(
                    title_text(probe_node),
                    title_text(control_node),
                    "{context}"
                );

                if namespaced {
                    let cluster = probe_layout
                        .clusters
                        .iter()
                        .find(|cluster| cluster.id == "group")
                        .expect("structural namespace cluster");
                    let control_cluster = control_layout
                        .clusters
                        .iter()
                        .find(|cluster| cluster.id == "group")
                        .unwrap();
                    assert_eq!(cluster.width, control_cluster.width, "{context}");
                    assert_eq!(cluster.height, control_cluster.height, "{context}");
                    assert!(cluster.width > probe.width, "{context}");
                    assert!(cluster.height > probe.height, "{context}");
                    assert!(
                        probe_document.descendants().any(|node| {
                            node.attribute("id") == Some("merman-group")
                                && node.attribute("class").is_some_and(|classes| {
                                    classes.split_whitespace().any(|class| class == "cluster")
                                })
                        }),
                        "namespace must remain a painted cluster: {context}"
                    );
                } else {
                    assert!(probe_layout.clusters.is_empty(), "{context}");
                }
            }
        }
    }
}

#[test]
fn class_cssclass_raw_targets_reach_only_the_matching_svg_nodes() {
    for layout in LAYOUTS {
        for html_labels in [false, true] {
            for (targets, styled_b) in [("A, B", false), ("A,B", true)] {
                let body = format!(
                    "class A\nclass B\ncssClass \"{targets}\" accent\nclassDef accent fill:#ff0000\n"
                );
                let (_, svg) = render_class(&body, layout, html_labels);
                let document = roxmltree::Document::parse(&svg).unwrap();
                for (id, styled) in [("A", true), ("B", styled_b)] {
                    let node = class_node(&document, id);
                    let context = format!("{layout}, htmlLabels={html_labels}, {targets}: {id}");
                    assert_eq!(
                        node.attribute("class")
                            .unwrap()
                            .split_whitespace()
                            .any(|class| class == "accent"),
                        styled,
                        "{context}"
                    );
                    assert_eq!(
                        outer_fill_path(node).attribute("fill") == Some("#ff0000"),
                        styled,
                        "source fill must follow the raw target: {context}"
                    );
                }
            }
        }
    }
}
