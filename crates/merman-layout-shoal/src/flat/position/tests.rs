use super::*;
use crate::flat::validate;
use crate::{EdgeSpec, NodeId, NoopWorkControl, Size, WorkControl};

fn expanded(rows: &[&[u32]], kinds: &[Kind], segments: &[(u32, u32)]) -> Expanded {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let n = kinds.len();
    let mut g = Expanded {
        layers: rows.iter().map(|r| r.to_vec()).collect(),
        kind: kinds.to_vec(),
        rank: vec![0; n],
        width: vec![10.0; n],
        from: segments.iter().map(|pair| pair.0).collect(),
        to: segments.iter().map(|pair| pair.1).collect(),
        ..Default::default()
    };
    for (i, row) in rows.iter().enumerate() {
        for &v in *row {
            g.rank[v as usize] = i as u32;
        }
    }
    g.pred = Csr::new(n, g.to.iter().copied(), &mut work).unwrap();
    g.succ = Csr::new(n, g.from.iter().copied(), &mut work).unwrap();
    g
}

fn graph(n: u32) -> Graph {
    Graph {
        nodes: (0..n)
            .map(|seq| {
                NodeSpec::new(
                    Size {
                        width: 80.0,
                        height: 40.0,
                    },
                    seq,
                )
            })
            .collect(),
        ..Default::default()
    }
}

fn edge(a: u32, b: u32) -> EdgeSpec {
    EdgeSpec::new(NodeId(a).into(), NodeId(b).into())
}

