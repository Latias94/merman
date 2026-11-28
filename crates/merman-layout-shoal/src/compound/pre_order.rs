use super::hierarchy::Hierarchy;
use super::sides::flow_sides;
use crate::flat::{self, Expanded};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{Endpoint, Graph, GroupId, LayoutError, Options, Side, WorkError};

#[derive(Debug)]
pub(super) struct Hints {
    pub(super) items: Vec<Option<f64>>,
    pub(super) ports: Vec<Option<f64>>,
    pub(super) flat_crossings: u64,
}

#[derive(Clone, Copy, Default)]
struct Leaves {
    count: usize,
    mean: f64,
    first_rank: u32,
    last_rank: u32,
}

impl Leaves {
    fn include(&mut self, other: Self) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = other;
            return;
        }
        let total = self.count + other.count;
        let fraction = other.count as f64 / total as f64;
        self.mean = if self.mean.is_sign_positive() == other.mean.is_sign_positive() {
            self.mean + (other.mean - self.mean) * fraction
        } else {
            self.mean * (1.0 - fraction) + other.mean * fraction
        };
        self.first_rank = self.first_rank.min(other.first_rank);
        self.last_rank = self.last_rank.max(other.last_rank);
        self.count = total;
    }
}

fn flow_hint(
    chain: &[u32],
    ranks: &[u32],
    across: &[f64],
    boundary_rank: u32,
    first_face: bool,
    near: usize,
    work: &mut Work<'_>,
) -> LayoutResult<Option<f64>> {
    work.visit(chain.len().max(1).ilog2() as usize + 2)?;
    let at = chain.partition_point(|&vertex| ranks[vertex as usize] < boundary_rank);
    let on_face = chain
        .get(at)
        .filter(|&&v| ranks[v as usize] == boundary_rank);
    let inside = on_face.map_or(across[near], |&v| across[v as usize]);
    let (outside, outside_rank) = if first_face {
        (at.checked_sub(1), boundary_rank.checked_sub(1))
    } else {
        (
            Some(at + usize::from(on_face.is_some())),
            boundary_rank.checked_add(1),
        )
    };
    if let Some(&vertex) = outside.and_then(|index| chain.get(index))
        && Some(ranks[vertex as usize]) == outside_rank
    {
        let key = inside + across[vertex as usize] * 1e-6;
        if key.is_finite() {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

pub(super) fn ordered(
    graph: &Graph,
    hierarchy: &Hierarchy,
    options: &Options,
    sides: &[Side],
    expanded: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Hints>> {
    work.check()?;
    if graph.nodes.is_empty()
        || hierarchy.borders.len() < 2
        || hierarchy.borders.len() > options.caps.max_ordered_compound_ports
        || graph.edges.len() > options.caps.max_ordered_compound_edges
    {
        return Ok(None);
    }
    if sides.len() != hierarchy.borders.len() {
        return Err(LayoutError::InvalidInput("missing compound border sides"));
    }
    for level in &hierarchy.levels {
        work.visit(1)?;
        if level.direction != graph.direction && !level.ports.is_empty() {
            return Ok(None);
        }
    }
    let nodes = graph.nodes.len();
    let root = graph.groups.len();
    let items = nodes
        .checked_add(root)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(items)?;
    let mut groups = reserved(nodes)?;
    let mut parents = filled(root, None)?;
    work.charge(nodes)?;
    let mut proxy = Graph {
        direction: graph.direction,
        nodes: reserved(nodes)?,
        ..Default::default()
    };
    for &original in &graph.nodes {
        work.visit(1)?;
        groups.push(original.parent);
        let mut node = crate::NodeSpec {
            parent: None,
            ..original
        };
        let level = original.parent.map_or(root, |id| id.0 as usize);
        node.align_flow(hierarchy.levels[level].direction);
        proxy.nodes.push(node);
    }
    for (parent, level) in hierarchy.levels[..root].iter().enumerate() {
        work.visit(1)?;
        for &item in &level.items {
            work.visit(1)?;
            if let Endpoint::Group(id) = item {
                parents[id.0 as usize] = Some(GroupId(parent as u32));
            }
        }
    }
    work.charge(
        graph
            .edges
            .len()
            .checked_mul(2)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    proxy.edges = reserved(graph.edges.len())?;
    let mut proxy_edge = reserved(graph.edges.len())?;
    let shares_direction = |node: crate::NodeId| {
        let level = groups[node.0 as usize].map_or(root, |id| id.0 as usize);
        hierarchy.levels[level].direction == graph.direction
    };
    for &edge in &graph.edges {
        work.visit(1)?;
        if matches!((edge.source, edge.target), (Endpoint::Node(from), Endpoint::Node(to))
            if from != to && shares_direction(from) && shares_direction(to))
        {
            proxy_edge.push(Some(proxy.edges.len()));
            proxy.edges.push(edge);
        } else {
            proxy_edge.push(None);
        }
    }
    let (across, flat_crossings) =
        flat::compound_order(&proxy, &groups, &parents, options, expanded, work)?;
    let ranks = expanded.order_ranks();
    let chains = expanded.order_chains();
    work.charge(items)?;
    let mut keys = filled(items, None)?;
    work.charge(hierarchy.levels.len())?;
    let mut leaves = filled(hierarchy.levels.len(), Leaves::default())?;
    for &level in hierarchy.order.iter().rev() {
        work.visit(1)?;
        let mut group = Leaves::default();
        for &item in &hierarchy.levels[level].items {
            work.visit(1)?;
            let child = match item {
                Endpoint::Node(id) => {
                    let index = id.0 as usize;
                    let mean = across[index];
                    if !mean.is_finite() {
                        return Err(LayoutError::InvalidInput(
                            "non-finite leaf order coordinate",
                        ));
                    }
                    if hierarchy.levels[level].direction == graph.direction {
                        keys[index] = Some(mean);
                    }
                    Leaves {
                        count: 1,
                        mean,
                        first_rank: ranks[index],
                        last_rank: ranks[index],
                    }
                }
                Endpoint::Group(id) => leaves[id.0 as usize],
            };
            group.include(child);
        }
        leaves[level] = group;
        if level != root && group.count != 0 {
            keys[nodes + level] = Some(group.mean);
        }
    }
    work.charge(hierarchy.borders.len())?;
    let mut ports = reserved(hierarchy.borders.len())?;
    for (border, &side) in hierarchy.borders.iter().zip(sides) {
        work.visit(1)?;
        let edge = graph.edges[border.edge];
        let (near, far) = if border.outgoing {
            (edge.source, edge.target)
        } else {
            (edge.target, edge.source)
        };
        let peer = if far == Endpoint::Group(GroupId(border.level as u32)) {
            near
        } else {
            far
        };
        let mut hint = keys[super::item_index(graph, peer)];
        let (first, last) = flow_sides(hierarchy.levels[border.level].direction);
        if (side == first || side == last)
            && let (Some(index), Endpoint::Node(near)) = (proxy_edge[border.edge], near)
        {
            let first_face = side == first;
            let group = leaves[border.level];
            hint = flow_hint(
                &chains[index],
                ranks,
                &across,
                if first_face {
                    group.first_rank
                } else {
                    group.last_rank
                },
                first_face,
                near.0 as usize,
                work,
            )?
            .or(hint);
        }
        ports.push(hint);
    }
    work.finish()?;
    Ok(Some(Hints {
        items: keys,
        ports,
        flat_crossings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compound::hierarchy::build;
    use crate::compound::sides;
    use crate::{
        Axis, Direction, EdgeSpec, GroupSpec, NodeId, NodeSpec, NoopWorkControl, PortSideHint,
        Size, WorkControl,
    };

    fn fan() -> Graph {
        let mut graph = Graph {
            groups: (0..5).map(GroupSpec::new).collect(),
            ..Default::default()
        };
        graph.groups[1].parent = Some(GroupId(0));
        graph.groups[2].parent = Some(GroupId(0));
        for (index, group) in [
            Some(1),
            Some(3),
            Some(2),
            Some(1),
            Some(3),
            Some(2),
            None,
            None,
            Some(1),
        ]
        .into_iter()
        .enumerate()
        {
            let size = Size {
                width: 24.0 + index as f64,
                height: 18.0,
            };
            let mut node = NodeSpec::new(size, index as u32);
            node.parent = group.map(GroupId);
            graph.nodes.push(node);
        }
        graph
            .edges
            .push(EdgeSpec::new(GroupId(0).into(), NodeId(0).into()));
        graph
            .edges
            .push(EdgeSpec::new(NodeId(0).into(), NodeId(0).into()));
        for node in 0..6 {
            graph
                .edges
                .push(EdgeSpec::new(NodeId(6).into(), NodeId(node).into()));
            graph
                .edges
                .push(EdgeSpec::new(NodeId(node).into(), NodeId(7).into()));
        }
        graph
    }

    fn hints(graph: &Graph, options: &Options, expanded: &mut Expanded) -> Option<Hints> {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let h = build(graph, options, &mut work).unwrap();
        let sides = sides::assign(graph, &h, options, &mut work).unwrap();
        ordered(graph, &h, options, &sides, expanded, &mut work).unwrap()
    }

    #[test]
    fn all_leaves_contribute_to_weighted_group_means_and_nested_blocks_stay_contiguous() {
        for direction in [Direction::Down, Direction::Right] {
            let mut graph = fan();
            graph.direction = direction;
            let mut expanded = Expanded::default();
            let hints = hints(&graph, &Options::default(), &mut expanded).unwrap();
            let ranks = expanded.order_ranks();
            assert!(ranks[..6].iter().all(|&rank| rank == ranks[0]));
            assert!(hints.items[..graph.nodes.len()].iter().all(Option::is_some));
            let key = |id: usize| hints.items[id].unwrap();
            let mut row: Vec<_> = (0..6).collect();
            row.sort_by(|&left, &right| key(left).total_cmp(&key(right)));
            for members in [&[0, 2, 3, 5][..], &[0, 3], &[2, 5], &[1, 4]] {
                let places: Vec<_> = row
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| members.contains(id))
                    .map(|(at, _)| at)
                    .collect();
                assert_eq!(
                    places.last().unwrap() - places.first().unwrap() + 1,
                    members.len(),
                    "{row:?}"
                );
            }
            for (group, members) in [&[0, 2, 3, 5, 8][..], &[0, 3, 8], &[2, 5], &[1, 4]]
                .into_iter()
                .enumerate()
            {
                let expected = members.iter().copied().map(key).sum::<f64>() / members.len() as f64;
                assert!((key(graph.nodes.len() + group) - expected).abs() < 1e-8);
            }
            assert_eq!(hints.items[graph.nodes.len() + 4], None);
        }
    }

    #[test]
    fn order_proxy_uses_the_repaired_forest() {
        let mut graph = fan();
        let options = Options::default();
        let mut expanded = Expanded::default();
        let expected = hints(&graph, &options, &mut expanded).unwrap();
        graph.groups[0].parent = Some(GroupId(1));
        let actual = hints(&graph, &options, &mut expanded).unwrap();
        assert_eq!(actual.items, expected.items);
        assert_eq!(actual.ports, expected.ports);
        assert_eq!(actual.flat_crossings, expected.flat_crossings);
        assert_eq!(graph.groups[0].parent, Some(GroupId(1)));
    }

    #[test]
    fn an_isolated_rotated_leaf_does_not_disable_same_axis_port_ordering() {
        let mut graph = fan();
        let isolated = GroupId(4);
        graph.groups[isolated.0 as usize].direction = Some(Direction::Right);
        let first = graph.nodes.len();
        for seq in first..first + 2 {
            let mut node = NodeSpec::new(
                Size {
                    width: 20.0,
                    height: 100.0,
                },
                seq as u32,
            );
            node.parent = Some(isolated);
            node.flow_relative_axis = Some(Axis::Horizontal);
            graph.nodes.push(node);
        }
        let mut local_edge =
            EdgeSpec::new(NodeId(first as u32).into(), NodeId(first as u32 + 1).into());
        local_edge.source_port = PortSideHint::FixedOffset {
            side: Side::Right,
            offset: 20.0,
        };
        graph.edges.push(local_edge);
        let mut options = Options::default();
        options.group_policy.inherit_crossed_direction = false;
        let mut expanded = Expanded::default();
        let ordered = hints(&graph, &options, &mut expanded).unwrap();
        assert!(ordered.items[..first].iter().all(Option::is_some));
        assert_eq!(ordered.items[first..first + 2], [None, None]);
        assert!(ordered.ports.iter().all(Option::is_some));
        let layout = crate::layout(&graph, &options).unwrap();
        let source = layout.nodes[first];
        let contact = layout.edges.last().unwrap().points[0];
        assert!((contact.x - source.x - source.width).abs() < 1e-9);
        assert!((contact.y - source.center().y - 20.0).abs() < 1e-9);

        graph
            .edges
            .push(EdgeSpec::new(NodeId(6).into(), NodeId(first as u32).into()));
        assert!(hints(&graph, &options, &mut expanded).is_none());
    }

    #[test]
    fn flow_face_hints_use_each_proxy_edge_at_the_groups_boundary_rank() {
        let mut graph = Graph {
            groups: vec![GroupSpec::new(0)],
            ..Default::default()
        };
        for id in 0..4 {
            let mut node = NodeSpec::new(
                Size {
                    width: 50.0,
                    height: 24.0,
                },
                id,
            );
            node.parent = (id < 2).then_some(GroupId(0));
            graph.nodes.push(node);
        }
        graph
            .edges
            .push(EdgeSpec::new(GroupId(0).into(), NodeId(0).into()));
        graph
            .edges
            .push(EdgeSpec::new(NodeId(1).into(), NodeId(1).into()));
        for (from, to) in [(0, 1), (1, 2), (2, 3), (0, 3), (0, 3)] {
            graph
                .edges
                .push(EdgeSpec::new(NodeId(from).into(), NodeId(to).into()));
        }
        let options = Options::default();
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let h = build(&graph, &options, &mut work).unwrap();
        let mut sides = sides::assign(&graph, &h, &options, &mut work).unwrap();
        let mut expanded = Expanded::default();
        let result = ordered(&graph, &h, &options, &sides, &mut expanded, &mut work)
            .unwrap()
            .unwrap();
        let groups: Vec<_> = graph.nodes.iter().map(|node| node.parent).collect();
        let mut proxy = graph.clone();
        for node in &mut proxy.nodes {
            node.parent = None;
        }
        proxy.groups.clear();
        proxy.edges.drain(..2);
        let (across, crossings) =
            flat::compound_order(&proxy, &groups, &[None], &options, &mut expanded, &mut work)
                .unwrap();
        assert_eq!(crossings, result.flat_crossings);
        let ranks = expanded.order_ranks();
        let last = ranks[0].max(ranks[1]);
        let mut distinct = Vec::new();
        for edge in [5, 6] {
            let chain = &expanded.order_chains()[edge - 2];
            let vertex_at = |rank| {
                chain
                    .iter()
                    .map(|&id| id as usize)
                    .find(|&id| ranks[id] == rank)
                    .unwrap()
            };
            let inside = vertex_at(last);
            assert!(inside >= graph.nodes.len());
            let expected = across[inside] + across[vertex_at(last + 1)] * 1e-6;
            let border = h
                .borders
                .iter()
                .position(|border| border.edge == edge && border.outgoing)
                .unwrap();
            assert_eq!(result.ports[border], Some(expected));
            distinct.push(expected);
        }
        assert_ne!(distinct[0], distinct[1]);
        let border = h
            .borders
            .iter()
            .position(|border| border.edge == 6 && border.outgoing)
            .unwrap();
        assert_eq!(sides[border], Side::Bottom);
        sides[border] = Side::Top;
        let changed = ordered(&graph, &h, &options, &sides, &mut expanded, &mut work)
            .unwrap()
            .unwrap();
        assert_eq!(changed.ports[border], result.items[3]);
        assert_ne!(changed.ports[border], result.ports[border]);
    }

    #[test]
    fn incoming_parallel_chains_use_the_first_rank_and_the_preceding_rank() {
        let mut graph = Graph {
            groups: vec![GroupSpec::new(0)],
            ..Default::default()
        };
        for index in 0..4 {
            let mut node = NodeSpec::new(
                Size {
                    width: 50.,
                    height: 24.,
                },
                index,
            );
            node.parent = matches!(index, 1 | 2).then_some(GroupId(0));
            graph.nodes.push(node);
        }
        for (source, target) in [(0, 1), (1, 2), (2, 3), (0, 2), (0, 2)] {
            graph
                .edges
                .push(EdgeSpec::new(NodeId(source).into(), NodeId(target).into()));
        }
        let options = Options::default();
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let hierarchy = build(&graph, &options, &mut work).unwrap();
        let sides = sides::assign(&graph, &hierarchy, &options, &mut work).unwrap();
        let mut expanded = Expanded::default();
        let hints = ordered(
            &graph,
            &hierarchy,
            &options,
            &sides,
            &mut expanded,
            &mut work,
        )
        .unwrap()
        .unwrap();
        let groups: Vec<_> = graph.nodes.iter().map(|node| node.parent).collect();
        let mut proxy = graph.clone();
        proxy.groups.clear();
        for node in &mut proxy.nodes {
            node.parent = None;
        }
        let (across, _) =
            flat::compound_order(&proxy, &groups, &[None], &options, &mut expanded, &mut work)
                .unwrap();
        let ranks = expanded.order_ranks();
        let first = ranks[1];
        assert_eq!(ranks[0] + 1, first);
        let mut keys = Vec::new();
        for edge in [3, 4] {
            let (id, _) = hierarchy
                .borders
                .iter()
                .enumerate()
                .find(|(_, port)| port.edge == edge)
                .unwrap();
            assert_eq!(sides[id], Side::Top);
            let chain = &expanded.order_chains()[edge];
            let inner = chain
                .iter()
                .map(|&vertex| vertex as usize)
                .find(|&vertex| ranks[vertex] == first)
                .unwrap();
            assert!(inner >= graph.nodes.len());
            let key = across[inner] + across[0] * 1e-6;
            assert_eq!(hints.ports[id], Some(key));
            keys.push(key);
        }
        assert_ne!(keys[0], keys[1]);
    }

    #[test]
    fn exact_caps_admit_the_pass_and_mixed_direction_uses_independent_levels() {
        let mut graph = fan();
        let mut opts = Options::default();
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let h = build(&graph, &opts, &mut work).unwrap();
        let sides = sides::assign(&graph, &h, &opts, &mut work).unwrap();
        let mut expanded = Expanded::default();
        opts.caps.max_ordered_compound_ports = h.borders.len();
        opts.caps.max_ordered_compound_edges = graph.edges.len();
        assert!(
            ordered(&graph, &h, &opts, &sides, &mut expanded, &mut work)
                .unwrap()
                .is_some()
        );
        opts.caps.max_ordered_compound_edges -= 1;
        assert!(
            ordered(&graph, &h, &opts, &sides, &mut expanded, &mut work)
                .unwrap()
                .is_none()
        );
        opts.caps.max_ordered_compound_edges += 1;
        opts.caps.max_ordered_compound_ports -= 1;
        assert!(
            ordered(&graph, &h, &opts, &sides, &mut expanded, &mut work)
                .unwrap()
                .is_none()
        );
        opts.caps.max_ordered_compound_ports += 1;
        opts.group_policy.inherit_crossed_direction = false;
        graph.groups[1].direction = Some(Direction::Left);
        assert!(hints(&graph, &opts, &mut expanded).is_none());
        graph.edges.truncate(1);
        graph.groups[1].direction = None;
        graph.edges[0].source = GroupId(1).into();
        assert!(hints(&graph, &opts, &mut expanded).is_none());
    }

    #[test]
    fn failed_ordering_can_retry_with_the_same_expansion_storage() {
        #[derive(Default)]
        struct Gate {
            checks: usize,
            stop: Option<usize>,
        }
        impl WorkControl for Gate {
            fn charge(&mut self, _: usize) -> Result<(), WorkError> {
                Ok(())
            }
            fn check(&mut self) -> Result<(), WorkError> {
                self.checks += 1;
                if self.stop == Some(self.checks) {
                    Err(WorkError::Interrupted)
                } else {
                    Ok(())
                }
            }
        }
        let graph = fan();
        let opts = Options::default();
        let h = build(&graph, &opts, &mut Work::new(&mut NoopWorkControl)).unwrap();
        let sides = sides::assign(&graph, &h, &opts, &mut Work::new(&mut NoopWorkControl)).unwrap();
        let mut expanded = Expanded::default();
        let mut meter = Gate::default();
        let expected = ordered(
            &graph,
            &h,
            &opts,
            &sides,
            &mut expanded,
            &mut Work::new(&mut meter),
        )
        .unwrap()
        .unwrap();
        for stop in [1, meter.checks / 2, meter.checks] {
            let mut gate = Gate {
                checks: 0,
                stop: Some(stop),
            };
            assert!(matches!(
                ordered(
                    &graph,
                    &h,
                    &opts,
                    &sides,
                    &mut expanded,
                    &mut Work::new(&mut gate)
                ),
                Err(LayoutError::Work(WorkError::Interrupted))
            ));
            let retry = ordered(
                &graph,
                &h,
                &opts,
                &sides,
                &mut expanded,
                &mut Work::new(&mut NoopWorkControl),
            )
            .unwrap()
            .unwrap();
            assert_eq!(retry.items, expected.items);
            assert_eq!(retry.ports, expected.ports);
            assert_eq!(retry.flat_crossings, expected.flat_crossings);
        }
    }
}
