use super::*;
use crate::work::push;
use crate::{GroupSpec, Options, TitlePosition, WidthPolicy};

pub(super) struct Fit {
    pub size: Size,
    pub offset: Point,
    pub seats: Vec<[Option<Rect>; 3]>,
    pub corridor: [f64; 4],
    marker_insets: [f64; 4],
    title_drop: f64,
    vertical_slack: f64,
}

pub(super) fn title_side(position: TitlePosition) -> Side {
    match position {
        TitlePosition::TopLeft | TitlePosition::TopCenter | TitlePosition::TopRight => Side::Top,
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
        options: &Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let group = group.copied().unwrap_or_default();
        let padding = group.padding;
        let mut inset = [padding.top, padding.right, padding.bottom, padding.left];
        let title = group.title.unwrap_or_default();
        let title_side = title_side(group.title_position) as usize;
        inset[title_side] = inset[title_side].max(title.height);
        let mut before_title = (inset[title_side] - title.height) / 2.0;
        let mut after_title = before_title;
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
        let mut title_clear = TITLE_CLEAR;
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
            if index == title_side && title.width > 0.0 && title.height > 0.0 {
                if !matches!(hint, PortSideHint::FixedOffset { .. }) {
                    movable_at_title += 1;
                    title_clear = title_clear.max(TITLE_CLEAR + markers[end].width / 2.0);
                }
                before_title = before_title.max(markers[end].length + spacing.edge);
                let shaft = if (flat.is_none() || wire.own) && inner.side == side {
                    markers[1 - end].length
                } else {
                    0.0
                };
                after_title = after_title.max(shaft + spacing.edge);
            }
            inset[index] = inset[index].max(markers[end].length + spacing.edge);
            if (flat.is_none() || wire.own) && markers[1 - end].width > 0.0 {
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
            if flat.is_none() {
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
                wire.spec.label.filter(|_| wire.band),
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
            let padded = padding.left + title.width + padding.right;
            if movable_at_title == 0 {
                return padded;
            }
            let lane =
                |count: usize| title_clear + inset + (count - 1) as f64 * spacing.min_port_pitch;
            let with_ports = match group.title_position {
                TitlePosition::TopLeft | TitlePosition::BottomLeft => {
                    padding.left + title.width + lane(movable_at_title)
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
        let offset = Point {
            x: finite(room(Side::Left) + (size.width - needed_width) / 2.0)?,
            y: finite(room(Side::Top) + (size.height - needed_height) / 2.0)?,
        };
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
            marker_insets,
            title_drop: before_title + title.height / 2.0,
            vertical_slack: (size.height - needed_height) / 2.0,
        })
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
        options: &Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Rect>> {
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
        let x = match group.title_position {
            TitlePosition::TopLeft | TitlePosition::BottomLeft => left,
            TitlePosition::TopRight | TitlePosition::BottomRight => right,
            _ if options.group_policy.adaptive_title_clearance => {
                let mut chosen = center;
                for candidate in [center, left, right] {
                    let mut clear = true;
                    for wire in wires {
                        work.visit(1)?;
                        if let Some((end, slot, port_side)) = wire.border()
                            && port_side == side
                        {
                            let marker = [wire.spec.start_marker, wire.spec.end_marker][end];
                            if (ports[slot].x - candidate).abs()
                                < title.width / 2.0 + marker.width / 2.0 + TITLE_CLEAR
                            {
                                clear = false;
                                break;
                            }
                        }
                    }
                    if clear {
                        chosen = candidate;
                        break;
                    }
                }
                chosen
            }
            _ => center,
        };
        Ok(Some(centered(Point { x, y }, title)))
    }
}
