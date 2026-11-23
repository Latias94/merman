use super::kernel::{Drawing, Extent, advance, finite};
use crate::work::{LayoutResult as Result, Work, push, reserved};
use crate::{Axis, EdgeGeometry, LayoutError, Point, PortSideHint, RoutePoint, RoutingStyle};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ops::Range;

#[derive(Clone, Copy, Default)]
pub(crate) struct Track {
    x: f64,
    source_port: f64,
    target_port: f64,
}

/// A lane can serve another long edge as soon as its previous edge enters its target row.
/// One heap entry per lane, independent of the number of ranks the edges span.
#[derive(Default)]
struct Lanes {
    ends: BinaryHeap<Reverse<(usize, usize)>>,
    half_widths: Vec<f64>,
}

impl Lanes {
    fn available(&self, row: usize) -> bool {
        self.ends
            .peek()
            .is_some_and(|&Reverse((end, _))| end <= row)
    }

    fn assign(&mut self, from: usize, to: usize, half: f64, work: &mut Work) -> Result<usize> {
        let levels = self.ends.len().checked_ilog2().unwrap_or(0) as usize + 1;
        work.visit(2 * levels)?;
        if let Some(mut earliest) = self.ends.peek_mut() {
            let Reverse((end, id)) = *earliest;
            if end <= from {
                *earliest = Reverse((to, id));
                self.half_widths[id] = self.half_widths[id].max(half);
                return Ok(id);
            }
        }
        let id = self.half_widths.len();
        push(&mut self.half_widths, half)?;
        self.ends
            .try_reserve(1)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.ends.push(Reverse((to, id)));
        Ok(id)
    }

    fn positions(
        self,
        mut outside: f64,
        sign: f64,
        gap: f64,
        extent: &mut Extent,
        work: &mut Work,
    ) -> Result<Vec<f64>> {
        let mut positions = self.half_widths;
        for value in &mut positions {
            work.visit(1)?;
            let half = *value;
            let center = advance(advance(outside, gap)?, half)?;
            outside = advance(center, half)?;
            *value = sign * center;
            extent.include(*value, half)?;
        }
        Ok(positions)
    }
}

fn label_gap(from: usize, to: usize) -> usize {
    from + (to - from - 1) / 2
}

