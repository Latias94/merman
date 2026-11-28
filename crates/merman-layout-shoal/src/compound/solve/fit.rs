use super::*;
use crate::work::push;
use crate::{GroupSpec, Options, TitlePosition, WidthPolicy};

pub(super) struct Fit {
    pub size: Size,
    pub offset: Point,
    pub seats: Vec<[Option<Rect>; 3]>,
    pub corridor: [f64; 4],
    pub adaptive_top_title: bool,
    pub protected_title_margin: f64,
    marker_insets: [f64; 4],
    flush_title: bool,
    title_drop: f64,
    vertical_slack: f64,
}

pub(super) struct TitlePlacement {
    pub rect: Rect,
    pub reserve_at: Option<TitlePosition>,
}

pub(super) fn title_side(position: TitlePosition) -> Side {
    match position {
        TitlePosition::TopAuto
        | TitlePosition::TopLeft
        | TitlePosition::TopCenter
        | TitlePosition::TopRight => Side::Top,
        TitlePosition::BottomLeft | TitlePosition::BottomCenter | TitlePosition::BottomRight => {
            Side::Bottom
        }
    }
}

pub(super) fn blocks(rect: Rect, from: Point, to: Point) -> bool {
    rect.width > 0.0
        && rect.height > 0.0
        && from.x.min(to.x) < rect.x + rect.width
        && from.x.max(to.x) > rect.x
        && from.y.min(to.y) < rect.y + rect.height
        && from.y.max(to.y) > rect.y
}

