//! Width trials share a work meter and reuse the same component scratch.

use crate::compound::{self, Topology};
use crate::flat::{self, Expanded};
use crate::validate;
use crate::work::{LayoutResult, Work, reserved};
use crate::{
    Axis, Direction, Endpoint, Graph, GroupId, Insets, LayoutDetail, LayoutError, LayoutOutput,
    Options, PackingPolicy, PortSideHint, RoutingStyle, Side, TitlePosition, WidthPolicy,
};

#[derive(Clone, Copy)]
pub(crate) struct WidthTrial {
    options: Options,
    turned: bool,
    tight_groups: bool,
    fold_at: Option<f64>,
}

impl WidthTrial {
    pub(crate) fn replay(
        self,
        graph: &mut Graph,
        expanded: &mut Expanded,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<LayoutOutput>> {
        if self.turned {
            turn_down(graph, &self.options, work)?;
        }
        if self.tight_groups {
            tighten_groups(graph, work)?;
        }
        if let Some(max_width) = self.fold_at {
            flat::fold(
                graph,
                &self.options,
                expanded,
                max_width,
                LayoutDetail::Full,
                work,
            )
        } else {
            let mut output =
                crate::solve(graph, &self.options, expanded, LayoutDetail::Full, work)?;
            crate::compound::relax_seams(graph, &self.options, &mut output, work)?;
            Ok(Some(output))
        }
    }

    pub(crate) fn refine_lanes(
        self,
        graph: &Graph,
        drawing: &mut LayoutOutput,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        if self.turned
            && self.options.routing == RoutingStyle::Orthogonal
            && self.options.global_orthogonal
        {
            let mut turned = copy_graph(graph, work)?;
            turn_down(&mut turned, &self.options, work)?;
            crate::orthogonal::refine(&turned, drawing, &self.options, work)
        } else {
            crate::orthogonal::refine(graph, drawing, &self.options, work)
        }
    }
}

type WidthSolution = (LayoutOutput, WidthTrial);

pub(crate) fn run(
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<WidthSolution> {
    let (mut drawing, trial) = candidates(graph, options, expanded, detail, work)?;
    // Seam edits stay inside the participating groups and their connecting corridor.
    // Trial-specific geometry is in the drawing. Endpoint ownership and label and marker sizes
    // come from the original graph and are shared by all trials.
    crate::compound::relax_seams(graph, &trial.options, &mut drawing, work)?;
    Ok((drawing, trial))
}

fn candidates(
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<WidthSolution> {
    let WidthPolicy::FitWidth {
        max_width,
        allow_turn,
        turn_breakpoint,
        fold_wide_ranks,
    } = options.width_policy
    else {
        unreachable!()
    };
    validate::options(options)?;
    let mut wrapped = *options;
    wrapped.packing = PackingPolicy::FitWidth { max_width };
    let original = WidthTrial {
        options: wrapped,
        turned: false,
        tight_groups: false,
        fold_at: None,
    };
    let prepare = |graph: &Graph, options: &Options, work: &mut Work<'_>| {
        (!graph.groups.is_empty())
            .then(|| Topology::new(graph, options, work))
            .transpose()
    };
    let solve = |graph: &Graph,
                 options: &Options,
                 topology: Option<&Topology>,
                 expanded: &mut Expanded,
                 work: &mut Work<'_>| {
        if let Some(topology) = topology {
            compound::run_with_topology(graph, options, topology, expanded, detail, work)
        } else {
            crate::solve(graph, options, expanded, detail, work)
        }
    };
    let topology = prepare(graph, &wrapped, work)?;
    let mut best = (
        solve(graph, &wrapped, topology.as_ref(), expanded, work)?,
        original,
    );
    if best.0.bounds.width <= max_width {
        return Ok(best);
    }

    let mut tight = wrapped;
    tight.spacing.node *= 0.5;
    tight.spacing.rank *= 0.75;
    let compact = if graph.groups.is_empty() {
        solve(graph, &tight, None, expanded, work)
    } else {
        let mut compact = copy_graph(graph, work)?;
        tighten_groups(&mut compact, work)?;
        // Tightening changes only padding and spacing. Topology owns the validated
        // hierarchy and semantic port sides, which are the same for both widths.
        solve(&compact, &tight, topology.as_ref(), expanded, work)
    };
    let compact_trial = WidthTrial {
        options: tight,
        tight_groups: true,
        ..original
    };
    let compact_kept = try_narrower(&mut best, compact, compact_trial)?;
    if best.0.bounds.width <= max_width {
        return Ok(best);
    }

    if allow_turn
        && max_width < turn_breakpoint
        && matches!(graph.direction, Direction::Right | Direction::Left)
    {
        drop(topology);
        let mut turned = copy_graph(graph, work)?;
        turn_down(&mut turned, &wrapped, work)?;
        wrapped.group_policy.inherit_crossed_direction = false;
        tight.group_policy.inherit_crossed_direction = false;
        let topology = match prepare(&turned, &wrapped, work) {
            Ok(topology) => topology,
            // Tightening only decreases already-valid padding and spacing. It cannot repair
            // an invalid transformed hierarchy or port, and compound graphs cannot fold ranks.
            Err(LayoutError::InvalidInput(_)) => return Ok(best),
            Err(error) => return Err(error),
        };
        let turn_trial = WidthTrial {
            options: wrapped,
            turned: true,
            ..original
        };
        let turned_kept = try_narrower(
            &mut best,
            solve(&turned, &wrapped, topology.as_ref(), expanded, work),
            turn_trial,
        )?;
        if best.0.bounds.width <= max_width {
            return Ok(best);
        }
        tighten_groups(&mut turned, work)?;
        let tight_turn_trial = WidthTrial {
            options: tight,
            turned: true,
            ..compact_trial
        };
        let tight_turn_kept = try_narrower(
            &mut best,
            solve(&turned, &tight, topology.as_ref(), expanded, work),
            tight_turn_trial,
        )?;
        if fold_wide_ranks && best.0.bounds.width > max_width {
            let trial = if turned_kept && !tight_turn_kept {
                turn_trial
            } else {
                tight_turn_trial
            };
            try_fold(
                &mut best,
                &turned,
                WidthTrial {
                    tight_groups: true,
                    ..trial
                },
                expanded,
                max_width,
                detail,
                work,
            )?;
        }
    } else if fold_wide_ranks {
        let trial = if compact_kept {
            WidthTrial {
                tight_groups: false,
                ..compact_trial
            }
        } else {
            original
        };
        try_fold(&mut best, graph, trial, expanded, max_width, detail, work)?;
    }
    Ok(best)
}

fn try_fold(
    best: &mut WidthSolution,
    graph: &Graph,
    trial: WidthTrial,
    expanded: &mut Expanded,
    max_width: f64,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    match flat::fold(
        graph,
        &trial.options,
        expanded,
        max_width,
        detail.flat(&trial.options),
        work,
    ) {
        Ok(Some(candidate)) if candidate.bounds.width < best.0.bounds.width => {
            *best = (
                candidate,
                WidthTrial {
                    fold_at: Some(max_width),
                    ..trial
                },
            );
        }
        Ok(_) | Err(LayoutError::InvalidInput(_)) => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn try_narrower(
    best: &mut WidthSolution,
    candidate: LayoutResult<LayoutOutput>,
    trial: WidthTrial,
) -> LayoutResult<bool> {
    match candidate {
        Ok(drawing) if drawing.bounds.width < best.0.bounds.width => {
            *best = (drawing, trial);
            Ok(true)
        }
        Ok(_) | Err(LayoutError::InvalidInput(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn copy_graph(graph: &Graph, work: &mut Work<'_>) -> LayoutResult<Graph> {
    fn copy<T: Copy>(items: &[T], work: &mut Work<'_>) -> LayoutResult<Vec<T>> {
        work.charge(items.len())?;
        let mut result = reserved(items.len())?;
        for chunk in items.chunks(128) {
            work.check()?;
            result.extend_from_slice(chunk);
        }
        Ok(result)
    }
    Ok(Graph {
        direction: graph.direction,
        nodes: copy(&graph.nodes, work)?,
        groups: copy(&graph.groups, work)?,
        edges: copy(&graph.edges, work)?,
        lane_order: copy(&graph.lane_order, work)?,
    })
}

fn turn_down(graph: &mut Graph, options: &Options, work: &mut Work<'_>) -> LayoutResult<()> {
    let original = graph.direction;
    let flat = graph.groups.is_empty();
    let lanes = !graph.lane_order.is_empty();
    let lane_shell = |id: GroupId| lanes && graph.groups[id.0 as usize].parent.is_none();
    let levels = if flat {
        None
    } else {
        Some(crate::compound::authored_directions(
            &graph.groups,
            original,
            work,
        )?)
    };
    let node_axis = |node: &crate::NodeSpec| {
        if node.parent.is_some_and(lane_shell) {
            original.axis()
        } else {
            levels
                .as_ref()
                .map_or(original.axis(), |levels| levels.axis(node.parent))
        }
    };
    graph.direction = Direction::Down;
    for node in &mut graph.nodes {
        work.visit(1)?;
        let primary = node.port_span;
        node.port_span = node.perpendicular_port_span.or(primary);
        node.perpendicular_port_span = primary.or_else(|| {
            // A vertical level already reads the perpendicular span as its primary span.
            // After the root turns, keep its secondary span inheriting that same value.
            node.port_span
                .filter(|_| node_axis(node) == Axis::Horizontal)
                .map(|_| (node.size.height - 2.0 * options.spacing.corner_clearance).max(0.0))
        });
        if flat {
            node.align_flow(graph.direction);
        }
    }
    let turn_port = |endpoint, opposite, hint, work: &mut Work<'_>| -> LayoutResult<_> {
        if matches!(endpoint, Endpoint::Group(id) if lane_shell(id)) {
            return Ok(flat::to_canonical_port(original, hint));
        }
        if !matches!(
            hint,
            PortSideHint::Side(Side::Left | Side::Right)
                | PortSideHint::FixedOffset {
                    side: Side::Left | Side::Right,
                    ..
                }
        ) {
            return Ok(hint);
        }
        let (node, axis) = match endpoint {
            Endpoint::Node(id) => {
                let node = graph.nodes[id.0 as usize];
                (Some(node), node_axis(&node))
            }
            Endpoint::Group(id) => {
                let levels = levels.as_ref().ok_or(LayoutError::InvalidInput(
                    "group port is outside the group array",
                ))?;
                (
                    None,
                    levels.group_port_axis(id, opposite, &graph.nodes, work)?,
                )
            }
        };
        if axis == Axis::Vertical {
            return Ok(hint);
        }
        // The root turn is shared by horizontal scopes. An opposed scope becomes Up.
        let turned = flat::to_canonical_port(original, hint);
        if let (Some(mut node), PortSideHint::FixedOffset { side, .. }) = (node, turned) {
            let measured = node.size;
            node.align_flow(Direction::Down);
            if validate::port(measured, turned).is_err()
                || validate::port(node.size, turned).is_err()
            {
                return Ok(PortSideHint::Side(side));
            }
        }
        Ok(turned)
    };
    for edge in &mut graph.edges {
        work.visit(1)?;
        edge.source_port = turn_port(edge.source, edge.target, edge.source_port, work)?;
        edge.target_port = turn_port(edge.target, edge.source, edge.target_port, work)?;
    }
    for group in &mut graph.groups {
        work.visit(1)?;
        if lanes && group.parent.is_none() {
            if let Some(title) = &mut group.title {
                std::mem::swap(&mut title.width, &mut title.height);
            }
            let Insets {
                top,
                right,
                bottom,
                left,
            } = group.padding;
            let (before, after) = if original == Direction::Right {
                (left, right)
            } else {
                (right, left)
            };
            group.padding = Insets {
                top: before,
                right: bottom,
                bottom: after,
                left: top,
            };
            group.direction = Some(Direction::Down);
            continue;
        }
        if let Some(direction) = group.direction
            && direction.axis() == Axis::Horizontal
        {
            group.direction = Some(if direction == original {
                Direction::Down
            } else {
                Direction::Up
            });
        }
    }
    Ok(())
}

fn tighten_groups(graph: &mut Graph, work: &mut Work<'_>) -> LayoutResult<()> {
    let lanes = !graph.lane_order.is_empty();
    for group in &mut graph.groups {
        work.visit(1)?;
        if lanes && group.parent.is_none() {
            // Preserve the body gap after the lane header on the entry face.
            let Insets {
                top,
                right,
                bottom,
                left,
            } = &mut group.padding;
            for (entry, inset) in [
                (Direction::Down, top),
                (Direction::Left, right),
                (Direction::Up, bottom),
                (Direction::Right, left),
            ] {
                if entry != graph.direction {
                    *inset *= 0.6;
                }
            }
            continue;
        }
        let title_height = group.title.map_or(0.0, |title| title.height);
        let [top, bottom] = match group.title_position {
            TitlePosition::TopAuto
            | TitlePosition::TopLeft
            | TitlePosition::TopCenter
            | TitlePosition::TopRight => [title_height, 0.0],
            _ => [0.0, title_height],
        };
        let tighten = |padding: f64, title: f64| {
            let band = padding.min(title);
            band + 0.6 * (padding - band)
        };
        group.padding.top = tighten(group.padding.top, top);
        group.padding.bottom = tighten(group.padding.bottom, bottom);
        group.padding.left *= 0.6;
        group.padding.right *= 0.6;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeSpec, GroupSpec, NodeId, NodeSpec, Size};

    #[test]
    fn a_horizontal_width_trial_remaps_flow_ports_and_matching_subgroups() {
        for (direction, source, target) in [
            (Direction::Right, Side::Right, Side::Left),
            (Direction::Left, Side::Left, Side::Right),
        ] {
            for grouped in [false, true] {
                let mut graph = Graph {
                    direction,
                    nodes: (0..4)
                        .map(|id| {
                            NodeSpec::new(
                                Size {
                                    width: 120.,
                                    height: 56.,
                                },
                                id,
                            )
                        })
                        .collect(),
                    edges: (0..3)
                        .map(|id| EdgeSpec::new(NodeId(id).into(), NodeId(id + 1).into()))
                        .collect(),
                    ..Default::default()
                };
                if grouped {
                    let mut group = GroupSpec::new(0);
                    group.direction = Some(direction);
                    graph.groups.push(group);
                    for node in &mut graph.nodes[..2] {
                        node.parent = Some(GroupId(0));
                    }
                }
                graph.edges[0].source_port = PortSideHint::FixedOffset {
                    side: source,
                    offset: 12.,
                };
                graph.edges[0].target_port = PortSideHint::FixedOffset {
                    side: target,
                    offset: -9.,
                };
                let options = Options {
                    width_policy: WidthPolicy::FitWidth {
                        max_width: 220.,
                        allow_turn: true,
                        turn_breakpoint: 300.,
                        fold_wide_ranks: false,
                    },
                    ..Default::default()
                };
                let drawing = crate::layout(&graph, &options).unwrap();
                assert_eq!(drawing.direction, Direction::Down);
                assert!(drawing.bounds.width <= 220.);
                if grouped {
                    assert_eq!(drawing.groups[0].effective_direction, Direction::Down);
                    assert!(!drawing.border_stitches.is_empty());
                }
                let route = &drawing.edges[0];
                assert_eq!(route.ports, (Side::Bottom, Side::Top));
                assert_eq!(route.points[0].x, drawing.nodes[0].center().x + 12.);
                assert_eq!(
                    route.points.last().unwrap().x,
                    drawing.nodes[1].center().x - 9.
                );
            }
        }
    }

    #[test]
    fn a_width_turn_preserves_physical_pins_in_authored_vertical_scopes() {
        let mut graph = Graph {
            direction: Direction::Left,
            groups: vec![GroupSpec::new(0), GroupSpec::new(1)],
            nodes: vec![NodeSpec::new(
                Size {
                    width: 40.,
                    height: 100.,
                },
                0,
            )],
            ..Default::default()
        };
        graph.groups[0].parent = Some(GroupId(1));
        graph.groups[1].direction = Some(Direction::Down);
        graph.nodes[0].parent = Some(GroupId(0));
        let pin = PortSideHint::FixedOffset {
            side: Side::Right,
            offset: 35.,
        };
        let mut edge = EdgeSpec::new(NodeId(0).into(), GroupId(0).into());
        edge.source_port = pin;
        edge.target_port = pin;
        graph.edges.push(edge);
        turn_down(
            &mut graph,
            &Options::default(),
            &mut Work::new(&mut crate::NoopWorkControl),
        )
        .unwrap();
        assert_eq!(graph.direction, Direction::Down);
        assert_eq!(graph.edges[0].source_port, pin);
        assert_eq!(graph.edges[0].target_port, pin);
    }

    #[test]
    fn a_width_turn_uses_the_level_that_routes_each_group_contact() {
        let mut graph = Graph {
            direction: Direction::Left,
            groups: (0..4).map(GroupSpec::new).collect(),
            nodes: (0..3)
                .map(|id| {
                    NodeSpec::new(
                        Size {
                            width: 60.,
                            height: 80.,
                        },
                        id,
                    )
                })
                .collect(),
            ..Default::default()
        };
        graph.groups[0].direction = Some(Direction::Down);
        graph.groups[1].parent = Some(GroupId(0));
        graph.groups[2].parent = Some(GroupId(1));
        graph.groups[2].direction = Some(Direction::Right);
        graph.groups[3].parent = Some(GroupId(2));
        graph.groups[3].direction = Some(Direction::Up);
        graph.nodes[0].parent = Some(GroupId(1));
        graph.nodes[2].parent = Some(GroupId(2));
        let group = |id| Endpoint::Group(GroupId(id));
        let node = |id| Endpoint::Node(NodeId(id));
        let pin = PortSideHint::FixedOffset {
            side: Side::Right,
            offset: 12.,
        };
        let turned = PortSideHint::FixedOffset {
            side: Side::Top,
            offset: 12.,
        };
        let cases = [
            (group(0), node(1), [turned, turned]),
            (node(1), group(0), [turned, turned]),
            (group(0), node(0), [pin, pin]),
            (node(0), group(0), [pin, pin]),
            (group(0), group(3), [pin, turned]),
            (group(2), group(1), [pin, pin]),
            (group(2), node(2), [turned, turned]),
            (group(0), group(0), [turned, turned]),
        ];
        graph.edges = cases
            .iter()
            .map(|&(from, to, _)| {
                let mut edge = EdgeSpec::new(from, to);
                edge.source_port = pin;
                edge.target_port = pin;
                edge
            })
            .collect();
        turn_down(
            &mut graph,
            &Options::default(),
            &mut Work::new(&mut crate::NoopWorkControl),
        )
        .unwrap();
        for (edge, (source, target, expected)) in graph.edges.iter().zip(cases) {
            assert_eq!(
                [edge.source_port, edge.target_port],
                expected,
                "{source:?} -> {target:?}",
            );
        }
    }

    #[test]
    fn a_width_turn_preserves_physical_face_spans_in_vertical_levels() {
        let size = Size {
            width: 60.,
            height: 80.,
        };
        let mut graph = Graph {
            direction: Direction::Right,
            groups: vec![GroupSpec::new(0), GroupSpec::new(1)],
            nodes: (0..4).map(|id| NodeSpec::new(size, id)).collect(),
            ..Default::default()
        };
        graph.groups[0].direction = Some(Direction::Up);
        graph.groups[1].parent = Some(GroupId(0));
        graph.nodes[1].parent = Some(GroupId(0));
        graph.nodes[2].parent = Some(GroupId(1));
        graph.nodes[3].parent = Some(GroupId(1));
        for node in &mut graph.nodes {
            node.perpendicular_port_span = Some(20.);
        }
        graph.nodes[3].port_span = Some(45.);
        let options = Options::default();
        turn_down(
            &mut graph,
            &options,
            &mut Work::new(&mut crate::NoopWorkControl),
        )
        .unwrap();
        let spans: Vec<_> = graph
            .nodes
            .iter()
            .map(|node| (node.port_span, node.perpendicular_port_span))
            .collect();
        let full_height = size.height - 2. * options.spacing.corner_clearance;
        assert_eq!(
            spans,
            [
                (Some(20.), Some(full_height)),
                (Some(20.), None),
                (Some(20.), None),
                (Some(20.), Some(45.))
            ]
        );
    }

    #[test]
    fn a_tall_upright_node_does_not_block_a_width_turn_with_its_old_face_offset() {
        let mut graph = Graph {
            direction: Direction::Right,
            nodes: (0..4)
                .map(|id| {
                    NodeSpec::new(
                        Size {
                            width: 40.,
                            height: 100.,
                        },
                        id,
                    )
                })
                .collect(),
            edges: (0..3)
                .map(|id| EdgeSpec::new(NodeId(id).into(), NodeId(id + 1).into()))
                .collect(),
            ..Default::default()
        };
        graph.edges[0].source_port = PortSideHint::FixedOffset {
            side: Side::Right,
            offset: 35.,
        };
        let options = Options {
            width_policy: WidthPolicy::FitWidth {
                max_width: 120.,
                allow_turn: true,
                turn_breakpoint: 200.,
                fold_wide_ranks: false,
            },
            ..Options::default()
        };
        let drawing = crate::layout(&graph, &options).unwrap();
        assert_eq!(drawing.direction, Direction::Down);
        assert!(drawing.bounds.width <= 120.);
        assert_eq!(drawing.edges[0].ports, (Side::Bottom, Side::Top));
    }
}
