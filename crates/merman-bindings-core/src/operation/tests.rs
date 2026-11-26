use super::*;

type OneShotDiagramDataExecutor = fn(
    &ValidatedArtifactContract,
    &[u8],
    &[u8],
    BindingEngineServices,
    Option<OperationControl>,
) -> Result<Vec<u8>, BindingError>;

#[test]
fn request_builder_preserves_execution_time_validation() {
    let request = BindingOperationRequest::new("document-analysis-json", b"flowchart TD\nA")
        .with_uri(b"file:///diagram.mmd")
        .with_options_json(br#"{"parse":{"suppress_errors":true}}"#);

    assert_eq!(request.operation_id(), "document-analysis-json");
    assert_eq!(request.source(), b"flowchart TD\nA");
    assert_eq!(request.input(), b"flowchart TD\nA");
    assert_eq!(request.uri(), Some(b"file:///diagram.mmd".as_slice()));
    assert_eq!(
        request.options_json(),
        br#"{"parse":{"suppress_errors":true}}"#
    );

    let defaults = BindingOperationRequest::new("semantic-json", b"flowchart TD\nA");
    assert_eq!(defaults.uri(), None);
    assert_eq!(defaults.options_json(), b"");

    assert_eq!(defaults.clone().with_uri(b"").uri(), None);
    assert_eq!(defaults.with_optional_uri(Some(b"")).uri(), None);
}

#[test]
fn typed_metadata_decodes_known_output_plans() {
    let json = br#"{
            "version":1,
            "operation_id":"png",
            "media_type":"image/png",
            "runtime_policy":"custom",
            "byte_length":42,
            "future_top_level":{"kept":true},
            "output_plan":{
                "kind":"raster",
                "requested_width_px":320.5,
                "requested_height_px":200.25,
                "width_px":320,
                "height_px":200,
                "requested_scale":2.0,
                "effective_scale":1.5,
                "limited":true,
                "future_plan_field":"kept"
            }
        }"#;
    let metadata = BindingOperationMetadata::from_json_bytes(json).unwrap();

    assert_eq!(metadata.version(), 1);
    assert_eq!(metadata.operation_id(), "png");
    assert_eq!(metadata.media_type(), "image/png");
    assert_eq!(metadata.runtime_policy(), "custom");
    assert_eq!(metadata.byte_length(), 42);
    assert_eq!(metadata.json_bytes(), json);
    let Some(BindingOutputPlan::Raster(plan)) = metadata.output_plan() else {
        panic!("expected a raster output plan");
    };
    assert_eq!(plan.width_px(), 320);
    assert_eq!(plan.height_px(), 200);
    assert_eq!(plan.effective_scale(), 1.5);
    assert!(plan.limited());
}

#[test]
fn typed_metadata_preserves_unknown_plan_and_exact_original_json() {
    let json = br#"{ "version": 1, "operation_id": "future", "media_type": "application/x-future", "runtime_policy": "future-policy", "byte_length": 7, "output_plan": { "kind": "future-plan", "nested": { "answer": 42 } } }"#;
    let metadata = BindingOperationMetadata::from_json_bytes(json).unwrap();

    assert_eq!(metadata.json_bytes(), json);
    assert_eq!(metadata.runtime_policy(), "future-policy");
    let Some(BindingOutputPlan::Unknown(plan)) = metadata.output_plan() else {
        panic!("expected an unknown output plan");
    };
    assert_eq!(plan.kind(), "future-plan");
    assert_eq!(plan.value()["nested"]["answer"], 42);
}

fn ascii_metadata_value() -> Value {
    serde_json::json!({
        "version": 1,
        "operation_id": "ascii",
        "media_type": "text/plain; charset=utf-8",
        "runtime_policy": "deterministic",
        "byte_length": 7,
        "output_plan": {
            "kind": "ascii",
            "schema_version": 4,
            "family": "flowchart-v2",
            "projection": "structured_text",
            "encoding": "plain",
            "primary_width": null,
            "primary_height": null,
            "emitted_width": 7,
            "emitted_height": 1,
            "width_profile": "unicode",
            "layout_profile": null,
            "requested_layout_profile": "canonical",
            "requested_graph_presentation": "adaptive",
            "compact_attempted": false,
            "requested_max_width": 80,
            "primary_overflowed": null,
            "emitted_overflowed": false,
            "outcome": "fallback",
            "fallback_capability": "available",
            "fallback_attempted": true,
            "fallback_reason": "routing_search_exhausted",
            "trimmed": false,
            "lossiness": "presentation_only"
        }
    })
}

#[test]
fn typed_ascii_metadata_distinguishes_unmeasured_and_empty_primary() {
    let unmeasured = ascii_metadata_value();
    let mut empty = unmeasured.clone();
    let plan = &mut empty["output_plan"];
    plan["primary_width"] = Value::from(0);
    plan["primary_height"] = Value::from(0);
    plan["layout_profile"] = Value::from("canonical");
    plan["primary_overflowed"] = Value::from(false);
    plan["emitted_width"] = Value::from(0);
    plan["emitted_height"] = Value::from(0);
    plan["requested_graph_presentation"] = Value::from("strict");
    plan["outcome"] = Value::from("empty");
    plan["fallback_attempted"] = Value::from(false);
    plan["fallback_reason"] = Value::Null;
    plan["lossiness"] = Value::from("none");

    for (value, expected_width, expected_profile, expected_overflow) in [
        (unmeasured, None, None, None),
        (empty, Some(0), Some("canonical"), Some(false)),
    ] {
        let json = serde_json::to_vec(&value).unwrap();
        let metadata = BindingOperationMetadata::from_json_bytes(&json).unwrap();
        assert_eq!(metadata.version(), 1);
        assert_eq!(metadata.json_bytes(), json);
        let Some(BindingOutputPlan::Ascii(plan)) = metadata.output_plan() else {
            panic!("expected an ASCII output plan");
        };
        assert_eq!(plan.schema_version(), 4);
        assert_eq!(plan.primary_width(), expected_width);
        assert_eq!(plan.primary_height(), expected_width);
        assert_eq!(plan.layout_profile(), expected_profile);
        assert_eq!(plan.primary_overflowed(), expected_overflow);
        assert!(!plan.emitted_overflowed());
        assert_eq!(
            serde_json::to_value(metadata.output_plan().unwrap()).unwrap(),
            value["output_plan"]
        );
    }
}

#[test]
fn typed_ascii_metadata_requires_schema_four_and_explicit_nullable_fields() {
    for field in [
        "primary_width",
        "primary_height",
        "layout_profile",
        "primary_overflowed",
        "requested_graph_presentation",
        "emitted_overflowed",
    ] {
        let mut value = ascii_metadata_value();
        value["output_plan"].as_object_mut().unwrap().remove(field);
        let error = BindingOperationMetadata::from_json_bytes(&serde_json::to_vec(&value).unwrap())
            .expect_err("schema 4 fields must be explicit, including nulls");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains(field), "{field}: {error:?}");
    }
    for (field, invalid) in [
        ("primary_width", Value::from(-1)),
        ("primary_height", Value::from(1.5)),
        ("layout_profile", Value::from(false)),
        ("primary_overflowed", Value::from("false")),
        ("requested_graph_presentation", Value::Null),
        ("emitted_overflowed", Value::Null),
    ] {
        let mut value = ascii_metadata_value();
        value["output_plan"][field] = invalid;
        assert!(
            BindingOperationMetadata::from_json_bytes(&serde_json::to_vec(&value).unwrap())
                .is_err(),
            "{field} must reject invalid types"
        );
    }
    for version in [0, 1, 2, 3, 5, 65536] {
        let mut value = ascii_metadata_value();
        value["output_plan"]["schema_version"] = Value::from(version);
        let error = BindingOperationMetadata::from_json_bytes(&serde_json::to_vec(&value).unwrap())
            .expect_err("ASCII schema revisions must not silently gain schema 4 meanings");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }
}

