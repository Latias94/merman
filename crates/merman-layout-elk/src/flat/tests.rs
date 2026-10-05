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
                container: Default::default(),
                label_text: None,
                width,
                height,
                parent: None,
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                port_alignment: None,
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
                terminal_labels: Vec::new(),
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

#[test]
fn container_sizing_lifecycle_matches_elkjs_across_parent_providers() {
    // Actual elkjs 0.9.3, same explicit-container input as the dedicated provider test.
    // Radial retains MINIMUM_SIZE/NODE_LABELS; Stress fixes its size during Force export.
    // Parent micro layout must shrink only Radial and retain fixed Stress title overhang.
    for parent in [
        Algorithm::Layered,
        Algorithm::Box,
        Algorithm::Rectpacking,
        Algorithm::Force,
        Algorithm::Stress,
        Algorithm::MrTree,
        Algorithm::Radial,
        Algorithm::SporeOverlap,
    ] {
        for child in [Algorithm::Radial, Algorithm::Stress] {
            let mut input = graph(parent);
            input.id = "root".into();
            input.nodes.truncate(2);
            for (node, id) in input.nodes.iter_mut().zip(["a", "b"]) {
                node.id = id.into();
                node.parent = Some("g".into());
            }
            let mut group = input.nodes[0].clone();
            group.id = "g".into();
            group.kind = NodeKind::Group;
            group.parent = None;
            group.label_text = Some("Group".into());
            group.label = Some(Label {
                width: 300.0,
                height: 10.0,
            });
            group.container = ContainerNodeOptions {
                algorithm: Some(child),
                padding: 15.0,
            };
            input.nodes.insert(0, group);
            input.edges.truncate(1);
            input.edges[0].id = "inside".into();
            input.edges[0].source = "a".into();
            input.edges[0].target = "b".into();
            input.edges[0].label = None;
            let output = super::super::layout(&input).unwrap();
            let g = &output.nodes[0];
            let origin = (g.x - g.width / 2.0, g.y - g.height / 2.0);
            let radial = child == Algorithm::Radial;
            let expected_size = if radial {
                if parent == Algorithm::SporeOverlap {
                    (147.08203932499367, 70.0)
                } else {
                    (330.0, 40.0)
                }
            } else {
                (179.74360154767774, 72.15639226832202)
            };
            close(g.width, expected_size.0);
            close(g.height, expected_size.1);
            let expected_origin = match parent {
                Algorithm::Box | Algorithm::Rectpacking => (15.0, 15.0),
                Algorithm::Force | Algorithm::Stress => (50.0, 50.0),
                Algorithm::MrTree => (30.0, 40.0),
                Algorithm::SporeOverlap => (8.0, 8.0),
                Algorithm::Layered | Algorithm::Radial if !radial => (72.12819922616113, 12.0),
                _ => (12.0, 12.0),
            };
            close(origin.0, expected_origin.0);
            close(origin.1, expected_origin.1);
            let a = &output.nodes[1];
            close(
                a.x - a.width / 2.0 - origin.0,
                if radial {
                    92.08203932499369
                } else {
                    124.74360154767774
                },
            );
            close(
                a.y - a.height / 2.0 - origin.1,
                if radial { 30.0 } else { 25.0 },
            );
        }
    }
}

