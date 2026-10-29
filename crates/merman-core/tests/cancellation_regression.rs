use merman_core::{
    CancelReason, CustomJsonRenderModel, CustomJsonRenderParser, DiagramSemanticParser, Engine,
    Error, OperationCancelled, OperationControl, OperationControlResult, OperationPhase,
    ParseMetadata, ParseOptions, Result,
};
use serde_json::{Value, json};

const DIAGRAM_TYPE: &str = "flowchart-v2";
const SOURCE: &str = "flowchart TD\nA --> B\n";
const CANCELLED: OperationCancelled = OperationCancelled {
    phase: OperationPhase::Parse,
    reason: CancelReason::Requested,
};

fn assert_cancelled<T>(result: Result<T>) {
    match result {
        Err(Error::OperationCancelled(error)) => assert_eq!(error, CANCELLED),
        Err(error) => panic!("expected operation cancellation, got {error:?}"),
        Ok(_) => panic!("cancellation must not publish a model or an error diagram"),
    }
}

fn cancel_after_success(
    _code: &str,
    _meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<Value>> {
    control.cancel();
    Ok(Ok(json!({})))
}

fn cancel_after_failure(
    _code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<Value>> {
    control.cancel();
    Ok(Err(Error::diagram_parse_fallback(
        meta.diagram_type.clone(),
        "custom parser failed after requesting cancellation",
    )))
}

fn cancel_render_after_success(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<CustomJsonRenderModel>> {
    cancel_after_success(code, meta, control).map(|parsed| {
        parsed.map(|value| CustomJsonRenderModel::new(meta.diagram_type.clone(), value))
    })
}

fn cancel_render_after_failure(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<CustomJsonRenderModel>> {
    cancel_after_failure(code, meta, control).map(|parsed| {
        parsed.map(|value| CustomJsonRenderModel::new(meta.diagram_type.clone(), value))
    })
}

fn direct_cancellation<T>(
    _code: &str,
    _meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<T>> {
    assert!(!control.is_cancelled());
    Err(CANCELLED)
}

fn callback_checkpoint_cancellation<T>(
    _code: &str,
    _meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<T>> {
    control.cancel();
    Err(control.checkpoint().expect_err("requested cancellation"))
}

#[test]
fn ordinary_json_observes_cancellation_after_callback_success_or_failure() {
    for (known_type, options, parser) in [
        (
            false,
            ParseOptions::strict(),
            cancel_after_success as DiagramSemanticParser,
        ),
        (true, ParseOptions::strict(), cancel_after_failure),
        (true, ParseOptions::lenient(), cancel_after_failure),
    ] {
        let mut engine = Engine::new();
        engine.diagram_registry_mut().insert(DIAGRAM_TYPE, parser);

        let result = if known_type {
            engine.parse_diagram_with_type_sync(DIAGRAM_TYPE, SOURCE, options)
        } else {
            engine.parse_diagram_sync(SOURCE, options)
        };

        assert_cancelled(result);
    }
}

#[test]
fn ordinary_render_observes_callback_cancellation_in_parse_phase() {
    for (known_type, options, parser) in [
        (
            false,
            ParseOptions::strict(),
            cancel_render_after_success as CustomJsonRenderParser,
        ),
        (true, ParseOptions::lenient(), cancel_render_after_failure),
    ] {
        let mut engine = Engine::new();
        engine
            .render_diagram_registry_mut()
            .insert(DIAGRAM_TYPE, parser);

        let result = if known_type {
            engine.parse_diagram_for_render_model_with_type_sync(DIAGRAM_TYPE, SOURCE, options)
        } else {
            engine.parse_diagram_for_render_model_sync(SOURCE, options)
        };

        assert_cancelled(result);
    }
}

#[test]
fn ordinary_render_semantic_fallback_preserves_callback_cancellation() {
    for (known_type, options, parser) in [
        (
            true,
            ParseOptions::strict(),
            cancel_after_success as DiagramSemanticParser,
        ),
        (false, ParseOptions::lenient(), cancel_after_failure),
    ] {
        let mut engine = Engine::new();
        engine.diagram_registry_mut().insert(DIAGRAM_TYPE, parser);

        let result = if known_type {
            engine.parse_diagram_for_render_model_with_type_sync(DIAGRAM_TYPE, SOURCE, options)
        } else {
            engine.parse_diagram_for_render_model_sync(SOURCE, options)
        };

        assert_cancelled(result);
    }
}

#[test]
fn lenient_ordinary_parsing_preserves_direct_callback_cancellation() {
    let mut semantic_engine = Engine::new();
    semantic_engine
        .diagram_registry_mut()
        .insert(DIAGRAM_TYPE, direct_cancellation::<Value>);
    assert_cancelled(semantic_engine.parse_diagram_with_type_sync(
        DIAGRAM_TYPE,
        SOURCE,
        ParseOptions::lenient(),
    ));

    let mut render_engine = Engine::new();
    render_engine
        .render_diagram_registry_mut()
        .insert(DIAGRAM_TYPE, direct_cancellation::<CustomJsonRenderModel>);
    for engine in [semantic_engine, render_engine] {
        assert_cancelled(
            engine.parse_diagram_for_render_model_sync(SOURCE, ParseOptions::lenient()),
        );
    }
}

#[test]
fn ordinary_callbacks_observe_parse_phase_at_their_own_checkpoint() {
    let mut engine = Engine::new();
    engine
        .diagram_registry_mut()
        .insert(DIAGRAM_TYPE, callback_checkpoint_cancellation::<Value>);
    engine.render_diagram_registry_mut().insert(
        DIAGRAM_TYPE,
        callback_checkpoint_cancellation::<CustomJsonRenderModel>,
    );

    assert_cancelled(engine.parse_diagram_sync(SOURCE, ParseOptions::strict()));
    assert_cancelled(engine.parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict()));
}

#[test]
fn controlled_render_keeps_callback_cancellation_in_outer_result() {
    let mut semantic_engine = Engine::new();
    semantic_engine
        .diagram_registry_mut()
        .insert(DIAGRAM_TYPE, cancel_after_failure);
    let mut render_engine = Engine::new();
    render_engine
        .render_diagram_registry_mut()
        .insert(DIAGRAM_TYPE, cancel_render_after_success);

    for (engine, options) in [
        (semantic_engine, ParseOptions::lenient()),
        (render_engine, ParseOptions::strict()),
    ] {
        let control = OperationControl::new();
        let error = engine
            .parse_diagram_for_render_model_controlled_sync(SOURCE, options, &control)
            .expect_err("callback cancellation must use the outer result");

        assert_eq!(error, CANCELLED);
        assert!(control.is_cancelled());
    }
}

#[test]
fn controlled_render_preserves_direct_cancellation_without_cancelled_control() {
    let mut semantic_engine = Engine::new();
    semantic_engine
        .diagram_registry_mut()
        .insert(DIAGRAM_TYPE, direct_cancellation::<Value>);
    let mut render_engine = Engine::new();
    render_engine
        .render_diagram_registry_mut()
        .insert(DIAGRAM_TYPE, direct_cancellation::<CustomJsonRenderModel>);

    for engine in [semantic_engine, render_engine] {
        let control = OperationControl::new();
        let error = engine
            .parse_diagram_for_render_model_with_type_controlled_sync(
                DIAGRAM_TYPE,
                SOURCE,
                ParseOptions::lenient(),
                &control,
            )
            .expect_err("direct cancellation must use the outer result");

        assert_eq!(error, CANCELLED);
        assert!(!control.is_cancelled());
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#[test]
fn ordinary_json_callback_deadline_precedes_a_suppressible_parse_error() {
    let mut engine = Engine::new();
    engine
        .diagram_registry_mut()
        .insert(DIAGRAM_TYPE, |_, meta, control| {
            assert!(control.set_deadline(std::time::Duration::ZERO));
            Ok(Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                "custom parser failed after expiring its deadline",
            )))
        });

    let error = engine
        .parse_diagram_sync(SOURCE, ParseOptions::lenient())
        .expect_err("callback deadline must prevent error JSON");
    let Error::OperationCancelled(error) = error else {
        panic!("callback deadline must remain an operation cancellation");
    };
    assert_eq!(error.phase, OperationPhase::Parse);
    assert_eq!(error.reason, CancelReason::DeadlineExceeded);
}

#[test]
fn ordinary_json_parse_failures_remain_suppressible_without_cancellation() {
    let mut engine = Engine::new();
    engine
        .diagram_registry_mut()
        .insert(DIAGRAM_TYPE, |_, meta, _| {
            Ok(Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                "ordinary custom parser failure",
            )))
        });

    assert!(matches!(
        engine.parse_diagram_sync(SOURCE, ParseOptions::strict()),
        Err(Error::DiagramParse { diagram_type, .. }) if diagram_type == DIAGRAM_TYPE
    ));
    let parsed = engine
        .parse_diagram_sync(SOURCE, ParseOptions::lenient())
        .expect("ordinary parse failures remain suppressible")
        .expect("suppressed parse failure produces an error diagram");
    assert_eq!(parsed.meta.diagram_type, "error");
}

#[cfg(feature = "diagram-git-graph")]
#[test]
fn controlled_render_preserves_the_caller_runtime_context() {
    use merman_core::{RenderSemanticModel, runtime::RuntimePolicy};

    let context = RuntimePolicy::deterministic()
        .with_fixed_seed(42)
        .begin_operation()
        .expect("fixed runtime context");
    let engine =
        Engine::new().with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(7));
    let expected_id = format!("0-{}", context.derive_hex("git-graph.commit-id", 0, 7));
    assert_ne!(
        expected_id,
        format!(
            "0-{}",
            engine
                .begin_operation()
                .unwrap()
                .derive_hex("git-graph.commit-id", 0, 7)
        )
    );

    for known_type in [false, true] {
        let control = OperationControl::new();
        let source = "gitGraph\ncommit\n";
        let result = if known_type {
            engine.parse_diagram_for_render_model_with_type_controlled_in_context_sync(
                "gitGraph",
                source,
                ParseOptions::strict(),
                &control,
                &context,
            )
        } else {
            engine.parse_diagram_for_render_model_controlled_in_context_sync(
                source,
                ParseOptions::strict(),
                &control,
                &context,
            )
        };
        let parsed = result
            .expect("active operation control")
            .expect("GitGraph parse succeeds")
            .expect("GitGraph render model");
        let RenderSemanticModel::GitGraph(model) = parsed.model() else {
            panic!("expected the built-in GitGraph model");
        };
        assert_eq!(model.commits.len(), 1);
        assert_eq!(model.commits[0].id, expected_id);
    }
}
