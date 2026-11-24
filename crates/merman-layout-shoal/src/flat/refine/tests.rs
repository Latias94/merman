use super::super::{BoundaryRank, Csr, Link};
use super::*;
use crate::{
    EdgeSpec, LayoutError, NodeId, NodeSpec, NoopWorkControl, OrderHint, Point, PositionHint, Size,
    WorkControl,
};

struct Case {
    g: Expanded,
    c: Component,
    orientation: Orientation,
    graph: Graph,
    options: Options,
}

impl Case {
    fn new(real: usize, rows: &[&[u32]], chains: &[&[u32]]) -> Self {
        let n = rows.iter().map(|r| r.len()).sum();
        let mut g = Expanded {
            layers: rows.iter().map(|r| r.to_vec()).collect(),
            rank: vec![0; n],
            logical_rank: (0..rows.len() as u64).collect(),
            kind: (0..n)
                .map(|v| if v < real { Kind::Node } else { Kind::Dummy })
                .collect(),
            width: (0..n).map(|v| if v < real { 20. } else { 0. }).collect(),
            height: vec![0.; n],
            direct: vec![false; chains.len()],
            label_vertex: vec![None; chains.len()],
            ..Default::default()
        };
        for (r, row) in rows.iter().enumerate() {
            for &v in *row {
                g.rank[v as usize] = r as u32;
            }
        }
        let nodes: Vec<_> = (0..real)
            .map(|v| {
                NodeSpec::new(
                    Size {
                        width: 20.,
                        height: 20.,
                    },
                    v as u32,
                )
            })
            .collect();
        let mut c = Component {
            nodes: nodes.clone(),
            ids: (0..real as u32).collect(),
            margin_left: vec![0.; real],
            margin_right: vec![0.; real],
            ..Default::default()
        };
        let mut graph = Graph {
            nodes,
            ..Default::default()
        };
        for (i, chain) in chains.iter().enumerate() {
            let (from, to) = (chain[0], *chain.last().unwrap());
            c.links.push(Link {
                from,
                to,
                edge: i as u32,
                span: 1,
                weight: 1.,
            });
            graph
                .edges
                .push(EdgeSpec::new(NodeId(from).into(), NodeId(to).into()));
            for pair in chain.windows(2) {
                g.from.push(pair[0]);
                g.to.push(pair[1]);
                g.source_offset.push(0.);
                g.target_offset.push(0.);
                g.weight.push(1.);
            }
            g.chains.push(chain.to_vec());
        }
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        g.pred = Csr::new(n, g.to.iter().copied(), &mut work).unwrap();
        g.succ = Csr::new(n, g.from.iter().copied(), &mut work).unwrap();
        let mut options = Options::default();
        options.spacing.node = 10.;
        options.spacing.edge = 4.;
        options.spacing.corner_clearance = 0.;
        options.spacing.port_pitch = 8.;
        options.spacing.min_port_pitch = 4.;
        Self {
            g,
            c,
            orientation: Orientation {
                reversed: vec![false; chains.len()],
                topo: Vec::new(),
                cyclic_sccs: 0,
            },
            graph,
            options,
        }
    }
    fn geom(&self, x: &[f64], work: &mut Work<'_>) -> Geometry<'_> {
        Geometry::new(
            &self.g,
            &self.c,
            &self.orientation,
            &self.graph,
            &self.options,
            x,
            work,
        )
        .unwrap()
    }
    fn run(&mut self, x: &mut [f64], work: &mut Work<'_>) -> LayoutResult<()> {
        refine(
            &mut self.g,
            &self.c,
            &self.orientation,
            &self.graph,
            &self.options,
            x,
            work,
        )
    }
}

#[test]
fn weighted_projection_pools_separated_centers_by_their_anchor_weights() {
    let mut values = [72., 54.];
    assert!(weighted_pava(
        &mut values,
        &[0., 20.],
        Interval { lo: 0., hi: 120. },
        ZeroGap::Ordered,
        &mut Vec::new(),
        |i| [9., 1.][i],
    ));
    assert!((values[0] - 68.2).abs() < 1e-10);
    assert!((values[1] - 88.2).abs() < 1e-10);
}

#[test]
fn full_anchor_weight_requires_unchanged_size_and_known_incident_signature() {
    let size = Size {
        width: 20.,
        height: 30.,
    };
    let prior = PositionHint {
        center: Point { x: 15., y: 35. },
        size,
        incident_signature: Some(42),
    };
    let mut node = NodeSpec::new(size, 0);
    node.order_hint = Some(OrderHint {
        rank: 1,
        cross_key: 35.,
    });
    node.order_hint_direction = Some(Direction::Left);
    node.set_position_hint(prior, Some(42));
    assert_eq!(node.position_hint_in(Direction::Left, 1), Some((35., true)));

    node.size.width += 5.;
    assert_eq!(
        node.position_hint_in(Direction::Left, 1),
        Some((35., false))
    );
    node.size = size;
    for signature in [Some(7), None] {
        node.set_position_hint(prior, signature);
        assert_eq!(
            node.position_hint_in(Direction::Left, 1),
            Some((35., false))
        );
    }
}

#[test]
fn projection_respects_boundary_pins_group_proxies_and_protected_forks() {
    let mut case = Case::new(6, &[&[0], &[1, 2, 3, 4], &[5]], &[&[0, 2], &[2, 5]]);
    case.g.kind[0] = Kind::StartPort;
    case.c.nodes[0].boundary = Some(BoundaryRank::Start);
    case.c.nodes[3].pin = 1;
    case.c.nodes[4].group_proxy = true;
    for ((node, &rank), wanted) in case
        .c
        .nodes
        .iter_mut()
        .zip(&case.g.rank)
        .zip([300., 130., 70., 70., 20., 140.])
    {
        node.order_hint = Some(OrderHint {
            rank: rank as i32,
            cross_key: wanted,
        });
        node.order_hint_direction = Some(Direction::Down);
        node.set_position_hint(
            PositionHint {
                center: Point { x: wanted, y: 0. },
                size: node.size,
                incident_signature: Some(1),
            },
            Some(1),
        );
    }
    let initial = [80., 10., 100., 200., 290., 140.];
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    work.charge(100).unwrap(); // Account for the required passes this fixture skips.
    let mut s = case.geom(&initial, &mut work);
    s.protected[1] = true;
    mental_map::stabilize(&mut s, &case.c, Direction::Down, &case.options, &mut work).unwrap();
    for v in [0, 1, 3, 4] {
        assert_eq!(s.x[v], initial[v]);
    }
    assert!(s.x[2] < 80. && s.x[2] >= 40.);
    assert!(work.editor_choice);
}

#[test]
fn measured_release_centers_stage_and_publish_and_routes_audit_monotonically_in_four_directions() {
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        let horizontal = matches!(direction, Direction::Left | Direction::Right);
        let mut graph = Graph {
            direction,
            ..Default::default()
        };
        let gate = Size {
            width: 171.,
            height: 171.,
        };
        let stadium = Size {
            width: 171.25,
            height: 45.,
        };
        let rectangle = Size {
            width: 152.,
            height: 45.,
        };
        for (seq, size) in [
            stadium, gate, rectangle, rectangle, rectangle, rectangle, stadium, stadium,
        ]
        .into_iter()
        .enumerate()
        {
            let mut node = NodeSpec::new(size, seq as u32);
            node.port_span = Some(if size == gate {
                0.
            } else if size == stadium {
                if horizontal {
                    0.
                } else {
                    size.width - size.height
                }
            } else if horizontal {
                size.height - 16.
            } else {
                size.width - 16.
            });
            graph.nodes.push(node);
        }
        for (source, target, label) in [
            (0, 1, None),
            (1, 2, Some(20.44)),
            (1, 3, Some(14.42)),
            (2, 4, None),
            (2, 5, None),
            (4, 6, None),
            (5, 6, None),
            (3, 7, None),
        ] {
            let mut edge = EdgeSpec::new(NodeId(source).into(), NodeId(target).into());
            edge.label = label.map(|width| Size { width, height: 21. });
            graph.edges.push(edge);
        }
        let drawing = crate::layout(&graph, &Options::default()).unwrap();
        let cross = |x: f64, y: f64| if horizontal { y } else { x };
        let center = |node: usize| {
            let point = drawing.nodes[node].center();
            cross(point.x, point.y)
        };
        let midpoint = center(4).midpoint(center(5));
        assert_eq!(center(2), midpoint, "Stage release, {direction:?}");
        assert_eq!(center(6), midpoint, "Publish, {direction:?}");
        let audit = &drawing.edges[6].points;
        assert!(audit.len() >= 2);
        let across = audit.iter().map(|point| cross(point.x, point.y));
        assert!(
            across.clone().is_sorted() || across.rev().is_sorted(),
            "Audit → Publish, {direction:?}"
        );
        assert_eq!(drawing.stats.routed_crossings, 0);
    }
}

