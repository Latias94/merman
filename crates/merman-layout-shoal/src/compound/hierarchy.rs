use crate::flat;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Direction, Endpoint, Graph, GroupId, LayoutError, NodeId, Options, PortSideHint, WorkError,
};

#[derive(Debug)]
pub(super) struct Hierarchy {
    pub(super) levels: Vec<Level>,
    pub(super) order: Vec<usize>,
    pub(super) borders: Vec<Border>,
    pub(super) paths: Vec<Vec<(usize, usize)>>,
    pub(super) truncated: usize,
    pub(super) node_slots: Vec<usize>,
    pub(super) group_slots: Vec<usize>,
    pub(super) lanes: Option<LaneMembership>,
}

#[derive(Debug)]
pub(super) struct LaneMembership {
    // The lane ordinal of every group, including the shells themselves.
    groups: Vec<usize>,
}

impl LaneMembership {
    fn new(graph: &Graph, parents: &[usize], work: &mut Work<'_>) -> LayoutResult<Self> {
        const UNASSIGNED: usize = usize::MAX;
        let root = graph.groups.len();
        work.charge(root)?;
        let mut groups = filled(root, UNASSIGNED)?;
        for (lane, &GroupId(group)) in graph.lane_order.iter().enumerate() {
            work.visit(1)?;
            let group = group as usize;
            if graph
                .groups
                .get(group)
                .is_none_or(|spec| spec.parent.is_some())
                || groups[group] != UNASSIGNED
            {
                return Err(LayoutError::InvalidInput(
                    "swimlane order must name distinct root groups",
                ));
            }
            groups[group] = lane;
        }
        let mut path = Vec::new();
        for group in 0..root {
            work.visit(1)?;
            let mut current = group;
            while current != root && groups[current] == UNASSIGNED {
                work.visit(1)?;
                push(&mut path, current)?;
                current = parents[current];
            }
            if current == root {
                return Err(LayoutError::InvalidInput(
                    "every root group must be a swimlane",
                ));
            }
            let lane = groups[current];
            work.visit(path.len())?;
            for member in path.drain(..) {
                groups[member] = lane;
            }
        }
        Ok(Self { groups })
    }

    fn shell(&self, graph: &Graph, id: usize) -> Option<usize> {
        let lane = *self.groups.get(id)?;
        (graph.lane_order[lane].0 as usize == id).then_some(lane)
    }

    fn level(&self, graph: &Graph, parent: usize) -> usize {
        if self.shell(graph, parent).is_some() {
            graph.groups.len()
        } else {
            parent
        }
    }

    pub(super) fn item(&self, graph: &Graph, endpoint: Endpoint) -> usize {
        let group = match endpoint {
            Endpoint::Node(id) => graph.nodes[id.0 as usize]
                .parent
                .expect("validated lane parent"),
            Endpoint::Group(id) => id,
        };
        self.groups[group.0 as usize]
    }
}

