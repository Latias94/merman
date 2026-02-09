//! Dagre layout pipelines.
//!
//! We keep the crate-level API as `dugong::layout(...)`, so this module is intentionally not
//! named `layout` to avoid a Rust item-name conflict.

mod compound;
mod dagreish;
mod minimal;

pub use dagreish::layout_dagreish;
pub use minimal::layout;
