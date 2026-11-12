//! Internal SPI for compact JSON serialization shared by Merman crates.
//!
//! This module is not part of the supported theme-authoring contract.

use std::io::Write;

use serde::Serialize;

/// Serializes a typed value as compact JSON, preserving Serde and I/O errors.
pub fn to_writer<T: Serialize + ?Sized>(
    writer: &mut dyn Write,
    value: &T,
) -> serde_json::Result<()> {
    serde_json::to_writer(writer, value)
}

/// Serializes a typed value to compact JSON bytes with the shared writer backend.
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<Vec<u8>> {
    // Match serde_json::to_vec's initial buffer capacity.
    let mut bytes = Vec::with_capacity(128);
    to_writer(&mut bytes, value)?;
    Ok(bytes)
}
