use super::*;
use crate::graph_layout::Size;
use crate::graph_layout::mermaid_grid::{
    GridItem, GridMeasuredEdge, GridMeasuredItem, prepare_grid,
};
use crate::resources::{RenderResourcePolicy, ResourceLimitId};
use merman_model::OperationControl;

fn items() -> [GridMeasuredItem<'static>; 2] {
    ["source", "target"].map(|id| GridMeasuredItem {
        item: GridItem {
            id,
            authored_id: None,
            source_ordinal: None,
            inline: Default::default(),
        },
        parent_id: None,
        kind: GridItemKind::Leaf(Size {
            width: 20.0,
            height: 20.0,
        }),
    })
}

fn label(id: &'static str, width: f64) -> GridMeasuredEdge<'static> {
    GridMeasuredEdge {
        id,
        source: 0,
        target: 1,
        label: Some(GridEdgeLabel {
            size: Size {
                width,
                height: 20.0,
            },
            start_marker: false,
            end_marker: false,
        }),
    }
}

fn policy() -> RenderResourcePolicy {
    RenderResourcePolicy::unbounded_for_trusted_input()
}

fn meter() -> OperationWorkMeter {
    OperationWorkMeter::new(policy())
}

fn route(points: &[(f64, f64)]) -> GridRoutedEdge {
    GridRoutedEdge {
        points: points.iter().map(|&point| Point::from(point)).collect(),
        label_center: Some(Point::new(-200.0, -10.0)),
    }
}

fn routes(last_x: f64) -> [GridRoutedEdge; 3] {
    [
        route(&[(10.0, 100.0), (310.0, 100.0)]),
        route(&[(10.0, 112.0), (last_x, 112.0)]),
        route(&[(160.0, 100.0), (160.0, 100.0)]),
    ]
}

fn centers(routes: &[GridRoutedEdge]) -> Vec<Option<Point>> {
    routes.iter().map(|route| route.label_center).collect()
}

fn assert_points_equal(actual: &[GridRoutedEdge], expected: &[GridRoutedEdge]) {
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.points, expected.points);
    }
}

#[test]
fn reserves_parked_and_earlier_labels_by_occurrence() {
    let items = items();
    let edges = [
        label("same", 60.0),
        label("same", 60.0),
        label("parked", 20.0),
    ];
    let work = meter();
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let mut placed = routes(310.0);
    prepared.position_labels(&mut placed, &work).unwrap();
    assert_eq!(
        centers(&placed),
        [
            Some(Point::new(120.0, 100.0)),
            Some(Point::new(200.0, 112.0)),
            Some(Point::new(160.0, 100.0)),
        ]
    );
    assert_points_equal(&placed, &routes(310.0));
}

#[test]
fn degrades_the_whole_batch_on_a_soft_limit_or_an_exhausted_existing_segment() {
    let items = items();
    let edges = [
        label("first", 60.0),
        label("second", 60.0),
        label("parked", 20.0),
    ];
    let prepared = prepare_grid(&items, &edges, None, &meter()).unwrap();

    let mut solids: Vec<_> = (0..items.len())
        .map(|slot| {
            let bounds = prepared.item_bounds(slot).unwrap();
            Rect::new(
                bounds.x,
                bounds.y,
                bounds.x + bounds.width,
                bounds.y + bounds.height,
            )
        })
        .collect();
    solids.push(label_rect_at(
        Point::new(160.0, 100.0),
        edges[2].label.unwrap().size,
    ));
    let points = routes(310.0);
    let mut segments = Vec::new();
    let search_meter = meter();
    let first = append_segments(&mut segments, &points[0].points, &search_meter).unwrap();
    append_segments(&mut segments, &points[1].points, &search_meter).unwrap();
    let before = search_meter.used();
    assert_eq!(
        place_label_on_segments(
            &segments[first.clone()],
            edges[0].label.unwrap().size,
            GridLabelMarkers::default(),
            LabelCollisionView {
                solids: &solids,
                group_borders: &[],
                foreign_segments: [&segments[..first.start], &segments[first.end..]],
            },
            &mut LabelSearchWork::new(&search_meter, None),
        )
        .unwrap(),
        Some(Point::new(120.0, 100.0))
    );
    let first_search = search_meter.used() - before;
    for (last_x, limit, expected) in [
        (310.0, 0, LabelPassOutcome::SearchWorkLimit),
        (310.0, first_search, LabelPassOutcome::SearchWorkLimit),
        (
            50.0,
            MAX_LABEL_SEARCH_WORK,
            LabelPassOutcome::ExistingSegmentsExhausted,
        ),
    ] {
        let mut placed = routes(last_x);
        assert_eq!(
            prepared
                .position_labels_with_limit(&mut placed, limit, &meter())
                .unwrap(),
            expected
        );
        assert_eq!(
            centers(&placed),
            [
                Some(Point::new(160.0, 100.0)),
                Some(Point::new((10.0 + last_x) / 2.0, 112.0)),
                Some(Point::new(160.0, 100.0)),
            ]
        );
        assert_points_equal(&placed, &routes(last_x));
    }
}

