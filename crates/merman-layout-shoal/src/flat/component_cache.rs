//! Bounded snapshots of accepted, unpacked editor components.

use super::{Component, Drawing};
use crate::scratch::capacity_bytes;
use crate::work::{LayoutResult, Work, reserved};
use crate::{
    Axis, Direction, EdgeGeometry, EdgeSpec, Graph, LayoutError, LayoutOutput, LayoutStats, NodeId,
    NodeSpec, Options, Point, PortSideHint, Rect, Size,
};

#[derive(Default, Debug)]
pub(crate) struct ComponentCache {
    accepted: Entries,
    pending: Entries,
    pending_limit: usize,
    active: bool,
}

impl ComponentCache {
    pub(crate) fn begin(&mut self, limit: usize) {
        // The previous snapshot remains available until the current solve is accepted.
        self.pending_limit = limit.saturating_sub(self.accepted.bytes());
        self.active = limit > 0;
    }

    pub(crate) fn pause(&mut self) {
        self.active = false;
    }

    pub(crate) fn discard_pending(&mut self) {
        self.pause();
        self.pending = Entries::default();
    }

    pub(crate) fn commit(&mut self, output: &LayoutOutput) {
        for entry in &mut self.pending.rows {
            entry.capture(output);
        }
        self.pending.rows.sort_unstable_by_key(|entry| entry.key);
        self.accepted = std::mem::take(&mut self.pending);
        self.pause();
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.accepted.bytes()
    }

    pub(crate) fn trim(&mut self, limit: usize) {
        if capacity_bytes(&self.accepted.rows) > limit {
            self.accepted = Entries::default();
        }
        while self.accepted.bytes() > limit {
            self.accepted.remove_last();
        }
        if self.accepted.rows.is_empty() {
            self.accepted = Entries::default();
        }
    }

    pub(super) fn active_for(&self, component: &Component) -> bool {
        self.active
            && !component.boundaries
            && !component.pinned
            && !component.group_proxies
            && (!component.links.is_empty() || !component.loops.is_empty())
    }

    pub(super) fn reuse(
        &self,
        component: &Component,
        graph: &Graph,
        options: Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<LocalLayout>> {
        let entries = &self.accepted.rows;
        let Some(first) = component.nodes.first().filter(|_| !entries.is_empty()) else {
            return Ok(None);
        };
        let (Some(hint), Some(order)) = (first.position_hint, first.order_hint) else {
            return Ok(None);
        };
        let key = Key::new(order.cross_key, hint.center, component);
        work.compare(|work| {
            work.visit(entries.len().ilog2() as usize + 1)?;
            let Ok(index) = entries.binary_search_by_key(&key, |entry| entry.key) else {
                return Ok(None);
            };
            if index > 0 && entries[index - 1].key == key
                || entries.get(index + 1).is_some_and(|entry| entry.key == key)
            {
                // An ambiguous first witness has no unique candidate.
                return Ok(None);
            }
            optional_allocation(entries[index].reuse(component, graph, options, work))
        })
        .map(Option::flatten)
    }

    pub(super) fn remember(
        &mut self,
        component: &Component,
        graph: &Graph,
        options: Options,
        local: &LocalLayout,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        if !options.capture_node_ranks {
            return Ok(());
        }
        let left = self.pending_limit.saturating_sub(self.pending.bytes());
        let Some(left) = left.checked_sub(size_of::<Entry>()) else {
            return Ok(());
        };
        // Prepay the publication sort and witness update. Publication follows the final work check.
        let sort_cost = (self.pending_limit / size_of::<Entry>()).ilog2() as usize + 1;
        let entry = work.compare(|work| {
            work.visit(sort_cost)?;
            optional_allocation(Entry::new(component, graph, options, local, left, work))
        })?;
        if let Some(Some(entry)) = entry {
            self.pending.push(entry, self.pending_limit);
        }
        Ok(())
    }
}

fn optional_allocation<T>(result: LayoutResult<Option<T>>) -> LayoutResult<Option<T>> {
    match result {
        Err(LayoutError::AllocationLimit) => Ok(None),
        result => result,
    }
}

#[derive(Default, Debug)]
struct Entries {
    rows: Vec<Entry>,
    bytes: usize,
}

impl Entries {
    fn bytes(&self) -> usize {
        capacity_bytes(&self.rows).saturating_add(self.bytes)
    }

