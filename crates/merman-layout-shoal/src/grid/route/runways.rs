//! Keep foreign shafts outside marker approaches without expanding the visibility graph.

use super::*;
use crate::MIN_MARKER_LEAD;
use crate::geom::{Bvh, BvhEntry};
use std::ops::ControlFlow::{Break, Continue};

struct Runway {
    edge: usize,
    bounds: Bounds,
}

pub(super) struct Runways(Bvh<Runway>);

pub(super) enum Blockage {
    Clear,
    FixedLead,
    Middle(Bounds),
}

fn hits(bounds: Bounds, from: Point, to: Point) -> bool {
    if !bounds.overlaps(Bounds::between(from, to)) {
        return false;
    }
    if from.x == to.x || from.y == to.y {
        return true;
    }
    let interval = |range: [f64; 2], from: f64, to: f64| {
        let [a, b] = range.map(|limit| (limit - from) / (to - from));
        [a.min(b), a.max(b)]
    };
    let [x0, x1] = interval(bounds.x, from.x, to.x);
    let [y0, y1] = interval(bounds.y, from.y, to.y);
    x0.max(y0).max(0.0) <= x1.min(y1).min(1.0)
}

impl Runways {
    pub(super) fn new(parts: &[Part], sites: &[Site], work: &mut Work<'_>) -> LayoutResult<Self> {
        let mut runways = Vec::new();
        if parts.len() > 1 {
            for part in parts {
                work.visit(1)?;
                for end in [part.from, part.to] {
                    let site = sites[end.site];
                    if site.marker.length > 0.0 && site.marker.width > 0.0 {
                        let side = if end.inner {
                            opposite(site.side)
                        } else {
                            site.side
                        };
                        let marker = MarkerMetrics {
                            length: site.marker.length + MIN_MARKER_LEAD,
                            ..site.marker
                        };
                        push(
                            &mut runways,
                            Runway {
                                edge: part.edge,
                                bounds: Bounds::marker(site.at, side, marker),
                            },
                        )?;
                    }
                }
            }
        }
        Ok(Self(Bvh::build_with_work(
            runways,
            |runway| runway.bounds,
            work,
        )?))
    }