#[test]
fn parks_empty_and_point_routes_and_clears_unmeasured_labels() {
    let items = items();
    let edges = [
        label("empty", 30.0),
        label("point", 30.0),
        label("unmeasured", f64::NAN),
    ];
    let work = meter();
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let source = prepared.item_bounds(0).unwrap().center();
    let mut placed = [
        route(&[]),
        route(&[(25.0, 60.0), (25.0 + 0.5e-6, 60.0)]),
        route(&[(10.0, 100.0), (310.0, 100.0)]),
    ];
    assert_eq!(
        prepared
            .position_labels_with_limit(&mut placed, 0, &work)
            .unwrap(),
        LabelPassOutcome::ExistingSegments
    );
    assert_eq!(
        centers(&placed),
        [
            Some(Point::new(source.x, source.y)),
            Some(Point::new(25.0, 60.0)),
            None
        ]
    );
    assert_eq!(placed[1].points.len(), 2);
}

#[test]
fn commits_only_after_the_cumulative_work_and_cancellation_checks() {
    let items = items();
    let edges = [
        label("first", 60.0),
        label("second", 60.0),
        label("parked", 20.0),
    ];
    let prepared = prepare_grid(&items, &edges, None, &meter()).unwrap();
    let original = routes(310.0);
    let mut placed = routes(310.0);
    let baseline = meter();
    baseline.charge(17).unwrap();
    prepared.position_labels(&mut placed, &baseline).unwrap();
    let total = baseline.used();

    let limited = |limit| {
        OperationWorkMeter::new(
            policy()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        )
    };
    let exact = limited(total);
    exact.charge(17).unwrap();
    let mut at_limit = routes(310.0);
    prepared.position_labels(&mut at_limit, &exact).unwrap();
    assert_eq!(centers(&at_limit), centers(&placed));

    let short = limited(total - 1);
    short.charge(17).unwrap();
    let mut interrupted = routes(310.0);
    assert!(matches!(
        prepared.position_labels(&mut interrupted, &short),
        Err(Error::ResourceLimitExceeded(_))
    ));
    assert_eq!(centers(&interrupted), centers(&original));
    assert_points_equal(&interrupted, &original);

    let control = OperationControl::new();
    control.cancel_after_checkpoints(10);
    let cancelled = OperationWorkMeter::new_with_control(policy(), control);
    assert!(matches!(
        prepared.position_labels(&mut interrupted, &cancelled),
        Err(Error::Cancelled(error)) if error.phase == OperationPhase::Layout
    ));
    assert_eq!(centers(&interrupted), centers(&original));
    assert_points_equal(&interrupted, &original);
}

#[test]
fn rejects_a_route_slice_with_missing_occurrences() {
    let items = items();
    let edges = [label("one", 60.0), label("two", 60.0)];
    let work = meter();
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let mut placed = [route(&[(10.0, 100.0), (310.0, 100.0)])];
    let before = placed[0].label_center;
    assert!(matches!(
        prepared.position_labels(&mut placed, &work),
        Err(Error::InvalidModel { .. })
    ));
    assert_eq!(placed[0].label_center, before);
}

