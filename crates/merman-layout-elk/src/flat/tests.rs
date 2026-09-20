use super::*;

fn graph(algorithm: Algorithm) -> Graph {
    Graph {
        id: "flat".into(),
        nodes: [(40.0, 20.0), (60.0, 30.0), (30.0, 50.0)]
            .into_iter()
            .enumerate()
            .map(|(i, (width, height))| Node {
                id: format!("n{i}"),
                kind: NodeKind::Leaf,
                width,
                height,
                parent: None,
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                label: None,
            })
            .collect(),
        edges: (0..2)
            .map(|i| Edge {
                id: format!("e{i}"),
                source: format!("n{i}"),
                target: format!("n{}", i + 1),
                label: (i == 0).then_some(Label {
                    width: 10.0,
                    height: 6.0,
                }),
                minlen: 1,
                inside_self_loops_yo: false,
            })
            .collect(),
        options: LayoutOptions {
            algorithm,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-7,
        "actual={actual}, expected={expected}"
    );
}
// Real elkjs 0.9.3 output, with Mermaid 12 root options and normal node micro layout enabled.
// Values are centers at the adapter boundary; edge labels remain top-left based.
#[test]
fn root_provider_projection_matches_elkjs() {
    let cases = [
        (
            Algorithm::Box,
            [
                [35.0, 25.0, 40.0, 20.0],
                [45.0, 95.0, 60.0, 30.0],
                [85.0, 40.0, 30.0, 50.0],
            ],
            [&[][..], &[][..]],
            [0.0, 0.0],
        ),
        (
            Algorithm::Rectpacking,
            [
                [45.0, 25.0, 60.0, 20.0],
                [45.0, 65.0, 60.0, 30.0],
                [45.0, 120.0, 60.0, 50.0],
            ],
            [&[][..], &[][..]],
            [0.0, 0.0],
        ),
        (
            Algorithm::Force,
            [
                [151.47093870588708, 279.216981926131, 40.0, 20.0],
                [80.00000000000001, 167.50598531365765, 60.0, 30.0],
                [207.22597011872534, 75.0, 30.0, 50.0],
            ],
            [
                &[
                    [145.07309598017758, 269.216981926131],
                    [89.59676408856426, 182.50598531365765],
                ][..],
                &[
                    [100.62990351716329, 152.50598531365765],
                    [192.22597011872534, 85.90649792970717],
                ][..],
            ],
            [120.73546935294354, 212.36148361989433],
        ),
        (
            Algorithm::Stress,
            [
                [70.0, 265.115660991585, 40.0, 20.0],
                [98.21365661832216, 169.1637616742679, 60.0, 30.0],
                [131.91773094564053, 75.0, 30.0, 50.0],
            ],
            [
                &[
                    [72.94039584615395, 255.115660991585],
                    [93.80306284909123, 184.1637616742679],
                ][..],
                &[
                    [103.5826128486246, 154.1637616742679],
                    [122.9694705618031, 100.0],
                ][..],
            ],
            [89.10682830916109, 222.13971133292648],
        ),
        (
            Algorithm::MrTree,
            [
                [70.0, 50.0, 40.0, 20.0],
                [70.0, 95.0, 60.0, 30.0],
                [70.0, 155.0, 30.0, 50.0],
            ],
            [
                &[[70.0, 60.0], [70.0, 63.0], [70.0, 80.0]][..],
                &[[70.0, 110.0], [70.0, 113.0], [70.0, 130.0]][..],
            ],
            [0.0, 0.0],
        ),
        (
            Algorithm::Radial,
            [
                [149.9999435957951, 26.999999999999993, 40.0, 20.0],
                [82.9179042708014, 27.0, 60.0, 30.0],
                [27.0, 80.58184277748595, 30.0, 50.0],
            ],
            [
                &[
                    [129.9999435957951, 26.999999999999996],
                    [112.9179042708014, 27.0],
                ][..],
                &[
                    [67.26393418348565, 42.0],
                    [42.00000000000001, 66.20849219964659],
                ][..],
            ],
            [0.0, 0.0],
        ),
        (
            Algorithm::SporeOverlap,
            [
                [28.0, 18.0, 40.0, 20.0],
                [86.0, 47.0, 60.0, 30.0],
                [33.0, 82.33333333333333, 30.0, 50.0],
            ],
            [
                &[[48.0, 28.0], [56.0, 32.0]][..],
                &[[63.5, 62.0], [48.0, 72.33333333333333]][..],
            ],
            [12.0, 12.0],
        ),
    ];
    for (algorithm, nodes, routes, label) in cases {
        let result = super::super::layout(&graph(algorithm)).unwrap();
        for (actual, expected) in result.nodes.iter().zip(nodes) {
            for (a, e) in [actual.x, actual.y, actual.width, actual.height]
                .into_iter()
                .zip(expected)
            {
                close(a, e);
            }
        }
        for (actual, expected) in result.edges.iter().zip(routes) {
            assert_eq!(actual.points.len(), expected.len(), "{algorithm:?}");
            for (a, e) in actual.points.iter().zip(expected) {
                close(a.x, e[0]);
                close(a.y, e[1]);
            }
        }
        close(result.edges[0].labels[0].x, label[0]);
        close(result.edges[0].labels[0].y, label[1]);
    }
}

#[test]
fn provider_errors_preserve_work_classification() {
    use source_port::algorithms::{box_layout, force, rectpacking, stress};
    let interrupted = WorkError::Interrupted;
    for error in [
        Error::Box(box_layout::Error::Work(interrupted)),
        Error::Rectpacking(rectpacking::Error::Box(box_layout::Error::Work(
            interrupted,
        ))),
        Error::Force(force::Error::Work(interrupted)),
        Error::Stress(stress::Error::Force(force::Error::Work(interrupted))),
        Error::MrTree(mrtree::Error::Work(interrupted)),
        Error::Radial(radial::Error::Work(interrupted)),
        Error::SporeOverlap(spore_overlap::Error::Work(interrupted)),
    ] {
        assert_eq!(error.work_error(), Some(interrupted));
    }
    assert_eq!(Error::Radial(radial::Error::MissingRoot).work_error(), None);
}

#[test]
fn additional_providers_do_not_flatten_hierarchy_or_enter_layered_diagnostics() {
    for algorithm in [
        Algorithm::Box,
        Algorithm::Rectpacking,
        Algorithm::Force,
        Algorithm::Stress,
        Algorithm::MrTree,
        Algorithm::Radial,
        Algorithm::SporeOverlap,
    ] {
        let mut graph = graph(algorithm);
        assert!(matches!(
            SourcePhaseDiagnostics::from_graph(&graph),
            Err(Error::LayeredDiagnosticsRequired)
        ));
        graph.nodes[0].kind = NodeKind::Group;
        graph.nodes[1].parent = Some("n0".into());
        assert!(matches!(
            super::super::layout(&graph),
            Err(Error::NonLayeredHierarchy)
        ));
    }
}

#[test]
fn force_and_stress_require_operation_authority_for_zero_seed() {
    let seed = ElkOperationSeed::from_operation_seed(NonZeroU64::new(987).unwrap());
    for algorithm in [Algorithm::Force, Algorithm::Stress] {
        let mut graph = graph(algorithm);
        graph.options.layered.random_seed = 0;
        assert!(matches!(
            super::super::layout(&graph),
            Err(Error::RandomSeed(_))
        ));
        let a = layout_with_operation_seed(&graph, seed).unwrap();
        assert_eq!(a, layout_with_operation_seed(&graph, seed).unwrap());
        assert_eq!(graph.options.layered.random_seed, 0);
    }
    // Packing does not use randomness and must not require authority for an unused option.
    let mut graph = graph(Algorithm::Box);
    graph.options.layered.random_seed = 0;
    assert!(super::super::layout(&graph).is_ok());
}

#[test]
fn source_failure_does_not_fall_back_to_layered() {
    let mut graph = graph(Algorithm::Radial);
    graph.edges.push(Edge {
        id: "cycle".into(),
        source: "n2".into(),
        target: "n0".into(),
        label: None,
        minlen: 1,
        inside_self_loops_yo: false,
    });
    assert!(matches!(
        super::super::layout(&graph),
        Err(Error::Radial(radial::Error::MissingRoot))
    ));
}
