use super::*;
use crate::work::push;
use crate::{MarkerMetrics, Options};

#[derive(Clone, Copy)]
struct Slot {
    wire: usize,
    end: usize,
    at: f64,
    fixed: bool,
}

type Faces = [Vec<Slot>; 4];

fn clear_across(mut at: f64, fixed: &[f64], pitch: f64, work: &mut Work<'_>) -> LayoutResult<f64> {
    work.visit(fixed.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
    if pitch > 0.0 {
        let first = fixed.partition_point(|&other| other <= at - pitch);
        for &other in &fixed[first..] {
            work.visit(1)?;
            if at + pitch <= other {
                break;
            }
            at = finite(other + pitch)?;
        }
    } else if pitch < 0.0 {
        let last = fixed.partition_point(|&other| other < at - pitch);
        for &other in fixed[..last].iter().rev() {
            work.visit(1)?;
            if other <= at + pitch {
                break;
            }
            at = finite(other + pitch)?;
        }
    }
    Ok(at)
}

fn spread(
    slots: &mut [Slot],
    low: f64,
    high: f64,
    pitch: f64,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if slots.iter().all(|slot| slot.fixed) {
        return Ok(());
    }
    work.charge(slots.len())?;
    let mut order = reserved(slots.len())?;
    order.extend(0..slots.len());
    work.sort(&mut order, |&left, &right| {
        slots[left]
            .at
            .total_cmp(&slots[right].at)
            .then(left.cmp(&right))
    })?;
    let mut fixed = reserved(slots.len())?;
    fixed.extend(
        order
            .iter()
            .filter(|&&index| slots[index].fixed)
            .map(|&index| slots[index].at),
    );
    order.retain(|&index| !slots[index].fixed);
    let mut ceiling = filled(order.len(), high)?;
    let span = (high - low).max(0.0);
    let preferred = if slots.len() > 1 {
        pitch.min(span / (slots.len() - 1) as f64)
    } else {
        pitch
    };
    let conservative = preferred.min(span / (fixed.len() as f64 * 2.0 + order.len() as f64 + 1.0));
    for pitch in [preferred, conservative, 0.0] {
        let mut upper = high;
        for bound in ceiling.iter_mut().rev() {
            work.visit(1)?;
            *bound = clear_across(upper, &fixed, -pitch, work)?;
            upper = *bound - pitch;
        }
        if ceiling[0] < low {
            continue;
        }
        let mut lower = low;
        let mut start = 0;
        while start < order.len() {
            let want = slots[order[start]].at;
            let mut stop = start + 1;
            while stop < order.len() && slots[order[stop]].at == want {
                stop += 1;
            }
            let middle = (stop - start - 1) as f64 / 2.0;
            for index in start..stop {
                work.visit(1)?;
                let wanted = want + ((index - start) as f64 - middle) * pitch;
                let candidate =
                    clear_across(wanted.max(lower).min(ceiling[index]), &fixed, pitch, work)?;
                let chosen = candidate.min(ceiling[index]).clamp(low, high);
                slots[order[index]].at = chosen;
                lower = chosen + pitch;
            }
            start = stop;
        }
        break;
    }
    Ok(())
}

fn marker_blockers(
    contact: Contact,
    marker: MarkerMetrics,
    clearance: f64,
    walls: &[Rect],
    blocked: &mut Vec<(f64, f64)>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    blocked.clear();
    let side = contact.side;
    let along = |point: Point| match side.axis() {
        Axis::Vertical => point.y,
        Axis::Horizontal => point.x,
    };
    let start = along(contact.at);
    let end = along(towards(
        contact.at,
        side,
        finite(marker.length + clearance)?,
    ));
    let half_width = marker.width / 2.0 + clearance;
    for wall in walls {
        work.visit(1)?;
        if wall.width <= 0.0 || wall.height <= 0.0 {
            continue;
        }
        let near = Point {
            x: wall.x,
            y: wall.y,
        };
        let far = Point {
            x: wall.x + wall.width,
            y: wall.y + wall.height,
        };
        if along(near) < start.max(end) && along(far) > start.min(end) {
            push(
                blocked,
                (
                    cross(side, near) - half_width,
                    cross(side, far) + half_width,
                ),
            )?;
        }
    }
    Ok(())
}

fn closest_clear_port(
    preferred: f64,
    (low, high): (f64, f64),
    blocked: &[(f64, f64)],
    work: &mut Work<'_>,
) -> LayoutResult<Option<f64>> {
    work.charge(blocked.len())?;
    let mut order = reserved(blocked.len())?;
    order.extend(0..blocked.len());
    work.sort(&mut order, |&left, &right| {
        blocked[left].0.total_cmp(&blocked[right].0)
    })?;
    let mut best = None::<f64>;
    let mut consider = |first: f64, last: f64| {
        if first <= last {
            let candidate = preferred.clamp(first, last);
            if best.is_none_or(|best| (preferred - candidate).abs() < (preferred - best).abs()) {
                best = Some(candidate);
            }
        }
    };
    let mut first = low;
    for index in order {
        work.visit(1)?;
        let (left, right) = blocked[index];
        if left > high {
            break;
        }
        consider(first, left);
        first = first.max(right);
    }
    consider(first, high);
    Ok(best)
}

pub(super) fn items(
    wires: &mut [Wire],
    graph: &Graph,
    rects: &[Rect],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let nodes = &graph.nodes;
    let custom = wires.iter().any(|wire| !wire.flat() || wire.own);
    work.charge(wires.len())?;
    let mut faces = if custom {
        filled::<Faces>(rects.len(), Default::default())?
    } else {
        Vec::new()
    };
    for (index, wire) in wires.iter_mut().enumerate() {
        for end in 0..2 {
            work.visit(1)?;
            let End::Item { slot, hint, child } = wire.ends[end] else {
                continue;
            };
            let rect = rects[slot];
            let old = wire.route.as_ref().map(|route| {
                let (knot, side) = if end == 0 {
                    (route.points.first(), route.ports.0)
                } else {
                    (route.points.last(), route.ports.1)
                };
                Contact {
                    at: point(*knot.expect("flat contact")),
                    side,
                }
            });
            let mut fixed = true;
            let contact = if let Some(child) = child {
                Contact {
                    at: translated(
                        child.at,
                        Point {
                            x: rect.x,
                            y: rect.y,
                        },
                    ),
                    ..child
                }
            } else if let Some(old) = old
                && !wire.own
            {
                old
            } else {
                let side = named(hint)
                    .unwrap_or_else(|| wire.border().expect("special leg has a border").2);
                let offset = match hint {
                    PortSideHint::FixedOffset { offset, .. } => Some(offset),
                    _ if side.axis() == Axis::Horizontal => nodes[slot]
                        .anchor_y
                        .map(|anchor| anchor - rect.height / 2.0),
                    _ => None,
                };
                fixed = offset.is_some();
                Contact {
                    at: face(
                        rect,
                        side,
                        cross(side, rect.center()) + offset.unwrap_or(0.0),
                    ),
                    side,
                }
            };
            wire.contacts[end] = Some(contact);
            if custom {
                push(
                    &mut faces[slot][contact.side as usize],
                    Slot {
                        wire: index,
                        end,
                        at: cross(contact.side, contact.at),
                        fixed,
                    },
                )?;
            }
        }
    }
    let mut walls = None;
    let mut blocked = Vec::new();
    for (slot, faces) in faces.iter_mut().enumerate() {
        let rect = rects[slot];
        for (slots, side) in faces.iter_mut().zip(SIDES) {
            work.visit(slots.len() + 1)?;
            if slots.iter().all(|slot| slot.fixed) {
                continue;
            }
            let length = across(side, nodes[slot].size);
            let measured = if flow_sides(graph.direction).0.axis() == side.axis() {
                nodes[slot].port_span
            } else {
                nodes[slot]
                    .perpendicular_port_span
                    .or(nodes[slot].port_span)
            };
            let span = measured
                .unwrap_or(length - 2.0 * options.spacing.corner_clearance)
                .clamp(0.0, length);
            let center = cross(side, rect.center());
            let bounds = (center - span / 2.0, center + span / 2.0);
            let pitch = options.spacing.port_pitch;
            spread(slots, bounds.0, bounds.1, pitch, work)?;
            for index in 0..slots.len() {
                let entry = slots[index];
                let wire = &wires[entry.wire];
                let marker = [wire.spec.start_marker, wire.spec.end_marker][entry.end];
                if entry.fixed || marker.length == 0.0 || span == 0.0 {
                    continue;
                }
                if walls.is_none() {
                    walls = Some(routes::item_boxes(rects, wires, work)?);
                }
                marker_blockers(
                    Contact {
                        at: face(rect, side, entry.at),
                        side,
                    },
                    marker,
                    options.spacing.edge,
                    walls.as_deref().unwrap(),
                    &mut blocked,
                    work,
                )?;
                work.visit(blocked.len())?;
                if blocked
                    .iter()
                    .any(|&(left, right)| left < entry.at && entry.at < right)
                {
                    for (other_index, other) in slots.iter().enumerate() {
                        work.visit(1)?;
                        let separation = (other.at - entry.at).abs().min(pitch);
                        if other_index != index && separation > 0.0 {
                            push(&mut blocked, (other.at - separation, other.at + separation))?;
                        }
                    }
                    if let Some(clear) = closest_clear_port(entry.at, bounds, &blocked, work)? {
                        slots[index].at = clear;
                    }
                }
            }
            for entry in slots.iter().filter(|slot| !slot.fixed) {
                wires[entry.wire].contacts[entry.end] = Some(Contact {
                    at: face(rect, side, entry.at),
                    side,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn border(
    wires: &[Wire],
    nodes: &[Rect],
    fit: &fit::Fit,
    count: usize,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Point>> {
    work.charge(count)?;
    let mut result = filled(count, Point::default())?;
    let mut faces: Faces = Default::default();
    for wire in wires {
        work.visit(1)?;
        let Some((end, slot, side)) = wire.border() else {
            continue;
        };
        let End::Border { flat, hint, .. } = wire.ends[end] else {
            unreachable!()
        };
        let (at, fixed) = if let PortSideHint::FixedOffset { offset, .. } = hint {
            let half = across(side, fit.size) / 2.0;
            if offset.abs() > half {
                return Err(LayoutError::InvalidInput("port offset outside group face"));
            }
            (half + offset, true)
        } else {
            let natural = flat.map_or_else(
                || wire.contacts[1 - end].expect("inner contact").at,
                |node| nodes[node.0 as usize].center(),
            );
            (cross(side, translated(natural, fit.offset)), false)
        };
        push(
            &mut faces[side as usize],
            Slot {
                wire: slot,
                end,
                at,
                fixed,
            },
        )?;
    }
    let rect = Rect {
        width: fit.size.width,
        height: fit.size.height,
        ..Rect::default()
    };
    for (slots, side) in faces.iter_mut().zip(SIDES) {
        let length = across(side, fit.size);
        let inset = options.spacing.corner_clearance.min(length / 2.0);
        spread(
            slots,
            inset,
            length - inset,
            options.spacing.min_port_pitch,
            work,
        )?;
        for slot in slots {
            result[slot.wire] = face(rect, side, slot.at);
        }
    }
    Ok(result)
}

pub(super) fn beside_title(
    wires: &[Wire],
    ports: &mut [Point],
    title: Rect,
    side: Side,
    width: f64,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if title.width == 0.0 || title.height == 0.0 {
        return Ok(());
    }
    let mut slots = Vec::new();
    let mut obstructed = false;
    let mut clearance = fit::TITLE_CLEAR;
    for wire in wires {
        work.visit(1)?;
        let Some((end, slot, port_side)) = wire.border() else {
            continue;
        };
        if port_side != side {
            continue;
        }
        let End::Border { hint, .. } = wire.ends[end] else {
            unreachable!()
        };
        let fixed = matches!(hint, PortSideHint::FixedOffset { .. });
        let at = ports[slot].x;
        if !fixed {
            let marker = [wire.spec.start_marker, wire.spec.end_marker][end];
            let half = fit::TITLE_CLEAR + marker.width / 2.0;
            clearance = clearance.max(half);
            obstructed |= at > title.x - half && at < title.x + title.width + half;
        }
        push(
            &mut slots,
            Slot {
                wire: slot,
                end,
                at,
                fixed,
            },
        )?;
    }
    if !obstructed {
        return Ok(());
    }
    let inset = options.spacing.corner_clearance.min(width / 2.0);
    let left = (title.x - clearance).min(width - inset);
    let right = (title.x + title.width + clearance).max(inset);
    let pitch = options.spacing.min_port_pitch;
    let capacity = |available: f64| {
        if available < 0.0 {
            0
        } else if pitch > 0.0 {
            ((available / pitch).floor() as usize)
                .saturating_add(1)
                .min(slots.len())
        } else {
            slots.len()
        }
    };
    let left_count = capacity(left - inset);
    let right_count = capacity(width - inset - right);
    let mut order = reserved(slots.len())?;
    order.extend(0..slots.len());
    work.sort(&mut order, |&first, &second| {
        slots[first]
            .at
            .total_cmp(&slots[second].at)
            .then(first.cmp(&second))
    })?;
    let mut sorted = reserved(slots.len())?;
    sorted.extend(order.into_iter().map(|index| slots[index]));
    let middle = title.center().x;
    let lower = sorted.partition_point(|slot| slot.at < middle);
    let upper = sorted.partition_point(|slot| slot.at <= middle);
    let split = (lower + (upper - lower).div_ceil(2))
        .max(sorted.len().saturating_sub(right_count))
        .min(left_count);
    let (before, after) = sorted.split_at_mut(split);
    for (batch, lo, hi) in [(before, inset, left), (after, right, width - inset)] {
        if lo > hi {
            continue;
        }
        spread(batch, lo, hi, pitch, work)?;
        for slot in batch.iter().filter(|slot| !slot.fixed) {
            ports[slot.wire].x = slot.at;
        }
    }
    Ok(())
}
