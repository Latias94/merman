use super::*;
use crate::work::push;
use crate::{MarkerMetrics, Options};

#[derive(Clone, Copy)]
pub(super) struct Slot {
    pub(super) wire: usize,
    pub(super) end: usize,
    pub(super) at: f64,
    pub(super) fixed: bool,
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

pub(super) fn spread(
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

pub(super) fn closest_clear_port(
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

struct MarkerPorts {
    walls: Vec<Rect>,
    blocked: Vec<(f64, f64)>,
    clearance: f64,
}

#[derive(Clone, Copy)]
enum PortPitch {
    Preserve(f64),
    Reserve(f64),
}

impl MarkerPorts {
    fn at(
        &mut self,
        port: Contact,
        marker: MarkerMetrics,
        bounds: (f64, f64),
        neighbors: &[Slot],
        pitch: PortPitch,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<f64>> {
        let preferred = cross(port.side, port.at);
        let preserve = matches!(pitch, PortPitch::Preserve(_));
        let clear = |blocked: &[(f64, f64)], work: &mut Work<'_>| -> LayoutResult<bool> {
            work.visit(blocked.len())?;
            Ok(bounds.0 <= preferred
                && preferred <= bounds.1
                && blocked
                    .iter()
                    .all(|&(lo, hi)| preferred <= lo || preferred >= hi))
        };
        for clearance in [self.clearance, 0.0] {
            marker_blockers(
                port,
                marker,
                clearance,
                &self.walls,
                &mut self.blocked,
                work,
            )?;
            if preserve && clear(&self.blocked, work)? {
                return Ok(Some(preferred));
            }
            for neighbor in neighbors {
                work.visit(1)?;
                let separation = match pitch {
                    PortPitch::Preserve(pitch) => (neighbor.at - preferred).abs().min(pitch),
                    PortPitch::Reserve(pitch) => pitch,
                };
                if separation > 0.0 {
                    push(
                        &mut self.blocked,
                        (neighbor.at - separation, neighbor.at + separation),
                    )?;
                }
            }
            if !preserve && clear(&self.blocked, work)? {
                return Ok(Some(preferred));
            }
            if let Some(at) = closest_clear_port(preferred, bounds, &self.blocked, work)? {
                return Ok(Some(at));
            }
            if clearance == 0.0 {
                break;
            }
        }
        Ok(None)
    }
}

fn anchor_offset(node: &NodeSpec, rect: Rect, side: Side) -> Option<f64> {
    match side.axis() {
        Axis::Horizontal => node.anchor_y.map(|anchor| anchor - rect.height / 2.0),
        Axis::Vertical => None,
    }
}

pub(super) fn items(
    wires: &mut [Wire],
    graph: &Graph,
    rects: &[Rect],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let nodes = &graph.nodes;
    let custom = wires
        .iter()
        .any(|wire| !wire.flat() || wire.uses_caption_band());
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
                && !wire.uses_caption_band()
            {
                old
            } else {
                let side = named(hint).unwrap_or_else(|| match wire.ends[1 - end] {
                    End::Lane { side, inward, .. } => {
                        if inward {
                            side
                        } else {
                            routes::opposite(side)
                        }
                    }
                    End::Border { side, .. } => side,
                    End::Item { .. } => unreachable!("special leg has a boundary"),
                });
                let offset = match hint {
                    PortSideHint::FixedOffset { offset, .. } => {
                        let length = across(
                            side,
                            Size {
                                width: rect.width,
                                height: rect.height,
                            },
                        );
                        if !offset.is_finite() || offset.abs() > length / 2.0 {
                            return Err(LayoutError::InvalidInput("port offset outside face"));
                        }
                        Some(offset)
                    }
                    _ => anchor_offset(&nodes[slot], rect, side),
                };
                fixed = offset.is_some();
                let across = finite(cross(side, rect.center()) + offset.unwrap_or(0.0))?;
                let at = face(rect, side, across);
                finite(at.x)?;
                finite(at.y)?;
                Contact { at, side }
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
    let mut marker_ports = None;
    let mut obstructed = Vec::new();
    let pitch = options.spacing.port_pitch;
    for (slot, faces) in faces.iter_mut().enumerate() {
        let rect = rects[slot];
        for (slots, side) in faces.iter_mut().zip(SIDES) {
            work.visit(slots.len() + 1)?;
            if slots.is_empty() {
                continue;
            }
            let span = item_span(&nodes[slot], side, graph, options);
            let center = cross(side, rect.center());
            let bounds = (center - span / 2.0, center + span / 2.0);
            spread(slots, bounds.0, bounds.1, pitch, work)?;
            for index in 0..slots.len() {
                let entry = slots[index];
                let wire = &wires[entry.wire];
                let marker = [wire.spec.start_marker, wire.spec.end_marker][entry.end];
                let auto = matches!(
                    wire.ends[entry.end],
                    End::Item {
                        hint: PortSideHint::Auto,
                        child: None,
                        ..
                    }
                ) && (!wire.flat() || wire.uses_caption_band());
                if marker.length == 0.0 || (entry.fixed && !auto) {
                    continue;
                }
                let marker_ports = match &mut marker_ports {
                    Some(ports) => ports,
                    empty => empty.insert(MarkerPorts {
                        walls: routes::item_boxes(rects, wires, work)?,
                        blocked: Vec::new(),
                        clearance: options.spacing.edge,
                    }),
                };
                let port = Contact {
                    at: face(rect, side, entry.at),
                    side,
                };
                let bounds = if entry.fixed {
                    (entry.at, entry.at)
                } else {
                    bounds
                };
                if let Some(at) = marker_ports.at(
                    port,
                    marker,
                    bounds,
                    slots,
                    PortPitch::Preserve(pitch),
                    work,
                )? {
                    slots[index].at = at;
                } else if auto {
                    push(&mut obstructed, (slot, side, entry))?;
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
    if let Some(marker_ports) = marker_ports.as_mut() {
        for (slot, preferred, entry) in obstructed {
            work.visit(1)?;
            let wire = &mut wires[entry.wire];
            let marker = [wire.spec.start_marker, wire.spec.end_marker][entry.end];
            let rect = rects[slot];
            let node = &nodes[slot];
            let faces = &mut faces[slot];
            // Spread the original faces first. Adopt a perpendicular face before reversing
            // the shaft, and use the live port positions to keep neighbors apart.
            'select: for pitch in [pitch, 0.0] {
                for turn in [1, 3, 2] {
                    work.visit(1)?;
                    let side = SIDES[(preferred as usize + turn) % SIDES.len()];
                    let center = cross(side, rect.center());
                    let (at, bounds) = if let Some(offset) = anchor_offset(node, rect, side) {
                        let at = finite(center + offset)?;
                        (at, (at, at))
                    } else {
                        let half = item_span(node, side, graph, options) / 2.0;
                        (center, (center - half, center + half))
                    };
                    let port = Contact {
                        at: face(rect, side, at),
                        side,
                    };
                    if let Some(at) = marker_ports.at(
                        port,
                        marker,
                        bounds,
                        &faces[side as usize],
                        PortPitch::Reserve(pitch),
                        work,
                    )? {
                        let old = &mut faces[preferred as usize];
                        work.visit(old.len())?;
                        old.retain(|slot| slot.wire != entry.wire || slot.end != entry.end);
                        push(&mut faces[side as usize], Slot { at, ..entry })?;
                        wire.contacts[entry.end] = Some(Contact {
                            at: face(rect, side, at),
                            side,
                        });
                        break 'select;
                    }
                }
                if pitch == 0.0 {
                    break;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn item_span(node: &NodeSpec, side: Side, graph: &Graph, options: &Options) -> f64 {
    let length = across(side, node.size);
    let measured = if flow_sides(graph.direction).0.axis() == side.axis() {
        node.port_span
    } else {
        node.perpendicular_port_span.or(node.port_span)
    };
    measured
        .unwrap_or(length - 2.0 * options.spacing.corner_clearance)
        .clamp(0.0, length)
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
        let inset = fit.border_inset(side, options.spacing.corner_clearance);
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
    fit: &fit::Fit,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if title.width == 0.0 || title.height == 0.0 {
        return Ok(());
    }
    let (mut slots, mut fixed) = (Vec::new(), Vec::new());
    let mut obstructed = false;
    let margin = fit.title_margin();
    let mut clearance = margin;
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
        let pinned = matches!(hint, PortSideHint::FixedOffset { .. });
        let at = ports[slot].x;
        if !pinned {
            let half = wire.title_clearance(end, margin);
            clearance = clearance.max(half);
            obstructed |= at > title.x - half && at < title.x + title.width + half;
        }
        push(
            if pinned { &mut fixed } else { &mut slots },
            Slot {
                wire: slot,
                end,
                at,
                fixed: pinned,
            },
        )?;
    }
    if !obstructed {
        return Ok(());
    }
    let width = fit.size.width;
    let inset = fit.border_inset(side, options.spacing.corner_clearance);
    let left = (title.x - clearance).min(width - inset);
    let right = (title.x + title.width + clearance).max(inset);
    let pitch = options.spacing.min_port_pitch;
    for face in [&mut slots, &mut fixed] {
        work.charge_sort(face.len())?;
        face.sort_unstable_by(|a, b| a.at.total_cmp(&b.at).then(a.wire.cmp(&b.wire)));
        work.check()?;
    }
    let mut anchors = reserved(fixed.len())?;
    work.visit(fixed.len())?;
    anchors.extend(fixed.iter().map(|slot| slot.at));
    let capacity = |lo: f64, hi: f64, work: &mut Work<'_>| -> LayoutResult<usize> {
        if hi < lo {
            return Ok(0);
        }
        if pitch == 0.0 {
            return Ok(slots.len());
        }
        if anchors.is_empty() {
            return Ok((((hi - lo) / pitch).floor() as usize)
                .saturating_add(1)
                .min(slots.len()));
        }
        let mut at = lo;
        for n in 0..slots.len() {
            at = clear_across(at, &anchors, pitch, work)?;
            if at > hi {
                return Ok(n);
            }
            at += pitch;
        }
        Ok(slots.len())
    };
    let left_count = capacity(inset, left, work)?;
    let right_count = capacity(right, width - inset, work)?;
    let middle = title.center().x;
    let lower = slots.partition_point(|slot| slot.at < middle);
    let upper = slots.partition_point(|slot| slot.at <= middle);
    let split = (lower + (upper - lower).div_ceil(2))
        .max(slots.len().saturating_sub(right_count))
        .min(left_count);
    let (before, after) = slots.split_at(split);
    let count = slots
        .len()
        .checked_add(fixed.len())
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(count)?;
    let mut batch = reserved(count)?;
    for (movable, lo, hi) in [(before, inset, left), (after, right, width - inset)] {
        if lo > hi || movable.is_empty() {
            continue;
        }
        batch.clear();
        work.visit(movable.len() + fixed.len())?;
        batch.extend_from_slice(movable);
        batch.extend(
            fixed
                .iter()
                .copied()
                .filter(|slot| lo - pitch < slot.at && slot.at < hi + pitch),
        );
        spread(&mut batch, lo, hi, pitch, work)?;
        for slot in &batch[..movable.len()] {
            ports[slot.wire].x = slot.at;
        }
    }
    Ok(())
}