#[test]
fn private_fork_and_join_keep_the_same_axis_after_settle_and_snap() {
    let mut case = Case::new(
        7,
        &[&[5], &[0, 4], &[1, 2, 6], &[3]],
        &[&[5, 0], &[0, 1], &[0, 2], &[1, 3], &[2, 3], &[4, 6]],
    );
    let mut x = [44., 15., 95., 85., 140., 44., 160.];
    case.run(&mut x, &mut Work::new(&mut NoopWorkControl))
        .unwrap();
    assert_eq!(x[0], x[1].midpoint(x[2]));
    assert_eq!(x[3], x[0]);
    assert_eq!(
        x[5], x[0],
        "the straight upstream spine moves with the fork"
    );
    for row in &case.g.layers {
        for p in row.windows(2) {
            assert!(x[p[1] as usize] - x[p[0] as usize] >= 30.);
        }
    }
}

#[test]
fn binary_fan_preserves_a_shared_straight_spine_and_its_branch_ports() {
    let spine = Case::new(
        8,
        &[&[7], &[4], &[0, 6, 1], &[2, 3, 5]],
        &[&[7, 4], &[4, 0], &[4, 1], &[0, 2], &[0, 3], &[1, 3]],
    );
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut geometry = spine.geom(&[30., 130., 10., 90., 30., 160., 80., 30.], &mut work);
    center_forks(&mut geometry, 8, &mut work).unwrap();
    assert_eq!([geometry.x[7], geometry.x[4], geometry.x[0]], [30.; 3]);

    let mut case = Case::new(4, &[&[0, 1], &[2, 3]], &[&[0, 2], &[0, 3], &[1, 3]]);
    case.g.target_offset.copy_from_slice(&[0., -2., 2.]);
    let mut x = [0., 100., 0., 60.];
    case.run(&mut x, &mut work).unwrap();
    assert_eq!(x[0], 29.);
    assert_eq!(
        x[0],
        (x[2] + case.g.target_offset[0]).midpoint(x[3] + case.g.target_offset[1])
    );
    assert!(
        case.g.target_offset[2] - case.g.target_offset[1] >= case.options.spacing.min_port_pitch
    );
}

