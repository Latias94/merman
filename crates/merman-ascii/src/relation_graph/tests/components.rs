use super::*;

#[test]
fn relation_components_split_disconnected_relation_subgraphs() {
    let boxes = [
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        RelationGraphBox::new("d".to_string(), vec!["D".to_string()], 1),
        RelationGraphBox::new("isolated".to_string(), vec!["I".to_string()], 1),
    ];
    let edges = vec![
        LayeredRelationEdge::new("a", "b", 0, 0),
        LayeredRelationEdge::new("c", "d", 0, 0),
    ];

    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let components =
        relation_components(&boxes, &edges, &mut resources).expect("components should split");
    let component_box_ids = components
        .iter()
        .map(|component| {
            component
                .boxes()
                .iter()
                .map(|relation_box| relation_box.id())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let component_edge_indices = components
        .iter()
        .map(|component| component.edge_indices().to_vec())
        .collect::<Vec<_>>();

    assert_eq!(
        component_box_ids,
        vec![vec!["a", "b"], vec!["c", "d"], vec!["isolated"]]
    );
    assert_eq!(component_edge_indices, vec![vec![0], vec![1], vec![]]);
}

#[test]
fn disconnected_component_rendering_borrows_non_clone_relations() {
    let boxes = vec![
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        RelationGraphBox::new("d".to_string(), vec!["D".to_string()], 1),
        RelationGraphBox::new("isolated".to_string(), vec!["I".to_string()], 1),
    ];
    let relations = vec![
        NonCloneTestRelation {
            source_id: "a",
            target_id: "b",
        },
        NonCloneTestRelation {
            source_id: "c",
            target_id: "d",
        },
    ];
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::None,
    };
    let projected_box = boxes[0].shared_projection();
    let shared_line = boxes[0].lines[0].clone();

    assert!(Rc::ptr_eq(&boxes[0].id, &projected_box.id));
    assert!(Rc::ptr_eq(&boxes[0].lines, &projected_box.lines));
    assert!(Rc::ptr_eq(&boxes[0].lines[0].line, &shared_line.line));

    let lines =
        render_relation_component_lines(&boxes, &relations, &options, &mut resources, &adapter)
            .expect("disconnected components should render")
            .expect("non-empty components should produce lines");
    let rendered = render_lines_with_options(&lines, &options, &mut resources)
        .expect("component lines should encode");

    for label in ["A", "B", "C", "D", "I"] {
        assert!(rendered.contains(label), "missing {label:?}: {rendered:?}");
    }
}

#[test]
fn render_layered_relation_component_propagates_grid_resource_errors() {
    let boxes = vec![
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
    ];
    let relations = vec![("a", "b")];
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::None,
    };

    let options = AsciiRenderOptions::ascii()
        .with_resource_limit(AsciiResourceLimitId::MaxGridCells, 1)
        .expect("test resource limit should be valid");
    let error = render_layered_relation_component(&boxes, &relations, &options, 1, &adapter)
        .expect_err("grid resource errors must not become summary fallback");

    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxGridCells
    ));
    assert_eq!(adapter.summary_reason.get(), None);
}

#[test]
fn render_layered_relation_component_uses_summary_when_route_path_overlaps_box() {
    let boxes = vec![
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
    ];
    let relations = vec![("a", "b")];
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::Route,
    };

    let rendered = render_layered_relation_component(
        &boxes,
        &relations,
        &AsciiRenderOptions::ascii(),
        1,
        &adapter,
    )
    .expect("route-overlapping layered relation should render as a summary");

    assert_eq!(
        adapter.summary_reason.get(),
        Some(LayeredRelationSummaryReason::RouteCollision)
    );
    assert!(rendered.contains("relations:\nA --> B\n"));
}

#[test]
fn render_layered_relation_component_uses_summary_when_overlay_overlaps_box() {
    let boxes = vec![
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
    ];
    let relations = vec![("a", "b"), ("a", "c")];
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::Overlay,
    };

    let rendered = render_layered_relation_component(
        &boxes,
        &relations,
        &AsciiRenderOptions::ascii(),
        1,
        &adapter,
    )
    .expect("overlay-overlapping layered relation should render as a summary");

    assert_eq!(
        adapter.summary_reason.get(),
        Some(LayeredRelationSummaryReason::OverlayCollision)
    );
    assert!(rendered.contains("relations:\nA --> B\nA --> B\n"));
}
