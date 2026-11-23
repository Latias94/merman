use super::*;
use crate::{NoopWorkControl, OrderHint, Size, WorkControl};

#[test]
fn compound_order_keeps_nested_siblings_and_their_internal_bend_contiguous() {
    use crate::{EdgeSpec, Graph, GroupId, NodeId, PortSideHint, Side};
    let mut graph = Graph::default();
    for v in 0..8 {
        graph.nodes.push(NodeSpec::new(
            Size {
                width: 80.,
                height: 40.,
            },
            7 - v,
        ));
    }
    for v in 1..7 {
        let mut first = EdgeSpec::new(NodeId(0).into(), NodeId(v).into());
        first.source_port = PortSideHint::FixedOffset {
            side: Side::Bottom,
            offset: f64::from(v) * 4. - 14.,
        };
        let mut last = EdgeSpec::new(NodeId(v).into(), NodeId(7).into());
        last.target_port = PortSideHint::FixedOffset {
            side: Side::Top,
            offset: 14. - f64::from(v) * 4.,
        };
        graph.edges.extend([first, last]);
    }
    graph
        .edges
        .push(EdgeSpec::new(NodeId(0).into(), NodeId(7).into()));
    let membership = [1, 1, 2, 3, 1, 2, 3, 1].map(|id| Some(GroupId(id)));
    let parents = [None, Some(GroupId(0)), None, Some(GroupId(0))];
    let mut expanded = Expanded::default();
    let options = Options::default();
    let (x, crossings) = crate::flat::compound_order(
        &graph,
        &membership,
        &parents,
        &options,
        &mut expanded,
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    let chain = &expanded.order_chains()[12];
    assert_eq!(chain.len(), 3);
    assert_eq!((chain[0], chain[2]), (0, 7));
    let bend = chain[1];
    let rank = expanded.order_ranks()[bend as usize];
    let row = &expanded.layers[rank as usize];
    assert!(
        row.windows(2)
            .all(|pair| x[pair[0] as usize] < x[pair[1] as usize])
    );
    for group in [&[1, 4, bend][..], &[3, 6], &[2, 5], &[1, 3, 4, 6, bend]] {
        let first = row.iter().position(|v| group.contains(v)).unwrap();
        assert!(
            row[first..first + group.len()]
                .iter()
                .all(|v| group.contains(v)),
            "{row:?} {group:?}"
        );
    }
    assert!(crossings > 0);
    assert_eq!(
        crossings,
        brute(
            &expanded,
            &positions(x.len(), expanded.layers.iter().map(Vec::as_slice))
        )
    );
}

#[test]
fn grouped_zero_crossing_order_and_deep_parent_chains_work_across_disconnected_nodes() {
    use crate::{Graph, GroupId};
    let graph = Graph {
        nodes: (0..6)
            .map(|seq| NodeSpec::new(Size::default(), seq))
            .collect(),
        ..Graph::default()
    };
    let mut parents: Vec<_> = (0..16_384).map(|id| Some(GroupId(id + 1))).collect();
    parents.extend([None, None]);
    let membership = [0, 16_385, 0, 16_385, 0, 16_385].map(|id| Some(GroupId(id)));
    let mut expanded = Expanded::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let (x, crossings) = crate::flat::compound_order(
        &graph,
        &membership,
        &parents,
        &Options::default(),
        &mut expanded,
        &mut work,
    )
    .unwrap();
    assert_eq!(crossings, 0);
    assert_eq!(expanded.layers, [vec![0, 2, 4, 1, 3, 5]]);
    assert!(x[4] < x[1]);
    parents[16_384] = Some(GroupId(0));
    assert!(matches!(
        crate::flat::compound_order(
            &graph,
            &membership,
            &parents,
            &Options::default(),
            &mut expanded,
            &mut work
        ),
        Err(LayoutError::InvalidInput(_))
    ));
    let (x, _) = crate::flat::compound_order(
        &Graph::default(),
        &[],
        &[],
        &Options::default(),
        &mut expanded,
        &mut work,
    )
    .unwrap();
    assert!(
        x.is_empty() && expanded.order_ranks().is_empty() && expanded.order_chains().is_empty()
    );
}

struct Random(u64);

impl Random {
    fn below(&mut self, limit: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as usize % limit
    }
}

type Segment = (u32, u32, f64, f64, f32);

fn graph(sizes: &[usize], segments: &[Segment]) -> (Expanded, Vec<NodeSpec>) {
    let mut g = Expanded::default();
    for (rank, &size) in sizes.iter().enumerate() {
        let start = g.rank.len() as u32;
        g.layers.push((start..start + size as u32).collect());
        g.rank.extend(std::iter::repeat_n(rank as u32, size));
    }
    let n = g.rank.len();
    g.width = vec![10.; n];
    for &(from, to, source, target, weight) in segments {
        g.from.push(from);
        g.to.push(to);
        g.source_offset.push(source);
        g.target_offset.push(target);
        g.weight.push(weight);
    }
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    g.pred = Csr::new(n, g.to.iter().copied(), &mut work).unwrap();
    g.succ = Csr::new(n, g.from.iter().copied(), &mut work).unwrap();
    let nodes = (0..n)
        .map(|v| NodeSpec::new(Size::default(), v as u32))
        .collect();
    (g, nodes)
}

fn ports<'a>(g: &'a Expanded, work: &mut Work<'_>) -> Ports<'a> {
    Ports {
        source: Face::new(g, false, work).unwrap(),
        target: Face::new(g, true, work).unwrap(),
    }
}

/// Count intersections of pairs of segments by independently comparing their two attachments.
fn brute(g: &Expanded, pos: &[usize]) -> u64 {
    let attachment = |u: u32, du: f64, v: u32, dv: f64| {
        pos[u as usize]
            .cmp(&pos[v as usize])
            .then_with(|| du.partial_cmp(&dv).unwrap())
    };
    let adjacent = |e: usize| g.rank[g.to[e] as usize] == g.rank[g.from[e] as usize] + 1;
    let mut count = 0;
    for a in 0..g.from.len() {
        for b in 0..a {
            if !adjacent(a)
                || !adjacent(b)
                || g.rank[g.from[a] as usize] != g.rank[g.from[b] as usize]
            {
                continue;
            }
            let top = attachment(g.from[a], g.source_offset[a], g.from[b], g.source_offset[b]);
            let bottom = attachment(g.to[a], g.target_offset[a], g.to[b], g.target_offset[b]);
            if top != Ordering::Equal && top.reverse() == bottom {
                count += 1;
            }
        }
    }
    count
}

fn positions<'a>(n: usize, rows: impl Iterator<Item = &'a [u32]>) -> Vec<usize> {
    let mut pos = vec![usize::MAX; n];
    for row in rows {
        for (i, &v) in row.iter().enumerate() {
            assert_eq!(pos[v as usize], usize::MAX);
            pos[v as usize] = i;
        }
    }
    assert!(!pos.contains(&usize::MAX));
    pos
}