    fn remove_last(&mut self) {
        self.bytes -= self.rows.pop().unwrap().bytes;
    }

    fn push(&mut self, entry: Entry, limit: usize) {
        let available = limit.saturating_sub(self.bytes.saturating_add(entry.bytes));
        let slots = available / size_of::<Entry>();
        let len = self.rows.len();
        if self.rows.capacity() > slots || len == slots {
            return;
        }
        if len == self.rows.capacity() {
            let capacity = len.max(2).saturating_mul(2).min(slots);
            if self.rows.try_reserve_exact(capacity - len).is_err() {
                return;
            }
        }
        if self.rows.capacity() <= slots {
            self.bytes += entry.bytes;
            self.rows.push(entry);
        }
    }
}

pub(super) struct LocalLayout {
    pub(super) drawing: Drawing,
    pub(super) ranks: Vec<u32>,
    pub(super) stats: LayoutStats,
    pub(super) editor_choice: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    absolute_cross: u64,
    relative: (u64, u64),
    nodes: usize,
    edges: usize,
}

impl Key {
    fn new(cross: f64, relative: Point, component: &Component) -> Self {
        Self {
            absolute_cross: cross.to_bits(),
            relative: (relative.x.to_bits(), relative.y.to_bits()),
            nodes: component.nodes.len(),
            edges: component.links.len() + component.loops.len(),
        }
    }
}

#[derive(Debug)]
struct Entry {
    key: Key,
    direction: Direction,
    options: Options,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    stats: LayoutStats,
    editor_choice: bool,
    bytes: usize,
}

#[derive(Debug)]
struct Node {
    output_id: usize,
    spec: NodeSpec,
    rect: Rect,
    rank: u32,
    relative: Point,
    absolute_cross: f64,
    incident_signature: u64,
}

#[derive(Debug)]
struct Edge {
    spec: EdgeSpec,
    geometry: EdgeGeometry,
}

impl Entry {
    fn new(
        c: &Component,
        graph: &Graph,
        options: Options,
        local: &LocalLayout,
        limit: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        let n = c.nodes.len();
        let m = local.drawing.edges.len();
        let mut bytes = n
            .saturating_mul(size_of::<Node>())
            .saturating_add(m.saturating_mul(size_of::<Edge>()));
        if bytes > limit {
            return Ok(None);
        }
        for (_, route) in &local.drawing.edges {
            work.visit(1)?;
            bytes = bytes.saturating_add(
                route
                    .points
                    .len()
                    .saturating_mul(size_of::<crate::RoutePoint>()),
            );
            if bytes > limit {
                return Ok(None);
            }
        }

        work.charge(
            n.checked_add(m)
                .ok_or(crate::WorkError::ArithmeticOverflow)?,
        )?;
        let mut nodes = reserved(n)?;
        for (i, &output_id) in c.ids.iter().enumerate() {
            work.visit(3)?;
            let Some(incident_signature) = c.nodes[i].current_incident_signature else {
                return Ok(None);
            };
            nodes.push(Node {
                output_id: output_id as usize,
                spec: node_spec(c.nodes[i]),
                rect: local.drawing.nodes[i],
                rank: local.ranks[i],
                relative: Point::default(),
                absolute_cross: 0.0,
                incident_signature,
            });
        }
        let mut edges = reserved(m)?;
        for ((id, geometry), (from, to, occurrence)) in
            local.drawing.edges.iter().zip(occurrences(c))
        {
            work.visit(1)?;
            if *id != occurrence {
                return Ok(None);
            }
            edges.push(Edge {
                spec: edge_spec(graph, from, to, occurrence),
                geometry: copy_edge(geometry, work)?,
            });
        }
        if edges.len() != m || m != c.links.len() + c.loops.len() {
            return Ok(None);
        }
        let bytes = edges.iter().fold(
            capacity_bytes(&nodes) + capacity_bytes(&edges),
            |bytes, edge| bytes.saturating_add(capacity_bytes(&edge.geometry.points)),
        );
        Ok(Some(Self {
            key: Key {
                nodes: n,
                edges: m,
                ..Key::default()
            },
            direction: graph.direction,
            options: cache_options(options),
            nodes,
            edges,
            stats: local.stats,
            editor_choice: local.editor_choice,
            bytes,
        }))
    }

