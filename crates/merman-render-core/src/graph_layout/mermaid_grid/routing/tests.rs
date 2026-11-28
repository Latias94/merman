use super::labels::{
    GridLabelMarkers, GridLabelObstacles, grid_label_fallback_anchor, place_grid_label_on_route,
};
use super::outline::{GridOutline, GridOutlineQuery, clip_grid_endpoints};
use super::*;
use crate::LayoutPoint;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::{RoutePoint, Size};
use crate::resources::{RenderResourcePolicy, ResourceLimitId};
use std::borrow::Cow;

fn meter() -> OperationWorkMeter {
    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
}

fn points<const N: usize>(coordinates: [(f64, f64); N]) -> [Point; N] {
    coordinates.map(|(x, y)| Point::new(x, y))
}

fn route_fixture(
    children: &[GridRoutingChild],
    occupied: &[GridSegment],
    plans: usize,
    work: &OperationWorkMeter,
) -> Result<Option<GridCorridorRoute>> {
    let view = GridRoutingView {
        children,
        horizontal_corridors: &[80.0, 40.0],
        vertical_corridors: &[],
    };
    let source = GridPort {
        child: 0,
        side: Side::Right,
        coordinate: 20.0,
        candidates: &points([(20.0, 20.0)]),
    };
    let target = GridPort {
        child: 1,
        side: Side::Left,
        coordinate: 100.0,
        candidates: &points([(140.0, 100.0)]),
    };
    route_grid_corridor(view, source, target, occupied, plans, work)
}

const GRID_CHILDREN: [GridRoutingChild; 2] = [
    GridRoutingChild {
        bounds: Rect::new(0.0, 0.0, 20.0, 40.0),
        corridors: Rect::new(-20.0, -20.0, 50.0, 40.0),
    },
    GridRoutingChild {
        bounds: Rect::new(140.0, 80.0, 160.0, 120.0),
        corridors: Rect::new(110.0, 40.0, 180.0, 140.0),
    },
];

