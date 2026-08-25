//! Dagre layout pipeline.

mod compound;
mod layout;

pub(crate) use layout::rank_plan_controlled;
pub use layout::{layout, layout_controlled};
