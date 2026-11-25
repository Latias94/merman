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
fn android_theme_resolution_preserves_the_document_path() {
    let result =
        merman_bindings_core::execute_once(merman_bindings_core::BindingOperationRequest::new(
            "resolve-theme",
            br#"{"document":{"schema_version":2,"seed":{"accent":"red"}}}"#,
        ))
        .unwrap();
    let payload = std::str::from_utf8(result.data()).unwrap();
    assert!(payload.contains(r#""ok":false"#), "{payload}");
    assert!(payload.contains(r#""path":"seed.accent""#), "{payload}");
}
