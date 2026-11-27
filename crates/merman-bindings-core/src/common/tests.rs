#[cfg(feature = "svg")]
use super::options::parse_options_value;
use super::options::{parse_request_overlay, resolve_request_options};
use super::options_wire::ResourceOptionsJson;
use super::resources::{effective_resource_limits, tighten_resource_options};
use super::*;
use crate::artifact_contract::DEFAULT_ARTIFACT_SNAPSHOT;
use crate::resource_contract::{
    BINDING_OPTIONS_JSON_LIMIT_ID, BINDING_OPTIONS_JSON_MAX_BYTES, BindingResourceScope,
};
use serde_json::Value;

#[test]
fn error_payload_json_uses_public_code_names() {
    let payload = error_payload_json_bytes(BindingStatus::RenderError, "failed");
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["code"], BindingStatus::RenderError.code());
    assert_eq!(json["code_name"], BindingStatus::RenderError.code_name());
    assert_eq!(json["kind"], "generic");
    assert!(json["capability_id"].is_null());
    assert_eq!(json["message"], "failed");
    assert!(json.get("details").is_none());

    let error = BindingError::missing_capability("layout-elk", "ELK is unavailable");
    let payload = binding_error_payload_json_bytes(&error);
    let json: Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(json["kind"], "missing-capability");
    assert_eq!(json["capability_id"], "layout-elk");

    let error = BindingError::resource_limit(
        "embedded_image_decode",
        "max_embedded_image_bytes",
        5,
        4,
        "constrained",
        "embedded image is too large",
    );
    let payload = binding_error_payload_json_bytes(&error);
    let json: Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(json["code_name"], "MERMAN_RESOURCE_LIMIT_EXCEEDED");
    assert_eq!(
        json["details"]["resource"]["limit_id"],
        "max_embedded_image_bytes"
    );
    assert_eq!(
        json["details"]["resource"]["phase"],
        "embedded_image_decode"
    );
    assert_eq!(json["details"]["resource"]["actual"], 5);
    assert_eq!(json["details"]["resource"]["max"], 4);
    assert_eq!(json["details"]["resource"]["profile"], "constrained");
    assert_eq!(json["details"]["resource"]["cause"], "ceiling");
    let payload = binding_error_js_payload_json_bytes(&error);
    let json: Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(json["details"]["resource"]["actual"], 5);
    assert_eq!(json["details"]["resource"]["max"], 4);

    let error = BindingError::resource_limit_with_cause(
        BindingResourceLimitCause::ArithmeticOverflow,
        "layout_model",
        "max_layout_work_units",
        u64::MAX,
        800_000,
        "interactive",
        "layout work accounting overflowed",
    );
    let payload = binding_error_payload_json_bytes(&error);
    let json: Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(json["details"]["resource"]["cause"], "arithmetic_overflow");
    assert_eq!(
        json["details"]["resource"]["actual"].as_u64(),
        Some(u64::MAX)
    );
    assert_eq!(json["details"]["resource"]["max"], 800_000);

    let payload = binding_error_js_payload_json_bytes(&error);
    let json: Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(json["details"]["resource"]["cause"], "arithmetic_overflow");
    assert_eq!(
        json["details"]["resource"]["actual"],
        "18446744073709551615"
    );
    assert_eq!(json["details"]["resource"]["max"], 800_000);
    assert_eq!(std::mem::size_of::<BindingResourceLimitCause>(), 1);
    assert!(std::mem::size_of::<BindingError>() < 128);
}

#[test]
fn runtime_policy_errors_are_terminal_safe_and_structured() {
    let error = runtime_policy_error(merman::model::runtime::RuntimePolicyError::SystemTimeZone(
        "adapter\u{1b}\u{7}".to_string(),
    ));

    assert_eq!(error.status(), BindingStatus::RenderError);
    assert!(!error.message().contains('\u{1b}'));
    assert!(!error.message().contains('\u{7}'));
    let details = error
        .diagnostic_details()
        .expect("runtime-policy errors expose diagnostic details");
    assert_eq!(details.code, "merman.runtime_policy");

    let missing = runtime_policy_error(
        merman::model::runtime::RuntimePolicyError::MissingCapability(
            merman::model::runtime::RuntimeCapability::SystemRandom,
        ),
    );
    assert_eq!(missing.kind(), BindingErrorKind::MissingCapability);
    assert_eq!(missing.capability_id(), Some("system-random"));
    assert_eq!(
        missing
            .diagnostic_details()
            .expect("missing-capability errors retain runtime diagnostics")
            .code,
        "merman.runtime_policy"
    );
}

#[test]
fn internal_failures_remain_internal_across_binding_payloads() {
    let error = merman::Error::Internal(merman::InternalFailure::new("fixture subsystem failure"));
    let error = core_error(error);

    assert_eq!(error.status(), BindingStatus::InternalError);
    let details = error
        .diagnostic_details()
        .expect("internal overlay failures expose diagnostic details");
    assert_eq!(details.code, "merman.internal.failure");
    assert_eq!(details.field, None);

    let payload: Value = serde_json::from_slice(&binding_error_payload_json_bytes(&error))
        .expect("binding payload JSON");
    assert_eq!(payload["code_name"], "MERMAN_INTERNAL_ERROR");
    assert_eq!(
        payload["details"]["diagnostic"]["code"],
        "merman.internal.failure"
    );
    assert!(payload["details"]["diagnostic"]["field"].is_null());
}

#[test]
fn error_details_projections_share_fields_and_preserve_transport_integer_encoding() {
    let diagnostic = BindingDiagnosticErrorDetails::new("merman.parse.diagram_parse")
        .with_span(BindingDiagnosticSpan::new(54, 55, "exact"))
        .with_diagram_type("flowchart-v2");
    let error = BindingError::resource_limit_with_cause(
        BindingResourceLimitCause::ArithmeticOverflow,
        "layout_model",
        "max_layout_work_units",
        u64::MAX,
        BINDING_JSON_SAFE_INTEGER_MAX + 1,
        "constrained",
        "layout work accounting overflowed",
    )
    .with_diagnostic_details(diagnostic.clone());
    let ordinary = binding_error_details_json(&error).unwrap().unwrap();
    let javascript = binding_error_js_details_json(&error).unwrap().unwrap();

    assert_eq!(
        ordinary["diagnostic"],
        serde_json::to_value(diagnostic).unwrap()
    );
    assert_eq!(ordinary["resource"]["actual"].as_u64(), Some(u64::MAX));
    assert_eq!(
        ordinary["resource"]["max"].as_u64(),
        Some(BINDING_JSON_SAFE_INTEGER_MAX + 1),
    );
    let mut expected_javascript = ordinary.clone();
    expected_javascript["resource"]["actual"] = serde_json::json!("18446744073709551615");
    expected_javascript["resource"]["max"] = serde_json::json!("9007199254740992");
    assert_eq!(javascript, expected_javascript);

    let payload: Value = serde_json::from_slice(&binding_error_payload_json_bytes(&error))
        .expect("ordinary error payload");
    assert_eq!(ordinary, payload["details"]);
    let payload: Value = serde_json::from_slice(&binding_error_js_payload_json_bytes(&error))
        .expect("JavaScript error payload");
    assert_eq!(javascript, payload["details"]);

    let plain = BindingError::invalid_argument("invalid request");
    assert_eq!(binding_error_details_json(&plain).unwrap(), None);
    assert_eq!(binding_error_js_details_json(&plain).unwrap(), None);
}

