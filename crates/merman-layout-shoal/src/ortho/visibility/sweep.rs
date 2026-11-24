//! Sweep each axis to find the visible runs of obstacle corners and the
//! midlines between opposing faces. Only the seed ordinates are compressed.

use super::Bounds;
use crate::model::Axis;
use crate::ortho::{OrthoSegment, axis_id, reset, sort, zero};
use crate::work::{LayoutResult, Work, WorkError, filled, push, reserved};

#[derive(Clone, Copy, PartialEq)]
struct Seed {
    along: f64,
    fixed: f64,
    corner: bool,
    outward: usize,
}

pub(super) fn corridors(rects: &[Bounds], work: &mut Work<'_>) -> LayoutResult<Vec<OrthoSegment>> {
    let mut lines = Vec::new();
    let mut midlines = [Vec::new(), Vec::new()];
    let count = rects
        .len()
        .checked_mul(6)
        .ok_or(WorkError::ArithmeticOverflow)?;
    let mut seeds = reserved(count)?;
    for axis in [Axis::Horizontal, Axis::Vertical] {
        seeds.clear();
        for &r in rects {
            work.visit(1)?;
            let (along, fixed) = r.project(axis);
            for (outward, along) in along.into_iter().enumerate() {
                for fixed in fixed {
                    seeds.push(Seed {
                        along,
                        fixed,
                        corner: true,
                        outward,
                    });
                }
                seeds.push(Seed {
                    along,
                    fixed: zero(fixed[0].midpoint(fixed[1])),
                    corner: false,
                    outward,
                });
            }
        }
        let rays = trace(axis, rects, &mut seeds, work)?;
        for (&seed, ray) in seeds.iter().zip(rays) {
            work.visit(1)?;
            let Some([min, max]) = ray else {
                continue;
            };
            if seed.corner && min < max {
                push(
                    &mut lines,
                    OrthoSegment {
                        axis,
                        fixed: seed.fixed,
                        min,
                        max,
                    },
                )?;
            }
            let wall = [min, max][seed.outward];
            if wall.is_finite() && wall != seed.along {
                let fixed = zero(seed.along.midpoint(wall));
                if fixed != seed.along && fixed != wall {
                    push(
                        &mut midlines[1 - axis_id(axis)],
                        Seed {
                            along: seed.fixed,
                            fixed,
                            corner: false,
                            outward: 0,
                        },
                    )?;
                }
            }
        }
    }
    for (axis, seeds) in [Axis::Horizontal, Axis::Vertical]
        .into_iter()
        .zip(&mut midlines)
    {
        let rays = trace(axis, rects, seeds, work)?;
        for (&seed, ray) in seeds.iter().zip(rays) {
            work.visit(1)?;
            if let Some([min, max]) = ray
                && min < max
            {
                push(
                    &mut lines,
                    OrthoSegment {
                        axis,
                        fixed: seed.fixed,
                        min,
                        max,
                    },
                )?;
            }
        }
    }
    Ok(lines)
}

struct Block {
    along: [f64; 2],
    first: usize,
    end: usize,
}

#[derive(Clone, Copy)]
struct Event {
    at: f64,
    first: usize,
    end: usize,
    opens: bool,
}

#[derive(Clone, Copy)]
struct Cell {
    covered: usize,
    last_wall: f64,
}

const EMPTY: Cell = Cell {
    covered: 0,
    last_wall: f64::NEG_INFINITY,
};

