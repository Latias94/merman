use crate::text::TextStyle;

mod css_binding;
#[cfg(feature = "layout-cytoscape")]
mod fcose;
pub(crate) mod labels;
#[cfg(feature = "layout-shoal")]
mod shoal;
mod theme;

pub(crate) use css_binding::ArchitectureCssBinding;

pub(crate) use theme::{
    ArchitectureArrowSide, ArchitectureEdgeTerminal, ArchitectureEdgeTerminalEmission,
    ArchitectureGroupTerminal, ArchitectureGroupTerminalEmission, ArchitectureGroupThemePlan,
    ArchitecturePaintTerminalEmission, ArchitectureServiceTerminal,
    ArchitectureServiceTerminalEmission, ArchitectureSurfaceRenderState,
    ArchitectureTextTerminalEmission, ArchitectureThemeOutcome, ArchitectureTypographyEmission,
    ArchitectureTypographyThemePlan,
};

#[cfg(feature = "layout-cytoscape")]
pub(crate) use fcose::layout_architecture_diagram_typed;
#[cfg(feature = "layout-shoal")]
pub(crate) use shoal::{
    ArchitectureShoalGeometry, ArchitectureShoalText, layout_architecture_shoal,
};

const CYTOSCAPE_DEFAULT_FONT_FAMILY: &str = "Helvetica Neue,Helvetica,sans-serif";

pub(crate) fn architecture_cytoscape_text_style(font_size_px: f64) -> TextStyle {
    TextStyle {
        // Mermaid sets only `font-size` on Architecture nodes, so Cytoscape retains its own
        // default canvas font family rather than inheriting Mermaid's root Trebuchet stack.
        font_family: Some(CYTOSCAPE_DEFAULT_FONT_FAMILY.to_string()),
        font_size: font_size_px,
        font_weight: None,
        font_style: None,
    }
}