#[test]
fn cancellation_payload_is_structured_and_disjoint_from_resource_details() {
    let cancelled = core_error(merman::Error::OperationCancelled(
        merman::OperationCancelled {
            phase: merman::OperationPhase::Layout,
            reason: merman::CancelReason::DeadlineExceeded,
        },
    ));
    assert_eq!(cancelled.status(), BindingStatus::Cancelled);
    assert_eq!(cancelled.status().code(), 12);
    assert_eq!(cancelled.status().code_name(), "MERMAN_CANCELLED");
    assert_eq!(cancelled.resource_details(), None);
    assert_eq!(
        cancelled.cancellation_details(),
        Some(BindingCancellationErrorDetails {
            reason: "deadline_exceeded",
            phase: "layout",
        })
    );

    let json: Value =
        serde_json::from_slice(&binding_error_payload_json_bytes(&cancelled)).unwrap();
    assert_eq!(json["code_name"], "MERMAN_CANCELLED");
    assert_eq!(
        json["details"]["cancellation"]["reason"],
        "deadline_exceeded"
    );
    assert_eq!(json["details"]["cancellation"]["phase"], "layout");
    assert!(json["details"].get("resource").is_none());
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn output_option_groups_follow_constructor_and_request_scopes() {
    parse_options(
        br#"{
            "raster":{"scale":2,"fit_to":{"width":640}},
            "jpeg":{"quality":85},
            "pdf":{"page_policy":{"kind":"fit-css-width","max_width_px":800}}
        }"#,
    )
    .expect("the reusable constructor accepts the compiled artifact union");

    let jpeg = resolve_request_options(
        b"",
        br#"{"raster":{"scale":2},"jpeg":{"quality":85}}"#,
        BindingResourceScope::Jpeg,
    )
    .expect("JPEG accepts shared raster and JPEG-specific options");
    assert_eq!(jpeg.jpeg.and_then(|options| options.quality), Some(85));

    for (scope, request, group) in [
        (
            BindingResourceScope::Png,
            br#"{"jpeg":{"quality":85}}"# as &[u8],
            "jpeg",
        ),
        (
            BindingResourceScope::Pdf,
            br#"{"raster":{"scale":2}}"#,
            "raster",
        ),
        (
            BindingResourceScope::Svg,
            br#"{"pdf":{"background":"white"}}"#,
            "pdf",
        ),
    ] {
        let error = resolve_request_options(b"", request, scope).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains(group), "{error:?}");
    }
}

#[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
#[test]
fn uncompiled_output_option_groups_are_rejected() {
    let error = parse_options(br#"{"raster":{"scale":2}}"#).unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("not exposed by target"));
}

#[test]
fn render_payload_json_returns_svg_or_error_shape() {
    let payload = render_payload_json_bytes(BindingStatus::Ok, None, Some("<svg/>"));
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["version"], 1);
    assert_eq!(json["ok"], true);
    assert_eq!(json["code"], BindingStatus::Ok.code());
    assert_eq!(json["code_name"], BindingStatus::Ok.code_name());
    assert!(json["message"].is_null());
    assert_eq!(json["svg"], "<svg/>");

    let payload =
        render_payload_json_bytes(BindingStatus::RenderError, Some("render failed"), None);
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["version"], 1);
    assert_eq!(json["ok"], false);
    assert_eq!(json["code"], BindingStatus::RenderError.code());
    assert_eq!(json["code_name"], BindingStatus::RenderError.code_name());
    assert_eq!(json["message"], "render failed");
    assert!(json["svg"].is_null());
}

#[test]
fn request_options_deeply_override_engine_options() {
    let options = resolve_request_options(
        br#"{
            "version": 4,
            "parse": { "suppress_errors": false },
            "resources": {
                "profile": "interactive",
                "limits": {
                    "max_source_bytes": 4096,
                    "max_model_items": 128
                }
            }
        }"#,
        br#"{
            "parse": { "suppress_errors": true },
            "resources": {
                "limits": { "max_source_bytes": 2048 }
            }
        }"#,
        BindingResourceScope::Model,
    )
    .unwrap();
    assert_eq!(options.version, Some(BINDING_OPTIONS_SCHEMA_VERSION));
    assert_eq!(
        options.parse.and_then(|parse| parse.suppress_errors),
        Some(true)
    );
    let resources = options.analysis.resources.unwrap();
    assert_eq!(resources.profile.as_deref(), Some("interactive"));
    assert_eq!(resources.limits.get("max_source_bytes"), Some(&2048));
    assert_eq!(resources.limits.get("max_model_items"), Some(&128));
}

#[test]
fn request_options_cannot_select_runtime_policy() {
    let error = resolve_request_options(
        br#"{"runtime_policy":"deterministic"}"#,
        br#"{"runtime_policy":"native"}"#,
        BindingResourceScope::Model,
    )
    .unwrap_err();

    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("cannot set runtime_policy"));
}

#[test]
fn wrapped_request_options_cannot_hide_runtime_policy() {
    for wrapper in ["analysis", "merman"] {
        let request = format!(r#"{{"{wrapper}":{{"runtime_policy":"native"}}}}"#);
        let error = resolve_request_options(
            b"",
            request.as_bytes(),
            BindingResourceScope::AnalysisDiagram,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("cannot set runtime_policy"));
    }
}

#[test]
fn request_options_normalize_wrapped_analysis_before_merging() {
    let options = resolve_request_options(
        br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#,
        br#"{"analysis":{"fixed_today":"2026-08-01","resources":{"limits":{"max_source_bytes":2048}}}}"#,
        BindingResourceScope::AnalysisDiagram,
    )
    .unwrap();
    assert_eq!(
        options
            .analysis
            .resources
            .as_ref()
            .and_then(|resources| resources.limits.get("max_source_bytes")),
        Some(&2048)
    );
    assert_eq!(options.analysis.fixed_today.as_deref(), Some("2026-08-01"));
}

#[cfg(feature = "analysis")]
#[test]
fn analysis_document_limit_tightens_without_entering_core_policy() {
    let base = br#"{
        "resources": {
            "profile": "interactive",
            "limits": {
                "max_source_bytes": 4096,
                "max_document_diagrams": 32
            }
        }
    }"#;
    let options = resolve_request_options(
        base,
        br#"{"resources":{"limits":{"max_document_diagrams":16}}}"#,
        BindingResourceScope::DocumentAnalysis,
    )
    .unwrap();
    let resources = options.analysis.resources.as_ref().unwrap();
    assert_eq!(
        resources
            .limits
            .get(merman_analysis::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID),
        Some(&16)
    );
    assert_eq!(
        binding_input_resource_policy(Some(resources))
            .unwrap()
            .value(merman::model::resources::InputResourceLimitId::MaxSourceBytes),
        Some(4096)
    );

    #[cfg(feature = "analysis")]
    assert_eq!(
        artifact_analysis_options(&options)
            .unwrap()
            .max_document_diagrams(),
        Some(16)
    );

    let error = resolve_request_options(
        base,
        br#"{"resources":{"limits":{"max_document_diagrams":33}}}"#,
        BindingResourceScope::DocumentAnalysis,
    )
    .unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("max_document_diagrams"));
}