#[test]
fn corridor_ties_use_numeric_coordinate_and_trim_occupancy() {
    let route = route_fixture(&GRID_CHILDREN, &[], 1, &meter())
        .unwrap()
        .unwrap();
    assert_eq!(
        route.points,
        points([
            (20.0, 20.0),
            (50.0, 20.0),
            (50.0, 40.0),
            (110.0, 40.0),
            (110.0, 100.0),
            (140.0, 100.0),
        ])
    );
    assert_eq!(
        route
            .occupancy
            .iter()
            .map(|segment| (segment.start, segment.end))
            .collect::<Vec<_>>(),
        [
            (Point::new(40.0, 20.0), Point::new(50.0, 20.0)),
            (Point::new(50.0, 20.0), Point::new(50.0, 40.0)),
            (Point::new(50.0, 40.0), Point::new(110.0, 40.0)),
            (Point::new(110.0, 40.0), Point::new(110.0, 100.0)),
            (Point::new(110.0, 100.0), Point::new(120.0, 100.0)),
        ]
    );

    let blocker = GridRoutingChild {
        bounds: Rect::new(75.0, 44.0, 85.0, 54.0),
        corridors: Rect::ZERO,
    };
    assert!(
        route_fixture(
            &[GRID_CHILDREN[0], GRID_CHILDREN[1], blocker],
            &[],
            1,
            &meter()
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn straight_fast_route_uses_the_measured_sibling_ports() {
    let children = [
        GridRoutingChild {
            bounds: Rect::new(0.0, 0.0, 80.0, 40.0),
            corridors: Rect::new(-27.5, -22.5, 107.5, 62.5),
        },
        GridRoutingChild {
            bounds: Rect::new(135.0, 0.0, 215.0, 40.0),
            corridors: Rect::new(107.5, -22.5, 242.5, 62.5),
        },
    ];
    let view = GridRoutingView {
        children: &children,
        horizontal_corridors: &[-22.5, 62.5],
        vertical_corridors: &[-27.5, 107.5, 242.5],
    };
    let [start, end] = points([(80.0, 20.0), (135.0, 20.0)]);
    let source = GridPort {
        child: 0,
        side: Side::Right,
        coordinate: 20.0,
        candidates: &[start],
    };
    let target = GridPort {
        child: 1,
        side: Side::Left,
        coordinate: 20.0,
        candidates: &[end],
    };
    let route = route_grid_corridor(view, source, target, &[], 1, &meter())
        .unwrap()
        .unwrap();
    assert_eq!(route.points, [start, end]);
    assert_eq!(
        route.occupancy,
        [GridSegment::new(Point::new(100.0, 20.0), Point::new(115.0, 20.0)).unwrap()]
    );
    assert!(
        route_grid_corridor(
            view,
            source,
            GridPort {
                candidates: &[],
                ..target
            },
            &[],
            1,
            &meter()
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn occupied_parallel_corridors_only_nudge_above_the_plan_cutoff() {
    let [start, end] = points([(70.0, 40.0), (90.0, 40.0)]);
    let occupied = [GridSegment::new(start, end).unwrap()];
    assert!(
        route_fixture(&GRID_CHILDREN, &occupied, 64, &meter())
            .unwrap()
            .is_none()
    );
    let route = route_fixture(&GRID_CHILDREN, &occupied, 65, &meter())
        .unwrap()
        .unwrap();
    assert_eq!(
        route.points,
        points([
            (20.0, 20.0),
            (50.0, 20.0),
            (50.0, 64.0),
            (110.0, 64.0),
            (110.0, 100.0),
            (140.0, 100.0),
        ])
    );
    let [start, end] = points([(80.0, 10.0), (80.0, 90.0)]);
    let crossing = [GridSegment::new(start, end).unwrap()];
    assert!(
        route_fixture(&GRID_CHILDREN, &crossing, 1, &meter())
            .unwrap()
            .is_some()
    );
}

#[test]
fn corridor_charges_cumulative_work_and_observes_cancel() {
    let work = meter();
    route_fixture(&GRID_CHILDREN, &[], 1, &work)
        .unwrap()
        .unwrap();
    let cost = work.used();
    for limit in [cost, cost + 1] {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
            .unwrap();
        let work = OperationWorkMeter::new(policy);
        work.charge(1).unwrap();
        let result = route_fixture(&GRID_CHILDREN, &[], 1, &work);
        if limit == cost {
            let Error::ResourceLimitExceeded(error) = result.unwrap_err() else {
                panic!("expected bounded route rejection")
            };
            assert_eq!(error.limit, "max_layout_work_units");
            assert_eq!(error.actual, cost + 1);
        } else {
            assert!(result.unwrap().is_some());
            assert_eq!(work.used(), cost + 1);
        }
    }
    let work = meter();
    work.control().cancel();
    assert!(
        matches!(route_fixture(&GRID_CHILDREN, &[], 1, &work), Err(Error::Cancelled(error)) if error.phase == OperationPhase::Layout)
    );
    assert_eq!(work.used(), 0);
}

#[test]
fn existing_label_midpoint_and_solid_band_tie_match_upstream() {
    let route = points([(50.0, 100.0), (350.0, 100.0)]);
    let size = Size {
        width: 40.0,
        height: 20.0,
    };
    let solids = [
        Rect::new(30.0, 90.0, 50.0, 110.0),
        Rect::new(350.0, 90.0, 370.0, 110.0),
    ];
    let obstacles = GridLabelObstacles {
        solids: &solids,
        ..Default::default()
    };
    let markers = GridLabelMarkers {
        start: false,
        end: true,
    };
    assert_eq!(
        place_grid_label_on_route(&route, size, markers, obstacles, &meter()).unwrap(),
        Some(Point::new(200.0, 100.0))
    );

    let solids = [solids[0], solids[1], Rect::new(190.0, 108.0, 210.0, 116.0)];
    let obstacles = GridLabelObstacles {
        solids: &solids,
        ..obstacles
    };
    assert_eq!(
        place_grid_label_on_route(&route, size, markers, obstacles, &meter()).unwrap(),
        Some(Point::new(170.0, 100.0))
    );
}

#[test]
fn labels_reject_foreign_shaft_and_group_border_contacts() {
    let route = points([(50.0, 100.0), (350.0, 100.0)]);
    let size = Size {
        width: 40.0,
        height: 20.0,
    };
    let [a, b] = points([(200.0, 50.0), (200.0, 150.0)]);
    let obstacles = GridLabelObstacles {
        foreign_segments: &[GridSegment::new(a, b).unwrap()],
        ..Default::default()
    };
    assert!(
        place_grid_label_on_route(
            &route,
            size,
            GridLabelMarkers::default(),
            obstacles,
            &meter()
        )
        .unwrap()
        .is_none()
    );

    let border = Rect::new(200.0, 0.0, 400.0, 200.0);
    let obstacles = GridLabelObstacles {
        group_borders: &[border],
        ..Default::default()
    };
    assert!(
        place_grid_label_on_route(
            &route,
            size,
            GridLabelMarkers::default(),
            obstacles,
            &meter()
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(
        grid_label_fallback_anchor(&route, None, None, &meter()).unwrap(),
        Some(Point::new(200.0, 100.0))
    );
    assert_eq!(
        grid_label_fallback_anchor(&[], Some(route[0]), Some(route[1]), &meter()).unwrap(),
        Some(route[0])
    );
}

#[test]
fn outline_query_retains_the_offset_diamond_terminal_axis() {
    let route = points([(80.0, 60.0), (80.0, 20.0), (300.0, 20.0)]);
    let profile = ShapePortProfile::diamond(80.0, 80.0);
    let center = LayoutPoint { x: 100.0, y: 100.0 };
    let intersect = |query: GridOutlineQuery| {
        assert_eq!(query.adjacent, Point::new(80.0, 20.0));
        assert_eq!(query.line_origin, Some(Point::new(80.0, 181.0)));
        let line = RoutePoint::new(query.adjacent.x, query.adjacent.y, Axis::Vertical);
        Ok(profile
            .try_contact(&center, Side::Top, line)
            .map(|contact| Point::new(contact.x, contact.y)))
    };
    let shape = GridOutline {
        center: Some(Point::new(100.0, 100.0)),
        size: Some(Size {
            width: 80.0,
            height: 80.0,
        }),
        intersect: &intersect,
    };
    let result = clip_grid_endpoints(&route, Some(shape), None, &meter()).unwrap();
    assert_eq!(
        result.as_ref(),
        &points([(80.0, 80.0), (80.0, 20.0), (300.0, 20.0)])
    );

    let off_axis = |_| Ok(Some(Point::new(81.0, 80.0)));
    let result = clip_grid_endpoints(
        &route,
        Some(GridOutline {
            intersect: &off_axis,
            ..shape
        }),
        None,
        &meter(),
    )
    .unwrap();
    assert!(matches!(result, Cow::Borrowed(value) if std::ptr::eq(value, &route)));
}

#[test]
fn clipped_two_point_route_preserves_midpoint_and_emit_phase() {
    let route = points([(0.0, 10.0), (100.0, 10.0)]);
    let tail = GridOutline {
        center: None,
        size: None,
        intersect: &|_| Ok(Some(Point::new(-10.0, 10.0))),
    };
    let head = GridOutline {
        intersect: &|_| Ok(Some(Point::new(110.0, 10.0))),
        ..tail
    };
    let result = clip_grid_endpoints(&route, Some(tail), Some(head), &meter()).unwrap();
    assert_eq!(
        result.as_ref(),
        &points([(-10.0, 10.0), (50.0, 10.0), (110.0, 10.0)])
    );

    let work = meter();
    let cancel = |_| {
        work.control().cancel();
        Ok(Some(Point::new(-10.0, 10.0)))
    };
    assert!(matches!(
        clip_grid_endpoints(&route, Some(GridOutline { intersect: &cancel, ..tail }), Some(head), &work),
        Err(Error::Cancelled(error)) if error.phase == OperationPhase::Emit
    ));
}