fn brute_order(g: &Expanded, order: &Order) -> u64 {
    let pos = positions(
        g.rank.len(),
        (0..order.ranks()).map(|r| &order.ids[order.range(r)]),
    );
    assert!(pos.iter().zip(&order.place).all(|(&i, p)| i == p.pos));
    brute(g, &pos)
}

#[test]
fn both_two_pointer_scores_match_quadratic_pairs_with_multiplicities() {
    let mut random = Random(0x5aa1);
    for i in 0..160 {
        let sizes = match i {
            0 => [0, 80],
            1 => [250, 250],
            _ => [random.below(80), random.below(80)],
        };
        let [a, b] = sizes.map(|n| {
            let mut keys: Vec<_> = (0..n).map(|_| random.below(17)).collect();
            keys.sort_unstable();
            keys
        });
        let mut expected = (0, 0);
        for x in &a {
            for y in &b {
                expected.0 += u64::from(x > y);
                expected.1 += u64::from(y > x);
            }
        }
        assert_eq!(between(&a, &b), expected);
    }
}

#[test]
fn randomized_fenwick_swap_deltas_and_cached_transpose_match_full_pair_enumeration() {
    let mut random = Random(0xfee1);
    let mut improved = 0;
    for _ in 0..64 {
        let sizes = [3, 5, 4, 2];
        let starts = [0, 3, 8, 12];
        let mut segments: Vec<_> = (0..48)
            .map(|_| {
                let r = random.below(3);
                let a = starts[r] + random.below(sizes[r]);
                let b = starts[r + 1] + random.below(sizes[r + 1]);
                let offset = |random: &mut Random| [-5., -0., 0., 2., 5.][random.below(5)];
                (
                    a as u32,
                    b as u32,
                    offset(&mut random),
                    offset(&mut random),
                    (1 + random.below(5)) as f32,
                )
            })
            .collect();
        segments.extend([(0, 13, 0., 0., 1.), segments[0]]); // A direct fallback and a parallel occurrence.
        let (mut g, nodes) = graph(&sizes, &segments);
        for row in &mut g.layers {
            for i in 1..row.len() {
                row.swap(i, random.below(i + 1));
            }
        }
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let ports = ports(&g, &mut work);
        let (mut order, _) = initial_order(&g, &nodes, &ports, &mut work).unwrap();
        let initial = brute_order(&g, &order);
        order
            .restore(&g.layers.concat(), &ports, &mut work)
            .unwrap();
        let before = brute_order(&g, &order);
        assert_eq!(
            crossings(&order, &ports, &mut Fenwick::default(), &mut work).unwrap(),
            before
        );
        let mut up = NeighborKeys::new(ports.target.segments.len(), order.ranks()).unwrap();
        let mut down = NeighborKeys::new(ports.source.segments.len(), order.ranks()).unwrap();
        for r in 0..order.ranks() {
            up.prepare(r, &order, &ports, true, &mut work).unwrap();
            down.prepare(r, &order, &ports, false, &mut work).unwrap();
            let row = order.range(r);
            for i in row.start..row.end - 1 {
                let (a, b) = (order.ids[i], order.ids[i + 1]);
                let (p, q) = up.pair(&ports.target, a, b, &mut work).unwrap();
                let (s, t) = down.pair(&ports.source, a, b, &mut work).unwrap();
                order.swap(i, &ports);
                assert_eq!(brute_order(&g, &order) + p + s, before + q + t);
                order.swap(i, &ports);
            }
        }
        let got =
            transpose::<false>(&mut order, &ports, before, &Rules::default(), &mut work).unwrap();
        assert_eq!(got, brute_order(&g, &order));
        improved += usize::from(got < before);
        let final_count = reduce(&mut g, &nodes, &Options::default(), &mut work).unwrap();
        assert!(final_count <= initial);
        assert_eq!(
            final_count,
            brute(
                &g,
                &positions(g.rank.len(), g.layers.iter().map(Vec::as_slice))
            )
        );
    }
    assert!(improved > 0);
}