#[cfg(feature = "analysis")]
#[test]
fn analysis_document_limit_is_not_available_to_single_diagram_requests() {
    let error = resolve_request_options(
        b"",
        br#"{"resources":{"limits":{"max_document_diagrams":16}}}"#,
        BindingResourceScope::AnalysisDiagram,
    )
    .unwrap_err();

    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(error.message().contains("single-diagram analysis"));
}

#[cfg(feature = "analysis")]
#[test]
fn analysis_document_limit_accepts_zero() {
    let options = resolve_request_options(
        br#"{"resources":{"profile":"interactive"}}"#,
        br#"{"resources":{"limits":{"max_document_diagrams":0}}}"#,
        BindingResourceScope::DocumentAnalysis,
    )
    .unwrap();

    assert_eq!(
        artifact_analysis_options(&options)
            .unwrap()
            .max_document_diagrams(),
        Some(0)
    );
}

#[cfg(feature = "analysis")]
#[test]
fn analysis_document_limit_uses_the_selected_profile() {
    let options = parse_options(br#"{"resources":{"profile":"constrained"}}"#).unwrap();

    assert_eq!(
        artifact_analysis_options(&options)
            .unwrap()
            .max_document_diagrams(),
        Some(128)
    );
}

#[test]
fn request_options_normalize_wrapped_engine_options_before_merging() {
    let options = resolve_request_options(
        br#"{"merman":{"resources":{"profile":"interactive","limits":{"max_source_bytes":4096}}}}"#,
        br#"{"resources":{"limits":{"max_source_bytes":2048,"max_model_items":128}}}"#,
        BindingResourceScope::Model,
    )
    .unwrap();
    let resources = options.analysis.resources.unwrap();
    assert_eq!(resources.profile.as_deref(), Some("interactive"));
    assert_eq!(resources.limits.get("max_source_bytes"), Some(&2048));
    assert_eq!(resources.limits.get("max_model_items"), Some(&128));
}

#[test]
fn options_reject_ambiguous_analysis_wrappers_at_the_input_boundary() {
    for input in [
        br#"{"merman":{},"analysis":{}}"#.as_slice(),
        br#"{"merman":{"fixed_today":"2025-01-01"},"analysis":{}}"#.as_slice(),
        br#"{"merman":{},"analysis":{"fixed_today":"2025-01-01"}}"#.as_slice(),
    ] {
        let error = parse_options(input).expect_err("dual wrappers are ambiguous");
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error
                .message()
                .contains("must not contain both `analysis` and `merman` wrappers"),
            "unexpected error: {error:?}"
        );
    }
}

#[cfg(feature = "ascii")]
#[test]
fn request_ascii_alias_spelling_collisions_remain_duplicate_field_errors() {
    for (base, request, field) in [
        (
            br#"{"ascii":{"graph_presentation":"strict"}}"#.as_slice(),
            br#"{"ascii":{"graphPresentation":"adaptive"}}"#.as_slice(),
            "graph_presentation",
        ),
        (
            br#"{"ascii":{"graphPresentation":"adaptive"}}"#.as_slice(),
            br#"{"ascii":{"graph_presentation":"strict"}}"#.as_slice(),
            "graph_presentation",
        ),
        (
            br#"{"ascii":{"default_direction":"leftToRight"}}"#.as_slice(),
            br#"{"ascii":{"defaultDirection":"topDown"}}"#.as_slice(),
            "default_direction",
        ),
        (
            br#"{"ascii":{"colorMode":"none"}}"#.as_slice(),
            br#"{"ascii":{"color_mode":"ansi"}}"#.as_slice(),
            "color_mode",
        ),
        (
            br#"{"ascii":{"colorMode":null}}"#.as_slice(),
            br#"{"ascii":{"color_mode":"none"}}"#.as_slice(),
            "color_mode",
        ),
    ] {
        let error = resolve_request_options(base, request, BindingResourceScope::Model)
            .expect_err("different wire spellings survive recursive object merge");

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error
                .message()
                .contains(&format!("duplicate field `{field}`")),
            "field={field}, error={error:?}"
        );
    }

    let options = resolve_request_options(
        br#"{"ascii":{"colorMode":null}}"#,
        br#"{"ascii":{"colorMode":"ansi"}}"#,
        BindingResourceScope::Model,
    )
    .expect("the same raw key replaces its null base value");
    assert_eq!(
        options
            .ascii
            .as_ref()
            .and_then(|ascii| ascii.color_mode.as_deref()),
        Some("ansi")
    );
}

#[cfg(feature = "ascii")]
#[test]
fn request_ascii_alias_collision_uses_merged_wire_order() {
    let base = br#"{
        "ascii": {
            "default_direction": "leftToRight",
            "color_mode": "none"
        }
    }"#;

    for (request, first_duplicate) in [
        (
            br#"{"ascii":{"colorMode":"ansi","defaultDirection":"topDown"}}"#.as_slice(),
            "color_mode",
        ),
        (
            br#"{"ascii":{"defaultDirection":"topDown","colorMode":"ansi"}}"#.as_slice(),
            "default_direction",
        ),
    ] {
        let error = resolve_request_options(base, request, BindingResourceScope::Model)
            .expect_err("both aliases collide after merge");

        assert!(
            error
                .message()
                .contains(&format!("duplicate field `{first_duplicate}`")),
            "expected first duplicate {first_duplicate}, error={error:?}"
        );
    }
}

#[cfg(feature = "ascii")]
#[test]
fn request_ascii_nested_theme_alias_error_precedes_later_top_level_alias_error() {
    let error = resolve_request_options(
        br##"{
            "ascii": {
                "color_mode": "none",
                "theme": {
                    "foreground": "#ffffff",
                    "background": "#000000"
                }
            }
        }"##,
        br##"{
            "ascii": {
                "colorMode": "ansi",
                "theme": { "fg": "#eeeeee" }
            }
        }"##,
        BindingResourceScope::Model,
    )
    .expect_err("theme and top-level aliases both collide");

    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(
        error.message().contains("duplicate field `foreground`"),
        "the in-place theme object is visited before the appended colorMode key: {error:?}"
    );
}