#[test]
fn diamond_polish_clamps_to_loop_clearance_and_does_not_touch_interleaved_branches() {
    let mut c = Case::new(
        5,
        &[&[0, 1], &[2, 3], &[4]],
        &[&[0, 2], &[0, 3], &[1, 3], &[2, 4], &[3, 4]],
    );
    c.c.margin_left[1] = 12.;
    let mut x = [12., 56., 20., 80., 60.];
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut s = c.geom(&x, &mut work);
    center_forks(&mut s, 5, &mut work).unwrap();
    assert_eq!(s.x[0], 14.);
    assert_eq!(s.x[1] - s.x[0], 42.);
    c.g.layers[1].insert(1, 5);
    c.g.kind.push(Kind::Node);
    c.g.width.push(20.);
    c.g.height.push(20.);
    c.g.rank.push(1);
    c.c.nodes.push(NodeSpec::new(
        Size {
            width: 20.,
            height: 20.,
        },
        5,
    ));
    c.c.margin_left.push(0.);
    c.c.margin_right.push(0.);
    c.g.pred = Csr::new(6, c.g.to.iter().copied(), &mut work).unwrap();
    c.g.succ = Csr::new(6, c.g.from.iter().copied(), &mut work).unwrap();
    x[0] = 12.;
    let mut xx = x.to_vec();
    xx.push(50.);
    let mut s = c.geom(&xx, &mut work);
    center_forks(&mut s, 6, &mut work).unwrap();
    assert_eq!(s.x[0], 12.);
}

#[test]
fn pava_pools_weighted_means_and_projects_to_common_bounds() {
    let mut pools = Vec::new();
    let mut xs = [8., -2., 6.];
    assert!(pava(
        &mut xs,
        &[0., 2., 2.],
        Interval { lo: 0., hi: 10. },
        ZeroGap::Equal,
        &mut pools
    ));
    assert_eq!(xs, [2., 4., 6.]);
    let mut xs = [-9., 9., 20.];
    assert!(pava(
        &mut xs,
        &[0., 0., 4.],
        Interval { lo: -3., hi: 6. },
        ZeroGap::Equal,
        &mut pools
    ));
    assert_eq!(xs, [0., 0., 6.]);
    let mut xs = [7., 8.];
    assert!(!pava(
        &mut xs,
        &[0., 4.],
        Interval { lo: 0., hi: 2. },
        ZeroGap::Equal,
        &mut pools
    ));
    assert_eq!(xs, [7., 8.]);
}

