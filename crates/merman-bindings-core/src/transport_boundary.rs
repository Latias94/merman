//! Recoverable Rust unwinds at native transport boundaries.

use std::panic::{AssertUnwindSafe, catch_unwind};

/// Returns `None` after a Rust unwind. Recovery requires a build with panic unwinding enabled.
///
/// The caller is responsible for the unwind safety of the operation and its transport's error
/// mapping.
pub fn catch_transport_unwind<T>(operation: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(value) => Some(value),
        Err(payload) => {
            // Preserve normal payload cleanup. If its destructor panics, dropping the new
            // payload could unwind again, so deliberately leak only that second payload.
            if let Err(drop_panic) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                std::mem::forget(drop_panic);
            }
            None
        }
    }
}
