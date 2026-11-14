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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameSegment {
    Blocked,
    Clear,
    Boundary,
}

impl CompoundRouteGeometry<'_> {
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
            if !group_allows_segment(
                group,
                group_contains_node(group, self.from),
                group_contains_node(group, self.to),
                from,
                to,
            ) {
                return Ok(FrameSegment::Blocked);
            }
            if group_perimeter_contains(group, to) {
                access = FrameSegment::Boundary;
            }
        }
        Ok(access)
    }
}

fn group_contains_node(group: &GroupLayout, node: &NodeLayout) -> bool {
    group.x < node.x
        && node.right() < group.right()
        && group.y < node.y
        && node.bottom() < group.bottom()
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
        let group = GroupLayout {
            id: "group".into(),
            kind: GraphGroupKind::Container,
            title: GraphLabel::empty_with_profile(crate::options::TerminalWidthProfile::Unicode),
            style: crate::graph::model::GraphGroupStyle::default(),
            divider_span: None,
            x: 10,
            y: 10,
            width: 11,
            height: 11,
        };
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
}
