//! Sankey layout and family configuration.

mod config;
mod layout;
mod model;
mod svg;

pub use config::{SankeyConfigView, SankeyRenderSettings};
pub use layout::layout_sankey_diagram_typed;
pub use model::{SankeyDiagramLayout, SankeyLinkLayout, SankeyNodeLayout};
pub use svg::{SankeySvgBody, SankeySvgBodyStyle, SankeySvgLink, SankeySvgLinkPaint};
