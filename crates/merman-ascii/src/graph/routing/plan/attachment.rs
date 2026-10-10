use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::shape::GraphNodeShapeSemantics;
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
        let (contact, outward, connector) = match inward {
            StepDirection::Left
                if berth.x >= node.right() && (node.y..=node.bottom()).contains(&berth.y) =>
            {
                (
                    CanvasCoord {
                        x: node.right(),
                        y: berth.y,
                    },
                    StepDirection::Right,
                    charset.right_connector,
                )
            }
            StepDirection::Right
                if berth.x <= node.x && (node.y..=node.bottom()).contains(&berth.y) =>
            {
                (
                    CanvasCoord {
                        x: node.x,
                        y: berth.y,
                    },
                    StepDirection::Left,
                    charset.left_connector,
                )
            }
            StepDirection::Up
                if berth.y >= node.bottom() && (node.x..=node.right()).contains(&berth.x) =>
            {
                (
                    CanvasCoord {
                        x: berth.x,
                        y: node.bottom(),
                    },
                    StepDirection::Down,
                    charset.down_connector,
                )
            }
            StepDirection::Down
                if berth.y <= node.y && (node.x..=node.right()).contains(&berth.x) =>
            {
                (
                    CanvasCoord {
                        x: berth.x,
                        y: node.y,
                    },
                    StepDirection::Up,
                    charset.up_connector,
                )
            }
            _ => return None,
        };
        let semantics = GraphNodeShapeSemantics::new(node.shape);
        if !semantics.allows_route_contact(node, contact) {
            return None;
        }
        Some(Self {
            contact,
            outward,
            connector: semantics.uses_route_connector().then_some(connector),
        })
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
