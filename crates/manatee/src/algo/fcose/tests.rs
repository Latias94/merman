use super::{
    BoundsExtras, CompoundTopology, Constraints, DisjointSet, FcoseConstraintWorkShape,
    FcoseIterationSchedule, FcoseRandom, FcoseRandomPolicy, FcoseRandomSource, FcoseWorkPlan,
    IndexedAlignmentConstraint, IndexedCompound, IndexedEdge, IndexedFcoseOptions, IndexedGraph,
    IndexedNode, IndexedRelativePlacementConstraint, Mulberry32, NoopWorkControl, RelConstraint,
    RepulsionGrid, RepulsionGridPlan, RepulsionGridStorageKind, SimGraph, SimNode, Vec2,
    WorkControl, XorShift64Star, admit_dynamic_work, apply_reflection_for_relative_placement,
    for_each_owner_local_pair, layout, layout_indexed,
    layout_indexed_with_random_policy_and_work_control, new_fcose_random, owner_local_pair_work,
    procrustes_transform_for_alignments, reset_fcose_random_for_run,
};
use crate::algo::{AlignmentConstraint, FcoseOptions, RelativePlacementConstraint};
use crate::error::{Error, WorkFailure};
use crate::graph::{Anchor, Compound, Edge, Graph, Node, Point};

#[derive(Default)]
struct RecordingWorkControl {
    charges: Vec<usize>,
    reject_after: Option<usize>,
}

impl WorkControl for RecordingWorkControl {
    fn charge(&mut self, units: usize) -> std::result::Result<(), WorkFailure> {
        if self
            .reject_after
            .is_some_and(|accepted| self.charges.len() >= accepted)
        {
            return Err(WorkFailure::Interrupted);
        }
        self.charges.push(units);
        Ok(())
    }
}

struct RejectingPreflight {
    checks: Vec<usize>,
    charges: Vec<usize>,
    reject_on_check: usize,
}

impl Default for RejectingPreflight {
    fn default() -> Self {
        Self {
            checks: Vec::new(),
            charges: Vec::new(),
            reject_on_check: 1,
        }
    }
}

impl WorkControl for RejectingPreflight {
    fn check(&mut self, units: usize) -> std::result::Result<(), WorkFailure> {
        self.checks.push(units);
        if self.checks.len() == self.reject_on_check {
            Err(WorkFailure::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), WorkFailure> {
        self.charges.push(units);
        Ok(())
    }
}

#[derive(Default)]
struct CheckOnlyBudget {
    max_units: usize,
    checks: Vec<usize>,
    charges: Vec<usize>,
}

impl WorkControl for CheckOnlyBudget {
    fn check(&mut self, units: usize) -> std::result::Result<(), WorkFailure> {
        self.checks.push(units);
        if units > self.max_units {
            Err(WorkFailure::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), WorkFailure> {
        self.charges.push(units);
        Ok(())
    }
}

fn node_at(left: f64, top: f64, w: f64, h: f64) -> SimNode {
    SimNode {
        parent: None,
        owner_idx: 0,
        is_compound: false,
        width: w,
        height: h,
        bounds_extras: BoundsExtras::default(),
        estimated_size: (w + h) / 2.0,
        left,
        top,
        spring_fx: 0.0,
        spring_fy: 0.0,
        repulsion_fx: 0.0,
        repulsion_fy: 0.0,
        gravitation_fx: 0.0,
        gravitation_fy: 0.0,
        no_of_children: 1.0,
        padding: 0.0,
        surrounding: Vec::new(),
        grid_start_x: 0,
        grid_finish_x: 0,
        grid_start_y: 0,
        grid_finish_y: 0,
    }
}

fn assert_point_close(actual: Point, expected: Point) {
    let dx = (actual.x - expected.x).abs();
    let dy = (actual.y - expected.y).abs();
    assert!(
        dx < 1e-9 && dy < 1e-9,
        "point mismatch: actual=({:.12},{:.12}) expected=({:.12},{:.12}) d=({:.3e},{:.3e})",
        actual.x,
        actual.y,
        expected.x,
        expected.y,
        dx,
        dy
    );
}

#[test]
fn iteration_schedule_normalizes_numbers_and_preserves_loop_boundary() {
    let empty = FcoseIterationSchedule::from_configured_number(None, 0, 0, true).unwrap();
    assert_eq!(empty.setup_work_units(), 0);
    assert_eq!(empty.iteration_work_units(), 0);
    assert_eq!(empty.maximum_work_units(), 0);

    let fallback = FcoseIterationSchedule::from_configured_number(None, 2, 1, true).unwrap();
    assert_eq!(fallback.configured_iterations(), 2500);
    assert_eq!(fallback.effective_max_iterations(), 2500);
    assert_eq!(fallback.run_count(), 2);
    assert_eq!(fallback.setup_work_units(), 40);
    assert_eq!(fallback.iteration_work_units(), 3);
    assert_eq!(fallback.maximum_work_units(), 40 + 2 * 2499 * 3);

    let node_floor =
        FcoseIterationSchedule::from_configured_number(Some(1.0), 2, 0, false).unwrap();
    assert_eq!(node_floor.configured_iterations(), 1);
    assert_eq!(node_floor.effective_max_iterations(), 10);
    assert_eq!(node_floor.setup_work_units(), 23);
    assert_eq!(node_floor.maximum_work_units(), 41);

    for configured in [Some(f64::NAN), Some(f64::INFINITY), Some(0.0), Some(0.9)] {
        assert_eq!(
            FcoseIterationSchedule::from_configured_number(configured, 1, 0, false)
                .unwrap()
                .configured_iterations(),
            2500,
        );
    }

    assert_eq!(
        FcoseIterationSchedule::from_configured_number(Some(1.49), 0, 0, false)
            .unwrap()
            .configured_iterations(),
        1,
    );
    assert_eq!(
        FcoseIterationSchedule::from_configured_number(Some(1.5), 0, 0, false)
            .unwrap()
            .configured_iterations(),
        2,
    );
}

#[test]
fn graph_cardinality_schedule_avoids_flat_log_ancestry_work() {
    const LEAF_COUNT: usize = 1_024;
    const EDGE_COUNT: usize = 1_024;

    let flat =
        FcoseIterationSchedule::from_normalized_graph_counts(5, LEAF_COUNT, 0, EDGE_COUNT, false)
            .expect("flat schedule");
    let count_only =
        FcoseIterationSchedule::from_normalized_counts(5, LEAF_COUNT, EDGE_COUNT, false)
            .expect("count-only conservative schedule");
    let flat_topology = CompoundTopology::work_units_with_compound_count(LEAF_COUNT, EDGE_COUNT, 0)
        .expect("flat topology work");

    assert_eq!(flat.iteration_work_units(), LEAF_COUNT + EDGE_COUNT);
    assert_eq!(
        flat.setup_work_units(),
        LEAF_COUNT + EDGE_COUNT + flat_topology
    );
    assert!(flat.setup_work_units() < count_only.setup_work_units());
}

#[test]
fn fcose_work_plan_uses_raw_input_and_filtered_runtime_cardinality() {
    let schedule = FcoseIterationSchedule::from_normalized_counts(5, 2, 1, true).unwrap();
    let shape = FcoseConstraintWorkShape::from_counts(2, 5, 3, 1, 3, 4);
    let plan = FcoseWorkPlan::from_schedule(schedule, 2, shape, 7, true).unwrap();

    assert_eq!(plan.setup_work_units(), 50);
    assert_eq!(plan.run_setup_work_units(), 7);
    assert_eq!(plan.iteration_work_units(), 11);
    assert_eq!(plan.maximum_work_units(), 276);
}

#[test]
fn constraint_work_shape_preserves_duplicates_and_counts_both_axes() {
    let options = IndexedFcoseOptions {
        alignment_constraint: Some(IndexedAlignmentConstraint {
            horizontal: vec![vec![0, 0, 1, 99], vec![2, 99], vec![99, 99]],
            vertical: vec![vec![1, 1]],
        }),
        relative_placement_constraint: vec![
            IndexedRelativePlacementConstraint {
                left: Some(0),
                right: Some(1),
                top: Some(0),
                bottom: Some(1),
                gap: 10.0,
            },
            IndexedRelativePlacementConstraint {
                left: Some(0),
                right: Some(99),
                top: Some(99),
                bottom: Some(1),
                gap: 10.0,
            },
            IndexedRelativePlacementConstraint {
                left: Some(0),
                right: Some(1),
                top: None,
                bottom: None,
                gap: 10.0,
            },
        ],
        ..IndexedFcoseOptions::default()
    };
    let (groups, _) = FcoseConstraintWorkShape::input_headers(&options).unwrap();
    let members = FcoseConstraintWorkShape::input_member_count(&options).unwrap();

    assert_eq!(
        FcoseConstraintWorkShape::from_indexed_options(&options, 3, groups, members).unwrap(),
        FcoseConstraintWorkShape::from_counts(4, 10, 3, 2, 5, 3),
    );
}

#[test]
fn iteration_schedule_fails_closed_on_numeric_and_derived_overflow() {
    assert_eq!(
        FcoseIterationSchedule::from_configured_number(Some(f64::MAX), 1, 0, false),
        Err(WorkFailure::ArithmeticOverflow),
    );
    assert_eq!(
        FcoseIterationSchedule::from_configured_number(Some(1.0), usize::MAX / 5 + 1, 0, false,),
        Err(WorkFailure::ArithmeticOverflow),
    );
    assert_eq!(
        FcoseIterationSchedule::from_normalized_counts(usize::MAX, 1, 0, true),
        Err(WorkFailure::ArithmeticOverflow),
    );
    assert_eq!(
        FcoseIterationSchedule::from_normalized_graph_counts(1, 2, 0, usize::MAX - 2, false,),
        Err(WorkFailure::ArithmeticOverflow),
    );
    assert_eq!(
        CompoundTopology::work_units(usize::MAX, 0),
        Err(WorkFailure::ArithmeticOverflow),
    );
    assert_eq!(
        CompoundTopology::work_units(1, usize::MAX),
        Err(WorkFailure::ArithmeticOverflow),
    );
    let schedule = FcoseIterationSchedule::from_normalized_counts(5, 1, 0, false).unwrap();
    assert_eq!(
        FcoseWorkPlan::from_schedule(
            schedule,
            1,
            FcoseConstraintWorkShape::from_counts(usize::MAX, 1, 0, 0, 0, 0),
            0,
            false,
        ),
        Err(WorkFailure::ArithmeticOverflow),
    );
    let rerun_schedule = FcoseIterationSchedule::from_normalized_counts(5, 1, 0, true).unwrap();
    assert_eq!(
        FcoseWorkPlan::from_schedule(
            rerun_schedule,
            1,
            FcoseConstraintWorkShape::default(),
            usize::MAX,
            true,
        ),
        Err(WorkFailure::ArithmeticOverflow),
    );
}

#[test]
fn work_plan_counts_seed_offset_for_each_rng_reset() {
    let schedule = FcoseIterationSchedule::from_normalized_counts(5, 1, 0, true).unwrap();
    let continuous =
        FcoseWorkPlan::from_schedule(schedule, 1, FcoseConstraintWorkShape::default(), 7, false)
            .unwrap();
    let reset =
        FcoseWorkPlan::from_schedule(schedule, 1, FcoseConstraintWorkShape::default(), 7, true)
            .unwrap();

    assert_eq!(continuous.maximum_work_units(), 26);
    assert_eq!(reset.maximum_work_units(), 33);
}

#[test]
fn controlled_layout_charges_each_iteration_and_grid_refresh_once() {
    let graph = IndexedGraph {
        nodes: vec![IndexedNode {
            parent: None,
            width: 40.0,
            height: 40.0,
            x: 0.0,
            y: 0.0,
            bounds_extras: BoundsExtras::default(),
        }],
        edges: Vec::new(),
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(5),
        rerun: true,
        ..IndexedFcoseOptions::default()
    };
    let mut control = RecordingWorkControl::default();

    layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut control,
    )
    .unwrap();

    assert_eq!(
        control.charges,
        vec![11, 1, 2, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1]
    );
}

#[test]
fn controlled_layout_runs_three_non_consuming_preflight_checks() {
    let graph = IndexedGraph {
        nodes: vec![IndexedNode {
            parent: None,
            width: 40.0,
            height: 40.0,
            x: 0.0,
            y: 0.0,
            bounds_extras: BoundsExtras::default(),
        }],
        edges: Vec::new(),
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(5),
        ..IndexedFcoseOptions::default()
    };
    let mut control = RejectingPreflight {
        reject_on_check: 3,
        ..RejectingPreflight::default()
    };

    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut control,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert_eq!(control.checks, vec![1, 1, 15]);
    assert!(control.charges.is_empty());
}

#[test]
fn controlled_layout_rejects_before_graph_validation() {
    let graph = IndexedGraph {
        nodes: vec![IndexedNode {
            parent: None,
            width: 40.0,
            height: 40.0,
            x: 0.0,
            y: 0.0,
            bounds_extras: BoundsExtras::default(),
        }],
        edges: vec![IndexedEdge {
            source: 0,
            target: 99,
            label_width: None,
            label_height: None,
            source_anchor: None,
            target_anchor: None,
            curve_style_segments: false,
            ideal_length: 50.0,
            elasticity: 0.45,
        }],
        compounds: Vec::new(),
    };
    let mut control = RejectingPreflight::default();

    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &IndexedFcoseOptions::default(),
        FcoseRandomPolicy::xorshift(1),
        &mut control,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert_eq!(control.checks, vec![2]);
    assert!(control.charges.is_empty());
}

#[test]
fn controlled_layout_preflight_includes_constraint_cardinality() {
    let graph = IndexedGraph {
        nodes: (0..2)
            .map(|index| IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: index as f64 * 50.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            })
            .collect(),
        edges: Vec::new(),
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(5),
        alignment_constraint: Some(IndexedAlignmentConstraint {
            horizontal: vec![vec![0, 1]; 8],
            vertical: Vec::new(),
        }),
        relative_placement_constraint: vec![
            IndexedRelativePlacementConstraint {
                left: Some(0),
                right: Some(1),
                top: None,
                bottom: None,
                gap: 50.0,
            };
            8
        ],
        ..IndexedFcoseOptions::default()
    };
    let mut control = RejectingPreflight {
        reject_on_check: 3,
        ..RejectingPreflight::default()
    };

    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut control,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert_eq!(control.checks, vec![10, 34, 390]);
    assert!(control.charges.is_empty());
}

#[test]
fn controlled_layout_preflight_includes_random_seed_offset() {
    let graph = IndexedGraph {
        nodes: vec![IndexedNode {
            parent: None,
            width: 40.0,
            height: 40.0,
            x: 0.0,
            y: 0.0,
            bounds_extras: BoundsExtras::default(),
        }],
        edges: Vec::new(),
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(5),
        ..IndexedFcoseOptions::default()
    };
    let seed_offset = 7;
    let mut control = RejectingPreflight {
        reject_on_check: 3,
        ..RejectingPreflight::default()
    };

    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1).with_seed_offset(seed_offset),
        &mut control,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert_eq!(control.checks, vec![1, 1, 22]);
    assert!(control.charges.is_empty());
}

#[test]
fn indexed_graph_schedule_charges_every_edge_slot() {
    let graph = IndexedGraph {
        nodes: vec![IndexedNode {
            parent: None,
            width: 40.0,
            height: 40.0,
            x: 0.0,
            y: 0.0,
            bounds_extras: BoundsExtras::default(),
        }],
        edges: vec![IndexedEdge {
            source: 0,
            target: 0,
            label_width: None,
            label_height: None,
            source_anchor: None,
            target_anchor: None,
            curve_style_segments: false,
            ideal_length: 50.0,
            elasticity: 0.45,
        }],
        compounds: Vec::new(),
    };

    let schedule = FcoseIterationSchedule::from_indexed_graph(Some(5), &graph, false).unwrap();

    assert_eq!(schedule.iteration_work_units(), 2);
    assert_eq!(schedule.setup_work_units(), 22);
    assert_eq!(schedule.maximum_work_units(), 30);
}

#[test]
fn controlled_layout_stops_before_a_rejected_iteration_tranche() {
    let graph = IndexedGraph {
        nodes: vec![IndexedNode {
            parent: None,
            width: 40.0,
            height: 40.0,
            x: 0.0,
            y: 0.0,
            bounds_extras: BoundsExtras::default(),
        }],
        edges: Vec::new(),
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(5),
        ..IndexedFcoseOptions::default()
    };
    let mut control = RecordingWorkControl {
        charges: Vec::new(),
        reject_after: Some(0),
    };

    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut control,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert!(control.charges.is_empty());
}

#[test]
fn relative_projection_clone_work_is_charged_before_allocation() {
    let graph = IndexedGraph {
        nodes: (0..3)
            .map(|index| IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: index as f64 * 50.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            })
            .collect(),
        edges: Vec::new(),
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(5),
        relative_placement_constraint: vec![
            IndexedRelativePlacementConstraint {
                left: Some(0),
                right: Some(1),
                top: None,
                bottom: None,
                gap: 50.0,
            },
            IndexedRelativePlacementConstraint {
                left: Some(1),
                right: Some(2),
                top: None,
                bottom: None,
                gap: 50.0,
            },
        ],
        ..IndexedFcoseOptions::default()
    };
    let mut control = RecordingWorkControl {
        charges: Vec::new(),
        reject_after: Some(2),
    };

    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut control,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert_eq!(control.charges, vec![30, 2]);
}

