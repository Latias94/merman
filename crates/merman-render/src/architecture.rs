#[cfg(feature = "layout-shoal")]
mod shoal;

pub(crate) use merman_render_graph::architecture::ArchitectureGroupThemePlan;
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_graph::architecture::labels;
#[cfg(feature = "layout-cytoscape")]
pub(crate) use merman_render_graph::architecture::layout_architecture_diagram_typed;
#[cfg(feature = "layout-shoal")]
pub(crate) use shoal::{ArchitectureShoalGeometry, layout_architecture_shoal};
