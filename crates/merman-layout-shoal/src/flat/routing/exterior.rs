use super::{LEAD, finite};
use crate::flat::{Component, Expanded, acyclic::Orientation, position};
use crate::work::{LayoutResult, Work, filled, push};
use crate::{Axis, Graph, Options, PortSideHint, Rect, ShapeContourHint, Side};

#[derive(Clone, Copy)]
pub(super) struct SidePort {
    pub(super) side: Side,
    pub(super) rail: f64,
}

impl SidePort {
    pub(super) fn contact(self, node: Rect) -> f64 {
        match self.side {
            Side::Left => node.x,
            Side::Right => node.x + node.width,
            _ => unreachable!("exterior side ports are horizontal"),
        }
    }
}

pub(in crate::flat) fn side_docking_allowed(
    g: &Expanded,
    c: &Component,
    graph: &Graph,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    work.visit(g.direct.len() + c.links.len())?;
    Ok(c.loops.is_empty()
        && !g.direct.contains(&true)
        && c.links.iter().all(|link| {
            let edge = &graph.edges[link.edge as usize];
            edge.source_port == PortSideHint::Auto
                && edge.target_port == PortSideHint::Auto
                && edge.start_label.is_none()
                && edge.end_label.is_none()
        }))
}

struct SideChannels<'a> {
    component: &'a Component,
    options: &'a Options,
    x: &'a [f64],
    used: Vec<(u32, f64, f64)>,
}

impl SideChannels<'_> {
    /// A terminal can turn inside its own rank when the leg has an unobstructed run to
    /// the rail. Keep other shafts pinned through that rank to preserve the clearance.
    fn reserve(
        &mut self,
        g: &mut Expanded,
        vertex: u32,
        port: SidePort,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let rank = g.rank[vertex as usize];
        let row = &g.layers[rank as usize];
        let half_width = g.width[vertex as usize] * 0.5;
        let contact = self.x[vertex as usize]
            + if port.side == Side::Left {
                -half_width
            } else {
                half_width
            };
        let (lo, hi) = (contact.min(port.rail), contact.max(port.rail));
        let pad = self.options.spacing.edge * 0.5;
        work.visit(row.len() + self.used.len())?;
        if row.iter().any(|&peer| {
            if peer == vertex {
                return false;
            }
            let (left, right) = position::extents(g, self.component, self.options, peer);
            let p = self.x[peer as usize];
            lo - pad <= p + right && p - left <= hi + pad
        }) || self
            .used
            .iter()
            .any(|&(r, left, right)| r == rank && lo - pad <= right && left <= hi + pad)
        {
            return Ok(false);
        }
        work.charge(1)?;
        push(&mut self.used, (rank, lo, hi))?;
        work.visit(row.len())?;
        for &peer in row {
            g.keep[peer as usize] = true;
        }
        Ok(true)
    }
}

pub(super) fn attach_side_ports(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    work: &mut Work<'_>,
) -> LayoutResult<Vec<[Option<SidePort>; 2]>> {
    let mut side_ports = Vec::new();
    let mut channels = SideChannels {
        component: c,
        options,
        x,
        used: Vec::new(),
    };
    let allow_side = !g.exterior_lanes.is_empty() && side_docking_allowed(g, c, graph, work)?;
    if allow_side {
        work.charge(c.links.len())?;
        side_ports = filled(c.links.len(), [None; 2])?;
    }
    let half_separation = options.spacing.edge * 0.5;
    for i in 0..g.exterior_lanes.len() {
        let lane = &g.exterior_lanes[i];
        let edge = lane.edge;
        let chain = &g.chains[edge];
        let last = chain.len() - 1;
        let vertices = [chain[0], chain[last]];
        let segments = [g.pred.at(chain[1])[0], g.succ.at(chain[last - 1])[0]];
        let rail = x[chain[1] as usize];
        let spec = &graph.edges[c.links[edge].edge as usize];
        let mut markers = [spec.start_marker, spec.end_marker];
        let mut hints = [spec.source_port, spec.target_port];
        if orientation.reversed[edge] {
            markers.swap(0, 1);
            hints.swap(0, 1);
        }
        let side = if lane.right { Side::Right } else { Side::Left };
        let sign = if lane.right { 1.0 } else { -1.0 };
        for end in 0..2 {
            work.visit(1)?;
            let v = vertices[end];
            let i = v as usize;
            let node = &c.nodes[i];
            if hints[end] != PortSideHint::Auto || node.anchor_y.is_some() {
                continue;
            }
            let want = rail - x[i];
            let span = node
                .port_span
                .unwrap_or((g.width[i] - 2.0 * options.spacing.corner_clearance).max(0.0));
            let segment = segments[end];
            let (siblings, offsets) = if end == 0 {
                (g.succ.at(v), &mut g.source_offset)
            } else {
                (g.pred.at(v), &mut g.target_offset)
            };
            let offset = offsets[segment as usize];
            if want.abs() <= span.min(g.width[i]) * 0.5 {
                work.visit(siblings.len())?;
                if siblings.iter().all(|&sibling| {
                    if sibling == segment {
                        return true;
                    }
                    let old = offsets[sibling as usize] - offset;
                    let new = offsets[sibling as usize] - want;
                    old != 0.0 && new.signum() == old.signum() && new.abs() >= old.abs()
                }) {
                    offsets[segment as usize] = want;
                }
                continue;
            }

            let marker = markers[end];
            let side_span = node
                .perpendicular_port_span
                .or(node.port_span)
                .unwrap_or((g.height[i] - 2.0 * options.spacing.corner_clearance).max(0.0));
            if !allow_side
                || side_span.min(g.height[i]) < marker.width
                || g.height[i] == 0.0
                || sign * want - g.width[i] * 0.5
                    <= marker.length.max(LEAD) + half_separation.max(LEAD)
            {
                continue;
            }
            work.visit(siblings.len())?;
            if siblings.iter().any(|&sibling| {
                sibling != segment && sign * (offset - offsets[sibling as usize]) <= 0.0
            }) {
                continue;
            }
            let port = SidePort { side, rail };
            if channels.reserve(g, v, port, work)? {
                side_ports[edge][end] = Some(port);
            }
        }
    }
    attach_terminal_ports(g, orientation, graph, &mut channels, &mut side_ports, work)?;
    Ok(side_ports)
}

