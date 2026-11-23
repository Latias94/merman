use super::*;
use crate::{EdgeSpec, Endpoint, LayoutScratch, NodeId, NoopWorkControl, WorkControl, WorkError};

fn graph(n: u32) -> Graph {
    let mut graph = Graph::default();
    graph.nodes = (0..n)
        .map(|seq| {
            NodeSpec::new(
                Size {
                    width: 45.0,
                    height: 25.0,
                },
                seq,
            )
        })
        .collect();
    graph
}
fn edge(from: u32, to: u32) -> EdgeSpec {
    EdgeSpec::new(Endpoint::Node(NodeId(from)), Endpoint::Node(NodeId(to)))
}

#[test]
fn loop_margins_alternate_and_components_follow_their_earliest_declaration() {
    let mut g = graph(5);
    g.nodes[3].seq = 0;
    g.nodes[0].seq = 30;
    g.edges = vec![
        edge(0, 0),
        edge(0, 0),
        edge(0, 0),
        edge(1, 0),
        edge(3, 2),
        edge(2, 2),
    ];
    let c = validate::components(
        &g,
        &Options::default(),
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert_eq!(
        c.iter().map(|c| c.ids.clone()).collect::<Vec<_>>(),
        [vec![3, 2], vec![1, 0], vec![4]]
    );
    assert_eq!(c[0].margin_left, [0.0, 0.0]);
    assert!(c[0].margin_right[1] > 0.0);
    assert_eq!(c[1].margin_right[1], 2.0 * c[1].margin_left[1]);
    assert_eq!(c[1].nodes[1].size, g.nodes[0].size);
    assert_eq!(
        c[1].loops
            .iter()
            .map(|l| (l.edge, l.node, l.right))
            .collect::<Vec<_>>(),
        [(0, 1, true), (1, 1, false), (2, 1, true)]
    );
    assert_eq!(c[1].loops[2].reach, c[1].margin_right[1]);
}

#[derive(Default)]
struct Meter {
    used: usize,
    left: Option<usize>,
}
impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }
    fn charge(&mut self, n: usize) -> Result<(), WorkError> {
        if let Some(left) = self.left.as_mut() {
            *left = left.checked_sub(n).ok_or(WorkError::Interrupted)?;
        }
        self.used += n;
        Ok(())
    }
}

#[test]
fn sparse_selective_ranks_avoid_global_doubling_and_preserve_reused_scratch() {
    let mut g = graph(100);
    g.edges = (0..99).map(|i| edge(i, i + 1)).collect();
    g.edges[50].label = Some(Size {
        width: 35.0,
        height: 17.0,
    });
    let o = Options::default();
    let mut unlimited = NoopWorkControl;
    let mut work = Work::new(&mut unlimited);
    let mut scratch = LayoutScratch::default();
    let mut expanded = Expanded::default();
    let c = validate::components(&g, &o, &mut work).unwrap();
    let (orientation, cross) = prepare_component(&c[0], &g, &o, &mut work, &mut expanded).unwrap();
    assert_eq!(
        (orientation.cyclic_sccs, cross, expanded.layers.len()),
        (0, 0, 101)
    );
    let label = expanded.label_vertex[50].unwrap() as usize;
    assert_eq!(
        (expanded.width[label], expanded.height[label]),
        (35.0, 17.0)
    );
    assert_eq!(expanded.kind[label], Kind::Label);
    // A second, smaller solve must not retain semantic rows or chains of the previous call.
    let g = graph(1);
    let c = validate::components(&g, &o, &mut work).unwrap();
    prepare_component(&c[0], &g, &o, &mut work, &mut expanded).unwrap();
    assert_eq!(expanded.layers, [vec![0]]);
    assert!(expanded.chains.is_empty());
    scratch.buffers.expanded = expanded;
    assert!(scratch.retained_bytes() > 0);
}