#[cfg(feature = "ascii")]
#[test]
fn request_resource_ceiling_error_precedes_cross_document_alias_error() {
    let error = resolve_request_options(
        br#"{
            "resources": {
                "profile": "constrained",
                "limits": { "max_source_bytes": 64 }
            },
            "ascii": { "color_mode": "none" }
        }"#,
        br#"{
            "resources": { "limits": { "max_source_bytes": 65 } },
            "ascii": { "colorMode": "ansi" }
        }"#,
        BindingResourceScope::Model,
    )
    .expect_err("resource tightening runs before merged options are reparsed");

    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(
        error.message().contains("loosen the transport ceiling"),
        "unexpected error: {error:?}"
    );
    assert!(!error.message().contains("duplicate field"));
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[test]
fn parse_options_accepts_analysis_wrapper_without_dropping_binding_options() {
    #[cfg(feature = "svg")]
    let input = br#"{
            "parse": { "suppress_errors": true },
            "analysis": {
                "resources": { "limits": { "max_source_bytes": 4 } }
            },
            "version": 4,
            "svg": { "pipeline": "resvg-safe" }
        }"#;
    #[cfg(all(not(feature = "svg"), feature = "ascii"))]
    let input = br#"{
            "parse": { "suppress_errors": true },
            "analysis": {
                "resources": { "limits": { "max_source_bytes": 4 } }
            },
            "version": 4,
            "ascii": { "color_mode": "none" }
        }"#;
    let options = parse_options(input).unwrap();

    assert_eq!(options.version, Some(BINDING_OPTIONS_SCHEMA_VERSION));
    assert_eq!(
        options
            .parse
            .as_ref()
            .and_then(|parse| parse.suppress_errors),
        Some(true)
    );
    assert_eq!(
        options
            .analysis
            .resources
            .as_ref()
            .and_then(|resources| resources.limits.get("max_source_bytes")),
        Some(&4)
    );
    #[cfg(feature = "svg")]
    assert_eq!(
        binding_resource_policy(options.analysis.resources.as_ref())
            .unwrap()
            .value(merman::svg::ResourceLimitId::MaxSourceBytes),
        Some(4)
    );
    #[cfg(feature = "svg")]
    assert_eq!(
        options.svg.as_ref().and_then(|svg| svg.pipeline.as_deref()),
        Some("resvg-safe")
    );
    #[cfg(all(not(feature = "svg"), feature = "ascii"))]
    assert_eq!(
        options
            .ascii
            .as_ref()
            .and_then(|ascii| ascii.color_mode.as_deref()),
        Some("none")
    );
}

#[test]
fn options_schema_v4_rejects_published_legacy_versions() {
    for options in [
        br#"{ "version": 2 }"#.as_slice(),
        br#"{ "version": 3 }"#,
        br#"{ "version": 3, "theme": { "recipe": {} } }"#,
    ] {
        let error = parse_options(options).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("expected 4"));
    }
}

#[test]
fn options_schema_v4_rejects_unknown_and_malformed_fields() {
    for input in [
        br#"{ "version": 4, "tyop": true }"#.as_slice(),
        br#"{ "version": 4, "parse": { "tyop": true } }"#.as_slice(),
    ] {
        let error = parse_options(input).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("unknown"), "{error:?}");
    }

    #[cfg(feature = "analysis")]
    {
        for input in [
            br#"{ "version": 4, "analysis": { "tyop": true } }"#.as_slice(),
            br#"{ "version": 4, "lint": { "profiel": "strict" } }"#.as_slice(),
            br#"{ "version": 4, "lint": { "rule_severities": [{ "rule_id": "merman.authoring.flowchart.explicit_direction", "severity": "warning", "tyop": true }] } }"#.as_slice(),
        ] {
            parse_options(input)
                .expect("analysis-owned forward-compatible fields must be ignored");
        }
    }

    for input in [
        br#"{ "version": "4" }"#.as_slice(),
        br#"{ "version": 4.0 }"#.as_slice(),
        br#"{ "version": -1 }"#.as_slice(),
        br#"{ "version": 4294967296 }"#.as_slice(),
    ] {
        let error = parse_options(input).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("unsigned 32-bit integer"));
    }

    let flat = parse_options(br#"{ "resources": { "max_source_bytes": 4 } }"#).unwrap_err();
    assert_eq!(flat.status(), BindingStatus::OptionsJsonError);
    assert!(flat.message().contains("unknown field `max_source_bytes`"));
}

#[test]
fn source_order_selects_the_first_error_within_each_options_phase() {
    for (json, expected) in [
        (
            r#"{"z_first":0,"a_second":0}"#,
            "unknown options_json field `z_first`",
        ),
        (
            r#"{"runtime_policy":"invalid","parse":{"a_invalid":0}}"#,
            "unknown variant `invalid`",
        ),
        (
            r#"{"a_top":0,"parse":{"z_first":0,"a_second":0}}"#,
            "unknown field `z_first`",
        ),
        (
            r#"{"parse":{"suppress_errors":true,"a_unknown":0,"\u0073uppress_errors":"last"}}"#,
            "invalid type: string \"last\", expected a boolean",
        ),
    ] {
        let raw = parse_options(json.as_bytes()).unwrap_err();
        assert_eq!(raw.status(), BindingStatus::OptionsJsonError);
        assert!(raw.message().contains(expected), "{raw:?}");
    }

    let json = r#"{"parse":{"z_first":0,"a_second":0}}"#;
    let raw = parse_request_overlay(json.as_bytes(), BindingResourceScope::Model).unwrap_err();
    let decoded = DecodedOperationOptions::decode(json, |error| {
        BindingError::invalid_options_json(error.to_string())
    })
    .unwrap();
    let decoded = parse_request_overlay_for_artifact(
        BindingRequestOptions::Decoded(decoded),
        BindingResourceScope::Model,
        &DEFAULT_ARTIFACT_SNAPSHOT,
    )
    .unwrap_err();
    assert_eq!(decoded, raw);
    assert!(decoded.message().contains("unknown field `z_first`"));
}

#[cfg(feature = "analysis")]
#[test]
fn source_order_reaches_nested_analysis_validation() {
    let json = r#"{"analysis":{"lint":{"rule_severities":[{"severity":"fatal","rule_id":"bogus"}],"profile":"bogus"},"fixed_today":false}}"#;
    let raw_error = parse_options(json.as_bytes()).unwrap_err();
    assert_eq!(raw_error.status(), BindingStatus::InvalidArgument);
    assert!(
        raw_error
            .message()
            .contains("lint.rule_severities[].severity")
    );

    let decoded = DecodedOperationOptions::decode(json, |error| {
        BindingError::invalid_options_json(error.to_string())
    })
    .unwrap();
    let error = parse_request_overlay_for_artifact(
        BindingRequestOptions::Decoded(decoded),
        BindingResourceScope::Model,
        &DEFAULT_ARTIFACT_SNAPSHOT,
    )
    .unwrap_err();
    assert_eq!(error, raw_error);
}

