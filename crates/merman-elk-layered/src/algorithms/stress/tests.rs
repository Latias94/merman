use super::*;
use crate::work::NoopWorkControl;

fn nodes() -> Vec<Node> {
    (0..3)
        .map(|i| Node {
            geometry: force::Node {
                width: 30.0 + i as f64 * 10.0,
                height: 20.0 + i as f64 * 5.0,
                x: i as f64 * 75.0,
                y: i as f64 * 40.0,
                ..force::Node::default()
            },
            fixed: false,
        })
        .collect()
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual {actual}, expected {expected}"
    );
}

// Real elkjs 0.9.3 bundled engine (ELK 0.9.1), id=g, algorithm=stress, nodes()
// dimensions/positions, seed=1. Options and edge overrides are explicit per case.
#[test]
fn source_oracles_cover_initialization_components_cycles_fixed_and_dimension_options() {
    let cases = [
        (
            "chain",
            vec![(0, 1), (1, 2)],
            185.05196774162908,
            319.802695232483,
            [
                [50.0, 249.80269523248302],
                [64.71309018287621, 149.25081361173528],
                [85.05196774162907, 50.0],
            ],
        ),
        (
            "cycle",
            vec![(0, 1), (1, 2), (2, 0)],
            236.06017430729628,
            224.52467266778305,
            [
                [136.32161535996556, 154.52467266778305],
                [50.0, 93.82849688925094],
                [136.06017430729628, 50.0],
            ],
        ),
        (
            "components",
            vec![(0, 1)],
            233.92423315100032,
            247.12860538419605,
            [
                [153.92423315100032, 67.12860538419605],
                [50.0, 50.0],
                [50.0, 167.12860538419605],
            ],
        ),
        (
            "interactive",
            vec![(0, 1), (1, 2)],
            316.62314389148213,
            218.8310451923499,
            [
                [50.0, 50.0],
                [133.31157194574106, 94.41552259617495],
                [216.62314389148213, 138.8310451923499],
            ],
        ),
        (
            "x_fixed_weighted",
            vec![(0, 1), (1, 2)],
            280.30889832467835,
            210.0,
            [
                [50.0, 50.0],
                [94.4287012234177, 90.0],
                [180.30889832467835, 130.0],
            ],
        ),
        (
            "y_once",
            vec![(0, 1), (1, 2)],
            300.0,
            220.3786202771862,
            [
                [50.0, 50.0],
                [125.0, 94.83685308642478],
                [200.0, 140.3786202771862],
            ],
        ),
    ];
    for (name, edge_pairs, width, height, positions) in cases {
        let mut input = nodes();
        let mut edges: Vec<_> = edge_pairs
            .into_iter()
            .map(|(a, b)| Edge::new(a, b))
            .collect();
        let mut options = Options::default();
        match name {
            "interactive" => options.force.interactive = true,
            "x_fixed_weighted" => {
                options.force.interactive = true;
                options.dimension = Dimension::X;
                options.iteration_limit = 7;
                input[1].fixed = true;
                edges[0].desired_length = Some(65.0);
            }
            "y_once" => {
                options.force.interactive = true;
                options.dimension = Dimension::Y;
                options.iteration_limit = 0;
            }
            _ => {}
        }
        let result = layout(&input, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, width);
        close(result.height, height);
        for (actual, expected) in result.nodes.iter().zip(positions) {
            close(actual.x, expected[0]);
            close(actual.y, expected[1]);
        }
    }
}

#[test]
fn source_oracle_refreshes_labels_after_majorization_and_clips_routes() {
    let edges: Vec<_> = [(0, 1), (1, 2), (2, 0)]
        .into_iter()
        .enumerate()
        .map(|(i, (a, b))| {
            let mut edge = Edge::new(a, b);
            edge.geometry.labels.push(force::Label {
                width: 25.0,
                height: 12.0,
                inline: i != 0,
                ..force::Label::default()
            });
            edge
        })
        .collect();
    let result = layout(&nodes(), &edges, &Options::default(), &mut NoopWorkControl).unwrap();
    close(result.width, 230.63107497758193);
    close(result.height, 222.43654527808593);
    let expected = [
        [
            150.63107497758193,
            157.85092131860029,
            90.0,
            139.31550064482897,
            122.81553748879097,
            130.81894032180037,
        ],
        [
            83.40405792021845,
            120.70133536551478,
            127.0491024537861,
            80.0,
            94.06698597902412,
            93.10066768275739,
        ],
        [
            146.59731873753327,
            80.0,
            163.32217712459192,
            152.43654527808593,
            141.88252346781508,
            107.71827263904297,
        ],
    ];
    for (edge, wanted) in result.edges.iter().zip(expected) {
        let edge = edge.as_ref().unwrap();
        for (actual, expected) in [
            edge.start.x,
            edge.start.y,
            edge.end.x,
            edge.end.y,
            edge.labels[0].x,
            edge.labels[0].y,
        ]
        .into_iter()
        .zip(wanted)
        {
            close(actual, expected);
        }
    }
}

#[test]
fn stress_admits_quadratic_state_before_allocation() {
    struct Limit;
    impl WorkControl for Limit {
        fn check(&mut self, units: usize) -> Result<(), WorkError> {
            if units > 100 {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.check(units)
        }
    }
    let input = vec![Node::default(); 10];
    let edges: Vec<_> = (0..9).map(|i| Edge::new(i, i + 1)).collect();
    let options = Options {
        force: force::Options {
            interactive: true,
            ..force::Options::default()
        },
        ..Options::default()
    };
    assert_eq!(
        layout(&input, &edges, &options, &mut Limit),
        Err(Error::Work(WorkError::Interrupted))
    );
}

#[test]
fn invalid_lengths_fail_without_mutating_input() {
    let input = nodes();
    let saved = input.clone();
    let mut edge = Edge::new(0, 1);
    edge.desired_length = Some(0.0);
    assert_eq!(
        layout(&input, &[edge], &Options::default(), &mut NoopWorkControl),
        Err(Error::InvalidEdgeLength(0))
    );
    assert_eq!(input, saved);
}
