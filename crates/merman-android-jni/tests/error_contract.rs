#[path = "../src/error.rs"]
mod error;

use error::binding_error_text;
use merman_bindings_core::{BindingError, BindingResourceLimitCause};

#[test]
fn android_error_wire_preserves_unsigned_resource_counts() {
    let error = BindingError::resource_limit_with_cause(
        BindingResourceLimitCause::ArithmeticOverflow,
        "layout_model",
        "max_layout_work_units",
        u64::MAX,
        800_000,
        "interactive",
        "layout work accounting overflowed",
    );

    let payload = binding_error_text(error);

    assert!(
        payload.contains(r#""actual":"18446744073709551615""#),
        "{payload}"
    );
    assert!(payload.contains(r#""max":800000"#), "{payload}");
}

#[cfg(feature = "svg")]
#[test]
fn android_materialize_theme_error_preserves_authoring_envelope() {
    let error = merman_bindings_core::execute_once(
        merman_bindings_core::BindingOperationRequest::new(
            "materialize-theme-json",
            br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":[{"kind":"rule","target":"node","style":{"typography":{"font_stack":[]}}}]}"#,
        )
    ).expect_err("invalid font stack");
    let payload = binding_error_text(error);
    for field in [
        r#""code_name":"MERMAN_INVALID_ARGUMENT""#,
        r#""theme_authoring":{"schema_version":1,"diagnostics":["#,
        r#""code":"theme-authoring.invalid-token-value""#,
        r#""path":"/styles/0/style/typography/font_stack""#,
        r#""details":{"expected_domain_id":"font-stack"}"#,
    ] {
        assert!(payload.contains(field), "missing {field}: {payload}");
    }
}