#[test]
fn source_order_for_resource_shape_precedes_stable_id_budget_checks() {
    let malformed =
        br#"{"version":3,"analysis":{"resources":{"limits":{"z_first":"wrong","a_second":[]}}}}"#;
    let error =
        validate_one_shot_resource_options(malformed, BindingResourceScope::Model).unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(
        error
            .message()
            .contains("invalid request options_json resources")
    );
    assert!(error.message().contains("string \"wrong\""));

    let error = apply_resource_ceiling_json(malformed, "constrained", &[]).unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("invalid options_json resources"));
    assert!(error.message().contains("string \"wrong\""));

    let error = apply_resource_ceiling_json(
        br#"{"resources":{"limits":{"max_source_bytes":5,"max_model_items":5}}}"#,
        "constrained",
        &[("max_source_bytes", 4), ("max_model_items", 4)],
    )
    .unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("ceiling for `max_model_items`"));
}

#[test]
fn source_order_reports_the_first_unsafe_theme_variable_in_a_wrapper() {
    let error = parse_options(
        br#"{"parse":{"suppress_errors":"wrong"},"merman":{"site_config":{"themeVariables":{"z_first":[{"stroke":"red;"}],"a_second":"blue;"}}}}"#,
    )
    .unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(
        error
            .message()
            .contains("site_config.themeVariables.z_first[0].stroke")
    );
}

#[cfg(feature = "svg")]
#[test]
fn source_order_for_prepared_output_uses_the_normalized_slots() {
    let overlay = parse_request_overlay(
        br#"{"resources":{},"site_config":{},"version":4,"parse":{},"fixed_today":"2026-11-25"}"#,
        BindingResourceScope::Svg,
    )
    .unwrap();
    let error = overlay.validate_prepared_output().unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(
        error
            .message()
            .contains("prepared output cannot set `fixed_today`")
    );
}

