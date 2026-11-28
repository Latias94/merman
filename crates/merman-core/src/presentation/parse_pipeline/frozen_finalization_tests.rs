use std::cell::Cell;

use super::*;

#[cfg(feature = "diagram-info")]
#[test]
fn source_defaults_freeze_without_charging_low_model_admission_limits() {
    use merman_model::resources::{
        InputResourceLimitId, InputResourcePolicy, ModelComplexity, ResourceProfile,
    };

    let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
        .with_limit(InputResourceLimitId::MaxModelItems, 1)
        .unwrap()
        .with_limit(InputResourceLimitId::MaxModelTextBytes, 1)
        .unwrap()
        .with_limit(InputResourceLimitId::MaxModelNestingDepth, 1)
        .unwrap();
    let engine = MermaidEngine::new();
    let source_defaults = engine.default_effective_config().unwrap();
    let config_complexity = ModelComplexity::from_json(source_defaults.as_value());
    assert!(config_complexity.items > 1);
    assert!(config_complexity.text_bytes > 1);
    assert!(config_complexity.nesting_depth > 1);
    assert!(policy.check_model_complexity(config_complexity).is_err());

    let frozen = SourcePresentationMetadata {
        diagram_type: "info".to_string(),
        config: MermaidConfig::empty_object(),
        effective_config: source_defaults.clone(),
        title: None,
    }
    .freeze_controlled(&OperationControl::new())
    .unwrap()
    .unwrap();
    assert_eq!(
        frozen.effective_config.as_value(),
        source_defaults.as_value()
    );

    let full = engine
        .parse_diagram_for_render_model_with_type_sync("info", "info", ParseOptions::strict())
        .unwrap()
        .unwrap();
    let actual = merman_model::resources::model_complexity_from_presentation(full.presentation());
    assert_eq!(actual, ModelComplexity::new(1, 0, 0));
    policy.check_model_complexity(actual).unwrap();
    assert_eq!(full.metadata().effective_config, frozen.effective_config);
}

#[test]
fn fallible_finish_is_not_suppressed_as_a_family_parse_error() {
    let engine = MermaidEngine::new();
    let context = engine.begin_operation().unwrap();
    let pipeline = ParsePipeline::known_type(&engine, "info", "info", ParseOptions::lenient());
    let sanitized = Cell::new(false);
    let remapped = Cell::new(false);
    let result = pipeline
        .parse_model_controlled_in_context(
            ParseTiming::Render,
            &OperationControl::new(),
            &context,
            |_, _, _, _| Ok(Ok(())),
            |_, _| sanitized.set(true),
            |_, _, _| -> OperationControlResult<Result<()>> {
                panic!("finish failure must not become a suppressed diagram")
            },
            |metadata, _, _| {
                assert!(sanitized.get());
                assert!(remapped.get());
                assert_eq!(metadata.diagram_type, "info");
                assert!(metadata.effective_config.as_value().is_object());
                Ok(Err(crate::InternalFailure::new(
                    "checked full-data construction failed",
                )
                .into()))
            },
            |model, _, _, _| {
                remapped.set(true);
                Ok(Ok(model))
            },
            |_| None,
        )
        .unwrap();
    assert!(
        matches!(result, Err(Error::Internal(error)) if error.message() == "checked full-data construction failed")
    );
}

#[test]
fn postprocess_inner_errors_preserve_strict_and_lenient_semantics() {
    let assert_projection_error = |error: &Error| {
        let Error::DiagramParse {
            diagram_type,
            diagnostic,
        } = error
        else {
            panic!("postprocess failure must retain its original error kind");
        };
        assert_eq!(diagram_type, "info");
        assert_eq!(diagnostic.message(), "postprocess projection failed");
    };
    for options in [ParseOptions::strict(), ParseOptions::lenient()] {
        let engine = MermaidEngine::new();
        let context = engine.begin_operation().unwrap();
        let pipeline = ParsePipeline::known_type(&engine, "info", "info", options);
        let suppressed = Cell::new(false);
        let result = pipeline
            .parse_model_controlled_in_context(
                ParseTiming::Render,
                &OperationControl::new(),
                &context,
                |_, _, _, _| Ok(Ok(())),
                |_, _| {},
                |metadata, error, _| {
                    assert_eq!(metadata.diagram_type, "info");
                    assert_projection_error(error);
                    suppressed.set(true);
                    Ok(Ok("suppressed"))
                },
                |_, _, _| panic!("postprocess failure must not finish a successful model"),
                |_, _, _, _| -> OperationControlResult<Result<()>> {
                    Ok(Err(Error::diagram_parse_fallback(
                        "info",
                        "postprocess projection failed",
                    )))
                },
                |_| None,
            )
            .expect("ordinary postprocess errors must remain in the inner result");

        if options.suppress_errors {
            assert_eq!(result.unwrap(), Some("suppressed"));
        } else {
            assert_projection_error(&result.unwrap_err());
        }
        assert_eq!(suppressed.get(), options.suppress_errors);
    }
}

#[test]
fn postprocess_cancellation_preempts_success_and_suppressed_errors() {
    for options in [ParseOptions::strict(), ParseOptions::lenient()] {
        for (request_cancellation, expected_phase) in [
            (false, OperationPhase::Parse),
            (true, OperationPhase::Semantic),
        ] {
            let engine = MermaidEngine::new();
            let context = engine.begin_operation().unwrap();
            let pipeline = ParsePipeline::known_type(&engine, "info", "info", options);
            let control = OperationControl::new();
            let result = pipeline.parse_model_controlled_in_context(
                ParseTiming::Render,
                &control,
                &context,
                |_, _, _, _| Ok(Ok(())),
                |_, _| {},
                |_, _, _| -> OperationControlResult<Result<()>> {
                    panic!("postprocess cancellation must not produce a suppressed diagram")
                },
                |_, _, _| panic!("postprocess cancellation must not finish a successful model"),
                |_, _, _, control| -> OperationControlResult<Result<()>> {
                    assert!(!control.is_cancelled());
                    if request_cancellation {
                        control.cancel();
                        Ok(Err(Error::diagram_parse_fallback(
                            "info",
                            "postprocess projection failed after cancellation",
                        )))
                    } else {
                        Err(crate::OperationCancelled {
                            phase: OperationPhase::Parse,
                            reason: crate::CancelReason::Requested,
                        })
                    }
                },
                |_| None,
            );

            assert_eq!(
                result.expect_err("postprocess cancellation must stay on the outer channel"),
                crate::OperationCancelled {
                    phase: expected_phase,
                    reason: crate::CancelReason::Requested,
                },
                "suppress_errors={}, request_cancellation={request_cancellation}",
                options.suppress_errors,
            );
            assert_eq!(control.is_cancelled(), request_cancellation);
        }
    }
}

#[test]
fn cancellation_after_finish_stays_on_the_outer_semantic_channel() {
    let engine = MermaidEngine::new();
    let context = engine.begin_operation().unwrap();
    let pipeline = ParsePipeline::known_type(&engine, "info", "info", ParseOptions::lenient());
    let result = pipeline.parse_model_controlled_in_context(
        ParseTiming::Render,
        &OperationControl::new(),
        &context,
        |_, _, _, _| Ok(Ok(())),
        |_, _| {},
        |_, _, _| -> OperationControlResult<Result<()>> {
            panic!("cancellation must not become a suppressed diagram")
        },
        |_, _, control| {
            control.cancel();
            Ok(Ok(()))
        },
        |model, _, _, _| Ok(Ok(model)),
        |_| None,
    );
    assert!(matches!(result, Err(cancelled) if cancelled.phase == OperationPhase::Semantic));
}
