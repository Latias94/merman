use merman_layout_shoal::{
    Direction, EdgeSpec, Graph, LayoutError, LayoutScratch, NodeId, NodeSpec, Options,
    PackingPolicy, Point, PortSideHint, Side, Size, WidthPolicy, WorkControl, WorkError, layout,
    layout_with_control,
};

const DIRECTIONS: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::Right,
    Direction::Left,
];
const BALANCED: PackingPolicy = PackingPolicy::BalancedAspect { target_ratio: 1.6 };

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn graph(sizes: &[(f64, f64)], edges: &[(u32, u32)]) -> Graph {
    let mut graph = Graph::default();
    graph.nodes = sizes
        .iter()
        .enumerate()
        .map(|(seq, &(w, h))| NodeSpec::new(size(w, h), seq as u32))
        .collect();
    graph.edges = edges
        .iter()
        .map(|&(a, b)| EdgeSpec::new(NodeId(a).into(), NodeId(b).into()))
        .collect();
    graph
}

fn options(packing: PackingPolicy, gap: f64) -> Options {
    let mut options = Options::default();
    options.packing = packing;
    options.spacing.component = gap;
    options
}

#[test]
fn measured_disconnected_fixture_fits_the_short_island_under_the_fork() {
    // Node measurements and topology from the disconnected Flowchart gallery fixture.
    let graph = graph(&[(152.0, 45.0); 7], &[(0, 1), (1, 2), (3, 4), (3, 5)]);
    let shelf = layout(&graph, &Options::default()).unwrap();
    assert_eq!(shelf.bounds, size(752.0, 231.0));
    let packed = layout(&graph, &options(BALANCED, 50.0)).unwrap();
    assert_eq!(packed.bounds, size(550.0, 233.0));
    assert_eq!((packed.nodes[6].x, packed.nodes[6].y), (202.0, 188.0));
    assert_eq!(packed.stats, shelf.stats);
}

#[test]
fn skyline_fills_low_spans_in_authored_order() {
    let mut graph = graph(
        &[
            (80.0, 200.0),
            (80.0, 40.0),
            (80.0, 40.0),
            (80.0, 40.0),
            (80.0, 40.0),
            (80.0, 40.0),
        ],
        &[],
    );
    graph.nodes.swap(0, 4);
    graph.nodes.swap(1, 5);
    let shelf = layout(&graph, &options(Options::default().packing, 10.0)).unwrap();
    assert_eq!(shelf.bounds, size(530.0, 200.0));
    let options = options(BALANCED, 10.0);
    let packed = layout(&graph, &options).unwrap();
    assert_eq!(packed.bounds, size(260.0, 200.0));
    let authored_positions = [
        (0.0, 0.0),
        (90.0, 0.0),
        (180.0, 0.0),
        (90.0, 50.0),
        (180.0, 50.0),
        (90.0, 100.0),
    ];
    for (node, rect) in graph.nodes.iter().zip(&packed.nodes) {
        assert_eq!((rect.x, rect.y), authored_positions[node.seq as usize]);
    }
    assert_eq!(layout(&graph, &options).unwrap(), packed);
}

#[test]
fn target_ratio_describes_physical_width_and_height_in_every_flow() {
    let mut graph = graph(&[(20.0, 20.0); 12], &[]);
    for direction in DIRECTIONS {
        graph.direction = direction;
        for (packing, expected) in [
            (BALANCED, size(104.0, 76.0)),
            (
                PackingPolicy::BalancedAspect {
                    target_ratio: 0.625,
                },
                size(76.0, 104.0),
            ),
        ] {
            assert_eq!(
                layout(&graph, &options(packing, 8.0)).unwrap().bounds,
                expected
            );
        }
    }
}

