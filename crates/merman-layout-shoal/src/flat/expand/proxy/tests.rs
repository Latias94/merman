use crate::flat::{Expanded, physical_side, prepare_component, validate};
use crate::work::Work;
use crate::{
    CyclePolicy, Direction, EdgeSpec, Graph, NodeId, NodeSpec, NoopWorkControl, Options,
    PortSideHint, Side, Size,
};

fn fixed_graph(direction: Direction, lanes: &[(f64, f64, bool)]) -> Graph {
    let mut graph = Graph {
        direction,
        ..Graph::default()
    };
    for seq in 0..2 {
        let mut node = NodeSpec::new(
            Size {
                width: 140.,
                height: 140.,
            },
            seq,
        );
        node.group_proxy = true;
        graph.nodes.push(node);
    }
    for &(from, to, reverse) in lanes {
        let mut ends = [
            (
                NodeId(0),
                PortSideHint::FixedOffset {
                    side: physical_side(direction, Side::Bottom),
                    offset: from,
                },
            ),
            (
                NodeId(1),
                PortSideHint::FixedOffset {
                    side: physical_side(direction, Side::Top),
                    offset: to,
                },
            ),
        ];
        if reverse {
            ends.swap(0, 1);
        }
        let mut edge = EdgeSpec::new(ends[0].0.into(), ends[1].0.into());
        edge.source_port = ends[0].1;
        edge.target_port = ends[1].1;
        graph.edges.push(edge);
    }
    graph
}

fn expanded(graph: &Graph) -> Expanded {
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut g = Expanded::default();
    let c = validate::components(graph, &options, &mut work).unwrap();
    prepare_component(&c[0], graph, &options, &mut work, &mut g).unwrap();
    g
}

#[test]
fn separated_proxy_lanes_remain_straight_for_parallel_and_antiparallel_semantic_edges() {
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        for (proxies, reverse) in [
            (2, false),
            (2, true),
            (1, false),
            (1, true),
            (0, false),
            (0, true),
        ] {
            let mut graph = fixed_graph(direction, &[(-23., -23., false), (23., 23., reverse)]);
            if proxies != 2 {
                graph.nodes[proxies].group_proxy = false;
                assert_eq!(expanded(&graph).layers.len(), 2);
            }
            for cycle_policy in [CyclePolicy::SccGreedyFas, CyclePolicy::SccDfs] {
                let options = Options {
                    cycle_policy,
                    ..Options::default()
                };
                let result = crate::layout(&graph, &options).unwrap();
                assert_eq!(result.stats.dummy_nodes, 0);
                assert_eq!(result.stats.topological_crossings, 0);
                assert_eq!(result.edges[1].reversed_for_ranking, reverse);
                let cross = |p: &crate::RoutePoint| match direction {
                    Direction::Left | Direction::Right => p.y,
                    Direction::Up | Direction::Down => p.x,
                };
                for edge in &result.edges {
                    let axis = cross(&edge.points[0]);
                    assert!(edge.points.iter().all(|p| cross(p) == axis));
                }
                assert_eq!(
                    cross(&result.edges[1].points[0]) - cross(&result.edges[0].points[0]),
                    46.
                );
            }
        }
    }
}

#[test]
fn any_shared_reversed_close_unheld_or_labeled_lane_keeps_the_parallel_tier() {
    let template = || {
        fixed_graph(
            Direction::Down,
            &[(-23., -23., false), (23., 23., true), (0., 0., false)],
        )
    };
    assert_eq!(expanded(&template()).layers.len(), 2);
    let fixed = |offset| PortSideHint::FixedOffset {
        side: Side::Top,
        offset,
    };
    for lost in [
        fixed(-23.),
        fixed(-22.),
        fixed(-30.),
        PortSideHint::Auto,
        PortSideHint::FixedOffset {
            side: Side::Left,
            offset: 0.,
        },
    ] {
        let mut graph = template();
        graph.edges[2].target_port = lost;
        assert_eq!(expanded(&graph).layers.len(), 3, "{lost:?}");
    }
    let mut graph = template();
    graph.edges[2].end_marker.width = 44.;
    assert_eq!(expanded(&graph).layers.len(), 3);
    let mut graph = template();
    graph.edges[2].label = Some(Size {
        width: 12.,
        height: 6.,
    });
    assert_eq!(expanded(&graph).layers.len(), 3);
    let mut graph = template();
    graph.nodes[0].group_proxy = false;
    graph.nodes[1].group_proxy = false;
    assert_eq!(expanded(&graph).layers.len(), 3);
    let mut graph = template();
    graph.edges[0].min_rank_span = 4;
    assert_eq!(expanded(&graph).layers.len(), 3);
}

#[test]
fn a_required_tier_for_another_bundle_preserves_the_separated_proxy_lanes() {
    let mut graph = fixed_graph(Direction::Down, &[(-23., -23., false), (23., 23., true)]);
    for seq in 2..4 {
        graph.nodes.push(NodeSpec::new(
            Size {
                width: 40.,
                height: 30.,
            },
            seq,
        ));
    }
    for (from, to) in [(2, 3), (2, 3), (0, 3)] {
        graph
            .edges
            .push(EdgeSpec::new(NodeId(from).into(), NodeId(to).into()));
    }
    let g = expanded(&graph);
    assert_eq!(g.layers.len(), 3);
    assert_eq!(g.separated_proxies, [(0, 1)]);
    let result = crate::layout(&graph, &Options::default()).unwrap();
    let mut reverse = result.edges[1].points.clone();
    reverse.reverse();
    let forward = &result.edges[0].points;
    assert_eq!(forward.len(), reverse.len());
    for (a, b) in forward.iter().zip(reverse) {
        assert_eq!(a.y, b.y);
        assert_eq!(b.x - a.x, 46.);
    }
}