#[test]
fn typed_ascii_metadata_rejects_inconsistent_primary_availability() {
    for (width, height, profile, overflow) in [
        (Some(0), None, None, None),
        (None, Some(0), None, None),
        (None, None, Some("canonical"), None),
        (None, None, None, Some(false)),
        (Some(0), Some(0), None, Some(false)),
        (Some(0), Some(0), Some("canonical"), None),
    ] {
        let mut value = ascii_metadata_value();
        let plan = &mut value["output_plan"];
        plan["primary_width"] = serde_json::to_value(width).unwrap();
        plan["primary_height"] = serde_json::to_value(height).unwrap();
        plan["layout_profile"] = serde_json::to_value(profile).unwrap();
        plan["primary_overflowed"] = serde_json::to_value(overflow).unwrap();
        let error = BindingOperationMetadata::from_json_bytes(&serde_json::to_vec(&value).unwrap())
            .expect_err("primary dimensions, layout and overflow must have matching availability");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }
}

#[test]
fn typed_ascii_metadata_preserves_planning_reasons_without_inventing_overflow() {
    for reason in ["routing_search_exhausted", "port_capacity_exceeded"] {
        for (max_width, emitted_width, emitted_overflowed) in
            [(None, 7, false), (Some(80), 7, false), (Some(5), 7, true)]
        {
            let mut value = ascii_metadata_value();
            value["output_plan"]["fallback_reason"] = Value::from(reason);
            value["output_plan"]["requested_max_width"] = serde_json::to_value(max_width).unwrap();
            value["output_plan"]["emitted_width"] = Value::from(emitted_width);
            value["output_plan"]["emitted_overflowed"] = Value::from(emitted_overflowed);
            let metadata =
                BindingOperationMetadata::from_json_bytes(&serde_json::to_vec(&value).unwrap())
                    .unwrap();
            let Some(BindingOutputPlan::Ascii(plan)) = metadata.output_plan() else {
                panic!("expected an ASCII output plan");
            };
            assert_eq!(plan.fallback_reason(), Some(reason));
            assert_eq!(plan.requested_graph_presentation(), "adaptive");
            assert_eq!(plan.primary_width(), None);
            assert_eq!(plan.primary_overflowed(), None);
            assert_eq!(plan.emitted_overflowed(), emitted_overflowed);
            assert_eq!(plan.requested_max_width(), max_width);
        }
    }
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_plan_copies_canonical_metadata_without_reinterpreting_availability() {
    let output = merman::Renderer::new()
        .render(merman::RenderRequest::ascii(
            "flowchart LR\nA --> B",
            OperationControl::new(),
            merman::AsciiRequest::default(),
        ))
        .unwrap();
    let merman::RenderOutput::Ascii(Some(output)) = output else {
        panic!("expected canonical ASCII output");
    };
    for width in [None, Some(0), Some(40)] {
        let mut metadata = output.metadata();
        metadata.primary_width = width;
        metadata.primary_height = width;
        metadata.layout_profile = width.map(|_| "canonical".to_owned());
        metadata.primary_overflowed = width.map(|_| false);
        metadata.requested_graph_presentation = "adaptive".to_owned();
        metadata.emitted_overflowed = false;
        let expected = serde_json::to_value(&metadata).unwrap();
        let plan = BindingAsciiOutputPlan::from_metadata(metadata);
        assert_eq!(serde_json::to_value(plan).unwrap(), expected);
    }
}

#[test]
fn known_output_plan_requires_all_schema_one_fields() {
    let error = BindingOperationMetadata::from_json_bytes(
            br#"{"version":1,"operation_id":"png","media_type":"image/png","runtime_policy":"deterministic","byte_length":1,"output_plan":{"kind":"raster"}}"#,
        )
        .unwrap_err();

    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(error.message().contains("requested_width_px"));
}

#[test]
fn typed_metadata_round_trips_theme_outcome() {
    let json = br#"{ "version": 1, "operation_id": "svg", "media_type": "image/svg+xml", "runtime_policy": "deterministic", "byte_length": 7, "theme_outcome": { "mode": "merman_native", "preset_id": "studio", "appearance": "dark", "notices": [] } }"#;
    let metadata = BindingOperationMetadata::from_json_bytes(json).unwrap();
    let outcome = metadata.theme_outcome().unwrap();
    assert_eq!(outcome["mode"], "merman_native");
    assert_eq!(outcome["preset_id"], "studio");
    assert_eq!(outcome["appearance"], "dark");
    assert_eq!(outcome["notices"], serde_json::json!([]));
    assert_eq!(metadata.json_bytes(), json);
}

#[test]
fn descriptor_owned_operation_ids_round_trip() {
    let specs = crate::capability::OPERATION_SPECS;
    let operations = BindingOperationKind::all().collect::<Vec<_>>();
    assert_eq!(operations.len(), specs.len());
    for (operation, spec) in operations.into_iter().zip(specs) {
        assert_eq!(BindingOperationKind::from_id(spec.id).unwrap(), operation);
        assert_eq!(operation.operation_id(), spec.id);
        assert!(!operation.media_type().is_empty());
    }
}

#[test]
fn svg_capability_planning_is_a_descriptor_owned_operation() {
    let operation = BindingOperationKind::from_id("svg-plan-json").unwrap();

    assert_eq!(operation.availability_capability_id(), Some("svg"));
    assert_eq!(operation.media_type(), "application/json");
    assert!(!operation.requires_uri());
    assert_eq!(operation.resource_scope(), BindingResourceScope::Svg);
}

#[test]
fn edge_geometry_json_is_a_descriptor_owned_operation() {
    let operation = BindingOperationKind::from_id("edge-geometry-json").unwrap();

    assert_eq!(operation.availability_capability_id(), Some("svg"));
    assert_eq!(operation.media_type(), "application/json");
    assert!(!operation.requires_uri());
}

#[cfg(feature = "svg")]
#[test]
fn edge_geometry_json_is_scoped_to_its_actual_output_budget() {
    let operation = BindingOperationKind::from_id("edge-geometry-json").unwrap();
    let scope = operation.resource_scope();

    assert!(scope.accepts("max_geometry_json_bytes"));
    assert!(scope.accepts("max_layout_work_units"));
    assert!(scope.accepts("max_prepared_text_retained_bytes"));
    assert!(!scope.accepts("max_svg_bytes"));
    assert!(!scope.accepts("max_svg_elements"));
    assert!(!scope.accepts("svg_backend_tree_depth"));
    for scope in [
        BindingResourceScope::Layout,
        BindingResourceScope::Svg,
        BindingResourceScope::Png,
        BindingResourceScope::Jpeg,
        BindingResourceScope::Pdf,
    ] {
        assert!(!scope.accepts("max_geometry_json_bytes"));
    }
}

#[test]
fn unknown_operation_is_a_typed_error() {
    let error = BindingOperationKind::from_id("bitmap").unwrap_err();
    assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::UnknownOperation);
    assert_eq!(error.capability_id(), None);
    assert!(error.message().contains("unknown operation `bitmap`"));
}

#[test]
fn semantic_parse_is_a_base_operation_not_a_fake_feature_capability() {
    let semantic = BindingOperationKind::from_id("semantic-json").unwrap();
    assert_eq!(semantic.availability_capability_id(), None);
    assert!(!semantic.requires_uri());

    for operation in BindingOperationKind::all() {
        if operation.requires_uri() {
            assert!(operation.operation_id().starts_with("document-analysis-"));
        }
    }
}

#[test]
fn transport_options_default_to_deterministic_policy() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let result = engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();

    assert_eq!(metadata["runtime_policy"], "deterministic");
    assert!(metadata.get("theme_outcome").is_none());
    assert!(result.metadata().theme_outcome().is_none());
}

#[test]
fn operation_request_cannot_override_engine_runtime_policy() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let error = engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"runtime_policy":"native"}"#,
            operation_control: None,
        })
        .unwrap_err();

    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("cannot set runtime_policy"));
}

#[test]
fn operation_request_merges_wrapped_options_over_direct_engine_options() {
    let engine =
        BindingEngine::from_options(br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#)
            .unwrap();
    let result = engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"analysis":{"resources":{"limits":{"max_source_bytes":2048}}}}"#,
            operation_control: None,
        })
        .unwrap();
    assert!(!result.data.is_empty());
}

