//! Source AvoidOverlap edge router, including shared gap updates and cycle side channels.

use super::*;
use std::collections::BTreeMap;

const END_TEXTURE: f64 = 7.0;

pub(super) fn route(
    c: &mut Component,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    start_points(c, options, work)?;
    special_edges(c, options, work)?;
    end_points(c, options, work)?;
    for i in 0..c.edges.len() {
        work.charge(1)?;
        if c.edges[i].points.len() < 2 {
            middle_to_middle(c, i);
        }
    }
    Ok(())
}

fn node_edges(
    c: &Component,
    n: usize,
    incoming: bool,
    work: &mut dyn WorkControl,
) -> Result<Vec<usize>, Error> {
    work.charge(checked_add(
        c.edge_order.len(),
        checked_n_log_n(c.edge_order.len())?,
    )?)?;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for &i in &c.edge_order {
        let e = &c.edges[i];
        let source = &c.nodes[e.source];
        let target = &c.nodes[e.target];
        if (if incoming { target.id } else { source.id }) == c.nodes[n].id
            && source.level != target.level
            && (incoming || !source.super_root)
            && seen.insert((&source.identity, &target.identity))
        {
            result.push(i);
        }
    }
    result.sort_by(|&a, &b| {
        let endpoint_a = if incoming {
            c.edges[a].source
        } else {
            c.edges[a].target
        };
        let endpoint_b = if incoming {
            c.edges[b].source
        } else {
            c.edges[b].target
        };
        c.nodes[endpoint_a]
            .position
            .x
            .total_cmp(&c.nodes[endpoint_b].position.x)
    });
    Ok(result)
}

fn first_point(c: &Component, edge: usize) -> Point {
    c.edges[edge]
        .points
        .first()
        .copied()
        .unwrap_or(c.nodes[c.edges[edge].target].position)
}

fn last_point(c: &Component, edge: usize) -> Point {
    c.edges[edge]
        .points
        .last()
        .copied()
        .unwrap_or(c.nodes[c.edges[edge].source].position)
}

fn start_points(
    c: &mut Component,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let d = options.direction;
    let spacing = options.edge_node_spacing;
    for n in 0..c.nodes.len() {
        if c.nodes[n].super_root {
            continue;
        }
        let mut outs = node_edges(c, n, false, work)?;
        outs.sort_by(|&a, &b| {
            d.cross(first_point(c, a))
                .total_cmp(&d.cross(first_point(c, b)))
        });
        let count = outs.len();
        work.charge(checked_mul(count, 2)?)?;
        for (i, edge) in outs.into_iter().enumerate() {
            let fraction = if count == 1 {
                0.5
            } else {
                (i + 1) as f64 / (count + 1) as f64
            };
            let node = &c.nodes[n];
            let cross = d.cross(node.position) + d.cross(node.size()) * fraction;
            let (start, next) = if d.reverse() {
                (
                    d.along(node.position),
                    node.level_min.min(d.along(node.position) - spacing),
                )
            } else {
                (
                    d.along(node.position) + d.along(node.size()),
                    node.level_max + spacing,
                )
            };
            c.edges[edge].points.push(d.point(cross, start));
            c.edges[edge].points.push(d.point(cross, next));
        }
    }
    Ok(())
}

struct Gap {
    before: Option<usize>,
    after: Option<usize>,
    bends: Vec<(usize, usize)>,
}

