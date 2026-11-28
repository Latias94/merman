#[cfg(feature = "transport-napi")]
use super::direct_svg::{
    direct_svg_id_fits, direct_svg_response, prepare_direct_svg_id_fallback,
    validate_direct_svg_framing,
};
#[cfg(feature = "transport-napi")]
use super::{
    DirectSvgOptions, SuccessEnvelope, execute_svg_with_admitted_control,
    parse_operation_request_framing, success_data_envelope,
};
use super::{
    admitted_operation_control, create_engine, create_engine_with_icon_packs, error_envelope,
    execute_wire, execute_wire_with_admitted_control, execute_wire_with_control,
    metadata_wire_for_engine, node_artifact_contract, node_wire_contract, runtime_catalog_wire,
};

#[cfg(feature = "math")]
#[test]
fn math_feature_renders_formula_geometry_through_the_owned_transport_contract() {
    let engine = create_engine("{}").unwrap();
    let request = serde_json::json!({
        "operation_id": "svg",
        "source": "flowchart LR\nA[\"$$x^2$$\"]",
        "options_json": "{\"site_config\":{\"htmlLabels\":true}}"
    });
    let response: serde_json::Value =
        serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
    assert_eq!(response["ok"], true);
    let svg = response["result"]["data"].as_str().unwrap();
    assert!(svg.matches("<svg").count() > 1);
    assert!(svg.contains("<path"));
    assert!(!svg.contains("$$x^2$$"));
}

#[test]
fn loaded_icon_packs_render_with_request_overlays_and_isolated_engines() {
    let packs = serde_json::json!([{
        "json": r#"{"prefix":"original","icons":{"rocket":{"body":"<path data-icon=\"node-registry\" d=\"M0 0H16V16H0z\"/>"}}}"#,
        "registrationName": "fleet",
    }])
    .to_string();
    let registered = create_engine_with_icon_packs("", Some(&packs)).unwrap();
    drop(packs);
    let plain = create_engine("").unwrap();
    let empty = create_engine_with_icon_packs("", Some("[]")).unwrap();
    let request = serde_json::json!({
        "operation_id": "svg",
        "source": "flowchart TD\nA@{ icon: \"fleet:rocket\", label: \"A\" }",
        "options_json": r#"{"svg":{"diagram_id":"node-icon-overlay"}}"#,
    })
    .to_string();
    let render = |engine: &merman_bindings_core::BindingEngine| {
        let result: serde_json::Value =
            serde_json::from_str(&execute_wire(engine, &request)).unwrap();
        assert_eq!(result["ok"], true, "{result}");
        result["result"]["data"].as_str().unwrap().to_owned()
    };
    let plain_svg = render(&plain);
    assert_eq!(render(&empty), plain_svg);
    assert!(!plain_svg.contains("node-registry"));
    assert!(render(&registered).contains("node-registry"));
    drop(registered);
    assert_eq!(render(&plain), plain_svg);
}

#[test]
fn icon_constructor_preserves_typed_content_and_resource_errors() {
    for (json, name, code, kind) in [
        (
            r#"{"prefix":"test","icons":{"bad":{"body":"<path>"}}}"#,
            "fleet".to_owned(),
            "MERMAN_INVALID_ARGUMENT",
            "invalid_xml",
        ),
        (
            r#"{"icons":{}}"#,
            "a".repeat(65),
            "MERMAN_RESOURCE_LIMIT_EXCEEDED",
            "resource_limit_exceeded",
        ),
    ] {
        let packs = serde_json::json!([{"json": json, "registrationName": name}]).to_string();
        let error = create_engine_with_icon_packs("", Some(&packs))
            .err()
            .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&error_envelope(&error)).unwrap();
        assert_eq!(envelope["error"]["code_name"], code);
        assert_eq!(
            envelope["error"]["details"]["icon_registry"]["kind_id"],
            kind
        );
        assert_eq!(
            envelope["error"]["details"]["icon_registry"]["pack_index"],
            0
        );
        if code == "MERMAN_RESOURCE_LIMIT_EXCEEDED" {
            assert_eq!(
                envelope["error"]["details"]["resource"]["profile"],
                "constructor-fixed"
            );
            assert_eq!(
                envelope["error"]["details"]["resource"]["limit_id"],
                "max_icon_registry_prefix_bytes"
            );
        }
    }
}

