use super::*;
use crate::work::NoopWorkControl;

fn nodes() -> Vec<Node> {
    (0..3)
        .map(|i| Node {
            width: 30.0 + i as f64 * 10.0,
            height: 20.0 + i as f64 * 5.0,
            x: i as f64 * 75.0,
            y: i as f64 * 40.0,
            ..Node::default()
        })
        .collect()
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual {actual}, expected {expected}"
    );
}

// Real elkjs 0.9.3 bundled engine, Eclipse ELK 0.9.1. Each case uses id=g, three
// nodes from nodes(), algorithm=force and the explicitly changed options below.
#[test]
fn source_oracles_cover_force_models_components_cycles_and_interactive_positions() {
    let cases = [
        (
            "chain",
            vec![(0, 1), (1, 2)],
            Options::default(),
            248.94549650305896,
            274.1919849597678,
            [
                [117.94717668061813, 204.1919849597678],
                [50.0, 125.71669201399993],
                [148.94549650305896, 50.0],
            ],
        ),
        (
            "cycle",
            vec![(0, 1), (1, 2), (2, 0)],
            Options::default(),
            251.31102313348725,
            200.33392708171107,
            [
                [141.33232416400875, 130.33392708171107],
                [50.0, 96.63619437990882],
                [151.31102313348725, 50.0],
            ],
        ),
        (
            "components",
            vec![(0, 1)],
            Options::default(),
            211.35474862684026,
            243.79110078787846,
            [
                [131.35474862684026, 63.79110078787845],
                [50.0, 50.0],
                [50.0, 163.79110078787846],
            ],
        ),
        (
            "eades",
            vec![(0, 1), (1, 2)],
            Options {
                model: Model::Eades,
                ..Options::default()
            },
            314.0371803133746,
            345.92456863026746,
            [
                [50.0, 275.92456863026746],
                [117.4175975541524, 159.05139309699945],
                [214.0371803133746, 50.0],
            ],
        ),
        (
            "interactive",
            vec![(0, 1), (1, 2)],
            Options {
                interactive: true,
                ..Options::default()
            },
            293.414658449967,
            206.5015373015449,
            [
                [50.0, 50.0],
                [117.93935291458632, 86.24903123587397],
                [193.41465844996702, 126.50153730154489],
            ],
        ),
    ];
    for (name, pairs, options, width, height, positions) in cases {
        let edges: Vec<_> = pairs.into_iter().map(|(s, t)| Edge::new(s, t)).collect();
        let actual = layout(&nodes(), &edges, &options, &mut NoopWorkControl).expect(name);
        close(actual.width, width);
        close(actual.height, height);
        for (actual, expected) in actual.nodes.iter().zip(positions) {
            close(actual.x, expected[0]);
            close(actual.y, expected[1]);
        }
    }
}

#[test]
fn source_label_particles_and_clipped_routes_match_real_elkjs() {
    let mut edges = vec![Edge::new(0, 1), Edge::new(1, 2)];
    for edge in &mut edges {
        edge.labels.push(Label {
            width: 24.0,
            height: 10.0,
            ..Label::default()
        });
    }
    let result = layout(&nodes(), &edges, &Options::default(), &mut NoopWorkControl).unwrap();
    close(result.width, 255.37185045727634);
    close(result.height, 287.42295946195344);
    let expected = [
        (
            [124.0386853925297, 217.42295946195344],
            [78.93009542411315, 154.2817109280719],
            [105.59138086591011, 169.60233519501267],
        ),
        (
            [87.96844735601672, 129.2817109280719],
            [158.80971363005625, 80.0],
            [130.18592522863815, 108.39085546403595],
        ),
    ];
    for (edge, (start, end, label)) in result.edges.iter().zip(expected) {
        let edge = edge.as_ref().unwrap();
        close(edge.start.x, start[0]);
        close(edge.start.y, start[1]);
        close(edge.end.x, end[0]);
        close(edge.end.y, end[1]);
        close(edge.labels[0].x, label[0]);
        close(edge.labels[0].y, label[1]);
    }
}

#[test]
fn source_excludes_self_loops_and_preserves_zero_sized_nodes() {
    let node = Node::default();
    let result = layout(
        &[node],
        &[Edge::new(0, 0)],
        &Options::default(),
        &mut NoopWorkControl,
    )
    .unwrap();
    assert_eq!(result.edges, vec![None]);
    assert_eq!(result.nodes, vec![Point { x: 50.5, y: 50.5 }]);
    assert_eq!((result.width, result.height), (101.0, 101.0));
}

#[test]
fn work_interruption_propagates_during_force_iterations() {
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
    assert_eq!(
        layout(
            &nodes(),
            &[Edge::new(0, 1), Edge::new(1, 2)],
            &Options::default(),
            &mut Budget(100)
        ),
        Err(Error::Work(WorkError::Interrupted))
    );
}

#[test]
fn invalid_dimensions_and_underflowing_temperature_fail_before_iteration() {
    let mut invalid = nodes();
    invalid[0].height = f64::NAN;
    assert!(matches!(
        layout(&invalid, &[], &Options::default(), &mut NoopWorkControl),
        Err(Error::InvalidNode { .. })
    ));
    assert_eq!(
        layout(
            &nodes(),
            &[],
            &Options {
                temperature: f64::from_bits(1),
                ..Options::default()
            },
            &mut NoopWorkControl
        ),
        Err(Error::InvalidOption("temperature"))
    );
}

#[test]
fn coincident_large_coordinates_fail_without_an_unbounded_jitter_loop() {
    let nodes = vec![
        Node {
            width: 30.0,
            height: 20.0,
            x: 1e20,
            y: 1e20,
            ..Node::default()
        };
        2
    ];
    assert_eq!(
        layout(
            &nodes,
            &[Edge::new(0, 1)],
            &Options {
                interactive: true,
                ..Options::default()
            },
            &mut NoopWorkControl
        ),
        Err(Error::NumericStagnation)
    );
}

#[test]
fn parallel_edges_are_charged_on_every_iteration() {
    struct IterationLimit;
    impl WorkControl for IterationLimit {
        fn check(&mut self, units: usize) -> Result<(), WorkError> {
            if units > 25 {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.check(units)
        }
    }
    // Disabling splitting avoids a bulk component-import charge; the per-iteration edge
    // traversal is then the first operation exceeding the single-charge ceiling.
    let input = vec![
        Node {
            width: 30.0,
            height: 20.0,
            ..Node::default()
        };
        2
    ];
    let edges = vec![Edge::new(0, 1); 24];
    assert_eq!(
        layout(
            &input,
            &edges,
            &Options {
                separate_components: false,
                ..Options::default()
            },
            &mut IterationLimit
        ),
        Err(Error::Work(WorkError::Interrupted))
    );
}