#[test]
fn distinct_ports_on_two_singleton_ranks_can_cross() {
    let (mut g, nodes) = graph(
        &[1, 1],
        &[
            (0, 1, -5., 5., 1.),
            (0, 1, 5., -5., 1.),
            (0, 1, 5., -5., 9.),
            (0, 1, -0., 0., 1.),
        ],
    );
    assert_eq!(
        reduce(
            &mut g,
            &nodes,
            &Options::default(),
            &mut Work::new(&mut NoopWorkControl)
        )
        .unwrap(),
        5
    );
}

#[test]
fn weighted_barycenters_use_neighbor_offsets_and_hold_only_unconnected_slots() {
    let (g, nodes) = graph(
        &[2, 4],
        &[
            (0, 2, 0., 0., 1.),
            (1, 2, 0., 0., 9.),
            (0, 4, 0., 0., 1.),
            (1, 4, 0., 0., 1.),
            (0, 5, 5., 0., 3.),
        ],
    );
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let ports = ports(&g, &mut work);
    let (mut order, _) = initial_order(&g, &nodes, &ports, &mut work).unwrap();
    order
        .restore(&g.layers.concat(), &ports, &mut work)
        .unwrap();
    let mut sweep = Sweep {
        mean: vec![0.; 6],
        moving: Vec::new(),
    };
    sweep
        .run::<false>(
            &g,
            &mut order,
            &ports,
            (true, false),
            &mut Rules::default(),
            &mut work,
        )
        .unwrap();
    assert_eq!(order.ids[order.range(1)], [4, 3, 5, 2]);
    assert_eq!(
        [sweep.mean[2], sweep.mean[4], sweep.mean[5]],
        [0.9, 0.5, 0.5]
    );
    sweep
        .run::<false>(
            &g,
            &mut order,
            &ports,
            (true, true),
            &mut Rules::default(),
            &mut work,
        )
        .unwrap();
    assert_eq!(order.ids[order.range(1)], [5, 3, 4, 2]);
}