#[cfg(feature = "transport-napi")]
#[test]
fn direct_svg_input_matches_the_escaped_wire_boundary() {
    let options = r#"{"svg":{"diagram_id":"unicode-🧪"}}"#;
    let prefix = "\"\\\t\n\r\u{8}\u{c}\u{1f}é🧪\u{2028}\u{2029}/";
    let mut request = serde_json::json!({
        "operation_id": "svg",
        "source": prefix,
        "uri": null,
        "include_metadata": false,
        "options_json": options,
        "operation_control": {"timeout_ms": u32::MAX},
    });
    let max = node_wire_contract().documents.request.max_utf8_bytes;
    let remaining = max - request.to_string().len();
    let mut source = format!(
        "{prefix}{}{}",
        "\0".repeat(remaining / 6),
        "x".repeat(remaining % 6)
    );
    request["source"] = serde_json::json!(source);
    let exact = request.to_string();
    assert_eq!(exact.len(), max);
    assert!(parse_operation_request_framing(&exact).is_ok());
    assert!(validate_direct_svg_framing(&source, Some(options), Some(u32::MAX)).is_ok());

    source.push('x');
    request["source"] = serde_json::json!(source);
    let legacy = parse_operation_request_framing(&request.to_string()).unwrap_err();
    let direct = validate_direct_svg_framing(&source, Some(options), Some(u32::MAX)).unwrap_err();
    assert_eq!(direct.status(), legacy.status());
    assert_eq!(direct.message(), legacy.message());
    assert!(direct.message().contains("wire limit"));
}

