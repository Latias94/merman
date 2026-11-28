//! Mermaid 12.4 ELK implicit-port alignment for crowded arrowheads.
//! Source: elk/spreadArrowheads.ts and elk/render.ts at 9f93a44a36e46a36b1379f9de72fb1ee5684ee3f.

use std::collections::HashMap;

use merman_layout_elk as elk;

use super::{hierarchy::charge_adapter_work, resolved_node_directions};
use crate::{Result, layout_work::OperationLayoutWorkControl};

const MIN_ARROWHEADS: usize = 3;
const PORT_INSET: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrowhead {
    Point,
    Circle,
    Cross,
    /// Any nonempty marker without a measured extent, including relation markers.
    Other,
}

impl Arrowhead {
    fn extent(self, margin: bool, start: bool) -> f64 {
        match self {
            Self::Point if margin && start => 14.0,
            Self::Point if margin => 12.8,
            Self::Point | Self::Other => 8.0,
            Self::Circle if margin => 14.0,
            Self::Cross if margin => 12.0,
            Self::Circle | Self::Cross => 11.0,
        }
    }
}

/// Marker variants at the semantic endpoints, even when the provider edge is reversed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EdgeArrowheads {
    pub start: Option<Arrowhead>,
    pub end: Option<Arrowhead>,
    pub margin: bool,
}

#[derive(Default)]
struct Side {
    ports: usize,
    arrowheads: usize,
    extent: f64,
}

impl Side {
    fn add_end(&mut self, extent: Option<f64>) -> bool {
        self.ports += 1;
        if let Some(extent) = extent {
            self.arrowheads += 1;
            self.extent = self.extent.max(extent);
        }
        self.arrowheads >= MIN_ARROWHEADS
    }

    fn overlaps(&self, side_length: f64) -> bool {
        let inset = (elk::MERMAID_PORTS_SURROUNDING_MARGIN - PORT_INSET).max(0.0);
        let gap = (side_length - 2.0 * inset) / (self.ports as f64 + 1.0);
        self.arrowheads >= MIN_ARROWHEADS && gap < self.extent
    }
}

#[derive(Default)]
struct NodeEnds {
    entering: Side,
    leaving: Side,
}

pub(super) type ArrowheadsForEdge<'a> =
    dyn FnMut(usize, &mut Option<&mut OperationLayoutWorkControl>) -> Result<EdgeArrowheads> + 'a;

/// Plan writes in node declaration order. `reversed` contains ascending edge indices from the
/// orientation pass. Endpoints have already been swapped, and edge indices are still semantic.
pub(super) fn crowded_ports(
    graph: &elk::Graph,
    reversed: &[usize],
    arrowheads: &mut ArrowheadsForEdge<'_>,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<Vec<(usize, Option<elk::PortAlignment>)>> {
    if graph.edges.len() < MIN_ARROWHEADS || graph.nodes.is_empty() {
        return Ok(Vec::new());
    }
    let mut ends = HashMap::<&str, NodeEnds>::new();
    let mut crowded = false;
    let mut reversed = reversed.iter().copied().peekable();
    for (index, edge) in graph.edges.iter().enumerate() {
        charge_adapter_work(work, 1)?;
        let was_reversed = reversed.next_if_eq(&index).is_some();
        if edge.source == edge.target {
            continue;
        }
        charge_adapter_work(work, 2)?;
        let markers = arrowheads(index, work)?;
        let start = markers.start.map(|kind| kind.extent(markers.margin, true));
        let end = markers.end.map(|kind| kind.extent(markers.margin, false));
        let (leaving, entering) = if was_reversed {
            (end, start)
        } else {
            (start, end)
        };
        crowded |= ends
            .entry(&edge.source)
            .or_default()
            .leaving
            .add_end(leaving);
        crowded |= ends
            .entry(&edge.target)
            .or_default()
            .entering
            .add_end(entering);
    }
    if !crowded {
        return Ok(Vec::new());
    }

    charge_adapter_work(work, graph.nodes.len())?;
    let directions = if graph
        .nodes
        .iter()
        .any(|node| node.kind == elk::NodeKind::Group && node.direction.is_some())
    {
        Some(resolved_node_directions(
            graph.nodes.iter().map(|node| {
                let direction = if node.kind == elk::NodeKind::Group {
                    node.direction
                } else {
                    None
                };
                (node.id.as_str(), node.parent.as_deref(), direction)
            }),
            graph.nodes.len(),
            graph.direction,
            work,
        )?)
    } else {
        None
    };

    let mut alignments = Vec::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        charge_adapter_work(work, 1)?;
        if node.kind == elk::NodeKind::Group
            || node.port_alignment == Some(elk::PortAlignment::Center)
        {
            continue;
        }
        let Some(ends) = ends.get(node.id.as_str()) else {
            continue;
        };
        // A leaf is laid out in its parent's direction, including inherited subgraph directions.
        let direction = directions
            .as_ref()
            .zip(node.parent.as_deref())
            .and_then(|(directions, parent)| directions.get(parent).copied())
            .unwrap_or(graph.direction);
        let side_length = match direction {
            elk::Direction::Left | elk::Direction::Right => node.height,
            elk::Direction::Up | elk::Direction::Down => node.width,
        };
        if ends.entering.overlaps(side_length) || ends.leaving.overlaps(side_length) {
            alignments.push((index, node.port_alignment));
        }
    }
    Ok(alignments)
}
