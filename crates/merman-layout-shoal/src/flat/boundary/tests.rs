use crate::flat::{self, BoundaryRank, Expanded, Kind, prepare_component, validate};
use crate::work::Work;
use crate::{
    CyclePolicy, Direction, EdgeSpec, Graph, NodeId, NodeSpec, NoopWorkControl, Options, OrderHint,
    PortSideHint, RankPolicy, Side, Size,
};

fn node(seq: u32) -> NodeSpec {
    NodeSpec::new(
        Size {
            width: 80.,
            height: 40.,
        },
        seq,
    )
}

fn port(seq: u32, rank: BoundaryRank, key: f64) -> NodeSpec {
    let mut port = NodeSpec::new(Size::default(), seq);
    port.boundary = Some(rank);
    port.order_hint = Some(OrderHint {
        rank: 0,
        cross_key: key,
    });
    port
}

fn edge(from: usize, to: usize) -> EdgeSpec {
    EdgeSpec::new(NodeId(from as u32).into(), NodeId(to as u32).into())
}

#[test]
fn true_ports_share_extreme_ranks_on_disconnected_islands_and_survive_both_fas_policies() {
    use BoundaryRank::{End, Start};
    let mut graph = Graph {
        nodes: vec![
            node(0),
            node(1),
            node(2),
            node(3),
            port(4, Start, 0.),
            port(5, Start, 1.),
            port(6, End, 0.),
            port(7, End, 1.),
        ],
        ..Graph::default()
    };
    graph.nodes[3].size = Size::default();
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 0),
        (0, 4),
        (5, 3),
        (2, 6),
        (7, 3),
        (1, 1),
    ];
    graph.edges.extend(pairs.map(|(a, b)| edge(a, b)));
    for cycle_policy in [CyclePolicy::SccGreedyFas, CyclePolicy::SccDfs] {
        for rank_policy in [
            RankPolicy::TightenedLongestPath,
            RankPolicy::BoundedNetworkSimplex { max_pivots: 64 },
        ] {
            let options = Options {
                cycle_policy,
                rank_policy,
                ..Options::default()
            };
            let mut control = NoopWorkControl;
            let mut work = Work::new(&mut control);
            let components = validate::components(&graph, &options, &mut work).unwrap();
            assert_eq!(components.len(), 1);
            let mut g = Expanded::default();
            let (orientation, _) =
                prepare_component(&components[0], &graph, &options, &mut work, &mut g).unwrap();
            assert_eq!(orientation.cyclic_sccs, 1);
            let flips = flat::orientations(&graph.nodes, &pairs, &options, &mut work).unwrap();
            assert_eq!(&flips[..7], orientation.reversed);
            assert_eq!(&flips[3..], [true, false, false, true, false]);
            assert_eq!(g.layers[0], [4, 5]);
            assert_eq!(g.layers.last().unwrap(), &[6, 7]);
            assert_eq!(
                &g.kind[3..8],
                [
                    Kind::Node,
                    Kind::StartPort,
                    Kind::StartPort,
                    Kind::EndPort,
                    Kind::EndPort
                ]
            );
            let last = g.rank[6];
            assert!(g.rank[..4].iter().all(|&rank| rank > 0 && rank < last));
            assert_eq!(g.rank[..4].iter().min(), Some(&1));
            assert_eq!(g.rank[..4].iter().max(), Some(&(last - 1)));
        }
    }
}

#[test]
fn boundary_normalization_preserves_large_minimum_spans_and_rejects_same_face_arcs() {
    let mut graph = Graph {
        nodes: vec![
            port(0, BoundaryRank::Start, 0.),
            node(1),
            port(2, BoundaryRank::End, 0.),
        ],
        edges: vec![edge(1, 0), edge(2, 1), edge(2, 0)],
        ..Graph::default()
    };
    graph.edges[0].min_rank_span = u32::MAX;
    graph.edges[1].min_rank_span = u32::MAX;
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut g = Expanded::default();
    let c = validate::components(&graph, &options, &mut work).unwrap();
    prepare_component(&c[0], &graph, &options, &mut work, &mut g).unwrap();
    let span = u64::from(u32::MAX);
    assert_eq!(g.logical_rank, [0, span, 2 * span]);
    graph.nodes[2].boundary = Some(BoundaryRank::Start);
    assert!(matches!(
        flat::orientations(&graph.nodes, &[(2, 0)], &options, &mut work),
        Err(crate::LayoutError::InvalidInput(_))
    ));
}