#[test]
fn dfs_initialization_and_complete_hints_preserve_dummy_slots() {
    let (g, mut nodes) = graph(
        &[2, 3],
        &[(0, 2, 0., 0., 1.), (0, 3, 0., 0., 1.), (1, 4, 0., 0., 1.)],
    );
    nodes.truncate(4); // Vertex 4 carries a dummy or label and has no declaration or hint.
    for (v, seq) in nodes.iter_mut().zip([9, 4, 8, 7]) {
        v.seq = seq;
    }
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let ports = ports(&g, &mut work);
    nodes[2].order_hint = Some(OrderHint {
        rank: 1,
        cross_key: -9.,
    });
    let (order, preference) = initial_order(&g, &nodes, &ports, &mut work).unwrap();
    assert_eq!(order.ids, [1, 0, 4, 3, 2]);
    assert_eq!(
        inversions(&order, &preference, &mut Fenwick::default(), &mut work).unwrap(),
        1
    );
    nodes[3].order_hint = Some(OrderHint {
        rank: 1,
        cross_key: -1.,
    });
    let (order, preference) = initial_order(&g, &nodes, &ports, &mut work).unwrap();
    assert_eq!(order.ids, [1, 0, 4, 2, 3]);
    assert_eq!(
        inversions(&order, &preference, &mut Fenwick::default(), &mut work).unwrap(),
        0
    );
}

#[test]
fn mirror_recounts_fixed_ports_and_breaks_equal_crossings_by_declaration_inversions() {
    let mut options = Options::default();
    options.caps.max_sweeps = 0;
    options.caps.max_transpose_nodes = 0;
    let (mut g, nodes) = graph(&[1, 2], &[(0, 1, 4., 0., 1.), (0, 2, -4., 0., 1.)]);
    assert_eq!(
        reduce(
            &mut g,
            &nodes,
            &options,
            &mut Work::new(&mut NoopWorkControl)
        )
        .unwrap(),
        0
    );
    assert_eq!(g.layers[1], [2, 1]);
    let (mut g, nodes) = graph(
        &[2, 4, 1, 1],
        &[
            (0, 4, 0., 0., 1.),
            (0, 5, 0., 0., 1.),
            (1, 2, 0., 0., 1.),
            (1, 3, 0., 0., 1.),
            (6, 7, -1., 1., 1.),
            (6, 7, 1., -1., 1.),
        ],
    );
    assert_eq!(
        reduce(
            &mut g,
            &nodes,
            &options,
            &mut Work::new(&mut NoopWorkControl)
        )
        .unwrap(),
        1
    );
    assert_eq!(g.layers[0], [1, 0]);
    assert_eq!(g.layers[1], [3, 2, 5, 4]);
}

struct Meter {
    remaining: usize,
    checks: usize,
    cancel_at: usize,
}

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        self.checks += 1;
        if self.checks == self.cancel_at {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, n: usize) -> Result<(), WorkError> {
        self.remaining = self
            .remaining
            .checked_sub(n)
            .ok_or(WorkError::Interrupted)?;
        Ok(())
    }
}

#[test]
fn budget_and_mid_search_cancellation_leave_the_input_reusable() {
    let (mut g, nodes) = graph(
        &[2, 2],
        &[
            (0, 2, 0., 0., 1.),
            (0, 3, 0., 0., 1.),
            (1, 2, 0., 0., 1.),
            (1, 3, 0., 0., 1.),
        ],
    );
    g.layers[1].reverse();
    let original = g.layers.clone();
    let mut meter = Meter {
        remaining: usize::MAX,
        checks: 0,
        cancel_at: 0,
    };
    let options = Options::default();
    let count = reduce(&mut g, &nodes, &options, &mut Work::new(&mut meter)).unwrap();
    let used = usize::MAX - meter.remaining;
    let result = g.layers.clone();
    assert_ne!(original, result);
    for (remaining, cancel_at) in [
        (used - 1, 0),
        (used, meter.checks / 2),
        (used, meter.checks),
    ] {
        g.layers.clone_from(&original);
        let mut control = Meter {
            remaining,
            checks: 0,
            cancel_at,
        };
        assert_eq!(
            reduce(&mut g, &nodes, &options, &mut Work::new(&mut control)),
            Err(LayoutError::Work(WorkError::Interrupted))
        );
        assert_eq!(g.layers, original);
    }
    let mut exact = Meter {
        remaining: used,
        checks: 0,
        cancel_at: 0,
    };
    assert_eq!(
        reduce(&mut g, &nodes, &options, &mut Work::new(&mut exact)).unwrap(),
        count
    );
    assert_eq!(exact.remaining, 0);
    assert_eq!(g.layers, result);
}
