//! Public version information for the Mermaid diagnostic diagram.

pub use merman_render_core::error_diagram::UPSTREAM_MERMAID_VERSION;
pub(crate) use merman_render_core::error_diagram::{
    ErrorTypographyThemePlan, prepare_error_diagram, render_error_diagram_svg,
};