    fn capture(&mut self, output: &LayoutOutput) {
        let origin = self.nodes.iter().fold(
            Point {
                x: f64::INFINITY,
                y: f64::INFINITY,
            },
            |origin, node| {
                let rect = output.nodes[node.output_id];
                Point {
                    x: origin.x.min(rect.x),
                    y: origin.y.min(rect.y),
                }
            },
        );
        for node in &mut self.nodes {
            let center = output.nodes[node.output_id].center();
            node.relative = Point {
                x: center.x - origin.x,
                y: center.y - origin.y,
            };
            node.absolute_cross = match self.direction.axis() {
                Axis::Vertical => center.x,
                Axis::Horizontal => center.y,
            };
        }
        let first = &self.nodes[0];
        self.key.absolute_cross = first.absolute_cross.to_bits();
        self.key.relative = (first.relative.x.to_bits(), first.relative.y.to_bits());
    }

    fn reuse(
        &self,
        c: &Component,
        graph: &Graph,
        options: Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<LocalLayout>> {
        let options_key = cache_options(options);
        if self.direction != graph.direction
            || self.options != options_key
            || spacing_bits(self.options) != spacing_bits(options_key)
        {
            return Ok(None);
        }
        for (i, (node, old)) in c.nodes.iter().zip(&self.nodes).enumerate() {
            work.visit(1)?;
            let (Some(position), Some(order)) = (node.position_hint, node.order_hint) else {
                return Ok(None);
            };
            // The flat solvers use declaration order and its ties. Dense NodeIds and the
            // absolute sequence values may shift when another component is edited.
            let same_declaration = (i == 0
                || c.nodes[i - 1].seq.cmp(&node.seq)
                    == self.nodes[i - 1].spec.seq.cmp(&old.spec.seq))
                && (node.seq == u32::MAX) == (old.spec.seq == u32::MAX);
            let mut spec = node_spec(*node);
            spec.seq = old.spec.seq;
            if !same_declaration
                || spec != old.spec
                || node_bits(spec) != node_bits(old.spec)
                || node.current_incident_signature != Some(old.incident_signature)
                || !node
                    .position_hint_in(self.direction, old.rank)
                    .is_some_and(|(_, unchanged)| unchanged)
                || position.center != old.relative
                || order.cross_key != old.absolute_cross
            {
                return Ok(None);
            }
        }
        for ((from, to, id), old) in occurrences(c).zip(&self.edges) {
            work.visit(1)?;
            if !same_edge(edge_spec(graph, from, to, id), old.spec) {
                return Ok(None);
            }
        }

        work.charge(self.nodes.len())?;
        let mut nodes = reserved(self.nodes.len())?;
        nodes.extend(self.nodes.iter().map(|node| node.rect));
        let mut ranks = Vec::new();
        if options.capture_node_ranks {
            work.charge(self.nodes.len())?;
            ranks = reserved(self.nodes.len())?;
            ranks.extend(self.nodes.iter().map(|node| node.rank));
        }
        work.charge(self.edges.len())?;
        let mut edges = reserved(self.edges.len())?;
        for ((_, _, id), edge) in occurrences(c).zip(&self.edges) {
            work.visit(1)?;
            edges.push((id, copy_edge(&edge.geometry, work)?));
        }
        Ok(Some(LocalLayout {
            drawing: Drawing { nodes, edges },
            ranks,
            stats: self.stats,
            editor_choice: self.editor_choice,
        }))
    }
}

fn cache_options(mut options: Options) -> Options {
    options.capture_node_ranks = false;
    options
}

fn spacing_bits(options: Options) -> [u64; 11] {
    let s = options.spacing;
    [
        s.node,
        s.rank,
        s.component,
        s.edge,
        s.loop_gap,
        s.corner_clearance,
        s.port_pitch,
        s.min_port_pitch,
        s.end_label_across,
        s.end_label_along,
        s.port_sep,
    ]
    .map(f64::to_bits)
}

fn node_spec(mut spec: NodeSpec) -> NodeSpec {
    spec.order_hint = None;
    spec.order_hint_direction = None;
    spec.position_hint = None;
    spec.current_incident_signature = None;
    spec
}

fn size_bits(size: Size) -> [u64; 2] {
    [size.width.to_bits(), size.height.to_bits()]
}

fn node_bits(node: NodeSpec) -> ([u64; 2], [Option<u64>; 3]) {
    (
        size_bits(node.size),
        [node.port_span, node.perpendicular_port_span, node.anchor_y].map(|v| v.map(f64::to_bits)),
    )
}

fn occurrences(c: &Component) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
    c.links
        .iter()
        .map(|edge| (edge.from, edge.to, edge.edge))
        .chain(c.loops.iter().map(|edge| (edge.node, edge.node, edge.edge)))
}

fn edge_spec(graph: &Graph, from: u32, to: u32, occurrence: u32) -> EdgeSpec {
    let mut spec = graph.edges[occurrence as usize];
    spec.source = NodeId(from).into();
    spec.target = NodeId(to).into();
    spec
}

fn same_edge(a: EdgeSpec, b: EdgeSpec) -> bool {
    let labels = |e: EdgeSpec| [e.label, e.start_label, e.end_label].map(|s| s.map(size_bits));
    let markers = |e: EdgeSpec| {
        [
            e.start_marker.width,
            e.start_marker.length,
            e.end_marker.width,
            e.end_marker.length,
        ]
        .map(f64::to_bits)
    };
    let offset = |port| match port {
        PortSideHint::FixedOffset { offset, .. } => Some(offset.to_bits()),
        _ => None,
    };
    a == b
        && labels(a) == labels(b)
        && markers(a) == markers(b)
        && offset(a.source_port) == offset(b.source_port)
        && offset(a.target_port) == offset(b.target_port)
}

fn copy_edge(edge: &EdgeGeometry, work: &mut Work<'_>) -> LayoutResult<EdgeGeometry> {
    work.charge(edge.points.len())?;
    let mut points = reserved(edge.points.len())?;
    for chunk in edge.points.chunks(128) {
        work.check()?;
        points.extend_from_slice(chunk);
    }
    Ok(EdgeGeometry { points, ..*edge })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LayoutScratch, NoopWorkControl, OrderHint, PositionHint, WorkControl, WorkError, layout,
        layout_with_control,
    };

