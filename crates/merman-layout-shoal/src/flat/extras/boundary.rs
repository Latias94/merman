use super::*;

/// Route explicit shell endpoints in the final coordinate frame. A requested normal can
/// point into a shell for an edge connecting the shell to one of its own members.
pub(crate) struct BoundaryRoutes(Obstacles);

impl BoundaryRoutes {
    pub(crate) fn new() -> Self {
        Self(Obstacles::new())
    }

    pub(crate) fn obstacle(&mut self, rect: Rect, work: &mut Work<'_>) -> LayoutResult<()> {
        self.0.add(Owner::Node, Bounds::checked_rect(rect)?, work)
    }

    pub(crate) fn record(
        &mut self,
        id: u32,
        edge: &EdgeGeometry,
        spec: &EdgeSpec,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        for (center, size) in [
            (edge.label_center, spec.label),
            (edge.start_label_center, spec.start_label),
            (edge.end_label_center, spec.end_label),
        ] {
            if let (Some(center), Some(size)) = (center, size) {
                self.0
                    .add(Owner::End, Bounds::checked_centered(center, size)?, work)?;
            }
        }
        self.markers(id, spec, old_ports(edge)?.map(|p| (p.at, p.side)), work)?;
        self.0.add_stroke(id, edge, work)
    }

    fn markers(
        &mut self,
        id: u32,
        spec: &EdgeSpec,
        contacts: [(Point, Side); 2],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        for ((at, side), marker) in contacts
            .into_iter()
            .zip([spec.start_marker, spec.end_marker])
        {
            if marker.length > 0.0 && marker.width > 0.0 {
                self.0.add(
                    Owner::LaneMarker(id, side),
                    Port { at, side }.marker(marker)?,
                    work,
                )?;
            }
        }
        Ok(())
    }

    /// Protect only the required normal from later labels; the optional shaft gap may shrink.
    pub(crate) fn reserve_lead(
        &mut self,
        id: u32,
        contact: (Point, Side),
        marker: MarkerMetrics,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let (at, side) = contact;
        let lead = lead_metrics(marker);
        self.0.add(
            Owner::LaneMarker(id, side),
            Port { at, side }.marker(lead)?,
            work,
        )
    }

    pub(crate) fn blocked_contacts(
        &self,
        id: u32,
        contact: (Point, Side),
        marker: MarkerMetrics,
        blocked: &mut Vec<(f64, f64)>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        blocked.clear();
        let (at, side) = contact;
        let shaft = Port { at, side }.marker(lead_metrics(marker))?;
        let (start, end) = shaft.along(side);
        let half = lead_metrics(marker).width / 2.0;
        for wall in self.0.for_route(id, [Port { at, side }; 2], work)?.0 {
            work.visit(1)?;
            let (wall_start, wall_end) = wall.along(side);
            if wall_start >= end || wall_end <= start {
                continue;
            }
            let [low, high] = if side.axis() == Axis::Vertical {
                wall.x
            } else {
                wall.y
            };
            push(
                blocked,
                (
                    finite(low - half)?.next_down(),
                    finite(high + half)?.next_up(),
                ),
            )?;
        }
        Ok(())
    }

    pub(crate) fn marker_clear(
        &self,
        id: u32,
        contact: (Point, Side),
        marker: MarkerMetrics,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let (at, side) = contact;
        self.0
            .clear_lane_marker(id, side, Port { at, side }.marker(marker)?, work)
    }

    pub(crate) fn draw(
        &mut self,
        id: u32,
        spec: &EdgeSpec,
        contacts: [(Point, Side); 2],
        options: &Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<EdgeGeometry> {
        let pair = contacts.map(|(at, side)| Port { at, side });
        let gap = options.spacing.edge.max(1.0);
        let blockers = self.0.for_route(id, pair, work)?;
        for (port, marker) in pair.into_iter().zip([spec.start_marker, spec.end_marker]) {
            if marker.length > 0.0
                && marker.width > 0.0
                && !self.marker_clear(id, (port.at, port.side), marker, work)?
            {
                return Err(LayoutError::InvalidInput(
                    "swimlane marker overlaps an obstacle",
                ));
            }
        }
        let same_side_loop = spec.source == spec.target && pair[0].side == pair[1].side;
        let label_center = spec
            .label
            .filter(|_| !same_side_loop)
            .map(|size| choose_direct_label(id, pair[0].at, pair[1].at, size, gap, &self.0, work))
            .transpose()?;
        let mut edge = EdgeGeometry {
            points: Vec::new(),
            ports: (pair[0].side, pair[1].side),
            label_center,
            start_label_center: None,
            end_label_center: None,
            reversed_for_ranking: false,
        };
        let loop_via;
        let via = if same_side_loop {
            let depth = finite(
                options.spacing.port_pitch.max(gap)
                    + spec.start_marker.length.max(spec.end_marker.length)
                    + gap,
            )?;
            loop_via = loop_waypoints(pair, depth, &options.spacing, &blockers, work)?;
            &loop_via
        } else {
            label_center.as_slice()
        };
        let routed = redraw(&mut edge, pair, spec, via, gap, &blockers, work);
        if let (Err(corridor::NO_CORRIDOR), Some(size), Some(mid)) =
            (routed, spec.label, label_center)
            && spec.source != spec.target
        {
            let outside = self.0.exterior(mid, size, gap, work)?;
            redraw(&mut edge, pair, spec, &[outside], gap, &blockers, work)?;
            edge.label_center = Some(outside);
        } else {
            routed?;
        }
        if same_side_loop && let Some(size) = spec.label {
            let side = pair[0].side;
            let at = Point {
                x: pair[0].at.x.midpoint(pair[1].at.x),
                y: pair[0].at.y.midpoint(pair[1].at.y),
            };
            let mut reach = 0.0_f64;
            for knot in &edge.points {
                work.visit(1)?;
                let distance = match side {
                    Side::Top => at.y - knot.y,
                    Side::Right => knot.x - at.x,
                    Side::Bottom => knot.y - at.y,
                    Side::Left => at.x - knot.x,
                };
                reach = reach.max(distance);
            }
            let depth = match side.axis() {
                Axis::Horizontal => size.width,
                Axis::Vertical => size.height,
            };
            let preferred = advance(
                at,
                side,
                finite(reach + options.spacing.loop_gap + depth / 2.0)?,
            )?;
            edge.label_center = Some(loop_label(id, preferred, size, side, &self.0, work)?);
        }
        if let (Some(center), Some(size)) = (edge.label_center, spec.label) {
            self.0
                .add(Owner::End, Bounds::checked_centered(center, size)?, work)?;
        }
        self.0.add_stroke(id, &edge, work)?;
        if let Some(size) = spec.start_label {
            let center = end_label(
                &edge,
                false,
                size,
                spec.start_marker,
                options,
                &self.0,
                work,
            )?;
            self.0
                .add(Owner::End, Bounds::checked_centered(center, size)?, work)?;
            edge.start_label_center = Some(center);
        }
        if let Some(size) = spec.end_label {
            let center = end_label(&edge, true, size, spec.end_marker, options, &self.0, work)?;
            self.0
                .add(Owner::End, Bounds::checked_centered(center, size)?, work)?;
            edge.end_label_center = Some(center);
        }
        Ok(edge)
    }
}

fn lead_metrics(marker: MarkerMetrics) -> MarkerMetrics {
    MarkerMetrics {
        length: marker.length.max(1.0),
        width: if marker.length == 0.0 {
            0.0
        } else {
            marker.width
        },
    }
}
