use std::cell::Cell;

use merman_bindings_core::BindingError;
use wasm_bindgen::JsValue;

use crate::binding_error_to_js;

pub(super) fn with_active<T>(
    active: &Cell<bool>,
    reentry_message: &'static str,
    run: impl FnOnce() -> Result<T, JsValue>,
) -> Result<T, JsValue> {
    if active.replace(true) {
        return Err(binding_error_to_js(BindingError::reentrant_call(
            reentry_message,
        )));
    }
    let _guard = ActiveOperation(active);
    run()
}

struct ActiveOperation<'a>(&'a Cell<bool>);

impl Drop for ActiveOperation<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
