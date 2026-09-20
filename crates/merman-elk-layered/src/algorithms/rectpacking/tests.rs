use super::*;
use crate::work::NoopWorkControl;

fn mermaid_options() -> Options {
    Options {
        aspect_ratio: 1.6,
        try_box: true,
        expand_nodes: true,
        row_height_reevaluation: true,
        compaction_iterations: 10,
        eliminate_whitespace: true,
        ..Default::default()
    }
}

fn check(sizes: &[(f64, f64)], options: Options, bounds: (f64, f64), expected: &[[f64; 4]]) {
    let rectangles: Vec<_> = sizes
        .iter()
        .map(|&(width, height)| Rectangle {
            width,
            height,
            ..Default::default()
        })
        .collect();
    let result = layout(&rectangles, &options, &mut NoopWorkControl).unwrap();
    assert!(
        (result.width - bounds.0).abs() < 1e-8,
        "width: {} != {}",
        result.width,
        bounds.0
    );
    assert!(
        (result.height - bounds.1).abs() < 1e-8,
        "height: {} != {}",
        result.height,
        bounds.1
    );
    assert_eq!(result.rectangles.len(), expected.len());
    for (index, (actual, expected)) in result.rectangles.iter().zip(expected).enumerate() {
        for (coordinate, (actual, expected)) in [actual.x, actual.y, actual.width, actual.height]
            .into_iter()
            .zip(expected)
            .enumerate()
        {
            assert!(
                (actual - expected).abs() < 1e-8,
                "node {index}, coordinate {coordinate}: {actual} != {expected}"
            );
        }
    }
}

// Numeric results from the pinned elkjs 0.9.3 public layout API, with the exact Mermaid 12
// RECTPACKING_OPTIONS (including the unrecognized SCANLINE string, resolved as GREEDY).
#[test]
fn mermaid_compaction_stacks_and_expands_rectangles_like_elkjs() {
    check(
        &[(50., 140.), (30., 30.), (70., 35.), (40., 25.), (20., 80.)],
        mermaid_options(),
        (200., 170.),
        &[
            [15., 15., 50., 140.],
            [80., 15., 70., 36.666666666666664],
            [80., 66.66666666666666, 70., 41.666666666666664],
            [80., 123.33333333333333, 70., 31.666666666666668],
            [165., 15., 20., 140.],
        ],
    );
    check(
        &[
            (80., 160.),
            (40., 20.),
            (60., 30.),
            (100., 55.),
            (30., 70.),
            (45., 35.),
            (90., 20.),
        ],
        mermaid_options(),
        (330., 190.),
        &[
            [15., 15., 80., 160.],
            [110., 15., 85., 33.333333333333336],
            [210., 15., 105., 33.333333333333336],
            [110., 63.333333333333336, 100., 73.33333333333333],
            [225., 63.333333333333336, 30., 73.33333333333333],
            [270., 63.333333333333336, 45., 73.33333333333333],
            [110., 151.66666666666666, 205., 23.333333333333332],
        ],
    );
}

#[test]
fn source_trybox_branch_is_distinct_from_rectpacking() {
    let sizes = [(40., 30.), (60., 40.), (80., 20.), (40., 30.)];
    check(
        &sizes,
        mermaid_options(),
        (145., 150.),
        &[
            [15., 15., 40., 40.],
            [70., 15., 60., 40.],
            [15., 70., 115., 20.],
            [15., 105., 115., 30.],
        ],
    );
    check(
        &sizes,
        Options {
            try_box: false,
            ..mermaid_options()
        },
        (165., 115.),
        &[
            [15., 15., 50., 40.],
            [80., 15., 70., 40.],
            [15., 70., 80., 30.],
            [110., 70., 40., 30.],
        ],
    );
}

#[test]
fn minimum_dimensions_and_asymmetric_padding_match_elkjs() {
    let sizes = [(50., 140.), (30., 30.), (70., 35.), (40., 25.), (20., 80.)];
    check(
        &sizes,
        Options {
            minimum_width: 500.,
            minimum_height: 400.,
            ..mermaid_options()
        },
        (500., 400.),
        &[
            [15., 15., 167.5, 370.],
            [197.5, 15., 69.16666666666666, 155.],
            [281.6666666666667, 15., 109.16666666666666, 155.],
            [405.8333333333333, 15., 79.16666666666666, 155.],
            [197.5, 185., 287.5, 200.],
        ],
    );
    check(
        &sizes,
        Options {
            spacing: 10.,
            padding: Padding {
                top: 33.,
                left: 10.,
                right: 12.,
                bottom: 14.,
            },
            ..mermaid_options()
        },
        (182., 187.),
        &[
            [10., 33., 50., 140.],
            [70., 33., 70., 40.],
            [70., 83., 70., 45.],
            [70., 138., 70., 35.],
            [150., 33., 20., 140.],
        ],
    );
}

#[test]
fn source_defaults_and_small_inputs_match_elkjs() {
    check(
        &[(50., 140.), (30., 30.), (70., 35.), (40., 25.), (20., 80.)],
        Options::default(),
        (200., 170.),
        &[
            [15., 15., 50., 140.],
            [80., 15., 30., 30.],
            [80., 60., 70., 35.],
            [80., 110., 40., 25.],
            [165., 15., 20., 80.],
        ],
    );
    check(
        &[(70., 40.)],
        mermaid_options(),
        (100., 70.),
        &[[15., 15., 70., 40.]],
    );
    check(
        &[(70., 40.), (20., 80.)],
        mermaid_options(),
        (135., 110.),
        &[[15., 15., 70., 80.], [100., 15., 20., 80.]],
    );
    check(&[], mermaid_options(), (0., 0.), &[]);
}

#[test]
fn cancellation_and_invalid_numbers_do_not_mutate_input() {
    struct Deny;
    impl WorkControl for Deny {
        fn check(&mut self, _: usize) -> Result<(), WorkError> {
            Err(WorkError::Interrupted)
        }
        fn charge(&mut self, _: usize) -> Result<(), WorkError> {
            panic!("unadmitted work");
        }
    }
    let rectangles = [Rectangle {
        width: 50.,
        height: 100.,
        x: 123.,
        y: 456.,
        ..Default::default()
    }];
    let original = rectangles;
    assert_eq!(
        layout(&rectangles, &mermaid_options(), &mut Deny),
        Err(Error::Work(WorkError::Interrupted))
    );
    assert_eq!(rectangles, original);
    assert_eq!(
        layout(
            &rectangles,
            &Options {
                aspect_ratio: f64::NAN,
                ..Default::default()
            },
            &mut NoopWorkControl
        ),
        Err(Error::InvalidOption("aspect_ratio"))
    );
}

#[test]
fn budget_interrupts_the_compaction_phase_without_publishing_partial_geometry() {
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
    let rectangles: Vec<_> = [(50., 140.), (30., 30.), (70., 35.), (40., 25.), (20., 80.)]
        .into_iter()
        .map(|(width, height)| Rectangle {
            width,
            height,
            x: 123.,
            y: 456.,
            ..Default::default()
        })
        .collect();
    let before = rectangles.clone();
    let mut budget = Budget(120);
    assert_eq!(
        layout(&rectangles, &mermaid_options(), &mut budget),
        Err(Error::Work(WorkError::Interrupted))
    );
    assert!(budget.0 < 10);
    assert_eq!(rectangles, before);
}