#[test]
fn reusable_semantic_output_is_stable_across_empty_version_and_real_overlays() {
    let engine =
        BindingEngine::from_options(br#"{"parse":{"suppress_errors":false},"version":4}"#).unwrap();
    let execute = |options_json| {
        engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json,
                operation_control: None,
            })
            .unwrap()
    };

    let empty = execute(b"");
    for unchanged in [
        br#"{}"#.as_slice(),
        br#"{"version":4}"#.as_slice(),
        b"{\n  \"version\": 4\n}".as_slice(),
    ] {
        let result = execute(unchanged);
        assert_eq!(result.data, empty.data);
        assert_eq!(result.metadata_json(), empty.metadata_json());
    }
    let real = execute(br#"{"parse":{"suppress_errors":true}}"#);

    assert_eq!(real.data, empty.data);
    assert_eq!(real.metadata_json(), empty.metadata_json());
}

#[test]
fn reusable_byte_execution_matches_result_data_and_errors_without_metadata() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let request = BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B");
    let result = engine.execute(request.clone()).unwrap();

    reset_metadata_serialization_count();
    assert_eq!(engine.parse_json(request.source()).unwrap(), result.data());
    assert_eq!(metadata_serialization_count(), 0);

    reset_metadata_serialization_count();
    assert_eq!(engine.execute_data(request).unwrap(), result.data());
    assert_eq!(metadata_serialization_count(), 0);

    for request in [
        BindingOperationRequest::new("unknown-operation", b"flowchart TD\nA --> B")
            .with_options_json(b"{"),
        BindingOperationRequest::new("document-analysis-json", b"flowchart TD\nA --> B")
            .with_options_json(b"{"),
        BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
            .with_options_json(b"{"),
        BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
            .with_options_json(br#"{"resources":{"limits":{"max_source_bytes":4}}}"#),
    ] {
        let expected = engine.execute(request.clone()).unwrap_err();

        reset_metadata_serialization_count();
        assert_eq!(engine.execute_data(request).unwrap_err(), expected);
        assert_eq!(metadata_serialization_count(), 0);
    }
}

#[test]
fn one_shot_byte_execution_matches_result_data_and_errors_without_metadata() {
    let request = BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B");
    let result = execute_once(request.clone()).unwrap();

    reset_metadata_serialization_count();
    assert_eq!(
        crate::parse_json(request.source(), b"").unwrap(),
        result.data()
    );
    assert_eq!(metadata_serialization_count(), 0);

    reset_metadata_serialization_count();
    assert_eq!(execute_once_data(request).unwrap(), result.data());
    assert_eq!(metadata_serialization_count(), 0);

    for request in [
        BindingOperationRequest::new("unknown-operation", b"flowchart TD\nA --> B")
            .with_options_json(b"{"),
        BindingOperationRequest::new("document-analysis-json", b"flowchart TD\nA --> B")
            .with_options_json(b"{"),
        BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
            .with_options_json(b"{"),
        BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
            .with_options_json(br#"{"resources":{"limits":{"max_source_bytes":4}}}"#),
    ] {
        let expected = execute_once(request.clone()).unwrap_err();

        reset_metadata_serialization_count();
        assert_eq!(execute_once_data(request).unwrap_err(), expected);
        assert_eq!(metadata_serialization_count(), 0);
    }
}

#[test]
fn narrow_one_shot_execution_preserves_admission_and_error_order() {
    const CONTRACT: ValidatedArtifactContract = crate::ArtifactContractSpec::new(
        crate::TargetKey::Typst,
        crate::BindingTransportKey::Typst,
    )
    .with_operations(&[])
    .materialize();

    for (operation_id, execute_data) in [
        (
            "svg",
            ValidatedArtifactContract::render_svg_once_data_with_services
                as OneShotDiagramDataExecutor,
        ),
        (
            "analysis-json",
            ValidatedArtifactContract::analyze_json_once_data_with_services,
        ),
    ] {
        for options in [
            b"".as_slice(),
            b"{".as_slice(),
            br#"{"resources":{"limits":{"max_svg_bytes":64}}}"#.as_slice(),
        ] {
            let expected = CONTRACT
                .execute_once(
                    BindingOperationRequest::new(operation_id, b"").with_options_json(options),
                )
                .unwrap_err();
            let error = execute_data(&CONTRACT, b"", options, BindingEngineServices::new(), None)
                .unwrap_err();
            assert_eq!(error, expected, "{operation_id}");
            if options == b"{" {
                assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            } else if options.is_empty() {
                assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
                assert!(error.message().contains("not exposed by target `typst`"));
            }
        }

        let control = OperationControl::new();
        control.cancel();
        let expected = CONTRACT
            .execute_once(
                BindingOperationRequest::new(operation_id, b"")
                    .with_options_json(b"{")
                    .with_control(control.clone()),
            )
            .unwrap_err();
        let error = execute_data(
            &CONTRACT,
            b"",
            b"{",
            BindingEngineServices::new(),
            Some(control),
        )
        .unwrap_err();
        assert_eq!(error, expected);
        assert_structured_requested_cancellation(&error, "admission");

        let expired = OperationControl::new().with_deadline(std::time::Duration::ZERO);
        let error = execute_data(
            &CONTRACT,
            b"",
            b"{",
            BindingEngineServices::new(),
            Some(expired.clone()),
        )
        .unwrap_err();
        let expected = CONTRACT
            .execute_once(
                BindingOperationRequest::new(operation_id, b"")
                    .with_options_json(b"{")
                    .with_control(expired),
            )
            .unwrap_err();
        assert_eq!(error, expected);
        assert_eq!(
            error.cancellation_details(),
            Some(crate::BindingCancellationErrorDetails {
                reason: "deadline_exceeded",
                phase: "admission",
            }),
        );
    }
}

#[cfg(all(feature = "analysis", feature = "diagram-flowchart"))]
#[test]
fn narrow_analysis_execution_preserves_results_and_diagnostics_without_metadata() {
    let contract = crate::artifact_contract::default_artifact_contract();
    for (source, options) in [
        (b"flowchart TD\nA --> B".as_slice(), b"".as_slice()),
        (b"not a diagram".as_slice(), b"".as_slice()),
        (b"flowchart TD\nA[unfinished".as_slice(), b"".as_slice()),
        (
            b"flowchart TD\nA --> B".as_slice(),
            br#"{"resources":{"limits":{"max_source_bytes":4}}}"#.as_slice(),
        ),
        (b"flowchart TD\nA --> B".as_slice(), b"{".as_slice()),
    ] {
        let expected = contract.execute_once_data(
            BindingOperationRequest::new("analysis-json", source).with_options_json(options),
        );
        reset_metadata_serialization_count();
        assert_eq!(
            contract.analyze_json_once_data_with_services(
                source,
                options,
                BindingEngineServices::new(),
                None,
            ),
            expected,
        );
        assert_eq!(metadata_serialization_count(), 0);
    }
}

#[cfg(feature = "svg")]
#[test]
fn svg_data_execution_matches_result_without_serializing_metadata() {
    let engine = BindingEngine::new(b"").unwrap();
    for options in [
        b"".as_slice(),
        br#"{"theme":"studio"}"#.as_slice(),
        br#"{"svg":{"pipeline":"resvg-safe"}}"#.as_slice(),
    ] {
        let request = BindingOperationRequest::new("svg", b"flowchart TD\nA --> B")
            .with_options_json(options);
        let result = engine.execute(request.clone()).unwrap();
        assert!(result.metadata().theme_outcome().is_some());

        reset_metadata_serialization_count();
        assert_eq!(engine.execute_data(request.clone()).unwrap(), result.data());
        assert_eq!(metadata_serialization_count(), 0);

        reset_metadata_serialization_count();
        assert_eq!(execute_once_data(request).unwrap(), result.data());
        assert_eq!(metadata_serialization_count(), 0);

        assert_eq!(
            crate::artifact_contract::default_artifact_contract()
                .render_svg_once_data_with_services(
                    b"flowchart TD\nA --> B",
                    options,
                    BindingEngineServices::new(),
                    None,
                )
                .unwrap(),
            result.data(),
        );
    }
}

#[cfg(feature = "svg")]
#[test]
fn svg_data_execution_preserves_source_and_resource_errors() {
    let engine = BindingEngine::new(b"").unwrap();
    for (source, options) in [
        (b"not a diagram".as_slice(), b"".as_slice()),
        (b"flowchart TD\nA[unfinished".as_slice(), b"".as_slice()),
        (
            b"flowchart TD\nA --> B".as_slice(),
            br#"{"resources":{"limits":{"max_svg_bytes":64}}}"#.as_slice(),
        ),
    ] {
        let request = BindingOperationRequest::new("svg", source).with_options_json(options);
        let expected = engine.execute(request.clone()).unwrap_err();
        assert_eq!(engine.execute_data(request.clone()).unwrap_err(), expected);
        assert_eq!(execute_once_data(request).unwrap_err(), expected);
        assert_eq!(
            crate::artifact_contract::default_artifact_contract()
                .render_svg_once_data_with_services(
                    source,
                    options,
                    BindingEngineServices::new(),
                    None,
                )
                .unwrap_err(),
            expected,
        );
    }
}

#[cfg(all(feature = "layout-dagre", feature = "diagram-flowchart"))]
#[test]
fn narrow_svg_execution_preserves_service_cancellation() {
    struct CancellingHostTextMeasurer(OperationControl);

    impl crate::HostTextMeasurer for CancellingHostTextMeasurer {
        fn measure(
            &self,
            _request: crate::HostTextMeasurementRequest<'_>,
        ) -> crate::HostMeasurementResult {
            self.0.cancel();
            Ok(None)
        }
    }

    const CONTRACT: ValidatedArtifactContract = crate::ArtifactContractSpec::new(
        crate::TargetKey::Native,
        crate::BindingTransportKey::Rust,
    )
    .with_operations(&[OperationKey::Svg])
    .with_supplemental_capabilities(&[crate::CapabilityKey::LayoutDagre])
    .with_constructor_services(&[crate::ConstructorServiceKey::HostTextMeasurement])
    .materialize();
    let source =
        b"---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA[Measured by host] --> B[Cancelled]";
    let mut expected = None;
    for narrow in [false, true] {
        let control = OperationControl::new();
        let services = BindingEngineServices::new()
            .with_host_text_measurer(Arc::new(CancellingHostTextMeasurer(control.clone())));
        let error = if narrow {
            CONTRACT
                .render_svg_once_data_with_services(source, b"", services, Some(control))
                .unwrap_err()
        } else {
            CONTRACT
                .execute_once_with_services(
                    BindingOperationRequest::new("svg", source).with_control(control),
                    services,
                )
                .unwrap_err()
        };
        assert_eq!(error.status(), BindingStatus::Cancelled);
        let details = error.cancellation_details().expect("callback cancellation");
        assert_eq!(details.reason, "requested");
        assert_ne!(details.phase, "admission");
        if let Some(expected) = &expected {
            assert_eq!(&error, expected);
        } else {
            expected = Some(error);
        }
    }
}

#[test]
fn reusable_engine_rejects_ambiguous_analysis_wrappers_at_construction() {
    let error = BindingEngine::from_options(
        br##"{
                "merman": { "fixed_today": "2025-01-01" },
                "analysis": {}
            }"##,
    )
    .err()
    .expect("ambiguous wrappers must fail before a reusable engine is created");

    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(
        error
            .message()
            .contains("must not contain both `analysis` and `merman` wrappers"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn unknown_operation_precedes_invalid_request_options() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let error = engine
        .execute(BindingOperationRequest {
            operation_id: "unknown-operation",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"{",
            operation_control: None,
        })
        .expect_err("operation resolution runs before request option parsing");

    assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::UnknownOperation);
    assert!(error.message().contains("unknown operation"));
}

#[test]
fn uri_presence_validation_precedes_invalid_request_options() {
    let engine = BindingEngine::from_options(b"").unwrap();

    let missing = engine
        .execute(BindingOperationRequest {
            operation_id: "document-analysis-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"{",
            operation_control: None,
        })
        .expect_err("missing URI is rejected before malformed options");
    assert_eq!(missing.status(), BindingStatus::InvalidArgument);
    assert!(missing.message().contains("requires a document URI"));

    let unexpected = engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: Some(b"file:///diagram.mmd"),
            options_json: b"{",
            operation_control: None,
        })
        .expect_err("unexpected URI is rejected before malformed options");
    assert_eq!(unexpected.status(), BindingStatus::InvalidArgument);
    assert!(
        unexpected
            .message()
            .contains("does not accept a document URI")
    );

    let empty_document_uri = engine
        .execute(
            BindingOperationRequest::new("document-analysis-json", b"flowchart TD\nA --> B")
                .with_uri(b"")
                .with_options_json(b"{"),
        )
        .expect_err("an empty URI is omission, so URI admission still wins");
    assert_eq!(empty_document_uri.status(), BindingStatus::InvalidArgument);
    assert!(
        empty_document_uri
            .message()
            .contains("requires a document URI")
    );

    let empty_non_document_uri = engine
        .execute(
            BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
                .with_uri(b"")
                .with_options_json(b"{"),
        )
        .expect_err("an empty URI is omission, so option parsing is reached");
    assert_eq!(
        empty_non_document_uri.status(),
        BindingStatus::OptionsJsonError
    );
}

