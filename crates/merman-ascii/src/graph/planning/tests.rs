use super::*;
use crate::AsciiRenderOptions;
use crate::graph::model::{GraphDirection, GraphEdgeMarker, GraphNodeShape};
use crate::resource::AsciiResourcePolicy;

#[test]
fn capacity_feedback_finishes_a_strict_diagram_with_every_independent_marker() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node_with_shape_and_style(
        "source",
        "",
        GraphNodeShape::ForkJoinHorizontal,
        Default::default(),
    );
    graph.add_node("target", "Target");
    for _ in 0..97 {
        graph.add_edge("source", "target");
        let edge = graph.edges.last_mut().unwrap();
        edge.start_marker = GraphEdgeMarker::Circle;
        edge.end_marker = GraphEdgeMarker::Open;
    }
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let policy = options.flowchart_layout().graph_policy();
    let resources_policy = AsciiResourcePolicy::default();
    let mut resources = ResourceContext::new(resources_policy);
    let execution = AsciiExecution::for_test(&resources_policy);
    let initial = layout_graph_with_feedback(
        &graph,
        &policy,
        &LayoutFeedback::default(),
        &mut resources,
        execution,
    )
    .unwrap();
    assert_eq!(initial.layout.nodes[0].width, 7);
    assert!(matches!(
        routing::prepare_route_scene_with_resources(
            &graph,
            &initial.layout,
            &graph.edges,
            &charset,
            &mut resources,
        ),
        Err(AsciiError::PortCapacityExceeded {
            required: 97,
            available: 96,
            ..
        })
    ));
    let planned = plan_graph(&graph, &policy, &charset, &mut resources, execution)
        .expect("measured capacity drives a complete fresh strict scene");
    assert_eq!(
        planned.layout.nodes[0].shape,
        GraphNodeShape::ForkJoinHorizontal
    );
    assert_eq!(planned.layout.nodes[0].width, 293);
    assert!(planned.layout.nodes[1].width >= 293);
    let diagram = crate::graph::draw::render_graph(&graph, &options, resources_policy)
        .expect("the accepted strict scene must materialize without a text projection");
    assert_eq!(
        diagram
            .chars()
            .filter(|character| *character == 'o')
            .count(),
        97
    );
    assert_eq!(diagram.matches("Target").count(), 1);
}
