use super::*;
use crate::model::Point;
use crate::work::{LayoutError, NoopWorkControl, WorkControl, WorkError};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn point(x: f64, y: f64) -> RoutePoint {
    RoutePoint::new(x, y, Axis::Horizontal)
}

fn request(start: (f64, f64), end: (f64, f64)) -> OrthoRequest {
    OrthoRequest {
        start: point(start.0, start.1),
        end: point(end.0, end.1),
        start_side: None,
        end_side: None,
        bounds: None,
    }
}

fn draw(router: &mut OrthoRouter, request: OrthoRequest) -> Option<Vec<RoutePoint>> {
    router
        .route(request, &mut Work::new(&mut NoopWorkControl))
        .unwrap()
}

fn assert_clear(path: &[RoutePoint], request: OrthoRequest, walls: &[Rect], margin: f64) {
    assert_eq!(path.first(), Some(&request.start));
    assert_eq!(path.last(), Some(&request.end));
    for segment in path.windows(2) {
        let [a, b] = [segment[0], segment[1]];
        assert_ne!(Point { x: a.x, y: a.y }, Point { x: b.x, y: b.y });
        assert!(a.x == b.x || a.y == b.y, "nonorthogonal leg {segment:?}");
        if let Some(r) = request.bounds {
            for p in [a, b] {
                assert!((r.x..=r.x + r.width).contains(&p.x));
                assert!((r.y..=r.y + r.height).contains(&p.y));
            }
        }
        for r in walls {
            let (x0, x1) = (r.x - margin, r.x + r.width + margin);
            let (y0, y1) = (r.y - margin, r.y + r.height + margin);
            let hit = if a.x == b.x {
                x0 < a.x && a.x < x1 && a.y.min(b.y).max(y0) < a.y.max(b.y).min(y1)
            } else {
                y0 < a.y && a.y < y1 && a.x.min(b.x).max(x0) < a.x.max(b.x).min(x1)
            };
            assert!(!hit, "{segment:?} enters {r:?} with margin {margin}");
        }
    }
}

fn maze() -> ([Rect; 3], OrthoRequest) {
    let walls = [
        rect(25.0, -10.0, 8.0, 49.0),
        rect(55.0, 22.0, 8.0, 48.0),
        rect(75.0, -10.0, 8.0, 48.0),
    ];
    let request = OrthoRequest {
        start_side: Some(Side::Right),
        end_side: Some(Side::Left),
        bounds: Some(rect(0.0, 0.0, 100.0, 60.0)),
        ..request((5.0, 25.0), (95.0, 25.0))
    };
    (walls, request)
}

#[test]
fn direct_and_single_turn_keep_both_fixed_knots() {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut router = OrthoRouter::new(&[], 4.0, 3, &mut work).unwrap();
    let start = RoutePoint::gap_boundary(2.0, 3.0, Axis::Horizontal);
    let end = RoutePoint::gap_boundary(12.0, 13.0, Axis::Vertical);
    let req = OrthoRequest {
        start,
        end,
        start_side: Some(Side::Right),
        end_side: Some(Side::Top),
        bounds: None,
    };
    assert_eq!(
        draw(&mut router, req).unwrap(),
        vec![start, point(12.0, 3.0), end]
    );
    let end = RoutePoint::gap_boundary(12.0, 3.0, Axis::Horizontal);
    assert_eq!(
        draw(&mut router, OrthoRequest { end, ..req }).unwrap(),
        vec![start, end]
    );
}

