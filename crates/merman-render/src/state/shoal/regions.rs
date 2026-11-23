use super::MeasuredState;
use crate::Result;
use crate::graph_layout::ir::{Direction, Endpoint, Rect, Side, SolvedEdge, SolvedGraph};
use crate::layout_work::OperationLayoutWorkControl;
use kurbo::Rect as Box2;

#[derive(Default)]
struct Level {
    children: Vec<Endpoint>,
    routes: Vec<usize>,
    has_regions: bool,
}

impl Level {
    fn record_route(&mut self, edge: usize) -> bool {
        if self.has_regions && self.routes.last() != Some(&edge) {
            self.routes.push(edge);
        }
        self.has_regions
    }
}

pub(super) fn stretch(
    input: &MeasuredState<'_>,
    solved: &mut SolvedGraph,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let is_region = |group: u32| input.groups[group as usize].node.shape == "divider";
    if !input
        .groups
        .iter()
        .any(|group| group.node.shape == "divider")
    {
        return Ok(());
    }
    let root = solved.groups.len();
    let level_count = work.checked_add(root, 1)?;
    let owner = |parent: Option<u32>| parent.map_or(root, |id| id as usize);
    work.charge_adapter(work.checked_add(
        work.checked_mul(level_count, 4)?,
        work.checked_add(solved.nodes.len(), solved.edges.len())?,
    )?)?;
    let mut levels: Vec<_> = (0..level_count).map(|_| Level::default()).collect();
    for (node, measured) in input.graph.nodes.iter().enumerate() {
        levels[owner(measured.parent)]
            .children
            .push(Endpoint::Node(node as u32));
    }
    for (group, placed) in solved.groups.iter().enumerate() {
        let level = &mut levels[owner(placed.parent)];
        let group = group as u32;
        level.children.push(Endpoint::Group(group));
        level.has_regions |= is_region(group);
    }

    work.charge_adapter(solved.border_stitches.len())?;
    let mut stitches = solved.border_stitches.iter().peekable();
    let mut attached = vec![[false; 4]; root];
    let mut route_boxes = vec![Box2::ZERO; solved.edges.len()];
    for (index, edge) in solved.edges.iter().enumerate() {
        work.charge_adapter(0)?;
        let terminals = [(edge.source, edge.ports.0), (edge.target, edge.ports.1)];
        let crossings = std::iter::from_fn(|| {
            stitches
                .next_if(|stitch| stitch.edge == index)
                .map(|stitch| (Endpoint::Group(stitch.group), stitch.side))
        });
        let mut needed = false;
        for (endpoint, side) in terminals.into_iter().chain(crossings) {
            let parent = match endpoint {
                Endpoint::Node(node) => input.graph.nodes[node as usize].parent,
                Endpoint::Group(group) => {
                    let group = group as usize;
                    attached[group][side as usize] = true;
                    needed |= levels[group].record_route(index);
                    solved.groups[group].parent
                }
            };
            needed |= levels[owner(parent)].record_route(index);
        }
        if needed {
            work.charge_adapter(edge.points.len())?;
            route_boxes[index] = route_bounds(edge);
        }
    }

    let mut pending = vec![root];
    let mut siblings = Vec::new();
    while let Some(container) = pending.pop() {
        work.charge_adapter(0)?;
        let level = &levels[container];
        pending.extend(level.children.iter().filter_map(|child| match child {
            Endpoint::Group(group) => Some(*group as usize),
            Endpoint::Node(_) => None,
        }));
        if !level.has_regions {
            continue;
        }
        let (interior, direction) = if container == root {
            (
                Box2::new(0.0, 0.0, solved.bounds.width, solved.bounds.height),
                solved.direction,
            )
        } else {
            let placed = &solved.groups[container];
            let padding = input.graph.groups[container].padding;
            let outer = rect_bounds(placed.rect);
            let top = placed
                .title
                .as_ref()
                .map_or(outer.y0 + padding.y0, |title| {
                    (title.y + title.height / 2.0 + padding.y1).max(outer.y0 + padding.y0)
                });
            (
                Box2::new(
                    outer.x0 + padding.x0,
                    top,
                    outer.x1 - padding.x1,
                    outer.y1 - padding.y1,
                ),
                placed.direction,
            )
        };
        let horizontal = matches!(direction, Direction::Left | Direction::Right);
        let (near_face, far_face) = if horizontal {
            (Side::Left, Side::Right)
        } else {
            (Side::Top, Side::Bottom)
        };
        siblings.clear();
        siblings.extend(level.children.iter().map(|&child| {
            let rect = match child {
                Endpoint::Node(node) => solved.nodes[node as usize],
                Endpoint::Group(group) => solved.groups[group as usize].rect,
            };
            (child, rect_bounds(rect))
        }));
        for &(child, rect) in &siblings {
            let Endpoint::Group(group) = child else {
                continue;
            };
            if !is_region(group) {
                continue;
            }
            let group = group as usize;
            let [start, end] = span(rect, horizontal);
            let [mut near, mut far] = span(interior, horizontal);
            near = if attached[group][near_face as usize] {
                start
            } else {
                near.min(start)
            };
            far = if attached[group][far_face as usize] {
                end
            } else {
                far.max(end)
            };
            if near == start && far == end {
                continue;
            }
            work.charge_adapter(work.checked_add(siblings.len(), level.routes.len())?)?;
            for &(other, bounds) in &siblings {
                if other != child && across_overlap(rect, bounds, horizontal) {
                    let [before, after] = span(bounds, horizontal);
                    if before < start {
                        near = near.max(start.midpoint(after.min(start)));
                    }
                    if after > end {
                        far = far.min(end.midpoint(before.max(end)));
                    }
                }
            }
            for &edge in &level.routes {
                let bounds = route_boxes[edge];
                if across_overlap(rect, bounds, horizontal) {
                    let [before, after] = span(bounds, horizontal);
                    if before < start {
                        near = near.max(after.min(start));
                    }
                    if after > end {
                        far = far.min(before.max(end));
                    }
                }
            }
            let placed = &mut solved.groups[group].rect;
            if horizontal {
                placed.x = near;
                placed.width = far - near;
            } else {
                placed.y = near;
                placed.height = far - near;
            }
        }
    }
    Ok(())
}

fn rect_bounds(rect: Rect) -> Box2 {
    Box2::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height)
}

fn route_bounds(edge: &SolvedEdge) -> Box2 {
    let Some(first) = edge.points.first() else {
        return Box2::ZERO;
    };
    let mut bounds = Box2::new(first.x, first.y, first.x, first.y);
    for point in &edge.points[1..] {
        let (x, y) = (point.x, point.y);
        bounds = Box2::new(
            bounds.x0.min(x),
            bounds.y0.min(y),
            bounds.x1.max(x),
            bounds.y1.max(y),
        );
    }
    if let Some(label) = &edge.label {
        bounds = bounds.union(Box2::from_center_size(
            (label.x, label.y),
            (label.width, label.height),
        ));
    }
    bounds
}

fn span(bounds: Box2, horizontal: bool) -> [f64; 2] {
    if horizontal {
        [bounds.x0, bounds.x1]
    } else {
        [bounds.y0, bounds.y1]
    }
}

fn across_overlap(first: Box2, second: Box2, horizontal: bool) -> bool {
    let [first_min, first_max] = span(first, !horizontal);
    let [second_min, second_max] = span(second, !horizontal);
    first_min < second_max && second_min < first_max
}
