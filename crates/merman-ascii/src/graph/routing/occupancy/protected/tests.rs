use super::*;
use crate::AsciiError;
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_model::{CancelReason, OperationControl, OperationPhase};

fn resources() -> ResourceContext {
    ResourceContext::new(AsciiResourcePolicy::unbounded())
}

fn scene(geometry: Vec<ProtectedGeometry<'static>>) -> ProtectedScene<'static> {
    let mut scene = ProtectedScene {
        geometry,
        index: Vec::new(),
        extent: CanvasCoord { x: 0, y: 0 },
    };
    scene.build_index(&resources()).unwrap();
    scene
}

fn node(x: usize, y: usize, width: usize, height: usize) -> ProtectedGeometry<'static> {
    ProtectedGeometry {
        owner_id: "node",
        group_index: None,
        kind: ProtectedKind::Node,
        shape: ProtectedShape::Rect(
            OccupiedRect::try_new(x, y, width, height, &resources()).unwrap(),
        ),
    }
}

fn relevant_at(geometry: &ProtectedGeometry<'_>, point: CanvasCoord) -> bool {
    if let (ProtectedKind::Node, ProtectedShape::Rect(rect)) = (geometry.kind, geometry.shape) {
        point.x >= rect.x.saturating_sub(1)
            && point.x <= rect.right
            && point.y >= rect.y.saturating_sub(1)
            && point.y <= rect.bottom
    } else {
        geometry.shape.contains(point)
    }
}

#[test]
fn spatial_protection_matches_full_scan_for_points_and_reservations() {
    let scene = scene(vec![
        node(0, 0, 1, 1),
        node(4, 6, 7, 5),
        node(8, 8, 2, 9),
        ProtectedGeometry {
            owner_id: "group",
            group_index: Some(0),
            kind: ProtectedKind::GroupBorder,
            shape: ProtectedShape::HorizontalSpan {
                x_start: 2,
                x_end: 25,
                y: 18,
            },
        },
        ProtectedGeometry {
            owner_id: "group",
            group_index: Some(0),
            kind: ProtectedKind::GroupBorder,
            shape: ProtectedShape::VerticalSpan {
                x: 25,
                y_start: 3,
                y_end: 18,
            },
        },
        ProtectedGeometry {
            owner_id: "title",
            group_index: Some(0),
            kind: ProtectedKind::GroupTitle,
            shape: ProtectedShape::Rect(OccupiedRect::try_new(13, 4, 8, 1, &resources()).unwrap()),
        },
    ]);
    let work = resources();
    for y in 0..=20 {
        for x in 0..=28 {
            let point = CanvasCoord { x, y };
            let expected = scene
                .geometry
                .iter()
                .filter(|geometry| relevant_at(geometry, point))
                .count();
            let mut seen = 0;
            scene
                .all_at(point, &work, |geometry| {
                    seen += usize::from(relevant_at(geometry, point));
                    true
                })
                .unwrap();
            assert_eq!(seen, expected, "point {point:?}");

            for (width, height) in [(1, 1), (3, 2), (12, 7)] {
                let rect = OccupiedRect::try_new(x, y, width, height, &work).unwrap();
                let clear = scene
                    .geometry
                    .iter()
                    .all(|geometry| !geometry.shape.intersects(rect));
                assert_eq!(
                    scene
                        .all_in(rect, &work, |geometry| !geometry.shape.intersects(rect))
                        .unwrap(),
                    clear,
                    "reservation {rect:?}"
                );
            }
        }
    }
}

#[test]
fn spatial_protection_visits_logarithmic_branches_in_separated_scenes() {
    let mut observations = Vec::new();
    for count in [8usize, 64, 512, 4096] {
        let scene = scene((0..count).map(|index| node(index * 10, 10, 3, 3)).collect());
        assert_eq!(scene.index.len(), count * 2 - 1);
        let query = resources();
        let mut inspected = 0;
        scene
            .all_at(CanvasCoord { x: 1, y: 11 }, &query, |_| {
                inspected += 1;
                true
            })
            .unwrap();
        assert_eq!(inspected, 1);
        let visited = query.layout_work_used();
        assert_eq!(visited, 2 * count.ilog2() as usize + 1);
        observations.push((count, visited));
    }
    assert_eq!(observations, [(8, 7), (64, 13), (512, 19), (4096, 25)]);
}