#[test]
fn rectpacking_final_micro_layout_preserves_expansion_translation() {
    // Actual elkjs 0.9.3 with Mermaid's root Rectpacking preset: whitespace expansion
    // translates nested children before the second micro layout resets active dimensions.
    for child in [Algorithm::Radial, Algorithm::Stress] {
        let mut input = graph(Algorithm::Rectpacking);
        input.id = "root".into();
        for (node, id) in input.nodes[..2].iter_mut().zip(["a", "b"]) {
            node.id = id.into();
            node.parent = Some("g".into());
        }
        input.nodes[2].id = "c".into();
        input.nodes[2].width = 400.0;
        input.nodes[2].height = 100.0;
        let mut group = input.nodes[0].clone();
        group.id = "g".into();
        group.kind = NodeKind::Group;
        group.parent = None;
        group.label_text = Some("Group".into());
        group.label = Some(Label {
            width: 300.0,
            height: 10.0,
        });
        group.container = ContainerNodeOptions {
            algorithm: Some(child),
            padding: 15.0,
        };
        input.nodes.insert(0, group);
        input.edges.truncate(1);
        input.edges[0].id = "inside".into();
        input.edges[0].source = "a".into();
        input.edges[0].target = "b".into();
        input.edges[0].label = None;
        let output = super::super::layout(&input).unwrap();
        let g = output.nodes.iter().find(|node| node.id == "g").unwrap();
        let origin = (g.x - g.width / 2.0, g.y - g.height / 2.0);
        let radial = child == Algorithm::Radial;
        close(g.width, if radial { 330.0 } else { 400.0 });
        close(g.height, if radial { 40.0 } else { 72.15639226832202 });
        close(origin.0, 15.0);
        close(origin.1, if radial { 30.0 } else { 15.0 });
        let a = output.nodes.iter().find(|node| node.id == "a").unwrap();
        close(
            a.x - a.width / 2.0 - origin.0,
            if radial {
                127.08203932499369
            } else {
                234.87180077383886
            },
        );
    }
}
// Real elkjs 0.9.3 output, with Mermaid 12 root options and normal node micro layout enabled.
// addEdgesToElkGraph creates an inline CENTER label on every edge, including the second
// edge's empty zero-size label. Force imports both labels as particles.
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
                [193.42617779808174, 303.35578838279287, 40.0, 20.0],
                [80.0, 210.49423738443934, 60.0, 30.0],
                [170.51463253901534, 75.0, 30.0, 50.0],
            ],
            [
                &[
                    [181.21163081080465, 293.35578838279287],
                    [98.32182048091563, 225.49423738443934],
                ][..],
                &[
                    [90.02049617972281, 195.49423738443934],
                    [155.51463253901534, 97.45397792329919],
                ][..],
            ],
            [131.71308889904088, 253.92501288361612],
        ),
        (
            Algorithm::Stress,
            [
                [84.83167066281209, 274.9065694325644, 40.0, 20.0],
                [80.0, 175.00946228651006, 60.0, 30.0],
                [80.94114129254214, 75.0, 30.0, 50.0],
            ],
            [
                &[
                    [84.34800594009414, 264.9065694325644],
                    [80.72549708407692, 190.00946228651006],
                ][..],
                &[
                    [80.14115783712234, 160.00946228651006],
                    [80.70587823067157, 100.0],
                ][..],
            ],
            [77.41583533140604, 221.95801585953723],
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
fn cross_provider_boundary_edges_are_not_flattened_or_sent_to_layered_diagnostics() {
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
            Err(Error::UnsupportedCrossProviderEdge { .. })
        ));
    }
}

fn cross_provider_graph(algorithm: Algorithm, directional_child: bool) -> Graph {
    let mut input = graph(algorithm);
    input.id = "root".into();
    for (node, id) in input.nodes.iter_mut().zip(["a", "b", "outside"]) {
        node.id = id.into();
    }
    input.nodes[0].parent = Some("g".into());
    input.nodes[1].parent = Some("g".into());
    input.nodes[2].width = 50.0;
    input.nodes[2].height = 25.0;
    let mut group = input.nodes[0].clone();
    group.id = "g".into();
    group.parent = None;
    group.kind = NodeKind::Group;
    group.direction = directional_child.then_some(Direction::Right);
    input.nodes.insert(0, group);
    input.edges[0] = Edge {
        id: "inside".into(),
        source: "a".into(),
        target: "b".into(),
        label: None,
        terminal_labels: Vec::new(),
        minlen: 1,
        inside_self_loops_yo: false,
    };
    input.edges[1] = Edge {
        id: "cross".into(),
        source: "a".into(),
        target: "outside".into(),
        label: Some(Label {
            width: 20.0,
            height: 10.0,
        }),
        terminal_labels: Vec::new(),
        minlen: 1,
        inside_self_loops_yo: false,
    };
    input
}