fn special_edges(
    c: &mut Component,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let d = options.direction;
    let mut sides = [0usize; 2];
    let mut outgoing = vec![0usize; c.nodes.len()];
    let mut incoming = vec![0usize; c.nodes.len()];
    let mut gaps: BTreeMap<(usize, usize), Gap> = BTreeMap::new();
    work.charge(c.edge_order.len())?;
    let mut seen = BTreeSet::new();
    let order: Vec<_> = c
        .edge_order
        .iter()
        .copied()
        .filter(|&e| seen.insert(e))
        .collect();
    for edge in order {
        work.charge(1)?;
        let source = c.edges[edge].source;
        let target = c.edges[edge].target;
        let source_level = c.nodes[source].level;
        let target_level = c.nodes[target].level;
        if target_level > source_level + 1 {
            for level in source_level + 1..target_level {
                work.charge(checked_add(c.nodes.len(), checked_n_log_n(c.nodes.len())?)?)?;
                let mut next: Vec<_> = (0..c.nodes.len())
                    .filter(|&n| c.nodes[n].level == level)
                    .collect();
                next.sort_by(|&a, &b| {
                    d.cross(c.nodes[a].position)
                        .total_cmp(&d.cross(c.nodes[b].position))
                });
                let interpolation =
                    (level - source_level) as f64 / (target_level - source_level) as f64;
                let threshold = d.cross(c.nodes[source].position) * (1.0 - interpolation)
                    + d.cross(c.nodes[target].position) * interpolation;
                let slot = next
                    .iter()
                    .position(|&n| d.cross(c.nodes[n].position) > threshold)
                    .unwrap_or(next.len());
                if let (Some(&first), Some(&last)) = (next.first(), next.last()) {
                    let start = last_point(c, edge);
                    let last_end = Point {
                        x: c.nodes[last].position.x + c.nodes[last].width,
                        y: c.nodes[last].position.y + c.nodes[last].height,
                    };
                    let first_end = Point {
                        x: c.nodes[first].position.x + c.nodes[first].width,
                        y: c.nodes[first].position.y + c.nodes[first].height,
                    };
                    if slot >= next.len() - 1
                        && d.cross(start) > d.cross(last_end)
                        && d.cross(c.nodes[target].position) > d.cross(last_end)
                    {
                        continue;
                    }
                    // Preserve the horizontal branch's source x/y asymmetry.
                    if slot == 0
                        && d.cross(start) < first_end.x
                        && d.cross(c.nodes[target].position) < d.cross(first_end)
                    {
                        continue;
                    }
                }
                work.charge(2)?;
                let index = c.edges[edge].points.len();
                c.edges[edge]
                    .points
                    .extend([Point::default(), Point::default()]);
                let gap = gaps.entry((level, slot)).or_insert_with(|| Gap {
                    before: slot.checked_sub(1).and_then(|i| next.get(i).copied()),
                    after: next.get(slot).copied(),
                    bends: Vec::new(),
                });
                gap.bends.push((edge, index));
                work.charge(checked_add(
                    gap.bends.len(),
                    checked_n_log_n(gap.bends.len())?,
                )?)?;
                gap.bends.sort_by(|&(a, _), &(b, _)| {
                    d.cross(c.nodes[c.edges[a].target].position)
                        .total_cmp(&d.cross(c.nodes[c.edges[b].target].position))
                });
                update_gap(c, gap, options);
                if gap.before.is_none()
                    && gap
                        .after
                        .is_some_and(|n| d.cross(c.nodes[n].position) <= d.cross(c.min))
                {
                    sides[0] += 1;
                }
                if gap.after.is_none()
                    && gap.before.is_some_and(|n| {
                        d.cross(c.nodes[n].position) + d.cross(c.nodes[n].size()) >= d.cross(c.max)
                    })
                {
                    sides[1] += 1;
                }
            }
        } else if target_level == source_level {
            middle_to_middle(c, edge);
        } else if target_level < source_level {
            outgoing[source_level] += 1;
            incoming[target_level] += 1;
            cycle(
                c,
                edge,
                options,
                &mut sides,
                incoming[target_level],
                outgoing[source_level],
                work,
            )?;
        }
    }
    Ok(())
}

fn update_gap(c: &mut Component, gap: &Gap, options: &Options) {
    let d = options.direction;
    let spacing = options.edge_node_spacing;
    for (i, &(edge, index)) in gap.bends.iter().enumerate() {
        let fraction = (i + 1) as f64 / (gap.bends.len() + 1) as f64;
        let (node, cross) = match (gap.before, gap.after) {
            (None, None) => return,
            (Some(before), None) => (
                before,
                d.cross(c.nodes[before].position)
                    + d.cross(c.nodes[before].size())
                    + spacing * (i + 1) as f64,
            ),
            (None, Some(after)) => (
                after,
                d.cross(c.nodes[after].position) - spacing * (i + 1) as f64,
            ),
            (Some(before), Some(after)) => (
                before,
                d.cross(c.nodes[after].position) * fraction
                    + (d.cross(c.nodes[before].position) + d.cross(c.nodes[before].size()))
                        * (1.0 - fraction),
            ),
        };
        let low = c.nodes[node].level_min - spacing;
        let high = c.nodes[node].level_max + spacing;
        let (first, second) = if d.reverse() {
            (high, low)
        } else {
            (low, high)
        };
        c.edges[edge].points[index] = d.point(cross, first);
        c.edges[edge].points[index + 1] = d.point(cross, second);
    }
}

