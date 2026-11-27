//! SVG compare commands and helpers.

#[cfg(feature = "render-parity")]
mod all;
#[cfg(feature = "render-parity")]
mod browser_text_layout;
mod diagrams;
mod family_lock;
#[cfg(feature = "render-parity")]
mod harness;
#[cfg(feature = "render-parity")]
mod labels;
#[cfg(feature = "render-parity")]
mod layout;
#[cfg(feature = "render-parity")]
mod parser_diagnostics;
#[cfg(feature = "render-parity")]
mod paths;
#[cfg(feature = "render-parity")]
mod root;
#[cfg(feature = "render-parity")]
mod root_contract;
#[cfg(feature = "render-parity")]
mod xml;

#[cfg(feature = "render-parity")]
pub(crate) use all::*;
#[cfg(feature = "render-parity")]
pub(crate) use browser_text_layout::*;
pub(crate) use diagrams::*;
pub(crate) use family_lock::*;
#[cfg(feature = "render-parity")]
pub(crate) use harness::*;
#[cfg(feature = "render-parity")]
pub(crate) use labels::*;
#[cfg(feature = "render-parity")]
pub(crate) use layout::*;
#[cfg(feature = "render-parity")]
pub(crate) use parser_diagnostics::*;
#[cfg(feature = "render-parity")]
pub(crate) use paths::*;
#[cfg(feature = "render-parity")]
pub(crate) use root::*;
#[cfg(feature = "render-parity")]
pub(crate) use root_contract::*;
#[cfg(feature = "render-parity")]
pub(crate) use xml::*;