#[test]
fn controlled_randomized_layout_charges_spectral_setup_before_execution() {
    let graph = IndexedGraph {
        nodes: (0..3)
            .map(|index| IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: index as f64 * 10.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            })
            .collect(),
        edges: vec![
            IndexedEdge {
                source: 0,
                target: 1,
                label_width: None,
                label_height: None,
                source_anchor: None,
                target_anchor: None,
                curve_style_segments: false,
                ideal_length: 50.0,
                elasticity: 0.45,
            },
            IndexedEdge {
                source: 1,
                target: 2,
                label_width: None,
                label_height: None,
                source_anchor: None,
                target_anchor: None,
                curve_style_segments: false,
                ideal_length: 50.0,
                elasticity: 0.45,
            },
        ],
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: true,
        num_iter: Some(5),
        ..IndexedFcoseOptions::default()
    };
    let mut control = RecordingWorkControl {
        charges: Vec::new(),
        reject_after: Some(1),
    };

    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut control,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert_eq!(control.charges, vec![54]);
}

#[test]
fn randomized_rerun_reuses_one_spectral_topology() {
    let graph = IndexedGraph {
        nodes: (0..5)
            .map(|index| IndexedNode {
                parent: match index {
                    0 | 1 => Some(1),
                    2 => Some(0),
                    _ => None,
                },
                width: 40.0,
                height: 40.0,
                x: index as f64 * 50.0,
                y: (index % 2) as f64 * 30.0,
                bounds_extras: BoundsExtras::default(),
            })
            .collect(),
        edges: [(0, 1), (0, 2), (1, 3), (2, 4), (3, 4)]
            .into_iter()
            .map(|(source, target)| IndexedEdge {
                source,
                target,
                label_width: None,
                label_height: None,
                source_anchor: None,
                target_anchor: None,
                curve_style_segments: false,
                ideal_length: 50.0,
                elasticity: 0.45,
            })
            .collect(),
        compounds: vec![
            IndexedCompound { parent: None },
            IndexedCompound { parent: Some(0) },
        ],
    };
    let options = IndexedFcoseOptions {
        randomize: true,
        rerun: true,
        num_iter: Some(1),
        ..IndexedFcoseOptions::default()
    };

    super::spectral::reset_topology_build_count();
    let first = layout_indexed(&graph, &options).expect("randomized rerun layout");

    assert_eq!(super::spectral::topology_build_count(), 1);
    assert_eq!(first.node_positions.len(), graph.nodes.len());
    assert!(
        first
            .node_positions
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
    );

    super::spectral::reset_topology_build_count();
    let replay = layout_indexed(&graph, &options).expect("deterministic rerun replay");
    assert_eq!(super::spectral::topology_build_count(), 1);
    assert_eq!(first.node_positions.len(), replay.node_positions.len());
    for (left, right) in first.node_positions.iter().zip(&replay.node_positions) {
        assert_eq!(left.x.to_bits(), right.x.to_bits());
        assert_eq!(left.y.to_bits(), right.y.to_bits());
    }
}