#[test]
fn spread_uses_all_three_pitch_tiers_and_preserves_a_held_zero() {
    use End::*;
    let role = [Arrives, Leaves, Held, Leaves, Leaves, Arrives];
    let ids = [0, 1, 2, 3, 4, 5];
    for (span, expected) in [
        (100.0, [-32.0, -16.0, 0.0, 0.0, 16.0, 32.0]),
        (24.0, [-12.0, -4.0, 0.0, 0.0, 4.0, 12.0]),
        (16.0, [-8.0, 0.0, 0.0, 0.0, 0.0, 8.0]),
        (10.0, [-5.0, 0.0, 0.0, 0.0, 0.0, 5.0]),
    ] {
        let mut offsets = [0.0; 6];
        let markers = [0.0; 6];
        spread_face(
            &ids,
            &role,
            &markers,
            &mut offsets,
            span,
            &Spacing::default(),
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        assert_eq!(offsets, expected);
    }
}

#[test]
fn wide_crows_foot_markers_separate_adjacent_ports_through_full_layout() {
    use crate::{MarkerMetrics, Options, layout};
    let mut g = graph(4);
    g.nodes[0].size = Size {
        width: 140.0,
        height: 60.0,
    };
    for target in 1..4 {
        let mut e = edge(0, target);
        e.start_marker = MarkerMetrics {
            length: 18.0,
            width: 24.0,
        };
        g.edges.push(e);
    }
    let out = layout(&g, &Options::default()).unwrap();
    let mut xs: Vec<f64> = out.edges.iter().map(|e| e.points[0].x).collect();
    xs.sort_by(f64::total_cmp);
    for pair in xs.windows(2) {
        assert!(
            pair[1] - pair[0] >= 25.0,
            "adjacent 24px markers must stay separated beyond default 16px port_pitch: {xs:?}"
        );
    }
}

#[test]
fn reverse_links_share_a_face_with_forward_tails_in_peer_order() {
    let mut graph = graph(5);
    graph.nodes[4].port_span = Some(16.0);
    graph.edges = vec![
        edge(4, 4),
        edge(3, 4),
        edge(4, 2),
        edge(4, 1),
        edge(0, 4),
        edge(4, 1),
    ];
    graph.edges[1].source_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: 3.0,
    };
    let mut c = Component {
        ids: vec![4, 3, 2, 1, 0],
        margin_left: vec![0.0; 5],
        margin_right: vec![0.0; 5],
        ..Default::default()
    };
    c.nodes = c.ids.iter().map(|&id| graph.nodes[id as usize]).collect();
    let local = |a, b, e| super::super::Link {
        from: a,
        to: b,
        edge: e,
        span: 1,
        weight: 1.0,
    };
    c.links = vec![
        local(4, 0, 4),
        local(0, 3, 5),
        local(0, 2, 2),
        local(1, 0, 1),
        local(0, 3, 3),
    ];
    let mut g = expanded(
        &[&[0], &[1, 2, 3, 4]],
        &[Kind::Node; 5],
        &[(0, 4), (0, 2), (0, 1), (0, 3)],
    );
    g.width.fill(80.0);
    g.chains = vec![vec![0, 4], vec![0, 3], vec![0, 2], vec![0, 1], vec![0, 3]];
    g.direct = vec![false, true, false, false, false];
    let o = Orientation {
        reversed: vec![true, false, false, true, false],
        topo: vec![0, 1, 2, 3, 4],
        cyclic_sccs: 1,
    };
    assign(
        &mut g,
        &c,
        Some(&o),
        &graph,
        &Options::default(),
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert_eq!(g.source_offset, [8.0, 0.0, -8.0, 0.0]);
    assert_eq!(g.target_offset, [0.0, 0.0, 3.0, 0.0]);
}

#[test]
fn fixed_and_icon_coordinates_project_onto_canonical_x_in_all_directions() {
    let mut node = graph(1).nodes[0];
    node.anchor_y = Some(10.0);
    for (dir, flow, side, half) in [
        (Direction::Down, Side::Bottom, Side::Left, 40.0),
        (Direction::Up, Side::Top, Side::Left, 40.0),
        (Direction::Right, Side::Right, Side::Top, 20.0),
        (Direction::Left, Side::Left, Side::Top, 20.0),
    ] {
        let project = |hint| held_offset(dir, &node, half * 2.0, Side::Bottom, hint);
        assert_eq!(
            project(PortSideHint::FixedOffset {
                side: flow,
                offset: 0.0
            }),
            Some(0.0)
        );
        assert_eq!(
            project(PortSideHint::FixedOffset {
                side: flow,
                offset: 3.0
            }),
            Some(3.0)
        );
        assert_eq!(
            project(PortSideHint::FixedOffset { side, offset: -4.0 }),
            Some(-half)
        );
        assert_eq!(project(PortSideHint::Side(side)), Some(-half));
        assert_eq!(
            project(PortSideHint::Auto),
            if half == 20.0 { Some(-10.0) } else { None }
        );
    }
}

#[test]
fn crossings_protect_inner_runs_and_choose_shorter_then_earlier_inner_segments() {
    use Kind::*;
    let g = expanded(
        &[&[0, 1, 2, 3, 4], &[5, 6, 7, 8, 9]],
        &[
            Dummy, Label, Node, Dummy, Node, Dummy, Node, Label, Dummy, Node,
        ],
        &[(0, 8), (1, 7), (3, 5), (2, 6), (4, 9)],
    );
    let blocked = conflicts(
        &g,
        &[0, 1, 2, 3, 4, 0, 1, 2, 3, 4],
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert_eq!(blocked, [true, false, true, true, false]);
    let tie = expanded(&[&[0, 1], &[2, 3]], &[Dummy; 4], &[(0, 3), (1, 2)]);
    assert_eq!(
        conflicts(&tie, &[0, 1, 0, 1], &mut Work::new(&mut NoopWorkControl)).unwrap(),
        [false, true]
    );
}

#[test]
fn reverse_topological_pull_tightens_loose_blocks_and_uses_physical_margins() {
    let g = expanded(&[&[0, 1, 2], &[3, 4]], &[Kind::Node; 5], &[]);
    let blocks = Blocks {
        root: vec![0, 1, 2, 3, 2],
        shift: vec![0.0, 0.0, 0.0, 0.0, 3.0],
    };
    let ext = [(5.0, 12.0), (9.0, 3.0), (8.0, 5.0), (3.0, 5.0), (6.0, 4.0)];
    for right in [false, true] {
        let xs = compact(
            &g,
            &ext,
            &blocks,
            right,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        assert_eq!(xs[1] - xs[0], 21.0);
        assert_eq!(xs[2] - xs[1], 11.0);
        assert_eq!(xs[4] - xs[3], 11.0);
        assert_eq!(xs[4] - xs[2], 3.0);
    }
    let c = Component {
        margin_left: vec![0.0, 12.0],
        margin_right: vec![22.0, 0.0],
        ..Default::default()
    };
    let g = expanded(&[&[0, 1, 2]], &[Kind::Node, Kind::Node, Kind::Label], &[]);
    let opts = Options::default();
    let node = 5.0 + opts.spacing.node / 2.0;
    let label = 5.0 + opts.spacing.edge / 2.0;
    assert_eq!(extents(&g, &c, &opts, 0), (node, node + 22.0));
    assert_eq!(extents(&g, &c, &opts, 1), (node + 12.0, node));
    assert_eq!(extents(&g, &c, &opts, 2), (label, label));
}

#[test]
fn four_passes_align_ports_not_centers_in_real_expanded_chains() {
    for dir in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        let mut graph = graph(2);
        graph.direction = dir;
        let side = super::super::physical_side(dir, Side::Bottom);
        let face = super::super::physical_side(dir, Side::Top);
        let mut e = edge(0, 1);
        e.source_port = PortSideHint::FixedOffset { side, offset: 12.0 };
        e.target_port = PortSideHint::FixedOffset {
            side: face,
            offset: -4.0,
        };
        e.label = Some(Size {
            width: 30.0,
            height: 9.0,
        });
        graph.edges.push(e);
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let opts = Options {
            inline_labels: false,
            ..Options::default()
        };
        let cs = validate::components(&graph, &opts, &mut work).unwrap();
        let mut g = Expanded::default();
        let (o, _) =
            super::super::prepare_component(&cs[0], &graph, &opts, &mut work, &mut g).unwrap();
        let xs = assign(&mut g, &cs[0], Some(&o), &graph, &opts, &mut work).unwrap();
        assert_eq!(g.source_offset, [12.0, 0.0]);
        assert_eq!(g.target_offset, [0.0, -4.0]);
        assert_eq!(xs[0] + 12.0, xs[1] - 4.0);
        assert_eq!(xs[0] + 12.0, xs[g.label_vertex[0].unwrap() as usize]);
    }
}

#[test]
fn interruption_before_balance_is_published_preserves_both_offset_arrays() {
    struct Budget(usize);
    impl WorkControl for Budget {
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.0 = self.0.checked_sub(units).ok_or(WorkError::Interrupted)?;
            Ok(())
        }
    }
    let mut graph = graph(3);
    graph.edges = vec![edge(0, 1), edge(0, 2)];
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let components = validate::components(&graph, &options, &mut work).unwrap();
    let component = &components[0];
    let mut g = Expanded::default();
    let (orientation, _) =
        super::super::prepare_component(component, &graph, &options, &mut work, &mut g).unwrap();
    let solve = |g: &mut Expanded, budget: &mut Budget| {
        assign(
            g,
            component,
            Some(&orientation),
            &graph,
            &options,
            &mut Work::new(budget),
        )
    };
    let mut unlimited = Budget(usize::MAX);
    let expected = solve(&mut g, &mut unlimited).unwrap();
    let cost = usize::MAX - unlimited.0;
    g.source_offset.fill(777.0);
    g.target_offset.fill(-777.0);
    assert_eq!(
        solve(&mut g, &mut Budget(cost - 1)),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    assert_eq!(g.source_offset, [777.0; 2]);
    assert_eq!(g.target_offset, [-777.0; 2]);
    assert_eq!(solve(&mut g, &mut Budget(cost)).unwrap(), expected);
    assert_eq!(g.source_offset, [-8.0, 8.0]);
    assert_eq!(g.target_offset, [0.0, 0.0]);
}