impl Drawing<'_> {
    fn preferred_track(&self, id: usize) -> f64 {
        let edge = &self.graph.edges[id];
        self.centers[edge.source]
            .x
            .midpoint(self.centers[edge.target].x)
    }

    fn label_half_width(&self, id: usize) -> f64 {
        self.graph.edges[id]
            .spec
            .label
            .map_or(0.0, |size| self.graph.direction.axes(size).width * 0.5)
    }

    pub(crate) fn plan_tracks(
        &mut self,
        rows: Range<usize>,
        extent: &mut Extent,
        work: &mut Work,
    ) -> Result<()> {
        let mut adjacent = Vec::new();
        let mut long = Vec::new();
        let mut lanes = [Lanes::default(), Lanes::default()];
        for from in rows {
            adjacent.clear();
            for &node in &self.layers.order[self.layers.rows[from].nodes.clone()] {
                work.visit(1)?;
                for &id in self.layers.outgoing.at(node) {
                    work.visit(1)?;
                    let edge = &self.graph.edges[id];
                    let to = self.layers.row_of[edge.target];
                    self.space[from].start_clearance = self.space[from]
                        .start_clearance
                        .max(edge.spec.start_marker.length);
                    self.space[to].end_clearance = self.space[to]
                        .end_clearance
                        .max(edge.spec.end_marker.length);
                    let gap = label_gap(from, to);
                    if let Some(size) = edge.spec.label {
                        self.space[gap].label_height = self.space[gap]
                            .label_height
                            .max(self.graph.direction.axes(size).height);
                    }
                    let preferred = self.preferred_track(id);
                    self.tracks[id].x = preferred;
                    if to == from + 1 {
                        push(&mut adjacent, id)?;
                    } else {
                        // Prefer the nearer side. For centered ends prefer a reusable lane,
                        // then the side with fewer lanes. Ties go to the left.
                        let side = if preferred == 0.0 {
                            usize::from(
                                (!lanes[0].available(from), lanes[0].half_widths.len())
                                    > (!lanes[1].available(from), lanes[1].half_widths.len()),
                            )
                        } else {
                            usize::from(preferred > 0.0)
                        };
                        let lane = lanes[side].assign(from, to, self.label_half_width(id), work)?;
                        push(&mut long, (id, side, lane))?;
                    }
                }
            }
            self.separate_adjacent(&mut adjacent, extent, work)?;
        }
        let [left, right] = lanes;
        let gap = self.graph.options.spacing.edge;
        let positions = [
            left.positions(-extent.min, -1.0, gap, extent, work)?,
            right.positions(extent.max, 1.0, gap, extent, work)?,
        ];
        for (id, side, lane) in long {
            work.visit(1)?;
            self.tracks[id].x = positions[side][lane];
        }
        Ok(())
    }

    /// In the label band, each adjacent edge owns an interval as wide as its label.
    /// Both greedy placements satisfy every separation constraint. Their midpoint keeps a
    /// group of equally placed parallel edges centered on its preferred track.
    fn separate_adjacent(
        &mut self,
        ids: &mut [usize],
        extent: &mut Extent,
        work: &mut Work,
    ) -> Result<()> {
        work.sort(ids, |&a, &b| {
            self.tracks[a]
                .x
                .total_cmp(&self.tracks[b].x)
                .then(a.cmp(&b))
        })?;
        let mut boundary = None;
        for &id in ids.iter() {
            work.visit(1)?;
            let half = self.label_half_width(id);
            let mut x = self.tracks[id].x;
            if let Some(right) = boundary {
                x = x.max(advance(
                    advance(right, self.graph.options.spacing.edge)?,
                    half,
                )?);
            }
            self.tracks[id].x = x;
            boundary = Some(advance(x, half)?);
        }
        boundary = None;
        for &id in ids.iter().rev() {
            work.visit(1)?;
            let half = self.label_half_width(id);
            let mut x = self.preferred_track(id);
            if let Some(left) = boundary {
                x = x.min(-advance(
                    advance(-left, self.graph.options.spacing.edge)?,
                    half,
                )?);
            }
            boundary = Some(-advance(-x, half)?);
            self.tracks[id].x = self.tracks[id].x.midpoint(x);
            extent.include(self.tracks[id].x, half)?;
        }
        Ok(())
    }

    pub(crate) fn assign_ports(&mut self, rows: Range<usize>, work: &mut Work) -> Result<()> {
        for row in &self.layers.rows[rows] {
            for &node in &self.layers.order[row.nodes.clone()] {
                work.visit(1)?;
                let spec = &self.graph.nodes[node];
                let width = self.graph.direction.axes(spec.size).width;
                let span = spec
                    .port_span
                    .unwrap_or_else(|| {
                        (width - 2.0 * self.graph.options.spacing.corner_clearance).max(0.0)
                    })
                    .min(width);
                let anchor = match self.graph.direction.ports().0.axis() {
                    Axis::Horizontal => spec.anchor_y.map(|y| y - width * 0.5),
                    Axis::Vertical => None,
                };
                let center = self.centers[node].x;
                for (outgoing, ends) in [
                    (true, self.layers.outgoing.at_mut(node)),
                    (false, self.layers.incoming.at_mut(node)),
                ] {
                    work.sort(ends, |&a, &b| {
                        self.tracks[a]
                            .x
                            .total_cmp(&self.tracks[b].x)
                            .then(a.cmp(&b))
                    })?;
                    for (i, &edge) in ends.iter().enumerate() {
                        work.visit(1)?;
                        let fraction = (i as f64 + 0.5) / ends.len() as f64 - 0.5;
                        let spec = self.graph.edges[edge].spec;
                        let hint = if outgoing {
                            spec.source_port
                        } else {
                            spec.target_port
                        };
                        let offset = match hint {
                            PortSideHint::FixedOffset { offset, .. } => offset,
                            _ => anchor.unwrap_or(span * fraction),
                        };
                        let port = finite(center + offset)?;
                        if outgoing {
                            self.tracks[edge].source_port = port;
                        } else {
                            self.tracks[edge].target_port = port;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn route(&self, id: usize) -> Result<EdgeGeometry> {
        let edge = &self.graph.edges[id];
        let from = self.layers.row_of[edge.source];
        let to = self.layers.row_of[edge.target];
        let track = &self.tracks[id];
        let source_depth = self
            .graph
            .direction
            .axes(self.graph.nodes[edge.source].size)
            .height;
        let target_depth = self
            .graph
            .direction
            .axes(self.graph.nodes[edge.target].size)
            .height;
        let start = self.space[from].departure;
        let end = self.space[to].arrival;
        let orthogonal = self.graph.options.routing == RoutingStyle::Orthogonal;
        let mut points = reserved(if orthogonal { 8 } else { 6 })?;
        // A diagonal preview leg has flow-axis tangents. An orthogonal leg gets a corner
        // on the same side of the label band, keeping the band and terminal shafts clear.
        let mut point = |x, y, axis| points.push(RoutePoint { x, y, axis });
        point(
            track.source_port,
            finite(self.centers[edge.source].y + source_depth * 0.5)?,
            Axis::Vertical,
        );
        point(track.source_port, start, Axis::Vertical);
        if track.source_port != track.x {
            if orthogonal {
                point(track.x, start, Axis::Horizontal);
            }
            point(track.x, self.space[from].band_top, Axis::Vertical);
        }
        if track.target_port != track.x {
            // Both elbows can land on the same track at a zero-height label band.
            if track.source_port == track.x
                || self.space[from].band_top != self.space[to - 1].band_bottom
            {
                point(track.x, self.space[to - 1].band_bottom, Axis::Vertical);
            }
            if orthogonal {
                point(track.x, end, Axis::Vertical);
            }
        }
        point(
            track.target_port,
            end,
            if orthogonal && track.target_port != track.x {
                Axis::Horizontal
            } else {
                Axis::Vertical
            },
        );
        point(
            track.target_port,
            finite(self.centers[edge.target].y - target_depth * 0.5)?,
            Axis::Vertical,
        );
        let label_center = edge.spec.label.map(|_| {
            let gap = &self.space[label_gap(from, to)];
            Point {
                x: track.x,
                y: gap.band_top.midpoint(gap.band_bottom),
            }
        });
        Ok(EdgeGeometry {
            points,
            ports: self.graph.direction.ports(),
            label_center,
            start_label_center: None,
            end_label_center: None,
            reversed_for_ranking: false,
        })
    }
}
