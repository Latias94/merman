//! Prefer the short forward path when long edges have exterior lanes.

use super::{End, Neighbours, Ports};
use crate::flat::{Component, Expanded, Kind, acyclic::Orientation, routing::side_docking_allowed};
use crate::scratch::{capacity_bytes, clear_reserve};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{Graph, Spacing};

#[derive(Debug, Default)]
pub(in crate::flat) struct SpinePorts {
    // One possible anchor per node face, in top / bottom order.
    anchors: Vec<[Option<u32>; 2]>,
    pending: Vec<PendingFace>,
}

#[derive(Debug)]
struct PendingFace {
    side: usize,
    anchor: u32,
    span: f64,
    ports: Vec<FacePort>,
}

#[derive(Clone, Copy, Debug)]
struct FacePort {
    segment: u32,
    natural_offset: f64,
    marker_width: f64,
    lane: Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct Segment {
    short_forward: bool,
    marker_width: [f64; 2],
    lane: Option<usize>,
}

#[derive(Clone, Copy)]
struct Centering {
    slot: usize,
    pitch: f64,
}

impl Centering {
    fn fit(
        ports: impl Iterator<Item = FacePort>,
        anchor: u32,
        span: f64,
        spacing: &Spacing,
    ) -> Option<Self> {
        let mut slot = None;
        let mut count = 0;
        let mut previous_width = None;
        let mut required = spacing.min_port_pitch;
        for port in ports {
            if port.segment == anchor {
                slot = Some(count);
            }
            if let Some(width) = previous_width {
                required = required.max(width * 0.5 + port.marker_width * 0.5);
            }
            previous_width = Some(port.marker_width);
            count += 1;
        }
        let slot = slot?;
        let steps = slot.max(count - 1 - slot);
        if steps == 0 {
            return Some(Self { slot, pitch: 0.0 });
        }
        let pitch = spacing
            .port_pitch
            .max(required)
            .min(span * 0.5 / steps as f64);
        (pitch > 0.0 && pitch >= required).then_some(Self { slot, pitch })
    }

    fn apply(self, segments: impl Iterator<Item = u32>, offsets: &mut [f64]) {
        for (i, edge) in segments.enumerate() {
            offsets[edge as usize] = (i as f64 - self.slot as f64) * self.pitch;
        }
    }
}

impl SpinePorts {
    pub(super) fn clear(&mut self) {
        self.anchors.clear();
        self.pending.clear();
    }

    pub(in crate::flat) fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.anchors)
            + capacity_bytes(&self.pending)
            + self
                .pending
                .iter()
                .map(|face| capacity_bytes(&face.ports))
                .sum::<usize>()
    }

    pub(in crate::flat) fn centered(&self, node: usize, side: usize, segment: usize) -> bool {
        self.anchors
            .get(node)
            .is_some_and(|sides| sides[side] == Some(segment as u32))
    }

    /// Decide narrow faces against actual docked terminals. The rank and box placement is
    /// already fixed. If the remaining rank-face terminals still need the full width, restore
    /// their original spread in the same order before collecting the gap pieces.
    pub(in crate::flat) fn resolve(
        &mut self,
        mut offsets: [&mut [f64]; 2],
        spacing: &Spacing,
        docked: impl Fn(usize, usize) -> bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        for face in self.pending.drain(..) {
            work.visit(2 * face.ports.len())?;
            // In ranked order the bottom incidence is at the start of its chain.
            let end = 1 - face.side;
            let on_face = || {
                face.ports
                    .iter()
                    .copied()
                    .filter(|port| port.lane.is_none_or(|edge| !docked(edge, end)))
            };
            let offsets = &mut offsets[face.side];
            if let Some(centered) = Centering::fit(on_face(), face.anchor, face.span, spacing) {
                centered.apply(on_face().map(|port| port.segment), offsets);
            } else {
                for port in on_face() {
                    offsets[port.segment as usize] = port.natural_offset;
                }
            }
        }
        Ok(())
    }
}

struct Face<'a> {
    vertex: usize,
    side: usize,
    span: f64,
    ids: &'a [u32],
    roles: &'a [End],
    offsets: &'a mut [f64],
}

impl Face<'_> {
    fn center(
        self,
        spine: &mut SpinePorts,
        segments: &[Segment],
        spacing: &Spacing,
        allow_side: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let Self {
            vertex,
            side,
            span,
            ids,
            roles,
            offsets,
        } = self;
        if ids.len() < 2 || span <= 0.0 || ids.iter().any(|&e| roles[e as usize] == End::Held) {
            return Ok(());
        }
        let mut forwards = ids
            .iter()
            .enumerate()
            .filter(|&(_, &e)| segments[e as usize].short_forward);
        let Some((slot, &anchor)) = forwards.next() else {
            return Ok(());
        };
        if forwards.next().is_some() {
            return Ok(());
        }
        work.visit(3 * ids.len())?;
        let ports = || {
            ids.iter().map(|&segment| {
                let info = segments[segment as usize];
                FacePort {
                    segment,
                    natural_offset: offsets[segment as usize],
                    marker_width: info.marker_width[side],
                    lane: info.lane,
                }
            })
        };
        let centered = if let Some(centered) = Centering::fit(ports(), anchor, span, spacing) {
            centered
        } else if allow_side
            && Centering::fit(
                ports().filter(|port| port.lane.is_none()),
                anchor,
                span,
                spacing,
            )
            .is_some()
        {
            work.charge(ids.len() + 1)?;
            let mut members = reserved(ids.len())?;
            members.extend(ports());
            push(
                &mut spine.pending,
                PendingFace {
                    side,
                    anchor,
                    span,
                    ports: members,
                },
            )?;
            // Keep provisional contacts ordered while positioning the boxes and testing
            // side access. They will be refitted to the available face before routing.
            Centering {
                slot,
                pitch: span * 0.5 / slot.max(ids.len() - 1 - slot) as f64,
            }
        } else {
            return Ok(());
        };
        centered.apply(ids.iter().copied(), offsets);
        spine.anchors[vertex][side] = Some(anchor);
        Ok(())
    }
}