#[cfg(feature = "analysis")]
#[test]
fn invalid_request_options_precede_invalid_document_uri_bytes() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let invalid_uri = [0xff];

    let options_error = engine
        .execute(BindingOperationRequest {
            operation_id: "document-analysis-json",
            source: b"flowchart TD\nA --> B",
            uri: Some(&invalid_uri),
            options_json: b"{",
            operation_control: None,
        })
        .expect_err("options are parsed before URI bytes are decoded");
    assert_eq!(options_error.status(), BindingStatus::OptionsJsonError);

    let uri_error = engine
        .execute(BindingOperationRequest {
            operation_id: "document-analysis-json",
            source: b"flowchart TD\nA --> B",
            uri: Some(&invalid_uri),
            options_json: b"",
            operation_control: None,
        })
        .expect_err("valid options allow execution to reach URI decoding");
    assert_eq!(uri_error.status(), BindingStatus::Utf8Error);
}

#[cfg(feature = "svg")]
#[test]
fn semantic_request_preserves_render_option_validation_before_source_errors() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let invalid_source = [0xff];
    let error = engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: &invalid_source,
            uri: None,
            options_json: br#"{"svg":{"pipeline":"invalid-pipeline"}}"#,
            operation_control: None,
        })
        .expect_err("artifact-wide request validation checks the render domain first");

    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(
        error
            .message()
            .contains("unsupported svg.pipeline: invalid-pipeline"),
        "unexpected error: {error:?}"
    );
}

#[cfg(all(feature = "analysis", feature = "svg"))]
#[test]
fn semantic_request_preserves_analysis_before_render_validation_order() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let error = engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{
                    "lint": { "profile": "invalid-profile" },
                    "svg": { "pipeline": "invalid-pipeline" }
                }"#,
            operation_control: None,
        })
        .expect_err("artifact-wide request validation checks analysis before rendering");

    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(
        error.message().contains("lint.profile")
            && error.message().contains("core, recommended, or strict"),
        "unexpected error: {error:?}"
    );
    assert!(!error.message().contains("svg.pipeline"));
}

#[cfg(not(feature = "png"))]
#[test]
fn invalid_request_options_precede_missing_operation_capability() {
    let engine = BindingEngine::from_options(b"").unwrap();
    let options_error = engine
        .execute(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"{",
            operation_control: None,
        })
        .expect_err("request options are validated before operation execution");
    assert_eq!(options_error.status(), BindingStatus::OptionsJsonError);

    let capability_error = engine
        .execute(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .expect_err("valid options reach the missing capability check");
    assert_eq!(
        capability_error.status(),
        BindingStatus::UnsupportedOperation
    );
    assert_eq!(
        capability_error.kind(),
        crate::BindingErrorKind::MissingCapability
    );
    assert_eq!(capability_error.capability_id(), Some("png"));
}