#[test]
fn indexed_layout_rejects_compound_parent_cycles_before_work() {
    let cases = [
        vec![IndexedCompound { parent: Some(0) }],
        vec![
            IndexedCompound { parent: Some(1) },
            IndexedCompound { parent: Some(0) },
        ],
    ];

    for compounds in cases {
        let graph = IndexedGraph {
            nodes: vec![IndexedNode {
                parent: Some(0),
                width: 40.0,
                height: 40.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            }],
            edges: Vec::new(),
            compounds,
        };
        let mut control = RecordingWorkControl::default();

        let error = layout_indexed_with_random_policy_and_work_control(
            &graph,
            &IndexedFcoseOptions {
                randomize: true,
                num_iter: Some(1),
                ..IndexedFcoseOptions::default()
            },
            FcoseRandomPolicy::xorshift(1),
            &mut control,
        )
        .expect_err("compound parent cycle must fail before layout work");

        assert!(matches!(
            error,
            Error::MissingEndpoint { edge_id }
                if edge_id.starts_with("compound-parent-cycle:#")
        ));
        assert!(control.charges.is_empty());
    }
}

#[test]
fn compound_topology_matches_layout_base_edge_projection_and_connectivity() {
    fn reference_projection(sim: &SimGraph, source: usize, target: usize) -> super::EdgeProjection {
        let root = sim.root_owner_idx;
        let mut seen = vec![false; sim.nodes.len() + 1];
        let mut owner = sim.nodes[source].owner_idx;
        loop {
            seen[owner] = true;
            if owner == root {
                break;
            }
            owner = sim.nodes[owner].owner_idx;
        }

        let mut lca_owner_idx = sim.nodes[target].owner_idx;
        while !seen[lca_owner_idx] {
            lca_owner_idx = sim.nodes[lca_owner_idx].owner_idx;
        }

        let child_below = |mut node_idx: usize| {
            while sim.nodes[node_idx].owner_idx != lca_owner_idx {
                node_idx = sim.nodes[node_idx].owner_idx;
            }
            node_idx
        };
        super::EdgeProjection {
            lca_owner_idx,
            source_in_lca: child_below(source),
            target_in_lca: child_below(target),
        }
    }

    fn reference_connected_owners(sim: &SimGraph) -> Vec<bool> {
        fn map_to_owner(sim: &SimGraph, mut node_idx: usize, owner: usize) -> Option<usize> {
            loop {
                if sim.nodes[node_idx].owner_idx == owner {
                    return Some(node_idx);
                }
                let parent = sim.nodes[node_idx].owner_idx;
                if parent == sim.root_owner_idx {
                    return None;
                }
                node_idx = parent;
            }
        }

        fn push_with_children(
            sim: &SimGraph,
            start: usize,
            visited: &mut [bool],
            queue: &mut std::collections::VecDeque<usize>,
        ) {
            let mut stack = vec![start];
            while let Some(node_idx) = stack.pop() {
                if std::mem::replace(&mut visited[node_idx], true) {
                    continue;
                }
                queue.push_back(node_idx);
                if let Some(children) = sim.children_by_owner.get(node_idx) {
                    stack.extend(children.iter().copied());
                }
            }
        }

        let mut edges_by_node = vec![Vec::new(); sim.nodes.len()];
        for (edge_idx, edge) in sim.edges.iter().enumerate() {
            edges_by_node[edge.a].push(edge_idx);
            edges_by_node[edge.b].push(edge_idx);
        }

        let mut connected = vec![true; sim.nodes.len() + 1];
        for (owner, children) in sim.children_by_owner.iter().enumerate() {
            let Some(&first) = children.first() else {
                continue;
            };
            let mut visited = vec![false; sim.nodes.len()];
            let mut queue = std::collections::VecDeque::new();
            push_with_children(sim, first, &mut visited, &mut queue);
            while let Some(node_idx) = queue.pop_front() {
                for &edge_idx in &edges_by_node[node_idx] {
                    let edge = &sim.edges[edge_idx];
                    let other = if edge.a == node_idx { edge.b } else { edge.a };
                    let Some(mapped) = map_to_owner(sim, other, owner) else {
                        continue;
                    };
                    if !visited[mapped] {
                        push_with_children(sim, mapped, &mut visited, &mut queue);
                    }
                }
            }
            connected[owner] = children.iter().all(|&node_idx| visited[node_idx]);
        }
        connected
    }

    let edge = |source, target| IndexedEdge {
        source,
        target,
        label_width: None,
        label_height: None,
        source_anchor: None,
        target_anchor: None,
        curve_style_segments: false,
        ideal_length: 50.0,
        elasticity: 0.45,
    };
    let graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: Some(1),
                width: 40.0,
                height: 40.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: Some(1),
                width: 40.0,
                height: 40.0,
                x: 50.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: Some(0),
                width: 40.0,
                height: 40.0,
                x: 100.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: Some(2),
                width: 40.0,
                height: 40.0,
                x: 150.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: 200.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: 250.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: vec![edge(0, 1), edge(0, 2), edge(1, 3), edge(2, 4), edge(3, 5)],
        compounds: vec![
            IndexedCompound { parent: None },
            IndexedCompound { parent: Some(0) },
            IndexedCompound { parent: None },
        ],
    };

    for edge_count in [graph.edges.len(), graph.edges.len() - 1] {
        let mut case = graph.clone();
        case.edges.truncate(edge_count);
        let sim = SimGraph::from_indexed(&case);
        let expected_projections = sim
            .edges
            .iter()
            .map(|edge| reference_projection(&sim, edge.a, edge.b))
            .collect::<Vec<_>>();

        assert_eq!(sim.compound_topology.edge_projections, expected_projections);
        assert_eq!(
            sim.compound_topology.owner_connected,
            reference_connected_owners(&sim)
        );
    }
}

#[test]
fn compound_topology_is_preflighted_once_across_reruns() {
    let one_run_schedule =
        FcoseIterationSchedule::from_normalized_counts(5, 64, 96, false).expect("one-run schedule");
    let rerun_schedule =
        FcoseIterationSchedule::from_normalized_counts(5, 64, 96, true).expect("rerun schedule");
    let topology_work = CompoundTopology::work_units(64, 96).expect("topology work");
    let one_run = FcoseWorkPlan::from_schedule(
        one_run_schedule,
        64,
        FcoseConstraintWorkShape::default(),
        0,
        false,
    )
    .expect("one-run work plan");
    let rerun = FcoseWorkPlan::from_schedule(
        rerun_schedule,
        64,
        FcoseConstraintWorkShape::default(),
        0,
        false,
    )
    .expect("rerun work plan");

    assert_eq!(one_run_schedule.setup_work_units(), 64 + 96 + topology_work);
    assert_eq!(
        one_run.setup_work_units(),
        one_run_schedule.setup_work_units()
    );
    assert_eq!(rerun.setup_work_units(), one_run.setup_work_units());
}

#[test]
fn compound_topology_dsu_bound_covers_balanced_parent_chains() {
    const NODE_COUNT: usize = 16;
    const EDGE_COUNT: usize = NODE_COUNT - 1;

    let mut dsu = DisjointSet::new(NODE_COUNT);
    let mut subtree_size = 1usize;
    while subtree_size < NODE_COUNT {
        for first in (0..NODE_COUNT).step_by(subtree_size * 2) {
            dsu.union(first, first + subtree_size);
        }
        subtree_size *= 2;
    }

    let deepest_parent_chain = dsu.parent_depth(NODE_COUNT - 1);
    let per_find_bound = CompoundTopology::lifting_level_count(NODE_COUNT);
    let find_call_count = EDGE_COUNT * 2 + NODE_COUNT;

    assert_eq!(deepest_parent_chain, 4);
    assert!(deepest_parent_chain < per_find_bound);
    assert_eq!(
        CompoundTopology::dsu_find_work_units(NODE_COUNT, EDGE_COUNT).unwrap(),
        find_call_count * per_find_bound
    );
}

#[test]
fn compound_topology_preflight_accepts_equal_budget_and_rejects_below() {
    let graph = IndexedGraph {
        nodes: vec![IndexedNode {
            parent: Some(0),
            width: 40.0,
            height: 40.0,
            x: 0.0,
            y: 0.0,
            bounds_extras: BoundsExtras::default(),
        }],
        edges: Vec::new(),
        compounds: vec![IndexedCompound { parent: None }],
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(1),
        ..IndexedFcoseOptions::default()
    };
    let node_count = graph.nodes.len() + graph.compounds.len();
    let schedule =
        FcoseIterationSchedule::from_indexed_graph(options.num_iter, &graph, options.rerun)
            .expect("schedule");
    let plan = FcoseWorkPlan::from_schedule(
        schedule,
        node_count,
        FcoseConstraintWorkShape::default(),
        0,
        false,
    )
    .expect("work plan");

    let mut below = CheckOnlyBudget {
        max_units: plan.maximum_work_units() - 1,
        ..CheckOnlyBudget::default()
    };
    let error = layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut below,
    )
    .expect_err("below-bound budget must fail the predictable preflight");
    assert!(matches!(
        error,
        Error::WorkFailure(WorkFailure::Interrupted)
    ));
    assert!(below.charges.is_empty());

    let mut equal = CheckOnlyBudget {
        max_units: plan.maximum_work_units(),
        ..CheckOnlyBudget::default()
    };
    layout_indexed_with_random_policy_and_work_control(
        &graph,
        &options,
        FcoseRandomPolicy::xorshift(1),
        &mut equal,
    )
    .expect("equal predictable budget should pass preflight");
    assert_eq!(equal.checks[2], plan.maximum_work_units());
    assert_eq!(
        equal.charges.first().copied(),
        Some(plan.setup_work_units())
    );
}