fn cycle(
    c: &mut Component,
    edge: usize,
    options: &Options,
    sides: &mut [usize; 2],
    incoming: usize,
    outgoing: usize,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    work.charge(checked_add(c.nodes.len(), 6)?)?;
    let d = options.direction;
    let spacing = options.edge_node_spacing;
    let s = &c.nodes[c.edges[edge].source];
    let t = &c.nodes[c.edges[edge].target];
    let middle = c.nodes.iter().map(|n| d.cross(n.center())).sum::<f64>() / c.nodes.len() as f64;
    let side = usize::from(d.cross(s.center()) > middle);
    sides[side] += 1;
    let bend = if side == 1 {
        d.cross(c.max) + spacing * sides[1] as f64
    } else {
        d.cross(c.min) - spacing * sides[0] as f64
    };
    let points = &mut c.edges[edge].points;
    match d {
        Direction::Left => points.extend([
            Point {
                x: s.level_min - spacing,
                y: bend,
            },
            Point {
                x: t.position.x + t.width + spacing + END_TEXTURE,
                y: bend,
            },
            Point {
                x: t.position.x + t.width + spacing + END_TEXTURE,
                y: t.center().y,
            },
            Point {
                x: t.position.x + t.width,
                y: t.center().y,
            },
        ]),
        Direction::Right => points.extend([
            Point {
                x: s.level_max + spacing,
                y: s.center().y,
            },
            Point {
                x: s.position.x + s.width + spacing,
                y: bend,
            },
            Point {
                x: t.position.x - spacing - END_TEXTURE,
                y: bend,
            },
            Point {
                x: t.position.x - spacing - END_TEXTURE,
                y: t.center().y,
            },
            Point {
                x: t.position.x,
                y: t.center().y,
            },
        ]),
        Direction::Up => points.extend([
            Point {
                x: bend,
                y: s.level_min - spacing,
            },
            Point {
                x: bend,
                y: t.position.y + t.height + spacing + END_TEXTURE,
            },
            Point {
                x: t.center().x,
                y: t.position.y + t.height + spacing + END_TEXTURE,
            },
            Point {
                x: t.center().x,
                y: t.position.y + t.height + spacing,
            },
        ]),
        Direction::Down => {
            if let Some(last) = points.last_mut() {
                last.y = s.level_max + spacing * outgoing as f64;
            }
            points.extend([
                Point {
                    x: bend,
                    y: s.level_max + spacing * outgoing as f64,
                },
                Point {
                    x: bend,
                    y: t.position.y - spacing * incoming as f64 - END_TEXTURE,
                },
            ]);
        }
    }
    Ok(())
}

fn end_points(
    c: &mut Component,
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let d = options.direction;
    let spacing = options.edge_node_spacing;
    for n in 0..c.nodes.len() {
        if c.nodes[n].super_root {
            continue;
        }
        let mut ins = node_edges(c, n, true, work)?;
        ins.sort_by(|&a, &b| {
            d.cross(last_point(c, a))
                .total_cmp(&d.cross(last_point(c, b)))
        });
        let count = ins.len();
        work.charge(checked_mul(count, 2)?)?;
        for (i, edge) in ins.into_iter().enumerate() {
            let fraction = if count == 1 {
                0.5
            } else {
                (i + 1) as f64 / (count + 1) as f64
            };
            let node = &c.nodes[n];
            let cross = d.cross(node.position) + d.cross(node.size()) * fraction;
            let along = d.along(node.position)
                + if d.reverse() {
                    d.along(node.size())
                } else {
                    0.0
                };
            let level_start = if d.reverse() {
                node.level_max
            } else {
                node.level_min
            };
            let sign = if d.reverse() { 1.0 } else { -1.0 };
            let last = last_point(c, edge);
            let cycle =
                d.along(node.position) - d.along(c.nodes[c.edges[edge].source].position) <= 0.0;
            if d == Direction::Down && cycle {
                c.edges[edge].points.push(d.point(cross, d.along(last)));
            } else if (along - level_start) * -sign > END_TEXTURE {
                c.edges[edge]
                    .points
                    .push(d.point(cross, level_start + sign * spacing));
            } else if !c.edges[edge].points.is_empty() {
                let center = node.center();
                if (d.cross(last) - d.cross(center)).abs()
                    / ((d.along(last) - d.along(center)).abs() / 40.0)
                    > 50.0
                {
                    let cross_shift = if d.cross(center) > d.cross(last) {
                        -END_TEXTURE / 2.0
                    } else {
                        END_TEXTURE / 2.0
                    };
                    c.edges[edge]
                        .points
                        .push(d.point(cross + cross_shift, along + sign * END_TEXTURE / 5.3));
                }
            }
            c.edges[edge].points.push(d.point(cross, along));
        }
    }
    Ok(())
}

fn to_border(center: Point, next: Point, size: Point) -> Point {
    let dx = (next.x - center.x).abs();
    let dy = (next.y - center.y).abs();
    let sx = if dx > size.x / 2.0 {
        size.x / 2.0 / dx
    } else {
        1.0
    };
    let sy = if dy > size.y / 2.0 {
        size.y / 2.0 / dy
    } else {
        1.0
    };
    let scale = sx.min(sy);
    Point {
        x: center.x + scale * (next.x - center.x),
        y: center.y + scale * (next.y - center.y),
    }
}

fn middle_to_middle(c: &mut Component, edge: usize) {
    let source = &c.nodes[c.edges[edge].source];
    let target = &c.nodes[c.edges[edge].target];
    let s = source.center();
    let t = target.center();
    let points = &mut c.edges[edge].points;
    points.insert(0, s);
    points.push(t);
    points[0] = to_border(s, points[1], source.size());
    let end = points.len() - 1;
    points[end] = to_border(t, points[end - 1], target.size());
}
