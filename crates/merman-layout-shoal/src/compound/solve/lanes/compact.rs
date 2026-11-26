//! Shorten unused flow depth by moving whole rank bands together.

use super::*;
use crate::Spacing;
use crate::geom::Bounds;

const CLOSE: f64 = 3.0;

struct Gap {
    // The full opening between occupied bands. Any turn or label in it keeps the full gap.
    opening: [f64; 2],
    // Reserve an ordinary rank gap around the empty transition depth.
    middle: [f64; 2],
}

struct Close {
    from: f64,
    to: f64,
    previous: f64,
    removed: f64,
}

fn span(axes: Axes, bounds: Bounds) -> [f64; 2] {
    match axes.start.axis() {
        Axis::Vertical => bounds.y,
        Axis::Horizontal => bounds.x,
    }
}

fn openings(
    drawing: &LayoutOutput,
    ranks: &Expanded,
    axes: Axes,
    rank_spacing: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Gap>> {
    let count = drawing.node_ranks.len();
    work.charge(count)?;
    let mut order = reserved(count)?;
    order.extend(0..count);
    let reversed = matches!(axes.start, Side::Bottom | Side::Right);
    work.sort(&mut order, |&a, &b| {
        let order = drawing.node_ranks[a].cmp(&drawing.node_ranks[b]);
        if reversed { order.reverse() } else { order }
    })?;
    let mut gaps = Vec::new();
    let mut previous = None::<(u32, f64)>;
    for row in order.chunk_by(|&a, &b| drawing.node_ranks[a] == drawing.node_ranks[b]) {
        work.visit(row.len())?;
        let rank = drawing.node_ranks[row[0]];
        let band = row
            .iter()
            .fold([f64::INFINITY, f64::NEG_INFINITY], |[lo, hi], &node| {
                let [a, b] = span(axes, Bounds::of(drawing.nodes[node]));
                [lo.min(a), hi.max(b)]
            });
        if let Some((prior_rank, start)) = previous
            && let slots = ranks.empty_rank_slots(prior_rank, rank, work)?
            && slots != 0
        {
            let end = band[0];
            let empty_depth = (end - start - rank_spacing).min(slots as f64 * rank_spacing);
            if empty_depth > 0.0 {
                let half = empty_depth / 2.0;
                let center = start.midpoint(end);
                push(
                    &mut gaps,
                    Gap {
                        opening: [start, end],
                        middle: [center - half, center + half],
                    },
                )?;
            }
        }
        previous = Some((rank, band[1]));
    }
    Ok(gaps)
}

// Record intersections with ordered, disjoint intervals without walking across long spans.
fn hold(
    gaps: &[Gap],
    delta: &mut [i64],
    extent: [f64; 2],
    interval: impl Fn(&Gap) -> [f64; 2],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.visit(2 * (gaps.len().ilog2() as usize + 1))?;
    let first = gaps.partition_point(|gap| interval(gap)[1] <= extent[0]);
    let last = gaps.partition_point(|gap| interval(gap)[0] < extent[1]);
    if first < last {
        delta[first] += 1;
        delta[last] -= 1;
    }
    Ok(())
}

fn occupied(
    drawing: &LayoutOutput,
    graph: &Graph,
    axes: Axes,
    gaps: &[Gap],
    spacing: &Spacing,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<i64>> {
    work.charge(gaps.len() + 1)?;
    let mut delta = filled(gaps.len() + 1, 0_i64)?;
    for (spec, route) in graph.edges.iter().zip(&drawing.edges) {
        work.visit(route.points.len() + 1)?;
        let mut mark = |extent, whole| {
            hold(
                gaps,
                &mut delta,
                extent,
                |gap| if whole { gap.opening } else { gap.middle },
                work,
            )
        };
        for pair in route.points.windows(2) {
            let [a, b] = [point(pair[0]), point(pair[1])];
            if cross(axes.start, a) != cross(axes.start, b) {
                mark(span(axes, Bounds::between(a, b)), true)?;
            }
        }
        for (center, size) in [
            (route.label_center, spec.label),
            (route.start_label_center, spec.start_label),
            (route.end_label_center, spec.end_label),
        ] {
            if let (Some(center), Some(size)) = (center, size) {
                mark(span(axes, Bounds::centered(center, size)), true)?;
            }
        }
        for ((tip, side, marker), runway) in [
            (route.points[0], route.ports.0, spec.start_marker),
            (
                *route.points.last().expect("layered route"),
                route.ports.1,
                spec.end_marker,
            ),
        ]
        .into_iter()
        .zip(spec.marker_runways())
        {
            let lead = crate::MarkerMetrics {
                length: routing::runway(runway, spacing)?,
                ..marker
            };
            mark(span(axes, Bounds::marker(point(tip), side, lead)), false)?;
        }
    }
    Ok(delta)
}

pub(super) fn close(
    drawing: &mut LayoutOutput,
    graph: &Graph,
    ranks: &Expanded,
    axes: Axes,
    spacing: &Spacing,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if drawing.nodes.len() < 2 || spacing.rank == 0.0 {
        return Ok(());
    }
    let gaps = openings(drawing, ranks, axes, spacing.rank, work)?;
    if gaps.is_empty() {
        return Ok(());
    }
    let delta = occupied(drawing, graph, axes, &gaps, spacing, work)?;
    let mut changes = Vec::new();
    let mut busy = 0;
    let mut removed = 0.0;
    for (gap, change) in gaps.into_iter().zip(delta) {
        work.visit(1)?;
        busy += change;
        if busy == 0 {
            let [from, to] = gap.middle;
            let previous = removed;
            let width = to - from;
            removed = finite(removed + (width - width / CLOSE))?;
            push(
                &mut changes,
                Close {
                    from,
                    to,
                    previous,
                    removed,
                },
            )?;
        }
    }
    if changes.is_empty() {
        return Ok(());
    }
    let along = |at: &mut f64, work: &mut Work<'_>| -> LayoutResult<()> {
        work.visit(changes.len().ilog2() as usize + 1)?;
        let next = changes.partition_point(|change| change.to <= *at);
        let offset = match changes.get(next) {
            Some(change) if change.from < *at => {
                let into = *at - change.from;
                change.previous + (into - into / CLOSE)
            }
            Some(change) => change.previous,
            None => removed,
        };
        *at -= offset;
        Ok(())
    };
    let vertical = axes.start.axis() == Axis::Vertical;
    for rect in &mut drawing.nodes {
        along(if vertical { &mut rect.y } else { &mut rect.x }, work)?;
    }
    for route in &mut drawing.edges {
        for knot in &mut route.points {
            along(if vertical { &mut knot.y } else { &mut knot.x }, work)?;
        }
        for at in [
            &mut route.label_center,
            &mut route.start_label_center,
            &mut route.end_label_center,
        ]
        .into_iter()
        .flatten()
        {
            along(if vertical { &mut at.y } else { &mut at.x }, work)?;
        }
        route.corridor_clearance = 0.0;
    }
    let extent = if vertical {
        &mut drawing.bounds.height
    } else {
        &mut drawing.bounds.width
    };
    let total = changes.last().expect("nonempty compression").removed;
    *extent = finite(*extent - total)?;
    Ok(())
}