#[test]
fn every_compiled_operation_shares_one_shot_and_reusable_result_contracts() {
    let engine = BindingEngine::from_options(b"").unwrap();

    for operation in BindingOperationKind::all().filter(|operation| operation.is_compiled()) {
        let input = match operation.input_kind() {
            "mermaid-source" => b"flowchart TD\nA --> B".as_slice(),
            "theme-resolve-request-json" => {
                br#"{"document":{"schema_version":2,"extends":"studio"},"appearance":"dark"}"#
                    .as_slice()
            }
            "theme-preset-id" => b"studio".as_slice(),
            other => panic!(
                "compiled operation `{}` has no representative input for `{other}`",
                operation.operation_id()
            ),
        };
        let second_options = match operation.operation_id() {
            "export-theme-preset" | "resolve-theme" => b"{}".as_slice(),
            _ => br#"{"parse":{"suppress_errors":false}}"#.as_slice(),
        };
        for options_json in [b"".as_slice(), second_options] {
            let request = BindingOperationRequest {
                operation_id: operation.operation_id(),
                source: input,
                uri: operation
                    .requires_uri()
                    .then_some(b"file:///diagram.mmd".as_slice()),
                options_json,
                operation_control: None,
            };
            let one_shot = execute_once(request.clone()).unwrap_or_else(|error| {
                panic!(
                    "one-shot operation `{}` failed: {}",
                    operation.operation_id(),
                    error.message()
                )
            });
            let reusable = engine.execute(request).unwrap_or_else(|error| {
                panic!(
                    "reusable operation `{}` failed: {}",
                    operation.operation_id(),
                    error.message()
                )
            });

            assert_eq!(
                one_shot,
                reusable,
                "operation={}, options={}",
                operation.operation_id(),
                String::from_utf8_lossy(options_json)
            );
            assert_eq!(one_shot.operation, operation);
            assert_eq!(one_shot.media_type, operation.media_type());
            let metadata: serde_json::Value =
                serde_json::from_slice(one_shot.metadata_json()).unwrap();
            assert_eq!(
                metadata["operation_id"],
                operation.operation_id(),
                "operation={}",
                operation.operation_id()
            );
            assert_eq!(
                metadata["media_type"],
                operation.media_type(),
                "operation={}",
                operation.operation_id()
            );
            assert_eq!(
                metadata["byte_length"],
                one_shot.data.len(),
                "operation={}",
                operation.operation_id()
            );
        }
    }
}

#[test]
fn one_shot_pre_cancelled_request_precedes_invalid_options_after_operation_resolution() {
    let control = OperationControl::new();
    control.cancel();

    let error = execute_once(
        BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
            .with_options_json(b"{")
            .with_control(control),
    )
    .expect_err("admission cancellation must skip option parsing and engine construction");
    assert_structured_requested_cancellation(&error, "admission");

    let unknown_control = OperationControl::new();
    unknown_control.cancel();
    let unknown = execute_once(
        BindingOperationRequest::new("unknown-operation", b"flowchart TD\nA --> B")
            .with_options_json(b"{")
            .with_control(unknown_control),
    )
    .expect_err("operation resolution keeps precedence over cancellation");
    assert_eq!(unknown.kind(), crate::BindingErrorKind::UnknownOperation);
}

#[cfg(feature = "svg")]
#[test]
fn svg_request_propagates_caller_control_into_the_canonical_renderer() {
    assert_render_request_propagates_control("svg");
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_propagates_caller_control_into_the_canonical_renderer() {
    assert_render_request_propagates_control("ascii");
}

#[cfg(feature = "ascii")]
#[test]
fn adaptive_ascii_requests_preserve_cancellation_across_transport_paths() {
    let options = br#"{"ascii":{"graphPresentation":"adaptive"}}"#;
    let engine = BindingEngine::from_options(options).unwrap();
    let control = OperationControl::new();
    control.cancel();
    let request = BindingOperationRequest::new("ascii", b"flowchart LR\nA --> B")
        .with_options_json(options)
        .with_control(control);
    let error = engine.execute(request.clone()).unwrap_err();
    assert_structured_requested_cancellation(&error, "admission");
    assert_eq!(engine.execute_data(request.clone()).unwrap_err(), error);
    assert_eq!(execute_once(request.clone()).unwrap_err(), error);
    assert_eq!(execute_once_data(request).unwrap_err(), error);
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn assert_render_request_propagates_control(operation_id: &'static str) {
    let engine = BindingEngine::from_options(b"").unwrap();
    let pre_cancelled = OperationControl::new();
    pre_cancelled.cancel();
    let request = BindingOperationRequest::new(operation_id, b"flowchart TD\nA --> B")
        .with_control(pre_cancelled);
    let error = engine
        .execute(request.clone())
        .expect_err("a pre-cancelled request must not produce render output");
    assert_structured_requested_cancellation(&error, "admission");
    assert_eq!(engine.execute_data(request.clone()).unwrap_err(), error);
    assert_eq!(execute_once_data(request).unwrap_err(), error);

    let request = BindingOperationRequest::new(operation_id, b"flowchart TD\nA --> B")
        .with_control(OperationControl::new().with_deadline(std::time::Duration::ZERO));
    let error = engine.execute(request.clone()).unwrap_err();
    assert_eq!(engine.execute_data(request).unwrap_err(), error);
}

fn assert_structured_requested_cancellation(error: &BindingError, phase: &'static str) {
    assert_eq!(error.status(), BindingStatus::Cancelled);
    assert_eq!(error.resource_details(), None);
    assert_eq!(
        error.cancellation_details(),
        Some(crate::BindingCancellationErrorDetails {
            reason: "requested",
            phase,
        })
    );
}

#[cfg(feature = "analysis")]
#[test]
fn every_analysis_operation_propagates_caller_cancellation_during_analysis() {
    let engine = BindingEngine::from_options(b"").unwrap();
    for operation_id in [
        "analysis-json",
        "analysis-facts-json",
        "validation-json",
        "document-analysis-json",
        "document-analysis-facts-json",
    ] {
        let control = OperationControl::new();
        control.cancel();
        let mut request = BindingOperationRequest::new(operation_id, b"flowchart TD\nA --> B")
            .with_control(control);
        if operation_id.starts_with("document-") {
            request = request.with_uri(b"file:///diagram.md");
        }

        let error = engine
            .execute(request)
            .expect_err("caller cancellation must stop analysis before output escapes");

        assert_eq!(error.status(), BindingStatus::Cancelled, "{operation_id}");
        assert_eq!(error.resource_details(), None, "{operation_id}");
        assert!(
            matches!(
                error.cancellation_details(),
                Some(crate::BindingCancellationErrorDetails {
                    reason: "requested",
                    phase: "admission" | "analysis",
                })
            ),
            "{operation_id}",
        );
    }
}

#[cfg(feature = "analysis")]
#[test]
fn every_analysis_operation_preserves_caller_deadline_reason() {
    let engine = BindingEngine::from_options(b"").unwrap();
    for operation_id in [
        "analysis-json",
        "analysis-facts-json",
        "validation-json",
        "document-analysis-json",
        "document-analysis-facts-json",
    ] {
        let expired = OperationControl::new().with_deadline(std::time::Duration::ZERO);
        let result = match operation_id {
            "analysis-json" => engine.analyze_json_data(b"flowchart TD\nA --> B", &expired),
            "analysis-facts-json" => {
                engine.analysis_facts_json_data(b"flowchart TD\nA --> B", &expired)
            }
            "validation-json" => engine.validate_json_data(b"flowchart TD\nA --> B", &expired),
            "document-analysis-json" => engine.analyze_document_json_data(
                b"```mermaid\nflowchart TD\nA --> B\n```\n",
                b"file:///diagram.md",
                &expired,
            ),
            "document-analysis-facts-json" => engine.analyze_document_facts_json_data(
                b"```mermaid\nflowchart TD\nA --> B\n```\n",
                b"file:///diagram.md",
                &expired,
            ),
            _ => unreachable!(),
        };
        let error = result.expect_err("an expired caller deadline must stop analysis");
        assert_eq!(error.status(), BindingStatus::Cancelled, "{operation_id}");
        assert_eq!(
            error.cancellation_details(),
            Some(crate::BindingCancellationErrorDetails {
                reason: "deadline_exceeded",
                phase: "analysis",
            }),
            "{operation_id}",
        );
    }
}

#[cfg(feature = "svg")]
#[test]
fn one_shot_options_reject_limits_owned_by_another_operation() {
    let error = execute_once(BindingOperationRequest {
        operation_id: "semantic-json",
        source: b"flowchart TD\nA --> B",
        uri: None,
        options_json: br#"{"resources":{"limits":{"max_svg_bytes":1024}}}"#,
        operation_control: None,
    })
    .unwrap_err();

    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(error.message().contains("max_svg_bytes"));
    assert!(error.message().contains("semantic-model"));
}

#[cfg(feature = "svg")]
#[test]
fn one_shot_with_services_preserves_operation_resource_scope() {
    struct PanicHostTextMeasurer;

    impl crate::HostTextMeasurer for PanicHostTextMeasurer {
        fn measure(
            &self,
            _request: crate::HostTextMeasurementRequest<'_>,
        ) -> crate::HostMeasurementResult {
            panic!("resource scope validation must precede service-backed execution")
        }
    }

    const CONTRACT: ValidatedArtifactContract = crate::ArtifactContractSpec::new(
        crate::TargetKey::Native,
        crate::BindingTransportKey::Rust,
    )
    .with_operations(&[OperationKey::SemanticJson, OperationKey::Svg])
    .with_constructor_services(&[crate::ConstructorServiceKey::HostTextMeasurement])
    .materialize();

    let request = BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
        .with_options_json(br#"{"resources":{"limits":{"max_svg_bytes":1024}}}"#);
    let expected = CONTRACT.execute_once(request.clone()).unwrap_err();
    let error = CONTRACT
        .execute_once_with_services(
            request,
            BindingEngineServices::new().with_host_text_measurer(Arc::new(PanicHostTextMeasurer)),
        )
        .unwrap_err();

    assert_eq!(error, expected);
    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(error.message().contains("max_svg_bytes"));
    assert!(error.message().contains("semantic-model"));
}

#[test]
fn reusable_request_resource_overlays_only_tighten_the_constructor_ceiling() {
    let engine = BindingEngine::from_options(
        br#"{"resources":{"profile":"constrained","limits":{"max_source_bytes":64}}}"#,
    )
    .unwrap();

    for options_json in [
        br#"{"resources":{"profile":"trusted-native"}}"#.as_slice(),
        br#"{"resources":{"limits":{"max_source_bytes":65}}}"#.as_slice(),
        br#"{"resources":null}"#.as_slice(),
    ] {
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json,
                operation_control: None,
            })
            .unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    }
}

#[test]
fn request_resource_tightening_does_not_mutate_the_engine_or_caller_control() {
    let engine = BindingEngine::from_options(
        br#"{"resources":{"profile":"constrained","limits":{"max_source_bytes":64}}}"#,
    )
    .unwrap();
    let control = OperationControl::new();
    let request = BindingOperationRequest {
        operation_id: "semantic-json",
        source: b"flowchart TD\nA --> B",
        uri: None,
        options_json: br#"{"resources":{"limits":{"max_source_bytes":4}}}"#,
        operation_control: Some(control.clone()),
    };
    let error = engine.execute(request.clone()).unwrap_err();
    assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);

    let baseline = engine
        .execute(BindingOperationRequest {
            options_json: b"",
            ..request
        })
        .unwrap();
    assert!(!baseline.data.is_empty());
    assert!(control.checkpoint().is_ok());
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_cannot_widen_the_constructor_grid_ceiling() {
    let engine =
        BindingEngine::from_options(br#"{"resources":{"limits":{"max_ascii_grid_cells":1}}}"#)
            .unwrap();

    let error = engine
        .execute(BindingOperationRequest {
            operation_id: "ascii",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"limits":{"max_ascii_grid_cells":2}}}"#,
            operation_control: None,
        })
        .unwrap_err();

    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("max_ascii_grid_cells"));
}