#[test]
fn foreign_route_keeps_its_prior_label_and_terminal_rays_by_occurrence() {
    let items = items();
    let edges = [label("shared-id", 20.0), label("shared-id", 40.0)];
    let work = meter();
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let mut placed = [
        route(&[(160.0, -50.0), (160.0, 150.0)]),
        route(&[(10.0, 100.0), (310.0, 100.0)]),
    ];
    assert_eq!(
        prepared
            .position_labels_with_limit(&mut placed, MAX_LABEL_SEARCH_WORK, &work)
            .unwrap(),
        LabelPassOutcome::ForeignReroutes
    );
    assert_eq!(
        centers(&placed),
        [
            Some(Point::new(160.0, 50.0)),
            Some(Point::new(160.0, 100.0))
        ]
    );
    let expected = route(&[
        (160.0, -50.0),
        (160.0, 84.0),
        (126.0, 84.0),
        (126.0, 116.0),
        (160.0, 116.0),
        (160.0, 150.0),
    ]);
    assert_eq!(placed[0].points, expected.points);
    assert_eq!(
        placed[1].points,
        route(&[(10.0, 100.0), (310.0, 100.0)]).points
    );
}

#[test]
fn foreign_route_proposals_discard_edits_when_a_label_cannot_be_placed() {
    let items = items();
    let unlabeled = GridMeasuredEdge {
        label: None,
        ..label("foreign", 0.0)
    };
    let edges = [label("owner", 40.0), unlabeled, label("short", 80.0)];
    let work = meter();
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let original = || {
        [
            route(&[(10.0, 100.0), (310.0, 100.0)]),
            route(&[(160.0, 50.0), (160.0, 150.0)]),
            route(&[(200.0, 160.0), (250.0, 160.0)]),
        ]
    };
    let mut placed = original();
    assert_eq!(
        prepared
            .position_labels_with_limit(&mut placed, MAX_LABEL_SEARCH_WORK, &work)
            .unwrap(),
        LabelPassOutcome::ExistingSegmentsExhausted
    );
    assert_points_equal(&placed, &original());
    assert_eq!(
        centers(&placed),
        [
            Some(Point::new(160.0, 100.0)),
            None,
            Some(Point::new(225.0, 160.0))
        ]
    );

    let mut earlier = label("earlier", 20.0);
    earlier.label.as_mut().unwrap().size.height = 2.0;
    let edges = [earlier, label("owner", 40.0)];
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let original = || {
        [
            route(&[(160.0, 22.0), (160.0, 150.0)]),
            route(&[(10.0, 100.0), (310.0, 100.0)]),
        ]
    };
    let mut placed = original();
    assert_eq!(
        prepared
            .position_labels_with_limit(&mut placed, MAX_LABEL_SEARCH_WORK, &work)
            .unwrap(),
        LabelPassOutcome::ExistingSegmentsExhausted
    );
    assert_points_equal(&placed, &original());
    assert_eq!(
        centers(&placed),
        [
            Some(Point::new(160.0, 86.0)),
            Some(Point::new(160.0, 100.0))
        ]
    );

    let self_loop = GridMeasuredEdge {
        source: 0,
        target: 0,
        label: None,
        ..label("z-loop", 0.0)
    };
    let edges = [label("owner", 40.0), unlabeled, self_loop];
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let original = || {
        [
            route(&[(10.0, 100.0), (310.0, 100.0)]),
            route(&[(160.0, 50.0), (160.0, 150.0)]),
            route(&[(165.0, 50.0), (165.0, 150.0)]),
        ]
    };
    let mut placed = original();
    assert_eq!(
        prepared
            .position_labels_with_limit(&mut placed, MAX_LABEL_SEARCH_WORK, &work)
            .unwrap(),
        LabelPassOutcome::ExistingSegmentsExhausted
    );
    assert_points_equal(&placed, &original());
}

