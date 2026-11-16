use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, GraphLayout, NodeLayout};
use super::super::super::model::GraphGroupKind;
use super::super::super::shape::{GraphNodeShapeSemantics, GraphNodeSide};
use super::super::path::StepDirection;
use crate::error::Result;
use crate::resource::ResourceContext;

#[derive(Clone, Copy)]
pub(in crate::graph::routing) struct EndpointGeometry<'a> {
    pub(in crate::graph::routing) layout: &'a NodeLayout,
    pub(in crate::graph::routing) interior: bool,
    pub(in crate::graph::routing) group: bool,
}

impl<'a> EndpointGeometry<'a> {
    #[cfg(test)]
    pub(super) fn node(layout: &'a NodeLayout) -> Self {
        Self {
            layout,
            interior: false,
            group: false,
        }
    }

    pub(in crate::graph::routing) fn for_route(
        layout: &'a NodeLayout,
        other: &NodeLayout,
        graph_layout: &GraphLayout,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.checkpoint()?;
        resources.charge_layout_work(graph_layout.groups.len())?;
        let group = graph_layout
            .groups
            .iter()
            .find(|group| group.kind == GraphGroupKind::Container && group.id == layout.id);
        let interior =
            group.is_some_and(|group| super::super::geometry::group_contains_node(group, other));
        Ok(Self {
            layout,
            interior,
            group: group.is_some(),
        })
    }

    pub(in crate::graph::routing) fn resolve(
        self,
        berth: CanvasCoord,
        inward: StepDirection,
        charset: &GraphCharset,
    ) -> Option<NodeAttachment> {
        let attachment =
            NodeAttachment::resolve_in_region(self.layout, berth, inward, self.interior, charset)?;
        let contact = attachment.contact;
        if self.group
            && (contact.x == self.layout.x || contact.x == self.layout.right())
            && (contact.y == self.layout.y || contact.y == self.layout.bottom())
        {
            return None;
        }
        Some(attachment)
    }
}

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
        Self::resolve_in_region(node, berth, inward, false, charset)
    }

    fn resolve_in_region(
        node: &NodeLayout,
        berth: CanvasCoord,
        inward: StepDirection,
        interior: bool,
        charset: &GraphCharset,
    ) -> Option<Self> {
        let boundary_direction = if interior { inward.opposite() } else { inward };
        let (side, intercept) = match boundary_direction {
            StepDirection::Left => (GraphNodeSide::Right, berth.y),
            StepDirection::Right => (GraphNodeSide::Left, berth.y),
            StepDirection::Up => (GraphNodeSide::Bottom, berth.x),
            StepDirection::Down => (GraphNodeSide::Top, berth.x),
        };
        let connector = match inward {
            StepDirection::Left => charset.right_connector,
            StepDirection::Right => charset.left_connector,
            StepDirection::Up => charset.down_connector,
            StepDirection::Down => charset.up_connector,
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
