use super::plan::{NodeAttachment, NodeAttachments, RoutePlan};
use crate::error::Result;
use crate::graph::layout::{CanvasCoord, GraphLayout, GroupLayout, NodeLayout};
use crate::graph::model::GraphGroupKind;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

#[derive(Clone, Copy)]
pub(super) struct CompoundRouteGeometry<'a> {
    pub(super) layout: &'a GraphLayout,
    pub(super) from: &'a NodeLayout,
    pub(super) to: &'a NodeLayout,
    pub(super) attachments: Option<NodeAttachments>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameSegment {
    Blocked,
    Clear,
    Boundary,
}

impl CompoundRouteGeometry<'_> {
    pub(super) fn endpoints_allow_plan(
        self,
        plan: &RoutePlan,
        resources: &ResourceContext,
    ) -> Result<bool> {
        if self.attachments.is_none() {
            return Ok(false);
        }
        for group in &self.layout.groups {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if group.kind != GraphGroupKind::Container
                || (group.id != self.from.id && group.id != self.to.id)
            {
                continue;
            }
            for (_, cell) in plan.active_cells() {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                if !self.group_allows_segment(group, cell.coord, cell.coord) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub(super) fn segment(
        self,
        from: CanvasCoord,
        to: CanvasCoord,
        resources: &mut ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<FrameSegment> {
        let mut access = FrameSegment::Clear;
        for (index, group) in self.layout.groups.iter().enumerate() {
            execution.checkpoint_loop(OperationPhase::Layout, index)?;
            resources.charge_layout_work(1)?;
            if group.kind != GraphGroupKind::Container {
                continue;
            }
            if !self.group_allows_segment(group, from, to) {
                return Ok(FrameSegment::Blocked);
            }
            if group_perimeter_contains(group, to) {
                access = FrameSegment::Boundary;
            }
        }
        Ok(access)
    }

    fn group_allows_segment(self, group: &GroupLayout, from: CanvasCoord, to: CanvasCoord) -> bool {
        match (group.id == self.from.id, group.id == self.to.id) {
            (false, false) => group_allows_segment(
                group,
                group_contains_node(group, self.from),
                group_contains_node(group, self.to),
                from,
                to,
            ),
            (owns_start, owns_end) => self.attachments.is_some_and(|attachments| {
                let contacts = [attachments.start, attachments.end];
                let (interior, contacts) = match (owns_start, owns_end) {
                    (true, true) => (false, &contacts[..]),
                    (true, false) => (group_contains_node(group, self.to), &contacts[..1]),
                    _ => (group_contains_node(group, self.from), &contacts[1..]),
                };
                group_allows_endpoint_segment(group, interior, contacts, from, to)
            }),
        }
    }
}

pub(super) fn group_contains_node(group: &GroupLayout, node: &NodeLayout) -> bool {
    group.x < node.x
        && node.right() < group.right()
        && group.y < node.y
        && node.bottom() < group.bottom()
}

fn group_allows_endpoint_segment(
    group: &GroupLayout,
    interior: bool,
    attachments: &[NodeAttachment],
    from: CanvasCoord,
    to: CanvasCoord,
) -> bool {
    let trim_contact = |coord: CanvasCoord, other: CanvasCoord| {
        let Some(attachment) = attachments
            .iter()
            .find(|attachment| coord == attachment.contact)
        else {
            return Some(coord);
        };
        if !attachment.allows_escape(other) {
            return None;
        }
        match attachment.outward {
            super::path::StepDirection::Left => {
                coord.x.checked_sub(1).map(|x| CanvasCoord { x, ..coord })
            }
            super::path::StepDirection::Right => {
                coord.x.checked_add(1).map(|x| CanvasCoord { x, ..coord })
            }
            super::path::StepDirection::Up => {
                coord.y.checked_sub(1).map(|y| CanvasCoord { y, ..coord })
            }
            super::path::StepDirection::Down => {
                coord.y.checked_add(1).map(|y| CanvasCoord { y, ..coord })
            }
        }
    };
    let (Some(start), Some(end)) = (trim_contact(from, to), trim_contact(to, from)) else {
        return false;
    };
    group_allows_segment(group, interior, interior, start, end)
}

fn group_perimeter_contains(group: &GroupLayout, coord: CanvasCoord) -> bool {
    ((coord.x == group.x || coord.x == group.right())
        && (group.y..=group.bottom()).contains(&coord.y))
        || ((coord.y == group.y || coord.y == group.bottom())
            && (group.x..=group.right()).contains(&coord.x))
}

fn group_allows_segment(
    group: &GroupLayout,
    contains_from: bool,
    contains_to: bool,
    from: CanvasCoord,
    to: CanvasCoord,
) -> bool {
    let min_x = from.x.min(to.x);
    let max_x = from.x.max(to.x);
    let min_y = from.y.min(to.y);
    let max_y = from.y.max(to.y);
    if contains_from && contains_to {
        return group.x < min_x
            && max_x < group.right()
            && group.y < min_y
            && max_y < group.bottom();
    }
    let intersects =
        min_x <= group.right() && max_x >= group.x && min_y <= group.bottom() && max_y >= group.y;
    if !intersects {
        return true;
    }
    if !contains_from && !contains_to {
        return false;
    }
    if from.y == to.y {
        group.y < from.y && from.y < group.bottom()
    } else {
        group.x < from.x && from.x < group.right()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::label::GraphLabel;

    #[test]
    fn compound_steps_cross_owned_sides_transversely_without_touching_corners() {
        let group = group();
        for (from, to) in [
            ((5, 15), (15, 15)),
            ((15, 15), (25, 15)),
            ((15, 5), (15, 15)),
            ((15, 15), (15, 25)),
        ] {
            let from = CanvasCoord {
                x: from.0,
                y: from.1,
            };
            let to = CanvasCoord { x: to.0, y: to.1 };
            assert!(group_allows_segment(&group, true, false, from, to));
            assert!(group_allows_segment(&group, false, true, to, from));
            assert!(!group_allows_segment(&group, false, false, from, to));
            assert!(!group_allows_segment(&group, true, true, from, to));
        }
        for (from, to) in [
            ((5, 10), (15, 10)),
            ((15, 20), (25, 20)),
            ((10, 5), (10, 15)),
            ((20, 15), (20, 25)),
        ] {
            let from = CanvasCoord {
                x: from.0,
                y: from.1,
            };
            let to = CanvasCoord { x: to.0, y: to.1 };
            assert!(!group_allows_segment(&group, true, false, from, to));
        }
        let inside = CanvasCoord { x: 12, y: 15 };
        let other_inside = CanvasCoord { x: 18, y: 15 };
        assert!(group_allows_segment(
            &group,
            true,
            true,
            inside,
            other_inside
        ));
        assert!(!group_allows_segment(
            &group,
            false,
            false,
            inside,
            other_inside
        ));
    }

    #[test]
    fn group_endpoint_segments_use_only_the_selected_contact_and_region() {
        use crate::graph::routing::path::StepDirection;
        let group = group();
        let contact = CanvasCoord { x: 10, y: 15 };
        for (interior, outward, berth) in [
            (false, StepDirection::Left, CanvasCoord { x: 9, y: 15 }),
            (true, StepDirection::Right, CanvasCoord { x: 11, y: 15 }),
        ] {
            let attachment = NodeAttachment {
                contact,
                outward,
                connector: None,
            };
            assert!(group_allows_endpoint_segment(
                &group,
                interior,
                &[attachment],
                contact,
                berth
            ));
            assert!(group_allows_endpoint_segment(
                &group,
                interior,
                &[attachment],
                berth,
                contact
            ));
            assert!(!group_allows_endpoint_segment(
                &group,
                !interior,
                &[attachment],
                contact,
                berth
            ));
            assert!(!group_allows_endpoint_segment(
                &group,
                interior,
                &[attachment],
                berth,
                CanvasCoord { x: 10, y: 16 },
            ));
            assert!(!group_allows_endpoint_segment(
                &group,
                interior,
                &[attachment],
                contact,
                CanvasCoord { x: 10, y: 16 },
            ));
        }
    }

    #[test]
    fn group_self_loop_segments_allow_both_contacts_and_reserve_the_interior() {
        use crate::graph::routing::path::StepDirection;
        let group = group();
        let attachments = [
            NodeAttachment {
                contact: CanvasCoord { x: 20, y: 15 },
                outward: StepDirection::Right,
                connector: None,
            },
            NodeAttachment {
                contact: CanvasCoord { x: 15, y: 20 },
                outward: StepDirection::Down,
                connector: None,
            },
        ];
        for (contact, berth) in [
            (attachments[0].contact, CanvasCoord { x: 21, y: 15 }),
            (attachments[1].contact, CanvasCoord { x: 15, y: 21 }),
        ] {
            assert!(group_allows_endpoint_segment(
                &group,
                false,
                &attachments,
                contact,
                berth
            ));
            assert!(group_allows_endpoint_segment(
                &group,
                false,
                &attachments,
                berth,
                contact
            ));
        }
        for (from, to) in [
            (CanvasCoord { x: 15, y: 15 }, CanvasCoord { x: 16, y: 15 }),
            (CanvasCoord { x: 20, y: 16 }, CanvasCoord { x: 21, y: 16 }),
            (attachments[0].contact, CanvasCoord { x: 19, y: 15 }),
            (attachments[1].contact, CanvasCoord { x: 15, y: 19 }),
            (attachments[0].contact, attachments[1].contact),
        ] {
            assert!(!group_allows_endpoint_segment(
                &group,
                false,
                &attachments,
                from,
                to
            ));
        }
    }

    fn group() -> GroupLayout {
        GroupLayout {
            id: "group".into(),
            kind: GraphGroupKind::Container,
            title: GraphLabel::empty_with_profile(crate::options::TerminalWidthProfile::Unicode),
            title_transform: crate::graph::surface::OutputTransform::Identity,
            style: crate::graph::model::GraphGroupStyle::default(),
            divider_span: None,
            x: 10,
            y: 10,
            width: 11,
            height: 11,
        }
    }
}
