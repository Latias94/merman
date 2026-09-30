//! Mermaid 12 ELK terminal-channel straightening after endpoint clipping.
//!
//! Source: packages/mermaid/src/rendering-util/layout-algorithms/elk/render.ts,
//! straightenTerminalJogs and straightenEdgeTerminals (mermaid@12.0.0).

use crate::model::LayoutPoint;

/// Paint routes may retain coordinates relative to separate parent groups.
pub(crate) trait TerminalRoute {
    fn points(&self) -> &[LayoutPoint];

    fn origin(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn replace_terminal_points(
        &mut self,
        points: Vec<LayoutPoint>,
        source_changed: bool,
        target_changed: bool,
    );
}

impl TerminalRoute for Vec<LayoutPoint> {
    fn points(&self) -> &[LayoutPoint] {
        self
    }

    fn replace_terminal_points(&mut self, points: Vec<LayoutPoint>, _: bool, _: bool) {
        *self = points;
    }
}

impl TerminalRoute for crate::model::LayoutEdge {
    fn points(&self) -> &[LayoutPoint] {
        &self.points
    }

    fn replace_terminal_points(&mut self, points: Vec<LayoutPoint>, _: bool, _: bool) {
        self.points = points;
    }
}

const JOG_EPS: f64 = 0.01;
const TERMINAL_JOG_MAX: f64 = 16.0;
const TERMINAL_RUN_MAX: f64 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

fn axis(a: &LayoutPoint, b: &LayoutPoint) -> Option<Axis> {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    if dx > JOG_EPS && dy <= JOG_EPS {
        Some(Axis::Horizontal)
    } else if dy > JOG_EPS && dx <= JOG_EPS {
        Some(Axis::Vertical)
    } else {
        None
    }
}

fn straighten_front(points: &mut Vec<LayoutPoint>) -> bool {
    if points.len() < 5 {
        return false;
    }
    let [p0, p1, p2, p3] = [&points[0], &points[1], &points[2], &points[3]];
    let Some(direction) = axis(p0, p1) else {
        return false;
    };
    let perpendicular = match direction {
        Axis::Horizontal => Axis::Vertical,
        Axis::Vertical => Axis::Horizontal,
    };
    if axis(p2, p3) != Some(direction)
        || axis(p1, p2) != Some(perpendicular)
        || (p1.x - p0.x).hypot(p1.y - p0.y) > TERMINAL_RUN_MAX
    {
        return false;
    }
    let (jog, forward) = match direction {
        Axis::Horizontal => (
            (p2.y - p1.y).abs(),
            (p1.x - p0.x).signum() == (p3.x - p2.x).signum(),
        ),
        Axis::Vertical => (
            (p2.x - p1.x).abs(),
            (p1.y - p0.y).signum() == (p3.y - p2.y).signum(),
        ),
    };
    if !(JOG_EPS..=TERMINAL_JOG_MAX).contains(&jog) || !forward {
        return false;
    }
    let mut last = 3;
    while last + 1 < points.len() && axis(&points[last], &points[last + 1]) == Some(direction) {
        last += 1;
    }
    if last == points.len() - 1 {
        return false;
    }
    let (x, y) = (p0.x, p0.y);
    for point in &mut points[2..=last] {
        match direction {
            Axis::Horizontal => point.y = y,
            Axis::Vertical => point.x = x,
        }
    }
    points.drain(1..3);
    true
}

fn straighten_terminal_jogs(points: &[LayoutPoint]) -> Option<(Vec<LayoutPoint>, bool, bool)> {
    if points.len() < 5 {
        return None;
    }
    let mut candidate = points.to_vec();
    let front_changed = straighten_front(&mut candidate);
    candidate.reverse();
    let back_changed = straighten_front(&mut candidate);
    candidate.reverse();
    (front_changed || back_changed).then_some((candidate, front_changed, back_changed))
}

fn crosses(a1: &LayoutPoint, a2: &LayoutPoint, b1: &LayoutPoint, b2: &LayoutPoint) -> bool {
    let side = |o: &LayoutPoint, p: &LayoutPoint, q: &LayoutPoint| {
        (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x)
    };
    let opposite = |a: f64, b: f64| (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0);
    opposite(side(b1, b2, a1), side(b1, b2, a2)) && opposite(side(a1, a2, b1), side(a1, a2, b2))
}

fn crossing_count(
    a: &[LayoutPoint],
    b: &impl TerminalRoute,
    origin_x: f64,
    origin_y: f64,
) -> usize {
    let (other_x, other_y) = b.origin();
    let dx = other_x - origin_x;
    let dy = other_y - origin_y;
    let mut count = 0;
    for pair in b.points().windows(2) {
        let b1 = LayoutPoint {
            x: pair[0].x + dx,
            y: pair[0].y + dy,
        };
        let b2 = LayoutPoint {
            x: pair[1].x + dx,
            y: pair[1].y + dy,
        };
        for segment in a.windows(2) {
            count += usize::from(crosses(&segment[0], &segment[1], &b1, &b2));
        }
    }
    count
}

/// Accept each candidate against the current routes, in source edge order. Ports never move.
/// Origin offsets put routes in the same coordinate frame without rewriting their stored points.
pub(crate) fn straighten_edge_terminals<R: TerminalRoute>(
    routes: &mut [R],
    mut charge: impl FnMut(usize) -> crate::Result<()>,
) -> crate::Result<()> {
    for index in 0..routes.len() {
        charge(routes[index].points().len().saturating_add(1))?;
        let Some((candidate, source_changed, target_changed)) =
            straighten_terminal_jogs(routes[index].points())
        else {
            continue;
        };
        let original = &routes[index];
        let (origin_x, origin_y) = original.origin();
        let mut before = 0usize;
        let mut after = 0usize;
        for (other_index, other) in routes.iter().enumerate() {
            if other_index == index || other.points().len() < 2 {
                continue;
            }
            charge(
                original
                    .points()
                    .len()
                    .saturating_sub(1)
                    .saturating_add(candidate.len().saturating_sub(1))
                    .saturating_mul(other.points().len() - 1),
            )?;
            before =
                before.saturating_add(crossing_count(original.points(), other, origin_x, origin_y));
            after = after.saturating_add(crossing_count(&candidate, other, origin_x, origin_y));
        }
        if after <= before {
            routes[index].replace_terminal_points(candidate, source_changed, target_changed);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::OperationWorkMeter;
    use crate::{RenderResourcePolicy, ResourceLimitId};

    struct Route {
        points: Vec<LayoutPoint>,
        origin: (f64, f64),
        changed: (bool, bool),
    }

    impl TerminalRoute for Route {
        fn points(&self) -> &[LayoutPoint] {
            &self.points
        }
        fn origin(&self) -> (f64, f64) {
            self.origin
        }
        fn replace_terminal_points(
            &mut self,
            points: Vec<LayoutPoint>,
            source: bool,
            target: bool,
        ) {
            self.points = points;
            self.changed = (source, target);
        }
    }

    fn points(input: &[(f64, f64)]) -> Vec<LayoutPoint> {
        input.iter().map(|&(x, y)| LayoutPoint { x, y }).collect()
    }
    fn route(input: &[(f64, f64)], offset: (f64, f64)) -> Route {
        Route {
            points: points(input),
            origin: offset,
            changed: (false, false),
        }
    }
    fn coords(points: &[LayoutPoint]) -> Vec<(f64, f64)> {
        points.iter().map(|p| (p.x, p.y)).collect()
    }
    fn meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    #[test]
    fn upstream_terminal_jog_cases_keep_ports_and_move_the_whole_run() {
        // Pinned geometry.spec.ts cases, also reflected into the vertical/end orientations.
        for (input, expected) in [
            (
                vec![
                    (193., 116.25),
                    (218., 116.25),
                    (218., 119.5),
                    (400., 119.5),
                    (400., 300.),
                ],
                vec![(193., 116.25), (400., 116.25), (400., 300.)],
            ),
            (
                vec![
                    (193., 116.25),
                    (218., 116.25),
                    (218., 119.5),
                    (300., 119.5),
                    (400., 119.5),
                    (400., 300.),
                ],
                vec![(193., 116.25), (300., 116.25), (400., 116.25), (400., 300.)],
            ),
            (
                vec![
                    (193., 116.5),
                    (218., 116.5),
                    (218., 117.358),
                    (400., 117.358),
                    (400., 300.),
                ],
                vec![(193., 116.5), (400., 116.5), (400., 300.)],
            ),
        ] {
            for vertical in [false, true] {
                for reverse in [false, true] {
                    let transform = |input: &[(f64, f64)]| {
                        let mut result = points(input);
                        if vertical {
                            for p in &mut result {
                                std::mem::swap(&mut p.x, &mut p.y);
                            }
                        }
                        if reverse {
                            result.reverse();
                        }
                        result
                    };
                    let input = transform(&input);
                    let expected = transform(&expected);
                    assert_eq!(
                        coords(&straighten_terminal_jogs(&input).unwrap().0),
                        coords(&expected)
                    );
                    assert_eq!(coords(&input).first(), coords(&expected).first());
                    assert_eq!(coords(&input).last(), coords(&expected).last());
                }
            }
        }
    }

    #[test]
    fn upstream_real_turns_and_far_ports_are_not_connectors() {
        for input in [
            vec![
                (193., 116.25),
                (218., 116.25),
                (218., 119.5),
                (300., 119.5),
                (400., 119.5),
            ],
            vec![
                (193., 100.),
                (218., 100.),
                (218., 140.),
                (400., 140.),
                (400., 300.),
            ],
            vec![
                (193., 116.25),
                (313., 116.25),
                (313., 119.5),
                (415., 119.5),
                (415., 300.),
            ],
            vec![
                (193., 116.),
                (218., 116.),
                (218., 119.),
                (100., 119.),
                (100., 300.),
            ],
            vec![(193., 120.), (315., 120.)],
        ] {
            assert!(straighten_terminal_jogs(&points(&input)).is_none());
        }
    }

    #[test]
    fn graph_pass_rejects_new_crossings_across_different_route_origins() {
        let original = [(0., 0.), (20., 0.), (20., 5.), (100., 5.), (100., 50.)];
        // The other route is globally x=60, y=-1..1, so moving the channel onto y=0 crosses it.
        let mut routes = [
            route(&original, (0., 0.)),
            route(&[(10., 9.), (10., 11.)], (50., -10.)),
        ];
        let work = meter();
        straighten_edge_terminals(&mut routes, |units| work.charge(units).map_err(Into::into))
            .unwrap();
        assert_eq!(coords(&routes[0].points), original);
        // A contact at the other route's endpoint is not a strict crossing.
        routes[1].points[0].y = 10.;
        let work = meter();
        straighten_edge_terminals(&mut routes, |units| work.charge(units).map_err(Into::into))
            .unwrap();
        assert_eq!(
            coords(&routes[0].points),
            [(0., 0.), (100., 0.), (100., 50.)]
        );
    }

    #[test]
    fn graph_pass_budget_rejection_does_not_accept_a_candidate() {
        let original = [(0., 0.), (20., 0.), (20., 5.), (100., 5.), (100., 50.)];
        let mut routes = [
            route(&original, (0., 0.)),
            route(&[(60., -1.), (60., 1.)], (0., 0.)),
        ];
        let work = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 6)
                .unwrap(),
        );
        assert!(matches!(
            straighten_edge_terminals(&mut routes, |units| work.charge(units).map_err(Into::into)),
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        assert_eq!(coords(&routes[0].points), original);
    }
}