#[test]
fn crowded_fixed_ports_use_the_pitch_that_fits_their_own_interval() {
    let mut case = Case::new(
        6,
        &[&[0], &[1, 2, 3, 4, 5]],
        &[&[0, 1], &[0, 2], &[0, 3], &[0, 4], &[0, 5]],
    );
    case.g.width[0] = 120.;
    case.c.nodes[0].port_span = Some(100.);
    case.g
        .source_offset
        .copy_from_slice(&[-6., -1., 0., 1., 6.]);
    for (edge, offset) in [(0, -6.), (4, 6.)] {
        case.graph.edges[edge].source_port = PortSideHint::FixedOffset {
            side: Side::Bottom,
            offset,
        };
    }
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let x = [100., 0., 50., 100., 150., 200.];
    for (reversed, expected) in [
        (false, [-6., -2., 0., 2., 6.]),
        (true, [-6., -3., 0., 3., 6.]),
    ] {
        case.orientation.reversed[2] = reversed;
        let mut s = case.geom(&x, &mut work);
        settle(&mut s, 6, &case.options, &mut work).unwrap();
        assert_eq!(s.dx[BOTTOM], expected);
    }
}

#[test]
fn held_ports_partition_pava_including_reversed_semantic_ends_and_horizontal_anchors() {
    let mut c = Case::new(
        6,
        &[&[0], &[1, 2, 3, 4, 5]],
        &[&[0, 1], &[0, 2], &[0, 3], &[0, 4], &[0, 5]],
    );
    c.graph.direction = Direction::Right;
    c.g.width[0] = 120.;
    c.c.nodes[0].size.height = 120.;
    c.c.nodes[0].port_span = Some(100.);
    c.c.nodes[1].anchor_y = Some(4.);
    c.orientation.reversed[2] = true;
    c.graph.edges[2].target_port = PortSideHint::FixedOffset {
        side: Side::Right,
        offset: 0.,
    };
    c.g.source_offset.copy_from_slice(&[-20., -8., 0., 8., 20.]);
    c.graph.edges[4].source_port = PortSideHint::Side(Side::Right);
    let x = [100., 0., 48., 100., 152., 204.];
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut s = c.geom(&x, &mut work);
    settle(&mut s, 6, &c.options, &mut work).unwrap();
    assert_eq!(s.dx[BOTTOM][2], 0.);
    assert_eq!(s.dx[BOTTOM][4], 20.);
    assert_eq!(s.dx[TOP][0], -6.);
    assert!(s.dx[BOTTOM][1] <= -8.);
    assert!(s.dx[BOTTOM][3] >= 8.);
    assert!(s.dx[BOTTOM].windows(2).all(|p| p[1] - p[0] >= 8.));
}

#[test]
fn labels_share_a_legal_chain_column_and_rigid_blocks_respect_snap_limits() {
    let mut c = Case::new(4, &[&[0], &[2, 4], &[5, 3], &[1]], &[&[0, 4, 5, 1]]);
    c.g.kind[5] = Kind::Label;
    c.g.width[5] = 20.;
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut s = c.geom(&[10., 60., 20., 90., 50., 50.], &mut work);
    align_labels(&mut s, &mut work).unwrap();
    assert_eq!([s.x[4], s.x[5]], [60., 60.]);
    let c = Case::new(3, &[&[0], &[1], &[2]], &[&[0, 1], &[1, 2]]);
    for (end, want) in [(15., 10.), (25., 25.)] {
        let mut s = c.geom(&[10., 10., end], &mut work);
        snap(&mut s, &mut work).unwrap();
        assert_eq!(s.x, [10., 10., want]);
    }
}