#[test]
fn spatial_protection_keeps_every_overlap_with_linear_storage_and_work() {
    for count in [1usize, 16, 128] {
        let scene = scene(vec![node(2, 2, 10, 10); count]);
        let query = resources();
        let mut inspected = 0;
        scene
            .all_at(CanvasCoord { x: 5, y: 5 }, &query, |_| {
                inspected += 1;
                true
            })
            .unwrap();
        assert_eq!(inspected, count);
        assert_eq!(query.layout_work_used(), count * 2 - 1);
        assert_eq!(scene.index.len(), count * 2 - 1);
    }
}

#[test]
fn spatial_protection_splits_long_regions_by_their_center_spread() {
    let scene = scene(
        (0..16)
            .map(|index| {
                let y = if index % 2 == 0 { 900 } else { 1100 };
                node(0, y + index, 10_000, 1)
            })
            .collect(),
    );
    let query = resources();
    let mut inspected = 0;
    scene
        .all_at(CanvasCoord { x: 5000, y: 1000 }, &query, |_| {
            inspected += 1;
            true
        })
        .unwrap();
    assert_eq!(inspected, 0);
    assert_eq!(query.layout_work_used(), 3);
}

#[test]
fn spatial_protection_uses_inclusive_bounds_without_large_coordinate_overflow() {
    let scene = scene(vec![node(usize::MAX - 5, usize::MAX - 4, 5, 4)]);
    for point in [
        CanvasCoord {
            x: usize::MAX,
            y: usize::MAX,
        },
        CanvasCoord {
            x: usize::MAX - 6,
            y: usize::MAX - 5,
        },
        CanvasCoord {
            x: usize::MAX - 7,
            y: usize::MAX - 5,
        },
    ] {
        let clear = !relevant_at(&scene.geometry[0], point);
        assert_eq!(scene.all_at(point, &resources(), |_| false).unwrap(), clear);
    }
}

#[test]
fn spatial_protection_honors_query_work_limits_and_cancellation() {
    let scene = scene((0..64).map(|index| node(index * 10, 10, 3, 3)).collect());
    let point = CanvasCoord { x: 1, y: 11 };
    for limit in [13, 12] {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
            .unwrap();
        let query = ResourceContext::new(policy);
        let result = scene.all_at(point, &query, |_| true);
        if limit == 13 {
            assert!(result.unwrap());
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                if details.actual == 13 && details.max == 12)
            );
        }
    }

    let control = OperationControl::new();
    let query = resources().controlled(control.clone(), OperationPhase::Layout);
    let mut visited = 0;
    let result = scene.all_at(point, &query, |_| {
        visited += 1;
        control.cancel();
        true
    });
    assert!(matches!(result, Err(AsciiError::Cancelled(cancelled))
        if cancelled.reason == CancelReason::Requested));
    assert_eq!(visited, 1);
}

#[test]
fn spatial_protection_admits_storage_before_allocating_the_index() {
    let mut scene = ProtectedScene {
        geometry: vec![node(2, 2, 3, 3); 64],
        index: Vec::new(),
        extent: CanvasCoord { x: 0, y: 0 },
    };
    let policy = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    let error = scene
        .build_index(&ResourceContext::new(policy))
        .unwrap_err();
    assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits));
    assert_eq!(scene.index.capacity(), 0);
}

#[test]
fn spatial_protection_observes_execution_cancellation_before_construction() {
    use crate::graph::layout::layout_graph;
    use crate::graph::model::{AsciiGraph, GraphDirection};
    use crate::operation::AsciiExecution;

    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("A", "A");
    let layout = layout_graph(&graph, &crate::AsciiRenderOptions::ascii());
    let policy = AsciiResourcePolicy::unbounded();
    let control = OperationControl::new();
    control.cancel();
    let mut work = ResourceContext::new(policy);
    let error = super::super::SceneOccupancy::try_new_for_routes(
        &layout,
        0,
        1,
        &mut work,
        AsciiExecution::new(&control, &policy),
    )
    .unwrap_err();
    assert!(matches!(error, AsciiError::Cancelled(cancelled)
        if cancelled.phase == OperationPhase::Layout
            && cancelled.reason == CancelReason::Requested));
    assert_eq!(work.layout_work_used(), 0);
}