#[test]
fn cross_provider_peer_edges_preserve_provider_layout_and_missing_sections() {
    // Force/MrTree explicitly exclude isHierarchical edges; Box/Rectpacking never
    // route them. Mermaid render.ts then clips its centre-to-centre fallback.
    for algorithm in [
        Algorithm::Box,
        Algorithm::Rectpacking,
        Algorithm::Force,
        Algorithm::Stress,
        Algorithm::MrTree,
    ] {
        for directional_child in [false, true] {
            for reverse in [false, true] {
                let mut input = cross_provider_graph(algorithm, directional_child);
                if reverse {
                    let edge = &mut input.edges[1];
                    std::mem::swap(&mut edge.source, &mut edge.target);
                }
                let mut without_cross = input.clone();
                without_cross.edges.pop();
                let expected = super::super::layout(&without_cross).unwrap();
                let actual = super::super::layout(&input).unwrap();
                assert_eq!(
                    actual.nodes, expected.nodes,
                    "{algorithm:?}, directional={directional_child}, reverse={reverse}"
                );
                assert_eq!(
                    actual.edges.iter().find(|edge| edge.id == "inside"),
                    expected.edges.iter().find(|edge| edge.id == "inside")
                );
                let cross = actual.edges.iter().find(|edge| edge.id == "cross").unwrap();
                assert!(
                    cross.points.is_empty(),
                    "{algorithm:?} must retain missing sections"
                );
                assert_eq!(
                    cross.labels,
                    [EdgeLabelLayout {
                        terminal: None,
                        x: 0.0,
                        y: 0.0,
                        width: 20.0,
                        height: 10.0
                    }]
                );
                assert_eq!(actual.edges.len(), 2);
            }
        }
    }
}

#[test]
fn radial_cross_provider_incoming_edge_keeps_raw_hierarchy_explicitly_unsupported() {
    let mut input = cross_provider_graph(Algorithm::Radial, false);
    input.nodes.rotate_right(1);
    assert_eq!(input.nodes[0].id, "outside");
    assert_eq!(input.edges[1].source, "a");
    assert_eq!(input.edges[1].target, "outside");
    // elkjs 0.9.3 RadialUtil.findRoot includes hierarchical incoming edges. With outside
    // declared before g, a->outside changes the selected root from outside to g. The current
    // flat kernel cannot represent cross-scope adjacency; do not return the edge-free layout.
    // Root Radial is a raw-adapter option; Mermaid clears explicit container algorithms on
    // cross-boundary paths before invoking ELK.
    assert!(
        matches!(super::super::layout(&input), Err(Error::UnsupportedCrossProviderEdge { edge_id }) if edge_id == "cross")
    );
    input.edges.pop();
    assert!(
        super::super::layout(&input).is_ok(),
        "ordinary Radial hierarchy without cross-scope adjacency remains supported"
    );
}

#[test]
fn spore_cross_provider_edges_use_source_local_coordinates_and_label_translation() {
    // Actual elkjs 0.9.3 with Mermaid's ordinary child options: padding/baseValue=24,
    // portsSurrounding=12, nodeNode=50, edgeNodeBetweenLayers=30, edgeEdge=20,
    // BRANDES_KOEPF/BALANCED, DEPTH_FIRST. SPOrE reads the descendant's own local x/y.
    let input = cross_provider_graph(Algorithm::SporeOverlap, false);
    let actual = super::super::layout(&input).unwrap();
    let cross = actual.edges.iter().find(|edge| edge.id == "cross").unwrap();
    let group = actual.nodes.iter().find(|node| node.id == "g").unwrap();
    let child = actual.nodes.iter().find(|node| node.id == "a").unwrap();
    close(
        child.x - child.width / 2.0 - (group.x - group.width / 2.0),
        24.0,
    );
    close(
        child.y - child.height / 2.0 - (group.y - group.height / 2.0),
        28.2,
    );
    assert_eq!(cross.points.len(), 2);
    for point in &cross.points {
        close(point.x, 37.78531073446328);
        close(point.y, 28.200000000000003);
    }
    close(cross.labels[0].x, 12.0);
    close(cross.labels[0].y, 12.0);
    assert_eq!(cross.labels[0].width, 20.0);
    let inside = actual
        .edges
        .iter()
        .find(|edge| edge.id == "inside")
        .unwrap();
    assert!(!inside.points.is_empty());
    let mut reverse = input.clone();
    let edge = &mut reverse.edges[1];
    std::mem::swap(&mut edge.source, &mut edge.target);
    let reversed = super::super::layout(&reverse).unwrap();
    assert_eq!(actual.nodes, reversed.nodes);
    let cross = reversed
        .edges
        .iter()
        .find(|edge| edge.id == "cross")
        .unwrap();
    for point in &cross.points {
        close(point.x, 40.7683615819209);
        close(point.y, 33.0);
    }
}