#[test]
fn straightening_keeps_symmetric_lenses_and_omits_only_clear_label_corridors() {
    let c = Case::new(
        2,
        &[&[0], &[2, 3], &[4, 5], &[1]],
        &[&[0, 2, 4, 1], &[0, 3, 5, 1]],
    );
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut s = c.geom(&[50., 50., 40., 62., 42., 60.], &mut work);
    straighten(&mut s, &c.options, &mut work).unwrap();
    assert_eq!(s.x, [50., 50., 48., 52., 48., 52.]);
    assert!(s.keep.iter().all(|&v| v));
    let mut c = Case::new(
        4,
        &[&[0, 2], &[4, 8], &[5, 6], &[7, 9], &[1, 3]],
        &[&[0, 4, 5, 7, 1], &[2, 8, 6, 9, 3]],
    );
    c.g.kind[6] = Kind::Label;
    c.g.width[6] = 12.;
    c.options.routing = RoutingStyle::Polyline;
    for (label, want) in [(70., false), (18., true)] {
        let mut s = c.geom(
            &[10., 10., 70., 70., 8., 0., label, 8., 70., 70.],
            &mut work,
        );
        straighten(&mut s, &c.options, &mut work).unwrap();
        assert_eq!(s.keep[5], want);
        assert!(s.keep[6]);
        assert!(s.x[5] + 2. <= s.x[6] - 8.);
    }
}

#[test]
fn independent_lenses_keep_their_own_axes_when_edge_spacing_is_zero() {
    let mut case = Case::new(
        4,
        &[&[0, 1], &[4, 5, 6, 7], &[2, 3]],
        &[&[0, 4, 2], &[0, 5, 2], &[1, 6, 3], &[1, 7, 3]],
    );
    case.options.spacing.edge = 0.;
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut s = case.geom(&[0., 100., 0., 100., 1., 4., 96., 99.], &mut work);
    straighten(&mut s, &case.options, &mut work).unwrap();
    assert_eq!(&s.x[4..], &[0., 0., 100., 100.]);
    assert!(s.keep.iter().all(|&keep| keep));
}

#[test]
fn near_vertical_dummies_snap_to_the_attachment_with_clearance_and_keep_held_geometry() {
    let mut case = Case::new(4, &[&[0], &[4, 2], &[5, 3], &[1]], &[&[0, 4, 5, 1]]);
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let x = [10., 50.6, 120., 120., 50.42, 50.23];
    let mut geometry = case.geom(&x, &mut work);
    let offsets = geometry.dx.clone();
    de_jitter(&mut geometry, &case.options, &mut work).unwrap();
    assert_eq!(&geometry.x[..4], &x[..4]);
    assert_eq!(&geometry.x[4..], &[50.6, 50.6]);
    assert_eq!(geometry.dx, offsets);

    let mut geometry = case.geom(&x, &mut work);
    geometry.protected[4] = true;
    geometry.twin[5] = 0;
    de_jitter(&mut geometry, &case.options, &mut work).unwrap();
    assert_eq!(geometry.x, x);

    let x = [10., 50.6, 120., 67.4, 50.42, 50.23];
    let mut geometry = case.geom(&x, &mut work);
    assert!(geometry.movable(5).hi < x[1]);
    de_jitter(&mut geometry, &case.options, &mut work).unwrap();
    assert_eq!(&geometry.x[4..], &[x[1], x[1]]);

    case.options.spacing.node = case.options.spacing.edge;
    let x = [10., 50.6, 120., 64.4, 50.42, 50.23];
    let mut geometry = case.geom(&x, &mut work);
    let before = geometry.movable(5);
    de_jitter(&mut geometry, &case.options, &mut work).unwrap();
    assert!(before.hi < x[1]);
    assert_eq!(&geometry.x[4..], &[before.hi, before.hi]);
}

#[test]
fn late_cancellation_does_not_publish_coordinates_offsets_or_waypoint_flags() {
    struct Meter {
        checks: usize,
        stop: usize,
    }
    impl WorkControl for Meter {
        fn check(&mut self) -> Result<(), WorkError> {
            self.checks += 1;
            if self.checks == self.stop {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
        fn charge(&mut self, _: usize) -> Result<(), WorkError> {
            Ok(())
        }
    }
    let case = || Case::new(3, &[&[0], &[1, 2]], &[&[0, 1], &[0, 2]]);
    let mut meter = Meter { checks: 0, stop: 0 };
    let mut x = [10., 0., 60.];
    case().run(&mut x, &mut Work::new(&mut meter)).unwrap();
    let mut c = case();
    let mut x = [10., 0., 60.];
    let old = x;
    let mut stop = Meter {
        checks: 0,
        stop: meter.checks,
    };
    let before = (
        c.g.source_offset.clone(),
        c.g.target_offset.clone(),
        c.g.keep.clone(),
    );
    assert_eq!(
        c.run(&mut x, &mut Work::new(&mut stop)),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    assert_eq!(x, old);
    assert_eq!((c.g.source_offset, c.g.target_offset, c.g.keep), before);
}