#[test]
fn one_shot_operation_may_choose_a_nondefault_profile() {
    let result = execute_once(BindingOperationRequest {
        operation_id: "semantic-json",
        source: b"flowchart TD\nA --> B",
        uri: None,
        options_json: br#"{"resources":{"profile":"trusted-native"}}"#,
        operation_control: None,
    })
    .unwrap();

    assert!(!result.data.is_empty());
}

#[cfg(feature = "svg")]
#[test]
fn reusable_constructor_accepts_artifact_union_but_request_scope_rejects_sibling_limit() {
    let engine = BindingEngine::from_options(
        br#"{"resources":{"profile":"constrained","limits":{"max_svg_bytes":1048576}}}"#,
    )
    .unwrap();
    let error = engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"limits":{"max_svg_bytes":524288}}}"#,
            operation_control: None,
        })
        .unwrap_err();

    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(error.message().contains("max_svg_bytes"));
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn output_options_are_artifact_wide_at_construction_and_operation_scoped_per_request() {
    let engine = BindingEngine::from_options(
            br##"{"export_matte":"#123456","raster":{"scale":2},"jpeg":{"quality":85},"pdf":{"page_paint":"white"}}"##,
        )
        .expect("constructor accepts the compiled artifact option union");
    engine
        .execute(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .expect("unrelated constructor options do not affect semantic operations");

    let error = execute_once(BindingOperationRequest {
        operation_id: "semantic-json",
        source: b"flowchart TD\nA --> B",
        uri: None,
        options_json: br#"{"raster":{"scale":2}}"#,
        operation_control: None,
    })
    .unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("raster"));

    let error = engine
        .execute(
            BindingOperationRequest::new("svg", b"flowchart TD\nA --> B")
                .with_options_json(br##"{"export_matte":"#123456"}"##),
        )
        .unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("export_matte"));
}

#[test]
fn explicit_native_policy_follows_the_default_artifact_exposure() {
    let contract = crate::artifact_contract::default_artifact_contract();
    assert_eq!(
        contract.runtime_policy_exposure(),
        crate::RuntimePolicyExposure::BindingOptions
    );
    let capabilities = contract.runtime_capabilities();
    let missing_adapter = contract
        .validate_native_runtime_policy()
        .err()
        .and_then(|error| error.capability_id());

    if let Some(missing_adapter) = missing_adapter {
        assert!(capabilities.system_adapter_ids.is_empty());
        let error = match BindingEngine::from_options(br#"{"runtime_policy":"native"}"#) {
            Ok(_) => panic!("the default artifact accepted an incomplete native policy"),
            Err(error) => error,
        };
        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some(missing_adapter));

        let free_function_error =
            crate::parse_json(b"flowchart TD\nA --> B", br#"{"runtime_policy":"native"}"#)
                .expect_err("free functions must honor the exact transport adapter set");
        assert_eq!(
            free_function_error.status(),
            BindingStatus::UnsupportedOperation
        );
        assert_eq!(
            free_function_error.kind(),
            crate::BindingErrorKind::MissingCapability
        );
        assert_eq!(free_function_error.capability_id(), Some(missing_adapter));
    } else {
        let engine = BindingEngine::from_options(br#"{"runtime_policy":"native"}"#).unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
                operation_control: None,
            })
            .unwrap();
        let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();

        assert_eq!(metadata["runtime_policy"], "native");
    }
}