    pub(super) fn blockage(
        &self,
        edge: usize,
        contacts: [Point; 2],
        path: &[RoutePoint],
        work: &mut Work<'_>,
    ) -> LayoutResult<Blockage> {
        if self.0.items().is_empty() || path.is_empty() {
            return Ok(Blockage::Clear);
        }
        work.visit(path.len())?;
        let start = position(path[0]);
        let end = position(path[path.len() - 1]);
        let leads = [
            Bounds::between(contacts[0], start),
            Bounds::between(end, contacts[1]),
        ];
        let mut envelope = Bounds::between(start, start);
        for &point in &path[1..] {
            envelope.include(position(point));
        }
        let lead_hit = |bounds: Bounds| leads.iter().any(|&lead| bounds.overlaps(lead));
        let mut blockage = Blockage::Clear;
        let _ = self.0.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            let bounds = match entry {
                BvhEntry::Node(bounds) => {
                    return Ok(Continue(bounds.overlaps(envelope) || lead_hit(bounds)));
                }
                BvhEntry::Item(runway) if runway.edge == edge => return Ok(Continue(true)),
                BvhEntry::Item(runway) => runway.bounds,
            };
            if lead_hit(bounds) {
                blockage = Blockage::FixedLead;
                // Every middle path contains these knots. Looking for another channel
                // cannot clear a runway that already contains either of them.
                if hits(bounds, start, start) || hits(bounds, end, end) {
                    return Ok(Continue(true));
                }
            }
            if bounds.overlaps(envelope) {
                work.visit(path.len() - 1)?;
                if path
                    .windows(2)
                    .any(|leg| hits(bounds, position(leg[0]), position(leg[1])))
                {
                    blockage = Blockage::Middle(bounds);
                    return Ok(Break(()));
                }
            }
            Ok(Continue(true))
        })?;
        Ok(blockage)
    }

    pub(super) fn slide(
        &self,
        edge: usize,
        (contacts, path): ([Point; 2], &[RoutePoint]),
        runway: Bounds,
        separation: f64,
        work: &mut Work<'_>,
        mut accept: impl FnMut(&[RoutePoint], &mut Work<'_>) -> LayoutResult<bool>,
    ) -> LayoutResult<Option<Vec<RoutePoint>>> {
        work.charge(path.len())?;
        let mut points = reserved::<RoutePoint>(path.len())?;
        for &p in path {
            if let [.., a, b] = points.as_mut_slice()
                && let (Some(ab), Some(bp)) =
                    (OrthoSegment::between(*a, *b), OrthoSegment::between(*b, p))
                && ab.axis == bp.axis
                && ab.min.max(bp.min) == ab.max.min(bp.max)
            {
                *b = p;
            } else {
                add_point(&mut points, p);
            }
        }
        // The bounded gutter simplicity check admits up to five inner legs.
        if points.len() > 6 {
            return Ok(None);
        }
        work.visit(points.len().saturating_sub(1))?;
        let Some(leg) = points
            .windows(2)
            .position(|leg| hits(runway, position(leg[0]), position(leg[1])))
        else {
            return Ok(None);
        };
        if leg == 0 || leg + 2 >= points.len() {
            return Ok(None);
        }
        let Some(segment) = OrthoSegment::between(points[leg], points[leg + 1]) else {
            return Ok(None);
        };
        let vertical = segment.axis == Axis::Vertical;
        let [lo, hi] = if vertical { runway.x } else { runway.y };
        let [mut near, mut far] = [lo - separation, hi + separation];
        if (segment.fixed - near).abs() > (segment.fixed - far).abs() {
            std::mem::swap(&mut near, &mut far);
        }
        for track in [near, far] {
            for p in &mut points[leg..=leg + 1] {
                if vertical {
                    p.x = track;
                } else {
                    p.y = track;
                }
            }
            if accept(&points, work)?
                && !matches!(
                    self.blockage(edge, contacts, &points, work)?,
                    Blockage::Middle(_)
                )
            {
                return Ok(Some(points));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    fn scene(bounds: &[Bounds], work: &mut Work<'_>) -> Runways {
        let runways = bounds
            .iter()
            .copied()
            .enumerate()
            .map(|(edge, bounds)| Runway { edge, bounds })
            .collect();
        Runways(Bvh::build_with_work(runways, |runway| runway.bounds, work).unwrap())
    }

    #[test]
    fn runway_checks_distinguish_fixed_leads_from_middle_legs() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let contacts = [point([0.0, 0.0]), point([84.0, 0.0])];
        let middle = [24.0, 60.0].map(|x| knot(point([x, 0.0]), Axis::Horizontal));
        let lead = Bounds {
            x: [0.0, 16.0],
            y: [-2.0, 6.0],
        };
        let shaft = Bounds {
            x: [35.0, 45.0],
            y: [-3.0, 3.0],
        };
        let check = |bounds: &[Bounds], middle: &[RoutePoint], work: &mut Work<'_>| {
            scene(bounds, work)
                .blockage(2, contacts, middle, work)
                .unwrap()
        };
        assert!(matches!(
            check(&[lead], &middle, &mut work),
            Blockage::FixedLead
        ));
        assert!(matches!(
            check(&[lead], &middle[..1], &mut work),
            Blockage::FixedLead
        ));

        let trapped = Bounds {
            x: [0.0, 30.0],
            ..lead
        };
        assert!(matches!(
            check(&[trapped], &middle, &mut work),
            Blockage::FixedLead
        ));
        assert!(
            matches!(check(&[trapped, shaft], &middle, &mut work), Blockage::Middle(bounds) if bounds == shaft)
        );
    }

    #[test]
    fn diagonal_runway_collision_requires_actual_segment_contact() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let contacts = [point([63.0, 42.0]), point([141.0, 162.0])];
        let middle = contacts.map(|p| knot(p, Axis::Horizontal));
        let missed = Bounds {
            x: [126.0, 142.0],
            y: [38.0, 46.0],
        };
        let crossed = Bounds {
            x: [100.0, 104.0],
            y: [100.0, 104.0],
        };
        assert!(matches!(
            scene(&[missed], &mut work)
                .blockage(2, contacts, &middle, &mut work)
                .unwrap(),
            Blockage::Clear
        ));
        assert!(
            matches!(scene(&[crossed], &mut work).blockage(2, contacts, &middle, &mut work).unwrap(), Blockage::Middle(bounds) if bounds == crossed)
        );
        let corner = Bounds {
            x: [102.0, 106.0],
            y: [98.0, 102.0],
        };
        assert!(hits(corner, contacts[0], contacts[1]));
        assert!(hits(corner, contacts[1], contacts[0]));
    }

    #[test]
    fn sliding_a_blocked_shaft_keeps_the_leads_in_place() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let contacts = [point([0.0, 0.0]), point([80.0, 40.0])];
        let middle = [[20.0, 0.0], [40.0, 0.0], [40.0, 40.0], [60.0, 40.0]]
            .map(|p| knot(point(p), Axis::Horizontal));
        let shaft = Bounds {
            x: [38.0, 42.0],
            y: [16.0, 24.0],
        };
        let runways = scene(
            &[
                Bounds {
                    x: [0.0, 16.0],
                    y: [-2.0, 6.0],
                },
                shaft,
            ],
            &mut work,
        );
        let Blockage::Middle(blocker) = runways.blockage(2, contacts, &middle, &mut work).unwrap()
        else {
            panic!("the crossing shaft must be actionable");
        };
        let moved = runways
            .slide(
                2,
                (contacts, &middle),
                blocker,
                2.0,
                &mut work,
                |path, _| Ok(simple_candidate(path, contacts)),
            )
            .unwrap()
            .unwrap();
        assert_eq!(moved.first(), middle.first());
        assert_eq!(
            moved.last().copied().map(position),
            middle.last().copied().map(position)
        );
        assert!(matches!(
            runways.blockage(2, contacts, &moved, &mut work).unwrap(),
            Blockage::FixedLead
        ));
    }

    #[test]
    fn compact_gutters_avoid_a_foreign_shaft_with_a_fixed_lead_conflict() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let router = OrthoRouter::new(&[], 4.0, 0, &mut work).unwrap();
        let bounds = Rect {
            width: 100.0,
            height: 60.0,
            ..Rect::default()
        };
        let runways = scene(
            &[
                // A 10px marker with a 6px runway extends beyond its own 14px lead.
                Bounds {
                    x: [0.0, 16.0],
                    y: [19.0, 21.0],
                },
                Bounds {
                    x: [2.0, 12.0],
                    y: [18.0, 22.0],
                },
                Bounds {
                    x: [40.0, 50.0],
                    y: [15.0, 25.0],
                },
            ],
            &mut work,
        );
        let [a, b] = [14.0, 86.0].map(|x| RoutePoint::new(x, 20.0, Axis::Horizontal));
        let span = GutterSpan {
            leads: [(a, Side::Right), (b, Side::Left)],
            contacts: [point([0.0, 20.0]), point([100.0, 20.0])],
            prefer_row_gutter: false,
            runways: Some((&runways, 0)),
        };
        assert!(matches!(
            runways
                .blockage(0, span.contacts, &[a, b], &mut work)
                .unwrap(),
            Blockage::Middle(_)
        ));
        let tracks = [&[30.0, 70.0][..], &[35.0][..]];
        let used = LaneUse::default();
        let card = card_channel(&router, span, bounds, tracks, &used, 4.0, &mut work)
            .unwrap()
            .unwrap();
        let gutter = gutter_route(&router, span, bounds, tracks, &mut work)
            .unwrap()
            .unwrap();
        for path in [card, gutter] {
            assert_eq!(path.first().copied().map(position), Some(position(a)));
            assert_eq!(path.last().copied().map(position), Some(position(b)));
            assert!(simple_candidate(&path, span.contacts));
            assert!(matches!(
                runways
                    .blockage(0, span.contacts, &path, &mut work)
                    .unwrap(),
                Blockage::FixedLead
            ));
        }
    }
}