impl Fit {
    pub(super) fn new(
        group: Option<&GroupSpec>,
        content: Size,
        nodes: &[Rect],
        wires: &[Wire],
        protected_title_margin: f64,
        options: &Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let group = group.copied().unwrap_or_default();
        let padding = group.padding;
        let mut inset = [padding.top, padding.right, padding.bottom, padding.left];
        let title = group.title.unwrap_or_default();
        let title_side = title_side(group.title_position) as usize;
        let flush_title = options.group_policy.flush_top_left_titles
            && group.title_position == TitlePosition::TopLeft;
        let adaptive_top_title = group.title_position == TitlePosition::TopAuto
            && options.group_policy.adaptive_title_clearance;
        inset[title_side] = inset[title_side].max(title.height);
        let slack = inset[title_side] - title.height;
        let mut before_title = if flush_title { 0.0 } else { slack / 2.0 };
        let mut after_title = slack - before_title;
        let body = Rect {
            width: content.width,
            height: content.height,
            ..Default::default()
        };
        let spacing = options.spacing;
        let label_gap = spacing.end_label_across.max(spacing.port_pitch / 2.0);
        let mut walls = Vec::new();
        if wires.iter().any(|wire| !wire.flat()) {
            work.charge(nodes.len())?;
            walls = reserved(nodes.len())?;
            walls.extend_from_slice(nodes);
            for wire in wires {
                work.visit(1)?;
                if let Some(edge) = &wire.route {
                    for (center, size) in [
                        (edge.label_center, wire.spec.label),
                        (edge.start_label_center, wire.spec.start_label),
                        (edge.end_label_center, wire.spec.end_label),
                    ] {
                        if let (Some(center), Some(size)) = (center, size) {
                            push(&mut walls, centered(center, size))?;
                        }
                    }
                }
            }
        }
        let mut corridor = [0.0_f64; 4];
        let mut rightmost = [0.0_f64; 4];
        let mut counts = [0usize; 4];
        let mut movable_at_title = 0usize;
        let mut title_trunk = 0.0;
        let title_margin = TITLE_CLEAR.max(protected_title_margin);
        let has_title = title.width > 0.0 && title.height > 0.0;
        let mut title_clear = title_margin;
        let fit_width = matches!(options.width_policy, WidthPolicy::FitWidth { .. });
        let mut marker_insets = [0.0_f64; 4];
        let mut fixed_offsets = [0.0_f64; 4];
        let mut fixed_reach = [0.0_f64; 4];
        for wire in wires {
            work.visit(1)?;
            let Some((end, _, side)) = wire.border() else {
                continue;
            };
            let index = side as usize;
            counts[index] += 1;
            let inner = wire.contacts[1 - end].expect("inner contact");
            let End::Border { flat, hint, .. } = wire.ends[end] else {
                unreachable!()
            };
            let manual_inner = flat.is_none() || wire.uses_caption_band();
            let preferred = flat.map_or(inner.at, |node| nodes[node.0 as usize].center());
            rightmost[index] = rightmost[index]
                .max(cross(side, inner.at))
                .max(cross(side, preferred));
            let markers = [wire.spec.start_marker, wire.spec.end_marker];
            if fit_width {
                let marker = markers[end];
                let offset = if let PortSideHint::FixedOffset { offset, .. } = hint {
                    fixed_offsets[index] = fixed_offsets[index].max(offset.abs());
                    Some(offset.abs())
                } else {
                    None
                };
                if marker.length > 0.0 && marker.width > 0.0 {
                    work.visit(1)?;
                    let half = marker.width / 2.0;
                    if let Some(offset) = offset {
                        fixed_reach[index] = fixed_reach[index].max(offset + half);
                    } else {
                        marker_insets[index] = marker_insets[index].max(half);
                    }
                }
            }
            if index == title_side && has_title {
                let movable = !matches!(hint, PortSideHint::FixedOffset { .. });
                let protect = movable && protected_title_margin > 0.0;
                if movable && (!adaptive_top_title || protect) {
                    movable_at_title += 1;
                    title_trunk = cross(side, preferred);
                    title_clear = title_clear.max(wire.title_clearance(end, title_margin));
                }
                let gap = spacing.edge.max(title_margin);
                if !flush_title && (!adaptive_top_title || protect || markers[end].length > 0.0) {
                    before_title = before_title.max(markers[end].length + gap);
                }
                let shaft = if manual_inner && inner.side == side {
                    markers[1 - end].length
                } else {
                    0.0
                };
                if !adaptive_top_title || protect || shaft > 0.0 {
                    after_title = after_title.max(shaft + gap);
                }
            }
            inset[index] = inset[index].max(markers[end].length + spacing.edge);
            if !matches!(hint, PortSideHint::FixedOffset { .. }) {
                // Keep outer corridors inside the perpendicular corner clearance so border
                // stitching does not force a 4 px Z-bend when group padding < corner_clearance.
                let at = cross(side, preferred);
                let (near, far) = if side.axis() == Axis::Vertical {
                    (Side::Left, Side::Right)
                } else {
                    (Side::Top, Side::Bottom)
                };
                let corner = spacing.corner_clearance;
                inset[near as usize] = inset[near as usize].max(corner - at);
                let opposite = across(side, content) - at;
                inset[far as usize] = inset[far as usize].max(corner - opposite);
            }
            if manual_inner && markers[1 - end].width > 0.0 {
                let radius = markers[1 - end].width / 2.0 + spacing.edge;
                let at = cross(inner.side, inner.at);
                let (near, far) = if inner.side.axis() == Axis::Vertical {
                    (Side::Left, Side::Right)
                } else {
                    (Side::Top, Side::Bottom)
                };
                inset[near as usize] = inset[near as usize].max(radius - at);
                let opposite = across(inner.side, content) - at;
                inset[far as usize] = inset[far as usize].max(radius - opposite);
            }
            if manual_inner {
                let inner_side = inner.side as usize;
                let to_body_edge = match inner.side {
                    Side::Top => inner.at.y,
                    Side::Bottom => content.height - inner.at.y,
                    Side::Left => inner.at.x,
                    Side::Right => content.width - inner.at.x,
                };
                inset[inner_side] =
                    inset[inner_side].max(markers[1 - end].length + spacing.edge - to_body_edge);
                if inner_side == title_side && title.height > 0.0 && title.width > 0.0 {
                    after_title =
                        after_title.max(markers[1 - end].length + spacing.edge - to_body_edge);
                }
            }
            if flat.is_none() {
                inset[index] =
                    inset[index].max(markers[0].length + markers[1].length + 2.0 * spacing.edge);
                let projected = face(body, side, cross(side, inner.at));
                let mut blocked = inner.side != side;
                for &wall in &walls {
                    work.visit(1)?;
                    if blocks(wall, inner.at, projected) {
                        blocked = true;
                        break;
                    }
                }
                if blocked {
                    let lane = spacing.edge.max(spacing.min_port_pitch / 2.0);
                    let lane = if has_title {
                        lane.max(title_margin)
                    } else {
                        lane
                    };
                    corridor[index] = lane;
                    inset[index] = inset[index].max(2.0 * lane + markers[end].length);
                    let adjacent = if side.axis() == Axis::Vertical {
                        [Side::Left, Side::Right]
                    } else {
                        [Side::Top, Side::Bottom]
                    };
                    for side in adjacent {
                        inset[side as usize] = inset[side as usize].max(2.0 * lane);
                    }
                }
            }
        }
        inset[title_side] =
            inset[title_side].max(finite(before_title + title.height + after_title)?);
        let mut core = content;
        let mut bands = [0.0; 4];
        let mut seats = filled(wires.len(), [None; 3])?;
        for (index, wire) in wires.iter().enumerate() {
            work.visit(1)?;
            let Some((_, _, side)) = wire.border() else {
                continue;
            };
            let face = side as usize;
            for (label, size) in [
                wire.spec.label,
                wire.spec.start_label.filter(|_| wire.manual_end_label(0)),
                wire.spec.end_label.filter(|_| wire.manual_end_label(1)),
            ]
            .into_iter()
            .enumerate()
            {
                let Some(size) = size else { continue };
                let along = bands[face] + spacing.loop_gap + depth(side, size) / 2.0;
                bands[face] = finite(along + depth(side, size) / 2.0)?;
                let far = finite(rightmost[face] + label_gap + across(side, size))?;
                let across = far - across(side, size) / 2.0;
                let local = match side.axis() {
                    Axis::Vertical => {
                        core.width = core.width.max(far);
                        Point {
                            x: across,
                            y: along,
                        }
                    }
                    Axis::Horizontal => {
                        core.height = core.height.max(far);
                        Point {
                            x: along,
                            y: across,
                        }
                    }
                };
                seats[index][label] = Some(centered(local, size));
            }
        }
        let room = |side: Side| {
            let index = side as usize;
            inset[index]
                + bands[index]
                + if bands[index] > 0.0 {
                    spacing.loop_gap
                } else {
                    0.0
                }
        };
        let needed_width = finite(room(Side::Left) + core.width + room(Side::Right))?;
        let needed_height = finite(room(Side::Top) + core.height + room(Side::Bottom))?;
        let title_width = |inset: f64| {
            let left = if flush_title { 0.0 } else { padding.left };
            let padded = left + title.width + padding.right;
            if movable_at_title == 0 {
                return padded;
            }
            let lane =
                |count: usize| title_clear + inset + (count - 1) as f64 * spacing.min_port_pitch;
            let with_ports = match group.title_position {
                TitlePosition::TopLeft | TitlePosition::BottomLeft => {
                    left + title.width + lane(movable_at_title)
                }
                TitlePosition::TopRight | TitlePosition::BottomRight => {
                    lane(movable_at_title) + title.width + padding.right
                }
                _ => title.width + 2.0 * lane(movable_at_title.div_ceil(2)),
            };
            padded.max(with_ports)
        };
        let port_span = |side: Side, inset: f64| {
            let n = counts[side as usize];
            if n == 0 {
                0.0
            } else {
                2.0 * inset + (n - 1) as f64 * spacing.min_port_pitch
            }
        };
        let corner = spacing.corner_clearance;
        let mut size = Size {
            width: finite(
                needed_width
                    .max(group.min_size.width)
                    .max(title_width(corner))
                    .max(port_span(Side::Top, corner))
                    .max(port_span(Side::Bottom, corner)),
            )?,
            height: finite(
                needed_height
                    .max(group.min_size.height)
                    .max(port_span(Side::Left, corner))
                    .max(port_span(Side::Right, corner)),
            )?,
        };
        if fit_width && (marker_insets != [0.0; 4] || fixed_reach != [0.0; 4]) {
            for side in SIDES {
                work.visit(1)?;
                if fixed_offsets[side as usize] > across(side, size) / 2.0 {
                    return Err(LayoutError::InvalidInput("port offset outside group face"));
                }
            }
            let marker_span = |side: Side| {
                let index = side as usize;
                port_span(side, corner.max(marker_insets[index])).max(2.0 * fixed_reach[index])
            };
            size.width = finite(
                size.width
                    .max(marker_span(Side::Top))
                    .max(marker_span(Side::Bottom))
                    .max(title_width(corner.max(marker_insets[title_side]))),
            )?;
            size.height = finite(
                size.height
                    .max(marker_span(Side::Left))
                    .max(marker_span(Side::Right)),
            )?;
        }
        let mut offset = Point {
            x: finite(room(Side::Left) + (size.width - needed_width) / 2.0)?,
            y: finite(room(Side::Top) + (size.height - needed_height) / 2.0)?,
        };
        if counts[title_side] == 1 && movable_at_title == 1 && !flush_title {
            // Keep an isolated entrance on the content's trunk. Borrow the unused shoulder
            // before widening the shell on the title's side.
            match group.title_position {
                TitlePosition::TopLeft | TitlePosition::BottomLeft => {
                    let clear_edge = (padding.left + title.width + title_clear).next_up();
                    let required = clear_edge - title_trunk;
                    let last = room(Side::Left) + (size.width - needed_width);
                    offset.x = finite(offset.x.max(required))?;
                    size.width = finite(size.width + (offset.x - last).max(0.0))?;
                }
                TitlePosition::TopRight | TitlePosition::BottomRight => {
                    let clear_edge =
                        (size.width - padding.right - title.width - title_clear).next_down();
                    let limit = clear_edge - title_trunk;
                    let first = room(Side::Left);
                    offset.x = finite(offset.x.min(limit).max(first))?;
                    size.width = finite(size.width + (first - limit).max(0.0))?;
                }
                _ => {}
            }
        }
        for (wire, boxes) in wires.iter().zip(&mut seats) {
            work.visit(1)?;
            let Some((_, _, side)) = wire.border() else {
                continue;
            };
            for rect in boxes.iter_mut().flatten() {
                match side {
                    Side::Top => {
                        rect.x += offset.x;
                        rect.y += offset.y - bands[side as usize] - spacing.loop_gap;
                    }
                    Side::Bottom => {
                        rect.x += offset.x;
                        rect.y += offset.y + core.height;
                    }
                    Side::Left => {
                        rect.x += offset.x - bands[side as usize] - spacing.loop_gap;
                        rect.y += offset.y;
                    }
                    Side::Right => {
                        rect.x += offset.x + core.width;
                        rect.y += offset.y;
                    }
                }
            }
        }
        Ok(Self {
            size,
            offset,
            seats,
            corridor,
            adaptive_top_title,
            protected_title_margin,
            marker_insets,
            flush_title,
            title_drop: before_title + title.height / 2.0,
            vertical_slack: (size.height - needed_height) / 2.0,
        })
    }