#[test]
fn cross_provider_parent_edge_uses_the_inner_non_layered_container() {
    for algorithm in [Algorithm::Box, Algorithm::SporeOverlap] {
        let mut input = cross_provider_graph(algorithm, true);
        input.edges[1].source = "g".into();
        input.edges[1].target = "a".into();
        let actual = super::super::layout(&input).unwrap();
        let cross = actual.edges.iter().find(|edge| edge.id == "cross").unwrap();
        let group = actual.nodes.iter().find(|node| node.id == "g").unwrap();
        let origin = Point {
            x: group.x - group.width / 2.0,
            y: group.y - group.height / 2.0,
        };
        if algorithm == Algorithm::Box {
            assert!(cross.points.is_empty());
            close(cross.labels[0].x, origin.x);
            close(cross.labels[0].y, origin.y);
        } else {
            let child = actual.nodes.iter().find(|node| node.id == "a").unwrap();
            assert_eq!(cross.points.len(), 2);
            // Source clips both ends to the descendant centre before the outer provider
            // positions g. The final public coordinates receive g's offset exactly once.
            for point in &cross.points {
                close(point.x, child.x);
                close(point.y, child.y);
            }
            // elkjs 0.9.3 with ordinary container spacing.nodeNode=50/padding=24
            // translates the original label by 49 in each axis before outer placement.
            close(group.width, 198.0);
            close(group.height, 123.0);
            close(child.x - origin.x, 44.0);
            close(child.y - origin.y, 34.0);
            close(cross.labels[0].x - origin.x, 49.0);
            close(cross.labels[0].y - origin.y, 49.0);
        }
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
        terminal_labels: Vec::new(),
        minlen: 1,
        inside_self_loops_yo: false,
    });
    assert!(matches!(
        super::super::layout(&graph),
        Err(Error::Radial(radial::Error::MissingRoot))
    ));
}

#[test]
fn mrtree_node_ids_do_not_become_synthetic_root_labels() {
    let original = graph(Algorithm::MrTree);
    let expected = super::super::layout(&original).unwrap();
    let mut renamed = original;
    renamed.nodes[0].id = "SUPER_ROOT".into();
    renamed.edges[0].source = "SUPER_ROOT".into();
    let actual = super::super::layout(&renamed).unwrap();
    assert_eq!(actual.edges, expected.edges);
    for (a, e) in actual.nodes.iter().zip(expected.nodes) {
        assert_eq!((a.x, a.y, a.width, a.height), (e.x, e.y, e.width, e.height));
    }
}