#[test]
fn options_schema_v4_materializes_the_current_version_when_omitted() {
    let explicit = parse_options(br#"{ "version": 4 }"#).unwrap();
    assert_eq!(explicit.version, Some(BINDING_OPTIONS_SCHEMA_VERSION));

    let implicit = parse_options(br#"{}"#).unwrap();
    assert_eq!(implicit.version, Some(BINDING_OPTIONS_SCHEMA_VERSION));
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn options_schema_v4_accepts_current_theme_and_export_paint_fields() {
    let options = parse_options(
        br#"{
            "version": 4,
            "theme": { "preset": "studio" },
            "raster": { "matte": "white" },
            "pdf": { "page_paint": "transparent" }
        }"#,
    )
    .unwrap();

    assert_eq!(options.version, Some(BINDING_OPTIONS_SCHEMA_VERSION));
    assert_eq!(
        options.raster.and_then(|raster| raster.matte),
        Some("white".into())
    );
    assert_eq!(
        options.pdf.and_then(|pdf| pdf.page_paint),
        Some("transparent".into())
    );
}

#[cfg(feature = "svg")]
#[test]
fn theme_null_and_omission_leave_the_outer_theme_unset() {
    for input in [b"{}".as_slice(), br#"{"theme":null}"#.as_slice()] {
        assert!(parse_options(input).unwrap().theme.is_none());
    }
    let error = parse_options(br#"{"version":2,"theme":{"preset":false}}"#)
        .expect_err("options schema admission precedes theme decoding");
    assert!(
        error
            .message()
            .contains("unsupported options_json schema version")
    );
}

#[cfg(feature = "svg")]
#[test]
fn options_schema_v4_rejects_removed_presentation_and_raw_css_fields() {
    for (input, rejected) in [
        (
            br#"{"version":4,"presentation":{}}"#.as_slice(),
            "presentation",
        ),
        (
            br#"{"version":4,"site_config":{"themeCSS":".node {}"}}"#.as_slice(),
            "site_config.themeCSS",
        ),
        (
            br#"{"version":4,"svg":{"scoped_css":".node {}"}}"#.as_slice(),
            "svg.scoped_css",
        ),
    ] {
        let error = parse_options(input).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains(rejected), "{input:?}: {error:?}");
    }
}

#[test]
fn options_schema_v4_rejects_groups_not_compiled_into_the_artifact() {
    for (group, compiled, input) in [
        (
            "lint",
            cfg!(feature = "analysis"),
            br#"{"version":4,"lint":{"profile":"strict"}}"#.as_slice(),
        ),
        (
            "lint",
            cfg!(feature = "analysis"),
            br#"{"version":4,"analysis":{"lint":{"profile":"strict"}}}"#.as_slice(),
        ),
        (
            "ascii",
            cfg!(feature = "ascii"),
            br#"{"version":4,"ascii":{}}"#.as_slice(),
        ),
        (
            "theme",
            cfg!(any(feature = "svg", feature = "ascii")),
            br#"{"version":4,"theme":{"preset":"studio"}}"#.as_slice(),
        ),
        (
            "layout",
            cfg!(feature = "svg"),
            br#"{"version":4,"layout":{}}"#.as_slice(),
        ),
        (
            "environment",
            cfg!(feature = "svg"),
            br#"{"version":4,"environment":{}}"#.as_slice(),
        ),
        (
            "svg",
            cfg!(feature = "svg"),
            br#"{"version":4,"svg":{}}"#.as_slice(),
        ),
        (
            "raster",
            cfg!(any(feature = "png", feature = "jpeg")),
            br#"{"version":4,"raster":{}}"#.as_slice(),
        ),
        (
            "jpeg",
            cfg!(feature = "jpeg"),
            br#"{"version":4,"jpeg":{}}"#.as_slice(),
        ),
        (
            "pdf",
            cfg!(feature = "pdf"),
            br#"{"version":4,"pdf":{}}"#.as_slice(),
        ),
    ] {
        if compiled {
            continue;
        }
        let error = parse_options(input).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains(group), "{error:?}");
        assert!(error.message().contains("not available"), "{error:?}");
    }
}

#[cfg(feature = "analysis")]
#[test]
fn resource_ceiling_preserves_stricter_limits_and_wrapper_shape() {
    for wrapper in [None, Some("analysis"), Some("merman")] {
        let analysis = serde_json::json!({
            "fixed_today": "2026-08-01",
            "fixed_local_offset_minutes": 60,
            "site_config": { "theme": "dark" },
            "lint": { "profile": "recommended" },
            "resources": { "limits": {
                "max_source_bytes": 4096,
                "max_document_diagrams": 64,
                "max_model_items": 1
            } }
        });
        let mut input = wrapper
            .map(|wrapper| serde_json::json!({ wrapper: analysis.clone() }))
            .unwrap_or(analysis);
        input["version"] = Value::from(BINDING_OPTIONS_SCHEMA_VERSION);
        input["parse"] = serde_json::json!({ "suppress_errors": true });
        let input = serde_json::to_vec(&input).unwrap();
        let constrained = apply_resource_ceiling_json(&input, "constrained", &[]).unwrap();
        let value: Value = serde_json::from_slice(&constrained).unwrap();
        let resources = wrapper
            .map(|wrapper| &value[wrapper]["resources"])
            .unwrap_or(&value["resources"]);

        assert_eq!(resources["profile"], "constrained");
        assert_eq!(resources["limits"]["max_source_bytes"], 4096);
        assert_eq!(resources["limits"]["max_document_diagrams"], 64);
        assert_eq!(resources["limits"]["max_model_items"], 1);
        assert_eq!(value["version"], BINDING_OPTIONS_SCHEMA_VERSION);
        assert_eq!(value["parse"]["suppress_errors"], true);
        assert!(value.get("resources").is_some() == wrapper.is_none());
        parse_options(&constrained).unwrap();

        let typed = analysis_options_with_resource_ceiling(&input, "constrained", &[]).unwrap();
        assert_eq!(typed.max_source_bytes(), Some(4096));
        assert_eq!(typed.max_document_diagrams(), Some(64));
        assert_eq!(typed.site_config().unwrap().as_value()["theme"], "dark");
        assert_eq!(
            typed.rule_config().profile(),
            merman_analysis::AnalysisRuleProfile::Recommended
        );
        assert_eq!(
            typed.runtime_policy().fixed_local_offset_minutes(),
            Some(60)
        );
        let operation = typed.runtime_policy().begin_operation().unwrap();
        let today = operation.today_local();
        assert_eq!((today.year(), today.month(), today.day()), (2026, 8, 1));
        assert!(operation.today_is_fixed());
    }
}

#[cfg(feature = "analysis")]
#[test]
fn analysis_resource_ceiling_projects_default_stricter_and_zero_limits() {
    for (input, source_bytes, document_diagrams) in [
        (b"".as_slice(), 2 * 1024 * 1024, 256),
        (br#"{"resources":{}}"#.as_slice(), 2 * 1024 * 1024, 256),
        (
            br#"{"resources":{"profile":null,"limits":{}}}"#.as_slice(),
            2 * 1024 * 1024,
            256,
        ),
        (
            br#"{"resources":{"profile":"constrained"}}"#.as_slice(),
            1024 * 1024,
            128,
        ),
        (
            br#"{"merman":{"resources":{"limits":{"max_source_bytes":1,"max_document_diagrams":0}}}}"#.as_slice(),
            1,
            0,
        ),
    ] {
        let options = analysis_options_with_resource_ceiling(input, "interactive", &[]).unwrap();
        assert_eq!(options.max_source_bytes(), Some(source_bytes));
        assert_eq!(options.max_document_diagrams(), Some(document_diagrams));
    }

    let options = analysis_options_with_resource_ceiling(
        br#"{"analysis":{"resources":{"limits":{"max_document_diagrams":0}}}}"#,
        "interactive",
        &[("max_source_bytes", 32), ("max_document_diagrams", 2)],
    )
    .unwrap();
    assert_eq!(options.max_source_bytes(), Some(32));
    assert_eq!(options.max_document_diagrams(), Some(0));
}

#[cfg(feature = "analysis")]
#[test]
fn analysis_resource_ceiling_preserves_binding_error_status_and_order() {
    for (input, status, message) in [
        (
            b"\xff".as_slice(),
            BindingStatus::Utf8Error,
            "invalid options_json UTF-8",
        ),
        (
            br#"{"analysis":[],"resources":{"profile":"trusted-native"}}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "`analysis` wrapper must be an object",
        ),
        (
            br#"{"analysis":{},"merman":{},"version":2}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "must not contain both `analysis` and `merman` wrappers",
        ),
        (
            br#"{"analysis":{},"resources":{"profile":"missing"}}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "must not mix top-level resources",
        ),
        (
            br#"{"analysis":{},"fixed_today":"bad"}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "must not mix top-level analysis options",
        ),
        (
            br#"{"version":2,"resources":{"limits":{"max_source_bytes":2097153}}}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "loosen the transport ceiling",
        ),
        (
            br#"{"version":2,"fixed_today":"bad"}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "unsupported options_json schema version",
        ),
        (
            br#"{"merman":{"parse":{"suppress_errors":true}}}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "analysis option `parse` was removed",
        ),
        (
            br#"{"parse":{"suppress_errors":"yes"},"fixed_today":"bad"}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "expected a boolean",
        ),
        (
            br#"{"resources":{"max_source_bytes":1}}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "unknown field `max_source_bytes`",
        ),
        (
            br#"{"resources":{"limits":null}}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "invalid options_json resources",
        ),
        (
            br#"{"resources":{"limits":{"max_source_bytes":0}}}"#.as_slice(),
            BindingStatus::InvalidArgument,
            "resources.limits.max_source_bytes must be at least 1",
        ),
        (
            br#"{"resources":{"profile":"Constrained"}}"#.as_slice(),
            BindingStatus::InvalidArgument,
            "unsupported resources.profile: Constrained",
        ),
        (
            br#"{"resources":{"limits":{"unknown_limit":1}}}"#.as_slice(),
            BindingStatus::InvalidArgument,
            "resource limit id `unknown_limit`",
        ),
    ] {
        let typed_error =
            analysis_options_with_resource_ceiling(input, "interactive", &[]).unwrap_err();
        let wire_error = apply_resource_ceiling_json(input, "interactive", &[]).unwrap_err();
        assert_eq!(typed_error.status(), status, "input={input:?}");
        assert!(typed_error.message().contains(message), "{typed_error:?}");
        assert_eq!(wire_error.status(), status, "input={input:?}");
        assert_eq!(
            typed_error.message(),
            wire_error.message(),
            "input={input:?}"
        );
    }

    for input in [
        br#"{"resources":{"limits":{"max_source_bytes":33}}}"#.as_slice(),
        br#"{"analysis":{"resources":{"profile":"constrained","limits":{"max_source_bytes":33}}}}"#
            .as_slice(),
    ] {
        let error = analysis_options_with_resource_ceiling(
            input,
            "interactive",
            &[("max_source_bytes", 32)],
        )
        .unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("requested 33, maximum 32"));
    }
}

#[cfg(feature = "analysis")]
#[test]
fn resource_ceiling_json_enforces_the_normalized_wire_byte_budget() {
    let prefix = r#"{"site_config":{"padding":""#;
    let suffix = r#""}}"#;
    let input = format!(
        "{prefix}{}{suffix}",
        "x".repeat(BINDING_OPTIONS_JSON_MAX_BYTES - prefix.len() - suffix.len())
    );
    assert_eq!(input.len(), BINDING_OPTIONS_JSON_MAX_BYTES);

    let error = apply_resource_ceiling_json(input.as_bytes(), "interactive", &[]).unwrap_err();
    assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
    assert_eq!(
        error.resource_details().unwrap().limit_id,
        BINDING_OPTIONS_JSON_LIMIT_ID
    );
    let options =
        analysis_options_with_resource_ceiling(input.as_bytes(), "interactive", &[]).unwrap();
    assert_eq!(options.max_source_bytes(), Some(2 * 1024 * 1024));
    assert_eq!(options.max_document_diagrams(), Some(256));
}

#[test]
fn resource_ceiling_rejects_a_looser_profile_hidden_by_generic_overrides() {
    let constrained = ResourceOptionsJson {
        profile: Some("constrained".to_string()),
        limits: Default::default(),
    };
    let requested = ResourceOptionsJson {
        profile: Some("interactive".to_string()),
        limits: effective_resource_limits(&constrained)
            .unwrap()
            .into_iter()
            .filter_map(|(id, value)| value.map(|value| (id.to_string(), value)))
            .collect(),
    };

    let error = tighten_resource_options(&constrained, &requested).unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("requested interactive"));
    assert!(error.message().contains("ceiling constrained"));
}