    #[derive(Default)]
    struct Meter(usize);

    impl WorkControl for Meter {
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }

        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.0 += units;
            Ok(())
        }

        fn try_charge_optional(&mut self, units: usize, _: usize) -> Result<bool, WorkError> {
            self.charge(units)?;
            Ok(true)
        }
    }

    fn edge(from: u32, to: u32) -> EdgeSpec {
        EdgeSpec::new(NodeId(from).into(), NodeId(to).into())
    }

    fn measured_graph(direction: Direction) -> Graph {
        let mut graph = Graph {
            direction,
            edges: vec![
                edge(0, 1),
                edge(2, 3),
                edge(2, 3),
                edge(3, 4),
                edge(2, 4),
                edge(4, 4),
            ],
            ..Graph::default()
        };
        graph.nodes = (0..5)
            .map(|id| {
                let mut node = NodeSpec::new(
                    Size {
                        width: 40. + f64::from(id),
                        height: 30.,
                    },
                    id,
                );
                node.set_current_incident_signature(Some(u64::from(80 + id)));
                node
            })
            .collect();
        graph.edges[1].label = Some(Size {
            width: 20.,
            height: 10.,
        });
        graph.edges[4].end_marker.length = 10.;
        graph.edges[4].end_marker.width = 8.;
        graph.edges[5].label = Some(Size {
            width: 18.,
            height: 12.,
        });
        graph
    }

    fn options() -> Options {
        Options {
            capture_node_ranks: true,
            reuse_unchanged_components: true,
            inline_labels: true,
            ..Options::default()
        }
    }

    fn restore(graph: &mut Graph, previous: &LayoutOutput, nodes: &[usize], offset: usize) {
        let origin = nodes.iter().fold(
            Point {
                x: f64::INFINITY,
                y: f64::INFINITY,
            },
            |origin, &i| Point {
                x: origin.x.min(previous.nodes[i].x),
                y: origin.y.min(previous.nodes[i].y),
            },
        );
        for &i in nodes {
            let node = &mut graph.nodes[i + offset];
            let bounds = previous.nodes[i];
            let center = bounds.center();
            node.order_hint_direction = Some(graph.direction);
            node.order_hint = Some(OrderHint {
                rank: previous.node_ranks[i] as i32,
                cross_key: match graph.direction.axis() {
                    Axis::Vertical => center.x,
                    Axis::Horizontal => center.y,
                },
            });
            let sig = node.current_incident_signature;
            node.set_position_hint(
                PositionHint {
                    center: Point {
                        x: center.x - origin.x,
                        y: center.y - origin.y,
                    },
                    size: Size {
                        width: bounds.width,
                        height: bounds.height,
                    },
                    incident_signature: sig,
                },
                sig,
            );
        }
    }

    #[test]
    fn accepted_component_reuses_geometry_and_statistics_after_occurrences_shift() {
        for direction in [
            Direction::Down,
            Direction::Up,
            Direction::Right,
            Direction::Left,
        ] {
            let mut graph = measured_graph(direction);
            let options = options();
            let mut scratch = LayoutScratch::default();
            let before =
                layout_with_control(&graph, &options, &mut scratch, &mut Meter::default()).unwrap();
            assert_eq!(
                scratch.frame().expanded.component_cache.accepted.rows.len(),
                2
            );

            for node in &mut graph.nodes {
                node.seq += 10;
            }
            graph.nodes.insert(
                0,
                NodeSpec::new(
                    Size {
                        width: 55.,
                        height: 26.,
                    },
                    0,
                ),
            );
            for spec in &mut graph.edges {
                let shift = |endpoint| match endpoint {
                    crate::Endpoint::Node(NodeId(id)) => NodeId(id + 1).into(),
                    crate::Endpoint::Group(_) => unreachable!(),
                };
                spec.source = shift(spec.source);
                spec.target = shift(spec.target);
            }
            graph.edges.insert(0, edge(0, 0));
            graph.nodes[2].size.width += 25.;
            restore(&mut graph, &before, &[2, 3, 4], 1);

            let (mut warm, mut cold) = (Meter::default(), Meter::default());
            let reused = layout_with_control(&graph, &options, &mut scratch, &mut warm).unwrap();
            let solved =
                layout_with_control(&graph, &options, &mut LayoutScratch::default(), &mut cold)
                    .unwrap();
            assert_eq!(reused, solved);
            assert!(warm.0 < cold.0, "{direction:?}: {} >= {}", warm.0, cold.0);
            assert!(scratch.retained_bytes() <= 2 * 1024 * 1024);
            println!(
                "{direction:?}: component replay {} → {} work units",
                cold.0, warm.0
            );
        }
    }

    #[test]
    fn cache_requires_every_witness_and_exact_local_specification() {
        let mut graph = measured_graph(Direction::Down);
        let options = options();
        let mut scratch = LayoutScratch::default();
        let before =
            layout_with_control(&graph, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        restore(&mut graph, &before, &[2, 3, 4], 0);

        let entry_matches = |graph: &Graph, options: Options, scratch: &mut LayoutScratch| {
            let mut control = NoopWorkControl;
            let mut work = Work::new(&mut control);
            work.charge(1000).unwrap();
            let components =
                super::super::validate::components(graph, &options, &mut work).unwrap();
            scratch
                .frame()
                .expanded
                .component_cache
                .reuse(&components[1], graph, options, &mut work)
                .unwrap()
                .is_some()
        };
        assert!(entry_matches(&graph, options, &mut scratch));
        for change in [
            |g: &mut Graph| g.nodes[3].position_hint.as_mut().unwrap().center.y += 1.,
            |g: &mut Graph| g.nodes[3].order_hint.as_mut().unwrap().cross_key += 1.,
            |g: &mut Graph| g.nodes[3].order_hint.as_mut().unwrap().rank += 1,
            |g: &mut Graph| g.nodes[3].order_hint = None,
            |g: &mut Graph| g.nodes[3].order_hint_direction = Some(Direction::Right),
            |g: &mut Graph| g.nodes[3].position_hint = None,
            |g: &mut Graph| g.nodes[3].seq = g.nodes[2].seq,
            |g: &mut Graph| (g.nodes[2].seq, g.nodes[3].seq) = (g.nodes[3].seq, g.nodes[2].seq),
            |g: &mut Graph| g.nodes[3].set_current_incident_signature(Some(0)),
            |g: &mut Graph| {
                let node = &mut g.nodes[3];
                node.position_hint.as_mut().unwrap().incident_signature = Some(0);
                node.set_current_incident_signature(Some(0));
            },
            |g: &mut Graph| g.nodes[3].port_span = Some(1.),
            |g: &mut Graph| g.nodes[3].split_incoming_ports = true,
            |g: &mut Graph| g.edges[1].label.as_mut().unwrap().height += 1.,
            |g: &mut Graph| g.edges.swap(1, 2),
        ] {
            let mut changed = graph.clone();
            change(&mut changed);
            assert!(!entry_matches(&changed, options, &mut scratch));
        }
        let mut capped = options;
        capped.caps.max_dummies -= 1;
        assert!(!entry_matches(&graph, capped, &mut scratch));
        let mut explicitly_replayed = options;
        explicitly_replayed.capture_node_ranks = false;
        assert!(entry_matches(&graph, explicitly_replayed, &mut scratch));
    }

    #[test]
    fn inactive_uncommitted_and_over_cap_frames_preserve_the_accepted_cache() {
        let graph = measured_graph(Direction::Down);
        let options = options();
        let mut scratch = LayoutScratch::default();
        let first =
            layout_with_control(&graph, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        let cached = scratch.frame().expanded.component_cache.retained_bytes();
        assert!(cached > 0);
        {
            let mut frame = scratch.frame();
            frame.begin_components();
            let mut unknown = measured_graph(Direction::Right);
            unknown.nodes[3].size.width += 11.;
            super::super::run(
                &unknown,
                &options,
                frame.expanded,
                &mut Work::new(&mut NoopWorkControl),
            )
            .unwrap();
            assert!(!frame.expanded.component_cache.pending.rows.is_empty());
            frame.pause_components();
            super::super::run(
                &graph,
                &options,
                frame.expanded,
                &mut Work::new(&mut NoopWorkControl),
            )
            .unwrap();
        }
        assert_eq!(
            scratch.frame().expanded.component_cache.retained_bytes(),
            cached
        );
        let mut inactive = options;
        inactive.reuse_unchanged_components = false;
        let (mut reused, mut fresh) = (Meter::default(), Meter::default());
        assert_eq!(
            layout_with_control(&graph, &inactive, &mut scratch, &mut reused).unwrap(),
            first
        );
        assert_eq!(
            layout_with_control(&graph, &inactive, &mut LayoutScratch::default(), &mut fresh)
                .unwrap(),
            first
        );
        assert_eq!(reused.0, fresh.0);
        let mut small = LayoutScratch::with_max_retained_bytes(cached - 1);
        assert_eq!(
            layout_with_control(&graph, &options, &mut small, &mut NoopWorkControl).unwrap(),
            layout(&graph, &options).unwrap()
        );
        assert!(small.retained_bytes() < cached);
        let mut zero = LayoutScratch::with_max_retained_bytes(0);
        assert_eq!(
            layout_with_control(&graph, &options, &mut zero, &mut NoopWorkControl).unwrap(),
            first
        );
        assert_eq!(zero.retained_bytes(), 0);
    }

    #[test]
    fn pending_snapshots_share_the_budget_with_the_accepted_snapshot() {
        let graph = measured_graph(Direction::Down);
        let options = options();
        let mut unrestricted = LayoutScratch::default();
        layout_with_control(&graph, &options, &mut unrestricted, &mut NoopWorkControl).unwrap();
        let capacity = unrestricted
            .frame()
            .expanded
            .component_cache
            .accepted
            .bytes();
        assert!(capacity > 0);

        let mut scratch = LayoutScratch::with_max_retained_bytes(capacity);
        layout_with_control(&graph, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        let mut frame = scratch.frame();
        let accepted = frame.expanded.component_cache.accepted.bytes();
        assert!(accepted > 0);
        frame.begin_components();
        super::super::run(
            &graph,
            &options,
            frame.expanded,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        let cache = &frame.expanded.component_cache;
        assert_eq!(cache.accepted.bytes(), accepted);
        assert!(cache.pending.bytes() + accepted <= capacity);
    }
}