#[test]
fn mixed_root_scopes_match_actual_elkjs_container_resolution() {
    // Real target ordinary container options, with a painted title and both an internal edge
    // and an edge attached to the group itself. Optional direction selects the diagram provider.
    let cases = [
        (
            Algorithm::Box,
            false,
            [
                [146.0, 61.5, 172.0, 93.0],
                [30.0, 40.0, 30.0, 50.0],
                [104.0, 68.2, 40.0, 20.0],
                [178.0, 69.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Box,
            true,
            [
                [114.0, 89.0, 108.0, 148.0],
                [30.0, 40.0, 30.0, 50.0],
                [104.0, 49.0, 40.0, 20.0],
                [114.0, 124.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Force,
            false,
            [
                [314.75964927325157, 129.89663305134906, 172.0, 93.0],
                [65.0, 75.0, 30.0, 50.0],
                [272.75964927325157, 136.59663305134904, 40.0, 20.0],
                [346.75964927325157, 137.39663305134906, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Force,
            true,
            [
                [
                    312.5284522612195,
                    136.8642048608271,
                    182.8461790112895,
                    79.08753373701688,
                ],
                [65.0, 75.0, 30.0, 50.0],
                [359.95154176686424, 131.32043799231866, 40.0, 20.0],
                [275.1053627555748, 137.40797172933554, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Rectpacking,
            false,
            [
                [101.0, 61.5, 172.0, 93.0],
                [217.0, 61.5, 30.0, 93.0],
                [59.0, 68.2, 40.0, 20.0],
                [133.0, 69.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Rectpacking,
            true,
            [
                [69.0, 89.0, 108.0, 148.0],
                [153.0, 89.0, 30.0, 148.0],
                [59.0, 49.0, 40.0, 20.0],
                [69.0, 124.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::MrTree,
            false,
            [
                [134.0, 94.5, 172.0, 93.0],
                [134.0, 186.5, 30.0, 50.0],
                [92.0, 101.2, 40.0, 20.0],
                [166.0, 102.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::MrTree,
            true,
            [
                [102.0, 119.5, 108.0, 143.0],
                [102.0, 236.5, 30.0, 50.0],
                [118.0, 98.0, 40.0, 20.0],
                [118.0, 173.0, 60.0, 30.0],
            ],
        ),
    ];
    for (algorithm, directional, expected) in cases {
        let mut graph = graph(algorithm);
        graph.id = "root".into();
        let mut group = graph.nodes[0].clone();
        group.id = "g".into();
        group.kind = NodeKind::Group;
        group.width = 0.0;
        group.height = 0.0;
        group.label = Some(Label {
            width: 30.0,
            height: 10.0,
        });
        group.label_text = Some("Group".into());
        if directional {
            group.direction = Some(Direction::Down);
            group.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
        }
        graph.nodes[0].id = "a".into();
        graph.nodes[0].parent = Some("g".into());
        graph.nodes[1].id = "b".into();
        graph.nodes[1].parent = Some("g".into());
        graph.nodes[2].id = "c".into();
        graph.nodes.insert(0, group);
        graph.edges[0].id = "inside".into();
        graph.edges[0].source = "a".into();
        graph.edges[0].target = "b".into();
        graph.edges[0].label = None;
        graph.edges[1].id = "outside".into();
        graph.edges[1].source = "g".into();
        graph.edges[1].target = "c".into();
        graph.edges[1].label = Some(Label {
            width: 10.0,
            height: 6.0,
        });
        let result = super::super::layout(&graph).unwrap();
        assert_eq!(result.nodes.len(), expected.len());
        for (node, wanted) in result.nodes.iter().zip(expected) {
            for (actual, expected) in [node.x, node.y, node.width, node.height]
                .into_iter()
                .zip(wanted)
            {
                assert!(
                    (actual - expected).abs() < 1e-7,
                    "{algorithm:?} directional={directional} node={} actual={actual} expected={expected}",
                    node.id
                );
            }
        }
        assert_eq!(result.edges.len(), 2);
    }
}

#[test]
fn explicit_container_algorithms_and_title_minima_match_elkjs() {
    // Mermaid 12 metadata presets, executed by elkjs 0.9.3. Root Box isolates each
    // container extent; wide titles exercise sizing before and after provider execution.
    let cases = [
        (
            Algorithm::Layered,
            30.0,
            [
                [154.0, 85.0, 0.0, 0.0],
                [15.0, 44.2, 40.0, 20.0],
                [79.0, 40.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Layered,
            300.0,
            [
                [330.0, 85.0, 0.0, 0.0],
                [103.0, 44.2, 40.0, 20.0],
                [167.0, 40.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Box,
            30.0,
            [
                [90.0, 140.0, 0.0, 0.0],
                [15.0, 25.0, 60.0, 20.0],
                [15.0, 95.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Box,
            300.0,
            [
                [330.0, 140.0, 0.0, 0.0],
                [15.0, 25.0, 300.0, 20.0],
                [15.0, 95.0, 300.0, 30.0],
            ],
        ),
        (
            Algorithm::Rectpacking,
            30.0,
            [
                [135.0, 60.0, 0.0, 0.0],
                [10.0, 20.0, 40.0, 30.0],
                [65.0, 20.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Rectpacking,
            300.0,
            [
                [330.0, 60.0, 0.0, 0.0],
                [10.0, 20.0, 137.5, 30.0],
                [162.5, 20.0, 157.5, 30.0],
            ],
        ),
        (
            Algorithm::Force,
            30.0,
            [
                [164.8461790112895, 71.08753373701688, 0.0, 0.0],
                [109.84617901128951, 25.0, 40.0, 20.0],
                [15.0, 26.08753373701689, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Force,
            300.0,
            [
                [330.0, 71.08753373701688, 0.0, 0.0],
                [109.84617901128951, 25.0, 40.0, 20.0],
                [15.0, 26.08753373701689, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Stress,
            30.0,
            [
                [179.74360154767774, 72.15639226832202, 0.0, 0.0],
                [124.74360154767774, 25.0, 40.0, 20.0],
                [15.0, 27.156392268322023, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Stress,
            300.0,
            [
                [179.74360154767774, 72.15639226832202, 0.0, 0.0],
                [124.74360154767774, 25.0, 40.0, 20.0],
                [15.0, 27.156392268322023, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::MrTree,
            30.0,
            [
                [90.0, 135.0, 0.0, 0.0],
                [50.0, 40.0, 40.0, 20.0],
                [40.0, 110.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::MrTree,
            300.0,
            [
                [330.0, 135.0, 0.0, 0.0],
                [50.0, 40.0, 40.0, 20.0],
                [40.0, 110.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Radial,
            30.0,
            [
                [60.0, 40.0, 0.0, 0.0],
                [92.08203932499369, 29.999999999999993, 40.0, 20.0],
                [15.0, 25.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::Radial,
            300.0,
            [
                [330.0, 40.0, 0.0, 0.0],
                [92.08203932499369, 29.999999999999993, 40.0, 20.0],
                [15.0, 25.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::SporeOverlap,
            30.0,
            [
                [180.0, 115.0, 0.0, 0.0],
                [15.0, 25.0, 40.0, 20.0],
                [105.0, 70.0, 60.0, 30.0],
            ],
        ),
        (
            Algorithm::SporeOverlap,
            300.0,
            [
                [330.0, 115.0, 0.0, 0.0],
                [15.0, 25.0, 40.0, 20.0],
                [105.0, 70.0, 60.0, 30.0],
            ],
        ),
    ];
    for (algorithm, title_width, expected) in cases {
        let mut graph = graph(Algorithm::Box);
        graph.id = "root".into();
        graph.nodes.truncate(2);
        for (node, id) in graph.nodes.iter_mut().zip(["a", "b"]) {
            node.id = id.into();
            node.parent = Some("g".into());
        }
        let mut group = graph.nodes[0].clone();
        group.id = "g".into();
        group.kind = NodeKind::Group;
        group.parent = None;
        group.width = 900.0; // Nonempty Mermaid groups discard their measured input dimensions.
        group.height = 900.0;
        group.label_text = Some("Group".into());
        group.label = Some(Label {
            width: title_width,
            height: 10.0,
        });
        group.container = ContainerNodeOptions {
            algorithm: Some(algorithm),
            padding: 15.0,
        };
        // Metadata wins over direction; no elk.direction is emitted for these containers.
        group.direction = Some(Direction::Up);
        graph.nodes.insert(0, group);
        graph.edges.truncate(1);
        graph.edges[0].id = "inside".into();
        graph.edges[0].source = "a".into();
        graph.edges[0].target = "b".into();
        graph.edges[0].label = None;
        let result = super::super::layout(&graph).unwrap();
        let group = &result.nodes[0];
        let origin = (group.x - group.width / 2.0, group.y - group.height / 2.0);
        let mut actual = vec![[group.width, group.height, 0.0, 0.0]];
        actual.extend(result.nodes[1..].iter().map(|node| {
            [
                node.x - node.width / 2.0 - origin.0,
                node.y - node.height / 2.0 - origin.1,
                node.width,
                node.height,
            ]
        }));
        for (actual, expected) in actual.into_iter().zip(expected) {
            for (a, e) in actual.into_iter().zip(expected) {
                assert!(
                    (a - e).abs() < 1e-7,
                    "{algorithm:?} title={title_width}: actual={a}, expected={e}"
                );
            }
        }
    }
}