#[test]
fn resource_ceiling_rejects_looser_profiles_and_overrides() {
    for input in [
        br#"{"resources":{"profile":"trusted-native"}}"#.as_slice(),
        br#"{"resources":{"limits":{"max_source_bytes":2097152}}}"#.as_slice(),
    ] {
        let error = apply_resource_ceiling_json(input, "constrained", &[]).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("loosen the transport ceiling"));
    }
}

#[test]
fn resource_ceiling_rejects_ambiguous_or_malformed_wrappers() {
    for input in [
        br#"{"analysis":{},"merman":{}}"#.as_slice(),
        br#"{"analysis":[]}"#.as_slice(),
        br#"{"merman":null}"#.as_slice(),
        br#"{"resources":{"profile":"constrained"},"analysis":{}}"#.as_slice(),
    ] {
        let error = apply_resource_ceiling_json(input, "constrained", &[]).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    }
}

#[cfg(feature = "svg")]
#[test]
fn resource_ceiling_allowed_overrides_preserve_unspecified_defaults() {
    for input in [
        b"".as_slice(),
        br#"{}"#.as_slice(),
        br#"{"resources":{}}"#.as_slice(),
        br#"{"resources":{"profile":null}}"#.as_slice(),
    ] {
        let normalized = apply_resource_ceiling_json_with_allowed_overrides(
            input,
            "constrained",
            &[],
            &["max_layout_work_units"],
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&normalized).unwrap();
        assert_eq!(
            value["resources"],
            serde_json::json!({ "profile": "constrained" })
        );
        let options = parse_options_value(&value).unwrap();
        assert_eq!(
            effective_resource_limits(options.analysis.resources.as_ref().unwrap()).unwrap()["max_layout_work_units"],
            Some(125_000)
        );
    }
}

#[cfg(feature = "svg")]
#[test]
fn resource_ceiling_allowed_layout_overrides_preserve_wrappers_and_other_limits() {
    for wrapper in [None, Some("analysis"), Some("merman")] {
        for limit in [1, 125_000, 1_000_000] {
            let analysis = serde_json::json!({ "resources": { "limits": {
                "max_layout_work_units": limit,
                "max_source_bytes": 4096
            } } });
            let input = wrapper
                .map(|wrapper| serde_json::json!({ wrapper: analysis.clone() }))
                .unwrap_or(analysis);
            let normalized = apply_resource_ceiling_json_with_allowed_overrides(
                &serde_json::to_vec(&input).unwrap(),
                "constrained",
                &[],
                &["max_layout_work_units"],
            )
            .unwrap();
            let value: Value = serde_json::from_slice(&normalized).unwrap();
            let resources = wrapper
                .map(|wrapper| &value[wrapper]["resources"])
                .unwrap_or(&value["resources"]);
            assert_eq!(resources["profile"], "constrained");
            assert_eq!(
                resources["limits"],
                serde_json::json!({
                    "max_layout_work_units": limit,
                    "max_source_bytes": 4096
                })
            );
            assert_eq!(value.get("resources").is_some(), wrapper.is_none());
        }
    }
}

#[test]
fn resource_ceiling_allowlist_cannot_bypass_resource_validation() {
    for (id, value, message) in [
        ("unknown_limit", 1, "not part of resource contract"),
        (BINDING_OPTIONS_JSON_LIMIT_ID, 1, "not overridable"),
        ("max_source_bytes", 0, "must be at least 1"),
    ] {
        let input = serde_json::json!({ "resources": { "limits": { id: value } } });
        let error = apply_resource_ceiling_json_with_allowed_overrides(
            &serde_json::to_vec(&input).unwrap(),
            "constrained",
            &[],
            &[id],
        )
        .unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains(message), "{error:?}");
    }
}

#[test]
fn resource_ceiling_allowed_overrides_keep_raw_options_byte_admission() {
    let error = apply_resource_ceiling_json_with_allowed_overrides(
        &vec![b' '; BINDING_OPTIONS_JSON_MAX_BYTES + 1],
        "constrained",
        &[],
        &["max_layout_work_units"],
    )
    .unwrap_err();
    assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
    assert_eq!(
        error.resource_details().unwrap().limit_id,
        BINDING_OPTIONS_JSON_LIMIT_ID
    );
}

#[cfg(not(feature = "svg"))]
#[test]
fn resource_ceiling_allowlist_rejects_uncompiled_layout_limits() {
    let error = apply_resource_ceiling_json_with_allowed_overrides(
        br#"{"resources":{"limits":{"max_layout_work_units":1000000}}}"#,
        "constrained",
        &[],
        &["max_layout_work_units"],
    )
    .unwrap_err();
    assert_eq!(error.status(), BindingStatus::InvalidArgument);
    assert!(error.message().contains("not part of resource contract"));
}

#[cfg(feature = "svg")]
#[test]
fn original_resource_ceiling_helpers_still_reject_raised_layout_limits() {
    let input = br#"{"resources":{"limits":{"max_layout_work_units":125001}}}"#;
    let error = apply_resource_ceiling_json(input, "constrained", &[]).unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(error.message().contains("max_layout_work_units"));
    #[cfg(feature = "analysis")]
    {
        let analysis_error =
            analysis_options_with_resource_ceiling(input, "constrained", &[]).unwrap_err();
        assert_eq!(analysis_error.status(), error.status());
        assert_eq!(analysis_error.message(), error.message());
    }
}

#[cfg(feature = "svg")]
#[test]
fn resource_options_builder_uses_the_descriptor_and_rejects_invalid_overrides() {
    let json = resource_options_json(
        Some("constrained"),
        &[("max_source_bytes", 4096), ("max_svg_bytes", 8192)],
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(value["version"], BINDING_OPTIONS_SCHEMA_VERSION);
    assert_eq!(value["resources"]["profile"], "constrained");
    assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);

    for error in [
        resource_options_json(Some("missing"), &[]).unwrap_err(),
        resource_options_json(Some("interactive"), &[("unknown_limit", 1)]).unwrap_err(),
        resource_options_json(
            Some("interactive"),
            &[("max_source_bytes", 1), ("max_source_bytes", 2)],
        )
        .unwrap_err(),
    ] {
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn export_backend_hard_caps_cannot_be_overridden() {
    let hard_cap = merman::svg::export::MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID;
    for error in [
        resource_options_json(Some("interactive"), &[(hard_cap, 1)]).unwrap_err(),
        parse_options(format!(r#"{{"resources":{{"limits":{{"{hard_cap}":1}}}}}}"#).as_bytes())
            .unwrap_err(),
    ] {
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("not overridable"));
    }
}

#[cfg(all(
    not(feature = "svg"),
    not(feature = "analysis"),
    not(feature = "ascii")
))]
#[test]
fn semantic_only_resource_options_accept_all_semantic_limits() {
    let json = resource_options_json(
        Some("constrained"),
        &[("max_source_bytes", 4096), ("max_model_items", 1)],
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);
    assert_eq!(value["resources"]["limits"]["max_model_items"], 1);
}

