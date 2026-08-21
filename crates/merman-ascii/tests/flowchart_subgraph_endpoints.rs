mod support;

use merman_ascii::{AsciiColorMode, AsciiRenderOptions};
use merman_core::diagrams::flowchart::FlowNodeProvenance;
use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use support::render_parsed;

const SUBGRAPH_ENDPOINT_FIXTURE: &str = include_str!(
    "../../../fixtures/flowchart/upstream_cypress_flowchart_v2_spec_should_render_subgraphs_with_title_margins_and_edge_labels_063.mmd"
);

fn render_flowchart(source: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("flowchart should parse")
        .expect("flowchart should be detected");
    render_parsed(&parsed, &AsciiRenderOptions::ascii())
        .expect("flowchart should render as terminal output")
}

#[test]
fn upstream_subgraph_endpoint_edges_route_to_groups_without_visible_placeholder_nodes() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(SUBGRAPH_ENDPOINT_FIXTURE, ParseOptions::strict())
        .expect("upstream subgraph endpoint fixture should parse")
        .expect("upstream subgraph endpoint fixture should be detected");
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("upstream subgraph endpoint fixture should produce a flowchart model");
    };
    for endpoint_id in ["TOP", "B1", "B2"] {
        let endpoint = model
            .nodes
            .iter()
            .find(|node| node.id == endpoint_id)
            .unwrap_or_else(|| panic!("missing endpoint node {endpoint_id:?}"));
        assert_eq!(
            endpoint.provenance,
            FlowNodeProvenance::SubgraphAnchor,
            "subgraph endpoints must carry explicit provenance"
        );
        assert!(
            model
                .subgraphs
                .iter()
                .any(|subgraph| subgraph.id == endpoint_id)
        );
    }
    let rendered = render_parsed(&parsed, &AsciiRenderOptions::ascii())
        .expect("upstream subgraph endpoint fixture should render");

    for title in ["TOP", "B1", "B2"] {
        assert_eq!(
            rendered.matches(title).count(),
            1,
            "subgraph {title:?} should own its endpoint id without a visible placeholder node:\n{rendered}"
        );
    }
    for label in ["lb1", "lb2", "lb3", "lb4", "lb5"] {
        assert!(
            rendered.contains(label),
            "subgraph endpoint routing should preserve edge label {label:?}:\n{rendered}"
        );
    }
}

#[test]
fn authored_node_colliding_with_a_subgraph_uses_group_first_projection() {
    let source = concat!(
        "flowchart-elk TD\n",
        "G\n",
        "subgraph G\n",
        "  A\n",
        "end\n",
        "G --> A\n",
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("authored node/subgraph collision should parse")
        .expect("flowchart should be detected");
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected a flowchart model");
    };
    let authored = model
        .nodes
        .iter()
        .find(|node| node.id == "G")
        .expect("authored G node");
    assert_eq!(authored.provenance, FlowNodeProvenance::Authored);

    let rendered = render_parsed(&parsed, &AsciiRenderOptions::ascii())
        .expect("authored node/subgraph collision should render through the group");
    assert_eq!(
        rendered.matches('G').count(),
        1,
        "Mermaid's group-first projection must not create a second leaf node:\n{rendered}"
    );
}

#[test]
fn standalone_shape_data_does_not_override_same_id_group_projection() {
    let source = concat!(
        "flowchart-elk TD\n",
        "subgraph G\n",
        "  A\n",
        "end\n",
        "G --> A\n",
        "G@{ shape: rect, label: \"Authored G\" }\n",
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("standalone shapeData should parse")
        .expect("flowchart should be detected");
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected a flowchart model");
    };
    let authored = model
        .nodes
        .iter()
        .find(|node| node.id == "G")
        .expect("authored G node");
    assert_eq!(authored.provenance, FlowNodeProvenance::Authored);

    let rendered = render_parsed(&parsed, &AsciiRenderOptions::ascii())
        .expect("standalone shapeData should retain the group-first projection");
    assert!(
        !rendered.contains("Authored G"),
        "same-id vertex labels do not replace the Mermaid subgraph title:\n{rendered}"
    );
    assert_eq!(rendered.matches('G').count(), 1, "{rendered}");
}

