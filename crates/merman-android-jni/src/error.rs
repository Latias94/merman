use merman_bindings_core::BindingError;

pub(crate) fn binding_error_text(error: BindingError) -> String {
    String::from_utf8(merman_bindings_core::binding_error_js_payload_json_bytes(
        &error,
    ))
    .unwrap_or_else(|utf8_error| format!("native error was not UTF-8: {utf8_error}"))
}