    pub(super) fn title_margin(&self) -> f64 {
        self.protected_title_margin.max(TITLE_CLEAR)
    }

    pub(super) fn border_inset(&self, side: Side, corner: f64) -> f64 {
        let marker = self.marker_insets[side as usize];
        let inset = if marker > corner { marker } else { corner };
        inset.min(across(side, self.size) / 2.0)
    }

    pub(super) fn reserve_title_room(
        &mut self,
        side: Side,
        extra: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let borrowed = self.vertical_slack.min(extra);
        self.size.height = finite(self.size.height + (extra - borrowed))?;
        let shift = if side == Side::Top { extra } else { -borrowed };
        self.offset.y = finite(self.offset.y + shift)?;
        for boxes in &mut self.seats {
            work.visit(1)?;
            for rect in boxes.iter_mut().flatten() {
                rect.y += shift;
            }
        }
        Ok(())
    }

    pub(super) fn title(
        &self,
        group: Option<&GroupSpec>,
        wires: &[Wire],
        ports: &[Point],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<TitlePlacement>> {
        let Some((group, title)) = group.and_then(|group| Some((group, group.title?))) else {
            return Ok(None);
        };
        let side = title_side(group.title_position);
        let padding = group.padding;
        let left = padding.left + title.width / 2.0;
        let right = (self.size.width - padding.right - title.width / 2.0).max(left);
        let center = (self.size.width / 2.0).clamp(left, right);
        let y = if side == Side::Top {
            self.title_drop
        } else {
            self.size.height - self.title_drop
        };
        let mut reserve_at = None;
        let x = match group.title_position {
            TitlePosition::TopLeft if self.flush_title => title.width / 2.0,
            TitlePosition::TopLeft | TitlePosition::BottomLeft => left,
            TitlePosition::TopRight | TitlePosition::BottomRight => right,
            TitlePosition::TopAuto if self.adaptive_top_title && ports.is_empty() => left,
            TitlePosition::TopAuto
                if self.adaptive_top_title && title.width > 0.0 && title.height > 0.0 =>
            {
                let candidates = [
                    (left, TitlePosition::TopLeft),
                    (right, TitlePosition::TopRight),
                    (center, TitlePosition::TopCenter),
                ];
                let mut scores = [(0usize, 0usize, 0usize, 0.0_f64); 3];
                for wire in wires {
                    work.visit(1)?;
                    let Some((end, slot, port_side)) = wire.border() else {
                        continue;
                    };
                    if port_side.axis() == Axis::Horizontal {
                        let outer = ports[slot];
                        let inner = wire.contacts[1 - end].expect("inner contact");
                        let runway = Point {
                            x: inner.at.x + self.offset.x,
                            ..outer
                        };
                        let margin = wire.title_clearance(end, self.title_margin());
                        let padded = Size {
                            width: finite(title.width + 2.0 * margin)?,
                            height: finite(title.height + 2.0 * margin)?,
                        };
                        for ((x, _), (_, lateral_hits, _, _)) in
                            candidates.into_iter().zip(&mut scores)
                        {
                            if blocks(centered(Point { x, y }, padded), outer, runway) {
                                *lateral_hits += 1;
                            }
                        }
                        continue;
                    }
                    if port_side != Side::Top {
                        continue;
                    }
                    let fixed = matches!(
                        wire.ends[end],
                        End::Border {
                            hint: PortSideHint::FixedOffset { .. },
                            ..
                        }
                    );
                    let clearance =
                        title.width / 2.0 + wire.title_clearance(end, self.title_margin());
                    for ((candidate, _), (fixed_hits, _, movable_hits, shift)) in
                        candidates.into_iter().zip(&mut scores)
                    {
                        let shortfall = clearance - (ports[slot].x - candidate).abs();
                        if shortfall > 0.0 {
                            if fixed {
                                *fixed_hits += 1;
                            } else {
                                *movable_hits += 1;
                            }
                            *shift = finite(*shift + shortfall)?;
                        }
                    }
                }
                let ((x, position), (_, _, movable_hits, _)) = candidates
                    .into_iter()
                    .zip(scores)
                    .min_by(|(_, a), (_, b)| {
                        a.0.cmp(&b.0)
                            .then_with(|| a.1.cmp(&b.1))
                            .then_with(|| a.2.cmp(&b.2))
                            .then_with(|| a.3.total_cmp(&b.3))
                    })
                    .expect("three title anchors");
                if movable_hits > 0 {
                    if let Some(x) = self.between_title_ports(
                        centered(Point { x, y }, title),
                        (left, right),
                        wires,
                        ports,
                        work,
                    )? {
                        return Ok(Some(TitlePlacement {
                            rect: centered(Point { x, y }, title),
                            reserve_at: None,
                        }));
                    }
                    // Nested lanes already reserve room to move their automatic title ports.
                    if self.protected_title_margin == 0.0 {
                        reserve_at = Some(position);
                    }
                }
                x
            }
            _ => center,
        };
        Ok(Some(TitlePlacement {
            rect: centered(Point { x, y }, title),
            reserve_at,
        }))
    }

    fn between_title_ports(
        &self,
        title: Rect,
        bounds: (f64, f64),
        wires: &[Wire],
        ports: &[Point],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<f64>> {
        let mut blocked = Vec::new();
        for wire in wires {
            work.visit(1)?;
            let Some((end, slot, side)) = wire.border() else {
                continue;
            };
            let outer = ports[slot];
            let margin = wire.title_clearance(end, self.title_margin());
            let interval = match side {
                Side::Top => (outer.x, outer.x),
                Side::Left | Side::Right
                    if outer.y > title.y - margin && outer.y < title.y + title.height + margin =>
                {
                    let inner = wire.contacts[1 - end].expect("inner contact").at.x + self.offset.x;
                    (outer.x.min(inner), outer.x.max(inner))
                }
                _ => continue,
            };
            let reach = title.width / 2.0 + margin;
            push(
                &mut blocked,
                (
                    (interval.0 - reach).next_down(),
                    (interval.1 + reach).next_up(),
                ),
            )?;
        }
        ports::closest_clear_port(title.center().x, bounds, &blocked, work)
    }
}
