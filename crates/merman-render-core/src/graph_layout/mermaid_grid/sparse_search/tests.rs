use super::*;
use crate::resources::{RenderResourcePolicy, ResourceLimitId};

struct TestTopology {
    points: Vec<Point>,
    rows: Vec<Vec<GridSearchArc>>,
}

impl GridSearchTopology for TestTopology {
    fn vertex_count(&self) -> usize {
        self.points.len()
    }

    fn point(&self, vertex: usize) -> Option<Point> {
        self.points.get(vertex).copied()
    }

    fn outgoing(&self, vertex: usize) -> &[GridSearchArc] {
        &self.rows[vertex]
    }
}

fn square() -> TestTopology {
    let east = |to| GridSearchArc {
        to,
        axis: Axis::Horizontal,
        length: 100.0,
        crosses_boundary: false,
    };
    let south = |to| GridSearchArc {
        axis: Axis::Vertical,
        ..east(to)
    };
    TestTopology {
        points: [
            Point::ZERO,
            Point::new(0.0, 100.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
        ]
        .to_vec(),
        rows: vec![
            vec![east(2), south(1)],
            vec![east(3), south(0)],
            vec![east(0), south(3)],
            vec![east(1), south(2)],
        ],
    }
}

fn query(source: usize, target: usize) -> GridSearchQuery {
    GridSearchQuery {
        source: GridSearchTerminal {
            vertex: source,
            ..Default::default()
        },
        target: GridSearchTerminal {
            vertex: target,
            ..Default::default()
        },
        endpoint_candidate_rank: 0,
    }
}

fn meter() -> OperationWorkMeter {
    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
}

fn search(
    topology: &ValidatedGridSearchTopology<'_, TestTopology>,
    options: GridSearchOptions,
    workspace: &mut GridSearchWorkspace,
    budget: &mut GridSearchBudget,
    work: &OperationWorkMeter,
) -> Result<Option<GridSparseRoute>, GridSearchError> {
    find_grid_sparse_route(
        topology,
        query(0, 3),
        options,
        workspace,
        budget,
        work,
        |_, _| Ok(true),
    )
}

#[test]
fn strict_tuple_order_preserves_subpixel_length_and_signed_zero() {
    let cost = GridRouteCost {
        length: 10.0,
        bends: 2,
        boundary_transitions: 0,
        endpoint_candidate_rank: 0,
    };
    assert!(cost.compare(GridRouteCost { bends: 3, ..cost }).is_lt());
    assert!(
        cost.compare(GridRouteCost {
            length: 10.0000000001,
            bends: 0,
            ..cost
        })
        .is_lt()
    );
    assert!(
        cost.compare(GridRouteCost {
            boundary_transitions: 1,
            ..cost
        })
        .is_lt()
    );
    assert!(
        cost.compare(GridRouteCost {
            endpoint_candidate_rank: 1,
            ..cost
        })
        .is_lt()
    );
    assert_eq!(
        GridRouteCost {
            length: -0.0,
            ..cost
        }
        .compare(GridRouteCost {
            length: 0.0,
            ..cost
        }),
        Ordering::Equal
    );
}

#[test]
fn canonical_chain_converges_under_both_heuristics_and_seeded_queue_orders() {
    let graph = square();
    let work = meter();
    let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
    let mut workspace = GridSearchWorkspace::default();
    let mut budget = GridSearchBudget::default();
    for heuristic in [GridSearchHeuristic::BendAware, GridSearchHeuristic::Zero] {
        for queue_order in [
            GridSearchQueueOrder::Canonical,
            GridSearchQueueOrder::Reverse,
        ]
        .into_iter()
        .chain((0..16).map(GridSearchQueueOrder::Seeded))
        {
            let options = GridSearchOptions {
                heuristic,
                queue_order,
                ..Default::default()
            };
            let route = search(&topology, options, &mut workspace, &mut budget, &work)
                .unwrap()
                .unwrap();
            assert_eq!(route.vertex_ids, [0, 1, 3]);
            assert_eq!(
                route.points,
                [graph.points[0], graph.points[1], graph.points[3]]
            );
            assert_eq!((route.cost.length, route.cost.bends), (200.0, 1));
        }
    }
}

#[test]
fn local_caps_keep_a_found_goal_and_report_the_exhausted_scope() {
    let graph = square();
    let work = meter();
    let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
    let mut workspace = GridSearchWorkspace::default();
    for (caps, scope) in [
        (
            GridSearchCaps {
                max_expanded_states: 3,
                ..Default::default()
            },
            GridSearchStateScope::Edge,
        ),
        (
            GridSearchCaps {
                max_invocation_expanded_states: 3,
                ..Default::default()
            },
            GridSearchStateScope::Invocation,
        ),
    ] {
        let options = GridSearchOptions {
            caps,
            queue_order: GridSearchQueueOrder::Reverse,
            ..Default::default()
        };
        let mut budget = GridSearchBudget::default();
        let route = search(&topology, options, &mut workspace, &mut budget, &work)
            .unwrap()
            .unwrap();
        assert_eq!(route.vertex_ids, [0, 2, 3]);
        assert_eq!(budget.expanded_states(), 3);

        let caps = GridSearchCaps {
            max_expanded_states: 0,
            ..caps
        };
        assert_eq!(
            budget.check_available(caps),
            Err(GridSearchLimit::StateCap { scope })
        );
    }

    let options = GridSearchOptions {
        caps: GridSearchCaps {
            max_expanded_states: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut budget = GridSearchBudget::default();
    assert!(matches!(
        search(&topology, options, &mut workspace, &mut budget, &work),
        Err(GridSearchError::Limit(GridSearchLimit::StateCap {
            scope: GridSearchStateScope::Edge
        }))
    ));
    assert_eq!(budget.expanded_states(), 1);
    let reused = search(
        &topology,
        GridSearchOptions::default(),
        &mut workspace,
        &mut budget,
        &work,
    )
    .unwrap()
    .unwrap();
    assert_eq!(reused.vertex_ids, [0, 1, 3]);
    assert!(budget.expanded_states() > 1);
}

#[test]
fn seeded_queue_exercises_both_valid_capped_goals() {
    let graph = square();
    let work = meter();
    let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
    let mut workspace = GridSearchWorkspace::default();
    let mut visited_middle = [false; 2];
    for seed in 0..64 {
        let options = GridSearchOptions {
            caps: GridSearchCaps {
                max_expanded_states: 3,
                ..Default::default()
            },
            queue_order: GridSearchQueueOrder::Seeded(seed),
            ..Default::default()
        };
        let mut budget = GridSearchBudget::default();
        let route = search(&topology, options, &mut workspace, &mut budget, &work);
        if let Ok(Some(route)) = route {
            assert_eq!((route.cost.length, route.cost.bends), (200.0, 1));
            visited_middle[route.vertex_ids[1] - 1] = true;
        } else {
            assert!(matches!(
                route,
                Err(GridSearchError::Limit(GridSearchLimit::StateCap {
                    scope: GridSearchStateScope::Edge
                }))
            ));
        }
        assert_eq!(budget.expanded_states(), 3);
    }
    assert_eq!(visited_middle, [true, true]);
}

#[test]
fn channel_seed_owner_directions_and_ordered_arc_filter_apply_to_the_pair() {
    let mut graph = square();
    graph.rows[0][1].crosses_boundary = true;
    let work = meter();
    let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
    let mut workspace = GridSearchWorkspace::default();
    let mut budget = GridSearchBudget::default();
    let route = search(
        &topology,
        GridSearchOptions::default(),
        &mut workspace,
        &mut budget,
        &work,
    )
    .unwrap()
    .unwrap();
    assert_eq!(route.vertex_ids, [0, 2, 3]); // Equal length and bends, fewer portal transitions.

    let mut query = query(0, 3);
    query.source.orientation = Some(Axis::Horizontal);
    query.source.owner_side = Some(Side::Left);
    query.source.approach_length = 20.0;
    query.target.orientation = Some(Axis::Vertical);
    query.target.owner_side = Some(Side::Bottom);
    query.target.approach_length = 12.0;
    query.endpoint_candidate_rank = 7;
    let mut examined = Vec::new();
    let route = find_grid_sparse_route(
        &topology,
        query,
        GridSearchOptions::default(),
        &mut workspace,
        &mut budget,
        &work,
        |from, to| {
            examined.push((from, to));
            Ok(to != graph.points[0])
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        examined[..2],
        [
            (graph.points[0], graph.points[2]),
            (graph.points[0], graph.points[1])
        ]
    );
    assert_eq!(route.vertex_ids, [0, 1, 3]);
    assert_eq!(
        route.cost,
        GridRouteCost {
            length: 232.0,
            bends: 3,
            boundary_transitions: 1,
            endpoint_candidate_rank: 7
        }
    );

    let none = find_grid_sparse_route(
        &topology,
        query,
        Default::default(),
        &mut workspace,
        &mut budget,
        &work,
        |_, _| Ok(false),
    )
    .unwrap();
    assert!(none.is_none());
}

#[test]
fn reconstructs_the_raw_chain_and_compresses_collinear_points() {
    let graph = TestTopology {
        points: vec![
            Point::ZERO,
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(20.0, 5.0),
        ],
        rows: vec![
            vec![GridSearchArc {
                to: 1,
                axis: Axis::Horizontal,
                length: 10.0,
                crosses_boundary: false,
            }],
            vec![GridSearchArc {
                to: 2,
                axis: Axis::Horizontal,
                length: 10.0,
                crosses_boundary: true,
            }],
            vec![GridSearchArc {
                to: 3,
                axis: Axis::Vertical,
                length: 5.0,
                crosses_boundary: false,
            }],
            vec![],
        ],
    };
    let work = meter();
    let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
    let route = search(
        &topology,
        Default::default(),
        &mut GridSearchWorkspace::default(),
        &mut GridSearchBudget::default(),
        &work,
    )
    .unwrap()
    .unwrap();
    assert_eq!(route.vertex_ids, [0, 1, 2, 3]);
    assert_eq!(
        route.points,
        [graph.points[0], graph.points[2], graph.points[3]]
    );
    assert_eq!(route.cost.boundary_transitions, 1);
}

#[test]
fn invalid_arcs_or_predecessors_fail_with_a_model_error() {
    let mut graph = square();
    graph.rows[0][0].to = 3;
    assert!(matches!(
        ValidatedGridSearchTopology::new(&graph, &meter()),
        Err(Error::InvalidModel { .. })
    ));
    graph.rows[0][0].to = usize::MAX;
    assert!(matches!(
        ValidatedGridSearchTopology::new(&graph, &meter()),
        Err(Error::InvalidModel { .. })
    ));

    let cost = GridRouteCost {
        length: 0.0,
        bends: 0,
        boundary_transitions: 0,
        endpoint_candidate_rank: 0,
    };
    let root = SearchState {
        vertex: 0,
        channel: Channel::Start,
        cost,
        parent: None,
        depth: 1,
    };
    let bad = SearchState {
        vertex: 1,
        parent: Some(1),
        depth: 2,
        ..root
    };
    assert!(matches!(
        reconstruct(&[root, bad], 1, cost, 0, |_| Ok(Point::ZERO), &meter()),
        Err(Error::InvalidModel { .. })
    ));
}

#[test]
fn memory_and_work_admission_are_cumulative_and_retry_safe() {
    let graph = square();
    let work = meter();
    let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
    let mut workspace = GridSearchWorkspace::default();
    let mut budget = GridSearchBudget::default();
    let local = GridSearchOptions {
        estimated_bytes_base: 500,
        caps: GridSearchCaps {
            max_estimated_bytes: Some(500),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        matches!(search(&topology, local, &mut workspace, &mut budget, &work), Err(GridSearchError::Limit(GridSearchLimit::EstimatedMemory { actual, max: 500 })) if actual > 500)
    );
    assert_eq!(budget.expanded_states(), 0);
    assert!(
        search(
            &topology,
            Default::default(),
            &mut workspace,
            &mut budget,
            &work
        )
        .unwrap()
        .is_some()
    );
    let exact = work.used();
    assert!(exact > 1);

    for max in [exact - 1, exact] {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, max)
            .unwrap();
        let work = OperationWorkMeter::new(policy);
        let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
        let result = search(
            &topology,
            Default::default(),
            &mut GridSearchWorkspace::default(),
            &mut GridSearchBudget::default(),
            &work,
        );
        if max < exact {
            assert!(
                matches!(result, Err(GridSearchError::Layout(Error::ResourceLimitExceeded(error))) if error.actual == exact && error.max == max)
            );
        } else {
            assert!(result.unwrap().is_some());
            assert_eq!(work.used(), exact);
        }
    }
}

#[test]
fn cancellation_during_arc_filter_stops_in_layout_without_rolling_back_consumed_states() {
    let graph = square();
    let work = meter();
    let topology = ValidatedGridSearchTopology::new(&graph, &work).unwrap();
    let mut workspace = GridSearchWorkspace::default();
    let mut budget = GridSearchBudget::default();
    let result = find_grid_sparse_route(
        &topology,
        query(0, 3),
        Default::default(),
        &mut workspace,
        &mut budget,
        &work,
        |_, _| {
            work.control().cancel();
            Ok(true)
        },
    );
    assert!(
        matches!(result, Err(GridSearchError::Layout(Error::Cancelled(error))) if error.phase == OperationPhase::Layout)
    );
    assert_eq!(budget.expanded_states(), 1);
    assert!(matches!(
        search(
            &topology,
            Default::default(),
            &mut workspace,
            &mut budget,
            &work
        ),
        Err(GridSearchError::Layout(Error::Cancelled(_)))
    ));
    assert_eq!(budget.expanded_states(), 1);
}
