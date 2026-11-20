use super::*;
use crate::AsciiError;
use crate::TerminalWidthProfile;
use crate::graph::label::GraphLabel;
use crate::graph::layout::{GridCoord, NodeLayout};
use crate::graph::model::{GraphGroupKind, GraphNodeShape};
use crate::graph::routing::occupancy::{OccupiedRect, SceneOccupancy};
use crate::graph::surface::OutputTransform;
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_model::{CancelReason, OperationControl, OperationPhase};

fn node(id: &str, x: usize, y: usize) -> NodeLayout {
    NodeLayout {
        id: id.to_string(),
        label: GraphLabel::empty_with_profile(TerminalWidthProfile::Unicode),
        shape: GraphNodeShape::Rect,
        style: Default::default(),
        grid: GridCoord { x: 0, y: 0 },
        x,
        y,
        width: 3,
        height: 3,
    }
}

fn group(id: &str, kind: GraphGroupKind, x: usize, y: usize, size: usize) -> GroupLayout {
    GroupLayout {
        id: id.to_string(),
        kind,
        title: GraphLabel::empty_with_profile(TerminalWidthProfile::Unicode),
        title_transform: OutputTransform::Identity,
        style: Default::default(),
        divider_span: None,
        x,
        y,
        width: size,
        height: size,
    }
}

fn layout(extra_nodes: usize) -> GraphLayout {
    let mut nodes = vec![node("A", 4, 4), node("B", 4, 24), node("Outside", 60, 4)];
    nodes.extend((0..extra_nodes).map(|index| node(&format!("peer-{index}"), 60 + index * 4, 40)));
    GraphLayout {
        nodes,
        groups: vec![
            group("P", GraphGroupKind::Container, 1, 1, 45),
            group("first", GraphGroupKind::Divider, 2, 2, 18),
            group("second", GraphGroupKind::Divider, 2, 22, 18),
        ],
        group_background_order: vec![0, 1, 2],
    }
}

fn scene<'layout>(
    layout: &'layout GraphLayout,
    cache: Rc<LabelRegionCache<'layout>>,
    resources: &ResourceContext,
) -> SceneOccupancy<'layout> {
    let protected = SceneOccupancy::try_prepare_protection(layout, resources).unwrap();
    let route_capacity = cache.canonical_edge_count.max(1);
    SceneOccupancy::try_new_with_protection(
        cache,
        route_capacity,
        4,
        protected,
        resources,
        AsciiExecution::for_test(&resources.policy()),
    )
    .unwrap()
}

fn repeated_work(layout: &GraphLayout, resources: &ResourceContext) -> crate::Result<()> {
    let cache = LabelRegionCache::try_new(layout, 1, resources)?;
    let owner = RouteOwner::for_test(0, "A", "B");
    for _ in 0..64 {
        assert_eq!(
            cache.classify(&owner, 0, &layout.groups[0], resources)?,
            LabelContainerRelation::Interior,
        );
    }
    assert_eq!(cache.relations.borrow().len(), 1);
    Ok(())
}

#[test]
fn repeated_label_region_classification_has_exact_work_and_boundary_admission() {
    let layout = layout(100);
    let policy = AsciiResourcePolicy::unbounded();
    let measured = ResourceContext::new(policy);
    repeated_work(&layout, &measured).unwrap();
    let work = measured.layout_work_used();
    let scan_width = layout.nodes.len() + layout.groups.len();
    assert_eq!(work, 2 * scan_width + 66);
    assert!(work < 64 * 2 * scan_width);
    for maximum in [work, work - 1] {
        let policy = policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, maximum)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let result = repeated_work(&layout, &resources);
        if maximum == work {
            result.unwrap();
            assert_eq!(resources.layout_work_used(), work);
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == work && details.max == maximum)
            );
            assert_eq!(resources.layout_work_used(), work - 1);
        }
    }
}

#[test]
fn label_region_cache_keeps_container_endpoint_and_cross_divider_semantics() {
    let layout = layout(0);
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let cache = LabelRegionCache::try_new(&layout, 5, &resources).unwrap();
    let occupancy = scene(&layout, Rc::clone(&cache), &resources);
    let inside = OccupiedRect::try_new(10, 10, 4, 1, &resources).unwrap();
    let outside = OccupiedRect::try_new(55, 10, 4, 1, &resources).unwrap();
    let cases = [
        ("P", "A", true, false),
        ("P", "Outside", false, true),
        ("A", "B", true, false),
        ("A", "Outside", true, true),
        ("P", "P", false, true),
    ];
    for (index, (from, to, inside_allowed, outside_allowed)) in cases.into_iter().enumerate() {
        let owner = RouteOwner::for_test(index, from, to);
        for _ in 0..2 {
            assert_eq!(
                occupancy
                    .label_region_is_clear(&owner, inside, &resources)
                    .unwrap(),
                inside_allowed,
            );
            assert_eq!(
                occupancy
                    .label_region_is_clear(&owner, outside, &resources)
                    .unwrap(),
                outside_allowed,
            );
        }
    }
    assert_eq!(cache.relations.borrow().len(), cases.len());
}