#[cfg(feature = "transport-napi")]
#[test]
fn direct_svg_response_matches_escaped_envelope_and_field_limits() {
    use merman_bindings_core::OperationKey;

    let contract = node_wire_contract();
    let operation = OperationKey::Svg;
    let mut escaped = String::from("\"\\\t\n\r\u{8}\u{c}\u{1f}é🧪\u{2028}\u{2029}/");
    let initial_wire =
        serde_json::to_string(&SuccessEnvelope::new(operation, &escaped, None)).unwrap();
    assert_eq!(
        direct_svg_response(escaped.as_bytes().to_vec()).unwrap(),
        escaped
    );
    let max_wire_bytes = contract.documents.response.max_utf8_bytes;
    let remaining = max_wire_bytes - initial_wire.len();
    escaped.push_str(&"\0".repeat(remaining / 6));
    escaped.push_str(&"x".repeat(remaining % 6));
    assert_eq!(
        direct_svg_response(escaped.as_bytes().to_vec()).unwrap(),
        escaped
    );
    assert_eq!(
        success_data_envelope(operation, escaped.as_bytes().to_vec(), None)
            .unwrap()
            .len(),
        max_wire_bytes
    );

    let max_field_bytes = contract.fields.data_utf8_bytes;
    let exact_field = vec![b'x'; max_field_bytes];
    assert_eq!(
        direct_svg_response(exact_field).unwrap().len(),
        max_field_bytes
    );

    escaped.push('x');
    for (data, expected) in [
        (escaped.into_bytes(), "wire limit"),
        // This exceeds both ceilings. The decoded field keeps priority.
        (vec![0; max_field_bytes + 1], "field limit"),
        (vec![0xff], "non-UTF-8"),
    ] {
        let direct = direct_svg_response(data.clone()).unwrap_err();
        let legacy = success_data_envelope(operation, data, None).unwrap_err();
        assert_eq!(direct.status(), legacy.status());
        assert_eq!(direct.message(), legacy.message());
        assert!(direct.message().contains(expected), "{direct:?}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_panic_boundary_returns_the_stable_panic_envelope() {
    let error = super::binding_boundary(|| -> Result<(), merman_bindings_core::BindingError> {
        panic!("synthetic Node transport panic")
    })
    .expect_err("native panic must become a binding error");
    assert_eq!(error.status(), merman_bindings_core::BindingStatus::Panic);
    let envelope: serde_json::Value =
        serde_json::from_str(&error_envelope(&error)).expect("panic envelope");
    assert_eq!(envelope["error"]["code"], 8);
    assert_eq!(envelope["error"]["code_name"], "MERMAN_PANIC");
}

#[test]
fn runtime_catalog_metadata_ids_match_the_contract_dispatcher() {
    let catalog: serde_json::Value =
        serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
    let advertised_ids = catalog["metadata_ids"]
        .as_array()
        .expect("Node runtime metadata IDs")
        .iter()
        .map(|id| id.as_str().expect("metadata ID string"))
        .collect::<Vec<_>>();
    let expected_ids = node_artifact_contract()
        .metadata_keys()
        .map(merman_bindings_core::MetadataKey::id)
        .collect::<Vec<_>>();
    assert_eq!(advertised_ids, expected_ids);

    let engine = create_engine("").expect("Node engine");
    for id in advertised_ids {
        let payload: serde_json::Value = serde_json::from_str(
            &metadata_wire_for_engine(&engine, id).unwrap_or_else(|error| {
                panic!("advertised Node metadata `{id}` failed: {error:?}")
            }),
        )
        .unwrap_or_else(|error| panic!("Node metadata `{id}` was not JSON: {error}"));
        assert!(!payload.is_null(), "Node metadata `{id}` returned null");
    }

    for key in merman_bindings_core::MetadataKey::ALL {
        if !expected_ids.contains(&key.id()) {
            let error = match metadata_wire_for_engine(&engine, key.id()) {
                Ok(_) => panic!("unadvertised metadata `{}` succeeded", key.id()),
                Err(error) => error,
            };
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::UnsupportedOperation
            );
        }
    }
}

#[cfg(feature = "svg")]
#[test]
fn data_only_wire_omits_metadata_and_preserves_result_bytes_and_errors() {
    let engine = create_engine("").unwrap();
    let source = "flowchart LR\r\nA[\"北京😀\"] --> B[\"Café\"]\r\n%% quote: \"\\\t\r\n";
    let options = r#"{"site_config":{"htmlLabels":false},"svg":{"diagram_id":"direct-svg"}}"#;
    for operation in ["svg", "semantic-json"] {
        let expected = engine
            .execute(
                merman_bindings_core::BindingOperationRequest::new(operation, source.as_bytes())
                    .with_options_json(options.as_bytes()),
            )
            .unwrap();
        let mut request = serde_json::json!({
            "operation_id": operation,
            "source": source,
            "uri": null,
            "options_json": options,
        });
        let full: serde_json::Value =
            serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
        assert_eq!(full["ok"], true);
        assert_eq!(
            full["result"]["data"].as_str().unwrap().as_bytes(),
            expected.data(),
        );
        assert_eq!(
            full["result"]["metadata_json"].as_str().unwrap().as_bytes(),
            expected.metadata_json(),
        );
        request["include_metadata"] = false.into();
        let data: serde_json::Value =
            serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
        assert_eq!(data["ok"], true);
        assert_eq!(
            data["result"]["operation_id"],
            full["result"]["operation_id"]
        );
        assert_eq!(data["result"]["media_type"], full["result"]["media_type"]);
        assert_eq!(data["result"]["data"], full["result"]["data"]);
        assert!(data["result"].get("metadata_json").is_none());

        #[cfg(feature = "transport-napi")]
        if operation == "svg" {
            let direct = execute_svg_with_admitted_control(
                &engine,
                source,
                DirectSvgOptions::Json(Some(options)),
                admitted_operation_control(None),
                None,
            )
            .unwrap();
            assert_eq!(direct, data["result"]["data"].as_str().unwrap());
        }
    }
    for (operation, options) in [
        ("unknown-operation", ""),
        ("svg", "{"),
        ("svg", r#"{"version":99}"#),
    ] {
        let mut request = serde_json::json!({
            "operation_id": operation,
            "source": "flowchart TD\nA --> B",
            "uri": null,
            "options_json": options,
        });
        let full: serde_json::Value =
            serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
        assert_eq!(full["ok"], false);
        request["include_metadata"] = false.into();
        let data: serde_json::Value =
            serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
        assert_eq!(data, full);

        #[cfg(feature = "transport-napi")]
        if operation == "svg" {
            let error = execute_svg_with_admitted_control(
                &engine,
                request["source"].as_str().unwrap(),
                DirectSvgOptions::Json(Some(options)),
                admitted_operation_control(None),
                None,
            )
            .unwrap_err();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&error_envelope(&error)).unwrap(),
                data
            );
        }
    }
}

