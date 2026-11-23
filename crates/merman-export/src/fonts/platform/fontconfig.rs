//! Fontconfig runtime and catalog availability.

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

/// A cached catalog can still name a removed file. Check only after snapshot admission
/// fails so a retained snapshot wins and existing links keep their normal source errors.
pub(super) fn source_disappeared(error: &super::NativeFontError) -> bool {
    matches!(error, super::NativeFontError::Source { path, .. }
        if path.symlink_metadata().is_err_and(|io| io.kind() == std::io::ErrorKind::NotFound))
}