#[test]
fn takes_multiple_turns_between_alternating_walls_deterministically() {
    let (mut walls, request) = maze();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut router =
        OrthoRouter::new(&walls, 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    let first = draw(&mut router, request).unwrap();
    assert!(first.len() >= 8, "{first:?}");
    assert_clear(&first, request, &walls, 2.0);
    assert_eq!(draw(&mut router, request), Some(first.clone()));
    walls.reverse();
    let mut router =
        OrthoRouter::new(&walls, 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    assert_eq!(draw(&mut router, request), Some(first));
}

#[test]
fn vertex_cap_declines_search_and_leaves_the_router_available() {
    let wall = [rect(10.0, -4.0, 4.0, 8.0)];
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let blocked = request((0.0, 0.0), (30.0, 0.0));
    let mut capped = OrthoRouter::new(&wall, 4.0, 4, &mut work).unwrap();
    assert_eq!(draw(&mut capped, blocked), None);
    assert_eq!(capped.grid.nodes.len(), 4);
    let direct = request((0.0, 14.0), (30.0, 14.0));
    assert_eq!(
        draw(&mut capped, direct),
        Some(vec![direct.start, direct.end])
    );
    let mut available =
        OrthoRouter::new(&wall, 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    assert_clear(&draw(&mut available, blocked).unwrap(), blocked, &wall, 2.0);
    let mut disabled = OrthoRouter::new(&wall, 4.0, 0, &mut work).unwrap();
    assert_eq!(draw(&mut disabled, direct), None);
}

#[test]
fn out_of_bounds_obstacles_do_not_add_vertices_to_a_guarded_span() {
    let (walls, req) = maze();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut local = OrthoRouter::new(&walls, 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    let path = draw(&mut local, req).unwrap();
    let cap = local.grid.nodes.len();
    let mut scene = walls.to_vec();
    scene.extend((0..128).map(|i| {
        let p = f64::from(i) * 9.0 + 300.0;
        rect(p, p, 4.0, 5.0)
    }));
    let mut router = OrthoRouter::new(&scene, 4.0, cap, &mut work).unwrap();
    assert_eq!(draw(&mut router, req), Some(path));
    assert_eq!(router.grid.nodes.len(), cap);
}

#[test]
fn bounds_and_lead_directions_are_hard_constraints() {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let walls = [rect(15.0, -100.0, 3.0, 200.0)];
    let mut router =
        OrthoRouter::new(&walls, 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    let req = request((0.0, 0.0), (30.0, 0.0));
    let blocked = OrthoRequest {
        bounds: Some(rect(0.0, -10.0, 30.0, 20.0)),
        ..req
    };
    assert_eq!(draw(&mut router, blocked), None);
    assert_eq!(draw(&mut router, request((14.0, 0.0), (30.0, 0.0))), None);

    let mut router = OrthoRouter::new(&[], 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    let facing_away = OrthoRequest {
        start_side: Some(Side::Left),
        end_side: Some(Side::Right),
        ..req
    };
    let path = draw(&mut router, facing_away).unwrap();
    assert!(path.len() >= 4);
    assert!(path[1].x <= path[0].x);
    assert!(path[path.len() - 2].x >= path[path.len() - 1].x);
    assert_clear(&path, facing_away, &[], 2.0);
}

#[test]
fn recorded_crossings_and_overlap_change_the_preferred_path() {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut router = OrthoRouter::new(&[], 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    let req = request((0.0, 0.0), (20.0, 20.0));
    assert_eq!(draw(&mut router, req).unwrap()[1].x, 0.0);
    router
        .record_route(&[point(-5.0, 10.0), point(5.0, 10.0)], &mut work)
        .unwrap();
    let path = draw(&mut router, req).unwrap();
    assert_eq!((path[1].x, path[1].y), (20.0, 0.0));

    let req = request((0.0, 0.0), (200.0, 0.0));
    router
        .record_route(&[req.start, point(100.0, 0.0), req.end], &mut work)
        .unwrap();
    let path = draw(&mut router, req).unwrap();
    assert!(path.len() >= 4);
    assert!(path[1].y != 0.0);
    assert_clear(&path, req, &[], 0.0);
}

#[test]
fn a_crossing_at_a_grid_vertex_is_charged_in_both_directions() {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let walls = [rect(48.0, 6.0, 4.0, 2.0)];
    let mut router =
        OrthoRouter::new(&walls, 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    for far in [1.0, 1.5] {
        router
            .record_route(
                &[point(46.0, -1.0), point(46.0, 0.0), point(46.0, far)],
                &mut work,
            )
            .unwrap();
    }
    for (a, b) in [(0.0, 100.0), (100.0, 0.0)] {
        let req = OrthoRequest {
            bounds: Some(rect(0.0, -3.0, 100.0, 15.0)),
            ..request((a, 0.0), (b, 0.0))
        };
        let path = draw(&mut router, req).unwrap();
        assert_clear(&path, req, &walls, 2.0);
        assert_eq!(path.len(), 4, "{path:?}");
        assert!(path[1].y <= -1.0 || path[1].y >= 1.5, "{path:?}");
    }
}

#[test]
fn channel_projects_obstacles_along_the_whole_run() {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let walls = [rect(20.0, 0.0, 5.0, 5.0), rect(70.0, 20.0, 5.0, 10.0)];
    let router = OrthoRouter::new(&walls, 4.0, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work).unwrap();
    let run = OrthoSegment {
        axis: Axis::Horizontal,
        fixed: 12.0,
        min: 0.0,
        max: 100.0,
    };
    assert_eq!(
        router.channel(run, None, &mut work).unwrap(),
        Some([7.0, 18.0])
    );
    assert_eq!(
        router
            .channel(run, Some(rect(0.0, 8.0, 100.0, 8.0)), &mut work)
            .unwrap(),
        Some([8.0, 16.0])
    );
    assert_eq!(
        router
            .channel(OrthoSegment { fixed: 5.0, ..run }, None, &mut work)
            .unwrap(),
        None
    );
}

#[derive(Default)]
struct CheckLimit {
    checks: usize,
    limit: Option<usize>,
}

impl WorkControl for CheckLimit {
    fn check(&mut self) -> Result<(), WorkError> {
        if self.limit == Some(self.checks) {
            return Err(WorkError::Interrupted);
        }
        self.checks += 1;
        Ok(())
    }

    fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
        Ok(())
    }
}

#[test]
fn cancellation_propagates_at_multiple_phases_and_a_query_can_retry() {
    let (walls, request) = maze();
    let build = || {
        OrthoRouter::new(
            &walls,
            4.0,
            DEFAULT_MAX_ORTHO_GRID_NODES,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap()
    };
    let mut control = CheckLimit::default();
    let path = build()
        .route(request, &mut Work::new(&mut control))
        .unwrap()
        .unwrap();
    assert!(control.checks >= 6);
    for limit in [
        0,
        control.checks / 3,
        control.checks * 2 / 3,
        control.checks - 1,
    ] {
        let mut router = build();
        let mut control = CheckLimit {
            limit: Some(limit),
            ..CheckLimit::default()
        };
        assert_eq!(
            router.route(request, &mut Work::new(&mut control)),
            Err(LayoutError::Work(WorkError::Interrupted)),
            "cancellation at check {limit}"
        );
        assert_eq!(draw(&mut router, request), Some(path.clone()));
    }
}

#[test]
fn cancelling_route_registration_leaves_no_partial_trail() {
    let build = || {
        OrthoRouter::new(
            &[],
            4.0,
            DEFAULT_MAX_ORTHO_GRID_NODES,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap()
    };
    let req = request((0.0, 0.0), (200.0, 0.0));
    let trail = [req.start, req.end, point(200.0, 15.0)];
    let mut observed = CheckLimit::default();
    build()
        .record_route(&trail, &mut Work::new(&mut observed))
        .unwrap();
    let mut limited = CheckLimit {
        limit: Some(observed.checks - 1),
        ..CheckLimit::default()
    };
    let mut router = build();
    assert_eq!(
        router.record_route(&trail, &mut Work::new(&mut limited)),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    assert_eq!(draw(&mut router, req), Some(vec![req.start, req.end]));
    router
        .record_route(&trail, &mut Work::new(&mut NoopWorkControl))
        .unwrap();
    assert_eq!(draw(&mut router, req).unwrap().len(), 4);
}

#[test]
fn measured_batch_keeps_holes_and_accounts_for_earlier_spans() {
    let bounds = rect(-10.0, -40.0, 220.0, 80.0);
    let walls = [rect(70.0, 16.0, 12.0, 8.0)];
    let start = RoutePoint::gap_boundary(0.0, 0.0, Axis::Horizontal);
    let end = RoutePoint::gap_boundary(200.0, 0.0, Axis::Horizontal);
    let clear = OrthogonalSpan {
        start: (start, Side::Right),
        end: (end, Side::Left),
    };
    let blocked = OrthogonalSpan {
        start: (point(75.0, 20.0), Side::Right),
        ..clear
    };
    let route = |control: &mut CheckLimit| {
        route_orthogonal_spans(&walls, Some(bounds), 4.0, &[clear, blocked, clear], control)
    };
    let mut observed = CheckLimit::default();
    let paths = route(&mut observed).unwrap();
    assert_eq!(paths.len(), 3);
    assert_eq!(paths[0], Some(vec![start, end]));
    assert_eq!(paths[1], None);
    let detour = paths[2].as_ref().expect("a less costly parallel span");
    assert!(detour.len() > 2);
    assert_clear(
        detour,
        OrthoRequest {
            start,
            end,
            start_side: Some(Side::Right),
            end_side: Some(Side::Left),
            bounds: Some(bounds),
        },
        &walls,
        2.0,
    );
    let mut limited = CheckLimit {
        limit: Some(observed.checks - 1),
        ..CheckLimit::default()
    };
    assert_eq!(
        route(&mut limited),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
}