#[test]
fn owner_local_fallback_pairs_charge_dense_work_and_preserve_order() {
    let children_by_owner = vec![vec![100, 101, 102], (0..64).collect::<Vec<_>>()];
    let pair_work = owner_local_pair_work(&children_by_owner).expect("pair work");
    assert_eq!(pair_work, 3 + (64 * 63 / 2));

    let mut below = CheckOnlyBudget {
        max_units: pair_work - 1,
        ..CheckOnlyBudget::default()
    };
    assert_eq!(
        admit_dynamic_work(&mut below, pair_work),
        Err(WorkFailure::Interrupted)
    );
    assert!(below.charges.is_empty());

    let mut equal = CheckOnlyBudget {
        max_units: pair_work,
        ..CheckOnlyBudget::default()
    };
    admit_dynamic_work(&mut equal, pair_work).expect("equal dense-pair budget");
    assert_eq!(equal.charges, vec![pair_work]);

    let mut pairs = Vec::with_capacity(pair_work);
    for_each_owner_local_pair(&children_by_owner, |first, second| {
        pairs.push((first, second));
    });
    assert_eq!(pairs.len(), pair_work);
    assert_eq!(&pairs[..3], &[(100, 101), (100, 102), (101, 102)]);
    assert_eq!(pairs[3], (0, 1));
    assert_eq!(pairs.last().copied(), Some((62, 63)));
}

#[test]
fn local_matrix_math_preserves_procrustes_orientation() {
    let mut covariance = super::Mat2::default();
    covariance.add_outer_product(Vec2::new(2.0, 3.0), Vec2::new(5.0, 7.0));
    assert_eq!(covariance, super::Mat2::new(10.0, 14.0, 15.0, 21.0));

    let transformed = super::Mat2::new(1.0, 2.0, 3.0, 4.0)
        .transpose()
        .transform(Vec2::new(5.0, 7.0));
    assert_eq!(transformed, Vec2::new(26.0, 38.0));
}

#[test]
fn sim_graph_handles_deep_compound_chain_with_small_stack() {
    const DEPTH: usize = 2048;
    let handle = std::thread::Builder::new()
        .name("manatee-fcose-deep-compound-chain".to_string())
        .stack_size(64 * 1024)
        .spawn(|| {
            let nodes = vec![IndexedNode {
                parent: Some(DEPTH - 1),
                width: 80.0,
                height: 80.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            }];
            let compounds = (0..DEPTH)
                .map(|idx| IndexedCompound {
                    parent: (idx > 0).then(|| idx - 1),
                })
                .collect::<Vec<_>>();
            let graph = IndexedGraph {
                nodes,
                edges: Vec::new(),
                compounds,
            };

            let sim = SimGraph::from_indexed(&graph);
            assert_eq!(sim.compounds_deep_first.len(), DEPTH);
            assert_eq!(sim.inclusion_depth[0], DEPTH + 1);

            let order = sim.all_nodes_layout_order();
            assert_eq!(order.len(), DEPTH + 1);
            assert_eq!(order.first().copied(), Some(1));
            assert_eq!(order.last().copied(), Some(0));
        })
        .expect("spawn manatee deep compound test");
    handle
        .join()
        .expect("deep compound SimGraph construction should not overflow");
}

#[test]
fn compound_topology_projects_deep_edges_with_subquadratic_work_curve() {
    const DEPTH: usize = 256;
    let graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: Some(DEPTH - 1),
                width: 40.0,
                height: 40.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: 50.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: vec![IndexedEdge {
            source: 0,
            target: 1,
            label_width: None,
            label_height: None,
            source_anchor: None,
            target_anchor: None,
            curve_style_segments: false,
            ideal_length: 50.0,
            elasticity: 0.45,
        }],
        compounds: (0..DEPTH)
            .map(|idx| IndexedCompound {
                parent: (idx > 0).then(|| idx - 1),
            })
            .collect(),
    };
    let sim = SimGraph::from_indexed(&graph);
    let projection = sim.compound_topology.edge_projections[0];

    assert_eq!(projection.lca_owner_idx, sim.root_owner_idx);
    assert_eq!(projection.source_in_lca, graph.nodes.len());
    assert_eq!(projection.target_in_lca, 1);

    let curve = [8usize, 16, 32, 64, 128, 256, 512, 1_024].map(|depth| {
        CompoundTopology::work_units_with_compound_count(depth + graph.nodes.len(), 1, depth)
            .expect("deep topology work")
    });
    for window in curve.windows(2) {
        let [half, full] = window else {
            unreachable!("two-point curve window")
        };
        assert!(full > half);
        assert!(
            *full < *half * 3,
            "binary-lifting setup should remain subquadratic: half={half}, full={full}"
        );
    }
}

#[test]
fn compound_child_weights_count_empty_compounds_like_layout_base() {
    let graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: Some(0),
                width: 10.0,
                height: 10.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: Some(2),
                width: 10.0,
                height: 10.0,
                x: 20.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: Vec::new(),
        compounds: vec![
            IndexedCompound { parent: None },
            IndexedCompound { parent: Some(0) },
            IndexedCompound { parent: Some(0) },
        ],
    };

    let sim = SimGraph::from_indexed(&graph);
    let root = graph.nodes.len();
    let empty = root + 1;
    let non_empty = root + 2;

    assert_eq!(sim.nodes[root].no_of_children, 3.0);
    assert_eq!(sim.nodes[empty].no_of_children, 1.0);
    assert_eq!(sim.nodes[non_empty].no_of_children, 1.0);
}

#[test]
fn linear_compound_displacement_matches_recursive_upstream_propagation() {
    fn propagate_to_leaf_descendants(
        sim: &SimGraph,
        owner: usize,
        dx: f64,
        dy: f64,
        displacements: &mut [(f64, f64)],
    ) {
        for &child in sim
            .children_by_owner
            .get(owner)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let Some(node) = sim.nodes.get(child) else {
                continue;
            };
            let is_non_empty_compound = node.is_compound
                && sim
                    .children_by_owner
                    .get(child)
                    .is_some_and(|children| !children.is_empty());
            if is_non_empty_compound {
                propagate_to_leaf_descendants(sim, child, dx, dy, displacements);
            } else if let Some(slot) = displacements.get_mut(child) {
                slot.0 += dx;
                slot.1 += dy;
            }
        }
    }

    fn recursive_upstream_displacements(
        sim: &SimGraph,
        order: &[usize],
        cooling_factor: f64,
        max_displacement: f64,
    ) -> Vec<(f64, f64)> {
        let mut displacements = vec![(0.0, 0.0); sim.nodes.len()];
        for &idx in order {
            let Some(node) = sim.nodes.get(idx) else {
                continue;
            };
            let denominator = node.no_of_children.max(1.0);
            let slot = &mut displacements[idx];
            slot.0 += cooling_factor * (node.spring_fx + node.repulsion_fx + node.gravitation_fx)
                / denominator;
            slot.1 += cooling_factor * (node.spring_fy + node.repulsion_fy + node.gravitation_fy)
                / denominator;
            if slot.0.abs() > max_displacement {
                slot.0 = max_displacement * super::imath_sign(slot.0);
            }
            if slot.1.abs() > max_displacement {
                slot.1 = max_displacement * super::imath_sign(slot.1);
            }

            if node.is_compound
                && sim
                    .children_by_owner
                    .get(idx)
                    .is_some_and(|children| !children.is_empty())
            {
                let (dx, dy) = *slot;
                propagate_to_leaf_descendants(sim, idx, dx, dy, &mut displacements);
            }
        }
        displacements
    }

    let graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: Some(1),
                width: 10.0,
                height: 10.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: Some(0),
                width: 10.0,
                height: 10.0,
                x: 20.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: None,
                width: 10.0,
                height: 10.0,
                x: 40.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: Vec::new(),
        compounds: vec![
            IndexedCompound { parent: None },
            IndexedCompound { parent: Some(0) },
            IndexedCompound { parent: Some(0) },
        ],
    };
    let mut sim = SimGraph::from_indexed(&graph);
    for (idx, node) in sim.nodes.iter_mut().enumerate() {
        let scale = idx as f64 + 1.0;
        node.spring_fx = 1.25 * scale;
        node.spring_fy = -0.75 * scale;
        node.repulsion_fx = 0.5 * scale;
        node.repulsion_fy = 0.25 * scale;
        node.gravitation_fx = -0.125 * scale;
        node.gravitation_fy = 0.375 * scale;
    }

    let order = sim.all_nodes_layout_order();
    let mut wide_actual = Vec::new();
    for max_displacement in [100.0, 0.75] {
        let expected = recursive_upstream_displacements(&sim, &order, 0.5, max_displacement);
        let mut actual = vec![(0.0, 0.0); sim.nodes.len()];
        let mut propagated = vec![(0.0, 0.0); sim.nodes.len() + 1];
        sim.calculate_displacements(&order, 0.5, max_displacement, &mut actual, &mut propagated);

        assert_eq!(
            actual
                .iter()
                .map(|(x, y)| (x.to_bits(), y.to_bits()))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(x, y)| (x.to_bits(), y.to_bits()))
                .collect::<Vec<_>>()
        );
        if max_displacement == 100.0 {
            wide_actual = actual;
        }
    }

    let root_compound = graph.nodes.len();
    let nested_compound = root_compound + 1;
    let empty_compound = root_compound + 2;
    assert_ne!(wide_actual[nested_compound], (0.0, 0.0));
    assert_ne!(wide_actual[empty_compound], (0.0, 0.0));
    assert_ne!(wide_actual[0], wide_actual[nested_compound]);

    let empty = &sim.nodes[empty_compound];
    let empty_own_dx =
        0.5 * (empty.spring_fx + empty.repulsion_fx + empty.gravitation_fx) / empty.no_of_children;
    assert_eq!(
        wide_actual[empty_compound].0.to_bits(),
        (wide_actual[root_compound].0 + empty_own_dx).to_bits()
    );
}

