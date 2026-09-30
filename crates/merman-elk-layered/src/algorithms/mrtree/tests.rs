use super::*;
use crate::work::NoopWorkControl;

fn close(actual: f64, expected: f64, name: &str) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "{name}: actual {actual}, expected {expected}"
    );
}

// Captured from the real elkjs 0.9.3 bundle (ELK 0.9.1), algorithm=mrtree and
// omitNodeMicroLayout=true. All non-default options are specified per case.
#[test]
fn source_oracles_cover_branches_cycles_dags_components_and_directions() {
    let cases = [
        (
            "branch",
            vec![
                [40.0, 20.0],
                [30.0, 50.0],
                [70.0, 30.0],
                [20.0, 20.0],
                [45.0, 25.0],
            ],
            vec![(0, 1), (0, 2), (1, 3), (2, 4)],
            Options::default(),
            [140.0, 173.0],
            vec![
                [70.0, 40.0],
                [40.0, 80.0],
                [90.0, 90.0],
                [45.0, 153.0],
                [102.5, 150.5],
            ],
            vec![
                Some(vec![
                    [83.33333333333333, 60.0],
                    [83.33333333333333, 63.0],
                    [55.0, 80.0],
                ]),
                Some(vec![
                    [96.66666666666666, 60.0],
                    [96.66666666666666, 63.0],
                    [125.0, 77.0],
                    [125.0, 90.0],
                ]),
                Some(vec![[55.0, 130.0], [55.0, 133.0], [55.0, 153.0]]),
                Some(vec![[125.0, 120.0], [125.0, 133.0], [125.0, 150.5]]),
            ],
        ),
        (
            "cycle",
            vec![[40.0, 20.0], [30.0, 50.0], [70.0, 30.0]],
            vec![(0, 1), (1, 2), (2, 0)],
            Options::default(),
            [110.0, 175.0],
            vec![[55.0, 40.0], [60.0, 80.0], [40.0, 150.0]],
            vec![
                Some(vec![[75.0, 60.0], [75.0, 63.0], [75.0, 80.0]]),
                Some(vec![[75.0, 130.0], [75.0, 133.0], [75.0, 150.0]]),
                Some(vec![
                    [75.0, 180.0],
                    [75.0, 183.0],
                    [37.0, 183.0],
                    [37.0, 30.0],
                    [75.0, 30.0],
                    [75.0, 40.0],
                ]),
            ],
        ),
        (
            "dag",
            vec![[40.0, 20.0], [30.0, 50.0], [70.0, 30.0], [20.0, 20.0]],
            vec![(0, 1), (0, 2), (1, 3), (2, 3), (0, 3)],
            Options::default(),
            [140.0, 170.0],
            vec![[70.0, 40.0], [40.0, 80.0], [90.0, 90.0], [45.0, 150.0]],
            vec![
                Some(vec![[80.0, 60.0], [80.0, 63.0], [55.0, 80.0]]),
                Some(vec![
                    [100.0, 60.0],
                    [100.0, 63.0],
                    [125.0, 77.0],
                    [125.0, 90.0],
                ]),
                Some(vec![[55.0, 130.0], [55.0, 133.0], [50.0, 150.0]]),
                Some(vec![
                    [125.0, 120.0],
                    [125.0, 133.0],
                    [63.5, 148.67924528301887],
                    [60.0, 150.0],
                ]),
                Some(vec![
                    [90.0, 60.0],
                    [90.0, 63.0],
                    [80.0, 77.0],
                    [80.0, 133.0],
                    [55.0, 150.0],
                ]),
            ],
        ),
        (
            "multiroot",
            vec![[40.0, 20.0], [30.0, 50.0], [70.0, 30.0]],
            vec![(0, 2), (1, 2)],
            Options::default(),
            [165.0, 150.0],
            vec![[55.0, 75.0], [115.0, 60.0], [40.0, 130.0]],
            vec![
                Some(vec![
                    [75.0, 95.0],
                    [75.0, 113.0],
                    [63.33333333333333, 130.0],
                ]),
                Some(vec![
                    [130.0, 110.0],
                    [130.0, 113.0],
                    [90.16666666666666, 128.67924528301887],
                    [86.66666666666666, 130.0],
                ]),
            ],
        ),
        (
            "components",
            vec![
                [40.0, 20.0],
                [30.0, 50.0],
                [70.0, 30.0],
                [20.0, 20.0],
                [45.0, 25.0],
            ],
            vec![(0, 1), (2, 3)],
            Options::default(),
            [157.5, 177.5],
            vec![
                [62.5, 52.5],
                [67.5, 92.5],
                [122.5, 52.5],
                [147.5, 102.5],
                [62.5, 162.5],
            ],
            vec![
                Some(vec![[82.5, 72.5], [82.5, 75.5], [82.5, 92.5]]),
                Some(vec![[157.5, 82.5], [157.5, 85.5], [157.5, 102.5]]),
            ],
        ),
        (
            "parallel",
            vec![[40.0, 20.0], [30.0, 50.0]],
            vec![(0, 1), (0, 1)],
            Options::default(),
            [80.0, 115.0],
            vec![[40.0, 40.0], [45.0, 80.0]],
            vec![
                Some(vec![[60.0, 60.0], [60.0, 63.0], [60.0, 80.0]]),
                Some(vec![[60.0, 60.0], [60.0, 80.0]]),
            ],
        ),
        (
            "empty",
            vec![],
            vec![],
            Options::default(),
            [0.0, 0.0],
            vec![],
            vec![],
        ),
        (
            "single",
            vec![[0.0, 0.0]],
            vec![],
            Options::default(),
            [41.0, 41.0],
            vec![[40.0, 40.0]],
            vec![],
        ),
        (
            "branch-RIGHT",
            vec![
                [40.0, 20.0],
                [30.0, 50.0],
                [70.0, 30.0],
                [20.0, 20.0],
                [45.0, 25.0],
            ],
            vec![(0, 1), (0, 2), (1, 3), (2, 4)],
            Options {
                direction: Direction::Right,
                ..Options::default()
            },
            [233.0, 150.0],
            vec![
                [40.0, 85.0],
                [120.0, 40.0],
                [100.0, 110.0],
                [203.0, 55.0],
                [190.5, 112.5],
            ],
            vec![
                Some(vec![
                    [80.0, 91.66666666666666],
                    [83.0, 91.66666666666666],
                    [97.0, 65.0],
                    [120.0, 65.0],
                ]),
                Some(vec![
                    [80.0, 98.33333333333333],
                    [83.0, 98.33333333333333],
                    [100.0, 125.0],
                ]),
                Some(vec![
                    [150.0, 65.0],
                    [173.0, 65.0],
                    [187.5, 65.0],
                    [203.0, 65.0],
                ]),
                Some(vec![[170.0, 125.0], [173.0, 125.0], [190.5, 125.0]]),
            ],
        ),
        (
            "branch-LEFT",
            vec![
                [40.0, 20.0],
                [30.0, 50.0],
                [70.0, 30.0],
                [20.0, 20.0],
                [45.0, 25.0],
            ],
            vec![(0, 1), (0, 2), (1, 3), (2, 4)],
            Options {
                direction: Direction::Left,
                ..Options::default()
            },
            [238.0, 150.0],
            vec![
                [195.5, 85.0],
                [125.5, 40.0],
                [105.5, 110.0],
                [52.5, 55.0],
                [40.0, 112.5],
            ],
            vec![
                Some(vec![
                    [195.5, 91.66666666666666],
                    [192.5, 91.66666666666666],
                    [178.5, 65.0],
                    [155.5, 65.0],
                ]),
                Some(vec![
                    [195.5, 98.33333333333333],
                    [192.5, 98.33333333333333],
                    [175.5, 125.0],
                ]),
                Some(vec![
                    [125.5, 65.0],
                    [105.5, 65.0],
                    [88.0, 65.0],
                    [72.5, 65.0],
                ]),
                Some(vec![[105.5, 125.0], [102.5, 125.0], [85.0, 125.0]]),
            ],
        ),
        (
            "branch-UP",
            vec![
                [40.0, 20.0],
                [30.0, 50.0],
                [70.0, 30.0],
                [20.0, 20.0],
                [45.0, 25.0],
            ],
            vec![(0, 1), (0, 2), (1, 3), (2, 4)],
            Options {
                direction: Direction::Up,
                ..Options::default()
            },
            [140.0, 178.0],
            vec![
                [70.0, 155.5],
                [40.0, 85.5],
                [90.0, 95.5],
                [45.0, 42.5],
                [102.5, 40.0],
            ],
            vec![
                Some(vec![
                    [83.33333333333333, 155.5],
                    [83.33333333333333, 152.5],
                    [55.0, 135.5],
                ]),
                Some(vec![
                    [96.66666666666666, 155.5],
                    [96.66666666666666, 152.5],
                    [125.0, 138.5],
                    [125.0, 125.5],
                ]),
                Some(vec![[55.0, 85.5], [55.0, 82.5], [55.0, 62.5]]),
                Some(vec![[125.0, 95.5], [125.0, 85.5], [125.0, 65.0]]),
            ],
        ),
        (
            "spacing",
            vec![[40.0, 20.0], [30.0, 50.0], [70.0, 30.0], [20.0, 20.0]],
            vec![(0, 1), (0, 2), (1, 3), (2, 3), (0, 3)],
            Options {
                spacing: 50.0,
                padding: Padding {
                    top: 45.0,
                    left: 25.0,
                    bottom: 15.0,
                    right: 35.0,
                },
                ..Options::default()
            },
            [190.0, 250.0],
            vec![[85.0, 40.0], [40.0, 110.0], [120.0, 120.0], [45.0, 210.0]],
            vec![
                Some(vec![[95.0, 60.0], [95.0, 63.0], [55.0, 110.0]]),
                Some(vec![
                    [115.0, 60.0],
                    [115.0, 63.0],
                    [155.0, 107.0],
                    [155.0, 120.0],
                ]),
                Some(vec![[55.0, 160.0], [55.0, 163.0], [50.0, 210.0]]),
                Some(vec![
                    [155.0, 150.0],
                    [155.0, 163.0],
                    [63.5, 208.67924528301887],
                    [60.0, 210.0],
                ]),
                Some(vec![
                    [105.0, 60.0],
                    [105.0, 63.0],
                    [95.0, 107.0],
                    [95.0, 163.0],
                    [55.0, 210.0],
                ]),
            ],
        ),
        (
            "apportion",
            vec![
                [40.0, 20.0],
                [50.0, 30.0],
                [60.0, 25.0],
                [30.0, 40.0],
                [45.0, 35.0],
                [70.0, 30.0],
                [35.0, 45.0],
                [25.0, 25.0],
                [50.0, 20.0],
                [80.0, 30.0],
                [25.0, 40.0],
                [45.0, 20.0],
            ],
            vec![
                (0, 1),
                (0, 2),
                (0, 3),
                (1, 4),
                (1, 5),
                (4, 6),
                (5, 7),
                (3, 8),
                (3, 9),
                (8, 10),
                (9, 11),
            ],
            Options::default(),
            [328.0, 228.0],
            vec![
                [161.5, 40.0],
                [76.5, 85.0],
                [156.5, 87.5],
                [247.5, 80.0],
                [40.0, 140.5],
                [105.5, 143.0],
                [45.0, 195.5],
                [128.0, 205.5],
                [195.5, 148.0],
                [265.5, 143.0],
                [208.0, 198.0],
                [283.0, 208.0],
            ],
            vec![
                Some(vec![
                    [171.5, 60.0],
                    [171.5, 63.0],
                    [105.0, 83.67924528301887],
                    [101.5, 85.0],
                ]),
                Some(vec![
                    [181.5, 60.0],
                    [181.5, 63.0],
                    [186.5, 77.0],
                    [186.5, 87.5],
                ]),
                Some(vec![
                    [191.5, 60.0],
                    [191.5, 63.0],
                    [259.0, 78.67924528301887],
                    [262.5, 80.0],
                ]),
                Some(vec![
                    [93.16666666666666, 115.0],
                    [93.16666666666666, 123.0],
                    [62.5, 140.5],
                ]),
                Some(vec![
                    [109.83333333333333, 115.0],
                    [109.83333333333333, 123.0],
                    [140.5, 143.0],
                ]),
                Some(vec![[62.5, 175.5], [62.5, 178.5], [62.5, 195.5]]),
                Some(vec![
                    [140.5, 173.0],
                    [140.5, 178.5],
                    [140.5, 192.5],
                    [140.5, 205.5],
                ]),
                Some(vec![
                    [257.5, 120.0],
                    [257.5, 123.0],
                    [220.5, 137.5],
                    [220.5, 148.0],
                ]),
                Some(vec![[267.5, 120.0], [267.5, 123.0], [305.5, 143.0]]),
                Some(vec![[220.5, 168.0], [220.5, 178.5], [220.5, 198.0]]),
                Some(vec![
                    [305.5, 173.0],
                    [305.5, 178.5],
                    [305.5, 192.5],
                    [305.5, 208.0],
                ]),
            ],
        ),
        (
            "cycle-right",
            vec![[40.0, 20.0], [50.0, 30.0], [60.0, 25.0]],
            vec![(0, 1), (1, 2), (2, 0)],
            Options {
                direction: Direction::Right,
                ..Options::default()
            },
            [220.0, 70.0],
            vec![[40.0, 45.0], [100.0, 40.0], [170.0, 42.5]],
            vec![
                Some(vec![[80.0, 55.0], [83.0, 55.0], [100.0, 55.0]]),
                Some(vec![[150.0, 55.0], [153.0, 55.0], [170.0, 55.0]]),
                Some(vec![
                    [230.0, 55.0],
                    [233.0, 55.0],
                    [233.0, 55.0],
                    [233.0, 37.0],
                    [30.0, 37.0],
                    [30.0, 55.0],
                    [40.0, 55.0],
                    [40.0, 55.0],
                ]),
            ],
        ),
        (
            "shared-gap",
            vec![
                [40.0, 20.0],
                [50.0, 30.0],
                [60.0, 25.0],
                [30.0, 40.0],
                [45.0, 35.0],
                [70.0, 30.0],
            ],
            vec![
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (0, 4),
                (0, 5),
                (1, 5),
            ],
            Options::default(),
            [110.0, 315.0],
            vec![
                [55.0, 40.0],
                [50.0, 80.0],
                [45.0, 130.5],
                [60.0, 175.0],
                [52.5, 235.5],
                [40.0, 290.0],
            ],
            vec![
                Some(vec![[75.0, 60.0], [75.0, 63.0], [75.0, 80.0]]),
                Some(vec![
                    [83.33333333333333, 110.0],
                    [83.33333333333333, 113.0],
                    [75.0, 130.5],
                ]),
                Some(vec![[75.0, 155.5], [75.0, 158.5], [75.0, 175.0]]),
                Some(vec![[75.0, 215.0], [75.0, 218.0], [82.5, 235.5]]),
                Some(vec![[75.0, 270.5], [75.0, 273.5], [92.5, 290.0]]),
                Some(vec![
                    [85.0, 60.0],
                    [85.0, 63.0],
                    [106.0, 77.0],
                    [106.0, 113.0],
                    [114.0, 127.5],
                    [114.0, 158.5],
                    [51.0, 172.0],
                    [51.0, 218.0],
                    [67.5, 235.5],
                ]),
                Some(vec![
                    [65.0, 60.0],
                    [65.0, 63.0],
                    [103.0, 77.0],
                    [103.0, 113.0],
                    [108.0, 127.5],
                    [108.0, 158.5],
                    [57.0, 172.0],
                    [57.0, 218.0],
                    [75.0, 290.0],
                ]),
                Some(vec![
                    [66.66666666666666, 110.0],
                    [66.66666666666666, 113.0],
                    [111.0, 127.5],
                    [111.0, 158.5],
                    [54.0, 172.0],
                    [54.0, 218.0],
                    [57.5, 290.0],
                ]),
            ],
        ),
        (
            "combined",
            vec![
                [40.0, 20.0],
                [50.0, 30.0],
                [60.0, 25.0],
                [30.0, 40.0],
                [45.0, 35.0],
            ],
            vec![(0, 1), (2, 3)],
            Options {
                separate_components: false,
                ..Options::default()
            },
            [233.0, 135.0],
            vec![
                [45.0, 68.0],
                [40.0, 120.0],
                [105.0, 65.5],
                [120.0, 115.0],
                [185.5, 60.5],
            ],
            vec![
                Some(vec![[65.0, 88.0], [65.0, 98.5], [65.0, 120.0]]),
                Some(vec![[135.0, 90.5], [135.0, 98.5], [135.0, 115.0]]),
            ],
        ),
    ];
    for (name, sizes, pairs, options, extent, positions, routes) in cases {
        let nodes: Vec<_> = sizes
            .into_iter()
            .map(|[width, height]| Node {
                width,
                height,
                ..Node::default()
            })
            .collect();
        let edges: Vec<_> = pairs
            .into_iter()
            .map(|(source, target)| Edge { source, target })
            .collect();
        let actual = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(actual.width, extent[0], name);
        close(actual.height, extent[1], name);
        assert_eq!(actual.nodes.len(), positions.len(), "{name}");
        for (point, expected) in actual.nodes.iter().zip(positions) {
            close(point.x, expected[0], name);
            close(point.y, expected[1], name);
        }
        assert_eq!(actual.edges.len(), routes.len(), "{name}");
        for (route, expected) in actual.edges.iter().zip(routes) {
            assert_eq!(route.is_some(), expected.is_some(), "{name}");
            if let (Some(route), Some(expected)) = (route, expected) {
                assert_eq!(
                    route.len(),
                    expected.len(),
                    "{name}: route {route:?}, expected {expected:?}"
                );
                for (point, expected) in route.iter().zip(expected) {
                    close(point.x, expected[0], name);
                    close(point.y, expected[1], name);
                }
            }
        }
    }
}

