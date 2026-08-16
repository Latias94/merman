#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

// The canonical encoder stays private until contract-owned wire types can enforce I-JSON input.
#[cfg_attr(not(test), allow(dead_code))]
mod canonical_json;
mod version;

pub use version::{
    ThemeContractVersion, ThemeContractVersionError, authoring_version_registry,
    resolve_authoring_version,
};