#[test]
fn indexed_layout_matches_string_graph_layout_for_compound_constraints() {
    let graph = Graph {
        nodes: vec![
            Node {
                id: "a".to_string(),
                parent: Some("group".to_string()),
                width: 80.0,
                height: 80.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            Node {
                id: "b".to_string(),
                parent: Some("group".to_string()),
                width: 80.0,
                height: 80.0,
                x: 120.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            Node {
                id: "c".to_string(),
                parent: None,
                width: 80.0,
                height: 80.0,
                x: 240.0,
                y: 120.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: vec![
            Edge {
                id: "ab".to_string(),
                source: "a".to_string(),
                target: "b".to_string(),
                label_width: Some(32.0),
                label_height: Some(16.0),
                source_anchor: Some(Anchor::Right),
                target_anchor: Some(Anchor::Left),
                ideal_length: 80.0,
                elasticity: 0.45,
            },
            Edge {
                id: "bc".to_string(),
                source: "b".to_string(),
                target: "c".to_string(),
                label_width: None,
                label_height: None,
                source_anchor: Some(Anchor::Bottom),
                target_anchor: Some(Anchor::Top),
                ideal_length: 80.0,
                elasticity: 0.001,
            },
        ],
        compounds: vec![Compound {
            id: "group".to_string(),
            parent: None,
        }],
    };

    let opts = FcoseOptions {
        random_seed: 1,
        random_seed_offset: None,
        rerun: false,
        randomize: true,
        node_separation: None,
        num_iter: None,
        default_edge_length: Some(80.0),
        alignment_constraint: Some(AlignmentConstraint {
            horizontal: vec![vec!["a".to_string(), "b".to_string()]],
            vertical: vec![vec!["b".to_string(), "c".to_string()]],
        }),
        relative_placement_constraint: vec![RelativePlacementConstraint {
            left: Some("a".to_string()),
            right: Some("c".to_string()),
            top: None,
            bottom: None,
            gap: 140.0,
        }],
        compound_padding: Some(12.0),
        relocate_center: None,
    };

    let compat = layout(&graph, &opts).expect("compat layout");

    let indexed_graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: Some(0),
                width: 80.0,
                height: 80.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: Some(0),
                width: 80.0,
                height: 80.0,
                x: 120.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: None,
                width: 80.0,
                height: 80.0,
                x: 240.0,
                y: 120.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: vec![
            IndexedEdge {
                source: 0,
                target: 1,
                label_width: Some(32.0),
                label_height: Some(16.0),
                source_anchor: Some(Anchor::Right),
                target_anchor: Some(Anchor::Left),
                curve_style_segments: false,
                ideal_length: 80.0,
                elasticity: 0.45,
            },
            IndexedEdge {
                source: 1,
                target: 2,
                label_width: None,
                label_height: None,
                source_anchor: Some(Anchor::Bottom),
                target_anchor: Some(Anchor::Top),
                curve_style_segments: true,
                ideal_length: 80.0,
                elasticity: 0.001,
            },
        ],
        compounds: vec![IndexedCompound { parent: None }],
    };
    let indexed_opts = IndexedFcoseOptions {
        random_seed: 1,
        random_seed_offset: None,
        rerun: false,
        randomize: true,
        node_separation: None,
        num_iter: None,
        default_edge_length: Some(80.0),
        alignment_constraint: Some(IndexedAlignmentConstraint {
            horizontal: vec![vec![0, 1]],
            vertical: vec![vec![1, 2]],
        }),
        relative_placement_constraint: vec![IndexedRelativePlacementConstraint {
            left: Some(0),
            right: Some(2),
            top: None,
            bottom: None,
            gap: 140.0,
        }],
        compound_padding: Some(12.0),
        relocate_center: None,
    };
    let indexed = layout_indexed(&indexed_graph, &indexed_opts).expect("indexed layout");

    assert_eq!(indexed.node_positions.len(), graph.nodes.len());
    assert_eq!(indexed.compound_positions.len(), graph.compounds.len());
    assert_eq!(indexed.compound_bounds.len(), graph.compounds.len());
    assert_point_close(indexed.node_positions[0], compat.positions["a"]);
    assert_point_close(indexed.node_positions[1], compat.positions["b"]);
    assert_point_close(indexed.node_positions[2], compat.positions["c"]);
    assert_point_close(indexed.compound_positions[0], compat.positions["group"]);
    let group_bounds = indexed.compound_bounds[0];
    assert!(
        group_bounds.width > 80.0 && group_bounds.height > 80.0,
        "expected compound bounds to include child graph padding, got {group_bounds:?}"
    );
    assert_point_close(
        indexed.compound_positions[0],
        Point {
            x: group_bounds.left + group_bounds.width / 2.0,
            y: group_bounds.top + group_bounds.height / 2.0,
        },
    );
}

#[test]
fn indexed_layout_rejects_positive_relative_constraint_cycles() {
    let graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: 100.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: Vec::new(),
        compounds: Vec::new(),
    };
    let options = IndexedFcoseOptions {
        randomize: false,
        num_iter: Some(1),
        alignment_constraint: Some(IndexedAlignmentConstraint {
            horizontal: vec![vec![0, 1]],
            vertical: Vec::new(),
        }),
        relative_placement_constraint: vec![
            IndexedRelativePlacementConstraint {
                left: Some(0),
                right: Some(1),
                top: None,
                bottom: None,
                gap: 40.0,
            },
            IndexedRelativePlacementConstraint {
                left: Some(1),
                right: Some(0),
                top: None,
                bottom: None,
                gap: 40.0,
            },
        ],
        ..IndexedFcoseOptions::default()
    };

    let error = layout_indexed(&graph, &options).expect_err("constraint cycle must fail");
    assert!(matches!(
        error,
        crate::error::Error::InfeasibleConstraints { axis: "horizontal" }
    ));
}

#[test]
fn eles_bbox_keeps_edge_label_stages_across_both_relocation_runs() {
    let graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: BoundsExtras::default(),
            },
            IndexedNode {
                parent: None,
                width: 40.0,
                height: 40.0,
                x: 100.0,
                y: 100.0,
                bounds_extras: BoundsExtras::default(),
            },
        ],
        edges: vec![IndexedEdge {
            source: 0,
            target: 1,
            label_width: Some(200.0),
            label_height: Some(20.0),
            source_anchor: Some(Anchor::Right),
            target_anchor: Some(Anchor::Left),
            curve_style_segments: false,
            ideal_length: 80.0,
            elasticity: 0.45,
        }],
        compounds: Vec::new(),
    };

    let sim = SimGraph::from_indexed(&graph);
    let (straight_center_x, _) = sim
        .bounding_box_center_eles(1)
        .expect("straight bbox center");
    assert!(
        (straight_center_x - 70.0).abs() < 1e-9,
        "straight edge label should stay centered on the straight midpoint, got {straight_center_x}"
    );

    let mut segments_graph = graph;
    segments_graph.edges[0].curve_style_segments = true;
    for (label_width, expected_center_x) in [(None, 50.0), (Some(20.0), 50.0), (Some(200.0), 91.5)]
    {
        let mut case = segments_graph.clone();
        case.edges[0].label_width = label_width;
        case.edges[0].label_height = label_width.map(|_| 20.0);
        let sim = SimGraph::from_indexed(&case);
        let (center_x, _) = sim
            .bounding_box_center_eles(1)
            .expect("segments bbox center");
        assert!(
            (center_x - expected_center_x).abs() < 1e-9,
            "unexpected second-run center for label width {label_width:?}: {center_x}"
        );
    }

    let mut sim = SimGraph::from_indexed(&segments_graph);
    for (run_idx, expected_center_x) in [(0, 39.57125353714373), (1, 81.07125353714373)] {
        let orig_center = sim
            .bounding_box_center_eles(run_idx)
            .expect("relocation origin");
        assert!((orig_center.0 - expected_center_x).abs() < 1e-9);
        let current_center = sim
            .bounding_box_center_rects()
            .expect("current rect center");
        sim.translate(
            orig_center.0 - current_center.0,
            orig_center.1 - current_center.1,
        );
        assert_eq!(sim.bounding_box_center_rects(), Some(orig_center));
    }
}

#[test]
fn cytoscape_relocation_bbox_keeps_body_parent_and_label_outsets_separate() {
    assert_eq!(super::CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX, 1.5);
    assert_eq!(super::CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX, 2.0);
    assert_eq!(super::CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX, 1.0);
    assert_eq!(super::CYTOSCAPE_PARENT_BODY_NON_PADDING_BBOX_OUTSET_PX, 1.5);

    let compound_child_extras = BoundsExtras {
        left: 1.0,
        right: 1.0,
        top: 1.0,
        bottom: 18.0,
    };
    let final_element_extras = BoundsExtras {
        bottom: 19.0,
        ..compound_child_extras
    };
    let graph = IndexedGraph {
        nodes: vec![
            IndexedNode {
                parent: Some(0),
                width: 80.0,
                height: 80.0,
                x: 0.0,
                y: 0.0,
                bounds_extras: compound_child_extras,
            },
            IndexedNode {
                parent: None,
                width: 80.0,
                height: 80.0,
                x: 200.0,
                y: 0.0,
                bounds_extras: final_element_extras,
            },
        ],
        edges: Vec::new(),
        compounds: vec![IndexedCompound { parent: None }],
    };

    let mut sim = SimGraph::from_indexed(&graph);
    let compound_idx = graph.nodes.len();
    sim.nodes[compound_idx].padding = 10.0;
    let center = sim
        .bounding_box_center_eles(1)
        .expect("compound relocation bbox center");

    assert_eq!(center, (94.25, 8.5));
}

