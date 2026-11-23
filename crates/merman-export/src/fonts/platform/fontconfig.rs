//! Fontconfig runtime and catalog availability.

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

use super::NativeFontError;
use fontconfig_sys::statics::LIB_RESULT;
use std::{io::ErrorKind, path::Path};

/// Whether the runtime library and all symbols required by the binding loaded.
///
/// This does not initialize a configuration or inspect fonts. A loaded runtime
/// with an intentionally empty font set is available. Loading failures include
/// an absent library, a missing dependency, and missing required symbols.
/// The binding retains its loading result for the lifetime of the process.
pub(super) fn library_available() -> bool {
    LIB_RESULT.is_ok()
}

/// Try the rest of the catalog after a file disappears. Admission runs first so retained
/// snapshots win. Symlinks in the file path keep their source errors.
pub(super) fn defer_missing_source<T>(
    error: NativeFontError,
    first_missing: &mut Option<NativeFontError>,
) -> Result<Option<T>, NativeFontError> {
    if let NativeFontError::Source { path, .. } = &error
        && source_disappeared(path)
    {
        first_missing.get_or_insert(error);
        Ok(None)
    } else {
        Err(error)
    }
}

fn source_disappeared(path: &Path) -> bool {
    // A dangling ancestor also gives ENOENT at the leaf. Inspect the components
    // before treating the original source error as a stale catalog entry.
    let mut missing = false;
    for component in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match component.symlink_metadata() {
            Err(error) if error.kind() == ErrorKind::NotFound => missing = true,
            Ok(metadata) if missing && !metadata.is_symlink() => (),
            _ => return false,
        }
    }
    missing
}
