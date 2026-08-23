use super::*;
use merman_core::{CancelReason, OperationControl, OperationPhase};

fn render_test_relation_components<R>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    policy: AsciiResourcePolicy,
    adapter: &TestRelationAdapter,
) -> Result<String>
where
    R: TestRelationEndpoints,
{
    let mut resources = test_resources(policy);
    let mut deferred = DeferredTextRegistry::new();
    let control = OperationControl::new();
    render_relation_components_with_deferred_with_execution(
        boxes,
        relations,
        &AsciiRenderOptions::ascii(),
        &mut resources,
        adapter,
        &mut deferred,
        AsciiExecution::new(&control, &policy),
    )
}

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

    let mut resources = test_resources(AsciiResourcePolicy::default());
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
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::None,
    };
    let mut deferred = DeferredTextRegistry::new();
    let projected_box = boxes[0].shared_projection();
    let shared_line = boxes[0].lines[0].clone();

    assert!(Rc::ptr_eq(&boxes[0].id, &projected_box.id));
    assert!(Rc::ptr_eq(&boxes[0].lines, &projected_box.lines));
    assert!(Rc::ptr_eq(&boxes[0].lines[0].line, &shared_line.line));

    let lines = render_relation_component_lines(
        &boxes,
        &relations,
        &options,
        &mut resources,
        &adapter,
        &mut deferred,
    )
    .expect("disconnected components should render");
    let rendered = render_lines_with_deferred_options(&lines, &options, &mut resources, &deferred)
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

    let options = AsciiRenderOptions::ascii();
    let policy = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
        .expect("test resource limit should be valid");
    let mut resources = test_resources(policy);
    let Err(error) = plan_layered_relation_component_result(
        &boxes,
        &relations,
        &options,
        1,
        &mut resources,
        &adapter,
    ) else {
        panic!("grid resource errors must not become summary fallback");
    };

    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxGridCells
    ));
    assert_eq!(adapter.summary_reason.get(), None);
}

#[test]
fn cancelled_edge_admission_precedes_work_limit_without_ledger_pollution() {
    let boxes = vec![
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
    ];
    let relations = vec![("a", "b"), ("b", "c")];
    let policy = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .expect("test work limit should be valid");
    let mut resources = test_resources(policy);
    let control = OperationControl::new();
    control.cancel();
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::None,
    };
    let mut deferred = DeferredTextRegistry::new();

    let error = render_relation_component_lines_with_execution(
        &boxes,
        &relations,
        &AsciiRenderOptions::ascii(),
        &mut resources,
        &adapter,
        &mut deferred,
        AsciiExecution::new(&control, &policy),
    )
    .expect_err("cancellation must win over the first edge-work charge");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == CancelReason::Requested
    ));
    assert_eq!(resources.layout_work_used(), 0);
    assert_eq!(resources.document_cells_used(), 0);
    assert_eq!(adapter.summary_reason.get(), None);
}

#[test]
fn render_layered_relation_component_uses_summary_when_route_path_overlaps_box() {
    let boxes = vec![
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
    ];
    let relations = vec![("a", "b"), ("a", "c")];
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::Route,
    };

    let rendered = render_test_relation_components(
        &boxes,
        &relations,
        AsciiResourcePolicy::default(),
        &adapter,
    )
    .expect("route-overlapping layered relation should render as a summary");

    assert_eq!(
        adapter.summary_reason.get(),
        Some(LayeredRelationSummaryReason::RouteCollision)
    );
    assert!(rendered.contains("relations:\nA --> B\nA --> B\n"));
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

    let rendered = render_test_relation_components(
        &boxes,
        &relations,
        AsciiResourcePolicy::default(),
        &adapter,
    )
    .expect("overlay-overlapping layered relation should render as a summary");

    assert_eq!(
        adapter.summary_reason.get(),
        Some(LayeredRelationSummaryReason::OverlayCollision)
    );
    assert!(rendered.contains("relations:\nA --> B\nA --> B\n"));
}

#[test]
fn strict_k2_2_overlay_collision_keeps_speculative_work_but_discards_document_cells() {
    let boxes = strict_k2_2_boxes();
    let relations = [("a", "c"), ("a", "d"), ("b", "c"), ("b", "d")];
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::Overlay,
    };
    let box_refs = boxes.iter().collect::<Vec<_>>();
    let edges = relations
        .iter()
        .map(|relation| adapter.build_edges(relation))
        .collect::<Vec<_>>();
    let scene = match plan_layered_relation_scene(
        &box_refs,
        edges,
        4,
        options.terminal_width_profile,
        &mut resources,
    )
    .expect("strict K2,2 scene should plan")
    {
        LayeredRelationScenePlan::Routed(scene) => scene,
        LayeredRelationScenePlan::Summary(reason) => {
            panic!("strict K2,2 should not summarize before route planning: {reason:?}")
        }
    };
    let relation_refs = relations.iter().collect::<Vec<_>>();
    let work_before = resources.layout_work_used();
    let document_before = resources.document_cells_used();

    let error = plan_layered_route_batch(&scene, &relation_refs, &resources, &adapter)
        .expect_err("a colliding overlay must reject the whole K2,2 route batch");

    assert!(matches!(
        error,
        LayeredRouteBatchError::Semantic(LayeredRelationSummaryReason::OverlayCollision)
    ));
    assert!(resources.layout_work_used() > work_before);
    assert_eq!(resources.document_cells_used(), document_before);
}