#[test]
fn xorshift64star_next_f64_unit_matches_seeded_upstream_baseline() {
    // Mirrors the JS prelude in `xtask` used to generate deterministic upstream SVGs:
    //
    // - xorshift64* (same shift/multiply constants)
    // - `Math.random = () => Number(nextU64() >> 11n) / 2^53`
    let mut rng = XorShift64Star::new(1);
    let expected = [
        0.28083505005035947,
        0.6711372530266764,
        0.7258461452833668,
        0.303529299965799,
        0.056176763098259475,
    ];
    for (i, &e) in expected.iter().enumerate() {
        let v = rng.next_f64_unit();
        assert!(
            (v - e).abs() < 1e-15,
            "unexpected rng value at {i}: got {v}, expected {e}"
        );
    }
}

#[test]
fn xorshift64star_next_usize_matches_js_floor_random_times_upper() {
    // For seed=1, the first `Math.random()` value is ~0.2808 so `floor(r * 3) == 0`.
    // Using `% 3` on the underlying u64 yields `1`, which would diverge from the upstream
    // spectral sampling path for small graphs.
    let mut rng = FcoseRandom::XorShift64Star(XorShift64Star::new(1));
    assert_eq!(rng.next_usize(3), 0);
}

#[test]
fn mulberry32_next_f64_unit_matches_mermaid_11_16_architecture_seed() {
    let mut rng = Mulberry32::new(1);
    let expected = [
        0.6270739405881613,
        0.002735721180215478,
        0.5274470399599522,
        0.9810509674716741,
        0.9683778982143849,
    ];
    for (i, &expected) in expected.iter().enumerate() {
        let actual = rng.next_f64_unit();
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "unexpected Mermaid 11.16 mulberry32 value at {i}: got {actual}, expected {expected}"
        );
    }
}

#[test]
fn deterministic_policy_restarts_the_second_run_from_the_same_seed_and_offset() {
    let policy = FcoseRandomPolicy::seeded(FcoseRandomSource::Mulberry32, 42)
        .with_seed_offset(2)
        .with_reset_seed_each_run(true);
    let mut rng = new_fcose_random(policy);
    super::advance_random(&mut rng, policy.seed_offset().unwrap_or_default());
    let first_run = [rng.next_f64_unit(), rng.next_f64_unit()];

    let mut work_control = super::NoopWorkControl;
    assert!(
        reset_fcose_random_for_run(
            &mut rng,
            policy,
            1,
            policy.seed_offset().unwrap_or_default(),
            &mut work_control,
        )
        .unwrap()
    );
    let second_run = [rng.next_f64_unit(), rng.next_f64_unit()];

    assert_eq!(second_run, first_run);
}

#[test]
fn rejected_seed_offset_charge_does_not_mutate_the_random_stream() {
    let policy = FcoseRandomPolicy::seeded(FcoseRandomSource::Mulberry32, 42)
        .with_seed_offset(3)
        .with_reset_seed_each_run(true);
    let mut rng = new_fcose_random(policy);
    let mut expected = rng.clone();
    let mut control = RecordingWorkControl {
        charges: Vec::new(),
        reject_after: Some(0),
    };

    assert_eq!(
        reset_fcose_random_for_run(&mut rng, policy, 1, 3, &mut control),
        Err(WorkFailure::Interrupted),
    );
    assert_eq!(rng.next_f64_unit(), expected.next_f64_unit());
    assert!(control.charges.is_empty());
}

#[test]
fn repulsion_grid_surrounding_excludes_processed_nodes() {
    // Build a tiny 1D-ish layout:
    //
    // - node0 and node1 are exactly within range
    // - node2 is far outside range
    let repulsion_range = 10.0;
    let mut nodes = vec![
        node_at(0.0, 0.0, 10.0, 10.0),
        node_at(20.0, 0.0, 10.0, 10.0),
        node_at(200.0, 0.0, 10.0, 10.0),
    ];
    let mut left = f64::INFINITY;
    let mut top = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for n in &nodes {
        left = left.min(n.left);
        top = top.min(n.top);
        right = right.max(n.left + n.width);
        bottom = bottom.max(n.top + n.height);
    }
    let node_order = [0usize, 1, 2];
    let mut work_control = NoopWorkControl;
    let mut grid = RepulsionGrid::build_or_reuse(
        None,
        left,
        top,
        right,
        bottom,
        &mut nodes,
        repulsion_range,
        &node_order,
        &mut work_control,
    )
    .unwrap()
    .expect("grid");

    let mut processed_generation = vec![0u32; nodes.len()];
    let current_processed_generation = 1u32;
    let mut surrounding_seen = vec![0u32; nodes.len()];
    let mut surrounding_seen_generation = 1u32;
    grid.refresh_node_surrounding(
        0,
        &mut nodes,
        &processed_generation,
        current_processed_generation,
        repulsion_range,
        &mut surrounding_seen,
        &mut surrounding_seen_generation,
        &mut work_control,
    )
    .unwrap();
    assert_eq!(nodes[0].surrounding, vec![1]);

    processed_generation[0] = current_processed_generation;
    grid.refresh_node_surrounding(
        1,
        &mut nodes,
        &processed_generation,
        current_processed_generation,
        repulsion_range,
        &mut surrounding_seen,
        &mut surrounding_seen_generation,
        &mut work_control,
    )
    .unwrap();
    assert!(
        !nodes[1].surrounding.contains(&0),
        "node1 should not include already-processed node0"
    );
}

#[test]
fn repulsion_grid_plan_prefers_sparse_storage_for_large_coordinate_span() {
    let repulsion_range = 100.0;
    let mut nodes = vec![
        node_at(0.0, 0.0, 10.0, 10.0),
        node_at(100_000_000_000.0, 100_000_000_000.0, 10.0, 10.0),
    ];
    let node_order = [0usize, 1];

    let plan = RepulsionGridPlan::from_geometry(
        0.0,
        0.0,
        100_000_000_010.0,
        100_000_000_010.0,
        &nodes,
        repulsion_range,
        &node_order,
    )
    .unwrap()
    .expect("grid plan");

    assert_eq!(plan.storage_kind(), RepulsionGridStorageKind::Sparse);
    assert_eq!(plan.cell_reference_count(), 2);
    assert_eq!(plan.total_cell_count(), 1_000_000_002_000_000_001);

    let mut control = RecordingWorkControl::default();
    let grid = RepulsionGrid::build_or_reuse(
        None,
        0.0,
        0.0,
        100_000_000_010.0,
        100_000_000_010.0,
        &mut nodes,
        repulsion_range,
        &node_order,
        &mut control,
    )
    .unwrap()
    .expect("sparse grid");
    assert!(matches!(
        grid.cells,
        super::RepulsionGridCells::Implicit { .. }
    ));
    assert_eq!(control.charges, vec![4, 6]);
}

#[test]
fn repulsion_grid_plan_prefers_dense_storage_when_costs_are_equal() {
    let nodes = vec![node_at(0.0, 0.0, 1.0, 1.0)];
    let plan = RepulsionGridPlan::from_geometry(0.0, 0.0, 1.0, 1.0, &nodes, 10.0, &[0])
        .unwrap()
        .expect("single-cell plan");

    assert_eq!(plan.storage_kind(), RepulsionGridStorageKind::Dense);
    assert_eq!(plan.work_units(), 2);
}

#[test]
fn repulsion_grid_plan_rejects_overflowing_finite_coordinate_span() {
    let nodes = vec![node_at(0.0, 0.0, 1.0, 1.0)];
    let error = RepulsionGridPlan::from_geometry(-f64::MAX, 0.0, f64::MAX, 1.0, &nodes, 10.0, &[0])
        .unwrap_err();

    assert_eq!(error, WorkFailure::ArithmeticOverflow);
}

#[test]
fn repulsion_grid_plan_prefers_implicit_storage_for_a_huge_node_rect() {
    let repulsion_range = 100.0;
    let mut nodes = vec![node_at(0.0, 0.0, 100_000_000_000.0, 100_000_000_000.0)];
    let node_order = [0usize];

    let plan = RepulsionGridPlan::from_geometry(
        0.0,
        0.0,
        100_000_000_000.0,
        100_000_000_000.0,
        &nodes,
        repulsion_range,
        &node_order,
    )
    .unwrap()
    .expect("grid plan");

    assert_eq!(plan.storage_kind(), RepulsionGridStorageKind::Implicit);
    assert_eq!(plan.work_units(), 2);

    let mut control = RecordingWorkControl::default();
    let mut grid = RepulsionGrid::build_or_reuse(
        None,
        0.0,
        0.0,
        100_000_000_000.0,
        100_000_000_000.0,
        &mut nodes,
        repulsion_range,
        &node_order,
        &mut control,
    )
    .unwrap()
    .expect("implicit grid");
    assert!(matches!(
        grid.cells,
        super::RepulsionGridCells::Implicit { .. }
    ));
    assert_eq!(control.charges, vec![2]);

    let mut surrounding_seen = [0u32];
    let mut surrounding_seen_generation = 1u32;
    grid.refresh_node_surrounding(
        0,
        &mut nodes,
        &[0],
        1,
        repulsion_range,
        &mut surrounding_seen,
        &mut surrounding_seen_generation,
        &mut control,
    )
    .unwrap();
    assert!(nodes[0].surrounding.is_empty());
}

#[test]
fn repulsion_grid_plan_ignores_overflowed_unselected_storage_costs() {
    let extent = 8_000_000_000_000_000_000.0;
    let nodes = vec![
        node_at(0.0, 0.0, extent, extent),
        node_at(0.0, 0.0, extent, extent),
        node_at(0.0, 0.0, extent, extent),
    ];

    let plan = RepulsionGridPlan::from_geometry(0.0, 0.0, extent, extent, &nodes, 1.0, &[0, 1, 2])
        .unwrap()
        .expect("implicit grid plan");

    assert_eq!(plan.storage_kind(), RepulsionGridStorageKind::Implicit);
    assert_eq!(plan.work_units(), 21);
}