#[test]
fn self_loops_are_not_imported_and_invalid_inputs_fail_before_layout() {
    let nodes = [Node {
        width: 30.0,
        height: 20.0,
        ..Node::default()
    }];
    let actual = layout(
        &nodes,
        &[Edge {
            source: 0,
            target: 0,
        }],
        &Options::default(),
        &mut NoopWorkControl,
    )
    .unwrap();
    assert_eq!(actual.edges, vec![None]);
    assert_eq!(
        layout(
            &nodes,
            &[Edge {
                source: 0,
                target: 1
            }],
            &Options::default(),
            &mut NoopWorkControl
        ),
        Err(Error::InvalidEdge(0))
    );
    let bad = [Node {
        width: f64::NAN,
        ..Node::default()
    }];
    assert!(matches!(
        layout(&bad, &[], &Options::default(), &mut NoopWorkControl),
        Err(Error::InvalidNode { field: "width", .. })
    ));
}

#[test]
fn work_control_interrupts_the_walk_without_mutating_input() {
    struct Budget(usize);
    impl WorkControl for Budget {
        fn check(&mut self, units: usize) -> Result<(), WorkError> {
            if units <= self.0 {
                Ok(())
            } else {
                Err(WorkError::Interrupted)
            }
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.check(units)?;
            self.0 -= units;
            Ok(())
        }
    }
    let nodes: Vec<_> = (0..16)
        .map(|_| Node {
            width: 30.0,
            height: 20.0,
            ..Node::default()
        })
        .collect();
    let edges: Vec<_> = (1..16)
        .map(|target| Edge {
            source: target - 1,
            target,
        })
        .collect();
    let original = nodes.clone();
    assert_eq!(
        layout(&nodes, &edges, &Options::default(), &mut Budget(250)),
        Err(Error::Work(WorkError::Interrupted))
    );
    assert_eq!(nodes, original);
}

#[test]
fn source_integer_coordinate_overflow_fails_closed() {
    let nodes = vec![
        Node {
            width: 1e20,
            height: 1e20,
            ..Node::default()
        };
        2
    ];
    assert_eq!(
        layout(
            &nodes,
            &[Edge {
                source: 0,
                target: 1
            }],
            &Options::default(),
            &mut NoopWorkControl
        ),
        Err(Error::NumericRange)
    );
}
