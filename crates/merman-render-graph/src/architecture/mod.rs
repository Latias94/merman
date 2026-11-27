//! Typed layout and terminal rendering for Mermaid Architecture diagrams.

mod model;

pub use model::{
    ArchitectureCompoundBounds, ArchitectureCytoscapeServiceBounds,
    ArchitectureCytoscapeServiceLabelMetrics, ArchitectureDiagramLayout,
};

#[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
mod renderer;

#[cfg(feature = "layout-cytoscape")]
pub use renderer::layout_architecture_diagram_typed;
#[cfg(feature = "layout-shoal")]
pub use renderer::{
    ArchitectureCanonicalEdge, ArchitectureCanonicalGroup, ArchitectureCanonicalSvg,
    ArchitecturePlacedText, labels, metrics,
};
#[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
pub use renderer::{
    ArchitectureCssBinding, ArchitectureGroupThemePlan, ArchitectureNativeStyle,
    ArchitectureSvgContext, ArchitectureTypographyThemePlan,
    render_architecture_diagram_svg_typed_with_config,
};
