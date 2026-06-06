#![forbid(unsafe_code)]

//! `merman` is a headless, parity-focused Mermaid implementation in Rust.
//!
//! It is pinned to Mermaid `@11.15.0`; upstream Mermaid is treated as the spec. See:
//! - `docs/adr/0014-upstream-parity-policy.md`
//! - `docs/alignment/STATUS.md`
//!
//! # Features
//!
//! - `ascii`: enable terminal/text rendering (`merman::ascii`)
//! - `render`: enable layout + SVG rendering (`merman::render`)
//! - `raster`: enable PNG/JPG/PDF output via pure-Rust SVG rasterization/conversion

pub use merman_core::*;

#[cfg(feature = "ascii")]
pub mod ascii;

#[cfg(feature = "render")]
pub mod render;
