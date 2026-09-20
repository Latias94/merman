use super::*;
use crate::work::NoopWorkControl;
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "actual {a}, expected {b}");
}
#[test]
fn actual_elkjs_093_overlap_removal_oracles() {
    // Measured nodes, default sporeOverlap options, sequential edges and original parent center 0.
    for (name, input, expected, routes, width, height) in [
        (
            "pair",
            vec![[0.0, 0.0, 40.0, 30.0], [10.0, 6.0, 50.0, 20.0]],
            vec![[8.0, 8.0], [56.0, 16.53333333333333]],
            vec![[48.0, 24.333333333333332, 56.0, 24.866666666666667]],
            114.0,
            46.0,
        ),
        (
            "separated",
            vec![[10.0, 20.0, 40.0, 30.0], [110.0, 26.0, 50.0, 20.0]],
            vec![[8.0, 8.0], [108.0, 14.0]],
            vec![[48.0, 23.19047619047619, 108.0, 23.761904761904763]],
            166.0,
            46.0,
        ),
        (
            "triangle",
            vec![
                [0.0, 0.0, 40.0, 30.0],
                [10.0, 6.0, 50.0, 20.0],
                [20.0, 10.0, 30.0, 35.0],
            ],
            vec![
                [8.0, 8.0],
                [56.0, 16.53333333333333],
                [56.0, 45.453862971970416],
            ],
            vec![
                [48.0, 24.333333333333332, 56.0, 24.866666666666667],
                [
                    78.25429583281199,
                    36.53333333333333,
                    75.804982292579,
                    45.453862971970416,
                ],
            ],
            114.0,
            88.45386297197041,
        ),
        (
            "square",
            vec![
                [0.0, 0.0, 40.0, 40.0],
                [10.0, 0.0, 40.0, 40.0],
                [0.0, 10.0, 40.0, 40.0],
                [10.0, 10.0, 40.0, 40.0],
            ],
            vec![[8.0, 8.0], [56.0, 8.0], [8.0, 56.0], [56.0, 56.0]],
            vec![
                [48.0, 28.0, 56.0, 28.0],
                [56.0, 48.0, 48.0, 56.0],
                [48.0, 76.0, 56.0, 76.0],
            ],
            104.0,
            104.0,
        ),
        (
            "collinear",
            vec![
                [0.0, 0.0, 40.0, 30.0],
                [15.0, 0.0, 40.0, 30.0],
                [30.0, 0.0, 40.0, 30.0],
                [45.0, 0.0, 40.0, 30.0],
            ],
            vec![[8.0, 8.0], [56.0, 8.0], [104.0, 8.0], [152.0, 8.0]],
            vec![
                [48.0, 23.0, 56.0, 23.0],
                [96.0, 23.0, 104.0, 23.0],
                [144.0, 23.0, 152.0, 23.0],
            ],
            200.0,
            46.0,
        ),
        (
            "none",
            vec![[0.0, 0.0, 0.0, 0.0]],
            vec![[8.0, 8.0]],
            vec![],
            16.0,
            16.0,
        ),
    ] {
        let nodes: Vec<_> = input
            .iter()
            .map(|&[x, y, width, height]| Node {
                x,
                y,
                width,
                height,
                ..Node::default()
            })
            .collect();
        let edges: Vec<_> = (1..nodes.len())
            .map(|i| Edge {
                source: i - 1,
                target: i,
            })
            .collect();
        let result = layout(
            &nodes,
            &edges,
            &Options::default(),
            &mut || panic!("{name} has distinct centers"),
            &mut NoopWorkControl,
        )
        .unwrap();
        close(result.width, width);
        close(result.height, height);
        for (actual, wanted) in result.nodes.iter().zip(expected) {
            close(actual.x, wanted[0]);
            close(actual.y, wanted[1]);
        }
        for (actual, wanted) in result.edges.iter().zip(routes) {
            close(actual[0].x, wanted[0]);
            close(actual[0].y, wanted[1]);
            close(actual[1].x, wanted[2]);
            close(actual[1].y, wanted[3]);
        }
    }
}
#[test]
fn duplicate_centers_use_only_caller_randomness_and_reject_numeric_stagnation() {
    let nodes = vec![
        Node {
            width: 40.,
            height: 30.,
            ..Node::default()
        };
        3
    ];
    let run = || {
        let mut random = crate::random::JavaRandom::new(7);
        layout(
            &nodes,
            &[],
            &Options::default(),
            &mut || random.next_double(),
            &mut NoopWorkControl,
        )
        .unwrap()
    };
    assert_eq!(run(), run());
    let large = vec![
        Node {
            x: 1e20,
            y: 1e20,
            ..nodes[0]
        };
        2
    ];
    assert_eq!(
        layout(
            &large,
            &[],
            &Options::default(),
            &mut || 0.25,
            &mut NoopWorkControl
        ),
        Err(Error::NumericStagnation)
    );
}
#[test]
fn cancellation_during_geometry_preserves_input() {
    struct Budget(usize);
    impl WorkControl for Budget {
        fn check(&mut self, units: usize) -> Result<(), WorkError> {
            if units > self.0 {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.check(units)?;
            self.0 -= units;
            Ok(())
        }
    }
    let nodes = vec![
        Node {
            width: 40.,
            height: 30.,
            ..Node::default()
        },
        Node {
            x: 10.,
            y: 6.,
            width: 50.,
            height: 20.,
            ..Node::default()
        },
    ];
    let before = nodes.clone();
    assert_eq!(
        layout(
            &nodes,
            &[],
            &Options::default(),
            &mut || panic!("no random needed"),
            &mut Budget(25)
        ),
        Err(Error::Work(WorkError::Interrupted))
    );
    assert_eq!(nodes, before);
}

#[test]
fn source_options_control_iterations_scanline_spacing_and_padding() {
    let nodes: Vec<_> = [
        [0., 0., 40., 30.],
        [10., 6., 50., 20.],
        [20., 10., 30., 35.],
    ]
    .into_iter()
    .map(|[x, y, width, height]| Node {
        x,
        y,
        width,
        height,
        ..Node::default()
    })
    .collect();
    let cases = [
        (
            Options {
                max_iterations: 0,
                ..Options::default()
            },
            [[8., 8.], [18., 14.], [28., 18.]],
            76.,
            61.,
        ),
        (
            Options {
                scanline: false,
                ..Options::default()
            },
            [
                [8., 8.],
                [56., 16.53333333333333],
                [56., 45.453862971970416],
            ],
            114.,
            88.45386297197041,
        ),
        (
            Options {
                spacing: 12.,
                padding: Padding {
                    top: 3.,
                    left: 4.,
                    bottom: 5.,
                    right: 6.,
                },
                ..Options::default()
            },
            [
                [4., 3.],
                [57.16870876531574, 12.904160240195779],
                [56., 44.90416024019578],
            ],
            113.16870876531574,
            84.90416024019578,
        ),
    ];
    for (mut options, expected, width, height) in cases {
        options.parent_center = Point { x: 150., y: 100. };
        let result = layout(
            &nodes,
            &[],
            &options,
            &mut || panic!("distinct centers"),
            &mut NoopWorkControl,
        )
        .unwrap();
        close(result.width, width);
        close(result.height, height);
        for (actual, expected) in result.nodes.iter().zip(expected) {
            close(actual.x, expected[0]);
            close(actual.y, expected[1]);
        }
    }
}