#[test]
fn complete_component_footprints_translate_together_and_keep_the_gap() {
    let mut graph = graph(
        &[
            (83.0, 96.0),
            (32.0, 35.0),
            (81.0, 23.0),
            (150.0, 51.0),
            (82.0, 28.0),
            (40.0, 22.0),
            (90.0, 40.0),
            (42.0, 38.0),
            (88.0, 31.0),
        ],
        &[(0, 3), (4, 6), (2, 5), (0, 0)],
    );
    for edge in &mut graph.edges[..2] {
        edge.source_port = PortSideHint::FixedOffset {
            side: Side::Right,
            offset: 1.0,
        };
        edge.target_port = PortSideHint::FixedOffset {
            side: Side::Top,
            offset: -2.0,
        };
        edge.label = Some(size(59.0, 14.0));
        edge.start_label = Some(size(31.0, 13.0));
        edge.end_label = Some(size(19.0, 21.0));
    }
    let components: [(&[usize], &[usize]); 6] = [
        (&[0, 3], &[0, 3]),
        (&[1], &[]),
        (&[2, 5], &[2]),
        (&[4, 6], &[1]),
        (&[7], &[]),
        (&[8], &[]),
    ];
    let gap = 12.0;
    for direction in DIRECTIONS {
        graph.direction = direction;
        let strip = layout(&graph, &options(PackingPolicy::Strip, gap)).unwrap();
        let packed = layout(&graph, &options(BALANCED, gap)).unwrap();
        assert_ne!(packed.bounds, strip.bounds, "{direction:?}");
        assert_eq!(packed.stats, strip.stats);
        let mut footprints = Vec::new();
        for (nodes, edges) in components {
            let i = nodes[0];
            let dx = packed.nodes[i].x - strip.nodes[i].x;
            let dy = packed.nodes[i].y - strip.nodes[i].y;
            let translated = |from: Point, to: Point| {
                assert!((from.x + dx - to.x).abs() < 1e-9, "{direction:?}");
                assert!((from.y + dy - to.y).abs() < 1e-9, "{direction:?}");
            };
            let (mut left, mut top) = (f64::INFINITY, f64::INFINITY);
            let (mut right, mut bottom) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
            let mut include = |p: Point, size: Size| {
                left = left.min(p.x - size.width * 0.5);
                top = top.min(p.y - size.height * 0.5);
                right = right.max(p.x + size.width * 0.5);
                bottom = bottom.max(p.y + size.height * 0.5);
            };
            for &i in nodes {
                let (a, b) = (strip.nodes[i], packed.nodes[i]);
                assert_eq!((a.width, a.height), (b.width, b.height));
                translated(a.center(), b.center());
                include(b.center(), size(b.width, b.height));
            }
            for &i in edges {
                let (a, b) = (&strip.edges[i], &packed.edges[i]);
                let spec = &graph.edges[i];
                assert_eq!(a.ports, b.ports);
                assert_eq!(a.reversed_for_ranking, b.reversed_for_ranking);
                assert_eq!(a.points.len(), b.points.len());
                for (a, b) in a.points.iter().zip(&b.points) {
                    let (from, to) = (Point { x: a.x, y: a.y }, Point { x: b.x, y: b.y });
                    assert_eq!((a.axis, a.kind), (b.axis, b.kind));
                    translated(from, to);
                    include(to, Size::default());
                }
                for (a, b, size) in [
                    (a.label_center, b.label_center, spec.label),
                    (a.start_label_center, b.start_label_center, spec.start_label),
                    (a.end_label_center, b.end_label_center, spec.end_label),
                ] {
                    if let Some(size) = size {
                        translated(a.unwrap(), b.unwrap());
                        include(b.unwrap(), size);
                    } else {
                        assert_eq!((a, b), (None, None));
                    }
                }
            }
            for &(other_left, other_top, other_right, other_bottom) in &footprints {
                assert!(
                    right + gap <= other_left + 1e-9
                        || other_right + gap <= left + 1e-9
                        || bottom + gap <= other_top + 1e-9
                        || other_bottom + gap <= top + 1e-9,
                    "{direction:?} {nodes:?}"
                );
            }
            assert!(left >= -1e-9 && top >= -1e-9);
            footprints.push((left, top, right, bottom));
        }
        let width = footprints.iter().map(|b| b.2).fold(0.0, f64::max);
        let height = footprints.iter().map(|b| b.3).fold(0.0, f64::max);
        assert!((packed.bounds.width - width).abs() < 1e-9);
        assert!((packed.bounds.height - height).abs() < 1e-9);
    }
}

#[test]
fn fragmented_skylines_have_bounded_work_and_keep_the_shelf_on_exhaustion() {
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
    }

    let sizes: Vec<_> = (0..1000)
        .map(|i| (16.0 + f64::from(i * 17 % 5), 10.0 + f64::from(i * 73 % 100)))
        .collect();
    let graph = graph(&sizes, &[]);
    let run = |packing| {
        let mut meter = Meter::default();
        let output = layout_with_control(
            &graph,
            &options(packing, 8.0),
            &mut LayoutScratch::default(),
            &mut meter,
        )
        .unwrap();
        (output, meter.0)
    };
    let (shelf, shelf_work) = run(Options::default().packing);
    assert_eq!(shelf.bounds, size(1658.0, 1858.0));
    let (packed, packed_work) = run(BALANCED);
    assert_eq!(packed, shelf);
    assert!((sizes.len()..=128 * sizes.len()).contains(&(packed_work - shelf_work)));
}

#[test]
fn balanced_aspect_requires_a_positive_finite_ratio_and_fit_width_still_wraps() {
    let graph = graph(&[(20.0, 20.0); 6], &[]);
    for target_ratio in [0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let options = options(PackingPolicy::BalancedAspect { target_ratio }, 8.0);
        assert_eq!(
            layout(&graph, &options),
            Err(LayoutError::InvalidInput("invalid aspect target"))
        );
    }
    let mut options = options(PackingPolicy::Strip, 8.0);
    options.width_policy = WidthPolicy::FitWidth {
        max_width: 48.0,
        allow_turn: false,
        turn_breakpoint: 0.0,
        fold_wide_ranks: false,
    };
    let shelf = layout(&graph, &options).unwrap();
    assert_eq!(shelf.bounds, size(48.0, 76.0));
    options.packing = BALANCED;
    assert_eq!(layout(&graph, &options).unwrap(), shelf);
}