#[test]
fn style_statement_on_a_colliding_vertex_remains_on_the_group_owner() {
    let source = concat!(
        "flowchart TD\n",
        "G\n",
        "subgraph G\n",
        "  A\n",
        "end\n",
        "style G fill:#ff0000\n",
        "G --> A\n",
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("styled node/subgraph collision should parse")
        .expect("flowchart should be detected");
    let rendered = render_parsed(
        &parsed,
        &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Html),
    )
    .expect("same-id vertex style should project through the group");

    assert!(
        rendered.contains("background-color:#ff0000"),
        "the vertex inline style must survive the group-first projection:\n{rendered}"
    );
}

#[test]
fn same_id_group_style_preserves_flowdb_class_and_style_statement_order() {
    let render = |statements: &str| {
        let source = format!(
            "flowchart TD\nclassDef base stroke:#0000ff\nsubgraph G\n  A\nend\n{statements}\n"
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .expect("same-id group style order should parse")
            .expect("flowchart should be detected");
        let rendered = render_parsed(
            &parsed,
            &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::TrueColor),
        )
        .expect("same-id group style order should render");
        rendered
    };

    let class_before_style_rendered = render("class G base\nstyle G fill:#ff0000");
    assert!(
        class_before_style_rendered.contains("\u{1b}[48;2;255;0;0m"),
        "vertex fill must replace the group CSS source:\n{class_before_style_rendered}"
    );
    assert!(
        !class_before_style_rendered.contains("\u{1b}[38;2;0;0;255m"),
        "a class assigned before vertex creation must not survive:\n{class_before_style_rendered}"
    );

    let style_before_class_rendered = render("style G fill:#ff0000\nclass G base");
    assert!(
        style_before_class_rendered.contains("\u{1b}[48;2;255;0;0m"),
        "vertex fill must remain visible:\n{style_before_class_rendered}"
    );
    assert!(
        style_before_class_rendered.contains("\u{1b}[38;2;0;0;255m"),
        "a class assigned after vertex creation must remain visible:\n{style_before_class_rendered}"
    );
}

#[test]
fn standalone_empty_subgraph_renders_a_real_group_perimeter() {
    let rendered = render_flowchart(concat!(
        "flowchart TD\n",
        "subgraph Empty[Empty Group]\n",
        "end\n",
    ));

    assert_eq!(rendered.matches("Empty Group").count(), 1, "{rendered}");
    assert!(
        rendered.contains('+'),
        "empty group should draw corners:\n{rendered}"
    );
    assert!(
        rendered.contains('|'),
        "empty group should draw sides:\n{rendered}"
    );
}

#[test]
fn empty_subgraph_endpoint_keeps_rank_and_routes_to_its_perimeter() {
    let rendered = render_flowchart(concat!(
        "flowchart TD\n",
        "subgraph Empty[Empty Group]\n",
        "end\n",
        "Empty -->|handoff| Done\n",
    ));

    assert_eq!(rendered.matches("Empty Group").count(), 1, "{rendered}");
    assert!(
        rendered.contains("Done"),
        "target node should remain visible:\n{rendered}"
    );
    assert!(
        rendered.contains("handoff"),
        "edge should route from the empty group perimeter:\n{rendered}"
    );
}

#[test]
fn nested_empty_subgraph_is_the_parent_endpoint_rank_anchor() {
    let rendered = render_flowchart(concat!(
        "flowchart TD\n",
        "subgraph Outer\n",
        "  subgraph Inner\n",
        "  end\n",
        "end\n",
        "Outer -->|exit| Done\n",
    ));

    for title in ["Outer", "Inner", "Done", "exit"] {
        assert!(rendered.contains(title), "missing {title:?}:\n{rendered}");
    }
    assert_eq!(rendered.matches("Outer").count(), 1, "{rendered}");
    assert_eq!(rendered.matches("Inner").count(), 1, "{rendered}");
}