#[cfg(feature = "svg")]
#[test]
fn generic_svg_operation_produces_versioned_metadata() {
    let engine = BindingEngine::new(b"").unwrap();
    let result = engine
        .execute(BindingOperationRequest {
            operation_id: "svg",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .unwrap();

    assert_eq!(result.operation.operation_id(), "svg");
    assert_eq!(result.media_type, "image/svg+xml");
    assert!(result.data.starts_with(b"<svg"));
    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
    assert_eq!(metadata["version"], BINDING_OPERATION_SCHEMA_VERSION);
    assert_eq!(metadata["operation_id"], "svg");
    assert_eq!(metadata["byte_length"], result.data.len());
    assert_eq!(metadata["runtime_policy"], "deterministic");
    let svg = std::str::from_utf8(result.data()).unwrap();
    let document = roxmltree::Document::parse(svg).unwrap();
    let view_box = document.root_element().attribute("viewBox").unwrap();
    let expected = view_box
        .split_whitespace()
        .map(|value| value.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    let viewport = result.metadata().svg_viewport().unwrap();
    assert_eq!(
        [
            viewport.min_x,
            viewport.min_y,
            viewport.width,
            viewport.height
        ],
        expected.as_slice()
    );
    let decoded = BindingOperationMetadata::from_json_bytes(result.metadata_json()).unwrap();
    assert_eq!(decoded.svg_viewport(), Some(viewport));
    assert_eq!(
        metadata["theme_outcome"],
        serde_json::json!({
            "mode": "mermaid_parity", "notices": [],
        })
    );
}

#[cfg(all(feature = "svg", feature = "diagram-info"))]
#[test]
fn responsive_info_svg_remains_available_without_invented_viewport_metadata() {
    let engine = BindingEngine::new(b"").unwrap();
    let result = engine
        .execute(BindingOperationRequest {
            operation_id: "svg",
            source: b"info",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .unwrap();
    let svg = std::str::from_utf8(result.data()).unwrap();
    let document = roxmltree::Document::parse(svg).unwrap();
    assert_eq!(document.root_element().attribute("width"), Some("100%"));
    assert!(document.root_element().attribute("viewBox").is_none());
    assert!(result.metadata().svg_viewport().is_none());
    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
    assert_eq!(metadata["byte_length"], result.data().len());
    assert!(metadata.get("svg_viewport").is_none());
}

#[cfg(feature = "svg")]
#[test]
fn svg_byte_path_skips_serializing_metadata() {
    let source = b"flowchart TD\nA --> B";
    let engine = BindingEngine::new(b"").unwrap();

    reset_metadata_serialization_count();
    let result = engine
        .execute(BindingOperationRequest::new("svg", source))
        .unwrap();
    assert_eq!(metadata_serialization_count(), 1);

    reset_metadata_serialization_count();
    let bytes = engine.render_svg(source).unwrap();

    assert_eq!(metadata_serialization_count(), 0);
    assert_eq!(result.data(), bytes);
    assert!(result.metadata().theme_outcome().is_some());
}

#[cfg(feature = "svg")]
#[test]
fn generic_svg_plan_operation_reports_required_and_missing_capabilities() {
    let engine = BindingEngine::new(b"").unwrap();
    let result = engine
        .execute(BindingOperationRequest {
            operation_id: "svg-plan-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .unwrap();

    assert_eq!(result.operation.operation_id(), "svg-plan-json");
    assert_eq!(result.media_type, "application/json");
    let plan: serde_json::Value = serde_json::from_slice(&result.data).unwrap();
    assert_eq!(plan["planned_operation_id"], "svg");
    assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
    assert_eq!(plan["ready"], true);
    assert!(result.metadata().theme_outcome().is_none());
    let metadata: Value = serde_json::from_slice(result.metadata_json()).unwrap();
    assert!(metadata.get("theme_outcome").is_none());
}

#[cfg(feature = "svg")]
#[test]
fn layout_operation_omits_theme_outcome() {
    let result = execute_once(BindingOperationRequest::new(
        "layout-json",
        b"flowchart TD\nA --> B",
    ))
    .unwrap();
    let metadata: Value = serde_json::from_slice(result.metadata_json()).unwrap();

    assert!(result.metadata().theme_outcome().is_none());
    assert!(metadata.get("theme_outcome").is_none());
}

#[cfg(feature = "svg")]
#[test]
fn request_theme_inherits_clears_and_replaces_atomically() {
    let engine =
        BindingEngine::new(br#"{"theme":{"preset":"studio","appearance":"dark"}}"#).unwrap();
    let execute = |options: &[u8]| {
        engine
            .execute(
                BindingOperationRequest::new("svg", b"flowchart TD\nA --> B")
                    .with_options_json(options),
            )
            .unwrap()
    };

    let baseline = execute(b"");
    assert_eq!(
        baseline.metadata().theme_outcome().unwrap()["appearance"],
        "dark"
    );
    assert_eq!(execute(br#"{"version":4}"#), baseline);

    let inherited = execute(br#"{"svg":{"diagram_id":"request-inherit"}}"#);
    assert_eq!(
        inherited.metadata().theme_outcome(),
        baseline.metadata().theme_outcome()
    );

    let cleared = execute(br#"{"theme":null}"#);
    assert_eq!(
        cleared.metadata().theme_outcome().unwrap()["mode"],
        "mermaid_parity"
    );

    assert_eq!(execute(br#"{"theme":"none"}"#), cleared);

    let replaced = execute(br#"{"theme":{"document":{"schema_version":2,"extends":"paper"}}}"#);
    let outcome = replaced.metadata().theme_outcome().unwrap();
    assert_eq!(outcome["mode"], "merman_native");
    assert_eq!(outcome["appearance"], "light");
    assert!(outcome.get("preset_id").is_none());
    assert_eq!(execute(b""), baseline);
}

#[cfg(feature = "svg")]
#[test]
fn native_theme_outcome_reports_source_precedence() {
    let source = b"%%{init: {theme: 'dark'}}%%\nflowchart TD\nA --> B";
    let render = |options: &[u8]| {
        execute_once(BindingOperationRequest::new("svg", source).with_options_json(options))
            .unwrap()
    };
    let native = render(br#"{"theme":"studio"}"#);
    assert_eq!(
        native.metadata().theme_outcome().unwrap()["preset_id"],
        "studio"
    );
    assert_eq!(
        native.metadata().theme_outcome().unwrap()["mode"],
        "merman_native"
    );
    let parity = render(br#"{"theme":{"preset":"studio","precedence":"source"}}"#);
    assert_eq!(
        parity.metadata().theme_outcome().unwrap()["mode"],
        "mermaid_parity"
    );
    assert_eq!(parity.data(), render(b"").data());
}

#[cfg(feature = "svg")]
#[test]
fn output_background_applies_to_both_theme_modes() {
    for (options, mode) in [
        (
            br##"{"background":"#123456"}"##.as_slice(),
            "mermaid_parity",
        ),
        (
            br##"{"theme":"studio","background":"#123456"}"##.as_slice(),
            "merman_native",
        ),
    ] {
        let result = execute_once(
            BindingOperationRequest::new("svg", b"flowchart TD\nA --> B")
                .with_options_json(options),
        )
        .unwrap();
        assert!(
            std::str::from_utf8(result.data())
                .unwrap()
                .contains("#123456")
        );
        assert_eq!(result.metadata().theme_outcome().unwrap()["mode"], mode);
    }
    let source = b"flowchart TD\nA --> B";
    assert_eq!(
        crate::render_svg(source, br#"{"theme":"studio","background":"transparent"}"#).unwrap(),
        crate::render_svg(source, br##"{"theme":"studio","background":"#00000000"}"##).unwrap(),
    );
}

#[cfg(feature = "svg")]
#[test]
fn theme_operations_return_editable_documents_and_resolution_diagnostics() {
    let engine = BindingEngine::new(b"").unwrap();
    let document = engine.export_theme_preset(b"studio").unwrap();
    let exported: Value = serde_json::from_slice(&document).unwrap();
    assert_eq!(exported["schema_version"], 2);
    assert!(exported["appearances"]["light"].is_object());
    assert!(exported["appearances"]["dark"].is_object());

    let input =
        serde_json::to_vec(&serde_json::json!({"document": exported, "appearance": "dark"}))
            .unwrap();
    let resolved: Value = serde_json::from_slice(&engine.resolve_theme(&input).unwrap()).unwrap();
    assert_eq!(resolved["ok"], true);
    assert_eq!(resolved["resolved"]["appearance"], "dark");
    assert!(resolved["notices"].is_array());

    let invalid = br#"{"document":{"schema_version":2,"extends":"studio","extends":"paper"}}"#;
    let diagnostic: Value =
        serde_json::from_slice(&engine.resolve_theme(invalid).unwrap()).unwrap();
    assert_eq!(diagnostic["ok"], false);
    assert!(
        diagnostic["message"]
            .as_str()
            .unwrap()
            .contains("duplicate key")
    );
    assert!(diagnostic["line"].is_number());
    assert!(diagnostic["column"].is_number());
}

#[cfg(feature = "svg")]
#[test]
fn request_options_override_nested_engine_options_without_mutating_the_baseline() {
    let engine = BindingEngine::new(
        br#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "base engine", "pipeline": "readable" }
            }"#,
    )
    .unwrap();
    let request_result = engine
        .execute(BindingOperationRequest {
            operation_id: "svg",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"svg":{"diagram_id":"request override"}}"#,
            operation_control: None,
        })
        .unwrap();
    let request_svg = String::from_utf8(request_result.data).unwrap();
    assert!(request_svg.contains("id=\"request-override\""));
    assert!(request_svg.contains("data-merman-foreignobject"));

    let baseline_result = engine
        .execute(BindingOperationRequest {
            operation_id: "svg",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .unwrap();
    let baseline_svg = String::from_utf8(baseline_result.data).unwrap();
    assert!(baseline_svg.contains("id=\"base-engine\""));
}

#[cfg(feature = "svg")]
#[test]
fn elk_layout_request_follows_the_artifact_owner_feature() {
    let engine = BindingEngine::new(b"").unwrap();
    let result = engine.execute(BindingOperationRequest {
        operation_id: "svg",
        source: b"---\nconfig:\n  layout: elk\n---\nflowchart TD\n  A --> B\n",
        uri: None,
        options_json: b"",
        operation_control: None,
    });

    let (required, ready) = crate::svg_plan::expected_elk_request();
    if !ready {
        let error = result.expect_err("the selected layout requires an artifact capability");
        assert_eq!(error.capability_id(), Some(required));
        return;
    }
    let output = result.unwrap();
    assert_eq!(output.media_type, "image/svg+xml");
    if required == "layout-dagre" {
        let dagre = engine
            .render_svg(b"---\nconfig:\n  layout: dagre\n---\nflowchart TD\n  A --> B\n")
            .unwrap();
        assert_eq!(output.data(), dagre);
    }
}

#[cfg(feature = "png")]
#[test]
fn generic_png_operation_exposes_a_real_binary_output() {
    let engine = BindingEngine::new(b"").unwrap();
    let result = engine
        .execute(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .unwrap();

    assert_eq!(result.media_type, "image/png");
    assert!(result.data.starts_with(b"\x89PNG\r\n\x1a\n"));
    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
    assert_eq!(metadata["output_plan"]["kind"], "raster");
    assert_eq!(metadata["output_plan"]["limited"], false);
    assert_eq!(metadata["output_plan"]["requested_scale"], 1.0);
    assert_eq!(metadata["output_plan"]["effective_scale"], 1.0);
    assert_eq!(metadata["theme_outcome"]["mode"], "mermaid_parity");
}

#[cfg(feature = "jpeg")]
#[test]
fn generic_jpeg_operation_carries_target_theme_outcome() {
    let result = execute_once(BindingOperationRequest::new(
        "jpeg",
        b"flowchart TD\nA --> B",
    ))
    .unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();

    assert_eq!(result.media_type(), "image/jpeg");
    assert!(result.data().starts_with(&[0xff, 0xd8]));
    assert_eq!(metadata["output_plan"]["kind"], "raster");
    assert_eq!(metadata["theme_outcome"]["mode"], "mermaid_parity");
}

#[cfg(feature = "png")]
#[test]
fn png_byte_and_result_convenience_paths_share_one_envelope() {
    let source = b"flowchart TD\nA --> B";
    let result = crate::render_png_result(source, b"").unwrap();

    reset_metadata_serialization_count();
    let bytes = crate::render_png(source, b"").unwrap();

    assert_eq!(metadata_serialization_count(), 0);
    assert_eq!(result.operation().operation_id(), "png");
    assert_eq!(result.media_type(), "image/png");
    assert_eq!(result.data(), bytes);
    assert!(matches!(
        result.metadata().output_plan(),
        Some(BindingOutputPlan::Raster(_))
    ));
}

#[cfg(feature = "png")]
#[test]
fn generic_png_operation_reports_resource_limited_effective_plan() {
    let result = execute_once(BindingOperationRequest {
        operation_id: "png",
        source: b"flowchart TD\nA --> B",
        uri: None,
        options_json: br#"{
                "version": 4,
                "raster": {"scale": 20},
                "resources": {"limits": {"max_raster_pixels": 4096}}
            }"#,
        operation_control: None,
    })
    .unwrap();

    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
    let plan = &metadata["output_plan"];
    assert_eq!(plan["kind"], "raster");
    assert_eq!(plan["limited"], true);
    assert_eq!(plan["requested_scale"], 20.0);
    assert!(plan["effective_scale"].as_f64().unwrap() < plan["requested_scale"].as_f64().unwrap());
    assert!(plan["width_px"].as_u64().unwrap() * plan["height_px"].as_u64().unwrap() <= 4096);
}

#[cfg(feature = "png")]
#[test]
fn reusable_png_request_overlay_reports_its_effective_plan_without_mutating_the_engine() {
    let engine =
        BindingEngine::from_options(br#"{"version":4,"resources":{"profile":"trusted-native"}}"#)
            .unwrap();
    let limited = engine
        .execute(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{
                    "version": 4,
                    "raster": {"scale": 20},
                    "resources": {"limits": {"max_raster_pixels": 4096}}
                }"#,
            operation_control: None,
        })
        .unwrap();
    let limited_metadata: serde_json::Value =
        serde_json::from_slice(limited.metadata_json()).unwrap();
    assert_eq!(limited_metadata["output_plan"]["limited"], true);
    assert!(
        limited_metadata["output_plan"]["width_px"]
            .as_u64()
            .unwrap()
            * limited_metadata["output_plan"]["height_px"]
                .as_u64()
                .unwrap()
            <= 4096
    );

    let baseline = engine
        .execute(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .unwrap();
    let baseline_metadata: serde_json::Value =
        serde_json::from_slice(baseline.metadata_json()).unwrap();
    assert_eq!(baseline_metadata["output_plan"]["limited"], false);
    assert_eq!(baseline_metadata["output_plan"]["requested_scale"], 1.0);
}

#[cfg(feature = "pdf")]
#[test]
fn generic_pdf_operation_reports_effective_filter_plan() {
    let result = execute_once(BindingOperationRequest {
        operation_id: "pdf",
        source: b"flowchart TD\nA --> B",
        uri: None,
        options_json: br#"{"version":4,"pdf":{"filterScale":0.1}}"#,
        operation_control: None,
    })
    .unwrap();

    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
    let plan = &metadata["output_plan"];
    assert_eq!(plan["kind"], "pdf-filter-images");
    assert_eq!(plan["requested_scale"], serde_json::json!(0.1));
    assert_eq!(plan["effective_scale"], serde_json::json!(0.1));
    assert_eq!(metadata["theme_outcome"]["mode"], "mermaid_parity");
    assert!(
        std::str::from_utf8(result.metadata_json())
            .unwrap()
            .contains("\"requested_scale\":0.1"),
        "schema-1 metadata must preserve the historical f32 JSON representation"
    );
}

#[cfg(not(feature = "png"))]
#[test]
fn unavailable_operation_is_reported_before_execution() {
    let engine = BindingEngine::new(b"").unwrap();
    let error = engine
        .execute(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
            operation_control: None,
        })
        .expect_err("PNG is not compiled");

    assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
    assert_eq!(error.capability_id(), Some("png"));
}