impl Neighbours {
    pub(super) fn prefer_spine(
        &mut self,
        g: &Expanded,
        (component, orientation): (&Component, &Orientation),
        graph: &Graph,
        spacing: &Spacing,
        ports: &mut Ports,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        if g.exterior_lanes.is_empty() {
            return Ok(());
        }
        work.charge(g.layers.len() + g.from.len() * 2 + g.rank.len())?;
        let mut rank_count = reserved(g.layers.len())?;
        let mut node_ranks = 0;
        for row in &g.layers {
            work.visit(row.len())?;
            node_ranks += usize::from(row.iter().any(|&v| g.kind[v as usize] == Kind::Node));
            rank_count.push(node_ranks);
        }
        let mut keep = filled(g.from.len(), true)?;
        let mut segments = filled(g.from.len(), Segment::default())?;
        for lane in &g.exterior_lanes {
            work.visit(1)?;
            let bends = &g.chains[lane.edge];
            for end in [g.pred.at(bends[1])[0], g.succ.at(bends[bends.len() - 2])[0]] {
                keep[end as usize] = false;
                segments[end as usize].lane = Some(lane.edge);
            }
        }
        let mut first = 0;
        for (edge, chain) in g.chains.iter().enumerate() {
            work.visit(1)?;
            if g.direct[edge] {
                continue;
            }
            let last = first + chain.len() - 2;
            let (from, to) = (chain[0], *chain.last().unwrap());
            let short = !orientation.reversed[edge]
                && rank_count[g.rank[to as usize] as usize]
                    == rank_count[g.rank[from as usize] as usize] + 1;
            segments[first].short_forward = short;
            segments[last].short_forward = short;
            let spec = &graph.edges[component.links[edge].edge as usize];
            let mut widths = [spec.end_marker.width, spec.start_marker.width];
            if orientation.reversed[edge] {
                widths.swap(0, 1);
            }
            segments[first].marker_width[1] = widths[1];
            segments[last].marker_width[0] = widths[0];
            first = last + 1;
        }
        let can_center = !g.direct.contains(&true);
        let allow_side = can_center && side_docking_allowed(g, component, graph, work)?;
        if can_center {
            work.charge(component.nodes.len())?;
            clear_reserve(&mut ports.spine.anchors, component.nodes.len())?;
            ports.spine.anchors.resize(component.nodes.len(), [None; 2]);
        }
        let Ports {
            source,
            target,
            starts,
            ends,
            start_markers: _,
            end_markers: _,
            spine,
        } = ports;
        for (vertex, node) in component.nodes.iter().enumerate() {
            work.visit(1)?;
            if g.kind[vertex] != Kind::Node {
                continue;
            }
            let width = g.width[vertex];
            let span = node
                .port_span
                .unwrap_or((width - 2.0 * spacing.corner_clearance).max(0.0))
                .min(width);
            for (side, (ids, roles, offsets)) in [
                (self.pred.at(vertex as u32), &*ends, &mut *target),
                (self.succ.at(vertex as u32), &*starts, &mut *source),
            ]
            .into_iter()
            .enumerate()
            {
                work.visit(ids.len() * 2)?;
                if can_center {
                    Face {
                        vertex,
                        side,
                        span,
                        ids,
                        roles,
                        offsets,
                    }
                    .center(spine, &segments, spacing, allow_side, work)?;
                }
                if ids.iter().any(|&e| segments[e as usize].short_forward) {
                    for &e in ids {
                        keep[e as usize] &= segments[e as usize].short_forward;
                    }
                }
            }
        }
        for list in [&mut self.pred, &mut self.succ] {
            let mut write = 0;
            for v in 0..g.rank.len() {
                work.visit(1)?;
                let (start, end) = (list.start[v] as usize, list.start[v + 1] as usize);
                list.start[v] = write as u32;
                work.visit(end - start)?;
                for read in start..end {
                    let edge = list.entries[read];
                    if keep[edge as usize] {
                        list.entries[write] = edge;
                        write += 1;
                    }
                }
            }
            *list.start.last_mut().unwrap() = write as u32;
            list.entries.truncate(write);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    #[test]
    fn narrow_face_centers_only_with_a_confirmed_side_terminal() {
        let spacing = Spacing::default();
        let segments = [
            Segment {
                short_forward: true,
                marker_width: [14.0; 2],
                lane: None,
            },
            Segment {
                marker_width: [14.0; 2],
                ..Segment::default()
            },
            Segment {
                lane: Some(3),
                ..Segment::default()
            },
        ];
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        for docked in [false, true] {
            let mut spine = SpinePorts {
                anchors: vec![[None; 2]],
                ..Default::default()
            };
            let mut offsets = [-14.0, 0.0, 14.0];
            Face {
                vertex: 0,
                side: 0,
                span: 28.0,
                ids: &[0, 1, 2],
                roles: &[End::Arrives; 3],
                offsets: &mut offsets,
            }
            .center(&mut spine, &segments, &spacing, true, &mut work)
            .unwrap();
            assert!(spine.centered(0, 0, 0));
            spine
                .resolve(
                    [&mut offsets, &mut []],
                    &spacing,
                    |edge, end| docked && edge == 3 && end == 1,
                    &mut work,
                )
                .unwrap();
            if docked {
                assert_eq!(offsets[..2], [0.0, 14.0]);
            } else {
                assert_eq!(offsets, [-14.0, 0.0, 14.0]);
            }
        }
    }
}