#[test]
fn repulsion_grid_plan_saturates_reference_count_for_implicit_storage() {
    let extent = 8_500_000_000_000_000_000.0;
    let nodes = vec![
        node_at(0.0, 0.0, extent, extent),
        node_at(0.0, 0.0, extent, extent),
        node_at(0.0, 0.0, extent, extent),
        node_at(0.0, 0.0, extent, extent),
        node_at(0.0, 0.0, extent, extent),
    ];

    let plan =
        RepulsionGridPlan::from_geometry(0.0, 0.0, extent, extent, &nodes, 1.0, &[0, 1, 2, 3, 4])
            .unwrap()
            .expect("implicit grid plan");

    assert_eq!(plan.storage_kind(), RepulsionGridStorageKind::Implicit);
    assert_eq!(plan.work_units(), 80);
}

#[test]
fn rejected_repulsion_grid_plan_does_not_mutate_node_grid_bounds() {
    let repulsion_range = 10.0;
    let mut nodes = vec![node_at(0.0, 0.0, 10.0, 10.0)];
    nodes[0].grid_start_x = -7;
    nodes[0].grid_finish_x = -6;
    nodes[0].grid_start_y = -5;
    nodes[0].grid_finish_y = -4;
    let mut control = RejectingPreflight::default();

    let result = RepulsionGrid::build_or_reuse(
        None,
        0.0,
        0.0,
        10.0,
        10.0,
        &mut nodes,
        repulsion_range,
        &[0],
        &mut control,
    );

    assert!(matches!(result, Err(WorkFailure::Interrupted)));
    assert_eq!(
        (
            nodes[0].grid_start_x,
            nodes[0].grid_finish_x,
            nodes[0].grid_start_y,
            nodes[0].grid_finish_y,
        ),
        (-7, -6, -5, -4)
    );
    assert!(control.charges.is_empty());
}

#[test]
fn repulsion_grid_storage_variants_preserve_surrounding_order() {
    fn surroundings_for(
        storage_kind: RepulsionGridStorageKind,
        source_nodes: &[SimNode],
        node_order: &[usize],
        repulsion_range: f64,
    ) -> Vec<Vec<usize>> {
        let mut nodes = source_nodes.to_vec();
        let mut plan = RepulsionGridPlan::from_geometry(
            0.0,
            0.0,
            25.0,
            25.0,
            &nodes,
            repulsion_range,
            node_order,
        )
        .unwrap()
        .expect("grid plan");
        plan.storage_kind = storage_kind;
        let mut grid = RepulsionGrid::build_from_plan(
            None,
            plan,
            0.0,
            0.0,
            &mut nodes,
            repulsion_range,
            node_order,
        )
        .unwrap();
        let mut work_control = NoopWorkControl;
        grid.prepare_refresh(&nodes, node_order, false, &mut work_control)
            .unwrap();
        let mut processed_generation = vec![0u32; nodes.len()];
        let mut surrounding_seen = vec![0u32; nodes.len()];
        let mut surrounding_seen_generation = 1u32;
        for &idx in node_order {
            grid.refresh_node_surrounding(
                idx,
                &mut nodes,
                &processed_generation,
                1,
                repulsion_range,
                &mut surrounding_seen,
                &mut surrounding_seen_generation,
                &mut work_control,
            )
            .unwrap();
            processed_generation[idx] = 1;
        }
        nodes.into_iter().map(|node| node.surrounding).collect()
    }

    let nodes = vec![
        node_at(0.0, 0.0, 15.0, 15.0),
        node_at(15.0, 0.0, 10.0, 10.0),
        node_at(0.0, 15.0, 10.0, 10.0),
        node_at(15.0, 15.0, 10.0, 10.0),
    ];
    let node_order = [0usize, 2, 1, 3];
    let dense = surroundings_for(RepulsionGridStorageKind::Dense, &nodes, &node_order, 10.0);
    let sparse = surroundings_for(RepulsionGridStorageKind::Sparse, &nodes, &node_order, 10.0);
    let implicit = surroundings_for(
        RepulsionGridStorageKind::Implicit,
        &nodes,
        &node_order,
        10.0,
    );

    assert_eq!(sparse, dense);
    assert_eq!(implicit, dense);
}

#[test]
fn repulsion_grid_promotes_cubic_materialized_queries_to_implicit() {
    fn assert_promoted(node_count: usize) {
        let repulsion_range = 10.0;
        let extent = node_count as f64 * repulsion_range;
        let mut nodes = (0..node_count)
            .map(|_| node_at(0.0, 0.0, extent, repulsion_range))
            .collect::<Vec<_>>();
        let node_order = (0..node_count).collect::<Vec<_>>();
        let plan = RepulsionGridPlan::from_geometry(
            0.0,
            0.0,
            extent,
            repulsion_range,
            &nodes,
            repulsion_range,
            &node_order,
        )
        .unwrap()
        .expect("dense preliminary plan");
        assert_eq!(plan.storage_kind(), RepulsionGridStorageKind::Dense);

        let implicit_work = usize::try_from(
            super::implicit_grid_work_units(node_count as u128).expect("implicit work"),
        )
        .unwrap();
        let scan_work = node_count * node_count;
        let mut control = RecordingWorkControl::default();
        let grid = RepulsionGrid::build_or_reuse(
            None,
            0.0,
            0.0,
            extent,
            repulsion_range,
            &mut nodes,
            repulsion_range,
            &node_order,
            &mut control,
        )
        .unwrap()
        .expect("promoted grid");

        assert!(matches!(
            grid.cells,
            super::RepulsionGridCells::Implicit { .. }
        ));
        assert_eq!(
            control.charges.iter().copied().sum::<usize>(),
            plan.work_units() + scan_work + implicit_work
        );
    }

    assert_promoted(32);
    assert_promoted(64);
}

#[test]
fn sparse_repulsion_grid_charges_cluster_candidate_visits_before_refresh() {
    let repulsion_range = 10.0;
    let mut nodes = (0..31)
        .map(|_| node_at(0.0, 0.0, 1.0, 1.0))
        .collect::<Vec<_>>();
    nodes.push(node_at(1_000_000_000.0, 0.0, 1.0, 1.0));
    let node_order = (0..nodes.len()).collect::<Vec<_>>();
    let mut control = RecordingWorkControl::default();
    let mut grid = RepulsionGrid::build_or_reuse(
        None,
        0.0,
        0.0,
        1_000_000_001.0,
        1.0,
        &mut nodes,
        repulsion_range,
        &node_order,
        &mut control,
    )
    .unwrap()
    .expect("sparse grid");
    assert!(matches!(grid.cells, super::RepulsionGridCells::Sparse(_)));

    let charges_before_refresh = control.charges.len();
    let processed_generation = vec![0u32; nodes.len()];
    let mut surrounding_seen = vec![0u32; nodes.len()];
    let mut surrounding_seen_generation = 1u32;
    grid.refresh_node_surrounding(
        0,
        &mut nodes,
        &processed_generation,
        1,
        repulsion_range,
        &mut surrounding_seen,
        &mut surrounding_seen_generation,
        &mut control,
    )
    .unwrap();

    assert_eq!(control.charges[charges_before_refresh..], [31]);
    assert_eq!(nodes[0].surrounding.len(), 30);
}

#[test]
fn rejected_materialized_refresh_preserves_existing_surrounding() {
    let repulsion_range = 10.0;
    let mut nodes = (0..31)
        .map(|_| node_at(0.0, 0.0, 1.0, 1.0))
        .collect::<Vec<_>>();
    nodes.push(node_at(1_000_000_000.0, 0.0, 1.0, 1.0));
    let node_order = (0..nodes.len()).collect::<Vec<_>>();
    let mut build_control = RecordingWorkControl::default();
    let mut grid = RepulsionGrid::build_or_reuse(
        None,
        0.0,
        0.0,
        1_000_000_001.0,
        1.0,
        &mut nodes,
        repulsion_range,
        &node_order,
        &mut build_control,
    )
    .unwrap()
    .expect("sparse grid");
    assert!(matches!(grid.cells, super::RepulsionGridCells::Sparse(_)));

    let sentinel = nodes.len() - 1;
    nodes[0].surrounding = vec![sentinel];
    let processed_generation = vec![0u32; nodes.len()];
    let mut surrounding_seen = vec![0u32; nodes.len()];
    let mut surrounding_seen_generation = 1u32;
    let mut rejecting = RejectingPreflight::default();
    let error = grid
        .refresh_node_surrounding(
            0,
            &mut nodes,
            &processed_generation,
            1,
            repulsion_range,
            &mut surrounding_seen,
            &mut surrounding_seen_generation,
            &mut rejecting,
        )
        .unwrap_err();

    assert_eq!(error, WorkFailure::Interrupted);
    assert_eq!(rejecting.checks, [31]);
    assert!(rejecting.charges.is_empty());
    assert_eq!(nodes[0].surrounding, [sentinel]);
}

#[test]
fn relative_placement_gap_is_center_to_center() {
    use super::{Constraints, RelConstraint, apply_constraints_to_displacements};

    let nodes = vec![
        node_at(0.0, 0.0, 10.0, 10.0),  // center_x = 5
        node_at(20.0, 0.0, 10.0, 10.0), // center_x = 25
    ];
    let mut disps = vec![(0.0, 0.0); nodes.len()];

    let c = Constraints {
        align_horizontal: Vec::new(),
        align_vertical: Vec::new(),
        relative: vec![RelConstraint {
            left: Some(0),
            right: Some(1),
            top: None,
            bottom: None,
            gap: 50.0,
        }],
    };

    apply_constraints_to_displacements(&nodes, &c, &mut disps, 1e9);
    let gap = (nodes[1].center_x() + disps[1].0) - (nodes[0].center_x() + disps[0].0);
    assert!((gap - 50.0).abs() < 1e-9, "gap: got {gap}");
}