#[cfg(all(feature = "analysis", not(feature = "svg"), not(feature = "ascii")))]
#[test]
fn analysis_artifact_resource_options_include_semantic_model_limits() {
    let json = resource_options_json(
        Some("constrained"),
        &[("max_source_bytes", 4096), ("max_model_items", 1)],
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);
    assert_eq!(value["resources"]["limits"]["max_model_items"], 1);
}

#[cfg(all(
    not(feature = "svg"),
    not(feature = "analysis"),
    not(feature = "ascii")
))]
#[test]
fn semantic_only_build_accepts_top_level_parse_options() {
    let options = parse_options(br#"{ "parse": { "suppress_errors": true } }"#).unwrap();
    assert_eq!(
        options
            .parse
            .as_ref()
            .and_then(|parse| parse.suppress_errors),
        Some(true)
    );
}

#[test]
fn parse_options_rejects_removed_nested_analysis_parse_option() {
    for wrapper in ["analysis", "merman"] {
        let input = format!(r#"{{ "{wrapper}": {{ "parse": {{ "suppress_errors": true }} }} }}"#);
        let err = parse_options(input.as_bytes()).unwrap_err();
        assert_eq!(err.status(), BindingStatus::OptionsJsonError);
        assert!(
            err.message()
                .contains("analysis option `parse` was removed"),
            "unexpected error: {err:?}"
        );
    }
}

#[test]
fn every_binding_artifact_rejects_site_config_trust_controls() {
    for input in [
        r#"{"site_config":{"themeCSS":".node {}"}}"#,
        r#"{"site_config":{"secure":[]}}"#,
        r#"{"site_config":{"maxEdges":1000000}}"#,
        r#"{"site_config":{"securityLevel":"loose"}}"#,
        r#"{"site_config":{"dompurifyConfig":{"ADD_ATTR":["onclick"]}}}"#,
        r#"{"analysis":{"site_config":{"themeCSS":".node {}"}}}"#,
        r#"{"analysis":{"site_config":{"secure":[]}}}"#,
        r#"{"analysis":{"site_config":{"maxEdges":1000000}}}"#,
        r#"{"analysis":{"site_config":{"securityLevel":"loose"}}}"#,
        r#"{"analysis":{"site_config":{"dompurifyConfig":{"ADD_URI_SAFE_ATTR":["href"]}}}}"#,
        r#"{"merman":{"site_config":{"themeCSS":".node {}"}}}"#,
        r#"{"merman":{"site_config":{"secure":[]}}}"#,
        r#"{"merman":{"site_config":{"max_edges":1000000}}}"#,
        r#"{"merman":{"site_config":{"security_level":"loose"}}}"#,
        r#"{"merman":{"site_config":{"dompurify_config":{"ADD_ATTR":["onclick"]}}}}"#,
    ] {
        let error = parse_options(input.as_bytes()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError, "{input}");
        assert!(
            error.message().contains("site_config.themeCSS")
                || error.message().contains("site_config.secure")
                || error.message().contains("site_config.maxEdges")
                || error.message().contains("site_config.max_edges")
                || error.message().contains("site_config.securityLevel")
                || error.message().contains("site_config.security_level")
                || error.message().contains("site_config.dompurifyConfig")
                || error.message().contains("site_config.dompurify_config"),
            "{input}: {error:?}"
        );
    }
}

#[test]
fn options_json_byte_budget_is_checked_before_materialization() {
    enforce_options_json_byte_len(BINDING_OPTIONS_JSON_MAX_BYTES)
        .expect("the documented maximum must remain admissible");
    let oversized = vec![b' '; BINDING_OPTIONS_JSON_MAX_BYTES + 1];
    let error = parse_options(&oversized).expect_err("oversized options must fail closed");
    assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
    let details = error
        .resource_details()
        .expect("options byte-limit details");
    assert_eq!(details.limit_id, BINDING_OPTIONS_JSON_LIMIT_ID);
    assert_eq!(details.phase, "options-json-preflight");
    assert_eq!(details.max, BINDING_OPTIONS_JSON_MAX_BYTES as u64);
}

#[test]
fn every_binding_artifact_rejects_unsafe_site_config_theme_variables() {
    for input in [
        r##"{"site_config":{"themeVariables":{"mainBkg":"#fff;}</style><script>alert(1)</script>"}}}"##,
        r#"{"site_config":{"themeVariables":{"nodeBorder":"url(javascript:alert(1))"}}}"#,
        r#"{"analysis":{"site_config":{"themeVariables":{"fontFamily":"u\\72l(javascript:alert(1))"}}}}"#,
        r#"{"merman":{"site_config":{"themeVariables":{"nested":["red;stroke:blue"]}}}}"#,
    ] {
        let error = parse_options(input.as_bytes()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError, "{input}");
        assert!(
            error.message().contains("themeVariables")
                || error.message().contains("theme_variables"),
            "{input}: {error:?}"
        );
    }
}

#[cfg(feature = "analysis")]
#[test]
fn parse_options_accepts_merman_wrapper() {
    let options = parse_options(
        br#"{
            "merman": {
                "lint": {
                    "profile": "recommended"
                }
            }
        }"#,
    )
    .unwrap();

    assert_eq!(
        options
            .analysis
            .lint
            .as_ref()
            .and_then(|lint| lint.profile.as_deref()),
        Some("recommended")
    );
}

#[cfg(feature = "analysis")]
#[test]
fn parse_options_delegates_forward_compatible_lint_fields_to_analysis_contract() {
    let options = parse_options(
        br#"{
            "analysis": {
                "lint": {
                    "profile": "recommended",
                    "future_lint": { "enabled": true },
                    "rule_severities": [{
                        "rule_id": "merman.parse.no_diagram",
                        "severity": "warning",
                        "future_override": "accepted"
                    }]
                }
            }
        }"#,
    )
    .unwrap();

    let lint = options.analysis.lint.expect("analysis lint options");
    assert_eq!(lint.profile.as_deref(), Some("recommended"));
    assert_eq!(lint.rule_severities.len(), 1);
    assert_eq!(lint.rule_severities[0].rule_id, "merman.parse.no_diagram");
    assert_eq!(lint.rule_severities[0].severity, "warning");
}

#[test]
fn parse_options_rejects_mixed_direct_and_wrapped_analysis_options() {
    let err = parse_options(
        br#"{
            "resources": { "limits": { "max_source_bytes": 4 } },
            "analysis": {
                "resources": { "limits": { "max_model_items": 4 } }
            }
        }"#,
    )
    .unwrap_err();

    assert_eq!(err.status(), BindingStatus::OptionsJsonError);
    assert!(
        err.message()
            .contains("must not mix top-level analysis options"),
        "{err:?}"
    );
}