#[derive(Debug, Default)]
pub(super) struct Level {
    pub(super) direction: Direction,
    pub(super) items: Vec<Endpoint>,
    pub(super) legs: Vec<Leg>,
    pub(super) ports: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Border {
    pub(super) level: usize,
    pub(super) slot: usize,
    pub(super) edge: usize,
    pub(super) outgoing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Terminal {
    Item(Endpoint, Option<usize>),
    Boundary(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Leg {
    pub(super) edge: usize,
    pub(super) source: Terminal,
    pub(super) target: Terminal,
    pub(super) main: bool,
}

fn admit(count: usize, columns: usize, work: &mut Work<'_>) -> LayoutResult<()> {
    work.charge(
        count
            .checked_mul(columns)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )
}

fn parent_id(parent: Option<GroupId>, root: usize) -> LayoutResult<usize> {
    match parent {
        None => Ok(root),
        Some(id) if (id.0 as usize) < root => Ok(id.0 as usize),
        _ => Err(LayoutError::InvalidInput(
            "parent is outside the group array",
        )),
    }
}

fn validate_endpoint(graph: &Graph, endpoint: Endpoint, port: PortSideHint) -> LayoutResult<()> {
    let missing = LayoutError::InvalidInput("edge endpoint is outside its input array");
    match endpoint {
        Endpoint::Node(id) => {
            let node = graph.nodes.get(id.0 as usize).ok_or(missing)?;
            flat::validate_port(node.size, port)
        }
        Endpoint::Group(id) => {
            graph.groups.get(id.0 as usize).ok_or(missing)?;
            if matches!(port, PortSideHint::FixedOffset { offset, .. } if !offset.is_finite()) {
                return Err(LayoutError::InvalidInput("non-finite group port offset"));
            }
            Ok(())
        }
    }
}

fn normalize_parents(graph: &Graph, work: &mut Work<'_>) -> LayoutResult<Vec<usize>> {
    let root = graph.groups.len();
    let count = root.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
    admit(count, 2, work)?;
    let mut parents = reserved(count)?;
    for group in &graph.groups {
        work.visit(1)?;
        let padding = group.padding;
        if ![padding.top, padding.right, padding.bottom, padding.left]
            .into_iter()
            .all(flat::length)
            || !flat::measured(group.min_size)
            || group.title.is_some_and(|size| !flat::measured(size))
        {
            return Err(LayoutError::InvalidInput("invalid group measurements"));
        }
        parents.push(parent_id(group.parent, root)?);
    }
    parents.push(root);
    let mut state = filled(count, 0_u8)?;
    state[root] = 2;
    for start in 0..root {
        work.visit(1)?;
        if state[start] != 0 {
            continue;
        }
        let mut at = start;
        while state[at] == 0 {
            work.visit(1)?;
            state[at] = 1;
            at = parents[at];
        }
        let mut lift = None;
        if state[at] == 1 {
            let mut smallest = at;
            let mut next = parents[at];
            while next != at {
                work.visit(1)?;
                smallest = smallest.min(next);
                next = parents[next];
            }
            lift = Some(smallest);
        }
        at = start;
        while state[at] == 1 {
            work.visit(1)?;
            state[at] = 2;
            at = parents[at];
        }
        if let Some(group) = lift {
            parents[group] = root;
        }
    }
    Ok(parents)
}

fn breadth_first(
    levels: &[Level],
    order: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    order.clear();
    order.push(levels.len() - 1);
    let mut cursor = 0;
    while cursor < order.len() {
        work.visit(1)?;
        for &item in &levels[order[cursor]].items {
            work.visit(1)?;
            if let Endpoint::Group(id) = item {
                order.push(id.0 as usize);
            }
        }
        cursor += 1;
    }
    Ok(())
}

/// Heavy paths give logarithmic common-ancestor and child-below queries in linear space.
struct Tree {
    parent: Vec<usize>,
    depth: Vec<usize>,
    heavy: Vec<usize>,
    head: Vec<usize>,
}

impl Tree {
    fn index(parent: Vec<usize>, order: &[usize], work: &mut Work<'_>) -> LayoutResult<Self> {
        let count = parent.len();
        let root = count - 1;
        admit(count, 4, work)?;
        let mut size = filled(count, 1_usize)?;
        let mut heavy = filled(count, root)?;
        for &group in order[1..].iter().rev() {
            work.visit(1)?;
            let outer = parent[group];
            size[outer] += size[group];
            if heavy[outer] == root || size[group] > size[heavy[outer]] {
                heavy[outer] = group;
            }
        }
        let mut depth = filled(count, 0)?;
        let mut head = filled(count, root)?;
        for &group in &order[1..] {
            work.visit(1)?;
            let outer = parent[group];
            depth[group] = depth[outer] + 1;
            head[group] = if heavy[outer] == group {
                head[outer]
            } else {
                group
            };
        }
        Ok(Self {
            parent,
            depth,
            heavy,
            head,
        })
    }

    fn common(
        &self,
        mut left: usize,
        mut right: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<usize> {
        while self.head[left] != self.head[right] {
            work.visit(1)?;
            if self.depth[self.head[left]] >= self.depth[self.head[right]] {
                left = self.parent[self.head[left]];
            } else {
                right = self.parent[self.head[right]];
            }
        }
        Ok(if self.depth[left] <= self.depth[right] {
            left
        } else {
            right
        })
    }

    fn child_below(
        &self,
        ancestor: usize,
        mut inside: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<usize> {
        while self.head[inside] != self.head[ancestor] {
            work.visit(1)?;
            let head = self.head[inside];
            inside = self.parent[head];
            if inside == ancestor {
                return Ok(head);
            }
        }
        Ok(self.heavy[ancestor])
    }
}

impl Hierarchy {
    pub(super) fn item_slot(&self, item: Endpoint) -> usize {
        match item {
            Endpoint::Node(id) => self.node_slots[id.0 as usize],
            Endpoint::Group(id) => self.group_slots[id.0 as usize],
        }
    }

    pub(super) fn lane_shell(&self, graph: &Graph, item: Endpoint) -> Option<usize> {
        match item {
            Endpoint::Group(id) => self.lanes.as_ref()?.shell(graph, id.0 as usize),
            Endpoint::Node(_) => None,
        }
    }

    fn new(
        graph: &Graph,
        parent: &[usize],
        lanes: Option<LaneMembership>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let root = graph.groups.len();
        admit(parent.len(), 5, work)?;
        admit(graph.nodes.len(), 2, work)?;
        work.charge(graph.edges.len())?;
        let mut levels = reserved(parent.len())?;
        levels.resize_with(parent.len(), Level::default);
        for (index, node) in graph.nodes.iter().enumerate() {
            work.visit(1)?;
            flat::validate_node(node)?;
            let owner = parent_id(node.parent, root)?;
            let level = if let Some(lanes) = &lanes {
                if owner == root {
                    return Err(LayoutError::InvalidInput(
                        "assign loose nodes to a swimlane",
                    ));
                }
                lanes.level(graph, owner)
            } else {
                owner
            };
            push(&mut levels[level].items, NodeId(index as u32).into())?;
        }
        for (group, &outer) in parent[..root].iter().enumerate() {
            work.visit(1)?;
            if lanes
                .as_ref()
                .is_some_and(|lanes| lanes.shell(graph, group).is_some())
            {
                levels[group].direction = graph.direction;
                continue;
            }
            push(&mut levels[outer].items, GroupId(group as u32).into())?;
        }
        let mut order = reserved(parent.len())?;
        breadth_first(&levels, &mut order, work)?;
        let mut seq = reserved(root)?;
        for group in &graph.groups {
            work.visit(1)?;
            seq.push(group.seq);
        }
        let declaration = |item, seq: &[u32]| match item {
            Endpoint::Node(id) => graph.nodes[id.0 as usize].seq,
            Endpoint::Group(id) => seq[id.0 as usize],
        };
        for &group in order[1..].iter().rev() {
            work.visit(1)?;
            let mut least = None;
            for &item in &levels[group].items {
                work.visit(1)?;
                let value = declaration(item, &seq);
                least = Some(least.map_or(value, |previous: u32| previous.min(value)));
            }
            if let Some(value) = least {
                seq[group] = value;
            }
        }
        let mut node_slots = filled(graph.nodes.len(), 0)?;
        let mut group_slots = filled(root, 0)?;
        for level in &mut levels {
            work.visit(1)?;
            let len = level.items.len();
            if len > 1 {
                admit(len, len.ilog2() as usize + 1, work)?;
                level.items.sort_unstable_by_key(|&item| {
                    (declaration(item, &seq), super::item_index(graph, item))
                });
                work.check()?;
            }
            for (slot, &item) in level.items.iter().enumerate() {
                work.visit(1)?;
                match item {
                    Endpoint::Node(id) => node_slots[id.0 as usize] = slot,
                    Endpoint::Group(id) => group_slots[id.0 as usize] = slot,
                }
            }
        }
        breadth_first(&levels, &mut order, work)?;
        Ok(Self {
            levels,
            order,
            borders: Vec::new(),
            paths: reserved(graph.edges.len())?,
            truncated: 0,
            node_slots,
            group_slots,
            lanes,
        })
    }

    fn border(&mut self, level: usize, edge: usize, outgoing: bool) -> LayoutResult<usize> {
        let id = self.borders.len();
        let ports = &mut self.levels[level].ports;
        let slot = ports.len();
        push(ports, id)?;
        push(
            &mut self.borders,
            Border {
                level,
                slot,
                edge,
                outgoing,
            },
        )?;
        Ok(id)
    }

    fn leg(&mut self, level: usize, leg: Leg) -> LayoutResult<(usize, usize)> {
        let legs = &mut self.levels[level].legs;
        let slot = legs.len();
        push(legs, leg)?;
        Ok((level, slot))
    }
}

#[derive(Clone, Copy)]
struct End {
    item: Endpoint,
    owner: usize,
    scope: usize,
}

impl End {
    fn new(item: Endpoint, graph: &Graph, tree: &Tree, hierarchy: &Hierarchy) -> Self {
        match item {
            Endpoint::Node(id) => {
                let owner = graph.nodes[id.0 as usize]
                    .parent
                    .map_or(graph.groups.len(), |id| id.0 as usize);
                let owner = hierarchy
                    .lanes
                    .as_ref()
                    .map_or(owner, |lanes| lanes.level(graph, owner));
                Self {
                    item,
                    owner,
                    scope: owner,
                }
            }
            Endpoint::Group(id) => {
                let scope = hierarchy
                    .lanes
                    .as_ref()
                    .map_or(id.0 as usize, |lanes| lanes.level(graph, id.0 as usize));
                Self {
                    item,
                    owner: tree.parent[scope],
                    scope,
                }
            }
        }
    }

    fn on_boundary(self, level: usize) -> bool {
        matches!(self.item, Endpoint::Group(id) if id.0 as usize == level)
    }

    fn transits(self, level: usize, tree: &Tree) -> usize {
        if self.on_boundary(level) {
            0
        } else {
            tree.depth[self.owner] - tree.depth[level]
        }
    }
}

struct EdgePath {
    edge: usize,
    level: usize,
    truncate: bool,
    steps: Vec<(usize, usize)>,
}

impl EdgePath {
    fn attach(
        &mut self,
        end: End,
        outgoing: bool,
        h: &mut Hierarchy,
        tree: &Tree,
        work: &mut Work<'_>,
    ) -> LayoutResult<Terminal> {
        if end.on_boundary(self.level) {
            return Ok(Terminal::Boundary(
                h.border(self.level, self.edge, !outgoing)?,
            ));
        }
        let mut owner = end.owner;
        let mut terminal = Terminal::Item(end.item, None);
        if self.truncate && owner != self.level {
            let child = tree.child_below(self.level, owner, work)?;
            return Ok(Terminal::Item(GroupId(child as u32).into(), None));
        }
        while owner != self.level {
            work.visit(1)?;
            let border = h.border(owner, self.edge, outgoing)?;
            let boundary = Terminal::Boundary(border);
            let (source, target) = if outgoing {
                (terminal, boundary)
            } else {
                (boundary, terminal)
            };
            self.steps.push(h.leg(
                owner,
                Leg {
                    edge: self.edge,
                    source,
                    target,
                    main: false,
                },
            )?);
            terminal = Terminal::Item(GroupId(owner as u32).into(), Some(border));
            owner = tree.parent[owner];
        }
        Ok(terminal)
    }
}

pub(super) fn build(
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Hierarchy> {
    work.check()?;
    flat::validate_options(options)?;
    for len in [graph.nodes.len(), graph.groups.len(), graph.edges.len()] {
        u32::try_from(len).map_err(|_| WorkError::ArithmeticOverflow)?;
    }
    graph
        .nodes
        .len()
        .checked_add(graph.groups.len())
        .ok_or(WorkError::ArithmeticOverflow)?;
    let mut parents = normalize_parents(graph, work)?;
    let lanes = if graph.lane_order.is_empty() {
        None
    } else {
        let lanes = LaneMembership::new(graph, &parents, work)?;
        for parent in &mut parents {
            work.visit(1)?;
            *parent = lanes.level(graph, *parent);
        }
        Some(lanes)
    };
    let mut hierarchy = Hierarchy::new(graph, &parents, lanes, work)?;
    let tree = Tree::index(parents, &hierarchy.order, work)?;
    work.charge(tree.parent.len())?;
    let mut crossed = filled(tree.parent.len(), 0_i64)?;
    let mut remaining = options.caps.max_border_ports;
    for (edge, spec) in graph.edges.iter().enumerate() {
        work.visit(1)?;
        flat::validate_edge(spec)?;
        validate_endpoint(graph, spec.source, spec.source_port)?;
        validate_endpoint(graph, spec.target, spec.target_port)?;
        let source = End::new(spec.source, graph, &tree, &hierarchy);
        let target = End::new(spec.target, graph, &tree, &hierarchy);
        let level = if source.item == target.item {
            source.owner
        } else {
            tree.common(source.scope, target.scope, work)?
        };
        let own = source.on_boundary(level) || target.on_boundary(level);
        if options.group_policy.inherit_crossed_direction && source.owner != target.owner {
            let outer = if own { tree.parent[level] } else { level };
            crossed[source.owner] += 1;
            crossed[target.owner] += 1;
            crossed[outer] -= 2;
        }
        let needed = source
            .transits(level, &tree)
            .checked_add(target.transits(level, &tree))
            .ok_or(WorkError::ArithmeticOverflow)?;
        let truncate = needed > remaining;
        let kept = if truncate {
            hierarchy.truncated = hierarchy
                .truncated
                .checked_add(needed)
                .ok_or(WorkError::ArithmeticOverflow)?;
            0
        } else {
            remaining -= needed;
            needed
        };
        let legs = kept.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
        let borders = kept + usize::from(own);
        admit(
            legs.checked_add(borders)
                .ok_or(WorkError::ArithmeticOverflow)?,
            2,
            work,
        )?;
        let mut path = EdgePath {
            edge,
            level,
            truncate,
            steps: reserved(legs)?,
        };
        let source = path.attach(source, true, &mut hierarchy, &tree, work)?;
        let incoming = path.steps.len();
        let target = path.attach(target, false, &mut hierarchy, &tree, work)?;
        path.steps.push(hierarchy.leg(
            level,
            Leg {
                edge,
                source,
                target,
                main: true,
            },
        )?);
        path.steps[incoming..].reverse();
        hierarchy.paths.push(path.steps);
    }
    for &group in hierarchy.order[1..].iter().rev() {
        work.visit(1)?;
        crossed[tree.parent[group]] += crossed[group];
    }
    hierarchy.levels[graph.groups.len()].direction = graph.direction;
    for &group in &hierarchy.order[1..] {
        work.visit(1)?;
        let inherited = hierarchy.levels[tree.parent[group]].direction;
        hierarchy.levels[group].direction = if crossed[group] != 0 {
            inherited
        } else {
            graph.groups[group].direction.unwrap_or(inherited)
        };
    }
    work.finish()?;
    Ok(hierarchy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeSpec, GroupSpec, NodeSpec, NoopWorkControl, Side, Size, WorkControl};

    fn group(seq: u32, owner: Option<u32>) -> GroupSpec {
        GroupSpec {
            parent: owner.map(GroupId),
            ..GroupSpec::new(seq)
        }
    }

    fn leaf(seq: u32, owner: Option<u32>) -> NodeSpec {
        NodeSpec {
            parent: owner.map(GroupId),
            ..NodeSpec::new(
                Size {
                    width: 40.0,
                    height: 20.0,
                },
                seq,
            )
        }
    }

    fn node(id: u32) -> Endpoint {
        NodeId(id).into()
    }
    fn container(id: u32) -> Endpoint {
        GroupId(id).into()
    }

    fn hierarchy(graph: &Graph, options: &Options) -> Hierarchy {
        build(graph, options, &mut Work::new(&mut NoopWorkControl)).unwrap()
    }

    fn fixture() -> Graph {
        let mut graph = Graph {
            groups: vec![
                group(10, None),
                group(11, Some(0)),
                group(12, None),
                group(13, Some(2)),
            ],
            nodes: vec![
                leaf(0, Some(1)),
                leaf(1, Some(3)),
                leaf(2, None),
                leaf(3, Some(0)),
            ],
            ..Default::default()
        };
        for (from, to) in [
            (node(0), node(1)),
            (node(0), node(1)),
            (container(0), node(0)),
            (node(0), container(0)),
            (node(0), node(0)),
            (container(1), container(1)),
            (container(1), node(1)),
            (container(0), container(1)),
            (container(1), container(0)),
            (node(3), node(2)),
        ] {
            let mut edge = EdgeSpec::new(from, to);
            edge.label = Some(Size {
                width: 12.0,
                height: 6.0,
            });
            graph.edges.push(edge);
        }
        graph
    }

    fn steps(hierarchy: &Hierarchy, edge: usize) -> Vec<(usize, Leg)> {
        hierarchy.paths[edge]
            .iter()
            .map(|&(level, slot)| (level, hierarchy.levels[level].legs[slot]))
            .collect()
    }

    fn assert_paths(hierarchy: &Hierarchy) {
        for (edge, path) in hierarchy.paths.iter().enumerate() {
            let steps = steps(hierarchy, edge);
            assert_eq!(steps.iter().filter(|(_, leg)| leg.main).count(), 1);
            for &(level, leg) in &steps {
                assert_eq!(leg.edge, edge);
                for terminal in [leg.source, leg.target] {
                    match terminal {
                        Terminal::Boundary(id) => {
                            assert_eq!(hierarchy.borders[id].level, level);
                            assert_eq!(hierarchy.borders[id].edge, edge);
                        }
                        Terminal::Item(item, border) => {
                            assert_eq!(
                                hierarchy.levels[level].items[hierarchy.item_slot(item)],
                                item
                            );
                            if let Some(id) = border {
                                let child = hierarchy.borders[id];
                                assert_eq!(child.edge, edge);
                                assert_eq!(item, container(child.level as u32));
                                assert_eq!(hierarchy.levels[child.level].ports[child.slot], id);
                            }
                        }
                    }
                }
            }
            for pair in steps.windows(2) {
                match (pair[0].1.target, pair[1].1.source) {
                    (Terminal::Boundary(left), Terminal::Item(_, Some(right))) => {
                        assert_eq!(left, right);
                        assert!(hierarchy.borders[left].outgoing);
                    }
                    (Terminal::Item(_, Some(left)), Terminal::Boundary(right)) => {
                        assert_eq!(left, right);
                        assert!(!hierarchy.borders[left].outgoing);
                    }
                    pair => panic!("disconnected edge {edge} at {pair:?} in {path:?}"),
                }
            }
        }
    }

    #[test]
    fn each_parent_cycle_lifts_its_smallest_group_and_preserves_attached_subtrees() {
        let graph = Graph {
            groups: vec![
                group(90, Some(3)),
                group(91, Some(2)),
                group(92, Some(1)),
                group(93, Some(2)),
                group(13, Some(4)),
                group(0, None),
            ],
            nodes: vec![
                leaf(5, Some(0)),
                leaf(10, None),
                leaf(20, Some(5)),
                leaf(18, Some(1)),
                leaf(5, None),
            ],
            ..Default::default()
        };
        let before = graph.clone();
        let h = hierarchy(&graph, &Options::default());
        assert_eq!(h.order, [6, 1, 4, 5, 2, 3, 0]);
        assert_eq!(
            h.levels[6].items,
            [node(4), container(1), node(1), container(4), container(5)]
        );
        assert_eq!(h.levels[1].items, [container(2), node(3)]);
        assert_eq!(h.levels[2].items, [container(3)]);
        assert_eq!(h.levels[3].items, [container(0)]);
        assert_eq!(h.levels[0].items, [node(0)]);
        for level in &h.levels {
            for (slot, &item) in level.items.iter().enumerate() {
                assert_eq!(h.item_slot(item), slot);
            }
        }
        assert_eq!(graph, before);
    }

    #[test]
    fn parallel_edges_loops_and_own_members_keep_occurrence_order_and_one_middle_leg() {
        let graph = fixture();
        let h = hierarchy(&graph, &Options::default());
        let root = graph.groups.len();
        assert_paths(&h);
        for edge in [0, 1] {
            let path = steps(&h, edge);
            assert_eq!(
                path.iter().map(|(level, _)| *level).collect::<Vec<_>>(),
                [1, 0, root, 2, 3]
            );
            assert!(path[2].1.main);
            assert_eq!(path[0].1.source, Terminal::Item(node(0), None));
            assert_eq!(path[4].1.target, Terminal::Item(node(1), None));
        }
        assert_ne!(h.paths[0], h.paths[1]);
        let outbound = steps(&h, 2);
        assert_eq!(outbound[0].0, 0);
        assert!(outbound[0].1.main);
        let Terminal::Boundary(id) = outbound[0].1.source else {
            panic!("missing group source")
        };
        assert!(!h.borders[id].outgoing);
        assert_eq!(outbound[1].1.target, Terminal::Item(node(0), None));
        let inbound = steps(&h, 3);
        assert!(inbound[1].1.main);
        let Terminal::Boundary(id) = inbound[1].1.target else {
            panic!("missing group target")
        };
        assert!(h.borders[id].outgoing);
        assert_eq!(inbound[0].1.source, Terminal::Item(node(0), None));
        for (edge, owner, endpoint) in [(4, 1, node(0)), (5, 0, container(1))] {
            let path = steps(&h, edge);
            assert_eq!(path.len(), 1);
            assert_eq!(path[0].0, owner);
            assert_eq!(path[0].1.source, Terminal::Item(endpoint, None));
            assert_eq!(path[0].1.target, path[0].1.source);
        }
    }

    fn border_counts(graph: &Graph, hierarchy: &Hierarchy) -> (Vec<usize>, Vec<usize>) {
        let mut transit = vec![0; graph.edges.len()];
        let mut own = vec![0; graph.edges.len()];
        for border in &hierarchy.borders {
            let edge = graph.edges[border.edge];
            let peer = if border.outgoing {
                edge.target
            } else {
                edge.source
            };
            if peer == container(border.level as u32) {
                own[border.edge] += 1;
            } else {
                transit[border.edge] += 1;
            }
        }
        (transit, own)
    }

    #[test]
    fn transit_cap_counts_only_omitted_ports_and_keeps_one_boundary_for_each_own_member() {
        let graph = fixture();
        let full = hierarchy(&graph, &Options::default());
        let (needed, required) = border_counts(&graph, &full);
        assert_eq!(needed, [4, 4, 1, 1, 0, 0, 3, 0, 0, 1]);
        assert_eq!(required, [0, 0, 1, 1, 0, 0, 0, 1, 1, 0]);
        for cap in 0..=needed.iter().sum() {
            let mut options = Options::default();
            options.caps.max_border_ports = cap;
            let h = hierarchy(&graph, &options);
            assert_paths(&h);
            let (transit, own) = border_counts(&graph, &h);
            assert_eq!(own, required);
            assert!(transit.iter().sum::<usize>() <= cap);
            let mut remaining = cap;
            for (&cost, &kept) in needed.iter().zip(&transit) {
                if cost <= remaining {
                    assert_eq!(kept, cost);
                    remaining -= cost;
                } else {
                    assert_eq!(kept, 0);
                }
            }
            assert_eq!(
                h.truncated,
                needed.iter().sum::<usize>() - transit.iter().sum::<usize>()
            );
            if cap == 0 {
                let main = steps(&h, 0)[0];
                assert_eq!(main.0, graph.groups.len());
                assert_eq!(main.1.source, Terminal::Item(container(0), None));
                assert_eq!(main.1.target, Terminal::Item(container(2), None));
                let own = steps(&h, 2)[0];
                assert_eq!(own.0, 0);
                assert!(matches!(own.1.source, Terminal::Boundary(_)));
                assert_eq!(own.1.target, Terminal::Item(container(1), None));
            }
        }
    }

    #[test]
    fn direction_inheritance_uses_semantic_crossings_even_when_transit_is_truncated() {
        let mut graph = Graph {
            groups: vec![
                group(0, None),
                group(1, Some(0)),
                group(2, Some(0)),
                group(3, None),
                group(4, Some(3)),
            ],
            nodes: vec![
                leaf(0, Some(1)),
                leaf(1, Some(2)),
                leaf(2, None),
                leaf(3, Some(4)),
            ],
            edges: vec![
                EdgeSpec::new(node(0), node(1)),
                EdgeSpec::new(container(3), node(2)),
            ],
            ..Default::default()
        };
        use Direction::*;
        for (group, direction) in graph.groups.iter_mut().zip([Right, Left, Up, Left, Up]) {
            group.direction = Some(direction);
        }
        let mut options = Options::default();
        options.caps.max_border_ports = 0;
        let directions = |graph: &Graph, options: &Options| {
            hierarchy(graph, options)
                .levels
                .iter()
                .map(|level| level.direction)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            directions(&graph, &options),
            [Right, Right, Right, Left, Up, Down]
        );
        graph.edges.push(EdgeSpec::new(container(3), node(3)));
        assert_eq!(
            directions(&graph, &options),
            [Right, Right, Right, Down, Down, Down]
        );
        options.group_policy.inherit_crossed_direction = false;
        assert_eq!(
            directions(&graph, &options),
            [Right, Left, Up, Left, Up, Down]
        );
    }

    #[test]
    fn rejects_out_of_bounds_handles_and_invalid_group_node_edge_and_option_measurements() {
        let graph = fixture();
        let invalid = |graph: &Graph, opts: &Options| {
            assert!(matches!(
                build(graph, opts, &mut Work::new(&mut NoopWorkControl)),
                Err(LayoutError::InvalidInput(_))
            ));
        };
        let options = Options::default();
        for edit in [
            (|g: &mut Graph| g.edges[0].source = node(35)) as fn(&mut Graph),
            |g| g.edges[0].target = container(75),
            |g| g.nodes[0].parent = Some(GroupId(75)),
            |g| g.groups[0].parent = Some(GroupId(75)),
            |g| g.groups[0].padding.left = f64::NAN,
            |g| g.groups[0].min_size.height = f64::from_bits(1),
            |g| {
                g.groups[0].title = Some(Size {
                    width: -1.0,
                    height: 1.0,
                })
            },
            |g| g.nodes[0].anchor_y = Some(100.0),
            |g| {
                g.edges[0].end_label = Some(Size {
                    width: 1.0,
                    height: f64::INFINITY,
                })
            },
            |g| g.edges[0].start_marker.width = -1.0,
            |g| g.edges[0].weight = 0.0,
            |g| {
                g.edges[0].source_port = PortSideHint::FixedOffset {
                    side: Side::Top,
                    offset: 21.0,
                }
            },
            |g| {
                g.edges[2].source_port = PortSideHint::FixedOffset {
                    side: Side::Left,
                    offset: f64::NAN,
                }
            },
        ] {
            let mut bad = graph.clone();
            edit(&mut bad);
            invalid(&bad, &options);
        }
        let mut bad = options;
        bad.spacing.port_pitch = -1.0;
        invalid(&graph, &bad);
        bad = options;
        bad.grid_quantum = Some(0.0);
        invalid(&graph, &bad);
        let mut valid = graph;
        valid.edges[2].source_port = PortSideHint::FixedOffset {
            side: Side::Top,
            offset: 5.0,
        };
        hierarchy(&valid, &options);
    }

    #[derive(Default)]
    struct Meter {
        units: usize,
        checks: usize,
        stop: Option<usize>,
    }

    impl WorkControl for Meter {
        fn charge(&mut self, count: usize) -> Result<(), WorkError> {
            self.units += count;
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

    #[test]
    fn deep_truncation_does_not_walk_the_omitted_transit_for_every_occurrence() {
        const DEPTH: u32 = 4_096;
        let graph = Graph {
            groups: (0..DEPTH).map(|id| group(id, id.checked_sub(1))).collect(),
            nodes: vec![leaf(0, Some(DEPTH - 1)), leaf(1, None)],
            edges: vec![EdgeSpec::new(node(0), node(1)); DEPTH as usize],
            ..Default::default()
        };
        let mut options = Options::default();
        options.caps.max_border_ports = 0;
        let mut meter = Meter::default();
        let h = build(&graph, &options, &mut Work::new(&mut meter)).unwrap();
        assert!(h.borders.is_empty());
        assert_eq!(h.truncated, DEPTH as usize * graph.edges.len());
        assert!(h.paths.iter().all(|path| path.len() == 1));
        assert!(
            meter.units < 64 * (graph.groups.len() + graph.edges.len()),
            "{} units",
            meter.units
        );
    }

    #[test]
    fn one_bulk_admission_covers_the_entire_border_and_leg_expansion() {
        #[derive(Default)]
        struct Charges {
            batches: Vec<usize>,
            deny_at: Option<usize>,
        }
        impl WorkControl for Charges {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }
            fn charge(&mut self, count: usize) -> Result<(), WorkError> {
                let index = self.batches.len();
                self.batches.push(count);
                if self.deny_at == Some(index) {
                    Err(WorkError::Interrupted)
                } else {
                    Ok(())
                }
            }
        }
        const DEPTH: u32 = 257;
        let graph = Graph {
            groups: (0..DEPTH).map(|id| group(id, id.checked_sub(1))).collect(),
            nodes: vec![leaf(0, Some(DEPTH - 1)), leaf(1, None)],
            edges: vec![EdgeSpec::new(node(1), node(0))],
            ..Default::default()
        };
        let mut fallback = Options::default();
        fallback.caps.max_border_ports = 0;
        let mut skipped = Charges::default();
        build(&graph, &fallback, &mut Work::new(&mut skipped)).unwrap();
        let mut full = Charges::default();
        let options = Options::default();
        build(&graph, &options, &mut Work::new(&mut full)).unwrap();
        let expansion = full
            .batches
            .iter()
            .zip(&skipped.batches)
            .position(|(a, b)| a != b)
            .unwrap();
        assert_eq!(
            full.batches[expansion] - skipped.batches[expansion],
            4 * DEPTH as usize
        );
        let mut denied = Charges {
            deny_at: Some(expansion),
            ..Default::default()
        };
        assert!(matches!(
            build(&graph, &options, &mut Work::new(&mut denied)),
            Err(LayoutError::Work(WorkError::Interrupted))
        ));
        assert_eq!(denied.batches, full.batches[..=expansion]);
        assert_eq!(
            hierarchy(&graph, &options).paths[0].len(),
            DEPTH as usize + 1
        );
    }

    #[test]
    fn early_midway_and_final_interruptions_leave_the_graph_reusable() {
        let graph = fixture();
        let options = Options::default();
        let original = graph.clone();
        let mut meter = Meter::default();
        let expected = build(&graph, &options, &mut Work::new(&mut meter)).unwrap();
        for stop in [1, meter.checks / 2, meter.checks] {
            let mut limited = Meter {
                stop: Some(stop),
                ..Default::default()
            };
            assert!(matches!(
                build(&graph, &options, &mut Work::new(&mut limited)),
                Err(LayoutError::Work(WorkError::Interrupted))
            ));
            let retry = hierarchy(&graph, &options);
            assert_eq!(retry.paths, expected.paths);
            assert_eq!(retry.borders, expected.borders);
            assert_eq!(graph, original);
        }
    }
}