#[test]
fn rect_clip_points_matches_layout_base_igeometry_getintersection2() {
    // Expected values computed via layout-base@2.0.1:
    //
    // `IGeometry.getIntersection(rectA, rectB, out)` where:
    // - rectA = (-274.090946,-129.901919,80,80)
    // - rectB = (512.630977,-782.722296,80,80)
    let a = node_at(-274.090_946, -129.901_919, 80.0, 80.0);
    let b = node_at(512.630_977, -782.722_296, 80.0, 80.0);
    let (ax, ay, bx, by) = super::rect_clip_points(&a, &b);

    let eps = 1e-6;
    assert!((ax - -194.090_946).abs() < eps, "ax: got {ax}");
    assert!((ay - -123.093_844_020_246_31).abs() < eps, "ay: got {ay}");
    assert!((bx - 512.630_977).abs() < eps, "bx: got {bx}");
    assert!((by - -709.530_370_979_753_7).abs() < eps, "by: got {by}");
}

#[test]
fn rects_intersect_keeps_positive_touch_gap_separate() {
    let a = node_at(0.0, 0.0, 80.0, 80.0);
    let exact_touch = node_at(80.0, 0.0, 80.0, 80.0);
    let positive_gap = node_at(80.0 + 1e-12, 0.0, 80.0, 80.0);
    let separated = node_at(80.0 + 1e-6, 0.0, 80.0, 80.0);

    assert!(super::rects_intersect(&a, &exact_touch));
    assert!(!super::rects_intersect(&a, &positive_gap));
    assert!(!super::rects_intersect(&a, &separated));
}

#[test]
fn overlap_separation_treats_nearly_equal_centers_as_equal() {
    let a = node_at(0.0, 0.0, 80.0, 80.0);
    let y_aligned = node_at(20.0, 1e-12, 80.0, 80.0);
    assert_eq!(
        super::decide_directions_for_overlapping_nodes(&a, &y_aligned),
        (-1.0, 1.0)
    );

    let near_same_center = node_at(1e-12, 1e-12, 80.0, 80.0);
    let (dx, dy) = super::calc_separation_amount(&a, &near_same_center, 0.0);
    assert!(
        (dx + 40.0).abs() < 1e-9 && (dy + 40.0).abs() < 1e-9,
        "expected exact-center separation direction, got ({dx}, {dy})"
    );
}

#[test]
fn constraint_handler_preserves_group_port_second_run_tiny_gap() {
    // Browser evidence for `stress_architecture_group_port_edges_017`, run=1:
    // Cytoscape/cose-base constraint handling leaves a 7.1e-15 positive gap between the
    // computed `inner` compound top and `out1` bottom after the next `updateBounds()` pass.
    // That tiny positive gap is enough for layout-base `RectangleD.intersects(...)` to return
    // false and for `inner/out1` repulsion to take the vertical clipping path.
    let mut nodes = vec![
        // in1
        node_at(-47.406_611_585_551_886, 59.051_469_403_565_15, 80.0, 80.0),
        // in2
        node_at(152.618_759_300_584_88, 59.051_469_403_565_15, 80.0, 80.0),
        // out1
        node_at(-47.406_611_585_551_886, -162.051_469_403_565_14, 80.0, 80.0),
        // ext
        node_at(-312.618_759_300_584_9, -162.051_469_403_565_14, 80.0, 80.0),
    ];
    let constraints = Constraints {
        align_horizontal: vec![vec![0, 1], vec![2, 3]],
        align_vertical: vec![vec![0, 2]],
        relative: vec![
            RelConstraint {
                left: Some(0),
                right: Some(1),
                top: None,
                bottom: None,
                gap: 120.0,
            },
            RelConstraint {
                left: None,
                right: None,
                top: Some(2),
                bottom: Some(0),
                gap: 120.0,
            },
            RelConstraint {
                left: Some(3),
                right: Some(2),
                top: None,
                bottom: None,
                gap: 120.0,
            },
        ],
    };

    let x: Vec<f64> = nodes.iter().map(|n| n.center_x()).collect();
    let y: Vec<f64> = nodes.iter().map(|n| n.center_y()).collect();
    let t = super::procrustes_transform_for_alignments(&x, &y, &constraints)
        .expect("alignment Procrustes transform");
    assert_eq!(
        t.m01.to_bits(),
        (f64::EPSILON / 2.0).to_bits(),
        "expected positive JS-compatible Procrustes skew, got {t:?}"
    );
    assert_eq!(
        t.m10.to_bits(),
        (-(f64::EPSILON / 2.0)).to_bits(),
        "expected negative JS-compatible Procrustes skew, got {t:?}"
    );

    let mut work_control = super::NoopWorkControl;
    super::handle_constraints_pre_layout(&mut nodes, &constraints, &mut work_control).unwrap();

    let inner_top_after_update_bounds = nodes[0].top.min(nodes[1].top) - 40.0;
    let out1_bottom = nodes[2].top + nodes[2].height;
    assert!(
        inner_top_after_update_bounds > out1_bottom,
        "expected a positive JS layout-base gap, got inner_top={inner_top_after_update_bounds:?} out1_bottom={out1_bottom:?} gap={:?}",
        inner_top_after_update_bounds - out1_bottom
    );

    let inner_left = nodes[0].left.min(nodes[1].left) - 40.0;
    let inner_right = nodes[0].right().max(nodes[1].right()) + 40.0;
    let mut inner = node_at(
        inner_left,
        inner_top_after_update_bounds,
        inner_right - inner_left,
        160.0,
    );
    inner.is_compound = true;
    inner.no_of_children = 2.0;

    assert!(
        !super::rects_intersect(&nodes[2], &inner),
        "expected positive-gap out1/inner pair to use the non-overlap clipping branch"
    );
    let (_out1_x, out1_y, _inner_x, inner_y) = super::rect_clip_points(&nodes[2], &inner);
    let eps = 1e-9;
    assert!((out1_y - out1_bottom).abs() < eps, "out1_y: {out1_y}");
    assert!(
        (inner_y - inner_top_after_update_bounds).abs() < eps,
        "inner_y: {inner_y}"
    );
}

#[test]
fn constraint_procrustes_transform_matches_upstream_fixture_025_checkpoint() {
    // Ground truth extracted via `tools/debug/arch_probe_fcose_vs_upstream_025.js`:
    //
    // - `draft.debug.recomputed`: raw spectral coordinates (pre-relocation)
    //
    // Upstream `ConstraintHandler` applies a Procrustes + reflection transform directly to the
    // raw coordinates; component relocation (`aux.relocateComponent(componentCenter, ...)`) is
    // performed later by cytoscape-fcose and shows up in `draft.pos` / `fromSpectral.*`.
    //
    // This test intentionally isolates the transform-only step on `draft.debug.recomputed`.
    //
    // This test guards against subtle transpose/sign mistakes in our Procrustes port.
    let ids = ["a", "b", "c", "d", "e", "f"];
    let draft = [
        (-69.77618192016361, 79.87553327881355),
        (34.28258770643722, 100.36650015929253),
        (104.06591551872783, 20.494458759991097),
        (69.78035458033064, -79.87753496744543),
        (-34.28895079233283, -100.37063982916823),
        (-104.06372509299923, -20.48831740148356),
    ];
    let expected = [
        (-63.516289670902054, 84.938197098671),
        (41.796999300167016, 97.47687430142939),
        (105.32067914788601, 12.54161697884377),
        (63.52029847371475, -84.94050953250945),
        (-41.80365806342419, -97.48051938039694),
        (-105.31802918744155, -12.535659466037792),
    ];

    let mut nodes: Vec<SimNode> = Vec::new();
    for (i, (x, y)) in draft.iter().copied().enumerate() {
        nodes.push(SimNode {
            parent: None,
            owner_idx: i,
            is_compound: false,
            width: 80.0,
            height: 80.0,
            bounds_extras: BoundsExtras::default(),
            estimated_size: 80.0,
            left: x - 40.0,
            top: y - 40.0,
            spring_fx: 0.0,
            spring_fy: 0.0,
            repulsion_fx: 0.0,
            repulsion_fy: 0.0,
            gravitation_fx: 0.0,
            gravitation_fy: 0.0,
            no_of_children: 1.0,
            padding: 0.0,
            surrounding: Vec::new(),
            grid_start_x: 0,
            grid_finish_x: 0,
            grid_start_y: 0,
            grid_finish_y: 0,
        });
    }

    let c = Constraints {
        align_horizontal: vec![vec![0, 5], vec![2, 3]],
        align_vertical: vec![vec![1, 2], vec![3, 4]],
        relative: vec![
            RelConstraint {
                left: Some(0),
                right: Some(5),
                top: None,
                bottom: None,
                gap: 120.0,
            },
            RelConstraint {
                left: Some(4),
                right: Some(1),
                top: None,
                bottom: None,
                gap: 120.0,
            },
            RelConstraint {
                left: None,
                right: None,
                top: Some(1),
                bottom: Some(2),
                gap: 120.0,
            },
            RelConstraint {
                left: None,
                right: None,
                top: Some(4),
                bottom: Some(3),
                gap: 120.0,
            },
            RelConstraint {
                left: Some(3),
                right: Some(2),
                top: None,
                bottom: None,
                gap: 120.0,
            },
        ],
    };

    let mut x: Vec<f64> = nodes.iter().map(|n| n.center_x()).collect();
    let mut y: Vec<f64> = nodes.iter().map(|n| n.center_y()).collect();

    let t = procrustes_transform_for_alignments(&x, &y, &c).expect("transform");
    let tt = t.transpose();
    for i in 0..x.len() {
        let r = tt.transform(Vec2::new(x[i], y[i]));
        x[i] = r.x;
        y[i] = r.y;
    }
    apply_reflection_for_relative_placement(&mut x, &mut y, &c.relative);

    for i in 0..ids.len() {
        let (ex, ey) = expected[i];
        let dx = (x[i] - ex).abs();
        let dy = (y[i] - ey).abs();
        assert!(
            dx < 1e-9 && dy < 1e-9,
            "mismatch for {}: got=({:.12},{:.12}) expected=({:.12},{:.12}) d=({:.3e},{:.3e})",
            ids[i],
            x[i],
            y[i],
            ex,
            ey,
            dx,
            dy
        );
    }
}
