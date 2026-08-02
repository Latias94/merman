use super::rough_lifecycle_probe::{
    StateRoughCacheFootprint, StateRoughGeometryCounters, StateRoughLifecycleOperationSnapshot,
    StateRoughLifecycleOutcome, StateRoughOperationCounters, StateRoughSeedResolution,
    state_rough_lifecycle_capture, state_rough_lifecycle_take_snapshots,
};
use merman_core::{Engine, MermaidConfig, ParseOptions};

const FALLBACK_SEEDS: [f64; 2] = [4_294_967_296.0, -1.0];

fn capture_state_operation(
    look: &str,
    configured_seed: f64,
    body: &str,
) -> StateRoughLifecycleOperationSnapshot {
    assert!(
        state_rough_lifecycle_take_snapshots().is_empty(),
        "State Rough dispatch capture should start without stale snapshots"
    );

    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": look,
        "handDrawnSeed": configured_seed,
    })));
    let source = format!("stateDiagram-v2\n{body}\n");
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("State Rough dispatch source should parse")
        .expect("State Rough dispatch source should be detected");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("State Rough dispatch render session should start");
    let artifact = crate::family::prepare(
        parsed,
        &crate::LayoutOptions::headless_svg_defaults(),
        session,
    )
    .expect("State Rough dispatch artifact should prepare");

    let capture = state_rough_lifecycle_capture();
    let rendered = artifact
        .render_svg(
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("state-rough-dispatch-test".to_string()),
                ..crate::svg::SvgRenderOptions::default()
            },
            &crate::svg::SvgDebugOptions::default(),
        )
        .expect("State Rough dispatch artifact should render");
    assert!(!rendered.svg().is_empty());
    drop(capture);

    let mut snapshots = state_rough_lifecycle_take_snapshots();
    assert_eq!(
        snapshots.len(),
        1,
        "one State render should produce exactly one Rough dispatch snapshot"
    );
    snapshots.pop().expect("State Rough dispatch snapshot")
}

fn assert_successful_fallback_operation(
    snapshot: &StateRoughLifecycleOperationSnapshot,
    configured_seed: f64,
    context: &str,
) {
    assert_eq!(snapshot.configured_seed, configured_seed, "{context}");
    assert_eq!(snapshot.resolved_seed, configured_seed, "{context}");
    assert_eq!(
        snapshot.seed_resolution,
        StateRoughSeedResolution::ConfiguredFallbackCapable,
        "{context}"
    );
    assert!(!snapshot.cache_allowed, "{context}");
    assert_eq!(
        snapshot.outcome,
        StateRoughLifecycleOutcome::Success,
        "{context}"
    );
    assert!(
        snapshot.release_proof.cache_drop_observed,
        "{context}: operation cache drop should be observed"
    );
}

fn assert_zero_release_witnesses(snapshot: &StateRoughLifecycleOperationSnapshot, context: &str) {
    assert_eq!(
        snapshot.operation_peak,
        StateRoughCacheFootprint::default(),
        "{context}: no operation-owned Rough entry should be retained"
    );
    assert_eq!(
        snapshot.release_proof.geometry_witnesses, 0,
        "{context}: no cached geometry should be witnessed"
    );
    assert_eq!(
        snapshot.release_proof.allocation_witnesses, 0,
        "{context}: no cached String allocation should be witnessed"
    );
    assert_eq!(
        snapshot.release_proof.witnessed_owned_bytes, 0,
        "{context}: no cached String capacity should be witnessed"
    );
    assert_eq!(
        snapshot.release_proof.live_allocation_witnesses, 0,
        "{context}: no cached String allocation should remain live"
    );
    assert_eq!(
        snapshot.release_proof.live_owned_bytes, 0,
        "{context}: no cached String capacity should remain live"
    );
}

fn assert_no_rough_dispatch(snapshot: &StateRoughLifecycleOperationSnapshot, context: &str) {
    assert_eq!(
        snapshot.counters,
        StateRoughOperationCounters::default(),
        "{context}: ordinary non-hand-drawn nodes should not request Rough geometry"
    );
    assert_zero_release_witnesses(snapshot, context);
}

fn assert_bypassed_geometry(
    actual: StateRoughGeometryCounters,
    expected_draws: usize,
    context: &str,
) {
    assert_eq!(
        actual,
        StateRoughGeometryCounters {
            draw_requests: expected_draws,
            bypass_builds: expected_draws,
            ..StateRoughGeometryCounters::default()
        },
        "{context}: fallback Rough requests should build exactly once per visible shape"
    );
}

#[test]
fn ordinary_state_rough_dispatch_matches_look() {
    let configured_seed = FALLBACK_SEEDS[0];
    for look in ["classic", "neo"] {
        let context = format!("ordinary {look} State node");
        let snapshot = capture_state_operation(look, configured_seed, "state \"same\" as A");
        assert_successful_fallback_operation(&snapshot, configured_seed, &context);
        assert_no_rough_dispatch(&snapshot, &context);
    }

    let context = "ordinary handDrawn State node";
    let snapshot = capture_state_operation("handDrawn", configured_seed, "state \"same\" as A");
    assert_successful_fallback_operation(&snapshot, configured_seed, context);
    assert_eq!(
        snapshot.counters.circle,
        StateRoughGeometryCounters::default(),
        "{context}: ordinary nodes should not request circle geometry"
    );
    assert_bypassed_geometry(snapshot.counters.paths, 1, context);
    assert_zero_release_witnesses(&snapshot, context);
}

#[test]
fn ordinary_non_hand_drawn_states_do_not_consume_fallback_rough_stream() {
    for look in ["classic", "neo"] {
        for configured_seed in FALLBACK_SEEDS {
            let context = format!("ordinary {look} State node with seed {configured_seed}");
            let snapshot = capture_state_operation(look, configured_seed, "state \"same\" as A");
            assert_successful_fallback_operation(&snapshot, configured_seed, &context);
            assert_no_rough_dispatch(&snapshot, &context);
        }
    }
}

#[test]
fn non_hand_drawn_special_state_shapes_keep_rough_requests() {
    struct SpecialShapeCase {
        name: &'static str,
        body: &'static str,
        circle_draws: usize,
        path_draws: usize,
    }

    let cases = [
        SpecialShapeCase {
            name: "stateEnd",
            body: "A --> [*]",
            circle_draws: 2,
            path_draws: 0,
        },
        SpecialShapeCase {
            name: "choice",
            body: "state A\nstate C <<choice>>\nA --> C",
            circle_draws: 0,
            path_draws: 1,
        },
        SpecialShapeCase {
            name: "note",
            body: "state A\nnote right of A : N",
            circle_draws: 0,
            path_draws: 1,
        },
        SpecialShapeCase {
            name: "fork/join",
            body: "state F <<fork>>\nstate J <<join>>\nF --> J",
            circle_draws: 0,
            path_draws: 2,
        },
    ];

    for look in ["classic", "neo"] {
        for case in &cases {
            let context = format!("{} control under {look} look", case.name);
            let snapshot = capture_state_operation(look, FALLBACK_SEEDS[0], case.body);
            assert_successful_fallback_operation(&snapshot, FALLBACK_SEEDS[0], &context);
            assert_bypassed_geometry(snapshot.counters.circle, case.circle_draws, &context);
            assert_bypassed_geometry(snapshot.counters.paths, case.path_draws, &context);
            assert_zero_release_witnesses(&snapshot, &context);
        }
    }
}