#[test]
fn foreign_route_proposals_share_the_original_meter_and_publish_atomically() {
    let items = items();
    let edges = [
        label("owner", 40.0),
        GridMeasuredEdge {
            label: None,
            ..label("foreign", 0.0)
        },
    ];
    let original = || {
        [
            route(&[(10.0, 100.0), (310.0, 100.0)]),
            route(&[(160.0, 50.0), (160.0, 150.0)]),
        ]
    };
    let work = meter();
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let prior = work.used();
    let mut placed = original();
    prepared.position_labels(&mut placed, &work).unwrap();
    let total = work.used();
    assert_eq!(placed[1].points.len(), 6);

    let mut result = original();
    let with_limit = |limit| {
        OperationWorkMeter::new(
            policy()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        )
    };
    let exact = with_limit(total);
    exact.charge(prior).unwrap();
    prepared.position_labels(&mut result, &exact).unwrap();
    assert_points_equal(&result, &placed);
    assert_eq!(centers(&result), centers(&placed));

    let short = with_limit(total - 1);
    short.charge(prior).unwrap();
    let mut rejected = original();
    assert!(matches!(
        prepared.position_labels(&mut rejected, &short),
        Err(Error::ResourceLimitExceeded(_))
    ));
    assert_points_equal(&rejected, &original());
    assert_eq!(centers(&rejected), centers(&original()));

    let control = OperationControl::new();
    control.cancel_after_checkpoints(30);
    let cancelled = OperationWorkMeter::new_with_control(policy(), control);
    assert!(matches!(
        prepared.position_labels(&mut rejected, &cancelled),
        Err(Error::Cancelled(error)) if error.phase == OperationPhase::Layout
    ));
    assert_points_equal(&rejected, &original());
    assert_eq!(centers(&rejected), centers(&original()));

    let soft = meter();
    assert_eq!(
        prepared
            .position_labels_with_limit(&mut rejected, 60, &soft)
            .unwrap(),
        LabelPassOutcome::SearchWorkLimit
    );
    assert_points_equal(&rejected, &original());
    assert_eq!(centers(&rejected), [Some(Point::new(160.0, 100.0)), None]);
    soft.checkpoint(OperationPhase::Layout).unwrap();
}

#[test]
fn short_owner_detour_publishes_its_route_and_label_together() {
    let items = items();
    let work = meter();
    let edges = [label("short", 80.0)];
    let prepared = prepare_grid(&items, &edges, None, &work).unwrap();
    let preparation_work = work.used();
    let original = || [route(&[(200.0, 160.0), (250.0, 160.0)])];
    let mut placed = original();
    assert_eq!(
        prepared
            .position_labels_with_limit(&mut placed, MAX_LABEL_SEARCH_WORK, &work)
            .unwrap(),
        LabelPassOutcome::OwnerDetour
    );
    assert_eq!(
        placed[0].points,
        [
            (200.0, 160.0),
            (212.0, 160.0),
            (212.0, 120.0),
            (179.0, 120.0),
            (179.0, 136.0),
            (271.0, 136.0),
            (271.0, 120.0),
            (238.0, 120.0),
            (238.0, 160.0),
            (250.0, 160.0),
        ]
        .map(Point::from)
    );
    assert_eq!(centers(&placed), [Some(Point::new(225.0, 136.0))]);

    let limit = work.used();
    let with_limit = |maximum| {
        OperationWorkMeter::new(
            policy()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, maximum)
                .unwrap(),
        )
    };
    let exact = with_limit(limit);
    exact.charge(preparation_work).unwrap();
    let mut result = original();
    prepared.position_labels(&mut result, &exact).unwrap();
    assert_points_equal(&result, &placed);
    assert_eq!(centers(&result), centers(&placed));

    let short = with_limit(limit - 1);
    short.charge(preparation_work).unwrap();
    let mut rejected = original();
    assert!(matches!(
        prepared.position_labels(&mut rejected, &short),
        Err(Error::ResourceLimitExceeded(_))
    ));
    assert_eq!(short.used(), limit - 1);
    assert_points_equal(&rejected, &original());
    assert_eq!(centers(&rejected), centers(&original()));
}
