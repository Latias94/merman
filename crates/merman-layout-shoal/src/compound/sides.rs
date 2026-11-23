use super::hierarchy::{Hierarchy, Terminal};
use crate::flat::{self, BoundaryRank};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{Direction, Endpoint, Graph, NodeSpec, Options, PortSideHint, Side, Size, WorkError};

pub(super) fn flow_sides(direction: Direction) -> (Side, Side) {
    match direction {
        Direction::Down => (Side::Top, Side::Bottom),
        Direction::Up => (Side::Bottom, Side::Top),
        Direction::Right => (Side::Left, Side::Right),
        Direction::Left => (Side::Right, Side::Left),
    }
}

fn explicit(hint: PortSideHint) -> Option<Side> {
    match hint {
        PortSideHint::Auto => None,
        PortSideHint::Side(side) | PortSideHint::FixedOffset { side, .. } => Some(side),
    }
}

pub(super) fn assign(
    graph: &Graph,
    hierarchy: &Hierarchy,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Side>> {
    work.charge(hierarchy.borders.len())?;
    let mut sides = filled(hierarchy.borders.len(), Side::Top)?;
    for (index, border) in hierarchy.borders.iter().enumerate() {
        let (start, end) = flow_sides(hierarchy.levels[border.level].direction);
        let edge = graph.edges[border.edge];
        let owner = Endpoint::Group(crate::GroupId(border.level as u32));
        sides[index] = if border.outgoing && edge.target == owner {
            explicit(edge.target_port).unwrap_or(end)
        } else if !border.outgoing && edge.source == owner {
            explicit(edge.source_port).unwrap_or(start)
        } else if border.outgoing {
            end
        } else {
            start
        };
    }
    for &level_id in &hierarchy.order {
        let level = &hierarchy.levels[level_id];
        work.visit(level.legs.len() + 1)?;
        if !level.legs.iter().any(|leg| {
            [leg.source, leg.target]
                .iter()
                .any(|t| matches!(t, Terminal::Item(_, Some(_))))
        }) {
            continue;
        }
        let count = level
            .items
            .len()
            .checked_add(level.ports.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        let mut nodes = reserved(count)?;
        let (start, end) = flow_sides(level.direction);
        for (slot, &item) in level.items.iter().enumerate() {
            let mut node = NodeSpec::new(
                Size::default(),
                u32::try_from(slot).map_err(|_| WorkError::ArithmeticOverflow)?,
            );
            if let Endpoint::Node(id) = item {
                node.order_hint = graph.nodes[id.0 as usize].order_hint;
            }
            nodes.push(node);
        }
        for &port in &level.ports {
            let mut node = NodeSpec::new(
                Size::default(),
                u32::try_from(nodes.len()).map_err(|_| WorkError::ArithmeticOverflow)?,
            );
            node.boundary = if sides[port] == start {
                Some(BoundaryRank::Start)
            } else if sides[port] == end {
                Some(BoundaryRank::End)
            } else {
                None
            };
            nodes.push(node);
        }
        let index = |terminal| match terminal {
            Terminal::Item(item, _) => hierarchy.item_slot(item),
            Terminal::Boundary(port) => level.items.len() + hierarchy.borders[port].slot,
        };
        work.charge(level.legs.len())?;
        let mut links = reserved(level.legs.len())?;
        links.extend(
            level
                .legs
                .iter()
                .map(|leg| (index(leg.source), index(leg.target))),
        );
        let reversed = flat::orientations(&nodes, &links, options, work)?;
        for (leg, reversed) in level.legs.iter().zip(reversed) {
            let (outgoing, incoming) = if reversed { (start, end) } else { (end, start) };
            for (terminal, side) in [(leg.source, outgoing), (leg.target, incoming)] {
                if let Terminal::Item(_, Some(port)) = terminal {
                    sides[port] = side;
                }
            }
        }
    }
    Ok(sides)
}