/// Forward and reverse sweeps get the nearest wall on either side of each seed.
/// A range update touches only O(log n) tree cells, independent of its height.
fn trace(
    axis: Axis,
    rects: &[Bounds],
    seeds: &mut Vec<Seed>,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Option<[f64; 2]>>> {
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    sort(
        seeds,
        |a, b| {
            a.along
                .total_cmp(&b.along)
                .then_with(|| a.fixed.total_cmp(&b.fixed))
                .then_with(|| a.corner.cmp(&b.corner))
                .then_with(|| a.outward.cmp(&b.outward))
        },
        work,
    )?;
    work.charge(seeds.len())?;
    seeds.dedup();
    let mut fixed = reserved(seeds.len())?;
    for seed in seeds.iter() {
        work.visit(1)?;
        fixed.push(seed.fixed);
    }
    sort(&mut fixed, f64::total_cmp, work)?;
    work.charge(fixed.len())?;
    fixed.dedup();

    let mut blocks = reserved(rects.len())?;
    let lookup = fixed.len().ilog2() as usize + 1;
    for &r in rects {
        work.visit(2 * lookup)?;
        let (along, across) = r.project(axis);
        let first = fixed.partition_point(|&x| x <= across[0]);
        let end = fixed.partition_point(|&x| x < across[1]);
        if first < end {
            blocks.push(Block { along, first, end });
        }
    }
    if blocks.is_empty() {
        work.charge(seeds.len())?;
        return filled(seeds.len(), Some([f64::NEG_INFINITY, f64::INFINITY]));
    }
    let size = fixed
        .len()
        .checked_next_power_of_two()
        .ok_or(WorkError::ArithmeticOverflow)?;
    let cells = size.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?;
    let mut events = reserved(
        blocks
            .len()
            .checked_mul(2)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    let mut tree = Vec::new();
    work.charge(seeds.len())?;
    let mut rays = filled(seeds.len(), Some([f64::NEG_INFINITY, f64::INFINITY]))?;
    for reverse in [false, true] {
        reset(&mut tree, cells, EMPTY, work)?;
        events.clear();
        for block in &blocks {
            work.visit(1)?;
            let [a, b] = block.along;
            let [start, end] = if reverse {
                [zero(-b), zero(-a)]
            } else {
                [a, b]
            };
            for (at, opens) in [(start, true), (end, false)] {
                events.push(Event {
                    at,
                    first: block.first,
                    end: block.end,
                    opens,
                });
            }
        }
        sort(
            &mut events,
            |a, b| a.at.total_cmp(&b.at).then_with(|| a.opens.cmp(&b.opens)),
            work,
        )?;
        let mut next = 0;
        for i in 0..seeds.len() {
            let i = if reverse { seeds.len() - 1 - i } else { i };
            let seed = seeds[i];
            let at = if reverse { -seed.along } else { seed.along };
            // Ends before a seed and starts after it keep exact wall faces legal.
            while let Some(&e) = events.get(next) {
                if e.at > at || (e.at == at && e.opens) {
                    break;
                }
                update(&mut tree, e, size, work)?;
                next += 1;
            }
            work.visit(lookup)?;
            let mut index = size + fixed.partition_point(|&x| x < seed.fixed);
            let mut wall = f64::NEG_INFINITY;
            let mut blocked = false;
            while index != 0 {
                work.visit(1)?;
                let cell = tree[index];
                blocked |= cell.covered != 0;
                wall = wall.max(cell.last_wall);
                index /= 2;
            }
            if blocked {
                rays[i] = None;
            } else if let Some(span) = &mut rays[i] {
                span[usize::from(reverse)] = if reverse { zero(-wall) } else { wall };
            }
        }
    }
    Ok(rays)
}

fn update(tree: &mut [Cell], event: Event, size: usize, work: &mut Work<'_>) -> LayoutResult<()> {
    let (mut first, mut end) = (size + event.first, size + event.end);
    let touch = |cell: &mut Cell| {
        if event.opens {
            cell.covered += 1;
        } else {
            cell.covered -= 1;
            cell.last_wall = event.at;
        }
    };
    while first < end {
        work.visit(1)?;
        if first % 2 == 1 {
            touch(&mut tree[first]);
            first += 1;
        }
        if end % 2 == 1 {
            end -= 1;
            touch(&mut tree[end]);
        }
        first /= 2;
        end /= 2;
    }
    Ok(())
}
