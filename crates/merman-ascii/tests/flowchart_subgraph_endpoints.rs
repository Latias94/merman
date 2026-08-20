mod support;

use merman_ascii::AsciiRenderOptions;
use merman_core::diagrams::flowchart::FlowNodeProvenance;
use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use support::render_model;

const SUBGRAPH_ENDPOINT_FIXTURE: &str = include_str!(
    "../../../fixtures/flowchart/upstream_cypress_flowchart_v2_spec_should_render_subgraphs_with_title_margins_and_edge_labels_063.mmd"
);

fn render_flowchart(source: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("flowchart should parse")
        .expect("flowchart should be detected");
    render_model(parsed.model(), &AsciiRenderOptions::ascii())
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
    let rendered = render_model(parsed.model(), &AsciiRenderOptions::ascii())
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
fn authored_node_can_share_a_subgraph_id_without_becoming_an_anchor() {
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

    let rendered = render_model(parsed.model(), &AsciiRenderOptions::ascii())
        .expect("authored node/subgraph collision should render");
    assert_eq!(
        rendered.matches('G').count(),
        2,
        "the authored node and subgraph title must both remain visible:\n{rendered}"
    );
}

#[test]
fn standalone_shape_data_upgrades_a_prior_subgraph_anchor_to_authored() {
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

    let rendered = render_model(parsed.model(), &AsciiRenderOptions::ascii())
        .expect("standalone shapeData node should remain visible");
    assert!(
        rendered.contains("Authored G"),
        "the authored node label must remain visible:\n{rendered}"
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