#[derive(Clone, Copy)]
struct Arrival {
    edge: usize,
    end: usize,
    port: SidePort,
    reach: f64,
}

#[derive(Clone, Copy, Default)]
struct TerminalFan {
    incoming: usize,
    held: [bool; 2],
    choices: [Option<Arrival>; 2],
}

fn attach_terminal_ports(
    g: &mut Expanded,
    orientation: &Orientation,
    graph: &Graph,
    channels: &mut SideChannels<'_>,
    side_ports: &mut Vec<[Option<SidePort>; 2]>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if graph.direction.axis() != Axis::Horizontal {
        return Ok(());
    }
    let c = channels.component;
    let spacing = channels.options.spacing;
    let small_circle = |node: &crate::NodeSpec| {
        node.split_incoming_ports
            && node.contour == ShapeContourHint::Ellipse
            && node.size.width == node.size.height
            && node.size.width > 0.0
            && node.size.width <= 2.0 * spacing.port_pitch
            && node.anchor_y.is_none()
    };
    work.visit(c.nodes.len())?;
    if !c.nodes.iter().any(small_circle) {
        return Ok(());
    }
    work.charge(c.nodes.len())?;
    let mut fans = crate::work::reserved(c.nodes.len())?;
    fans.extend(
        c.nodes
            .iter()
            .map(|node| small_circle(node).then(TerminalFan::default)),
    );
    for edge in &c.loops {
        work.visit(1)?;
        fans[edge.node as usize] = None;
    }
    for (edge, link) in c.links.iter().enumerate() {
        work.visit(1)?;
        let spec = &graph.edges[link.edge as usize];
        let reversed = orientation.reversed[edge];
        let ranked_side = side_ports.get(edge).copied().unwrap_or_default();
        for (semantic_end, (vertex, hint)) in
            [(link.from, spec.source_port), (link.to, spec.target_port)]
                .into_iter()
                .enumerate()
        {
            let Some(fan) = &mut fans[vertex as usize] else {
                continue;
            };
            let end = semantic_end ^ usize::from(reversed);
            let hint = crate::flat::to_canonical_port(graph.direction, hint);
            let side = ranked_side[end].map(|port| port.side).or(match hint {
                PortSideHint::Auto => None,
                PortSideHint::Side(side) | PortSideHint::FixedOffset { side, .. } => Some(side),
            });
            match side {
                Some(Side::Left) => fan.held[0] = true,
                Some(Side::Right) => fan.held[1] = true,
                _ => {}
            }
            if semantic_end == 0 {
                continue;
            }
            fan.incoming += 1;
            let v = vertex as usize;
            if side.is_some() || g.direct[edge] || spec.end_marker.width > g.height[v] {
                continue;
            }
            let chain = &g.chains[edge];
            let previous = chain[if reversed { 1 } else { chain.len() - 2 }] as usize;
            let center = channels.x[v];
            let delta = channels.x[previous] - center;
            // Use one normal approach per side. Additional or centered arrivals keep the
            // flow face. Reserve the painted marker lead before turning across the rank.
            let lead = spec.marker_runways()[1].max(LEAD) + spacing.edge * 0.5;
            let reach = delta.abs() - g.width[v] * 0.5 - lead;
            if reach < 0.0 {
                continue;
            }
            let right = delta > 0.0;
            let choice = &mut fan.choices[usize::from(right)];
            if choice.is_none_or(|old| reach > old.reach) {
                let rail = finite(center + (g.width[v] * 0.5 + lead).copysign(delta))?;
                let side = if right { Side::Right } else { Side::Left };
                *choice = Some(Arrival {
                    edge,
                    end,
                    port: SidePort { side, rail },
                    reach,
                });
            }
        }
    }
    for (vertex, fan) in fans.into_iter().enumerate() {
        work.visit(1)?;
        let Some(fan) = fan.filter(|fan| fan.incoming >= 2) else {
            continue;
        };
        for (choice, held) in fan.choices.into_iter().zip(fan.held) {
            if let Some(choice) = choice
                && !held
                && channels.reserve(g, vertex as u32, choice.port, work)?
            {
                if side_ports.is_empty() {
                    work.charge(c.links.len())?;
                    *side_ports = filled(c.links.len(), [None; 2])?;
                }
                side_ports[choice.edge][choice.end] = Some(choice.port);
            }
        }
    }
    Ok(())
}