#[test]
fn pairwise_validation_work_uses_the_exact_linear_prefix_formula() {
    let resources = ResourceContext::new(AsciiResourcePolicy::default());

    let planar = measure_pairwise_validation_work([(2, 1), (3, 2), (5, 4)], &resources, true)
        .expect("fixed pairwise work should fit");
    assert_eq!(
        planar,
        PairwiseValidationWork {
            segment_count: 10,
            overlay_count: 7,
            pair_work: 87,
        }
    );

    let overlays_only =
        measure_pairwise_validation_work([(2, 1), (3, 2), (5, 4)], &resources, false)
            .expect("fixed overlay work should fit");
    assert_eq!(
        overlays_only,
        PairwiseValidationWork {
            segment_count: 10,
            overlay_count: 7,
            pair_work: 14,
        }
    );
}

#[test]
fn strict_k2_2_route_batch_admits_exact_work_and_rolls_back_n_minus_one() {
    let boxes = strict_k2_2_boxes();
    let relations = [("a", "c"), ("a", "d"), ("b", "c"), ("b", "d")];
    let options = AsciiRenderOptions::ascii();
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::None,
    };

    let build_scene = |resources: &mut ResourceContext| {
        let box_refs = boxes.iter().collect::<Vec<_>>();
        let edges = relations
            .iter()
            .map(|relation| adapter.build_edges(relation))
            .collect::<Vec<_>>();
        match plan_layered_relation_scene(
            &box_refs,
            edges,
            4,
            options.terminal_width_profile,
            resources,
        )
        .expect("strict K2,2 scene should plan")
        {
            LayeredRelationScenePlan::Routed(scene) => scene,
            LayeredRelationScenePlan::Summary(reason) => {
                panic!("strict K2,2 should not summarize before route planning: {reason:?}")
            }
        }
    };
    let relation_refs = relations.iter().collect::<Vec<_>>();

    let mut measured_resources = test_resources(AsciiResourcePolicy::default());
    let measured_scene = build_scene(&mut measured_resources);
    let work_before = measured_resources.layout_work_used();
    let (plans, _) = plan_layered_route_batch(
        &measured_scene,
        &relation_refs,
        &measured_resources,
        &adapter,
    )
    .expect("unbounded K2,2 route batch should plan");
    assert_eq!(plans.len(), 4);
    let route_work = measured_resources
        .layout_work_used()
        .checked_sub(work_before)
        .expect("route work should be monotonic");

    let exact_policy = AsciiResourcePolicy::default()
        .with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            work_before + route_work,
        )
        .expect("exact work limit should be valid");
    let mut exact_resources = test_resources(exact_policy);
    let exact_scene = build_scene(&mut exact_resources);
    let exact_before = exact_resources.layout_work_used();
    assert_eq!(exact_before, work_before);
    let (exact_plans, _) =
        plan_layered_route_batch(&exact_scene, &relation_refs, &exact_resources, &adapter)
            .expect("the exact route work budget should admit");
    assert_eq!(exact_plans.len(), 4);
    assert_eq!(exact_resources.layout_work_used(), work_before + route_work);

    let below_policy = AsciiResourcePolicy::default()
        .with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            work_before + route_work - 1,
        )
        .expect("N-1 work limit should be valid");
    let mut below_resources = test_resources(below_policy);
    let below_scene = build_scene(&mut below_resources);
    let below_before = below_resources.layout_work_used();
    assert_eq!(below_before, work_before);
    let error = plan_layered_route_batch(&below_scene, &relation_refs, &below_resources, &adapter)
        .expect_err("the N-1 route work budget must reject");
    assert!(matches!(error, LayeredRouteBatchError::Resource(_)));
    assert_eq!(below_resources.layout_work_used(), below_before);
    assert_eq!(below_resources.document_cells_used(), 0);
}

#[test]
fn strict_k2_2_route_batch_observes_control_from_the_resource_ledger() {
    let boxes = strict_k2_2_boxes();
    let relations = [("a", "c"), ("a", "d"), ("b", "c"), ("b", "d")];
    let options = AsciiRenderOptions::ascii();
    let policy = AsciiResourcePolicy::default();
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::None,
    };
    let mut resources = test_resources(policy);
    let box_refs = boxes.iter().collect::<Vec<_>>();
    let edges = relations
        .iter()
        .map(|relation| adapter.build_edges(relation))
        .collect::<Vec<_>>();
    let scene = match plan_layered_relation_scene(
        &box_refs,
        edges,
        4,
        options.terminal_width_profile,
        &mut resources,
    )
    .expect("strict K2,2 scene should plan")
    {
        LayeredRelationScenePlan::Routed(scene) => scene,
        LayeredRelationScenePlan::Summary(reason) => {
            panic!("strict K2,2 should not summarize before route planning: {reason:?}")
        }
    };
    let relation_refs = relations.iter().collect::<Vec<_>>();
    let control = OperationControl::new();
    let controlled = resources.controlled(control.clone(), OperationPhase::Layout);
    let work_before = controlled.layout_work_used();
    let document_before = controlled.document_cells_used();
    control.cancel();

    let error = plan_layered_route_batch(&scene, &relation_refs, &controlled, &adapter)
        .expect_err("route planning should observe cancellation from the shared ledger");

    assert!(matches!(
        error,
        LayeredRouteBatchError::Resource(AsciiError::Cancelled(cancelled))
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == CancelReason::Requested
    ));
    assert_eq!(controlled.layout_work_used(), work_before);
    assert_eq!(controlled.document_cells_used(), document_before);
}
