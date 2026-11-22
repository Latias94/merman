//! Fontconfig runtime availability through the same loader used by Fontique.

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

use fontconfig_sys::statics::LIB_RESULT;

/// Whether the runtime library and all symbols required by the binding loaded.
///
/// This does not initialize a configuration or inspect fonts. A loaded runtime
/// with an intentionally empty font set is available. Loading failures include
/// an absent library, a missing dependency, and missing required symbols.
/// The binding retains its loading result for the lifetime of the process.
pub(super) fn library_available() -> bool {
    LIB_RESULT.is_ok()
}