#[test]
fn label_region_classification_preserves_every_matching_detached_id() {
    let mut layout = layout(0);
    layout.nodes[2].id = "A".into();
    layout.nodes.swap(0, 2);
    layout.nodes.push(node("child", 70, 4));
    layout
        .groups
        .push(group("child", GraphGroupKind::Container, 28, 28, 8));
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    for canonical_edge_count in [0, 3] {
        let cache = LabelRegionCache::try_new(&layout, canonical_edge_count, &resources).unwrap();
        for (index, from) in ["A", "child", "missing"].into_iter().enumerate() {
            let owner = RouteOwner::for_test(index, from, "B");
            for _ in 0..2 {
                assert_eq!(
                    cache
                        .classify(&owner, 0, &layout.groups[0], &resources)
                        .unwrap(),
                    if from == "missing" {
                        LabelContainerRelation::Crossing
                    } else {
                        LabelContainerRelation::Interior
                    },
                );
            }
        }
        assert_eq!(cache.relations.borrow().len(), canonical_edge_count);
    }
}

#[test]
fn label_region_cache_is_shared_by_repair_but_rebuilt_for_new_geometry() {
    let layout = layout(0);
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let cache = LabelRegionCache::try_new(&layout, 1, &resources).unwrap();
    let occupancy = scene(&layout, Rc::clone(&cache), &resources);
    let owner = RouteOwner::for_test(0, "A", "B");
    let candidate = OccupiedRect::try_new(10, 10, 4, 1, &resources).unwrap();
    assert!(
        occupancy
            .label_region_is_clear(&owner, candidate, &resources)
            .unwrap()
    );
    let rebuilt = occupancy
        .rebuild_for_routes(&[], &[], &mut resources, "state")
        .unwrap();
    assert!(Rc::ptr_eq(&occupancy.label_regions, &rebuilt.label_regions));
    let before = resources.layout_work_used();
    assert!(
        rebuilt
            .label_region_is_clear(&owner, candidate, &resources)
            .unwrap()
    );
    assert_eq!(
        resources.layout_work_used() - before,
        layout.groups.len() + 1
    );
    let mut next_layout = layout.clone();
    next_layout.nodes[0].x = 60;
    let next_cache = LabelRegionCache::try_new(&next_layout, 1, &resources).unwrap();
    let next = scene(&next_layout, Rc::clone(&next_cache), &resources);
    assert!(!Rc::ptr_eq(&cache, &next_cache));
    assert_eq!(
        next_cache
            .classify(&owner, 0, &next_layout.groups[0], &resources)
            .unwrap(),
        LabelContainerRelation::Crossing,
    );
    let outside = OccupiedRect::try_new(55, 10, 4, 1, &resources).unwrap();
    assert!(
        next.label_region_is_clear(&owner, outside, &resources)
            .unwrap()
    );
    assert!(
        !occupancy
            .label_region_is_clear(&owner, outside, &resources)
            .unwrap()
    );
}

#[test]
fn label_region_cache_hits_observe_cancellation_before_resource_failure() {
    let layout = layout(0);
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let cache = LabelRegionCache::try_new(&layout, 1, &resources).unwrap();
    let owner = RouteOwner::for_test(0, "A", "B");
    cache
        .classify(&owner, 0, &layout.groups[0], &resources)
        .unwrap();
    let policy = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    let control = OperationControl::new();
    let resources =
        ResourceContext::new(policy).controlled(control.clone(), OperationPhase::Layout);
    resources.charge_layout_work(1).unwrap();
    control.cancel();
    let error = cache
        .classify(&owner, 0, &layout.groups[0], &resources)
        .unwrap_err();
    assert!(matches!(error, AsciiError::Cancelled(cancelled)
        if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested));
    assert_eq!(resources.layout_work_used(), 1);
}

#[test]
fn failed_label_region_cache_fill_does_not_allocate_and_restores_transaction_work() {
    let layout = layout(0);
    let setup = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let cache = LabelRegionCache::try_new(&layout, 1, &setup).unwrap();
    let owner = RouteOwner::for_test(0, "A", "B");
    let required = 2 * (layout.nodes.len() + layout.groups.len()) + 2;
    let policy = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, required - 1)
        .unwrap();
    let resources = ResourceContext::new(policy);
    let result =
        resources.transaction(|resources| cache.classify(&owner, 0, &layout.groups[0], resources));
    assert!(
        matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
            && details.actual == required && details.max == required - 1)
    );
    assert_eq!(resources.layout_work_used(), 0);
    assert_eq!(cache.relations.borrow().len(), 0);
    assert_eq!(cache.relations.borrow().capacity(), 0);

    cache
        .classify(&owner, 0, &layout.groups[0], &setup)
        .unwrap();
    let control = OperationControl::new();
    let policy = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    let resources =
        ResourceContext::new(policy).controlled(control.clone(), OperationPhase::Layout);
    resources.charge_layout_work(1).unwrap();
    let first = cache
        .classify(&owner, 0, &layout.groups[0], &resources)
        .unwrap_err();
    control.cancel();
    let second = cache
        .classify(&owner, 0, &layout.groups[0], &resources)
        .unwrap_err();
    assert_eq!(
        first, second,
        "a resource terminal must precede later cancellation"
    );
    assert_eq!(cache.relations.borrow().len(), 1);
    assert_eq!(resources.layout_work_used(), 1);
}
