use super::*;
#[cfg(feature = "ascii")]
use merman_ascii_test_contracts::ascii_resource_boundary_contract;
#[cfg(any(feature = "analysis", feature = "ascii"))]
use serde_json::Value;

#[cfg(feature = "ascii")]
fn wasm_ascii_options(limit_id: &str, value: u64) -> String {
    format!(r#"{{"resources":{{"limits":{{"{limit_id}":{value}}}}}}}"#)
}

#[cfg(feature = "ascii")]
fn assert_wasm_ascii_resource_error(
    error: BindingError,
    expected_limit_id: &str,
    expected_phase: &str,
    expected_max: u64,
) -> u64 {
    assert_eq!(
        error.status(),
        merman_bindings_core::BindingStatus::ResourceLimitExceeded
    );
    assert_eq!(error.capability_id(), None);
    let details = error
        .resource_details()
        .expect("WASM ASCII resource errors must expose typed details");
    assert_eq!(details.limit_id, expected_limit_id);
    assert_eq!(details.phase, expected_phase);
    assert_eq!(details.max, expected_max);
    assert_eq!(details.profile, "interactive");
    assert!(details.actual > details.max);

    let payload = binding_error_payload_value(&error).expect("structured WASM error payload");
    let resource = payload["details"]["resource"]
        .as_object()
        .expect("WASM resource details object");
    assert_eq!(resource.len(), 6, "WASM resource error shape changed");
    assert_eq!(resource["cause"], "ceiling");
    assert_eq!(resource["limit_id"], expected_limit_id);
    assert_eq!(resource["phase"], expected_phase);
    assert_eq!(resource["actual"], details.actual);
    assert_eq!(resource["max"], expected_max);
    assert_eq!(resource["profile"], "interactive");
    details.actual
}

#[test]
fn package_version_matches_crate_version() {
    assert_eq!(package_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn transport_api_version_is_independent_from_host_measurement_protocol() {
    assert_eq!(transport_api_version(), WASM_TRANSPORT_API_VERSION);
    assert_eq!(WASM_TRANSPORT_API_VERSION, 12);
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_metadata_exposes_the_total_capability_contract() {
    let bytes = wasm_artifact_contract()
        .metadata_json("ascii-capabilities")
        .unwrap();
    let capabilities: Value = serde_json::from_slice(&bytes).unwrap();
    let capabilities = capabilities.as_array().unwrap();

    assert_eq!(capabilities.len(), 34);
    let flowchart = capabilities
        .iter()
        .find(|capability| capability["diagram_type"] == "flowchart")
        .unwrap();
    assert_eq!(flowchart["semantic_coverage"], "partial");
    assert_eq!(flowchart["primary_projection"], "diagrammatic");
    assert_eq!(flowchart["support_level"], "partial");
    assert_eq!(
        flowchart["graph_presentations"],
        serde_json::json!(["strict", "adaptive"])
    );
    assert!(flowchart.get("summary_fallback").is_none());

    let state = capabilities
        .iter()
        .find(|capability| capability["diagram_type"] == "state")
        .unwrap();
    assert_eq!(
        state["graph_presentations"],
        serde_json::json!(["strict", "adaptive"])
    );
    assert_eq!(state["layout_profiles"], serde_json::json!(["canonical"]));

    let class = capabilities
        .iter()
        .find(|capability| capability["diagram_type"] == "class")
        .unwrap();
    assert_eq!(class["graph_presentations"], serde_json::json!(["strict"]));

    #[cfg(feature = "diagram-zenuml")]
    {
        let zenuml = capabilities
            .iter()
            .find(|capability| capability["diagram_type"] == "zenuml")
            .unwrap();
        assert_eq!(zenuml["semantic_coverage"], "partial");
        assert_eq!(zenuml["primary_projection"], "diagrammatic");
        assert_eq!(zenuml["support_level"], "partial");
        assert_eq!(zenuml["layout_profiles"], serde_json::json!(["canonical"]));
        assert_eq!(zenuml["graph_presentations"], serde_json::json!(["strict"]));
        assert_eq!(zenuml["fallback_encodings"], serde_json::json!(["plain"]));
    }

    let radar = capabilities
        .iter()
        .find(|capability| capability["diagram_type"] == "radar")
        .unwrap();
    assert!(radar["semantic_coverage"].is_null());
    assert_eq!(radar["primary_projection"], "none");
    assert_eq!(radar["support_level"], "unsupported");
    assert_eq!(radar["graph_presentations"], serde_json::json!([]));
}

#[test]
fn wasm_timeout_option_becomes_a_relative_deadline_and_is_removed_from_shared_options() {
    let (options, timeout) = wasm_options(
        format!(
            r#"{{"version":{},"timeout_ms":125,"resources":{{"profile":"constrained"}}}}"#,
            merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION
        )
        .as_bytes(),
    )
    .unwrap();

    assert_eq!(timeout, Some(Duration::from_millis(125)));
    let value: serde_json::Value = serde_json::from_slice(&options).unwrap();
    assert_eq!(
        value["version"],
        merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION
    );
    assert_eq!(value["resources"]["profile"], "constrained");
    assert!(value.get("timeout_ms").is_none());
}

#[test]
fn wasm_timeout_option_rejects_non_integer_values() {
    for options in [
        br#"{"timeout_ms":-1}"#.as_slice(),
        br#"{"timeout_ms":1.5}"#.as_slice(),
        br#"{"timeout_ms":"10"}"#.as_slice(),
        br#"{"timeout_ms":true}"#.as_slice(),
        br#"{"timeout_ms":4294967296}"#.as_slice(),
    ] {
        let error = wasm_options(options).expect_err("invalid timeout_ms must be rejected");
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::OptionsJsonError
        );
        assert!(error.message().contains("timeout_ms"));
    }
}

#[test]
fn wasm_timeout_preserves_raw_options_members_for_shared_admission() {
    let raw_theme = r#"{"schema_version":2,"schema_version":2,"extends":"studio"}"#;
    let raw_options = format!(r#"{{"theme":{raw_theme},"theme":{raw_theme},"timeout_ms":125}}"#);
    let (normalized, timeout) = wasm_options(raw_options.as_bytes()).unwrap();
    assert_eq!(timeout, Some(Duration::from_millis(125)));
    assert_eq!(
        String::from_utf8(normalized).unwrap(),
        format!(r#"{{"theme":{raw_theme},"theme":{raw_theme}}}"#)
    );
    assert!(wasm_options(br#"{"timeout_ms":1,"timeout_ms":2}"#).is_err());
}

#[cfg(feature = "svg")]
#[test]
fn wasm_timeout_does_not_hide_duplicate_theme_document_fields() {
    for document in [
        r#"{"schema_version":2,"schema_version":2,"extends":"studio"}"#,
        r#"{"schema_version":2,"schema_\u0076ersion":2,"extends":"studio"}"#,
        r##"{"schema_version":2,"extends":"studio","seed":{"accent":"#123456","accent":"#123456"}}"##,
    ] {
        let options = format!(r#"{{"theme":{document}}}"#);
        let timed_options = format!(r#"{{"theme":{document},"timeout_ms":60000}}"#);
        let plain =
            execute_wasm_operation("svg", b"flowchart LR\nA --> B", options.as_bytes(), None)
                .expect_err("duplicate theme fields must be rejected");
        let timed = execute_wasm_operation(
            "svg",
            b"flowchart LR\nA --> B",
            timed_options.as_bytes(),
            None,
        )
        .expect_err("transport timeout must preserve duplicate theme field rejection");
        assert_eq!(timed.status(), plain.status());
        assert_eq!(timed.message(), plain.message());
        assert!(plain.message().contains("duplicate"), "{plain:?}");
    }
}

#[test]
fn wasm_options_rejects_oversized_documents_before_transport_parsing() {
    let oversized = vec![b' '; merman_bindings_core::BINDING_OPTIONS_JSON_MAX_BYTES + 1];
    let error = wasm_options(&oversized).expect_err("oversized options must fail closed");
    assert_eq!(
        error.status(),
        merman_bindings_core::BindingStatus::ResourceLimitExceeded
    );
    let details = error
        .resource_details()
        .expect("options byte-limit details");
    assert_eq!(details.limit_id, "max_options_json_bytes");
    assert_eq!(details.phase, "options-json-preflight");
}

#[test]
fn wasm_zero_timeout_returns_structured_deadline_cancellation_at_admission() {
    let error = execute_wasm_operation(
        "semantic-json",
        b"flowchart TD\nA --> B",
        br#"{"timeout_ms":0}"#,
        None,
    )
    .expect_err("a zero deadline must cancel before operation work starts");

    assert_eq!(
        error.status(),
        merman_bindings_core::BindingStatus::Cancelled
    );
    let details = error
        .cancellation_details()
        .expect("cancelled errors carry structured details");
    assert_eq!(details.reason, "deadline_exceeded");
    assert_eq!(details.phase, "admission");

    let payload = binding_error_payload_value(&error).unwrap();
    assert_eq!(payload["code_name"], "MERMAN_CANCELLED");
    assert_eq!(
        payload["details"]["cancellation"]["reason"],
        "deadline_exceeded"
    );
    assert_eq!(payload["details"]["cancellation"]["phase"], "admission");
}

#[cfg(feature = "svg")]
#[test]
fn host_measurement_operations_match_the_exact_wasm_protocol() {
    let operations = merman_bindings_core::TextMeasurementOperation::ALL
        .map(|operation| (operation.external_code(), operation.external_name()));

    assert_eq!(
        operations,
        [
            (0, "measure"),
            (1, "computed-length"),
            (2, "bbox-x"),
            (3, "bbox-x-with-ascii-overhang"),
            (4, "title-bbox-x"),
            (5, "simple-bbox-width"),
            (6, "raw-bbox-width"),
            (7, "tspan-bbox-width"),
            (8, "tspan-bbox-height"),
            (9, "wrap-probe-bbox-width"),
            (10, "simple-bbox-height"),
            (11, "wrapped"),
            (12, "wrapped-with-raw-width"),
            (13, "bounding-client-rect-width"),
            (14, "create-text-bbox-y-offset"),
            (15, "mermaid-calculate-text-dimensions"),
            (16, "canvas-measure-text-width"),
            (17, "create-text-middle-bbox-y-offset"),
            (18, "raw-bbox-height"),
            (19, "raw-bbox-y-offset"),
            (20, "raw-middle-bbox-y-offset"),
            (21, "raw-hanging-bbox-y-offset"),
            (22, "html-line-height"),
        ]
    );
}

#[cfg(feature = "svg")]
#[test]
fn wasm_host_request_preserves_tspan_whitespace_and_wrap_contract() {
    use merman_bindings_core::{
        HostTextMeasurementRequest,
        TextMeasurementOperation::{
            BBoxX, CanvasMeasureTextWidth, ComputedLength, Measure, RawBBoxWidth, TspanBBoxHeight,
            TspanBBoxWidth,
        },
        TextMeasurementPhase, TextStyle,
    };

    let style = TextStyle {
        font_family: Some("Test Font".to_string()),
        font_size: 16.0,
        font_weight: Some("600".to_string()),
        font_style: Some("italic".to_string()),
    };
    for (operation, wrap_mode, max_width, white_space) in [
        (ComputedLength, WrapMode::HtmlLike, None, "break-spaces"),
        (BBoxX, WrapMode::HtmlLike, None, "break-spaces"),
        (RawBBoxWidth, WrapMode::HtmlLike, None, "break-spaces"),
        (TspanBBoxWidth, WrapMode::HtmlLike, None, "break-spaces"),
        (TspanBBoxHeight, WrapMode::HtmlLike, None, "break-spaces"),
        (
            TspanBBoxWidth,
            WrapMode::HtmlLike,
            Some(80.0),
            "break-spaces",
        ),
        (Measure, WrapMode::HtmlLike, None, "nowrap"),
        (Measure, WrapMode::HtmlLike, Some(80.0), "break-spaces"),
        (CanvasMeasureTextWidth, WrapMode::HtmlLike, None, "nowrap"),
        (TspanBBoxWidth, WrapMode::SvgLike, None, "normal"),
        (Measure, WrapMode::SvgLike, Some(80.0), "normal"),
        (TspanBBoxWidth, WrapMode::SvgLikeSingleRun, None, "normal"),
        (Measure, WrapMode::SvgLikeSingleRun, Some(80.0), "normal"),
    ] {
        let projected = wasm_host_text_measure_request(HostTextMeasurementRequest {
            operation,
            phase: TextMeasurementPhase::SvgBBox,
            text: "a  b",
            style: &style,
            max_width,
            wrap_mode,
        })
        .expect("request projection");
        let (expected_wrap_mode, line_height) = match wrap_mode {
            WrapMode::HtmlLike => ("html-like", 24.0),
            WrapMode::SvgLike => ("svg-like", 17.6),
            WrapMode::SvgLikeSingleRun => ("svg-like-single-run", 17.6),
        };
        assert_eq!(
            serde_json::to_value(projected).expect("serialize projected request"),
            serde_json::json!({
                "operation": operation.external_name(),
                "phase": "svg-bbox",
                "text": "a  b",
                "font_family": "Test Font",
                "font_size": 16.0,
                "font_weight": "600",
                "font_style": "italic",
                "max_width": max_width,
                "has_max_width": max_width.is_some(),
                "line_height": line_height,
                "letter_spacing": 0.0,
                "word_spacing": 0.0,
                "wrap_mode": expected_wrap_mode,
                "direction": "auto",
                "white_space": white_space,
            }),
            "{operation:?}, {wrap_mode:?}, max_width={max_width:?}"
        );
    }
}

#[cfg(feature = "svg")]
#[test]
fn wasm_checked_decoder_rejects_missing_fields_and_oversized_counts() {
    let style = merman_bindings_core::TextStyle::default();
    let request = merman_bindings_core::HostTextMeasurementRequest {
        operation: merman_bindings_core::TextMeasurementOperation::Measure,
        phase: merman_bindings_core::TextMeasurementPhase::Layout,
        text: "x",
        style: &style,
        max_width: None,
        wrap_mode: merman_bindings_core::WrapMode::SvgLike,
    };
    let result = |height, line_count| WasmHostTextMeasureResult {
        kind: Some("metrics".to_string()),
        width: Some(1.0),
        height,
        length: None,
        line_count,
        bbox_left: None,
        bbox_right: None,
        raw_width: None,
    };

    assert!(decode_wasm_host_text_measurement(request, result(None, Some(1))).is_err());
    assert!(decode_wasm_host_text_measurement(request, result(Some(1.0), Some(3))).is_err());
}

#[cfg(feature = "svg")]
#[test]
fn svg_plan_reports_the_owner_capability_payload() {
    let result = execute_wasm_operation(
        "svg-plan-json",
        b"flowchart TD\nA[Hello] --> B[World]",
        b"",
        None,
    )
    .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&result).unwrap();

    assert_eq!(plan["planned_operation_id"], "svg");
    assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
    assert_eq!(plan["ready"], true);
}

#[test]
fn wasm_theme_catalog_and_operations_follow_the_artifact_capabilities() {
    let capabilities = wasm_runtime_catalog().capabilities;
    for operation_id in ["resolve-theme", "export-theme-preset"] {
        assert_eq!(
            capabilities.operation_ids.contains(&operation_id),
            cfg!(feature = "svg"),
            "unexpected availability for {operation_id}"
        );
    }
    let catalog = wasm_artifact_contract()
        .metadata_json("theme-catalog")
        .unwrap();
    let catalog: serde_json::Value = serde_json::from_slice(&catalog).unwrap();
    assert_eq!(catalog["schema_version"], 2);
    assert!(
        catalog["mermaid_themes"]
            .as_array()
            .unwrap()
            .contains(&"dark".into())
    );
    let presets = catalog["presets"].as_array().unwrap();
    let families = catalog["families"].as_array().unwrap();
    if cfg!(any(feature = "svg", feature = "ascii")) {
        let studio = presets
            .iter()
            .find(|preset| preset["id"] == "studio")
            .unwrap();
        assert_eq!(studio["swatches"].as_array().unwrap().len(), 6);
        assert!(!families.is_empty());
    } else {
        assert!(presets.is_empty());
        assert!(families.is_empty());
    }
}

#[cfg(feature = "svg")]
#[test]
fn wasm_theme_authoring_operations_share_the_canonical_operation_surface() {
    let preset = execute_wasm_operation("export-theme-preset", b"studio", b"", None)
        .expect("WASM should export a built-in preset");
    let preset: serde_json::Value = serde_json::from_slice(&preset).unwrap();
    assert_eq!(preset["schema_version"], 2);
    assert_eq!(preset["id"], "studio");

    let request = serde_json::to_vec(&serde_json::json!({
        "document": preset,
        "appearance": "dark",
        "auto_fix_contrast": true,
    }))
    .unwrap();
    let result = execute_wasm_operation("resolve-theme", &request, b"", None).unwrap();
    let result: serde_json::Value = serde_json::from_slice(&result).unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["resolved"]["appearance"], "dark");
    assert!(result["notices"].is_array());

    for (document, column, path, message) in [
        (
            "{\n  \"schema_version\": 2,\n  \"extends\": ]\n}",
            14,
            "extends",
            "expected value",
        ),
        (
            "{\n  \"schema_version\": 2,\n  \"schema_version\": 2\n}",
            18,
            ".",
            "duplicate",
        ),
    ] {
        let request = serde_json::to_vec(&serde_json::json!({ "document": document })).unwrap();
        let result = execute_wasm_operation("resolve-theme", &request, b"", None).unwrap();
        let error: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(error["ok"], false);
        assert_eq!(error["line"], 3);
        assert_eq!(error["column"], column);
        assert_eq!(error["path"], path);
        assert!(error["message"].as_str().unwrap().contains(message));
    }
}

#[cfg(all(
    feature = "svg",
    not(feature = "layout-cytoscape"),
    not(feature = "layout-dagre"),
    not(feature = "layout-elk"),
    not(feature = "math")
))]
#[test]
fn ambient_render_dependencies_do_not_widen_the_wasm_owner_contract() {
    let capabilities = wasm_artifact_contract().runtime_capabilities();
    assert!(capabilities.has_capability("svg"));

    for (capability_id, source) in [
        (
            "layout-cytoscape",
            "architecture-beta\n  service api(server)[API]",
        ),
        (
            "layout-dagre",
            "---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B",
        ),
        ("math", "sequenceDiagram\nA->>B: $$x^2$$"),
    ] {
        assert!(!capabilities.has_capability(capability_id));

        let plan = execute_wasm_operation("svg-plan-json", source.as_bytes(), b"", None).unwrap();
        let plan: serde_json::Value = serde_json::from_slice(&plan).unwrap();
        assert_eq!(
            plan["required_capability_ids"],
            serde_json::json!([capability_id])
        );
        assert_eq!(
            plan["missing_capability_ids"],
            serde_json::json!([capability_id])
        );
        assert_eq!(plan["ready"], false);

        let error = execute_wasm_operation("svg", source.as_bytes(), b"", None)
            .expect_err("the WASM owner contract must reject ambient render capabilities");
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::UnsupportedOperation
        );
        assert_eq!(error.capability_id(), Some(capability_id));
    }

    let error = execute_wasm_operation(
        "svg",
        b"flowchart TD\nA --> B",
        br#"{"environment":{"math_renderer":"ratex"}}"#,
        None,
    )
    .expect_err("explicit ratex selection requires owner-selected math");
    assert_eq!(
        error.status(),
        merman_bindings_core::BindingStatus::UnsupportedOperation
    );
    assert_eq!(error.capability_id(), Some("math"));
}

#[cfg(feature = "analysis")]
#[test]
fn validation_error_uses_binding_status() {
    let json: Value =
        serde_json::from_slice(&merman_bindings_core::validate_json(b"", b"").unwrap()).unwrap();

    assert_eq!(json["valid"], false);
    assert_eq!(json["code_name"], "MERMAN_NO_DIAGRAM");
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("no Mermaid diagram")
    );
}

#[cfg(all(target_arch = "wasm32", feature = "analysis"))]
#[test]
fn analyze_json_exposes_diagnostics_payload() {
    let value: Value = serde_wasm_bindgen::from_value(analyze_json("", None).unwrap()).unwrap();
    assert_no_diagram_analysis_payload(&value);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
#[test]
fn analyze_json_exposes_diagnostics_payload() {
    let value: Value =
        serde_json::from_slice(&merman_bindings_core::analyze_json(b"", b"").unwrap()).unwrap();
    assert_no_diagram_analysis_payload(&value);
}

#[cfg(feature = "analysis")]
fn assert_no_diagram_analysis_payload(value: &Value) {
    assert_eq!(value["version"], 1);
    assert_eq!(value["valid"], false);
    assert_eq!(value["diagnostics"][0]["code_name"], "MERMAN_NO_DIAGRAM");
}

#[cfg(all(target_arch = "wasm32", feature = "analysis"))]
#[test]
fn analysis_facts_exposes_parser_backed_syntax_payload() {
    let value: Value =
        serde_wasm_bindgen::from_value(analysis_facts("flowchart TD\nA-->B\n", None).unwrap())
            .unwrap();
    assert_parser_backed_analysis_facts_payload(&value);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
#[test]
fn analysis_facts_exposes_parser_backed_syntax_payload() {
    let value: Value = serde_json::from_slice(
        &merman_bindings_core::analysis_facts_json(b"flowchart TD\nA-->B\n", b"").unwrap(),
    )
    .unwrap();
    assert_parser_backed_analysis_facts_payload(&value);
}

#[cfg(feature = "analysis")]
fn assert_parser_backed_analysis_facts_payload(value: &Value) {
    assert_eq!(
        value["version"],
        merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION
    );
    assert_eq!(value["valid"], true);
    assert_eq!(
        value["diagrams"][0]["syntax"]["fact_source"],
        "parser_complete"
    );
    assert_eq!(value["diagrams"][0]["syntax"]["source_mapped_spans"], true);
    assert_eq!(value["diagrams"][0]["syntax"]["effective_layout"], "elk");
    assert!(
        value["diagrams"][0]["syntax"]["semantic_items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["name"] == "A"
                    && item["rename_policy"] == "flowchart_node_id"
                    && item["span"]["document"].is_object()
            })
    );
}

#[cfg(all(target_arch = "wasm32", feature = "analysis"))]
#[test]
fn analysis_facts_serializes_unavailable_body_semantics() {
    let value: Value = serde_wasm_bindgen::from_value(
        analysis_facts("unknownDiagram\nPretendNode --> OtherNode\n", None).unwrap(),
    )
    .unwrap();
    assert_unavailable_analysis_facts_payload(&value);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
#[test]
fn analysis_facts_serializes_unavailable_body_semantics() {
    let value: Value = serde_json::from_slice(
        &merman_bindings_core::analysis_facts_json(
            b"unknownDiagram\nPretendNode --> OtherNode\n",
            b"",
        )
        .unwrap(),
    )
    .unwrap();
    assert_unavailable_analysis_facts_payload(&value);
}

#[cfg(feature = "analysis")]
fn assert_unavailable_analysis_facts_payload(value: &Value) {
    assert_eq!(
        value["version"],
        merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION
    );
    let syntax = &value["diagrams"][0]["syntax"];
    assert_eq!(syntax["fact_source"], "unavailable");
    assert_eq!(syntax["parser_backed"], false);
    assert_eq!(syntax["source_mapped_spans"], false);
    assert_eq!(syntax["node_ids"], serde_json::json!([]));
    assert_eq!(syntax["references"], serde_json::json!([]));
    assert_eq!(syntax["outline_items"], serde_json::json!([]));
    assert_eq!(syntax["semantic_items"], serde_json::json!([]));
}

#[cfg(all(target_arch = "wasm32", feature = "analysis"))]
#[test]
fn analyze_document_exposes_markdown_diagnostics_payload() {
    let value: Value = serde_wasm_bindgen::from_value(
        analyze_document(
            "before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
            "file:///tmp/example.md".to_string(),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert_markdown_document_analysis_payload(&value);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
#[test]
fn analyze_document_exposes_markdown_diagnostics_payload() {
    let value: Value = serde_json::from_slice(
        &merman_bindings_core::analyze_document_json(
            b"before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
            b"file:///tmp/example.md",
            b"",
        )
        .unwrap(),
    )
    .unwrap();
    assert_markdown_document_analysis_payload(&value);
}

#[cfg(feature = "analysis")]
fn assert_markdown_document_analysis_payload(value: &Value) {
    assert_eq!(value["valid"], false);
    assert_eq!(value["source"]["kind"], "markdown");
    assert_eq!(value["diagnostics"][0]["span"]["line"], 4);
    assert!(
        value["diagnostics"][0]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|related| related["message"] == "Mermaid fence 1")
    );
}

#[cfg(all(target_arch = "wasm32", feature = "analysis"))]
#[test]
fn analyze_document_facts_exposes_markdown_syntax_payload() {
    let value: Value = serde_wasm_bindgen::from_value(
        analyze_document_facts(
            "before\n```mermaid\nflowchart TD\nA@{\n  shape: rou\n}\n```\nafter\n",
            "file:///tmp/example.md".to_string(),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert_markdown_document_analysis_facts_payload(&value);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
#[test]
fn analyze_document_facts_exposes_markdown_syntax_payload() {
    let value: Value = serde_json::from_slice(
        &merman_bindings_core::analyze_document_facts_json(
            b"before\n```mermaid\nflowchart TD\nA@{\n  shape: rou\n}\n```\nafter\n",
            b"file:///tmp/example.md",
            b"",
        )
        .unwrap(),
    )
    .unwrap();
    assert_markdown_document_analysis_facts_payload(&value);
}

#[cfg(feature = "analysis")]
fn assert_markdown_document_analysis_facts_payload(value: &Value) {
    assert_eq!(
        value["version"],
        merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION
    );
    assert_eq!(value["valid"], false);
    assert_eq!(value["source"]["kind"], "markdown");
    assert_eq!(value["diagrams"][0]["source_id"], "mermaid-fence-1");
    assert_eq!(value["diagrams"][0]["syntax"]["parser_backed"], true);
    assert!(
        value["diagrams"][0]["syntax"]["expected_syntax"]
            .as_array()
            .unwrap()
            .iter()
            .any(|expected| {
                expected["kind"] == "shape" && expected["span"]["document"].is_object()
            })
    );
}

#[test]
fn wasm_error_payload_is_structured() {
    let err = merman_bindings_core::render_svg(b"flowchart TD\nA", b"{").unwrap_err();
    let json = binding_error_payload_value(&err).unwrap();

    assert_eq!(json["version"], 1);
    assert_eq!(json["ok"], false);
    assert_eq!(json["code_name"], "MERMAN_OPTIONS_JSON_ERROR");
    assert_eq!(json["kind"], "generic");
    assert!(json["capability_id"].is_null());
    assert!(json["message"].as_str().unwrap().contains("options_json"));

    let err = BindingError::resource_limit(
        "embedded_image_decode",
        "max_embedded_image_bytes",
        5,
        4,
        "constrained",
        "embedded image is too large",
    );
    let json = binding_error_payload_value(&err).unwrap();
    assert_eq!(
        json["details"]["resource"]["limit_id"],
        "max_embedded_image_bytes"
    );
    assert_eq!(json["details"]["resource"]["actual"], 5);
    assert_eq!(json["details"]["resource"]["cause"], "ceiling");

    let err = BindingError::resource_limit_with_cause(
        merman_bindings_core::BindingResourceLimitCause::ArithmeticOverflow,
        "layout_model",
        "max_layout_work_units",
        u64::MAX,
        800_000,
        "interactive",
        "layout work accounting overflowed",
    );
    let json = binding_error_payload_value(&err).unwrap();
    assert_eq!(json["code_name"], "MERMAN_RESOURCE_LIMIT_EXCEEDED");
    assert_eq!(json["details"]["resource"]["cause"], "arithmetic_overflow");
    assert_eq!(
        json["details"]["resource"]["limit_id"],
        "max_layout_work_units"
    );
    assert_eq!(json["details"]["resource"]["phase"], "layout_model");
    assert_eq!(
        json["details"]["resource"]["actual"],
        "18446744073709551615"
    );
    assert_eq!(json["details"]["resource"]["max"], 800_000);
    assert_eq!(json["details"]["resource"]["profile"], "interactive");

    let err = BindingError::new(
        merman_bindings_core::BindingStatus::ParseError,
        "invalid edge",
    )
    .with_diagnostic_details(
        merman_bindings_core::BindingDiagnosticErrorDetails::new("flowchart.edge.invalid")
            .with_span(merman_bindings_core::BindingDiagnosticSpan::new(
                3, 8, "exact",
            ))
            .with_field("edge")
            .with_diagram_type("flowchart"),
    );
    let json = binding_error_payload_value(&err).unwrap();
    assert_eq!(
        json["details"]["diagnostic"]["code"],
        "flowchart.edge.invalid"
    );
    assert_eq!(json["details"]["diagnostic"]["span"]["start"], 3);
    assert_eq!(json["details"]["diagnostic"]["span"]["end"], 8);
    assert_eq!(json["details"]["diagnostic"]["span"]["kind"], "exact");
    assert_eq!(json["details"]["diagnostic"]["field"], "edge");
    assert_eq!(json["details"]["diagnostic"]["diagram_type"], "flowchart");
}

#[cfg(feature = "svg")]
#[test]
fn wasm_svg_data_and_result_share_bytes_and_errors() {
    let source = b"flowchart TD\nA --> B";
    for (options, expected) in [
        (
            b"".as_slice(),
            serde_json::json!({ "mode": "mermaid_parity", "notices": [] }),
        ),
        (
            br#"{"theme":{"preset":"studio","appearance":"dark"}}"#.as_slice(),
            serde_json::json!({
                "mode": "merman_native", "preset_id": "studio", "appearance": "dark", "notices": []
            }),
        ),
    ] {
        let full = execute_wasm_operation_result("svg", source, options, None).unwrap();
        let data = execute_wasm_operation("svg", source, options, None).unwrap();
        assert_eq!(data, full.data());
        assert_eq!(full.metadata().theme_outcome(), Some(&expected));
    }
    for options in [
        b"{".as_slice(),
        br#"{"version":99}"#.as_slice(),
        br#"{"timeout_ms":0}"#.as_slice(),
    ] {
        let full = execute_wasm_operation_result("svg", source, options, None).unwrap_err();
        let data = execute_wasm_operation("svg", source, options, None).unwrap_err();
        assert_eq!(data, full);
    }
}

#[cfg(all(feature = "layout-shoal", feature = "diagram-flowchart"))]
#[test]
fn wasm_svg_result_carries_an_opt_in_shoal_layout_frame() {
    let source = b"---\nconfig:\n  layout: shoal\n---\nflowchart LR\nA --> B";
    let options = br#"{"layout":{"capture_shoal_frame":true}}"#;
    let result = execute_wasm_operation_result("svg", source, options, None).unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
    assert!(metadata["shoal_layout_frame"].is_object());
    let plain = execute_wasm_operation_result("svg", source, b"", None).unwrap();
    assert_eq!(plain.data(), result.data());
    assert!(plain.metadata().shoal_layout_frame().is_none());
}

#[test]
fn wasm_execution_is_bound_to_the_deterministic_web_contract() {
    let error = execute_wasm_operation(
        "semantic-json",
        b"flowchart TD\nA --> B",
        br#"{"runtime_policy":"native"}"#,
        None,
    )
    .expect_err("the Web contract must reject native runtime policy selection");

    assert_eq!(
        error.status(),
        merman_bindings_core::BindingStatus::OptionsJsonError
    );
    assert!(error.message().contains("not exposed by target `web`"));
}

#[test]
fn runtime_catalog_follows_the_wasm_feature_surface_and_local_relations() {
    let catalog = wasm_runtime_catalog();
    assert_eq!(
        catalog
            .payload_schemas
            .iter()
            .map(|schema| (schema.id, schema.version))
            .collect::<Vec<_>>(),
        vec![(
            "binding-result",
            merman_bindings_core::BINDING_RESULT_PAYLOAD_VERSION,
        )]
    );
    let capabilities = catalog.capabilities;

    assert_eq!(
        capabilities.has_capability("analysis"),
        cfg!(feature = "analysis")
    );
    assert_eq!(
        capabilities.has_capability("ascii"),
        cfg!(feature = "ascii")
    );
    assert_eq!(
        wasm_ascii_supported_diagrams().is_empty(),
        !cfg!(feature = "ascii")
    );
    assert_eq!(capabilities.has_capability("svg"), cfg!(feature = "svg"));
    assert!(!capabilities.has_capability("embedded-fonts"));
    #[cfg(feature = "svg")]
    {
        assert_eq!(
            capabilities.has_capability("layout-cytoscape"),
            cfg!(feature = "layout-cytoscape")
        );
        assert_eq!(
            capabilities.has_capability("layout-dagre"),
            cfg!(feature = "layout-dagre")
        );
        assert_eq!(
            capabilities.has_capability("layout-elk"),
            cfg!(feature = "layout-elk")
        );
        assert_eq!(
            capabilities.has_capability("layout-shoal"),
            cfg!(feature = "layout-shoal")
        );
        assert_eq!(capabilities.has_capability("math"), cfg!(feature = "math"));
    }
    assert!(
        capabilities.system_adapter_ids.is_empty(),
        "browser WASM must not claim native system adapters"
    );
    assert_eq!(
        capabilities.has_capability("editor"),
        cfg!(feature = "editor")
    );
    if capabilities.has_capability("svg") {
        let providers = capabilities
            .text_measurement
            .as_ref()
            .expect("SVG surface must report a text-measurement route")
            .provider_ids
            .as_slice();
        assert!(providers.contains(&"deterministic"));
        assert_eq!(
            providers.contains(&"host-callback"),
            cfg!(all(feature = "svg", target_arch = "wasm32"))
        );
    } else {
        assert!(capabilities.text_measurement.is_none());
    }

    assert!(
        capabilities
            .output_ids
            .iter()
            .all(|output| capabilities.has_operation(output))
    );
    assert!(
        capabilities
            .system_adapter_ids
            .iter()
            .all(|adapter| capabilities.has_capability(adapter))
    );
}

#[test]
fn runtime_catalog_uses_the_wasm_transport_api_and_resource_catalog() {
    let catalog = wasm_runtime_catalog();
    assert_eq!(
        catalog.schema_version,
        merman_bindings_core::RUNTIME_CATALOG_SCHEMA_VERSION
    );
    assert_eq!(catalog.transport_api_version, WASM_TRANSPORT_API_VERSION);
    #[cfg(target_arch = "wasm32")]
    assert!(
        catalog
            .output_contracts
            .iter()
            .all(|output| output.system_fonts.is_none()),
        "browser WASM cannot discover host system fonts"
    );
    let operation_ids = &catalog.capabilities.operation_ids;
    let resources = catalog.resources;
    assert_eq!(resources.general_binding_default_profile, "interactive");
    assert_eq!(resources.profiles.len(), 4);
    #[cfg(feature = "ascii")]
    for case in ascii_resource_boundary_contract().binding_core_interactive {
        let limit = resources
            .limits
            .iter()
            .find(|limit| limit.id == case.id)
            .unwrap_or_else(|| panic!("WASM resource catalog is missing {}", case.id));
        assert!(
            limit.operation_ids.contains(&"ascii"),
            "WASM resource catalog does not route {} to ASCII",
            case.id
        );
    }
    for limit in resources.limits {
        assert!(
            limit
                .operation_ids
                .iter()
                .all(|operation_id| operation_ids.contains(operation_id)),
            "resource limit {} must only name callable Web operations",
            limit.id
        );
    }
}

#[cfg(feature = "all-diagrams")]
#[test]
fn family_capabilities_expose_the_unique_catalog() {
    let capabilities = merman_bindings_core::diagram_family_capabilities();
    let expects_editor_parsers = cfg!(any(feature = "analysis", feature = "editor"));
    assert!(capabilities.iter().any(|capability| {
        capability.diagram_type == "flowchart"
            && capability.family_id.as_str() == "flowchart"
            && capability.metadata_id == Some("flowchart")
            && capability.render_model_kind == Some("flowchart")
            && !capability.has_detector
            && capability.has_semantic_parser
            && capability.has_editor_parser == expects_editor_parsers
            && capability.has_combined_parser == expects_editor_parsers
            && capability.has_render_parser
            && !capability.has_header
            && capability.config_namespace == Some("flowchart")
    }));
    assert!(capabilities.iter().any(|capability| {
        capability.diagram_type == "flowchart-v2"
            && capability.logical_family_kind == "flowchart"
            && capability.metadata_id == Some("flowchart")
            && capability.render_model_kind == Some("flowchart")
            && capability.has_detector
            && capability.has_header
    }));
    for (diagram_type, metadata_id) in [
        ("agentflow", Some("agentflow")),
        ("usecase", Some("usecase")),
    ] {
        assert!(capabilities.iter().any(|capability| {
            capability.diagram_type == diagram_type
                && capability.logical_family_kind == diagram_type
                && capability.metadata_id == metadata_id
                && capability.render_model_kind == Some(diagram_type)
                && capability.has_detector
                && capability.has_semantic_parser
                && capability.has_editor_parser == expects_editor_parsers
                && capability.has_combined_parser == expects_editor_parsers
                && capability.has_render_parser
                && capability.has_header
                && capability.config_namespace == Some(diagram_type)
        }));
    }
    assert!(
        capabilities
            .iter()
            .any(|capability| capability.diagram_type == "mindmap")
    );
}

#[cfg(feature = "ascii")]
#[test]
fn render_ascii_impl_returns_text() {
    let text = string_result(merman_bindings_core::render_ascii(
        b"flowchart TD\nA[Hello] --> B[World]",
        b"",
    ))
    .unwrap();

    assert!(text.contains("Hello"));
    assert!(text.contains("World"));
}

#[cfg(feature = "ascii")]
#[test]
fn render_ascii_impl_applies_flowchart_node_label_wrap_width() {
    let text = render_ascii(
        "flowchart TD\nA[\"Alpha Beta Gamma Delta\"]",
        Some(r#"{ "ascii": { "flowchartNodeLabelWrapWidth": 8 } }"#.to_string()),
    )
    .unwrap();

    for expected in ["Alpha", "Beta", "Gamma", "Delta"] {
        assert!(text.contains(expected), "missing {expected:?}:\n{text}");
    }
    assert!(!text.contains("Alpha Beta Gamma Delta"), "{text}");
}

#[cfg(all(feature = "ascii", feature = "diagram-flowchart"))]
#[test]
fn wasm_ascii_result_preserves_the_canonical_report_and_text_projection() {
    let source = "flowchart LR\nA[Hello] --> B[World]";
    let result = execute_wasm_operation_result("ascii", source.as_bytes(), b"", None).unwrap();
    assert_eq!(
        result.data(),
        render_ascii(source, None).unwrap().as_bytes()
    );
    let metadata: Value = serde_json::from_slice(result.metadata_json()).unwrap();
    assert_eq!(metadata["version"], 1);
    assert_eq!(metadata["operation_id"], "ascii");
    assert_eq!(metadata["byte_length"], result.data().len());
    let plan = &metadata["output_plan"];
    assert_eq!(plan["kind"], "ascii");
    assert_eq!(plan["schema_version"], 4);
    assert_eq!(plan["requested_graph_presentation"], "strict");
    assert_eq!(plan["projection"], "diagrammatic");
    assert_eq!(plan["layout_profile"], "canonical");
    assert_eq!(plan["primary_width"], plan["emitted_width"]);
    assert_eq!(plan["primary_height"], plan["emitted_height"]);
    assert_eq!(plan["primary_overflowed"], false);
    assert_eq!(plan["emitted_overflowed"], false);
    assert!(plan.get("overflowed").is_none());

    let empty = execute_wasm_operation_result("ascii", b"flowchart LR", b"", None).unwrap();
    assert!(empty.data().is_empty());
    let metadata: Value = serde_json::from_slice(empty.metadata_json()).unwrap();
    let plan = &metadata["output_plan"];
    assert_eq!(plan["primary_width"], 0);
    assert_eq!(plan["primary_height"], 0);
    assert_eq!(plan["primary_overflowed"], false);
    assert_eq!(plan["layout_profile"], "canonical");
    assert_eq!(plan["outcome"], "empty");
}

#[cfg(all(feature = "ascii", feature = "diagram-flowchart"))]
#[test]
fn wasm_ascii_adaptive_result_keeps_unmeasured_primary_fields_null() {
    let mut source = String::from("flowchart LR\nA@{ shape: f-circ }\nB[Target 🦀]\n");
    for _ in 0..97 {
        source.push_str("A o--o B\n");
    }
    let result = execute_wasm_operation_result(
        "ascii",
        source.as_bytes(),
        br#"{"version":4,"ascii":{"graph_presentation":"adaptive","charset":"ascii"}}"#,
        None,
    )
    .unwrap();
    let metadata: Value = serde_json::from_slice(result.metadata_json()).unwrap();
    assert_eq!(metadata["byte_length"], result.data().len());
    let plan = &metadata["output_plan"];
    assert_eq!(plan["schema_version"], 4);
    assert_eq!(plan["requested_graph_presentation"], "adaptive");
    assert_eq!(plan["projection"], "structured_text");
    assert_eq!(plan["outcome"], "fallback");
    assert_eq!(plan["fallback_reason"], "port_capacity_exceeded");
    assert_eq!(plan["requested_layout_profile"], "canonical");
    assert_eq!(plan["compact_attempted"], false);
    for field in [
        "primary_width",
        "primary_height",
        "layout_profile",
        "primary_overflowed",
        "requested_max_width",
    ] {
        assert_eq!(plan.get(field), Some(&Value::Null), "{field}");
    }
    assert_eq!(plan["emitted_overflowed"], false);
    assert!(plan.get("overflowed").is_none());
    assert!(plan["emitted_width"].as_u64().unwrap() > 0);
    let text = std::str::from_utf8(result.data()).unwrap();
    assert!(text.contains("Target 🦀"));
    assert!(text.contains("E97:"));
}

#[cfg(all(feature = "ascii", feature = "diagram-timeline"))]
#[test]
fn wasm_ascii_result_reports_measured_width_fallback() {
    let result = execute_wasm_operation_result(
        "ascii",
        b"timeline\ntitle Basic\n2024 : Event with a long authored value",
        br#"{"ascii":{"maxWidth":10,"overflow":"fallback"}}"#,
        None,
    )
    .unwrap();
    let metadata: Value = serde_json::from_slice(result.metadata_json()).unwrap();
    let plan = &metadata["output_plan"];
    assert_eq!(plan["schema_version"], 4);
    assert_eq!(plan["requested_graph_presentation"], "strict");
    assert_eq!(plan["outcome"], "fallback");
    assert_eq!(plan["fallback_reason"], "primary_overflow");
    assert_eq!(plan["primary_overflowed"], true);
    assert_eq!(plan["emitted_overflowed"], false);
    assert!(plan["primary_width"].as_u64().unwrap() > 10);
    assert!(plan["emitted_width"].as_u64().unwrap() <= 10);
    assert_eq!(plan["layout_profile"], "canonical");
}

#[cfg(all(
    feature = "ascii",
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "diagram-state"
))]
#[test]
fn wasm_ascii_result_retains_presentation_admission_boundaries() {
    for (source, options) in [
        (
            "flowchart LR\nA --> B",
            r#"{"ascii":{"graphPresentation":"adaptive","colorMode":"ansi16"}}"#,
        ),
        (
            "sequenceDiagram\nA->>B: Hello",
            r#"{"ascii":{"graph_presentation":"adaptive"}}"#,
        ),
        (
            "stateDiagram-v2\nA --> B",
            r#"{"ascii":{"graphPresentation":"adaptive","layoutProfile":"compact"}}"#,
        ),
    ] {
        let error =
            execute_wasm_operation_result("ascii", source.as_bytes(), options.as_bytes(), None)
                .expect_err("invalid presentation must remain an error");
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::InvalidArgument
        );
    }
    let styled = execute_wasm_operation_result(
        "ascii",
        b"flowchart LR\nA --> B",
        br#"{"ascii":{"colorMode":"ansi16"}}"#,
        None,
    )
    .unwrap();
    let metadata: Value = serde_json::from_slice(styled.metadata_json()).unwrap();
    assert_eq!(
        metadata["output_plan"]["requested_graph_presentation"],
        "strict"
    );
    assert_eq!(metadata["output_plan"]["encoding"], "ansi16");
}

#[cfg(feature = "ascii")]
#[test]
fn wasm_ascii_operation_preserves_a_typed_exact_resource_boundary() {
    let case = ascii_resource_boundary_contract()
        .transport_representatives
        .wasm_interactive;
    let expected = case.exact;

    let exact_options = wasm_ascii_options(&case.id, expected);
    let output = execute_wasm_operation(
        "ascii",
        case.source.as_bytes(),
        exact_options.as_bytes(),
        None,
    )
    .unwrap_or_else(|error| panic!("exact {} boundary failed: {error:?}", case.id));
    assert!(!output.is_empty());

    let below_options = wasm_ascii_options(&case.id, expected - 1);
    let error = execute_wasm_operation_result(
        "ascii",
        case.source.as_bytes(),
        below_options.as_bytes(),
        None,
    )
    .expect_err("one-below WASM ASCII boundary must fail");
    let actual = assert_wasm_ascii_resource_error(error, &case.id, &case.phase, expected - 1);
    assert_eq!(actual, expected);
}