#[test]
fn stress_scc_order_controls_preserve_bridge_directions_and_work() {
    for n in [25, 50, 75, 100] {
        let mut g = graph(n);
        for i in 0..n {
            g.edges
                .push(edge(i, if i % 5 == 4 { i - 4 } else { i + 1 }));
            if i + 5 < n {
                g.edges.push(edge(i, i + 5));
            }
        }
        let mut runs = Vec::new();
        for _ in 0..2 {
            let mut meter = Meter::default();
            let mut work = Work::new(&mut meter);
            let options = Options::default();
            let c = validate::components(&g, &options, &mut work).unwrap();
            let mut expanded = Expanded::default();
            let (orientation, _) =
                prepare_component(&c[0], &g, &options, &mut work, &mut expanded).unwrap();
            assert_eq!(orientation.cyclic_sccs, n as usize / 5);
            let mut reversed = Vec::new();
            for (edge, reverse) in c[0].links.iter().zip(orientation.reversed) {
                if reverse {
                    assert_eq!(edge.from / 5, edge.to / 5);
                    reversed.push((edge.from, edge.to));
                }
            }
            reversed.sort_unstable();
            work.finish().unwrap();
            runs.push((expanded.rank.len() - n as usize, reversed, meter.used));
            g.edges.reverse();
        }
        assert_eq!(runs[0].0, runs[1].0);
        assert_eq!(runs[0].1, runs[1].1);
        let (a, b) = (runs[0].2, runs[1].2);
        eprintln!("SCC {n}, dummies {}, layout-stage work {a}/{b}", runs[0].0);
        assert!((a.max(b) as f64) / (a.min(b) as f64) < 1.25);
    }
}

#[test]
fn cancellation_during_topology_delivers_no_partial_result() {
    let mut graph = graph(100);
    graph.edges = (0..100).map(|v| edge(v, (v + 1) % 100)).collect();
    let options = Options::default();
    let components =
        validate::components(&graph, &options, &mut Work::new(&mut NoopWorkControl)).unwrap();
    for budget in [0, 10, 300] {
        let mut meter = Meter {
            left: Some(budget),
            ..Default::default()
        };
        let result = prepare_component(
            &components[0],
            &graph,
            &options,
            &mut Work::new(&mut meter),
            &mut Expanded::default(),
        );
        assert!(matches!(
            result,
            Err(LayoutError::Work(WorkError::Interrupted))
        ));
    }
}

#[test]
fn compressed_adjacency_preserves_occurrences_on_empty_and_reused_ranges() {
    let mut unlimited = NoopWorkControl;
    let mut work = Work::new(&mut unlimited);
    let mut adjacency = Csr::new(5, [4, 1, 2, 1, 4, 2].into_iter(), &mut work).unwrap();
    assert_eq!(adjacency.at(0), []);
    assert_eq!(adjacency.at(1), [1, 3]);
    assert_eq!(adjacency.at(2), [2, 5]);
    assert_eq!(adjacency.at(3), []);
    assert_eq!(adjacency.at(4), [0, 4]);
    adjacency
        .rebuild(2, [0, 0, 0].into_iter(), &mut work)
        .unwrap();
    assert_eq!(adjacency.at(0), [0, 1, 2]);
    assert!(adjacency.at(1).is_empty());
    assert!(matches!(
        adjacency.rebuild(2, [2].into_iter(), &mut work),
        Err(LayoutError::InvalidInput(_))
    ));
}

#[test]
fn invalid_port_span_marker_and_end_label_fail_before_cycle_removal() {
    let mut g = graph(2);
    let options = Options::default();
    let mut unlimited = NoopWorkControl;
    let mut work = Work::new(&mut unlimited);
    g.edges = vec![edge(0, 1), edge(1, 0)];
    g.edges[1].source_port = crate::PortSideHint::FixedOffset {
        side: crate::Side::Top,
        offset: 24.0,
    };
    assert!(matches!(
        validate::components(&g, &options, &mut work),
        Err(LayoutError::InvalidInput(_))
    ));
    g.edges[1].source_port = crate::PortSideHint::Auto;
    g.edges[1].start_marker.width = f64::NAN;
    assert!(matches!(
        validate::components(&g, &options, &mut work),
        Err(LayoutError::InvalidInput(_))
    ));
    g.edges[1].start_marker.width = 10.0;
    g.edges[0].end_label = Some(Size {
        width: -1.0,
        height: 3.0,
    });
    assert!(matches!(
        validate::components(&g, &options, &mut work),
        Err(LayoutError::InvalidInput(_))
    ));
}
