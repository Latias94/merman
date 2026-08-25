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
