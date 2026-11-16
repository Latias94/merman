#![cfg(feature = "diagram-flowchart")]

use merman_core::SemanticParser;
use merman_model::semantic::SemanticDiagramModel;

const SOURCE: &str = "graph LR\n A[\"returns &lt;T&gt;\"] -->|returns &lt;T&gt;| B\n subgraph G[\"group &lt;T&gt;\"]\n C\n end\n";

#[test]
fn semantic_flowchart_retains_entity_sources_without_graphical_preparation() {
    let parsed = SemanticParser::new().parse_diagram_sync(SOURCE).unwrap();
    let SemanticDiagramModel::Flowchart(model) = &parsed.model else {
        panic!("expected Flowchart model");
    };
    let node = model.nodes.iter().find(|node| node.id == "A").unwrap();
    assert_eq!(node.label.as_deref(), Some("returns "));
    assert_eq!(model.node_label_source(node), Some("returns &lt;T&gt;"));
    assert_eq!(model.label_sources.node_source("B"), None);
    assert_eq!(model.edges[0].label.as_deref(), Some("returns "));
    assert_eq!(
        model.edge_label_source(0, &model.edges[0]),
        Some("returns &lt;T&gt;")
    );
    let (ordinal, group) = model
        .subgraphs
        .iter()
        .enumerate()
        .find(|(_, group)| group.id == "G")
        .unwrap();
    assert_eq!(group.title, "group <T>");
    assert_eq!(
        model.subgraph_title_source(ordinal, group),
        "group &lt;T&gt;"
    );

    let canonical_json = serde_json::to_vec(model).unwrap();
    let mut without_sources = model.clone();
    without_sources.label_sources = Default::default();
    assert_eq!(
        canonical_json,
        serde_json::to_vec(&without_sources).unwrap()
    );
    assert_eq!(
        model
            .label_sources
            .validate_for_model_controlled(model, &merman_model::OperationControl::new())
            .unwrap(),
        Ok(())
    );
}

#[test]
fn semantic_label_sources_use_the_existing_sanitized_render_spelling() {
    let source = "graph LR\n A[\"<script>secret</script>returns &lt;T&gt;\"] -->|\"<script>secret</script>edge &lt;T&gt;\"| B\n subgraph G[\"<script>secret</script>group &lt;T&gt;\"]\n C\n end\n";
    let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
    let SemanticDiagramModel::Flowchart(model) = &parsed.model else {
        panic!("expected Flowchart model");
    };
    assert_eq!(
        model.label_sources.node_source("A"),
        Some("returns &lt;T&gt;")
    );
    assert_eq!(model.label_sources.edge_source(0), Some("edge &lt;T&gt;"));
    assert_eq!(
        model.label_sources.subgraph_source(0, "G"),
        Some("group &lt;T&gt;")
    );

    #[cfg(feature = "presentation")]
    {
        let rendered = merman_core::presentation::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let context = rendered.flowchart_render_context().unwrap();
        let merman_model::presentation::RenderSemanticModel::Flowchart(full) = rendered.model()
        else {
            panic!("expected Flowchart render model");
        };
        let node = full.nodes.iter().find(|node| node.id == "A").unwrap();
        assert_eq!(
            context.node_label_for_render(node),
            model.label_sources.node_source("A")
        );
        assert_eq!(
            context.edge_label_for_render(0, &full.edges[0]),
            model.label_sources.edge_source(0)
        );
        assert_eq!(
            context.subgraph_title_for_render(0, &full.subgraphs[0]),
            model.subgraph_title_source(0, &model.subgraphs[0])
        );
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn retained_entity_sources_do_not_change_public_compatibility_json() {
    let parsed = SemanticParser::new().parse_diagram_sync(SOURCE).unwrap();
    let SemanticDiagramModel::Flowchart(model) = &parsed.model else {
        panic!("expected Flowchart model");
    };
    let public = merman_core::presentation::MermaidEngine::new()
        .parse_diagram_sync(SOURCE, merman_core::ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(public.model["edges"][0]["label"], "returns ");
    let metadata = merman_model::presentation::compat::CompatJsonMetadata::from(&public.meta);
    let projected = merman_model::families::flowchart::compat::project(model, &metadata).unwrap();
    let mut without_sources = model.clone();
    without_sources.label_sources = Default::default();
    let canonical =
        merman_model::families::flowchart::compat::project(&without_sources, &metadata).unwrap();
    assert_eq!(
        serde_json::to_vec(&projected).unwrap(),
        serde_json::to_vec(&canonical).unwrap()
    );
    assert_eq!(projected, *public.model.as_value());
}
