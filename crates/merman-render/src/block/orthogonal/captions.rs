use super::{
    EdgePlacement, LabelPass, LayoutLabel, OperationLayoutWorkControl, Point, Rect, Result, Wall,
    intersects, label_box,
};
use crate::model::LayoutEdge;
use merman_render_core::route_paint::ObstacleOwner;

pub(super) fn seat(
    edge: &LayoutEdge,
    placement: EdgePlacement,
    bounds: &[Rect],
    walls: &[Wall],
    margin: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<LayoutLabel>> {
    let (Some(label), [start, mid, end]) = (&edge.label, edge.points.as_slice()) else {
        return Ok(None);
    };
    let bodies = placement.bodies;
    if bodies[0] == bodies[1] || (label.x, label.y) != (mid.x, mid.y) {
        return Ok(None);
    }
    let horizontal = (end.x - start.x).abs() >= (end.y - start.y).abs();
    let project = |rect: Rect| {
        if horizontal {
            ([rect.x0, rect.x1], [rect.y0, rect.y1])
        } else {
            ([rect.y0, rect.y1], [rect.x0, rect.x1])
        }
    };
    let [a, b] = bodies.map(|node| project(bounds[node as usize]).0);
    let gap = [a[1].min(b[1]), a[0].max(b[0])];
    let caption = label_box(label);
    work.charge_adapter(walls.len())?;
    let blocked = walls.iter().any(|wall| {
        let endpoint = match wall.owner {
            ObstacleOwner::Node(node) | ObstacleOwner::GroupTitle(node) => bodies.contains(&node),
            _ => false,
        };
        intersects(caption, wall.rect) && (!endpoint || gap[0] < gap[1])
    });
    if !blocked {
        if gap[0] >= gap[1] {
            return Ok(None);
        }
        let centers = [Point::new(start.x, start.y), Point::new(end.x, end.y)];
        for pass in LabelPass::candidates(label, centers, margin) {
            if pass.clear(walls, placement.occurrence, margin, work)? {
                return Ok(None);
            }
        }
    }

    // A bridge knot and the router's obstacle halo each need one margin.
    let area = caption.inflate(2.0 * margin, 2.0 * margin);
    let (along, across) = project(area);
    let initial = across[0].midpoint(across[1]);
    let along_center = if gap[0] < gap[1] {
        gap[0].midpoint(gap[1])
    } else {
        along[0].midpoint(along[1])
    };
    let half_along = (along[1] - along[0]) / 2.0;
    let half_across = (across[1] - across[0]) / 2.0;
    work.charge_adapter(walls.len())?;
    let mut occupied: Vec<_> = walls
        .iter()
        .filter_map(|wall| {
            let ([lo, hi], [a, b]) = project(wall.rect);
            (lo < along_center + half_along && along_center - half_along < hi)
                .then_some([a - half_across, b + half_across])
        })
        .collect();
    let levels = occupied.len().checked_ilog2().map_or(0, |n| n as usize + 1);
    work.charge_adapter(work.checked_mul(occupied.len(), levels)?)?;
    occupied.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]));
    let cross_center = closest_free(initial, &occupied);
    let center = if horizontal {
        Point::new(along_center, cross_center)
    } else {
        Point::new(cross_center, along_center)
    };
    Ok(
        (center.is_finite() && (center.x, center.y) != (label.x, label.y)).then_some(LayoutLabel {
            x: center.x,
            y: center.y,
            ..*label
        }),
    )
}

fn closest_free(initial: f64, occupied: &[[f64; 2]]) -> f64 {
    let mut intervals = occupied.iter().copied().peekable();
    while let Some([lo, mut hi]) = intervals.next() {
        if initial < lo {
            break;
        }
        while let Some(&[start, end]) = intervals.peek() {
            if start > hi {
                break;
            }
            hi = hi.max(end);
            intervals.next();
        }
        if initial <= hi {
            let options = [lo.next_down(), hi.next_up()];
            return options
                .into_iter()
                .filter(|end| end.is_finite())
                .min_by(|a, b| (initial - a).abs().total_cmp(&(initial - b).abs()))
                .unwrap_or(initial);
        }
    }
    initial
}
