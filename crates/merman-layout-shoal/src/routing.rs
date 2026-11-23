use crate::kernel::{Drawing, Extent, GUTTER, advance, finite};
use crate::work::{Result, Work, push, reserved};
use crate::{LayoutError, Point, Route};
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

    fn assign<E>(
        &mut self,
        from: usize,
        to: usize,
        half: f64,
        work: &mut Work<E>,
    ) -> Result<usize, E> {
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
            .map_err(|_| LayoutError::Overflow)?;
        self.ends.push(Reverse((to, id)));
        Ok(id)
    }

    fn positions<E>(
        self,
        mut outside: f64,
        sign: f64,
        extent: &mut Extent,
        work: &mut Work<E>,
    ) -> Result<Vec<f64>, E> {
        let mut positions = self.half_widths;
        for value in &mut positions {
            work.visit(1)?;
            let half = *value;
            let center = advance(advance(outside, GUTTER)?, half)?;
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
        self.centers[edge.source.0]
            .x
            .midpoint(self.centers[edge.target.0].x)
    }

    fn label_half_width(&self, id: usize) -> f64 {
        self.graph.edges[id]
            .label
            .map_or(0.0, |size| self.graph.direction.axes(size).width * 0.5)
    }

    pub(crate) fn plan_tracks<E>(
        &mut self,
        rows: Range<usize>,
        extent: &mut Extent,
        work: &mut Work<E>,
    ) -> Result<(), E> {
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
                    let to = self.layers.row_of[edge.target.0];
                    self.space[from].start_clearance =
                        self.space[from].start_clearance.max(edge.start_clearance);
                    self.space[to].end_clearance =
                        self.space[to].end_clearance.max(edge.end_clearance);
                    let gap = label_gap(from, to);
                    if let Some(size) = edge.label {
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
        let positions = [
            left.positions(-extent.min, -1.0, extent, work)?,
            right.positions(extent.max, 1.0, extent, work)?,
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
    fn separate_adjacent<E>(
        &mut self,
        ids: &mut [usize],
        extent: &mut Extent,
        work: &mut Work<E>,
    ) -> Result<(), E> {
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
                x = x.max(advance(advance(right, GUTTER)?, half)?);
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
                x = x.min(-advance(advance(-left, GUTTER)?, half)?);
            }
            boundary = Some(-advance(-x, half)?);
            self.tracks[id].x = self.tracks[id].x.midpoint(x);
            extent.include(self.tracks[id].x, half)?;
        }
        Ok(())
    }

    pub(crate) fn assign_ports<E>(
        &mut self,
        rows: Range<usize>,
        work: &mut Work<E>,
    ) -> Result<(), E> {
        for row in &self.layers.rows[rows] {
            for &node in &self.layers.order[row.nodes.clone()] {
                work.visit(1)?;
                let width = self.graph.direction.axes(self.graph.nodes[node]).width;
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
                        let port = finite(center + width * fraction)?;
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

    pub(crate) fn route<E>(&self, id: usize) -> Result<Route, E> {
        let edge = &self.graph.edges[id];
        let from = self.layers.row_of[edge.source.0];
        let to = self.layers.row_of[edge.target.0];
        let track = &self.tracks[id];
        let start = Point {
            x: track.source_port,
            y: self.space[from].departure,
        };
        let end = Point {
            x: track.target_port,
            y: self.space[to].arrival,
        };
        let bend_in = start.x != track.x;
        let bend_out = end.x != track.x;
        let mut points = reserved(4 + usize::from(bend_in) + usize::from(bend_out))?;
        points.push(self.centers[edge.source.0]);
        points.push(start);
        if bend_in {
            points.push(Point {
                x: track.x,
                y: self.space[from].band_top,
            });
        }
        if bend_out {
            let bend = Point {
                x: track.x,
                y: self.space[to - 1].band_bottom,
            };
            if points.last() != Some(&bend) {
                points.push(bend);
            }
        }
        points.push(end);
        points.push(self.centers[edge.target.0]);
        let label_center = edge.label.map(|_| {
            let gap = &self.space[label_gap(from, to)];
            Point {
                x: track.x,
                y: gap.band_top.midpoint(gap.band_bottom),
            }
        });
        Ok(Route {
            points,
            label_center,
        })
    }
}