#[test]
fn decoded_request_options_keep_duplicate_keys_for_theme_diagnostics() {
    let engine = create_engine("").unwrap();
    let source = "flowchart LR\nA --> B";
    let options = "{\n  \"theme\": {\n    \"schema_version\": 2,\n    \"extends\": \"studio\", \"extends\": \"paper-cut\"\n  }\n}";
    let raw = merman_bindings_core::BindingOperationRequest::new("svg", source.as_bytes())
        .with_options_json(options.as_bytes());
    let expected = engine.execute_data(raw).unwrap_err();
    assert!(expected.message().contains("duplicate key"));
    let envelope = serde_json::json!({
        "operation_id": "svg",
        "source": source,
        "options_json": options,
        "include_metadata": false,
    });
    assert_eq!(
        execute_wire(&engine, &envelope.to_string()),
        error_envelope(&expected)
    );
    #[cfg(feature = "transport-napi")]
    assert_eq!(
        execute_svg_with_admitted_control(
            &engine,
            source,
            DirectSvgOptions::Json(Some(options)),
            admitted_operation_control(None),
            None,
        )
        .unwrap_err(),
        expected,
    );
}

#[cfg(feature = "svg")]
#[test]
fn theme_catalog_presets_export_through_the_node_transport() {
    let engine = create_engine(r#"{"resources":{"profile":"constrained"}}"#)
        .expect("constrained Node engine");
    let catalog: serde_json::Value = serde_json::from_str(
        &metadata_wire_for_engine(&engine, "theme-catalog").expect("engine theme catalog"),
    )
    .expect("Node theme catalog JSON");
    assert_eq!(catalog["schema_version"], 2);
    let presets = catalog["presets"].as_array().expect("preset catalog");
    assert!(!presets.is_empty());
    for preset in presets {
        let request = serde_json::json!({
            "operation_id": "export-theme-preset",
            "source": preset["id"], "uri": null,
        });
        let result: serde_json::Value =
            serde_json::from_str(&execute_wire(&engine, &request.to_string()))
                .expect("Node preset export envelope");
        assert_eq!(result["ok"], true, "{}: {result}", preset["id"]);
        assert_eq!(result["result"]["media_type"], "application/json");
        let document: serde_json::Value =
            serde_json::from_str(result["result"]["data"].as_str().unwrap()).unwrap();
        assert_eq!(document["schema_version"], 2);
        assert_eq!(document["id"], preset["id"]);
    }
}

#[test]
fn static_svg_transport_preserves_missing_and_unknown_operation_errors() {
    let native = match create_engine(r#"{"runtime_policy":"native"}"#) {
        Ok(_) => panic!("Node static-SVG transport accepted native runtime policy"),
        Err(error) => error,
    };
    assert_eq!(
        native.status(),
        merman_bindings_core::BindingStatus::OptionsJsonError
    );
    assert_eq!(
        native.kind(),
        merman_bindings_core::BindingErrorKind::Generic
    );
    assert_eq!(native.capability_id(), None);
    assert!(native.message().contains("not exposed by target"));

    let engine = create_engine("").unwrap();
    let missing: serde_json::Value = serde_json::from_str(&execute_wire(
        &engine,
        r#"{"operation_id":"png","source":"flowchart TD\nA-->B","uri":null}"#,
    ))
    .expect("Node error envelope");
    assert_eq!(
        missing["error"]["code_name"],
        "MERMAN_UNSUPPORTED_OPERATION"
    );
    assert_eq!(missing["error"]["kind"], "missing-capability");
    assert_eq!(missing["error"]["capability_id"], "png");
    assert!(
        missing["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("not exposed by target"))
    );

    let unknown: serde_json::Value = serde_json::from_str(&execute_wire(
        &engine,
        r#"{"operation_id":"bitmap","source":"flowchart TD\nA-->B","uri":null}"#,
    ))
    .expect("Node error envelope");
    assert_eq!(unknown["error"]["kind"], "unknown-operation");
    assert!(unknown["error"]["capability_id"].is_null());
}

#[test]
fn error_wire_preserves_structured_resource_details() {
    let error = merman_bindings_core::BindingError::resource_limit(
        "embedded_image_decode",
        "max_embedded_image_bytes",
        5,
        4,
        "constrained",
        "embedded image is too large",
    );
    let payload: serde_json::Value =
        serde_json::from_str(&error_envelope(&error)).expect("Node error envelope");

    assert_eq!(
        payload["error"]["details"]["resource"]["limit_id"],
        "max_embedded_image_bytes"
    );
    assert_eq!(
        payload["error"]["details"]["resource"]["profile"],
        "constrained"
    );
    assert_eq!(payload["error"]["details"]["resource"]["cause"], "ceiling");
    assert_eq!(payload["error"]["details"]["resource"]["actual"], 5);
    assert_eq!(payload["error"]["details"]["resource"]["max"], 4);

    let error = merman_bindings_core::BindingError::resource_limit_with_cause(
        merman_bindings_core::BindingResourceLimitCause::ArithmeticOverflow,
        "layout_model",
        "max_layout_work_units",
        u64::MAX,
        800_000,
        "interactive",
        "layout work accounting overflowed",
    );
    let payload: serde_json::Value =
        serde_json::from_str(&error_envelope(&error)).expect("Node error envelope");
    assert_eq!(
        payload["error"]["code_name"],
        "MERMAN_RESOURCE_LIMIT_EXCEEDED"
    );
    assert_eq!(
        payload["error"]["details"]["resource"]["cause"],
        "arithmetic_overflow"
    );
    assert_eq!(
        payload["error"]["details"]["resource"]["limit_id"],
        "max_layout_work_units"
    );
    assert_eq!(
        payload["error"]["details"]["resource"]["phase"],
        "layout_model"
    );
    assert_eq!(
        payload["error"]["details"]["resource"]["actual"],
        "18446744073709551615"
    );
    assert_eq!(payload["error"]["details"]["resource"]["max"], 800_000);
    assert_eq!(
        payload["error"]["details"]["resource"]["profile"],
        "interactive"
    );
}

#[cfg(feature = "svg")]
#[test]
fn resolve_theme_wire_preserves_document_diagnostics() {
    let engine = create_engine("").unwrap();
    let document =
        "{\n  \"schema_version\": 2,\n  \"schema_version\": 2,\n  \"extends\": \"studio\"\n}";
    let request = serde_json::json!({
        "operation_id": "resolve-theme",
        "source": serde_json::json!({ "document": document }).to_string(),
        "include_metadata": false,
    });
    let envelope: serde_json::Value =
        serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
    assert_eq!(envelope["ok"], true, "{envelope}");
    let result: serde_json::Value =
        serde_json::from_str(envelope["result"]["data"].as_str().unwrap()).unwrap();
    assert_eq!(result["ok"], false);
    assert_eq!(result["line"], 3);
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("duplicate key `schema_version`")
    );
}

#[test]
fn error_wire_preserves_structured_diagnostic_details() {
    use merman_bindings_core::{
        BindingDiagnosticErrorDetails, BindingDiagnosticSpan, BindingError, BindingStatus,
    };

    let error = BindingError::new(BindingStatus::ParseError, "invalid edge")
        .with_diagnostic_details(
            BindingDiagnosticErrorDetails::new("flowchart.edge.invalid")
                .with_span(BindingDiagnosticSpan::new(3, 8, "exact"))
                .with_field("edge")
                .with_diagram_type("flowchart"),
        );
    let payload: serde_json::Value =
        serde_json::from_str(&error_envelope(&error)).expect("Node error envelope");

    let diagnostic = &payload["error"]["details"]["diagnostic"];
    assert_eq!(diagnostic["code"], "flowchart.edge.invalid");
    assert_eq!(diagnostic["span"]["start"], 3);
    assert_eq!(diagnostic["span"]["end"], 8);
    assert_eq!(diagnostic["span"]["kind"], "exact");
    assert_eq!(diagnostic["field"], "edge");
    assert_eq!(diagnostic["diagram_type"], "flowchart");
}

#[test]
fn operation_control_preserves_requested_and_deadline_cancellation_details() {
    let engine = create_engine("").unwrap();
    let deadline: serde_json::Value = serde_json::from_str(
        &execute_wire_with_admitted_control(
            &engine,
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":0}}"#,
            admitted_operation_control(Some(0)),
            Some(0),
        ),
    )
    .expect("deadline error envelope");
    assert_eq!(deadline["error"]["code_name"], "MERMAN_CANCELLED");
    assert_eq!(
        deadline["error"]["details"]["cancellation"]["reason"],
        "deadline_exceeded"
    );
    assert_eq!(
        deadline["error"]["details"]["cancellation"]["phase"],
        "admission"
    );

    let requested = merman_bindings_core::OperationControl::new();
    requested.cancel();
    let cancelled: serde_json::Value = serde_json::from_str(&execute_wire_with_control(
        &engine,
        r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null}"#,
        requested,
    ))
    .expect("requested cancellation envelope");
    assert_eq!(cancelled["error"]["code_name"], "MERMAN_CANCELLED");
    assert_eq!(
        cancelled["error"]["details"]["cancellation"]["reason"],
        "requested"
    );
    assert_eq!(
        cancelled["error"]["details"]["cancellation"]["phase"],
        "admission"
    );
    assert!(cancelled["error"]["details"].get("resource").is_none());
}

