use super::{Placement, Tile, aspect_measure, aspect_shelves, reserve_zero_extent, shelves};
use crate::work::{LayoutResult, Work, reserved};
use crate::{Point, Size};
use std::collections::VecDeque;

const WIDTH_SCALES: [f64; 4] = [0.75, 1.0, 1.25, 1.5];
const SPAN_VISITS_PER_TILE: usize = 32;
const MAX_SPAN_VISITS: usize = 131_072;

pub(super) fn balanced(
    tiles: &[Tile],
    gap: f64,
    target_ratio: f64,
    vertical: bool,
    work: &mut Work<'_>,
) -> LayoutResult<Placement> {
    if tiles.len() < 2 {
        return shelves(tiles, gap, f64::INFINITY, vertical, work);
    }

    let (area, widest) = aspect_measure(tiles, gap, vertical, work)?;
    let half_log_target = target_ratio.ln() * 0.5;
    let half_log_cross = if vertical {
        -half_log_target
    } else {
        half_log_target
    };
    let estimate = area.sqrt() * half_log_cross.exp();
    let mut best = aspect_shelves(
        tiles,
        gap,
        estimate.max(widest),
        vertical,
        2.0 * half_log_cross,
        work,
    )?;
    if tiles.len() > MAX_SPAN_VISITS || !estimate.is_finite() {
        return Ok(best);
    }

    // Minimize the enclosing rectangle at the requested aspect, then the occupied area.
    // Compare in log space to keep very large or small drawings representable.
    let score = |s: Size| {
        let (w, h) = (s.width.ln(), s.height.ln());
        if !w.is_finite() || !h.is_finite() {
            return (f64::INFINITY, f64::INFINITY);
        }
        ((w - half_log_target).max(h + half_log_target), w + h)
    };
    let mut best_score = score(best.1);
    let mut previous_limit = -1.0;
    for scale in WIDTH_SCALES {
        let limit = (estimate * scale).max(widest);
        if limit <= previous_limit || !limit.is_finite() {
            continue;
        }
        previous_limit = limit;
        if let Some(trial) = place(tiles, gap, limit, vertical, work)? {
            let trial_score = score(trial.1);
            if trial_score < best_score {
                best = trial;
                best_score = trial_score;
            }
        }
    }
    Ok(best)
}

/// A span begins at the preceding end, with a final zero-depth span extending to infinity.
#[derive(Clone, Copy)]
struct Span {
    end: f64,
    depth: f64,
}

fn place(
    tiles: &[Tile],
    gap: f64,
    limit: f64,
    vertical: bool,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Placement>> {
    let mut remaining = (tiles.len() * SPAN_VISITS_PER_TILE).min(MAX_SPAN_VISITS);
    let mut positions = reserved(tiles.len())?;
    let mut spans = reserved(tiles.len() + 1)?;
    let mut peaks = VecDeque::from(reserved(tiles.len() + 1)?);
    spans.push(Span {
        end: f64::INFINITY,
        depth: 0.0,
    });
    let (mut across, mut along) = (0.0_f64, 0.0_f64);
    for tile in tiles {
        // Both the search and the span replacement take at most linear work in the current
        // skyline. Each placement adds at most one span.
        if spans.len() > remaining {
            return Ok(None);
        }
        remaining -= spans.len();
        work.visit(spans.len())?;
        let b = tile.bounds;
        let (w, h) = orient(vertical, b.width, b.height);
        let Some((first, x, y)) = bottom_left(&spans, w, gap, limit, &mut peaks) else {
            return Ok(None);
        };
        let (right, bottom) = (x + w, y + h);
        let end = reserve_zero_extent(x, right + gap, w);
        let depth = reserve_zero_extent(y, bottom + gap, h);
        if !end.is_finite()
            || !depth.is_finite()
            || end <= x
            || depth <= y
            || (w > 0.0 && right <= x)
            || (h > 0.0 && bottom <= y)
            || (gap > 0.0 && (end <= right || depth <= bottom))
        {
            return Ok(None);
        }
        let (physical_x, physical_y) = orient(vertical, x, y);
        positions.push(Point {
            x: physical_x,
            y: physical_y,
        });
        across = across.max(right);
        along = along.max(bottom);
        // The trailing gap is reserved in both axes and excluded from the reported extent.
        raise(&mut spans, first, end, depth);
    }
    let (width, height) = orient(vertical, across, along);
    Ok(Some((positions, Size { width, height })))
}

fn orient(vertical: bool, x: f64, y: f64) -> (f64, f64) {
    if vertical { (y, x) } else { (x, y) }
}

fn bottom_left(
    spans: &[Span],
    width: f64,
    gap: f64,
    limit: f64,
    peaks: &mut VecDeque<usize>,
) -> Option<(usize, f64, f64)> {
    peaks.clear();
    let (mut x, mut next) = (0.0, 0);
    let mut best: Option<(usize, f64, f64)> = None;
    for (first, span) in spans.iter().enumerate() {
        let right = x + width;
        let end = reserve_zero_extent(x, right + gap, width);
        if right > limit || !end.is_finite() {
            break;
        }
        if peaks.front().is_some_and(|&i| i < first) {
            peaks.pop_front();
        }
        // Both ends of the footprint advance monotonically. Keep the highest support for
        // the moving window at the front of the deque.
        while next <= first || spans[next - 1].end < end {
            while peaks
                .back()
                .is_some_and(|&i| spans[i].depth <= spans[next].depth)
            {
                peaks.pop_back();
            }
            peaks.push_back(next);
            next += 1;
        }
        let y = spans[peaks[0]].depth;
        if best.is_none_or(|(_, _, best_y)| y < best_y) {
            best = Some((first, x, y));
        }
        if y == 0.0 {
            break;
        }
        x = span.end;
    }
    best
}

fn raise(spans: &mut Vec<Span>, mut first: usize, mut end: f64, depth: f64) {
    let mut past = spans.partition_point(|span| span.end <= end);
    if first > 0 && spans[first - 1].depth == depth {
        first -= 1;
    }
    if spans[past].depth == depth {
        end = spans[past].end;
        past += 1;
    }
    drop(spans.splice(first..past, [Span { end, depth }]));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoopWorkControl, Rect};

    #[test]
    fn skyline_keeps_zero_width_zero_depth_and_points_on_distinct_spans() {
        for vertical in [false, true] {
            for (w, h) in [(0.0, 20.0), (20.0, 0.0), (0.0, 0.0)] {
                let (width, height) = orient(vertical, w, h);
                let tiles: [Tile; 3] = std::array::from_fn(|_| Tile {
                    ids: Vec::new(),
                    ranks: Vec::new(),
                    drawing: Default::default(),
                    bounds: Rect {
                        width,
                        height,
                        ..Default::default()
                    },
                });
                let mut control = NoopWorkControl;
                let mut work = Work::new(&mut control);
                let (positions, bounds) =
                    place(&tiles, 0.0, w, vertical, &mut work).unwrap().unwrap();
                assert!(positions.windows(2).all(|pair| pair[0] != pair[1]));
                let last = positions[2];
                assert_eq!(
                    bounds,
                    Size {
                        width: last.x + width,
                        height: last.y + height
                    }
                );
            }
        }
    }
}
