use super::*;
use crate::AsciiError;
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_model::{CancelReason, OperationControl};

#[test]
fn indexed_neighbors_match_geometric_neighbors_on_sparse_and_extreme_axes() {
    let policy = AsciiResourcePolicy::unbounded();
    let resources = ResourceContext::new(policy);
    let execution = AsciiExecution::for_test(&policy);
    let axes = ChannelAxes {
        x: vec![0, 1, 17, 91, usize::MAX],
        y: vec![0, 9, 1024, usize::MAX - 1, usize::MAX],
    };
    for &x in &axes.x {
        for &y in &axes.y {
            let coord = CanvasCoord { x, y };
            let point = axes.locate(coord, &resources, execution).unwrap().unwrap();
            assert_eq!(axes.coord(point), Some(coord));
            for heading in super::super::DIRECTIONS {
                let expected = match heading {
                    StepDirection::Left => axes
                        .x
                        .iter()
                        .rfind(|&&value| value < x)
                        .map(|&value| CanvasCoord { x: value, y }),
                    StepDirection::Right => axes
                        .x
                        .iter()
                        .find(|&&value| value > x)
                        .map(|&value| CanvasCoord { x: value, y }),
                    StepDirection::Up => axes
                        .y
                        .iter()
                        .rfind(|&&value| value < y)
                        .map(|&value| CanvasCoord { x, y: value }),
                    StepDirection::Down => axes
                        .y
                        .iter()
                        .find(|&&value| value > y)
                        .map(|&value| CanvasCoord { x, y: value }),
                };
                let actual = axes
                    .neighbor(point, heading, &resources, execution)
                    .unwrap()
                    .and_then(|next| axes.coord(next));
                assert_eq!(actual, expected, "{coord:?}, {heading:?}");
            }
        }
    }
}

#[test]
fn indexed_coordinate_resolution_rejects_missing_axes_and_nonmembers() {
    let policy = AsciiResourcePolicy::unbounded();
    let resources = ResourceContext::new(policy);
    let execution = AsciiExecution::for_test(&policy);
    for (x, y) in [(vec![], vec![]), (vec![7], vec![]), (vec![], vec![11])] {
        let axes = ChannelAxes { x, y };
        assert!(
            axes.locate(CanvasCoord { x: 7, y: 11 }, &resources, execution)
                .unwrap()
                .is_none()
        );
    }
    let axes = ChannelAxes {
        x: vec![7],
        y: vec![11],
    };
    let point = axes
        .locate(CanvasCoord { x: 7, y: 11 }, &resources, execution)
        .unwrap()
        .unwrap();
    for heading in super::super::DIRECTIONS {
        assert!(
            axes.neighbor(point, heading, &resources, execution)
                .unwrap()
                .is_none()
        );
    }
    for coord in [CanvasCoord { x: 6, y: 11 }, CanvasCoord { x: 7, y: 12 }] {
        assert!(axes.locate(coord, &resources, execution).unwrap().is_none());
    }
    let invalid = ChannelPoint {
        column: usize::MAX,
        row: usize::MAX,
    };
    assert!(axes.coord(invalid).is_none());
    for heading in super::super::DIRECTIONS {
        assert!(
            axes.neighbor(invalid, heading, &resources, execution)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn indexed_adjacency_work_is_independent_of_axis_length() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let mut observations = Vec::new();
    for count in [8usize, 64, 512, 4096] {
        let axes = ChannelAxes {
            x: (0..count).map(|index| index * 10).collect(),
            y: vec![0, 10, 20],
        };
        let setup = ResourceContext::new(policy);
        let point = axes
            .locate(
                CanvasCoord {
                    x: (count / 2) * 10,
                    y: 10,
                },
                &setup,
                execution,
            )
            .unwrap()
            .unwrap();
        assert!(setup.layout_work_used() <= count.ilog2() as usize + 4);
        let queries = ResourceContext::new(policy);
        for heading in super::super::DIRECTIONS.into_iter().cycle().take(512) {
            assert!(
                axes.neighbor(point, heading, &queries, execution)
                    .unwrap()
                    .is_some()
            );
        }
        observations.push((count, queries.layout_work_used()));
    }
    assert_eq!(observations, [(8, 512), (64, 512), (512, 512), (4096, 512)]);
}

#[test]
fn indexed_adjacency_honors_exact_and_n_minus_one_work_limits() {
    let axes = ChannelAxes {
        x: vec![0, 10, 20],
        y: vec![0, 10, 20],
    };
    let policy = AsciiResourcePolicy::unbounded();
    let point = axes
        .locate(
            CanvasCoord { x: 10, y: 10 },
            &ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        )
        .unwrap()
        .unwrap();
    for limit in [13, 12] {
        let bounded = policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
            .unwrap();
        let resources = ResourceContext::new(bounded);
        let execution = AsciiExecution::for_test(&bounded);
        let result = (0..13).try_for_each(|_| {
            axes.neighbor(point, StepDirection::Right, &resources, execution)
                .map(|_| ())
        });
        if limit == 13 {
            result.unwrap();
            assert_eq!(resources.layout_work_used(), 13);
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                if details.actual == 13 && details.max == 12)
            );
        }
    }
}

#[test]
fn indexed_resolution_and_adjacency_observe_cancellation() {
    let policy = AsciiResourcePolicy::unbounded();
    let axes = ChannelAxes {
        x: (0..4096).collect(),
        y: vec![0],
    };
    let control = OperationControl::new();
    control.cancel_after_checkpoints(4);
    let resources =
        ResourceContext::new(policy).controlled(control.clone(), OperationPhase::Layout);
    let error = axes
        .locate(
            CanvasCoord { x: 2048, y: 0 },
            &resources,
            AsciiExecution::new(&control, &policy),
        )
        .unwrap_err();
    assert!(matches!(error, AsciiError::Cancelled(cancelled)
        if cancelled.reason == CancelReason::Requested && cancelled.phase == OperationPhase::Layout));
    assert!(resources.layout_work_used() < 12);

    let control = OperationControl::new();
    control.cancel();
    let resources =
        ResourceContext::new(policy).controlled(control.clone(), OperationPhase::Layout);
    let error = axes
        .neighbor(
            ChannelPoint { column: 1, row: 0 },
            StepDirection::Right,
            &resources,
            AsciiExecution::new(&control, &policy),
        )
        .unwrap_err();
    assert!(matches!(error, AsciiError::Cancelled(cancelled)
        if cancelled.reason == CancelReason::Requested && cancelled.phase == OperationPhase::Layout));
    assert_eq!(resources.layout_work_used(), 0);
}

#[test]
fn indexed_frontier_positions_retain_the_physical_position_storage_size() {
    use super::super::{ChannelPosition, Position};
    assert_eq!(size_of::<ChannelPosition>(), size_of::<Position>());
}
