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

    let mut first_anchor = [None; 3];
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
        search_existing_segments(
            &[PendingLabel {
                slot: 0,
                label: edges[0].label.unwrap(),
                segments: first
            }],
            &segments,
            &mut solids,
            &[],
            &mut first_anchor,
            &mut LabelSearchWork::new(&search_meter, None),
        )
        .unwrap(),
        LabelPassOutcome::ExistingSegments
    );
    let first_search = search_meter.used() - before;
    assert_eq!(first_anchor[0], Some(Point::new(120.0, 100.0)));
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