#[test]
fn cancelled_operation_preserves_canonical_mixed_error_precedence() {
    let engine = create_engine("").unwrap();
    let cases = [
        (
            r#"{"operation_id":"unknown-operation","source":"flowchart TD\nA-->B","uri":null,"options_json":"{"}"#,
            "MERMAN_UNSUPPORTED_OPERATION",
            "unknown-operation",
            Some("unknown operation `unknown-operation`"),
        ),
        (
            r#"{"operation_id":"document-analysis-json","source":"flowchart TD\nA-->B","uri":null,"options_json":"{"}"#,
            "MERMAN_INVALID_ARGUMENT",
            "generic",
            Some("operation `document-analysis-json` requires a document URI"),
        ),
        (
            r#"{"operation_id":"document-analysis-json","source":"flowchart TD\nA-->B","uri":"","options_json":"{"}"#,
            "MERMAN_INVALID_ARGUMENT",
            "generic",
            Some("operation `document-analysis-json` requires a document URI"),
        ),
        (
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":"file:///diagram.mmd","options_json":"{"}"#,
            "MERMAN_INVALID_ARGUMENT",
            "generic",
            Some("operation `semantic-json` does not accept a document URI"),
        ),
        (
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":"","options_json":"{"}"#,
            "MERMAN_CANCELLED",
            "generic",
            None,
        ),
        (
            r#"{"operation_id":"svg","source":"flowchart TD\nA-->B","uri":null,"include_metadata":false,"options_json":"{"}"#,
            "MERMAN_CANCELLED",
            "generic",
            None,
        ),
    ];

    for (request, expected_code_name, expected_kind, expected_message) in cases {
        let control = merman_bindings_core::OperationControl::new();
        control.cancel();
        let response: serde_json::Value = serde_json::from_str(&execute_wire_with_control(
            &engine,
            request,
            control.clone(),
        ))
        .expect("mixed-error response");

        assert_eq!(response["error"]["code_name"], expected_code_name);
        assert_eq!(response["error"]["kind"], expected_kind);
        if expected_code_name == "MERMAN_CANCELLED" {
            assert_eq!(
                response["error"]["details"]["cancellation"]["phase"],
                "admission"
            );
        } else {
            assert_eq!(response["error"]["message"], expected_message.unwrap());
            assert!(response["error"].get("details").is_none());
        }
    }
}

#[cfg(feature = "transport-napi")]
#[test]
fn direct_svg_deadline_precedes_nested_option_validation() {
    let engine = create_engine("").unwrap();
    let source = "flowchart TD\nA-->B";
    let request = r#"{"operation_id":"svg","source":"flowchart TD\nA-->B","uri":null,"include_metadata":false,"options_json":"{","operation_control":{"timeout_ms":0}}"#;
    let legacy = execute_wire_with_admitted_control(
        &engine,
        request,
        admitted_operation_control(Some(0)),
        Some(0),
    );
    let direct = execute_svg_with_admitted_control(
        &engine,
        source,
        DirectSvgOptions::Json(Some("{")),
        admitted_operation_control(Some(0)),
        Some(0),
    )
    .unwrap_err();
    assert_eq!(error_envelope(&direct), legacy);
    assert_eq!(
        direct.status(),
        merman_bindings_core::BindingStatus::Cancelled
    );
}

#[cfg(feature = "transport-napi")]
#[test]
fn typed_svg_ids_preserve_json_output_and_deadlines() {
    let engine = create_engine(r#"{"site_config":{"layout":"dagre"}}"#).unwrap();
    let source = "flowchart LR\nA[Hello] --> B[World]";
    for id in ["", "one:部署 😀\"\\\n"] {
        let json = serde_json::json!({"svg": {"diagram_id": id}}).to_string();
        for timeout in [None, Some(0)] {
            let render = |options| {
                execute_svg_with_admitted_control(
                    &engine,
                    source,
                    options,
                    admitted_operation_control(timeout),
                    timeout,
                )
            };
            let expected = render(DirectSvgOptions::Json(Some(&json)));
            if timeout.is_none() {
                assert!(expected.as_ref().is_ok_and(|svg| svg.starts_with("<svg")));
            }
            assert_eq!(render(DirectSvgOptions::DiagramId(id)), expected);
        }
    }
}

#[cfg(feature = "transport-napi")]
#[test]
fn typed_svg_id_fallback_matches_json_at_the_options_limit() {
    let source = "flowchart LR\nA --> B";
    let prefix = "\"\\\t\n\r\u{8}\u{c}\u{1f}é🧪\u{2028}\u{2029}/";
    let options_json = |id: &str| serde_json::json!({"svg": {"diagram_id": id}}).to_string();
    let max = node_wire_contract().fields.options_json_utf8_bytes;
    let remaining = max - options_json(prefix).len();
    let mut id = format!(
        "{prefix}{}{}",
        "\0".repeat(remaining / 6),
        "x".repeat(remaining % 6)
    );
    let expected = options_json(&id);
    assert_eq!(expected.len(), max);
    assert!(!direct_svg_id_fits(source, &id, None));

    let control = admitted_operation_control(None);
    assert_eq!(
        prepare_direct_svg_id_fallback(source, &id, &control, None).unwrap(),
        expected
    );
    id.push('x');
    let error = prepare_direct_svg_id_fallback(source, &id, &control, None).unwrap_err();
    assert!(error.message().contains("options_json exceeds"));
}

#[cfg(feature = "transport-napi")]
#[test]
fn typed_svg_id_fallback_preserves_admission_error_precedence() {
    let engine = create_engine("").unwrap();
    let contract = node_wire_contract();
    let id = "\0".repeat(contract.fields.options_json_utf8_bytes / 6 + 1);
    let options = serde_json::json!({"svg": {"diagram_id": &id}}).to_string();
    let timeout = Some(u32::MAX);
    let empty_source_bytes = serde_json::json!({
        "operation_id": "svg",
        "source": "",
        "uri": null,
        "options_json": &options,
        "include_metadata": false,
        "operation_control": {"timeout_ms": u32::MAX},
    })
    .to_string()
    .len();
    let remaining = contract.documents.request.max_utf8_bytes - empty_source_bytes;
    let mut source = format!(
        "{}{}",
        "\0".repeat(remaining / 6),
        "x".repeat(remaining % 6)
    );
    assert!(!direct_svg_id_fits(&source, &id, timeout));

    let reject = |source: &str, cancelled| {
        let control = admitted_operation_control(timeout);
        if cancelled {
            control.cancel();
        }
        let run = |options| {
            execute_svg_with_admitted_control(&engine, source, options, control.clone(), timeout)
                .unwrap_err()
        };
        let error = run(DirectSvgOptions::DiagramId(&id));
        assert_eq!(error, run(DirectSvgOptions::Json(Some(&options))));
        error
    };
    assert!(
        reject(&source, false)
            .message()
            .contains("options_json exceeds")
    );
    let cancelled = reject(&source, true);
    assert_eq!(cancelled.cancellation_details().unwrap().phase, "admission");
    source.push('x');
    assert!(reject(&source, true).message().contains("wire limit"));
}

#[test]
fn admitted_timeout_must_match_the_wire_request_after_deadline_priority() {
    let engine = create_engine("").unwrap();
    let matching: serde_json::Value = serde_json::from_str(
        &execute_wire_with_admitted_control(
            &engine,
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":"","operation_control":{"timeout_ms":60000}}"#,
            admitted_operation_control(Some(60_000)),
            Some(60_000),
        ),
    )
    .expect("matching timeout response");
    assert_eq!(matching["ok"], true);

    let request_only: serde_json::Value = serde_json::from_str(&execute_wire(
        &engine,
        r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":60000}}"#,
    ))
    .expect("request-only timeout response");
    assert_eq!(
        request_only["error"]["code_name"],
        "MERMAN_INVALID_ARGUMENT"
    );

    for (request, admitted) in [
        (
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":60000}}"#,
            59_999,
        ),
        (
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null}"#,
            60_000,
        ),
    ] {
        let rejected: serde_json::Value =
            serde_json::from_str(&execute_wire_with_admitted_control(
                &engine,
                request,
                admitted_operation_control(Some(admitted)),
                Some(admitted),
            ))
            .expect("timeout mismatch response");
        assert_eq!(rejected["error"]["code_name"], "MERMAN_INVALID_ARGUMENT");
    }

    let expired: serde_json::Value = serde_json::from_str(
        &execute_wire_with_admitted_control(
            &engine,
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":1}}"#,
            admitted_operation_control(Some(0)),
            Some(0),
        ),
    )
    .expect("expired timeout response");
    assert_eq!(expired["error"]["code_name"], "MERMAN_CANCELLED");
    assert_eq!(
        expired["error"]["details"]["cancellation"]["reason"],
        "deadline_exceeded"
    );
    assert_eq!(
        expired["error"]["details"]["cancellation"]["phase"],
        "admission"
    );
}

#[test]
fn disposed_error_is_a_stable_bounded_error_envelope() {
    let first = error_envelope(&super::disposed_error());
    let second = error_envelope(&super::disposed_error());
    assert_eq!(first, second);
    let payload: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code_name"], "MERMAN_INVALID_ARGUMENT");
    assert_eq!(
        payload["error"]["message"],
        "Node transport engine has been disposed"
    );
}
