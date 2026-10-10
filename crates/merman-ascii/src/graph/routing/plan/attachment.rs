use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::shape::{GraphNodeShapeSemantics, GraphNodeSide};
use super::super::path::StepDirection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) struct NodeAttachment {
    pub(in crate::graph::routing) contact: CanvasCoord,
    pub(in crate::graph::routing) outward: StepDirection,
    pub(in crate::graph::routing) connector: Option<char>,
}

impl NodeAttachment {
    pub(in crate::graph::routing) fn resolve(
        node: &NodeLayout,
        berth: CanvasCoord,
        inward: StepDirection,
        charset: &GraphCharset,
    ) -> Option<Self> {
        let (side, intercept, connector) = match inward {
            StepDirection::Left => (GraphNodeSide::Right, berth.y, charset.right_connector),
            StepDirection::Right => (GraphNodeSide::Left, berth.y, charset.left_connector),
            StepDirection::Up => (GraphNodeSide::Bottom, berth.x, charset.down_connector),
            StepDirection::Down => (GraphNodeSide::Top, berth.x, charset.up_connector),
        };
        let semantics = GraphNodeShapeSemantics::new(node.shape);
        let contact = semantics.route_contact(node, side, intercept)?;
        let attachment = Self {
            contact,
            outward: inward.opposite(),
            connector: semantics.uses_route_connector().then_some(connector),
        };
        if !attachment.allows_escape(berth) {
            return None;
        }
        Some(attachment)
    }

    pub(in crate::graph::routing) fn allows_escape(self, coord: CanvasCoord) -> bool {
        match self.outward {
            StepDirection::Up => coord.x == self.contact.x && coord.y <= self.contact.y,
            StepDirection::Down => coord.x == self.contact.x && coord.y >= self.contact.y,
            StepDirection::Left => coord.y == self.contact.y && coord.x <= self.contact.x,
            StepDirection::Right => coord.y == self.contact.y && coord.x >= self.contact.x,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) struct NodeAttachments {
    pub(in crate::graph::routing) start: NodeAttachment,
    pub(in crate::graph::routing) end: NodeAttachment,
}