#[test]
fn fixed_port_hints_and_lateral_pins_survive_crossing_reduction() {
    let mut graph = Graph::default();
    graph.nodes.extend((0..3).map(node));
    for id in 0..3 {
        graph.nodes[id].pin = id as i8 - 1;
        graph
            .nodes
            .push(port(3 + id as u32, BoundaryRank::Start, -(id as f64)));
    }
    graph
        .nodes
        .extend((0..3).map(|i| port(i + 6, BoundaryRank::End, f64::from(i))));
    for id in 0..3 {
        graph.edges.extend([edge(3 + id, id), edge(id, 6 + id)]);
    }
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let c = validate::components(&graph, &options, &mut work).unwrap();
    let mut g = Expanded::default();
    let (_, crossings) = prepare_component(&c[0], &graph, &options, &mut work, &mut g).unwrap();
    assert_eq!(crossings, 3);
    assert_eq!(g.layers, [vec![5, 4, 3], vec![0, 1, 2], vec![6, 7, 8]]);
    for node in &mut graph.nodes[3..6] {
        node.order_hint = None;
    }
    let c = validate::components(&graph, &options, &mut work).unwrap();
    let (_, crossings) = prepare_component(&c[0], &graph, &options, &mut work, &mut g).unwrap();
    assert_eq!(crossings, 0);
    assert_eq!(g.layers[0], [3, 4, 5]);
}

fn fan(direction: Direction) -> Graph {
    let mut graph = Graph {
        direction,
        nodes: vec![
            node(0),
            port(1, BoundaryRank::Start, 0.),
            port(2, BoundaryRank::Start, 1.),
            port(3, BoundaryRank::End, 0.),
            port(4, BoundaryRank::End, 1.),
        ],
        edges: vec![edge(1, 0), edge(0, 2), edge(0, 3), edge(4, 0)],
        ..Graph::default()
    };
    let contact = |side, offset| PortSideHint::FixedOffset {
        side: flat::physical_side(direction, side),
        offset,
    };
    graph.edges[0].target_port = contact(Side::Top, -1.);
    graph.edges[1].source_port = contact(Side::Top, 1.);
    graph.edges[2].source_port = contact(Side::Bottom, -1.);
    graph.edges[3].target_port = contact(Side::Bottom, 1.);
    graph
}

#[test]
fn aligned_boundary_routes_use_the_settled_attachment_and_port_gap_in_four_directions() {
    let mut options = Options::default();
    options.spacing.rank = 100.;
    options.spacing.port_sep = 13.;
    options.spacing.edge = 10.;
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        let graph = fan(direction);
        let drawing = crate::layout(&graph, &options).unwrap();
        let horizontal = matches!(direction, Direction::Right | Direction::Left);
        let cross = |x, y| if horizontal { y } else { x };
        let center = drawing.nodes[0].center();
        let axis = cross(center.x, center.y);
        for (i, geometry) in drawing.edges.iter().enumerate() {
            let at = axis + if i % 2 == 0 { -1. } else { 1. };
            assert!(
                geometry.points.iter().all(|p| cross(p.x, p.y) == at),
                "{direction:?} {geometry:?}"
            );
            let a = geometry.points.first().unwrap();
            let b = geometry.points.last().unwrap();
            assert_eq!(
                (a.x - b.x).abs() + (a.y - b.y).abs(),
                options.spacing.port_sep
            );
            let p = drawing.nodes[i + 1].center();
            assert_eq!(cross(p.x, p.y), at);
            assert_eq!(geometry.reversed_for_ranking, i == 1 || i == 3);
        }
    }
}

#[test]
fn a_boundary_row_with_reversed_attachments_keeps_its_complete_hint_order() {
    let mut graph = fan(Direction::Down);
    graph.edges[0].target_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: 1.,
    };
    graph.edges[1].source_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: -1.,
    };
    let options = Options::default();
    let drawing = crate::layout(&graph, &options).unwrap();
    assert!(drawing.nodes[2].x - drawing.nodes[1].x >= options.spacing.edge);
    let axis = drawing.nodes[0].center().x;
    assert_eq!(
        (drawing.nodes[3].x, drawing.nodes[4].x),
        (axis - 1., axis + 1.)
    );
}

#[test]
fn free_attachments_slide_before_boundary_vertices_take_their_final_coordinates() {
    let mut graph = fan(Direction::Down);
    graph.nodes.truncate(3);
    graph.edges.truncate(2);
    graph.nodes[0].size.width = 160.;
    graph.nodes[0].port_span = Some(120.);
    graph.edges[0].target_port = PortSideHint::Auto;
    graph.edges[1].source_port = PortSideHint::Auto;
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let components = validate::components(&graph, &options, &mut work).unwrap();
    let c = &components[0];
    let mut g = Expanded::default();
    let (o, _) = prepare_component(c, &graph, &options, &mut work, &mut g).unwrap();
    let mut x = flat::position::assign(&mut g, c, Some(&o), &graph, &options, &mut work).unwrap();
    assert_eq!(g.target_offset, [-8., 8.]);
    x.copy_from_slice(&[0., -100., 100.]);
    flat::refine::refine(&mut g, c, &o, &graph, &options, &mut x, &mut work).unwrap();
    assert_eq!(g.target_offset, [-20., 20.]);
    assert_eq!(x, [0., -20., 20.]);
    assert!(g.keep.iter().all(|keep| *keep));
}
